use super::{Space, input::KEY_ASCII, render::Render, state::*};
use mt_engine::present::prim::Raster;

pub const HOTSPOTS: u16 = 0x48D3;

impl Space {
    pub fn outline_menu_item(&mut self, rects: &[[i16; 4]], index: i16, color: u8) {
        let Some(r) = usize::try_from(index).ok().and_then(|i| rects.get(i)) else { return };
        let page = Render::page(&self.s);
        self.r.fb.rect(Raster::Wrap, page, r[0] as i32, r[1] as i32, (r[0] + r[2] - 1) as i32, (r[1] + r[3]) as i32, color, 0);
    }
    pub fn select_rect(&mut self, rects: &[[i16; 4]], selected: i16, highlight: i16, background: i16, hotkeys: Option<&[u8]>) -> i16 {
        self.select_rect_or_handover(rects, selected, highlight, background, hotkeys, false).unwrap_or(-1)
    }
    pub fn select_rect_or_handover(&mut self, rects: &[[i16; 4]], selected: i16, highlight: i16, background: i16, hotkeys: Option<&[u8]>, handover: bool) -> Option<i16> {
        let count = rects.len() as i16;
        let machine = handover && self.input.data_mode();
        self.drop_stale_clicks();
        self.hide_cursor();
        let accept_t = hotkeys.is_some_and(|k| !k.contains(&b'T'));
        let highlight = if self.s.word(VIDEO_MODE) == 2 { 0 } else { highlight as u8 };
        let background = background as u8;
        let mut selected = if selected < 0 || selected >= count { 0 } else { selected };
        for (i, r) in rects.iter().enumerate() {
            let at = HOTSPOTS.wrapping_add(9 * i as u16);
            for (k, v) in r.iter().enumerate() { self.s.set_word(at.wrapping_add(2 * k as u16), *v); }
            self.s.set_byte(at.wrapping_add(8), (i + 1) as u8);
        }
        self.s.set_byte(HOTSPOTS.wrapping_add(9 * count as u16).wrapping_add(8), 0);
        self.r.front(&mut self.s);
        self.outline_menu_item(rects, selected, highlight);
        let mut active = true;
        let mut deadline = self.ticks().wrapping_add(9);
        if self.input.wants_decision() { self.play_rects = rects.iter().map(|r| [r[1] as i32, r[0] as i32, r[3] as i32, r[2] as i32]).collect(); }
        super::play::selector(self,"D7B4",rects.len(),selected,hotkeys);
        loop {
            self.s.set_byte(KEY, 0);
            self.show_cursor();
            while self.s.byte(KEY) == 0 {
                self.update_cursor();
                if machine && !self.input.data_mode() && self.s.byte(KEY) == 0 {
                    self.hide_cursor(); self.r.front(&mut self.s);
                    if active { self.outline_menu_item(rects, selected, background); }
                    self.r.back(&mut self.s);
                    return None;
                }
                if self.ticks() > deadline {
                    self.hide_cursor();
                    self.r.front(&mut self.s);
                    deadline = self.ticks().wrapping_add(9);
                    self.outline_menu_item(rects, selected, if active { background } else { highlight });
                    active = !active;
                    self.show_cursor();
                }
            }
            self.hide_cursor();
            self.r.front(&mut self.s);
            let key = self.s.byte(KEY);
            match key {
                0x6E => {
                    self.mouse_command(HOTSPOTS);
                    let hit = self.s.byte(KEY);
                    if hit == 0x6E { self.s.set_byte(KEY, 0); continue; }
                    if active { self.outline_menu_item(rects, selected, background); }
                    selected = hit as i16 - 1;
                    self.s.set_byte(KEY, 0); self.s.set_byte(super::mouse::PENDING, 0);
                    break;
                }
                0x1C => { if active { self.outline_menu_item(rects, selected, background); } break; }
                0x14 if accept_t => { if active { self.outline_menu_item(rects, selected, background); } break; }
                0x4B | 0x48 | 0x4D | 0x50 => {
                    if active { self.outline_menu_item(rects, selected, background); }
                    selected += if key == 0x4B || key == 0x48 { -1 } else { 1 };
                    if selected < 0 { selected = count - 1; }
                    if selected >= count { selected = 0; }
                    self.outline_menu_item(rects, selected, highlight);
                    active = true;
                    deadline = self.ticks().wrapping_add(9);
                }
                0x3B..=0x44 if i16::from(key - 0x3B) < count => { if active { self.outline_menu_item(rects, selected, background); } selected = (key - 0x3B) as i16; break; }
                1 => { if active { self.outline_menu_item(rects, selected, background); } selected = -1; break; }
                _ => {
                    let translated = self.s.byte(KEY_ASCII.wrapping_add(key as u16));
                    if translated != 0 {
                        if let Some(i) = hotkeys.and_then(|k| k.iter().position(|&c| c == translated)) {
                            if active { self.outline_menu_item(rects, selected, background); }
                            selected = i as i16;
                            break;
                        }
                    }
                }
            }
        }
        self.r.back(&mut self.s);
        self.s.set_byte(KEY, 0);
        Some(selected)
    }
}
