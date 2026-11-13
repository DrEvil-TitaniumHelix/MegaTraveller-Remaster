use super::{Rect, Surface};
use mt_engine::{data::{itm, ly, pic, Variant}, io::Files, present::{fb::Fb, ly::{layout, Pen}, prim::Raster, textlog::Run}};

#[derive(Default)]
pub struct Assets {
    pub terrain: Vec<Bitmap>, pub objects: Vec<Bitmap>, pub people: Vec<Bitmap>,
    ground: Vec<ly::Entry>, font: Vec<ly::Entry>, top: Vec<ly::Entry>,
    pub items: Vec<itm::Item>,
    panel_ds: Vec<u8>,
    panel_font: Option<mt_engine::present::text::Font>,
    pub places: super::place::Places,
}
#[derive(Clone, Debug, Default)]
pub struct Bitmap { pub w: usize, pub h: usize, pub pixels: Vec<u8> }
impl Bitmap {
    fn from_pic(p: pic::Pic) -> Self {
        Self { w: mt_engine::present::pic::width(&p), h: p.h as usize, pixels: mt_engine::present::pic::unpack(&p) }
    }
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h { return None }
        self.pixels.get((y as usize).checked_mul(self.w)?.checked_add(x as usize)?).copied()
    }
    pub fn draw(&self, out: &mut Surface, from: Rect, at: (i32, i32), transparent: Option<u8>) {
        let x0 = (at.0 as i64).max(0); let y0 = (at.1 as i64).max(0);
        let x1 = (at.0 as i64 + from.w as i64).min(out.w as i64);
        let y1 = (at.1 as i64 + from.h as i64).min(out.h as i64);
        for y in y0..y1 { for x in x0..x1 {
            let (sx, sy) = (from.x as i64 + x - at.0 as i64, from.y as i64 + y - at.1 as i64);
            if let (Ok(sx), Ok(sy)) = (i32::try_from(sx), i32::try_from(sy)) {
                if let Some(c) = self.get(sx, sy) { if Some(c) != transparent { out.put(x as i32, y as i32, c); } }
            }
        } }
    }
}
impl Assets {
    pub fn load(files: &mut dyn Files) -> Self {
        let sheets = |name| files.read(name).and_then(|b| pic::decode_set(Variant::Mt1, &b).ok()).unwrap_or_default().into_iter().map(Bitmap::from_pic).collect();
        let (terrain, objects, people) = (sheets("X.DAT"), sheets("OBJ.DAT"), sheets("C.DAT"));
        let entries = |name| files.read(name).and_then(|b| ly::decode(&b).ok()).unwrap_or_default();
        let (ground, font, top) = (entries("GROUND.LY"), entries("FONT.LY"), entries("TOP.LY"));
        let items = files.read("TRAV.ITM").and_then(|b| itm::decode(Variant::Mt1, &b).ok()).unwrap_or_default();
        let panel_ds = files.read("T.DAT").and_then(|b| mt1::ground::state::ds_image(&b)).unwrap_or_default();
        let panel_font = mt_engine::present::text::Font::from_tables(font.clone(), &panel_ds, 0x5128, 0x5637, 8);
        let places = files.read("W.DAT").map(|w| super::place::Places::decode(&w)).unwrap_or_default();
        Self { terrain, objects, people, ground, font, top, items, panel_ds, panel_font, places }
    }
    pub fn ds_u8(&self, off: usize) -> Option<u8> { self.panel_ds.get(off).copied() }
    pub fn ds_string(&self, table: usize, i: usize) -> Option<String> {
        let at = table.checked_add(i.checked_mul(4)?)?;
        let word = |at: usize| Some(u16::from_le_bytes([*self.panel_ds.get(at)?, *self.panel_ds.get(at + 1)?]));
        if word(at + 2)? != mt1::ground::state::Mem::DS { return None; }
        let off = word(at)? as usize;
        let bytes: Vec<u8> = self.panel_ds.get(off..)?.iter().copied().take_while(|&b| b != 0).take(64).collect();
        (!bytes.is_empty() && bytes.iter().all(|b| (0x20..0x7f).contains(b))).then(|| bytes.iter().map(|&b| b as char).collect())
    }
    pub fn cpic(&self, scene: &mt1::ui::Scene, slot: u8) -> Option<&Bitmap> {
        let index = if slot < 2 { slot as usize } else { *scene.art.slots.get(slot as usize)? as usize + 4 };
        self.people.get(index)
    }
    fn panel_art(entries: &[ly::Entry], glyphs: &[(usize, i32, i32)], size: (u32, u32)) -> Surface {
        let mut fb = Fb::new(); let mut pen = Pen::new(0);
        for &(glyph, x, y) in glyphs { layout(&mut fb, Raster::Clip, 0, entries, glyph, y, x, &mut pen); }
        let mut out = Surface::new(size.0, size.1);
        for y in 0..out.h as i32 { for x in 0..out.w as i32 { out.put(x, y, fb.get(0, y, x)); } }
        out
    }
    pub fn walker_background(&self) -> Surface { Self::panel_art(&self.ground, &[(3, 1, 0), (5, 3, 8)], (128, 87)) }
    pub fn party_box(&self, scene: &mt1::ui::Scene) -> Surface {
        let mut glyphs = vec![(9, -256, 0)];
        let mut counters = Vec::new();
        for (slot, who) in scene.party.order().into_iter().enumerate() {
            if who >= 5 || scene.party.mask() & (1 << who) == 0 || !scene.party.alive(who) { continue; }
            let word = |at| self.panel_ds.get(at..at + 2).map(|b| i16::from_le_bytes([b[0], b[1]]) as i32);
            let (x, y) = word(0x29D + slot * 4).zip(word(0x29B + slot * 4)).map(|(x, y)| (x - 255, y + 1))
                .unwrap_or([(14, 4), (2, 18), (29, 18), (2, 32), (29, 32)][slot]);
            glyphs.push((10 + who as usize, x, y));
            let byte = |at| scene.party.0.get(at).copied().unwrap_or(0);
            counters.push((who, byte(0x122 + who as usize * 256), byte(0x123 + who as usize * 256), x + 1, y + 1));
        }
        let mut out = Self::panel_art(&self.top, &glyphs, (64, 54));
        let mut fb = Fb::new();
        for y in 0..54 { for x in 0..64 { fb.pixel(Raster::Clip, 0, y, x, out.get(x, y).unwrap_or(0), 0); } }
        for (who, glyph, count, x, y) in counters {
            let offset = |at| self.panel_ds.get(at).copied().unwrap_or(0) as i32;
            self.party_glyph(&mut fb, glyph, y, x + offset(0x522C + glyph as usize), 0);
            if glyph != 3 && count > 0 { if let Some(n) = count.checked_add(0xB0) {
                self.party_glyph(&mut fb, n, y + 1, x + offset(0x5231 + glyph as usize) + i32::from(count == 1), who + 1);
            } }
        }
        for y in 0..54 { for x in 0..64 { out.put(x, y, fb.get(0, y, x)); } }
        out.text = mt_engine::present::textlog::frame(&fb, 0);
        out
    }
    fn party_glyph(&self, fb: &mut Fb, c: u8, y: i32, x: i32, colour: u8) {
        if let Some(font) = &self.panel_font {
            let y = y - if font.descend[c as usize] { 2 } else { 0 };
            mt_engine::present::text::glyph(fb, Raster::Clip, 0, font, c, y, x, &mut Pen::new(colour), colour, None);
        } else { layout(fb, Raster::Clip, 0, &self.font, c as usize, y, x, &mut Pen::new(colour)); }
    }
    pub fn item_icon(&self, out: &mut Surface, icon: u8, at: (i32, i32)) {
        let (index, sheet, columns, width) = if icon >= 100 { (icon as i32 - 100, 1, 6, 48) } else { (icon as i32, 0, 12, 24) };
        if let Some(b) = self.objects.get(sheet) { b.draw(out, Rect::new(index % columns * width, index / columns * 15, width as u32, 15), at, None); }
    }
    pub fn ground(&self, out: &mut Surface, glyph: usize, at: (i32, i32), pen: &mut u8) {
        if glyph >= self.ground.len() { return }
        let mut fb = Fb::new();
        for y in 0..64 { for x in 0..64 { fb.pixel(Raster::Clip, 0, y, x, out.get(at.0.saturating_add(x - 16), at.1.saturating_add(y - 16)).unwrap_or(0), 0); } }
        let mut p = Pen::new(*pen); layout(&mut fb, Raster::Clip, 0, &self.ground, glyph, 16, 16, &mut p); *pen = p.colour;
        for y in 0..64 { for x in 0..64 { out.put(at.0.saturating_add(x - 16), at.1.saturating_add(y - 16), fb.get(0, y, x)); } }
    }
    pub fn text(&self, out: &mut Surface, text: &str, at: (i32, i32), colour: u8) {
        let mut x = at.0;
        for c in text.bytes().take(out.w.saturating_add(6) / 6) {
            if x as i64 >= out.w as i64 { break }
            let mut fb = Fb::new();
            layout(&mut fb, Raster::Clip, 0, &self.font, c as usize, 0, 0, &mut Pen::new(if colour == 0 { 15 } else { colour }));
            let mut run = Run { page: out.text.page, x: x as i64, y: at.1 as i64, colour: label_colour(colour), shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false,
                covered: false, background_width: 6, background_height: 8, background: Vec::with_capacity(48), bitmap: Vec::with_capacity(48) };
            for y in 0..8 { for dx in 0..6 {
                let bg = out.get(x.saturating_add(dx), at.1.saturating_add(y)).unwrap_or(0);
                let ink = fb.get(0, y, dx); let pixel = if ink == 0 { bg } else if colour == 0 { 0 } else { ink };
                run.background.push(bg); run.bitmap.push(if ink != 0 && pixel == bg { pixel | 0x10 } else { pixel }); out.put(x.saturating_add(dx), at.1.saturating_add(y), pixel);
            } }
            out.text.visible.push(run.clone()); out.text.runs.push(run); x = x.saturating_add(6);
        }
    }
    pub fn text_hot(&self, out: &mut Surface, text: &str, at: (i32, i32), colour: u8, hot: Option<usize>) {
        let before = out.text.runs.len();
        self.text(out, text, at, colour);
        if let Some(i) = hot { Self::flag_hot(out, before, i); }
    }
    pub fn text_fit_hot(&self, out: &mut Surface, text: &str, rect: Rect, colour: u8, hot: Option<usize>) {
        let before = out.text.runs.len();
        self.text_fit(out, text, rect, colour);
        if let Some(i) = hot { Self::flag_hot(out, before, i); }
    }
    fn flag_hot(out: &mut Surface, before: usize, i: usize) {
        let Some(run) = out.text.runs.get_mut(before + i) else { return };
        run.colour |= crate::pipeline::text::UNDERLINE; let (x, y) = (run.x, run.y);
        if let Some(v) = out.text.visible.iter_mut().rev().find(|r| r.x == x && r.y == y) { v.colour |= crate::pipeline::text::UNDERLINE; }
    }
    pub fn text_fit(&self, out: &mut Surface, text: &str, rect: Rect, colour: u8) {
        let left = rect.x.max(0); let top = rect.y.max(0);
        let right = (i64::from(rect.x) + i64::from(rect.w)).min(out.w as i64);
        let bottom = (i64::from(rect.y) + i64::from(rect.h)).min(out.h as i64);
        if right <= left as i64 || bottom <= top as i64 { return; }
        let (width, height) = ((right - left as i64) as usize, (bottom - top as i64).min(8) as usize);
        let advance = |c: u8| self.panel_ds.get(0x5128 + c as usize).copied().filter(|&w| w > 0).unwrap_or(6).min(16) as usize;
        let mut bytes: Vec<_> = text.bytes().take(1024).collect();
        let limit = width / 3;
        if bytes.len() > limit { bytes.truncate(limit.saturating_sub(3)); bytes.extend(std::iter::repeat_n(b'.', limit.min(3))); }
        let measured: usize = bytes.iter().map(|&c| advance(c)).sum();
        if measured == 0 { return; }
        let fitted = measured.min(width); let mut pen_x = 0;
        for c in bytes {
            let native = advance(c);
            let x0 = pen_x * fitted / measured; pen_x += native;
            let w = pen_x * fitted / measured - x0;
            if w == 0 { continue; }
            let x = left + x0 as i32;
            let mut fb = Fb::new();
            layout(&mut fb, Raster::Clip, 0, &self.font, c as usize, 0, 0, &mut Pen::new(15));
            let mut run = Run { page: out.text.page, x: x as i64, y: top as i64, colour: label_colour(colour), shadow: None, bytes: vec![c], font_id: 0,
                width: w as u8, descend: false, covered: false, background_width: w as u16, background_height: height as u8,
                background: Vec::with_capacity(w * height), bitmap: Vec::with_capacity(w * height) };
            for y in 0..height { for dx in 0..w {
                let bg = out.get(x + dx as i32, top + y as i32).unwrap_or(0);
                let ink = fb.get(0, (y * 8 / height) as i32, (dx * native / w) as i32) != 0;
                let pixel = if ink { colour } else { bg };
                run.background.push(bg); run.bitmap.push(if ink && pixel == bg { pixel | 0x10 } else { pixel }); out.put(x + dx as i32, top + y as i32, pixel);
            } }
            out.text.visible.push(run.clone()); out.text.runs.push(run);
        }
    }
}
fn label_colour(colour: u8) -> u8 { if colour == 8 { colour | crate::pipeline::text::DIMMED } else { colour } }

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    #[test]
    fn bitmap_clips_both_sides_and_transparency() {
        let b = Bitmap { w: 2, h: 2, pixels: vec![1, 2, 3, 4] }; let mut s = Surface::new(3, 3);
        b.draw(&mut s, Rect::new(0, 0, 2, 2), (-1, 1), Some(1)); assert_eq!(s.get(0, 1), Some(2)); assert_eq!(s.get(0, 2), Some(4));
        b.draw(&mut s, Rect::new(i32::MAX, i32::MAX, u32::MAX, u32::MAX), (i32::MIN, i32::MIN), None);
        assert_eq!(s.get(0, 0), Some(0));
    }
    #[test]
    fn ds_strings_name_skills_careers_and_titles() {
        assert_eq!(Assets::default().ds_string(0x1479, 66), None, "no game files: no names, never a panic");
        assert_eq!(Assets::default().ds_u8(0x13F6), None);
        let Some(dir) = oracle_env::gamefiles() else { return };
        let a = Assets::load(&mut mt_engine::io::Dir(std::path::PathBuf::from(dir)));
        assert_eq!(a.ds_string(0x1479, mt1::ground::rules::adapters::SKILL_TACTICS as usize).as_deref(), Some("tactics"), "the tables are lower case");
        assert_eq!(a.ds_string(0x1479, mt1::ground::rules::adapters::SKILL_LEADER as usize).as_deref(), Some("leader"));
        let skills: Vec<String> = (0..80).filter_map(|i| a.ds_string(0x1479, i)).collect();
        assert!(skills.len() >= 60 && skills.iter().all(|s| !s.is_empty()), "{skills:?}");
        let careers: Vec<String> = (0..8).filter_map(|i| a.ds_string(0x1465, i)).collect();
        assert!(careers.iter().any(|c| c == "navy") && careers.iter().any(|c| c.starts_with("marine")), "{careers:?}");
        let title = a.ds_u8(0x13F6).and_then(|t| a.ds_string(0x1419, t as usize));
        assert!(title.is_some_and(|t| !t.is_empty()), "the first rank title of career 0");
        assert_eq!(a.ds_string(0x1479, 10_000), None); assert_eq!(a.ds_string(usize::MAX, 1), None);
    }
}
