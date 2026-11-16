use super::super::{Space, mouse, render::Render, state::*, ui::{TEXT_X, TEXT_Y}};
use mt_engine::present::{ly, pic, prim::Raster};

pub const CH: [u16; 2] = [0x0008, 0x8F00];
pub const SHAPES: u16 = 0x4996;
pub const STATION_SHAPES: u16 = 0x4622;
pub const SELECTED: u16 = 0x4724;
const TEMPORARY: u16 = 0x46F2;
const CYCLE: u16 = 0x4722;
pub const FIRST_SKILL: u16 = 0x46F6;
pub const SKILLS_DONE: u16 = 0x46F8;
const POSITIONS: u16 = 0x1C1B;
const REGIONS: u16 = 0x4656;
const REGION_SLOTS: u16 = 0x46FA;
const REGION_COUNT: u16 = 0x46F4;
const REGION_DEFAULT: u16 = 0x4720;
const WIDE_ALLOWED: u16 = 0x2004;
pub const HITS: u16 = 0x1E78;
const NUMBER: u16 = 0x486C;
const TENTHS: u16 = 0x487D;
const LONG: u16 = 0x48AF;

fn inv(c: i16, slot: i16) -> u16 { (c as u16).wrapping_mul(256).wrapping_add(0x1CE).wrapping_add((slot as u16).wrapping_mul(2)) }
pub fn wide(icon: i16) -> bool { icon >= 100 }

impl Space {
    fn item(&self, c: i16, slot: i16) -> i16 { self.s.pword(inv(c, slot)) }
    fn set_item(&mut self, c: i16, slot: i16, v: i16) { self.s.set_pword(inv(c, slot), v) }
    pub fn item_instance(&self, c: i16, slot: i16) -> Option<u16> {
        let n = self.item(c, slot);
        (n > 0).then(|| ((n - 1) as u16).wrapping_mul(5).wrapping_add(0xA82))
    }
    pub fn item_definition(&self, c: i16, slot: i16) -> Option<u16> {
        self.item_instance(c, slot).map(|at| (self.s.pbyte(at) as u16).wrapping_mul(46))
    }
    fn def_byte(&self, at: u16) -> u8 { self.s.world.get(at as usize).copied().unwrap_or(0) }
    fn def_word(&self, at: u16) -> i16 { i16::from_le_bytes([self.def_byte(at), self.def_byte(at.wrapping_add(1))]) }
    pub fn item_icon(&self, c: i16, slot: i16) -> i16 { self.item_definition(c, slot).map_or(-1, |d| self.def_byte(d.wrapping_add(0x26)) as i16) }
    pub fn item_category(&self, c: i16, slot: i16) -> i16 { self.item_definition(c, slot).map_or(-1, |d| self.def_word(d.wrapping_add(0x1D))) }
    pub fn inventory_slot_marker(&self, c: i16, slot: i16) -> bool { self.item(c, slot) == -1 }
    fn wide_allowed(&self, slot: i16) -> bool { 1u32.checked_shl(slot as u8 as u32).unwrap_or(0) & self.s.dword(WIDE_ALLOWED) != 0 }
    fn fits_wide(&self, c: i16, slot: i16) -> bool {
        self.wide_allowed(slot) && ((!self.inventory_slot_empty(c, slot) && wide(self.item_icon(c, slot)))
            || (self.inventory_slot_empty(c, slot) && self.inventory_slot_empty(c, slot.wrapping_add(1))))
    }
    fn fits_small(&self, c: i16, slot: i16) -> bool { self.inventory_slot_empty(c, slot) || !wide(self.item_icon(c, slot)) }
    pub fn carried_weight(&self, c: i16) -> i16 {
        (0..23).fold(0i16, |total, slot| total.wrapping_add(match self.item_definition(c, slot) {
            Some(d) if !self.inventory_slot_marker(c, slot) => self.def_word(d.wrapping_add(0x1F)),
            _ => 0,
        }))
    }
    pub fn weight_limit(&self, c: i16) -> i16 {
        let at = (c as u16).wrapping_mul(256);
        (self.s.pbyte(at.wrapping_add(0x116)) as i16 + self.s.pbyte(at.wrapping_add(0x118)) as i16) * 30
    }
    fn put_string(&mut self, at: u16, text: &[u8]) {
        for (i, &b) in text.iter().chain(std::iter::once(&0)).enumerate() { self.s.set_byte(at.wrapping_add(i as u16), b); }
    }
    pub fn number_to_string(&mut self, n: i16) -> u16 { self.put_string(NUMBER, n.to_string().as_bytes()); NUMBER }
    pub fn long_to_string(&mut self, n: i32) -> u16 { self.put_string(LONG, n.to_string().as_bytes()); LONG }
    pub fn format_tenths(&mut self, value: i16, suffix: &[u8]) -> u16 {
        let mut text = (value / 10).to_string().into_bytes();
        self.number_to_string(value / 10);
        text.extend(self.s.string(0x29A9));
        text.extend((value % 10).to_string().into_bytes());
        self.number_to_string(value % 10);
        text.extend_from_slice(suffix);
        self.put_string(TENTHS, &text); TENTHS
    }
    fn small_number(&mut self, n: i16) {
        let text: Vec<u8> = n.to_string().bytes().map(|b| b.wrapping_add(128)).collect();
        self.put_string(NUMBER, &text); self.draw_text(&text);
    }
    fn far_string(&self, at: u16) -> Vec<u8> { self.s.pointer_string(at).unwrap_or_default() }

    pub fn load_record_shapes(&mut self) -> [u16; 2] { self.select_shapes(CH); CH }
    pub fn select_shapes(&mut self, p: [u16; 2]) { self.s.set_word(SHAPES, p[0] as i16); self.s.set_word(SHAPES + 2, p[1] as i16); }
    pub fn draw_shape(&mut self, index: i16, y: i16, x: i16) {
        if [self.s.word(SHAPES) as u16, self.s.word(SHAPES + 2) as u16] != CH { return self.r.layout(&self.s, index, y, x); }
        self.named_shape("CH.LY", index, y, x);
    }
    fn named_shape(&mut self, library: &str, index: i16, y: i16, x: i16) {
        let page = Render::page(&self.s);
        if index >= 0 { ly::layout(&mut self.r.fb, Raster::Wrap, page, self.r.assets.layout(library), index as usize, y as i32, x as i32, &mut self.r.pen); }
    }
    pub fn read_translated_key(&mut self) -> u8 {
        if self.s.word(mouse::ENABLED) != 0 { while self.read_mouse().0 != 0 { self.spin(); } }
        self.s.set_byte(KEY, 0);
        self.show_cursor();
        while self.s.byte(KEY) == 0 { self.update_cursor(); }
        self.hide_cursor();
        let key = self.s.byte(KEY);
        self.s.set_byte(KEY, 0); self.s.set_byte(mouse::PENDING, 0);
        key
    }

    pub fn draw_career(&mut self, c: i16, y: i16, x: i16) {
        let at = (c as u16).wrapping_mul(256);
        let service = self.s.pbyte(at.wrapping_add(0x122)) as u16;
        self.set_text_position(y, x);
        let text = self.s.string(0x24D0); self.draw_text(&text);
        let text = self.far_string(0x1AC7 + service * 4); self.draw_text(&text);
        if service != 3 {
            self.draw_text_character(b' ');
            let rank = self.s.pbyte(at.wrapping_add(0x123)) as u16;
            let title = self.s.byte((0x1A58 + rank * 5).wrapping_add(service)) as u16;
            let text = self.far_string(0x1A7B + title * 4); self.draw_text(&text);
        }
    }
    pub fn draw_attributes(&mut self, values: [u8; 6], vertical: bool) {
        for v in values {
            let (y, x) = (self.s.word(TEXT_Y), self.s.word(TEXT_X));
            self.draw_text_character(if v < 10 { v.wrapping_add(b'0') } else { v.wrapping_add(b'A' - 10) });
            if vertical { self.set_text_position(y.wrapping_add(8), x); }
        }
    }
    pub fn draw_character_skills(&mut self, c: i16) {
        let (mut skill, mut row) = (self.s.word(FIRST_SKILL), 0i16);
        self.set_color(10);
        while skill < 80 {
            if row == 21 { break; }
            let level = crate::record::shown_skill(skill as u8, self.s.pbyte((c as u16).wrapping_mul(256).wrapping_add(skill as u16).wrapping_add(0x12B)));
            if level > 0 {
                self.set_text_position(row * 6 + 66, 5);
                let name = self.far_string(0x1ADBu16.wrapping_add((skill as u16).wrapping_mul(4))); self.draw_text(&name);
                self.set_text_position(row * 6 + 66, 76);
                self.small_number(level as i16);
                row += 1;
            }
            skill += 1;
        }
        if skill >= 80 { self.s.set_word(SKILLS_DONE, 1); }
    }
    pub fn draw_item_icon(&mut self, icon: i16, y: i16, x: i16) {
        let (sheet, sy, sx, w) = if wide(icon) { let n = icon - 100; (1, n / 6 * 15, n % 6 * 6, 6) } else { (0, icon / 12 * 15, icon % 12 * 3, 3) };
        for (at, v) in [(0x49F2, sy), (0x49EC, sx), (0x49F4, 15), (0x49F6, w)] { self.s.set_word(at, v); }
        let page = Render::page(&self.s);
        if let Some(sheet) = self.r.assets.objects.get(sheet) {
            pic::draw_rect(&mut self.r.fb, page, sheet, y as i32, x as i32 * 8, sy as i32, sx as i32 * 8, 15, w as i32 * 8, None);
        }
    }
    fn position(&self, slot: i16) -> (i16, i16) {
        let at = POSITIONS.wrapping_add((slot as u16).wrapping_mul(4));
        (self.s.word(at), self.s.word(at.wrapping_add(2)))
    }
    pub fn draw_inventory_slot(&mut self, c: i16, slot: i16) -> i16 {
        let icon = self.item_icon(c, slot);
        if icon != -1 { let (y, x) = self.position(slot); self.draw_item_icon(icon, y, x >> 3); }
        icon
    }
    pub fn draw_character_inventory(&mut self, c: i16) {
        let mut slot = 0;
        while slot < 19 {
            if self.item(c, slot) != 0 && wide(self.draw_inventory_slot(c, slot)) { slot += 1; }
            slot += 1;
        }
        for slot in 19..23 { if self.item(c, slot) != 0 { self.draw_inventory_slot(c, slot); } }
        let selected = self.s.word(SELECTED);
        if selected != 0 { self.highlight_inventory_slot(c, selected.wrapping_sub(1), 10); }
    }
    pub fn draw_value_bar(&mut self, y: i16, x: i16, height: i16, color: u8, value: i16, maximum: i16, extent: i16) -> i16 {
        if value == 0 { return 0; }
        let size = Self::scale_bar_value(value, maximum, extent);
        self.r.fill(&self.s, y, x, height, size, color);
        size
    }
    pub fn highlight_inventory_action(&mut self, n: i16) {
        self.select_shapes([self.s.word(0x4987) as u16, self.s.word(0x4989) as u16]);
        self.named_shape("FONT.LY", 5, n.wrapping_mul(8).wrapping_add(76), 176);
    }
    pub fn unhighlight_inventory_action(&mut self, n: i16) { self.r.fill(&self.s, n.wrapping_mul(8).wrapping_add(76), 176, 9, 6, 135); }
    pub fn highlight_inventory_slot(&mut self, c: i16, slot: i16, color: u8) {
        let width = if wide(self.item_icon(c, slot)) { 47 } else { 23 };
        let (y, x) = self.position(slot);
        self.draw_rectangle(y, x.wrapping_add(1), y.wrapping_add(14), x.wrapping_add(width), color);
    }
    fn erase_inventory_slot(&mut self, c: i16, slot: i16) {
        let color = if slot >= 19 { 0 } else { 8 };
        let (y, x) = self.position(slot);
        self.r.fill(&self.s, y, x.wrapping_add(1), 15, 23, 0);
        if wide(self.item_icon(c, slot)) {
            self.r.fill(&self.s, y, x.wrapping_add(24), 15, 1, color);
            self.r.fill(&self.s, y, x.wrapping_add(25), 15, 23, 0);
        }
    }
    pub fn draw_inventory_action(&mut self, action: i16) {
        let (fy, fx, fh, fw, color, ty) = match action { i16::MIN..=1 => (80, 98, 7, 66, 6, 80), 2 | 3 => (94, 186, 5, 111, 10, 92), 4 | 5 => (102, 186, 5, 111, 10, 100), _ => return };
        self.r.fill(&self.s, fy, fx, fh, fw, color);
        self.set_text_position(ty, fx);
        self.set_color(0);
        let text = self.far_string(0x2008u16.wrapping_add((action as u16).wrapping_mul(4))); self.draw_text(&text);
        if action < 2 { let key = self.s.byte(0x2020u16.wrapping_add(action as u16)); self.s.set_byte(HITS + 8, key); }
    }
    pub fn draw_weapon_capacity(&mut self, c: i16, slot: i16, row: i16, column: i16) -> i16 {
        let Some(d) = self.item_definition(c, slot) else { return -1 };
        if self.def_word(d.wrapping_add(0x1D)) != 2 { return -1; }
        let instance = self.item_instance(c, slot).unwrap_or(0);
        let mode = (self.def_byte(d.wrapping_add(0x27)) & 0xC0) as i16;
        let glyph = match mode { 0 => 0, 0x40 => 7, _ => 8 };
        if glyph != 0 {
            let (loaded, capacity) = (self.s.pbyte(instance.wrapping_add(2)) as i16, self.def_byte(d.wrapping_add(0x28)) as i16);
            self.set_color(10);
            self.set_text_position(row, column);
            for _ in 0..loaded { self.draw_text_character(glyph); }
            self.set_color(8);
            for _ in loaded..capacity { self.draw_text_character(glyph); }
        }
        mode
    }
    pub fn draw_equipped_weapon(&mut self, c: i16, slot: i16, row: i16) -> i16 {
        if self.inventory_slot_empty(c, slot) || self.inventory_slot_marker(c, slot) { return 0; }
        if self.item_category(c, slot) != 2 { return 0; }
        let (d, instance) = (self.item_definition(c, slot).unwrap_or(0), self.item_instance(c, slot).unwrap_or(0));
        self.set_text_position(row, 0xBA);
        let name: Vec<u8> = (0..46).map(|i| self.def_byte(d.wrapping_add(i))).take_while(|&b| b != 0).collect();
        self.draw_text(&name);
        if self.def_byte(d.wrapping_add(0x27)) & 0x20 != 0 {
            self.set_text_position(row, 0x126);
            self.small_number(self.s.pbyte(instance.wrapping_add(1)) as i16);
        }
        1
    }
    pub fn next_carried_item(&mut self, c: i16) -> i16 {
        let mut slot = self.s.word(CYCLE);
        loop {
            if self.inventory_slot_marker(c, slot) || self.inventory_slot_empty(c, slot) {
                slot = slot.wrapping_add(1);
                if slot >= 19 { slot = 0; }
            } else {
                let next = slot.wrapping_add(1);
                self.s.set_word(CYCLE, if next >= 19 { 0 } else { next });
                return slot;
            }
            if slot == self.s.word(CYCLE) { return -1; }
        }
    }
    pub fn set_temporary_selection(&mut self, on: bool) { self.s.set_word(TEMPORARY, on as i16); }

    fn build_inventory_regions(&mut self, c: i16, moving: bool) {
        let selected = self.s.word(SELECTED);
        let held_wide = moving && wide(self.item_icon(c, selected.wrapping_sub(1)));
        self.s.set_word(REGION_COUNT, 0);
        self.s.set_word(REGION_DEFAULT, -1);
        let mut slot = 0i16;
        while slot < 19 {
            let value = self.item(c, slot);
            if (moving && value == 0) || value > 0 {
                if moving && !held_wide && wide(self.item_icon(c, slot)) { slot += 2; continue; }
                if moving && held_wide && !self.fits_wide(c, slot) { slot += 1; continue; }
                let n = self.s.word(REGION_COUNT);
                if selected.wrapping_sub(1) == slot { self.s.set_word(REGION_DEFAULT, n); }
                let icon = if value == 0 { 0 } else { self.item_icon(c, slot) };
                self.s.set_array(REGION_SLOTS, n, slot);
                let (y, x) = self.position(slot);
                let at = REGIONS.wrapping_add((n as u16).wrapping_mul(8));
                for (k, v) in [y.wrapping_sub(1), x, 17, if wide(icon) { 48 } else { 24 }].into_iter().enumerate() { self.s.set_word(at.wrapping_add(2 * k as u16), v); }
                if wide(icon) { slot += 1; }
                self.s.set_word(REGION_COUNT, n.wrapping_add(1));
            }
            slot += 1;
        }
        if self.s.word(REGION_DEFAULT) == -1 { self.s.set_word(REGION_DEFAULT, 0); }
    }
    fn rect_table(&self, at: u16, count: i16) -> Vec<[i16; 4]> {
        (0..count.max(0) as u16).map(|i| std::array::from_fn(|k| self.s.word(at.wrapping_add(8 * i).wrapping_add(2 * k as u16)))).collect()
    }
    fn choose_region(&mut self) -> i16 {
        let rects = self.rect_table(REGIONS, self.s.word(REGION_COUNT));
        let choice = self.select_rect(&rects, self.s.word(REGION_DEFAULT), 6, 8, None);
        if choice == -1 { -1 } else { self.s.array(REGION_SLOTS, choice) }
    }
    pub fn can_swap_inventory_slots(&self, c: i16, slot: i16) -> bool {
        if wide(self.item_icon(c, self.s.word(SELECTED).wrapping_sub(1))) { self.fits_wide(c, slot) } else { self.fits_small(c, slot) }
    }
    pub fn swap_inventory_slots(&mut self, c: i16, slot: i16) {
        let held = self.s.word(SELECTED).wrapping_sub(1);
        let is_wide = wide(self.item_icon(c, held));
        self.r.front(&mut self.s);
        self.erase_inventory_slot(c, held);
        let (first, second) = (self.item(c, slot), self.item(c, slot.wrapping_add(1)));
        self.set_item(c, slot, self.item(c, held));
        if is_wide { self.set_item(c, slot.wrapping_add(1), -1); }
        self.set_item(c, held, first);
        if is_wide { self.set_item(c, held.wrapping_add(1), second); }
        self.draw_inventory_slot(c, slot);
        self.draw_inventory_slot(c, held);
        self.s.set_word(SELECTED, 0);
        self.draw_inventory_action(1);
    }
    pub fn select_inventory_slot(&mut self, c: i16, restart: bool) {
        if self.s.word(SELECTED) == 0 || restart {
            self.build_inventory_regions(c, false);
            if self.s.word(REGION_COUNT) > 0 {
                let slot = self.choose_region();
                if slot != -1 {
                    self.r.front(&mut self.s);
                    self.s.set_word(SELECTED, slot.wrapping_add(1));
                    self.highlight_inventory_slot(c, slot, 10);
                    self.r.front(&mut self.s);
                    self.draw_inventory_action(0);
                } else { self.s.set_word(SELECTED, 0); }
            }
        } else {
            self.build_inventory_regions(c, true);
            let slot = self.choose_region();
            if slot != -1 {
                if self.s.word(SELECTED).wrapping_sub(1) == slot {
                    self.r.front(&mut self.s);
                    self.highlight_inventory_slot(c, slot, 0);
                    self.draw_inventory_slot(c, slot);
                    self.s.set_word(SELECTED, 0);
                    self.draw_inventory_action(1);
                } else { self.swap_inventory_slots(c, slot); }
            }
        }
    }
    fn find_inventory_slot(&self, icon: i16, c: i16, preferred: i16, use_preferred: bool) -> i16 {
        let free = |slot: i16| (if wide(icon) { self.fits_wide(c, slot) } else { self.fits_small(c, slot) }) && self.inventory_slot_empty(c, slot);
        if use_preferred && free(preferred) { return preferred; }
        (0..19).find(|&slot| free(slot)).unwrap_or(-1)
    }
    pub fn transfer_selected_item(&mut self, from: i16, to: i16) {
        let slot = self.s.word(SELECTED).wrapping_sub(1);
        let icon = self.item_icon(from, slot);
        let destination = self.find_inventory_slot(icon, to, slot, true);
        if destination != -1 {
            self.set_item(to, destination, self.item(from, slot));
            if wide(icon) {
                self.set_item(to, destination.wrapping_add(1), -1);
                self.set_item(from, slot.wrapping_add(1), 0);
            }
            self.set_item(from, slot, 0);
        }
        self.s.set_word(SELECTED, 0);
    }
    fn add_inventory_item(&mut self, c: i16, instance: i16, redraw: bool) -> i16 {
        let kind = self.s.pbyte(((instance - 1) as u16).wrapping_mul(5).wrapping_add(0xA82)) as u16;
        let slot = self.find_inventory_slot(self.def_byte(kind.wrapping_mul(46).wrapping_add(0x26)) as i16, c, 0, false);
        if slot == -1 { return -1; }
        self.set_item(c, slot, instance);
        if wide(self.item_icon(c, slot)) { self.set_item(c, slot.wrapping_add(1), -1); }
        if redraw { self.r.front(&mut self.s); self.draw_inventory_slot(c, slot); }
        slot
    }
    fn relocate_inventory_item(&mut self, c: i16, slot: i16) -> bool {
        let instance = self.item(c, slot);
        if self.add_inventory_item(c, instance, true) == -1 { return false; }
        self.erase_inventory_slot(c, slot);
        self.set_item(c, slot, 0);
        true
    }
    pub fn draw_character_equipment(&mut self, c: i16, refresh: bool) {
        if self.s.pbyte((c as u16).wrapping_mul(256).wrapping_add(0x127)) == 0 { return; }
        if refresh { self.r.back(&mut self.s); }
        for (slot, shape, y) in [(19, 20, 17), (20, 30, 33)] {
            if self.inventory_slot_empty(c, slot) {
                self.top_layout(c.wrapping_add(shape), y, c.wrapping_mul(40).wrapping_add(9));
                self.select_shapes([self.s.word(STATION_SHAPES) as u16, self.s.word(STATION_SHAPES + 2) as u16]);
            } else {
                self.draw_item_icon(self.item_icon(c, slot), y, c.wrapping_mul(5).wrapping_add(1));
                let x = c.wrapping_mul(40).wrapping_add(8);
                self.draw_line(y, x, y + 14, x, 0);
            }
        }
        if refresh { self.clear_party_panel(); self.r.front(&mut self.s); }
    }
    pub fn equip_inventory_item(&mut self, c: i16, mut slot: i16, mut equipment: i16) {
        self.build_inventory_regions(c, false);
        if self.s.word(REGION_COUNT) <= 0 { return; }
        if slot == -1 {
            let rects = self.rect_table(REGIONS, self.s.word(REGION_COUNT));
            slot = self.select_rect(&rects, self.s.word(REGION_DEFAULT), 6, 8, None);
        }
        if slot == -1 { return; }
        if equipment == -1 { slot = self.s.array(REGION_SLOTS, slot); }
        match self.item_category(c, slot) {
            3 => equipment = 20,
            4 => equipment = 19,
            _ => {
                if equipment == -1 { let rects = self.rect_table(0x1C0B, 2); equipment = self.select_rect(&rects, 0, 6, 8, None); }
                if equipment != -1 { equipment = equipment.wrapping_add(21); }
            }
        }
        if equipment < 19 { return; }
        let available = self.item(c, equipment) == 0 || self.relocate_inventory_item(c, equipment);
        self.r.front(&mut self.s);
        if !available { return; }
        self.erase_inventory_slot(c, slot);
        if wide(self.item_icon(c, slot)) { self.set_item(c, slot.wrapping_add(1), 0); }
        self.set_item(c, equipment, self.item(c, slot));
        self.set_item(c, slot, 0);
        self.draw_inventory_slot(c, equipment);
        if equipment == 21 { self.draw_inventory_action((self.item_category(c, 21) == 2) as i16 + 2); }
        if equipment == 22 { self.draw_inventory_action((self.item_category(c, 22) == 2) as i16 + 4); }
        if equipment == 19 || equipment == 20 {
            self.r.back(&mut self.s);
            if equipment == 19 { self.draw_party_oxygen_bars(); }
            self.draw_character_equipment(c, true);
            self.hide_cursor();
        }
        if self.s.word(SELECTED).wrapping_sub(1) == slot {
            self.draw_inventory_action(1);
            self.s.set_word(SELECTED, 0);
        }
    }
    pub fn unequip_inventory_item(&mut self, c: i16, mut equipment: i16) {
        if equipment == -1 { let rects = self.rect_table(0x1BFB, 4); equipment = self.select_rect(&rects, 0, 6, 8, None); }
        if equipment == -1 { return; }
        equipment = equipment.wrapping_add(19);
        if !self.relocate_inventory_item(c, equipment) { return; }
        if equipment == 19 {
            self.r.back(&mut self.s);
            self.draw_party_oxygen_bars();
            self.draw_character_equipment(c, true);
        } else if equipment == 20 { self.draw_character_equipment(c, true); }
        self.hide_cursor();
        self.r.front(&mut self.s);
        if equipment == 21 { self.draw_inventory_action(2); }
        if equipment == 22 { self.draw_inventory_action(4); }
    }
}
