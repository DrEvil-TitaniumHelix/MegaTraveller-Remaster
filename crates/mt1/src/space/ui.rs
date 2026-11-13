use super::{Space, state::*, render::Render};
use mt_engine::present::{pic, prim::Raster, text};

pub const TEXT_Y: u16 = 0x498F;
pub const TEXT_X: u16 = 0x4991;
pub const TAB_CHAR: u16 = 0x4993;
pub const TEXT_COLOR: u16 = 0x4994;
pub const TEXT_STYLE: u16 = 0x4995;
pub const TAB1: u16 = 0x298A;
pub const TAB2: u16 = 0x298C;
pub const MARGIN: u16 = 0x01B4;
pub const SHADOW_COLOR: u16 = 0x48D0;
pub const LAST_COLOR: u16 = 0x48D1;
pub const PEN_COLOR: u16 = 0x2A2A;
pub const CURSOR_MODE: u16 = 0x4868;
pub const SYMBOL_OFFSET: u16 = 0x298E;
pub const OVERLAY_OFFSET: u16 = 0x2993;
pub type Rect = [i16; 4];
pub type Label = (i16, i16, Option<Vec<u8>>, u8);

pub fn comms(mut text: Vec<u8>) -> Vec<u8> {
    for (from, to) in [(b"Commo", b's'), (b"COMMO", b'S')] {
        let mut at = 0;
        while let Some(i) = text[at..].windows(5).position(|w| w == from) { text[at + i + 4] = to; at += i + 5; }
    }
    text
}
impl Space {
    pub fn set_graphics_color(&mut self, c: u8) { self.r.pen.colour = c; self.s.set_byte(PEN_COLOR, c); }
    pub fn set_color(&mut self, c: i16) {
        let c = if self.s.word(VIDEO_MODE) == 2 && c != 0 { 6 } else { c };
        self.s.set_byte(TEXT_COLOR, c as u8); self.set_graphics_color(c as u8); self.s.set_word(LAST_COLOR, c);
    }
    pub fn set_text_position(&mut self, y: i16, x: i16) { self.s.set_word(TEXT_Y, y); self.s.set_word(TEXT_X, x); }
    fn glyph(&mut self, ch: u8, pipe: bool, double_s: bool) {
        if ch == self.s.byte(TAB_CHAR) {
            let x = self.s.word(TEXT_X);
            if x < self.s.word(TAB2) { let stop = if x >= self.s.word(TAB1) { TAB2 } else { TAB1 }; self.s.set_word(TEXT_X, self.s.word(stop)); }
            return;
        }
        if pipe && ch == b'|' { self.s.set_word(TEXT_X, self.s.word(MARGIN).wrapping_add(15)); return; }
        let mut ch = ch;
        if double_s && ch == 0xE1 { self.glyph(b's', pipe, double_s); ch = b's'; }
        let (y, x) = (self.s.word(TEXT_Y), self.s.word(TEXT_X));
        let shadow = (self.s.byte(TEXT_STYLE) & 1 != 0).then(|| self.s.word(SHADOW_COLOR) as u8);
        let colour = self.s.byte(TEXT_COLOR);
        let page = Render::page(&self.s);
        let next = text::glyph(&mut self.r.fb, Raster::Wrap, page, &self.r.font, ch, y as i32, x as i32, &mut self.r.pen, colour, shadow);
        let pen = self.r.pen.colour; self.s.set_byte(PEN_COLOR, pen);
        self.s.set_word(TEXT_X, next as i16);
    }
    pub fn draw_text_glyph(&mut self, ch: u8) { self.glyph(ch, true, false) }
    pub fn draw_text_character(&mut self, ch: u8) { self.glyph(ch, false, true) }
    pub fn draw_text(&mut self, s: &[u8]) {
        if self.product { for c in comms(s.to_vec()) { self.draw_text_glyph(c) } } else { for &c in s { self.draw_text_glyph(c) } }
    }
    pub fn draw_labels(&mut self, labels: &[Label], top: i16, left: i16) {
        for (y, x, text, color) in labels {
            if let Some(text) = text { self.set_text_position(y.wrapping_add(top), x.wrapping_add(left)); self.set_color(*color as i16); self.draw_text(text); }
        }
    }
    pub fn ds_labels(&self, at: u16) -> Vec<Label> {
        (0..).map(|i| at.wrapping_add(9 * i)).take_while(|&p| self.s.byte(p.wrapping_add(8)) != 255)
            .map(|p| (self.s.word(p), self.s.word(p.wrapping_add(2)), self.s.pointer_string(p.wrapping_add(4)), self.s.byte(p.wrapping_add(8)))).collect()
    }
    pub fn select_font(&mut self, bits: u8) {
        self.s.set_byte(TEXT_STYLE, self.s.byte(TEXT_STYLE) | bits);
        if self.s.word(VIDEO_MODE) == 2 { self.set_color(3); }
    }
    pub fn finish_text(&mut self, bits: u8) { self.s.set_byte(TEXT_STYLE, self.s.byte(TEXT_STYLE) & !bits); }
    pub fn set_text_origin(&mut self, first: i16, second: i16) { self.s.set_word(TAB1, first); self.s.set_word(TAB2, second); }
    pub fn set_viewport(&mut self, y: i16, x: i16, h: i16, w: i16) { for (at, v) in [(0x49D2, y), (0x49BA, x), (0x49C6, h), (0x49BC, w)] { self.s.set_word(at, v); } }
    pub fn set_cursor_mode(&mut self, mode: i16) { self.s.set_word(CURSOR_MODE, mode); }
    pub fn set_palette_color(&mut self, index: i16, color: i16) { self.combat_palette(index as u16, color as u8); }
    pub fn draw_line(&mut self, y1: i16, x1: i16, y2: i16, x2: i16, color: u8) {
        let page = Render::page(&self.s);
        self.r.fb.line(Raster::Wrap, page, y1 as i32, x1 as i32, y2 as i32, x2 as i32, color, self.r.pen.op);
    }
    pub fn draw_rectangle(&mut self, top: i16, left: i16, bottom: i16, right: i16, color: u8) {
        self.draw_line(top, left, top, right, color);
        self.draw_line(bottom, left, bottom, right, color);
        if bottom.wrapping_sub(top) > 0 {
            self.draw_line(top.wrapping_add(1), left, bottom.wrapping_sub(1), left, color);
            self.draw_line(top.wrapping_add(1), right, bottom.wrapping_sub(1), right, color);
        }
    }
    pub fn draw_station_icon(&mut self, icon: i16, y: i16, x: i16, selected: bool) {
        const HIGHLIGHT: [i16; 12] = [0, 0, 1, 0, 0, 0, 2, 3, 0, 0, 4, 0];
        let variant = HIGHLIGHT.get(icon as usize).copied().unwrap_or(0);
        let sy = (icon / 20) << 4;
        let (sx, w) = if selected && variant != 0 { ((variant - 1) * 4 + 24, 4) } else { ((icon % 20) << 1, 2) };
        for (at, value) in [(0x49F2, sy), (0x49F4, 16), (0x49EC, sx), (0x49F6, w)] { self.s.set_word(at, value); }
        let page = Render::page(&self.s);
        pic::draw_rect(&mut self.r.fb, page, &self.r.assets.space, y as i32, x as i32 * 8, sy as i32, sx as i32 * 8, 16, w as i32 * 8, None);
    }
    pub fn direction_sector(x: i16, y: i16) -> i16 { let mut q = if y >= 186 { 4 } else { 1 }; if x < 186 { q = if q == 1 { 2 } else { 3 }; } q }
    pub fn draw_symbol_overlay(&mut self, color: i16, symbol: u8, level: i16, y: i16, x: i16) {
        self.set_color(0);
        let off = self.s.byte(SYMBOL_OFFSET.wrapping_add(symbol as u16)) as i16;
        self.set_text_position(y, x.wrapping_add(off)); self.draw_text_character(symbol);
        if symbol != 3 && level > 0 {
            self.set_color(color.wrapping_add(1));
            let off = self.s.byte(OVERLAY_OFFSET.wrapping_add(symbol as u16)) as i16;
            self.set_text_position(y.wrapping_add(1), x.wrapping_add(off).wrapping_add((level == 1) as i16));
            self.draw_text_character((level as u8).wrapping_add(0xB0));
            self.set_color(0);
        }
    }
    pub fn beep(&mut self) {
        self.snapshot_clock(); self.wait_with_input(2);
        self.snapshot_clock(); self.wait_with_input(2);
    }
}
