use super::{fb::Fb, ly::{self, Pen}, prim::Raster};
use crate::data::ly::Entry;

pub struct Font { pub glyphs: Vec<Entry>, pub widths: [u8; 256], pub descend: [bool; 256] }
impl Font {
    pub fn new(glyphs: Vec<Entry>, widths: [u8; 256], flags: &[u8; 256], bit: u8) -> Font {
        let font = Font { glyphs, widths, descend: std::array::from_fn(|c| flags[c] & bit != 0 || (0xA1..=0xBF).contains(&c)) };
        super::textlog::register(&font);
        font
    }
    pub fn from_tables(glyphs: Vec<Entry>, data: &[u8], widths_at: usize, flags_at: usize, bit: u8) -> Option<Font> {
        let t = |at: usize| data.get(at..at.checked_add(256)?).map(|s| <[u8; 256]>::try_from(s).unwrap());
        Some(Font::new(glyphs, t(widths_at)?, &t(flags_at)?, bit))
    }
}
impl Drop for Font { fn drop(&mut self) { super::textlog::forget(self); } }

fn glyph64(fb: &mut Fb, r: Raster, page: usize, font: &Font, c: usize, y: i64, x: i64, pen: &mut Pen, colour: u8, shadow: Option<u8>) -> i64 {
    if c >= 256 { return x }
    let capture = super::textlog::begin(fb, font, page, c, y, x, if shadow.is_some() { colour } else { pen.colour }, shadow);
    let row = y.wrapping_add(if font.descend[c] { 2 } else { 0 });
    if let Some(bg) = shadow { ly::layout(fb, r, page, &font.glyphs, c, ly::cast(r, row.wrapping_add(1)), ly::cast(r, x.wrapping_add(1)), &mut Pen { colour: bg, ..*pen }); pen.colour = colour }
    ly::layout(fb, r, page, &font.glyphs, c, ly::cast(r, row), ly::cast(r, x), pen);
    super::textlog::finish(fb, capture, pen.colour);
    x.wrapping_add(font.widths[c] as i64)
}
pub fn glyph(fb: &mut Fb, r: Raster, page: usize, font: &Font, c: u8, y: i32, x: i32, pen: &mut Pen, colour: u8, shadow: Option<u8>) -> i64 {
    glyph64(fb, r, page, font, c as usize, y as i64, x as i64, pen, colour, shadow)
}
pub fn draw(fb: &mut Fb, r: Raster, page: usize, font: &Font, s: &[u8], y: i32, x: i32, colour: u8, shadow: Option<u8>) -> i64 {
    let mut pen = Pen::new(colour);
    s.iter().fold(x as i64, |x, &c| glyph64(fb, r, page, font, c as usize, y as i64, x, &mut pen, colour, shadow))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cursor { pub r: Raster, pub page: usize, pub y: i64, pub x: i64, pub pen: u8, pub colour: u8, pub shadow: bool, pub bg: u8, pub tab: u8, pub tabs: [i64; 2], pub window_x: i64, pub ops: [u8; 4] }
impl Cursor {
    pub fn ink(&mut self, c: u8) { self.pen = c; self.colour = c }
    pub fn at(&mut self, y: i64, x: i64) { self.y = y; self.x = x }
    pub fn glyph(&mut self, fb: &mut Fb, font: &Font, c: u8) { self.glyph64(fb, font, c as usize) }
    fn glyph64(&mut self, fb: &mut Fb, font: &Font, c: usize) {
        if c == self.tab as usize {
            if self.x < self.tabs[0] { self.x = self.tabs[0] } else if self.x < self.tabs[1] { self.x = self.tabs[1] }
        } else {
            let (mut pen, shadow) = (Pen { colour: self.pen, op: 0, ops: self.ops }, self.shadow.then_some(self.bg));
            self.x = glyph64(fb, self.r, self.page, font, c, self.y, self.x, &mut pen, self.colour, shadow);
            self.pen = pen.colour;
        }
    }
    pub fn text(&mut self, fb: &mut Fb, font: &Font, s: &[u8]) {
        for &c in s { if c == b'|' { self.x = self.window_x.wrapping_add(15) } else { self.glyph(fb, font, c) } }
    }
    pub fn counted_glyph(&mut self, fb: &mut Fb, font: &Font, colour: u8, glyph: u8, count: u8, y: i64, x: i64, off: &[u8], off2: &[u8]) {
        let Some(&o) = off.get(glyph as usize) else { return };
        self.ink(0); self.at(y, x.wrapping_add(o as i64)); self.glyph(fb, font, glyph);
        if glyph != 3 && count > 0 {
            let Some(&o2) = off2.get(glyph as usize) else { return };
            self.ink(colour.wrapping_add(1)); self.at(y.wrapping_add(1), x.wrapping_add(o2 as i64).wrapping_add((count == 1) as i64));
            self.glyph64(fb, font, 0xB0 + count as usize); self.ink(0);
        }
    }
    pub fn wrapped(&mut self, fb: &mut Fb, font: &Font, s: &[u8], y: i64, x: i64, w: i64, h: i64, lh: i64, colour: u8, bg: u8) {
        self.ink(colour); self.at(y, x);
        let (mut row, mut col, mut start) = (y, x, 0);
        while start < s.len() {
            let stop = s[(start + 1).min(s.len())..].iter().position(|&c| c == b' ').map_or(s.len(), |p| start + 1 + p);
            let mut word = &s[start..stop];
            let mut wid: i64 = word.iter().fold(0i64, |a, &c| a.wrapping_add(font.widths[c as usize] as i64));
            if col.wrapping_add(wid) > x.wrapping_add(w) {
                row = row.wrapping_add(lh); col = x;
                if word.first() == Some(&b' ') { word = &word[1..]; wid -= font.widths[b' ' as usize] as i64 }
                if lh.wrapping_mul(2).wrapping_add(row) > y.wrapping_add(h) {
                    self.at(row, col); self.text(fb, font, b"(more)");
                    fb.fill(self.r, self.page, ly::cast(self.r, y), ly::cast(self.r, x), ly::cast(self.r, h), ly::cast(self.r, w), bg, 0);
                    row = y; col = x;
                }
                self.at(row, col);
            }
            self.text(fb, font, word); col = col.wrapping_add(wid); start = stop;
        }
    }
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::data::ly::Rec;
    fn font() -> Font {
        let mut g: Vec<Entry> = vec![vec![]; 256];
        g[b'a' as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 1, x2: 1 }];
        g[b'g' as usize] = g[b'a' as usize].clone();
        g[b'c' as usize] = vec![Rec { op: 4, colour: 7, y1: 0, x1: 0, y2: 0, x2: 0 }];
        let mut w = [0u8; 256]; w[b'a' as usize] = 3; w[b'g' as usize] = 3; w[b'c' as usize] = 2; w[b' ' as usize] = 2;
        let mut f = [0u8; 256]; f[b'g' as usize] = 8;
        Font::new(g, w, &f, 8)
    }
    fn cursor() -> Cursor { Cursor { r: Raster::Clip, page: 1, y: 0, x: 0, pen: 4, colour: 4, shadow: false, bg: 0, tab: 9, tabs: [30, 60], window_x: 100, ops: ly::OPS } }
    #[test]
    fn s_text_and_t_cursor() {
        let (f, mut fb) = (font(), Fb::new());
        assert!(f.descend[b'g' as usize] && f.descend[0xA1] && !f.descend[b'a' as usize]);
        let mut data = vec![0u8; 600]; data[100 + b'a' as usize] = 3; data[300 + b'g' as usize] = 8;
        let t = Font::from_tables(f.glyphs.clone(), &data, 100, 300, 8).unwrap();
        let mut w = [0u8; 256]; w[b'a' as usize] = 3; assert_eq!((t.widths, t.descend), (w, f.descend));
        assert!(Font::from_tables(vec![], &data, 100, 400, 8).is_none());
        assert_eq!(draw(&mut fb, Raster::Wrap, 0, &f, b"ag", 10, 10, 4, Some(2)), 16);
        assert_eq!((fb.get(0, 10, 10), fb.get(0, 12, 12), fb.get(0, 12, 13), fb.get(0, 14, 15)), (4, 2, 4, 2));
        assert_eq!(draw(&mut fb, Raster::Wrap, 0, &f, b"ca", 20, 10, 4, None), 15);
        assert_eq!((fb.get(0, 20, 10), fb.get(0, 20, 12)), (7, 7));
        assert_eq!(draw(&mut fb, Raster::Wrap, 0, &f, b"ca", 40, 10, 4, Some(1)), 15);
        assert_eq!((fb.get(0, 41, 11), fb.get(0, 40, 12)), (7, 4));
        let mut c = cursor();
        c.at(20, 10); c.text(&mut fb, &f, b"ca");
        assert_eq!((fb.get(1, 20, 10), fb.get(1, 20, 12)), (7, 7));
        c.shadow = true; c.bg = 1; c.at(30, 10); c.text(&mut fb, &f, b"ca");
        assert_eq!((fb.get(1, 31, 11), fb.get(1, 30, 12)), (7, 4));
        c.at(0, 0); c.text(&mut fb, &f, b"\ta\t|a");
        assert_eq!(c.x, 118); c.x = 70; c.glyph(&mut fb, &f, 9); assert_eq!(c.x, 70);
        let mut o = Cursor { shadow: false, ..c.clone() };
        o.counted_glyph(&mut fb, &f, 2, 1, 1, 50, 50, &[0, 5, 0, 0, 0], &[0, 7, 0, 0, 0]);
        assert_eq!((o.pen, o.colour, o.y, o.x), (0, 0, 51, 58));
        o.counted_glyph(&mut fb, &f, 2, 3, 1, 50, 50, &[0; 5], &[0; 5]); assert_eq!((o.y, o.x), (50, 50));
        o.wrapped(&mut fb, &f, b"aa aa aa", 100, 10, 8, 30, 5, 6, 9);
        assert_eq!((o.y, o.x), (110, 16)); assert_eq!(fb.get(1, 110, 10), 6);
        o.wrapped(&mut fb, &f, b"aa aa aa", 100, 10, 8, 14, 5, 6, 9);
        assert_eq!((o.y, o.x), (100, 16)); assert_eq!(fb.get(1, 105, 10), 9);
    }
    #[test]
    fn extremes() {
        let (f, mut fb) = (font(), Fb::new());
        assert!(Font::from_tables(vec![], &[0; 600], usize::MAX, 0, 8).is_none() && Font::from_tables(vec![], &[0; 600], 0, usize::MAX, 8).is_none());
        assert_eq!(draw(&mut fb, Raster::Clip, 0, &f, b"ga", i32::MAX, i32::MAX, 4, Some(1)), i32::MAX as i64 + 6);
        assert!(fb.planes.iter().all(|&b| b == 0));
        assert_eq!(draw(&mut fb, Raster::Wrap, 0, &f, b"a", 0, i32::MIN, 4, None), i32::MIN as i64 + 3);
        let mut g: Vec<Entry> = vec![vec![]; 256]; g[b'p' as usize] = vec![Rec { op: 4, colour: 16, y1: 0, x1: 0, y2: 0, x2: 0 }];
        let mut w1 = [0u8; 256]; w1[b'p' as usize] = 1; let fp = Font::new(g.clone(), w1, &[0; 256], 8);
        let mut fb = Fb::new();
        assert_eq!(draw(&mut fb, Raster::Wrap, 0, &fp, b"pp", 0, i32::MAX, 4, None), i32::MAX as i64 + 2);
        assert_eq!((fb.get(0, 204, 255), fb.get(0, 0, 0)), (4, 4));
        assert_eq!(glyph(&mut fb, Raster::Clip, 0, &fp, b'p', 0, i32::MAX, &mut Pen::new(4), 4, None), 2147483648);
        let mut c = cursor(); c.window_x = i64::from(i32::MAX); c.text(&mut fb, &f, b"|a"); assert_eq!(c.x, i32::MAX as i64 + 18);
        c.at(0, i64::MAX); c.glyph(&mut fb, &f, b'a'); assert_eq!(c.x, i64::MIN + 2);
        c.window_x = i64::MAX; c.text(&mut fb, &f, b"|"); assert_eq!(c.x, i64::MIN + 14);
        c.at(0, 0); c.wrapped(&mut fb, &fp, b"p", 0, 1, i64::MAX, 10, 1, 6, 9); assert_eq!(fb.get(1, 1, 1), 6);
        c.at(0, 0); c.counted_glyph(&mut fb, &f, 2, 1, 1, 50, 50, &[0], &[0]); assert_eq!((c.y, c.x, c.pen), (0, 0, 6));
        c.counted_glyph(&mut fb, &f, 2, 1, 1, 50, 50, &[0, 0], &[0]); assert_eq!((c.y, c.x, c.pen), (50, 50, 0));
        let mut g: Vec<Entry> = vec![vec![]; 256]; g[0] = vec![Rec { op: 4, colour: 16, y1: 0, x1: 0, y2: 0, x2: 0 }]; g[1] = vec![];
        let f0 = Font::new(g, [0; 256], &[0; 256], 8);
        c.counted_glyph(&mut fb, &f0, 2, 1, 80, 0, 0, &[0, 0], &[0, 0]); assert_eq!((fb.get(1, 1, 0), c.x), (0, 0));
        c.wrapped(&mut fb, &f, b"a", 0, 1, i64::from(i32::MAX), 10, 1, 6, 9); assert_eq!(fb.get(1, 0, 1), 6);
        c.wrapped(&mut fb, &f, b"a a a", i64::from(i32::MAX) - 1, i64::from(i32::MAX), 3, 10, i64::from(i32::MAX), 6, 9);
    }
}
