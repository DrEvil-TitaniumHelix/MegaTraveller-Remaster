use super::{Space, state::*, stations::{self, Host, Reply, STACK}, guns::{StationUi, Rect}, ui::Label};
use mt_engine::io::Files;
pub mod record;

pub const DEAD: &str = "S: party dead";
pub const TOP_FRAME: i32 = 0xFEFE;

pub struct StationHost<'a> { pub sp: &'a mut Space, pub scratch: Vec<u8>, pub bp: i32, bars: bool }
impl<'a> StationHost<'a> {
    pub fn new(sp: &'a mut Space, scratch: Vec<u8>, bp: i32) -> Self { Self { sp, scratch, bp, bars: false } }
    fn byte(&self, off: i32, seg: i32) -> u8 {
        let off = off as u16;
        match seg as u16 {
            DS => self.sp.s.byte(off),
            STACK_SEG => if (off as usize) < PARTY_SIZE { self.sp.s.pbyte(off) } else { self.scratch.get(off as usize).copied().unwrap_or(0) },
            WORLD_SEG => self.sp.s.world.get(off as usize).copied().unwrap_or(0),
            _ => 0,
        }
    }
    fn set_byte(&mut self, off: i32, seg: i32, v: u8) {
        let off = off as u16;
        match seg as u16 {
            DS => self.sp.s.set_byte(off, v),
            STACK_SEG => if (off as usize) < PARTY_SIZE { self.sp.s.set_pbyte(off, v) } else if let Some(b) = self.scratch.get_mut(off as usize) { *b = v },
            WORLD_SEG => if let Some(b) = self.sp.s.world.get_mut(off as usize) { *b = v },
            _ => {}
        }
    }
    fn string(&self, off: i32, seg: i32) -> stations::Result<Vec<u8>> {
        let mut out = Vec::new();
        for i in 0..4096 { let b = self.byte(off + i, seg); if b == 0 { return Ok(out); } out.push(b); }
        Err("S host: unterminated string".into())
    }
    fn labels(&self, off: i32, seg: i32) -> stations::Result<Vec<Label>> {
        let mut out = Vec::new();
        for i in 0..64 {
            let at = off + 9 * i;
            let color = self.byte(at + 8, seg);
            if color == 255 { return Ok(out); }
            let (y, x) = (self.read(at, seg, 2) as u16 as i16, self.read(at + 2, seg, 2) as u16 as i16);
            let (to, ts) = (self.read(at + 4, seg, 2) as i32, self.read(at + 6, seg, 2) as i32);
            let text = if to == 0 && ts == 0 { None } else { Some(self.string(to, ts)?) };
            out.push((y, x, text, color));
        }
        Err("S host: unterminated label table".into())
    }
    fn rects(&self, off: i32, seg: i32, count: i32) -> Vec<Rect> {
        (0..count.max(0)).map(|i| std::array::from_fn(|j| self.read(off + 8 * i + 2 * j as i32, seg, 2) as u16 as i16)).collect()
    }
    fn page(seg: i32) -> stations::Result<usize> {
        let seg = seg as u16;
        if (0xA000..0xB000).contains(&seg) && seg % 0x200 == 0 { Ok(((seg - 0xA000) / 0x200) as usize) } else { Err(format!("S host: copy_rect segment {seg:X} is not an EGA page")) }
    }
}
const STACK_SEG: u16 = STACK as u16;

impl Host for StationHost<'_> {
    fn ds(&self) -> i32 { DS as i32 }
    fn entry_di(&self) -> stations::Result<u16> { Ok(15) }
    fn decision_wait(&mut self, routine: &str, selector: Option<(usize,i16)>, live: &[i32]) {
        if self.sp.observer.is_some() || !self.sp.input.wants_decision() { return }
        if routine == "1C1C" && self.sp.switch_to.is_some() { return }
        let mut state=self.scratch.clone(); state.extend_from_slice(&self.bp.to_le_bytes());
        for n in live { state.extend_from_slice(&n.to_le_bytes()); }
        match selector {
            Some((n,selected)) => {
                let stations = routine == "1C1C" && live.first() == Some(&0x355) && live.get(1) == Some(&(DS as i32));
                if stations && self.sp.product {
                    self.sp.play_labels = ["Pilot", "Gunner 1", "Gunner 2", "Computer", "Engineering", "Communications", "Medical", "Crew"]
                        .iter().enumerate().map(|(i, name)| {
                            let slots = if i == 7 { 7..12 } else { i..i+1 };
                            let crew: Vec<_> = slots.filter_map(|slot| {
                                let who = self.sp.s.pbyte(0xa0 + slot as u16);
                                (1..=5).contains(&who).then(|| {
                                    let at = 0x102 + (u16::from(who)-1)*256;
                                    let bytes: Vec<_> = (0..20).map(|j|self.sp.s.pbyte(at+j)).take_while(|&b|b!=0).collect();
                                    String::from_utf8_lossy(&bytes).into_owned()
                                })
                            }).collect();
                            format!("{name}: {}", if crew.is_empty() { "Unassigned".into() } else { crew.join(", ") })
                        }).collect();
                }
                if routine == "1C1C" && live.len() >= 2 && self.sp.input.wants_decision() {
                    self.sp.play_rects = self.rects(live[0], live[1], n as i32).iter().map(|r| [r[1] as i32, r[0] as i32, r[3] as i32, r[2] as i32]).collect();
                }
                super::play::selector_state(self.sp,routine,n,selected,None,&state);
                if stations && self.sp.product { self.sp.play_labels.clear(); }
            },
            None => super::play::station_state(self.sp,routine,&state),
        }
    }
    fn read(&self, off: i32, seg: i32, width: usize) -> u32 {
        (0..width.min(4) as i32).fold(0u32, |v, i| v | (self.byte(off + i, seg) as u32) << (8 * i))
    }
    fn write(&mut self, off: i32, seg: i32, value: u32, width: usize) {
        for i in 0..width.min(4) as i32 { self.set_byte(off + i, seg, (value >> (8 * i)) as u8); }
    }
    fn key_spin(&mut self) -> stations::Result<()> { self.sp.spin(); Ok(()) }
    fn take_switch(&mut self) -> Option<i32> { self.sp.switch_to.take() }
    fn unmanned(&mut self, station: i32) { self.sp.no_one_at(station) }
    fn click_through(&self) -> bool { self.sp.product }
    fn bars(&mut self, on: Option<bool>) -> bool { if let Some(on) = on { self.bars = on; } self.bars }
    fn repair_sign(&mut self) -> Option<Vec<u8>> {
        if !self.sp.product { return None }
        let i = (1..5u16).find(|i| self.sp.s.word(super::tick::ACTIVE + 2 * i) != -1)?;
        let left = self.sp.s.dword(super::tick::DEADLINES + 4 * i).saturating_sub(self.sp.last_ticks);
        let rest = (self.sp.s.pbyte(0xC1 + i).max(1) as u32 - 1) * self.sp.s.word(super::tick::INTERVALS + 2 * i).max(0) as u32;
        Some(format!("{} +1 in {}s, all {}m", ["comp", "man", "gun1", "gun2"][i as usize - 1], (left * 10).div_ceil(182).min(99), ((left + rest) * 10).div_ceil(182 * 60).min(99)).into_bytes())
    }
    fn call(&mut self, name: &str, a: &[i32]) -> stations::Result<Reply> {
        if self.sp.dead { return Err(DEAD.into()); }
        let name = if name == "format_columns" { "remap_numeric_glyphs" } else { name };
        let bp = self.bp - 1024;
        self.bp = bp;
        let nested = stations::invoke(self, name, a, bp);
        self.bp = bp + 1024;
        if let Some(r) = nested { return r; }
        let n = |i: usize| a.get(i).copied().unwrap_or(0);
        let w = |i: usize| a.get(i).copied().unwrap_or(0) as i16;
        let ok = |v: i32| Ok(Reply::int(v));
        match name {
            "update_cursor" => { self.sp.update_cursor(); ok(0) }
            "show_cursor" => { self.sp.show_cursor(); ok(0) }
            "hide_cursor" => { self.sp.hide_cursor(); ok(0) }
            "run_tasks" => ok(self.sp.run_tasks(w(0)) as i32),
            "poll_tasks" => { self.sp.run_tasks(-1); ok(0) }
            "get_ticks" => { let t = self.sp.ticks(); Ok(Reply { ax: t as u16, dx: (t >> 16) as u16 }) }
            "snapshot_clock" => { self.sp.snapshot_clock(); ok(0) }
            "rand" => ok(self.sp.rand() as i32),
            "improve_skill" => ok(self.sp.combat_skill(w(0), w(1)) as i32),
            "translate_station_key" => ok(self.sp.translate_station_key() as i32),
            "update_encounter_countdown" => ok(self.sp.update_encounter_countdown(w(0), n(1) != 0) as i32),
            "select_draw_page" => { self.sp.r.front(&mut self.sp.s); ok(0) }
            "back_buffer" => { self.sp.r.back(&mut self.sp.s); ok(0) }
            "flip_with_cursor" | "flip_buffers" | "flip_display" => { self.sp.flip_with_cursor(); ok(0) }
            "draw_layout" | "draw_image" => { self.sp.r.layout(&self.sp.s, w(0), w(1), w(2)); ok(0) }
            "fill_rect" | "fill_rectangle" => { self.sp.r.fill(&self.sp.s, w(0), w(1), w(2), w(3), n(4) as u8); ok(0) }
            "draw_labels" => { let labels = self.labels(n(0), n(1))?; self.sp.draw_labels(&labels, w(2), w(3)); ok(0) }
            "draw_text" => { let s = self.string(n(0), n(1))?; self.sp.draw_text(&s); ok(0) }
            "set_text_position" => { self.sp.set_text_position(w(0), w(1)); ok(0) }
            "set_color" | "set_text_color" => { self.sp.set_color(w(0)); ok(0) }
            "draw_character" => { self.sp.draw_text_character(n(0) as u8); ok(0) }
            "select_font" => { self.sp.select_font(n(0) as u8); ok(0) }
            "finish_text" => { self.sp.finish_text(n(0) as u8); ok(0) }
            "set_text_origin" => { self.sp.set_text_origin(w(0), w(1)); ok(0) }
            "set_viewport" => { self.sp.set_viewport(w(0), w(1), w(2), w(3)); ok(0) }
            "set_cursor_mode" => { self.sp.set_cursor_mode(w(0)); ok(0) }
            "set_palette_color" | "set_station_colors" => { self.sp.set_palette_color(w(0), w(1)); ok(0) }
            "draw_rectangle" => { self.sp.draw_rectangle(w(0), w(1), w(2), w(3), n(4) as u8); ok(0) }
            "draw_station_icon" => { self.sp.draw_station_icon(w(0), w(1), w(2), n(3) != 0); ok(0) }
            "draw_portrait" => { self.sp.draw_symbol_overlay(w(0), n(1) as u8, w(2), w(3), w(4)); ok(0) }
            "direction_sector" => ok(Space::direction_sector(w(0), w(1)) as i32),
            "copy_rect" => {
                let (src, dst) = (Self::page(n(1))?, Self::page(n(7))?);
                self.sp.copy(src, w(2), w(3), w(4), w(5), dst, w(8), w(9)); ok(0)
            }
            "redraw_active_station" | "draw_station_frame" => { self.sp.redraw_active_station(); ok(0) }
            "redraw_tactical" => ok(self.sp.redraw_tactical_radar(w(0)) as i32),
            "show_tactical_map" => ok(self.sp.show_tactical_map()? as i32),
            "gun" => {
                let mut ui = GunUi { scratch: std::mem::take(&mut self.scratch), bp: self.bp };
                let r = self.sp.operate_gun_station(w(0), &mut ui);
                self.scratch = ui.scratch;
                r?; ok(0)
            }
            "select_rect" => {
                let rects = self.rects(n(0), n(1), n(2));
                let keys = if n(6) != 0 || n(7) != 0 { Some(self.string(n(6), n(7))?) } else { None };
                ok(self.sp.select_rect(&rects, w(3), w(4), w(5), keys.as_deref()) as i32)
            }
            "beep" => { self.sp.beep(); ok(0) }
            "mouse_command" => { self.sp.mouse_command(n(0) as u16); ok(0) }
            "inventory_slot_empty" => ok(self.sp.inventory_slot_empty(w(0), w(1)) as i32),
            "inventory_slot_marker" => ok(self.sp.inventory_slot_marker(w(0), w(1)) as i32),
            "get_inventory_item_type" => ok(self.sp.item_icon(w(0), w(1)) as i32),
            "inventory_item_category" => ok(self.sp.item_category(w(0), w(1)) as i32),
            "get_inventory_definition" => Ok(self.far(WORLD_PTR, self.sp.item_definition(w(0), w(1)))),
            "get_inventory_instance" => Ok(self.far(PARTY_PTR, self.sp.item_instance(w(0), w(1)))),
            "carried_weight" => ok(self.sp.carried_weight(w(0)) as i32),
            "weight_limit" => ok(self.sp.weight_limit(w(0)) as i32),
            "scaled_bar_value" => ok(Space::scale_bar_value(w(0), w(1), w(2)) as i32),
            "number_to_string" => Ok(Reply { ax: self.sp.number_to_string(w(0)), dx: DS }),
            "long_to_string" => Ok(Reply { ax: self.sp.long_to_string((n(0) as u16 as u32 | (n(1) as u16 as u32) << 16) as i32), dx: DS }),
            "format_tenths" => { let suffix = self.string(n(1), n(2))?; Ok(Reply { ax: self.sp.format_tenths(w(0), &suffix), dx: DS }) }
            "shift_string" => {
                for i in 0..4096 { let b = self.byte(n(0) + i, n(1)); if b == 0 { break; } self.set_byte(n(0) + i, n(1), b.wrapping_add(n(2) as u8)); }
                Ok(Reply { ax: n(0) as u16, dx: n(1) as u16 })
            }
            "load_layout" => {
                let file = self.string(n(0), n(1))?;
                if !file.eq_ignore_ascii_case(b"ch.ly") { return Err(format!("S host: load_layout `{}` is not the record's library", String::from_utf8_lossy(&file))); }
                let p = self.sp.load_record_shapes(); Ok(Reply { ax: p[0], dx: p[1] })
            }
            "release_buffer" => ok(0),
            "select_shapes" => { self.sp.select_shapes([n(0) as u16, n(1) as u16]); ok(0) }
            "draw_shape" => { self.sp.draw_shape(w(0), w(1), w(2)); ok(0) }
            "read_translated_key" => { super::play::record(self.sp); ok(self.sp.read_translated_key() as i32) }
            "draw_career" => { self.sp.draw_career(w(0), w(1), w(2)); ok(0) }
            "draw_attributes" => { let values = std::array::from_fn(|i| self.byte(n(0) + i as i32, n(1))); self.sp.draw_attributes(values, n(2) != 0); ok(0) }
            "draw_character_skills" => { self.sp.draw_character_skills(w(0)); ok(0) }
            "draw_item_icon" => { self.sp.draw_item_icon(w(0), w(1), w(2)); ok(0) }
            "draw_inventory_slot" => ok(self.sp.draw_inventory_slot(w(0), w(1)) as i32),
            "draw_character_inventory" => { self.sp.draw_character_inventory(w(0)); ok(0) }
            "draw_value_bar" => ok(self.sp.draw_value_bar(w(0), w(1), w(2), n(3) as u8, w(4), w(5), w(6)) as i32),
            "highlight_inventory_action" => { self.sp.highlight_inventory_action(w(0)); ok(0) }
            "unhighlight_inventory_action" => { self.sp.unhighlight_inventory_action(w(0)); ok(0) }
            "highlight_inventory_slot" => { self.sp.highlight_inventory_slot(w(0), w(1), n(2) as u8); ok(0) }
            "draw_inventory_action" => { self.sp.draw_inventory_action(w(0)); ok(0) }
            "draw_weapon_capacity" => ok(self.sp.draw_weapon_capacity(w(0), w(1), w(2), w(3)) as i32),
            "draw_equipped_weapon" => ok(self.sp.draw_equipped_weapon(w(0), w(1), w(2)) as i32),
            "show_inventory_action_details" => { self.details(n(0), n(1))?; ok(0) }
            "first_skill_page" => { self.first_skill_page(n(0))?; ok(0) }
            "next_skill_page" => { self.next_skill_page(n(0))?; ok(0) }
            "can_swap_inventory_slots" => ok(self.sp.can_swap_inventory_slots(w(0), w(1)) as i32),
            "swap_inventory_slots" => { self.sp.swap_inventory_slots(w(0), w(1)); ok(0) }
            "select_inventory_slot" => { self.sp.select_inventory_slot(w(0), n(1) != 0); ok(0) }
            "transfer_selected_item" => { self.sp.transfer_selected_item(w(0), w(1)); ok(0) }
            "equip_inventory_item" => { self.sp.equip_inventory_item(w(0), w(1), w(2)); ok(0) }
            "unequip_inventory_item" => { self.sp.unequip_inventory_item(w(0), w(1)); ok(0) }
            _ => Err(format!("S host: station service `{name}` is not wired to the shared host")),
        }
    }
}
impl StationHost<'_> {
    fn far(&self, base: u16, at: Option<u16>) -> Reply {
        at.map_or(Reply::default(), |at| { let p = self.ptr(base as i32); Reply { ax: (p[0] as u16).wrapping_add(at), dx: p[1] as u16 } })
    }
    fn details(&mut self, c: i32, action: i32) -> stations::Result<()> {
        let sel = record::SELECTED as i32;
        if !(0..8).contains(&action) { return Err(format!("S host: record detail row {action}")); }
        let mut icons: [i32; 8] = std::array::from_fn(|i| self.w(0x2022 + 2 * i as i32));
        let saved = self.w(sel);
        if action == 7 && saved == 0 {
            let slot = self.sp.next_carried_item(c as i16);
            if slot == -1 { return Ok(()); }
            self.sw(sel, slot as i32 + 1); self.sp.set_temporary_selection(true);
        }
        for (row, slot) in [(2, 21), (3, 22)] {
            if action == row && self.sp.item_category(c as i16, slot) != 2 {
                if self.sp.inventory_slot_empty(c as i16, slot) { return Ok(()); }
                icons[row as usize] = 17;
            }
        }
        let icon = icons[action as usize];
        self.sp.r.back(&mut self.sp.s);
        self.int_call("draw_inventory_panel", &[c, 0])?;
        let shapes = self.ptr(0x46EE); self.sp.select_shapes([shapes[0] as u16, shapes[1] as u16]);
        self.sp.draw_shape(icon as i16, ((action + 1) * 8 + 0x4C) as i16, 0xB8);
        self.int_call("draw_inventory_detail_panel", &[c, action, icon])?;
        self.sp.flip_with_cursor();
        self.sp.s.set_byte(KEY, 0);
        super::play::record_detail(self.sp);
        self.sp.read_translated_key();
        self.sp.set_temporary_selection(false);
        self.sw(sel, if action == 7 { 0 } else { saved });
        self.int_call("draw_inventory_panel", &[c, 0])?;
        self.sp.highlight_inventory_action(action as i16);
        self.sp.flip_with_cursor();
        Ok(())
    }
    fn first_skill_page(&mut self, c: i32) -> stations::Result<()> {
        if self.sp.s.word(record::FIRST_SKILL) == 0 { return Ok(()); }
        self.sp.r.back(&mut self.sp.s);
        self.sp.s.set_word(record::FIRST_SKILL, 0);
        self.int_call("draw_inventory_panel", &[c, 1])?;
        self.sp.s.set_word(record::SKILLS_DONE, 0);
        self.sp.r.front(&mut self.sp.s);
        Ok(())
    }
    fn next_skill_page(&mut self, c: i32) -> stations::Result<()> {
        if self.sp.s.word(record::SKILLS_DONE) != 0 { return Ok(()); }
        let (mut skill, mut count) = (self.sp.s.word(record::FIRST_SKILL), 0);
        while skill < 80 {
            if count == 21 { break; }
            if self.pb((c << 8) + 0x12B + skill as i32) > 0 { count += 1; }
            skill += 1;
        }
        if skill <= 80 {
            self.sp.r.back(&mut self.sp.s);
            self.sp.s.set_word(record::FIRST_SKILL, skill);
            self.int_call("draw_inventory_panel", &[c, 1])?;
            self.sp.r.front(&mut self.sp.s);
        }
        Ok(())
    }
}

pub struct GunUi { pub scratch: Vec<u8>, pub bp: i32 }
impl StationUi for GunUi {
    fn call(&mut self, sp: &mut Space, name: &str, args: &[i16]) -> Result<i16, String> {
        let mut h = StationHost::new(sp, std::mem::take(&mut self.scratch), self.bp);
        let a: Vec<i32> = args.iter().map(|&v| v as i32).collect();
        let r = h.call(name, &a);
        self.scratch = h.scratch;
        Ok(r?.signed() as i16)
    }
    fn select(&mut self, sp: &mut Space, boxes: &[Rect], count: i16, selected: i16, hotkeys: Option<&[u8]>, boarding: bool) -> Result<i16, String> {
        let count = (count.max(0) as usize).min(boxes.len());
        if !boarding { return Ok(sp.select_rect(&boxes[..count], selected, 6, 8, hotkeys)); }
        let frame = self.bp - 1024;
        let mut h = StationHost::new(sp, std::mem::take(&mut self.scratch), frame);
        for (i, r) in boxes[..count].iter().enumerate() { for (j, v) in r.iter().enumerate() { h.write(frame + 8 * i as i32 + 2 * j as i32, STACK, *v as u16 as u32, 2); } }
        let keys_at = frame + 8 * count as i32;
        let (koff, kseg) = match hotkeys {
            Some(k) => { for (i, b) in k.iter().chain(std::iter::once(&0)).enumerate() { h.write(keys_at + i as i32, STACK, *b as u32, 1); } (keys_at, STACK) }
            None => (0, 0),
        };
        let r = stations::invoke(&mut h, "station_menu", &[frame, STACK, count as i32, selected as i32, 6, 8, koff, kseg, 0], frame - 1024)
            .unwrap_or_else(|| Err("S host: station_menu missing".into()));
        self.scratch = h.scratch;
        Ok(r?.signed() as i16)
    }
}

impl Space {
    pub fn run_stations(&mut self) {
        self.drop_stale_clicks();
        let r = {
            let mut h = StationHost::new(self, vec![0; 65536], TOP_FRAME);
            stations::invoke(&mut h, "dispatch_ship_stations", &[], TOP_FRAME)
        };
        match r {
            Some(Err(e)) if e == DEAD => {}
            Some(Err(e)) if self.product && self.observer.is_none() && !self.input.data_mode() => {
                eprintln!("S: {e}");
                self.s.set_word(STATION, -1); self.s.set_word(PREVIOUS_STATION, -1); self.switch_to = None;
                self.pause(b"STATION FAULT - press a key");
            }
            Some(Err(e)) => self.fail(e),
            _ => {}
        }
    }
    pub(super) fn record_elsewhere(&mut self, c: i16) -> bool {
        if !self.product || self.observer.is_some() || self.play_tactical || self.s.byte(0x4626) != 0 { return false }
        self.hide_cursor();
        let (ds, dac, pen, range) = (self.s.image().to_vec(), self.dac, self.r.pen, self.mouse.range);
        let (planes, regs, text) = (self.r.fb.planes.clone(), self.r.fb.regs, mt_engine::present::textlog::capture(&self.r.fb));
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0);
        if self.s.word(STATION_ACTIVE) == 0 { self.s.set_word(STATION_ACTIVE, 1); self.mouse_bounds(true); self.redraw_active_station(); }
        else { self.r.back(&mut self.s); }
        let r = { let mut h = StationHost::new(self, vec![0; 65536], TOP_FRAME); h.call("crew_record", &[c as i32]) };
        self.r.fb.planes = planes; self.r.fb.regs = regs;
        let frame = self.input.play_checkpoint().map(|p| p.frame).unwrap_or_default();
        if let Err(e) = mt_engine::present::textlog::restore(&self.r.fb, &text, &frame) { eprintln!("S: crew record: {e}"); }
        self.s.image_mut().copy_from_slice(&ds); self.dac = dac; self.r.pen = pen; self.mouse.range = range;
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0);
        if let Err(e) = r { if e != DEAD { eprintln!("S: crew record: {e}"); } }
        true
    }
    pub fn fail(&mut self, e: String) -> ! {
        eprintln!("S: {e}");
        self.stop(&format!("error: {e}"));
        std::process::exit(3)
    }
    pub fn files(&mut self) -> &mut dyn Files { self.files.as_mut() }
}
#[cfg(test)]
mod t {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir, Scripted}, sim::tick::Counter};
    #[test]
    fn scan_of_a_destroyed_weapon_has_its_di() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        assert_eq!(StationHost::new(&mut sp, vec![], TOP_FRAME).entry_di(), Ok(15));
    }
    #[test]
    fn a_running_repair_says_what_it_is_doing() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        sp.product = true;
        for i in 1..5 { sp.s.set_word(crate::space::tick::ACTIVE + 2 * i, -1); }
        assert_eq!(StationHost::new(&mut sp, vec![], TOP_FRAME).repair_sign(), None, "no repair, no words");
        sp.s.set_word(crate::space::tick::ACTIVE + 2, 1); sp.s.set_dword(crate::space::tick::DEADLINES + 4, 1000); sp.last_ticks = 1000 - 728;
        sp.s.set_pbyte(0xC2, 25); sp.s.set_word(crate::space::tick::INTERVALS + 2, 728);
        let sign = |sp: &mut Space| StationHost::new(sp, vec![], TOP_FRAME).repair_sign().map(|s| String::from_utf8(s).unwrap());
        assert_eq!(sign(&mut sp).as_deref(), Some("comp +1 in 40s, all 17m"));
        sp.last_ticks = 1000 - 18; assert_eq!(sign(&mut sp).as_deref(), Some("comp +1 in 1s, all 17m"));
        sp.s.set_word(crate::space::tick::ACTIVE + 2, -1); sp.s.set_word(crate::space::tick::ACTIVE + 8, 1); sp.s.set_dword(crate::space::tick::DEADLINES + 16, 0);
        assert_eq!(sign(&mut sp).as_deref(), Some("gun2 +1 in 0s, all 0m"));
        sp.product = false; assert_eq!(sign(&mut sp), None, "the 1990 has no such words");
    }
    fn crewed(keys: &[mt_engine::io::Key]) -> Option<Space> {
        let dir = oracle_env::gamefiles()?;
        let mut items = std::fs::read(dir.join("TRAV.ITM")).unwrap()[2..].to_vec();
        for record in items.chunks_exact_mut(46) { for b in record.iter_mut().take_while(|b| **b != 0) { if b.is_ascii_digit() { *b += 128; } } }
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), std::fs::read(dir.join("DEFAULT.SAV")).unwrap(), items,
            Box::new(Scripted::new(keys.iter().copied())), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        sp.r.initialize(&mut sp.s);
        sp.s.set_word(PARTY_PTR, 0); sp.s.set_word(PARTY_PTR + 2, PARTY_SEG as i16);
        sp.s.set_word(WORLD_PTR, 0); sp.s.set_word(WORLD_PTR + 2, WORLD_SEG as i16);
        Some(sp)
    }
    fn ppm(sp: &Space, name: &str) {
        let page = (sp.s.word(ACTIVE_PAGE) & 1) as usize;
        let mut out = b"P6 320 200 255\n".to_vec();
        for c in sp.r.fb.rgb(page, &sp.dac) { out.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]); }
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/crew-record");
        std::fs::create_dir_all(&dir).unwrap(); std::fs::write(dir.join(name), out).unwrap();
    }
    struct Shot(String);
    impl crate::space::tick::Hooks for Shot {
        fn update_cursor(&mut self, sp: &mut Space) { if !self.0.is_empty() { ppm(sp, &self.0); self.0.clear(); } sp.poll_input(); sp.update_software_cursor() }
    }
    #[test]
    fn crew_record_opens_pages_and_closes() {
        for product in [false, true] {
            use mt_engine::io::keys as K;
            let mut keys = vec![];
            for _ in 0..8 { keys.extend([K::ENTER, 32, K::DOWN]); }
            keys.extend([K::PGDN, K::PGUP, K::F1 + 1, K::ENTER, 32, K::ESC]);
            let Some(mut sp) = crewed(&keys) else { return };
            sp.product = product;
            let party = sp.s.party.clone();
            let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
            assert_eq!(h.call("crew_record", &[0]).map(|r| r.signed()), Ok(0), "product {product}");
            assert_eq!(sp.s.party, party, "looking changes nothing");
            assert_eq!((sp.s.word(record::SELECTED), sp.s.word(0x49C6)), (0, 200));
        }
    }
    #[test]
    fn f_keys_open_the_record_where_the_1990_has_none() {
        use mt_engine::io::keys as K;
        let Some(mut sp) = crewed(&[K::F1 + 1, K::ESC]) else { return };
        sp.product = true; sp.s.set_word(STATION_ACTIVE, 0);
        let pixels = |sp: &Space| (0..2).map(|p| sp.r.fb.indexed(p)).collect::<Vec<_>>();
        let (ds, shown, party) = (sp.s.image().to_vec(), pixels(&sp), sp.s.party.clone());
        assert_eq!(sp.translate_station_key(), 256);
        let changed: Vec<usize> = (0..ds.len()).filter(|&i| sp.s.image()[i] != ds[i] && i != 0x4840).collect();
        assert!(changed.is_empty(), "the DS image is put back: {changed:X?}");
        assert!(pixels(&sp) == shown, "both pages are put back");
        assert_eq!(sp.s.party, party);
        let Some(mut sp) = crewed(&[K::F1 + 1, K::ESC]) else { return };
        assert_eq!(sp.translate_station_key(), 27, "1990: F2 is no key here");
        let Some(mut sp) = crewed(&[K::F1 + 1, K::ESC]) else { return };
        sp.product = true; sp.play_tactical = true;
        assert_eq!(sp.translate_station_key(), 27, "not over the tactical map");
    }
    fn pack(sp: &Space, c: u16) -> Vec<i16> { (0..23).map(|i| sp.s.pword(c * 256 + 0x1CE + 2 * i)).collect() }
    #[test]
    fn crew_record_hands_an_item_over() {
        use mt_engine::io::keys as K;
        let Some(mut sp) = crewed(&[b'P' as u16, K::ENTER, K::F1 + 1, K::ESC]) else { return };
        let (a, b) = (pack(&sp, 0), pack(&sp, 1));
        let slot = a[..19].iter().position(|&v| v > 0).expect("Magellan carries something");
        let (wa, wb) = (sp.carried_weight(0), sp.carried_weight(1));
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        h.call("back_buffer", &[]).unwrap();
        assert_eq!(h.call("crew_record", &[0]).map(|r| r.signed()), Ok(0));
        let (a2, b2) = (pack(&sp, 0), pack(&sp, 1));
        assert_eq!(a2[slot], 0, "left Magellan");
        let got = (0..19).find(|&i| b2[i] != b[i]).expect("arrived");
        assert_eq!((b2[got], sp.s.word(record::SELECTED)), (a[slot], 0));
        let moved = wa - sp.carried_weight(0);
        assert!(moved > 0 && sp.carried_weight(1) - wb == moved, "the weight went with it");
    }
    #[test]
    fn crew_record_equips_and_puts_back() {
        let Some(mut sp) = crewed(&[]) else { return };
        sp.r.back(&mut sp.s);
        let before = pack(&sp, 0);
        let slot = before[..19].iter().position(|&v| v > 0).unwrap() as i16;
        let (carried, held) = (before[slot as usize], before[21]);
        let category = sp.item_category(0, slot);
        sp.equip_inventory_item(0, slot, 0);
        let after = pack(&sp, 0);
        let place = match category { 3 => 20, 4 => 19, _ => 21 };
        assert_eq!(after[place], carried, "category {category}");
        if place == 21 && held > 0 { assert!(after[..19].contains(&held), "the hand's old item is back in the pack"); }
        let mut sorted = (before.clone(), after.clone()); sorted.0.sort(); sorted.1.sort();
        assert_eq!(sorted.0, sorted.1, "nothing lost, nothing made");
        sp.unequip_inventory_item(0, place as i16 - 19);
        assert_eq!(pack(&sp, 0)[place], 0);
        assert!(pack(&sp, 0)[..19].contains(&carried));
        let now = pack(&sp, 0);
        let from = now[..19].iter().position(|&v| v > 0).unwrap() as i16;
        if !record::wide(sp.item_icon(0, from)) {
            let to = (0..19).find(|&i| now[i as usize] == 0 && i != from).unwrap();
            sp.s.set_word(record::SELECTED, from + 1);
            assert!(sp.can_swap_inventory_slots(0, to));
            sp.swap_inventory_slots(0, to);
            let swapped = pack(&sp, 0);
            assert_eq!((swapped[to as usize], swapped[from as usize], sp.s.word(record::SELECTED)), (now[from as usize], 0, 0));
        }
        let now = pack(&sp, 0);
        let Some(small) = (0..19).find(|&i| now[i as usize] > 0 && !record::wide(sp.item_icon(0, i))) else { return };
        let Some(hand) = [21u16, 22, 19, 20].into_iter().find(|&i| now[i as usize] > 0) else { return };
        for i in 0..19u16 { if now[i as usize] == 0 { sp.s.set_pword(0x1CE + 2 * i, now[small as usize]); } }
        let full = pack(&sp, 0);
        sp.unequip_inventory_item(0, hand as i16 - 19);
        assert_eq!(pack(&sp, 0), full, "no room: nothing moves");
    }
    #[test]
    fn the_gun_station_says_what_to_do_now() {
        use crate::space::combat::{MISSILES, TARGET_Y, CHARGE};
        let Some(mut sp) = crewed(&[]) else { return };
        sp.product = true; sp.s.set_word(STATION, 12); sp.s.set_word(STATION_ACTIVE, 1); sp.s.set_pbyte(0xC9, 9);
        for i in 0..20 { sp.s.set_pbyte(0xAC + i, 0); } for i in 0..4 { sp.s.set_byte(MISSILES + i, 0); sp.s.set_array(TARGET_Y, i as i16, -1); }
        sp.s.set_word(CHARGE + 2, 100); sp.s.set_word(ENCOUNTER, 0);
        let line = |sp: &Space| sp.gun_line(1).map(|t| String::from_utf8(t).unwrap());
        assert_eq!(line(&sp).as_deref(), Some("COMP: run Launch first"));
        sp.s.set_pbyte(0xAC, 2 | 0x20); assert_eq!(line(&sp).as_deref(), Some("Click Rel: load a tube"));
        sp.s.set_byte(MISSILES, 3); assert_eq!(line(&sp), None, "nobody to shoot at: the 1990 title");
        sp.s.set_word(ENCOUNTER, 1); assert_eq!(line(&sp).as_deref(), Some("Purple arrows: find hostile"));
        sp.s.set_array(TARGET_Y, 0, 200); assert_eq!(line(&sp).as_deref(), Some("Click Tar (red cross-hair)"), "the button as it is labelled");
        sp.gun_note = sp.tube_line(); assert_eq!(line(&sp).as_deref(), Some("Click tube 1"));
        sp.s.set_byte(MISSILES + 2, 3); assert_eq!(sp.tube_line(), "Click tube 1 or 3");
        let px = |sp: &Space, t: &str| t.bytes().map(|b| sp.s.byte(0x288A + b as u16) as u32).sum::<u32>();
        for t in ["COMP: run Launch first", "Click Rel: load a tube", "Gun charging: wait", "Purple arrows: find hostile", "Click Tar (red cross-hair)", "Click tube 1 or 2 or 3 or 4",
            "Sight on ship? Click Fire", "No damage. Click Target again", "Missed. Click Target again", "Destroyed. Next hostile", "Click Target first", "That tube is empty", "Hostile too close"] {
            assert!(px(&sp, t) <= 180, "{t}: {} px", px(&sp, t));
        }
        sp.gun_note.clear(); sp.s.set_pbyte(0xC9, 1); assert_eq!(line(&sp).as_deref(), Some("Click Target"), "the laser's button reads Target");
        sp.s.set_word(CHARGE + 2, 0); assert_eq!(line(&sp).as_deref(), Some("Gun charging: wait"));
        sp.s.set_pbyte(0xC9, 0); assert_eq!(line(&sp).as_deref(), Some("GUN2: no weapon fitted"), "an empty bay says so");
        {
            let mut ui = GunUi { scratch: vec![0; 65536], bp: TOP_FRAME };
            sp.redraw_gun_station(1, &mut ui).unwrap();
            let page = (sp.s.word(ACTIVE_PAGE) & 1) as usize;
            let px = sp.r.fb.indexed(page).to_vec();
            let lit: Vec<(usize, usize)> = (100..150).flat_map(|y| (157..278).map(move |x| (y, x))).filter(|&(y, x)| px[y * 320 + x] == 7).collect();
            assert!(!lit.is_empty() && lit.iter().all(|&(y, x)| (116..135).contains(&y) && (166..270).contains(&x)), "the words sit inside the view: {:?}", (lit.first(), lit.last()));
            sp.product = false; sp.redraw_gun_station(1, &mut ui).unwrap(); sp.product = true;
            let px = sp.r.fb.indexed((sp.s.word(ACTIVE_PAGE) & 1) as usize);
            assert!((100..150).all(|y| (157..278).all(|x| px[y * 320 + x] != 7)), "the 1990 view stays black");
        }
        sp.s.set_pbyte(0xC9, 9);
        let mut ui = GunUi { scratch: vec![0; 65536], bp: TOP_FRAME };
        sp.redraw_gun_station(1, &mut ui).unwrap();
        let words = |sp: &Space| [0x49EE, 0x2A15, 0x2792, 0x2794].map(|at| sp.s.word(at));
        let (rands, before, pen, shown) = (sp.ev.rands, words(&sp), sp.r.pen, sp.r.fb.indexed(0).to_vec());
        sp.gun_note = "Sight on ship? Click Fire".into(); sp.gun_say(1);
        assert_eq!((sp.ev.rands, words(&sp), sp.r.pen), (rands, before, pen));
        let now = sp.r.fb.indexed(0).to_vec();
        assert!((0..64000).any(|i| now[i] != shown[i]) && (0..64000).all(|i| now[i] == shown[i] || (89..100).contains(&(i / 320)) && (124..313).contains(&(i % 320))), "only the title bar changes");
        sp.product = false; sp.gun_note = "Click Tar first".into(); sp.gun_say(1); assert!(sp.r.fb.indexed(0)[..] == now[..], "the 1990 draws no such words");
    }
    #[test]
    fn the_ship_status_rows_stand_in_columns() {
        use mt_engine::present::textlog;
        let Some(mut sp) = crewed(&[]) else { return };
        textlog::set_enabled(true);
        sp.s.set_word(STATION, 14); sp.s.set_word(STATION_ACTIVE, 1); sp.s.set_pbyte(0xC2, 25); sp.s.set_pbyte(0xC4, 93);
        let rows = |sp: &Space| -> Vec<(String, i64, i64, i64)> {
            let log = textlog::frame(&sp.r.fb, (sp.s.word(ACTIVE_PAGE) & 1) as usize);
            (0..4).map(|i| { let mut row: Vec<_> = log.visible.iter().filter(|r| (r.row() - (113 + 8 * i)).abs() <= 2 && (136..200).contains(&r.x) && r.bytes[0] != 32).collect(); row.sort_by_key(|r| r.x);
                let colon = row.iter().find(|r| r.bytes[0] == b':').map_or(-1, |r| r.x);
                (row.iter().map(|r| match r.bytes[0] { b @ 0xB0..=0xB9 => (b - 0x80) as char, b => b as char }).collect(), row.first().map_or(-1, |r| r.x), colon, row.last().map_or(-1, |r| r.x + r.width as i64)) }).collect()
        };
        let words = |rows: &[(String, i64, i64, i64)]| rows.iter().map(|r| r.0.clone()).collect::<Vec<_>>();
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        h.call("draw_engineer_station", &[]).unwrap();
        let old = rows(h.sp);
        assert_eq!(words(&old), ["comp:75", "man:100", "gun1:7", "gun2:100"], "the 1990 rows");
        assert!(old[0].2 != old[1].2 && old[2].3 != old[3].3, "the 1990 colon and number follow the name: {old:?}");
        h.sp.product = true;
        h.call("draw_engineer_station", &[]).unwrap();
        let new = rows(h.sp);
        assert_eq!(words(&new), words(&old));
        assert!(new.iter().all(|r| (r.1, r.2, r.3) == (new[0].1, new[0].2, new[0].3)) && new[0].1 == 136, "one column each: {new:?}");
        h.call("select_draw_page", &[]).unwrap();
        h.bars(Some(true));
        let before = h.sp.r.fb.indexed((h.sp.s.word(ACTIVE_PAGE) & 1) as usize).to_vec();
        for (row, colour) in [(0, 6), (0, 8), (1, 6)] { h.call("outline_station_item", &[0x8B6, DS as i32, row, colour]).unwrap(); }
        assert_eq!(rows(h.sp), new, "the bars touch no letter");
        assert!(h.sp.r.fb.indexed((h.sp.s.word(ACTIVE_PAGE) & 1) as usize)[..] != before[..], "the mark is drawn");
        h.bars(Some(false));
        h.call("outline_station_item", &[0x8B6, DS as i32, 0, 6]).unwrap();
        assert_ne!(words(&rows(h.sp)), words(&new), "the 1990 outline runs through the letters");
    }
    #[test]
    fn the_crew_record_words_view_and_close_take_a_click() {
        use mt_engine::{io::Input, present::textlog, play::Decision};
        struct Seen(std::rc::Rc<std::cell::RefCell<Vec<Decision>>>);
        impl Input for Seen {
            fn wants_decision(&self) -> bool { true }
            fn decide(&mut self, d: &Decision) { self.0.borrow_mut().push(d.clone()); }
            fn next_key(&mut self) -> u16 { 0 }
        }
        let Some(mut sp) = crewed(&[]) else { return };
        textlog::set_enabled(true);
        let seen: std::rc::Rc<std::cell::RefCell<Vec<Decision>>> = Default::default();
        sp.product = true; sp.input = Box::new(Seen(seen.clone()));
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        h.call("back_buffer", &[]).unwrap();
        h.call("set_viewport", &[53, 0, 147, 40]).unwrap();
        let p = h.call("load_layout", &[0x258E, DS as i32]).unwrap();
        h.write(0x46EE, DS as i32, p.long(), 4);
        h.call("draw_inventory_panel", &[0, 1]).unwrap();
        h.call("select_draw_page", &[]).unwrap();
        let log = textlog::frame(&h.sp.r.fb, (h.sp.s.word(ACTIVE_PAGE) & 1) as usize);
        let mut line: Vec<_> = log.visible.iter().filter(|r| (58..=62).contains(&r.y) && r.x >= 200 && r.bytes[0] != 32).collect(); line.sort_by_key(|r| r.x);
        let word: String = line.iter().map(|r| r.bytes[0] as char).collect();
        assert_eq!(word, "ViewClose", "the words on the sheet");
        let (left, right) = (line[4].x as i32, line[8].x as i32 + line[8].width as i32);
        crate::space::play::record(h.sp);
        let d = seen.borrow().last().cloned().expect("the record's wait is published");
        let rect = |id: &str| d.rects.iter().find(|(k, _)| d.options[*k].id == id).map(|r| r.1);
        assert_eq!(rect("cmd:activate"), Some([233, 58, 36, 11]), "View = the 1990 region under the word");
        let close = rect("back").expect("Close has a place to click");
        assert!((close[0] - left).abs() <= 1 && (close[0] + close[2] - right).abs() <= 2 && close[1] <= line[4].y as i32 && close[1] + close[3] >= line[4].y as i32 + 6,
            "Close's box {close:?} is its drawn word (x {left}..{right}, y {})", line[4].y);
        assert!(line[..4].iter().all(|r| (233..269).contains(&(r.x as i32))), "the word view lies in View's region");
    }
    #[test]
    fn crew_record_frames() {
        let Some(mut sp) = crewed(&[32]) else { return };
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        h.call("back_buffer", &[]).unwrap();
        h.call("set_viewport", &[53, 0, 147, 40]).unwrap();
        let p = h.call("load_layout", &[0x258E, DS as i32]).unwrap();
        h.write(0x46EE, DS as i32, p.long(), 4);
        h.call("draw_inventory_panel", &[0, 1]).unwrap();
        h.call("select_draw_page", &[]).unwrap();
        h.call("highlight_inventory_action", &[0]).unwrap();
        ppm(h.sp, "record.ppm");
        for row in [0, 1, 4, 5, 6, 7] {
            h.sp.input = Box::new(Scripted::new([32]));
            h.sp.hooks = Some(Box::new(Shot(format!("detail{row}.ppm"))));
            h.call("show_inventory_action_details", &[0, row]).unwrap();
        }
        let shown = h.sp.r.fb.indexed((h.sp.s.word(ACTIVE_PAGE) & 1) as usize);
        assert!(shown.iter().filter(|&&c| c != 0).count() > 8000, "the record is drawn");
    }
    #[test]
    fn station_error_keeps_the_session() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(Scripted::new([32])), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        sp.product = true; sp.r.initialize(&mut sp.s);
        sp.s.set_word(PARTY_PTR, 0); sp.s.set_word(PARTY_PTR + 2, PARTY_SEG as i16);
        sp.s.set_word(PREVIOUS_STATION, 16);
        sp.run_stations();
        assert_eq!((sp.s.word(STATION), sp.s.word(PREVIOUS_STATION), sp.s.byte(KEY), sp.dead), (-1, -1, 0, false));
    }
}
