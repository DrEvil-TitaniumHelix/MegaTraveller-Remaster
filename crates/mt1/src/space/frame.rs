use super::{Space, state::*, render::Render, tick};
use mt_engine::present::{ly, prim::Raster};

impl Space {
    pub fn redraw_active_station(&mut self) {
        if self.s.word(STATION_ACTIVE) == 0 { return; }
        self.r.back(&mut self.s);
        self.redraw_station();
    }
    pub(super) fn top_layout(&mut self, index: i16, y: i16, x: i16) {
        let page = Render::page(&self.s);
        ly::layout(&mut self.r.fb, Raster::Wrap, page, self.r.assets.layout("SPTOP.LY"), index as usize, y as i32, x as i32, &mut self.r.pen);
    }
    fn redraw_station(&mut self) {
        let labels = self.ds_labels(0xDB6);
        let color = if self.s.word(VIDEO_MODE) == 2 { 8 } else { 9 };
        self.r.layout(&self.s, 1, 0x36, 0);
        for i in 0..6u16 {
            if self.s.word(tick::ACTIVE + 2 * i) != -1 {
                let x = match i { 0 => 0xF3, 5 => 0x9F, _ => 0xBB };
                self.set_graphics_color(color); self.r.layout(&self.s, 9, 0x4D, x);
            }
        }
        self.draw_labels(&labels, 0x35, 0);
        for i in 0..8u16 {
            let crew = self.s.pbyte(0xA0 + i);
            if crew == 0 { continue; }
            let h = if i == 0 || i == 7 { 0x15 } else { 0xD };
            self.r.fill(&self.s, 0x3E, (i * 0x1C + 0x4A) as i16, h, 0x1A, crew);
            let who = (crew as u16).wrapping_sub(1).wrapping_mul(256);
            let (symbol, level) = (self.s.pbyte(who.wrapping_add(0x122)), self.s.pbyte(who.wrapping_add(0x123)) as i16);
            self.draw_symbol_overlay(crew as i16 - 1, symbol, level, 0x3F, (i * 0x1C + 0x4B) as i16);
        }
        if self.s.pbyte(0xA7) == 0 {
            for i in 8..13u16 {
                if self.s.pbyte(0xA0 + i) != 0 && self.s.word(tick::ACTIVE) != -1 { self.r.layout(&self.s, 9, 0x54, 0xF2); }
            }
        }
        self.top_layout(9, 0, 0); self.top_layout(4, 2, 0xD0); self.top_layout(6, 0, 0);
        self.redraw_records();
        for i in 0..5 { self.draw_character_equipment(i, false); }
        self.set_graphics_color(0);
        for i in 0..5u16 {
            let at = i * 256;
            if self.s.pbyte(at + 0x127) == 0 { self.clear_record(i as i16); }
            else {
                let (symbol, level) = (self.s.pbyte(at + 0x122), self.s.pbyte(at + 0x123) as i16);
                self.draw_symbol_overlay(i as i16, symbol, level, 4, (i * 0x28 + 8) as i16);
            }
        }
        self.clear_party_panel();
    }
    fn redraw_records(&mut self) {
        for i in 0..5 { self.draw_character_health_bar(i); }
        self.draw_party_oxygen_bars();
    }
    pub fn scale_bar_value(value: i16, maximum: i16, extent: i16) -> i16 {
        if value > maximum { return extent; }
        if maximum == 0 { return 0; }
        if value > 0 { (value as i32 * extent as i32 / maximum as i32) as i16 } else { 0 }
    }
    pub fn draw_vertical_scaled_bar(&mut self, y: i16, x: i16, width: i16, color: u8, value: i16, maximum: i16, extent: i16) -> i16 {
        if value == 0 { return 0; }
        let size = Self::scale_bar_value(value, maximum, extent);
        self.r.fill(&self.s, y, x, extent.wrapping_sub(size), width, 0);
        self.r.fill(&self.s, y.wrapping_add(extent).wrapping_sub(size), x, size, width, color);
        size
    }
    fn draw_character_health_bar(&mut self, m: i16) {
        let at = (m as u16) * 256;
        let (current, maximum, threshold) = (self.s.pbyte(at + 0x127) as i16, self.s.pbyte(at + 0x125) as i16, self.s.pbyte(at + 0x126) as i16);
        self.draw_vertical_scaled_bar(3, m * 40 + 39, 4, (m + 1) as u8, current, maximum, 45);
        if current > 0 {
            let marker = Self::scale_bar_value(threshold, maximum, 45);
            self.r.fill(&self.s, 48 - marker, m * 40 + 39, 2, 4, 6);
        }
    }
    pub(super) fn draw_party_oxygen_bars(&mut self) {
        for c in 0..5i16 {
            let at = (c as u16) * 256;
            if self.s.pbyte(at + 0x127) != 0 {
                let value = self.s.pbyte(at + 0x128) as i16;
                let color = if self.inventory_slot_empty(c, 19) { 120 } else { 6 };
                self.r.fill(&self.s, 3, c * 40 + 34, 45 - value, 4, 0);
                self.r.fill(&self.s, 48 - value, c * 40 + 34, value, 4, color);
            }
        }
    }
    pub fn inventory_slot_empty(&self, character: i16, slot: i16) -> bool {
        self.s.pword((character as u16).wrapping_mul(256).wrapping_add(0x1CE).wrapping_add(2 * slot as u16)) == 0
    }
    fn clear_record(&mut self, n: i16) { self.r.fill(&self.s, 2, n * 40 + 7, 46, 36, 0); }
    pub(super) fn clear_party_panel(&mut self) { self.hide_cursor(); self.restore_display_rectangle(0, 0, 0x35, 0x28); self.show_cursor(); }
}
