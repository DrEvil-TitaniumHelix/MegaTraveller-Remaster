use super::{assets::Assets, state::*, system::*};
use mt_engine::present::{fb::Fb, ly::{self, Pen}, pic, prim::Raster, text::{self, Font}};

pub struct Render { pub assets: Assets, pub fb: Fb, pub font: Font, pub pen: Pen }
impl Render {
    pub fn new(s: &State, assets: Assets) -> Result<Self, String> {
        let font = Font::from_tables(assets.layout("FONT.LY").to_vec(), s.image(), 0x288A, 0x2E51, 8).ok_or("S: font tables missing")?;
        Ok(Self { assets, fb: Fb::new(), font, pen: Pen::new(0) })
    }
    pub fn page(s: &State) -> usize { ((s.word(0x2A15) as u16).wrapping_sub(0xA000) as usize / 512) % 8 }
    pub fn front(&mut self, s: &mut State) {
        let p = (s.word(ACTIVE_PAGE) as u16) & 1;
        s.set_word(0x49EE, 1);
        let mode = s.word(VIDEO_MODE);
        if !(0..=2).contains(&mode) { return; }
        s.set_word(0x2A15, if mode == 0 { (0xA000 + p * 512) as i16 } else { 0xB800u16 as i16 });
        s.set_word(0x2792, s.word(0x49BE + (p ^ 1) * 4)); s.set_word(0x2794, s.word(0x49C0 + (p ^ 1) * 4));
    }
    pub fn back(&mut self, s: &mut State) {
        let p = (s.word(ACTIVE_PAGE) as u16) & 1;
        s.set_word(0x49EE, 0);
        let mode = s.word(VIDEO_MODE);
        if !(0..=2).contains(&mode) { return; }
        s.set_word(0x2A15, if mode == 0 { (0xA000 + (p ^ 1) * 512) as i16 } else { s.word(0x49C8) });
        s.set_word(0x2792, s.word(0x49BE + p * 4)); s.set_word(0x2794, s.word(0x49C0 + p * 4));
    }
    pub fn flip(&mut self, s: &mut State) { s.set_word(ACTIVE_PAGE, s.word(ACTIVE_PAGE) ^ 1); self.back(s); }
    pub fn initialize(&mut self, s: &mut State) {
        s.set_word(VIDEO_MODE, 0);
        for (at, v) in [(0x49D2, 0), (0x49BA, 0), (0x49C6, 0xC8), (0x49BC, 0x28), (0x279A, 4), (0x2798, 1)] { s.set_word(at, v); }
        for (at, seg) in [(0x499A, 0xA000u16), (0x499E, 0xA200), (0x49A2, 0xA400), (0x49A6, 0xA600), (0x49AA, 0xA800), (0x49AE, 0xAA00), (0x49B2, 0xAC00), (0x49B6, 0xAE00), (0x49BE, 0xA200), (0x49C2, 0xA000)] {
            s.set_word(at, 0); s.set_word(at + 2, seg as i16);
        }
        s.set_byte(0x49F8, 8);
        s.set_word(ACTIVE_PAGE, 0); self.front(s);
        pic::atlas(&mut self.fb, &self.assets.stars, 2 * mt_engine::present::fb::PAGE);
    }
    pub fn fill(&mut self, s: &State, y: i16, x: i16, h: i16, w: i16, color: u8) { self.fb.fill(Raster::Wrap, Self::page(s), y as i32, x as i32, h as i32, w as i32, color, self.pen.op); }
    pub fn pixel(&mut self, s: &State, y: i16, x: i16, color: u8) { self.fb.pixel(Raster::Wrap, Self::page(s), y as i32, x as i32, color, self.pen.op); }
    pub fn layout(&mut self, s: &State, index: i16, y: i16, x: i16) {
        let entries = self.assets.layout("STATIONS.LY");
        if index >= 0 { ly::layout(&mut self.fb, Raster::Wrap, Self::page(s), entries, index as usize, y as i32, x as i32, &mut self.pen); }
    }
    pub fn text(&mut self, s: &State, value: &[u8], y: i16, x: i16, color: u8) -> i16 {
        let next = text::draw(&mut self.fb, Raster::Wrap, Self::page(s), &self.font, value, y as i32, x as i32, color, None);
        self.pen.colour = color; self.pen.op = 0;
        next as i16
    }
    pub fn labels(&mut self, s: &State, labels: &[(i16, i16, Option<Vec<u8>>, u8)]) {
        for (y, x, value, color) in labels { if let Some(value) = value { self.text(s, value, *y, *x, *color); } }
    }
    pub fn controls(&mut self, s: &State) { for y in [79, 91, 103] { self.fill(s, y, 253, 11, 60, 7); } }
    pub fn thrust(&mut self, s: &mut State, value: i16, mut color: u8) {
        if value == 0 { return; }
        let glyph = if value < 0 { if color != 7 { color = 2; } 208u16.wrapping_sub(value as u16) } else { 208u16.wrapping_add(value as u16) } as u8;
        if s.pbyte(0xFC) == 0 && color != 7 { return; }
        self.back(s); self.text(s, &[glyph], 161, 297, color);
        self.front(s); self.text(s, &[glyph], 161, 297, color);
    }
    pub fn plot_minimap(&mut self, s: &mut State, x: i16, y: i16, selected: i16, color: u8) {
        if s.word(CURRENT) != -1 { return; }
        let mut quadrant = 1;
        let py = if y < 186 { (y as i32 * 58) / 186 } else { quadrant = 4; ((y - 186) as i32 * 58) / 186 };
        let px = if x < 186 { quadrant = if quadrant == 1 { 2 } else { 3 }; (x as i32 * 59) / 186 } else { ((x - 186) as i32 * 59) / 186 };
        if quadrant != selected { return; }
        self.front(s); self.pixel(s, (py + 3) as i16, (px + 253) as i16, color);
        self.back(s); self.pixel(s, (py + 3) as i16, (px + 253) as i16, color);
    }
    pub fn update_minimap_quadrant(&mut self, s: &mut State) {
        if s.word(CURRENT) > -1 { return; }
        let (px, py) = (s.word(X), s.word(Y));
        let mut quadrant = if py < 186 { 1 } else { 4 };
        if px < 186 { quadrant = if quadrant == 1 { 2 } else { 3 }; }
        self.back(s);
        if s.word(QUADRANT) != quadrant {
            s.set_word(QUADRANT, quadrant);
            self.front(s); self.fill(s, 2, 252, 59, 62, 0);
            self.back(s); self.fill(s, 2, 252, 59, 62, 0);
        }
        self.plot_minimap(s, px, py, quadrant, 12);
    }
}

pub const PAUSE_ROW: (i16, i16, i16, i16) = (115, 253, 11, 60);
pub const OUT_OF_FUEL: &str = "Out of fuel: the engines are off. The ship drifts.";
pub fn fuel_percent(fuel: u8) -> u16 { (fuel as u16 * 100).div_ceil(124) }
impl super::Space {
    pub fn copy(&mut self, src: usize, sy: i16, sx: i16, h: i16, w: i16, dst: usize, dy: i16, dx: i16) {
        self.r.fb.copy(src, sy as i32, sx as i32, h as i32, w as i32, dst, dy as i32, dx as i32);
        self.ev.copies = self.ev.copies.wrapping_add(1);
        self.event(crate::lockstep::SpaceEvent::Copy);
    }
    pub fn hud(&mut self, mode: i16) {
        self.hud_mode = mode;
        let s = &self.s;
        let mut labels: Vec<(i16, i16, Option<Vec<u8>>, u8)> = (0..10u16).map(|i| { let at = 0xCB3 + 9 * i; (s.word(at), s.word(at + 2), s.pointer_string(at + 4), s.byte(at + 8)) }).collect();
        let mut title = s.pointer_string(SYSTEM_NAMES + s.pbyte(6) as u16 * 4).unwrap_or_default();
        title.extend_from_slice(b" in-system travel");
        if self.product { title = format!("Flying in the {} system", super::jump_host::proper(&title[..title.len() - 17])).into_bytes(); }
        labels[0].2 = Some(title);
        if self.product { for l in &mut labels { if let Some(t) = l.2.take() { l.2 = Some(super::ui::comms(t)); } } }
        if self.product { let page = Render::page(&self.s); for y in 2..12 { let c = self.r.fb.get(page, y, 244); self.r.fill(&self.s, y as i16, 8, 1, 236, c); } }
        else { self.r.fill(&self.s, 3, 8, 6, 239, 6); }
        if mode >= 0 {
            for (i, label) in labels.iter_mut().enumerate().take(8) { label.3 = if i == 0 { 6 } else { 7 }; }
            if self.product { labels[0].2 = None; }
            if mode != 99 { self.r.labels(&self.s, &labels); }
            self.r.controls(&self.s);
            let s = &self.s;
            let location = if mode == 55 { s.string(if s.pbyte(0xC1) & 24 == 24 { 0x173E } else { 0x175C }) } else {
                let world = s.array(WORLD, s.word(CURRENT)).wrapping_add(s.array(WORLD_BASE, s.pbyte(6) as i16)).wrapping_sub(1);
                let mut value = b"tethered to ".to_vec();
                value.extend(s.pointer_string(WORLD_NAMES.wrapping_add((world as u16).wrapping_mul(4))).unwrap_or_default());
                value.extend_from_slice(b" space dock");
                if self.product {
                    let name = super::jump_host::proper(&value[12..value.len() - 11]);
                    value = format!("Docked at {name}. Take off to fly or jump.").into_bytes();
                    if self.text_width(&value) > 232 { value = b"Docked. Take off to fly or jump.".to_vec(); }
                }
                value
            };
            labels[0].2 = if self.product && mode == 99 { None } else { Some(location) };
            labels[3].2 = if mode != 55 { Some(s.string(0x1779)) } else { None };
            labels[4].2 = Some(s.string(0x177E)); labels[5].2 = None;
            if mode == 99 { self.r.fill(&self.s, 2, 252, 59, 62, 0); self.r.labels(&self.s, &labels); return; }
            let draw = Render::page(&self.s); let source = (1 - self.s.word(ACTIVE_PAGE)) as usize & 1;
            self.copy(source, 12, 1, 160, 30, draw, 12, 1);
            self.r.fill(&self.s, 2, 252, 59, 62, 0);
            self.copy(2, 112, if mode == 55 { 0 } else { 8 }, 59, 8, draw, 2, 31);
            for label in labels.iter_mut().take(8) { label.3 = 0; }
        }
        self.r.controls(&self.s);
        if mode != -2 { self.r.labels(&self.s, &labels); } else { self.r.fill(&self.s, 67, 253, 11, 60, 7); }
        if self.s.pbyte(0xFC) != 0 { let f = self.s.pbyte(0xFC) as i16; self.r.fill(&self.s, 183, 4, 5, f, 9); }
        if self.product && mode != -2 { self.r.fill(&self.s, PAUSE_ROW.0, PAUSE_ROW.1, PAUSE_ROW.2, PAUSE_ROW.3, 7); self.panel_words(); }
    }
    pub fn panel_words(&mut self) {
        self.r.fill(&self.s, 69, 259, 8, 54, 7); self.r.text(&self.s, b"Stations", 69, 259, 0);
        self.r.fill(&self.s, 130, 255, 8, 37, 6); self.r.text(&self.s, b"rotate", 130, 255, 0);
        self.r.fill(&self.s, PAUSE_ROW.0 + 2, 259, 8, 32, 7); self.r.text(&self.s, b"Pause", PAUSE_ROW.0 + 2, 259, 0);
        if self.hud_mode == -1 {
            let at_object = self.s.word(CURRENT) >= 0;
            for (i, tethered) in [(3u16, 0x1779u16), (4, 0x177E)] {
                let at = 0xCB3 + 9 * i; let (y, x, color) = (self.s.word(at), self.s.word(at + 2), self.s.byte(at + 8));
                let text = if at_object { self.s.string(tethered) } else { self.s.pointer_string(at + 4).unwrap_or_default() };
                self.r.fill(&self.s, y, 253, 11, 60, 7); self.r.text(&self.s, &text, y, x, color);
            }
        }
        self.readout(false);
    }
    pub fn readout(&mut self, both: bool) {
        if !self.product { return }
        let keep = ([0x49EE, 0x2A15, 0x2792, 0x2794].map(|at| (at, self.s.word(at))), self.r.pen);
        let (t, dry) = (self.s.word(THRUST), self.s.pbyte(0xFC) == 0);
        let (number, ink) = (t.to_string().into_bytes(), match t { 0 => 0, 1.. => 9, _ => 2 });
        let x = 293 + (21 - self.text_width(&number)) / 2;
        let percent = format!("{}%", fuel_percent(self.s.pbyte(0xFC))).into_bytes();
        let docked = { let c = self.s.word(CURRENT); c >= 0 && (self.s.array(WORLD, c) != 0 || self.s.array(KIND, c) == 4) };
        let lines = if !dry { vec![] } else { self.wrap(if docked { "Out of fuel: the engines are off." } else { OUT_OF_FUEL }, 143) };
        for front in [true, false] {
            if both { if front { self.r.front(&mut self.s) } else { self.r.back(&mut self.s) } } else if front { continue }
            self.r.fill(&self.s, 158, 293, 19, 21, 7); self.r.text(&self.s, &number, 163, x, ink);
            self.r.fill(&self.s, 175, 76, 7, 51, 6); self.r.text(&self.s, &percent, 175, 78, 0);
            self.r.fill(&self.s, 175, 135, 21, 151, 6);
            for (i, l) in lines.iter().enumerate() { self.r.text(&self.s, l, 177 + 9 * i as i16, 139, 15); }
            if dry { self.r.fill(&self.s, 183, 4, 5, 124, 0); self.r.fill(&self.s, 183, 3, 5, 1, 8); }
        }
        for (at, v) in keep.0 { self.s.set_word(at, v); }
        self.r.pen = keep.1;
    }
    pub fn paused_badge(&mut self, text: &[u8]) {
        let w = text.len() as i16 * 8 + 16; let x = (8 + (240 - w) / 2).max(8);
        for front in [true, false] {
            if front { self.r.front(&mut self.s) } else { self.r.back(&mut self.s) }
            self.r.fill(&self.s, 86, x, 14, w, 0); self.r.text(&self.s, text, 89, x + 8, 15);
        }
        self.r.front(&mut self.s);
    }
    pub fn frame(&mut self) { self.r.back(&mut self.s); self.r.layout(&self.s, 21, 0, 0); self.hud(-1); self.r.flip(&mut self.s); self.r.layout(&self.s, 21, 0, 0); self.hud(-1); }
    pub fn object(&mut self, sprite: i16, x: i16, y: i16, size: i16) {
        if !(0..38).contains(&sprite) || size <= 0 { return; }
        let tile = self.s.array(0xA0C, sprite); let page = Render::page(&self.s);
        for cell in 0..(size as i32 * size as i32) {
            let (col, row) = (cell % size as i32, cell / size as i32);
            let (px, py) = (x as i32 + col, y as i32 + row);
            if (0..=14).contains(&px) && (0..=9).contains(&py) {
                let tile = tile as i32 + cell;
                self.copy(2, (tile / 20 * 16) as i16, (tile % 20 * 2) as i16, 16, 2, page, (py * 16 + 12) as i16, (px * 2 + 1) as i16);
            }
        }
    }
}
