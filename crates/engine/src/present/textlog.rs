use std::{cell::{Cell, RefCell}, collections::HashMap};
use super::{fb::Fb, palette::DEFAULT_REGS, prim::Raster, text::Font};
use crate::data::ly::Entry;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub page: usize, pub x: i64, pub y: i64,
    pub colour: u8, pub shadow: Option<u8>, pub bytes: Vec<u8>, pub font_id: u64,
    pub width: u8, pub descend: bool,
    pub background_width: u16, pub background_height: u8,
    pub background: Vec<u8>,
    pub bitmap: Vec<u8>,
    pub covered: bool,
}
impl Run {
    pub fn row(&self) -> i64 { self.y.saturating_add(if self.descend { 2 } else { 0 }) }
    pub fn erase_width(&self) -> usize { self.background_width as usize }
    pub fn erase_height(&self) -> usize { self.background_height as usize }
    fn right(&self) -> i64 { self.x.saturating_add(self.erase_width() as i64) }
    fn bottom(&self) -> i64 { self.y.saturating_add(self.erase_height() as i64) }
    fn same_cell(&self, other: &Self) -> bool {
        (self.page, self.x, self.y) == (other.page, other.x, other.y)
    }
    fn onscreen(&self) -> bool {
        self.width != 0 && self.x < 320 && self.right() > 0 && self.y < 200 && self.bottom() > 0
    }
    fn pixels(&self, fb: &Fb) -> Vec<u8> {
        if !self.onscreen() { return Vec::new(); }
        (0..self.erase_height()).flat_map(|dy| (0..self.erase_width()).map(move |dx| {
            let (x, y) = (self.x.saturating_add(dx as i64), self.y.saturating_add(dy as i64));
            if (0..320).contains(&x) && (0..200).contains(&y) { fb.get(self.page, y as i32, x as i32) } else { 0 }
        })).collect()
    }
    fn refresh(&self, now: Vec<u8>) -> bool {
        self.onscreen() && (now == self.bitmap || (self.covered && self.ink_intact(&now)))
    }
    fn ink_intact(&self, now: &[u8]) -> bool {
        if now.len() != self.bitmap.len() || self.bitmap.len() != self.background.len() { return false; }
        if self.bytes.first() == Some(&b' ') && self.bitmap == self.background { return now.iter().zip(&self.bitmap).filter(|(n, b)| n == b).count() * 2 > now.len(); }
        let (mut ink, mut intact, mut other, mut flooded) = (0usize, 0usize, 0usize, 0usize);
        for ((b, bg), n) in self.bitmap.iter().zip(&self.background).zip(now) {
            if b != bg { ink += 1; if n == b { intact += 1; } } else { other += 1; if n != b && *n == self.colour { flooded += 1; } }
        }
        ink > 0 && intact * 2 >= ink && flooded * 2 < other.max(1)
    }

}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextLog {
    pub runs: Vec<Run>,
    pub visible: Vec<Run>,
    pub overflowed: bool,
    pub page: usize, pub regs: [u8; 16],
}
impl Default for TextLog {
    fn default() -> Self { Self { runs: Vec::new(), visible: Vec::new(), overflowed: false, page: 0, regs: DEFAULT_REGS } }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SurfaceImage {
    pub pending: Vec<Run>, pub cells: Vec<Run>, pub overflowed: bool,
    pub events: usize, pub frame_bytes: usize, pub last: Option<TextLog>,
}
pub fn capture(fb: &Fb) -> Option<SurfaceImage> {
    if !enabled() { return None; }
    let mut slot=fb.textlog.borrow_mut();let surface=slot.get_or_insert_with(Surface::default);surface.current();
    let last=STATE.with(|s|s.borrow().ready.as_ref().map(|(_,log)|log.clone()));
    let mut image=SurfaceImage {pending:surface.pending.clone(),cells:surface.cells.clone(),overflowed:surface.overflowed,events:surface.events,frame_bytes:surface.frame_bytes,last};
    for r in image.pending.iter_mut().chain(image.cells.iter_mut()) {r.font_id=0;}
    if let Some(log)=&mut image.last {for r in log.runs.iter_mut().chain(log.visible.iter_mut()) {r.font_id=0;}}
    Some(image)
}
pub fn restore(fb: &Fb, image: &Option<SurfaceImage>, frame: &[u32]) -> Result<(),String> {
    let Some(image)=image else {clear(fb);return Ok(())};
    let valid=|r:&Run| r.page<16 && r.background.len()==r.erase_width()*r.erase_height() && r.bitmap.len()==r.background.len() && r.erase_width()<=320 && r.erase_height()<=200;
    if !image.pending.iter().chain(&image.cells).all(valid) || image.last.as_ref().is_some_and(|l|!l.runs.iter().chain(&l.visible).all(valid)) {return Err("play: invalid text surface".into())}
    if !enabled() {return Ok(())}
    *fb.textlog.borrow_mut()=Some(Surface {pending:image.pending.clone(),cells:image.cells.clone(),generation:GENERATION.with(Cell::get),
        pending_bytes:image.pending.iter().map(run_bytes).sum(),cell_bytes:image.cells.iter().map(run_bytes).sum(),overflowed:image.overflowed,events:image.events,frame_bytes:image.frame_bytes});
    STATE.with(|s|s.borrow_mut().ready=image.last.clone().map(|log|(frame.to_vec(),log)));
    Ok(())
}
const MAX_EVENTS: usize = 4096;
const MAX_LOG_BYTES: usize = 4 * 1024 * 1024;
fn run_bytes(run: &Run) -> usize { std::mem::size_of::<Run>() + run.bytes.len() + run.background.len() + run.bitmap.len() }
#[derive(Default)]
pub(super) struct Surface {
    pending: Vec<Run>, cells: Vec<Run>, generation: u64,
    pending_bytes: usize, cell_bytes: usize, overflowed: bool,
    events: usize, frame_bytes: usize,
}
impl Surface {
    fn current(&mut self) {
        let generation = GENERATION.with(Cell::get);
        if self.generation != generation { *self = Self { generation, ..Self::default() }; }
    }
    fn overflow(&mut self) {
        count(2); self.pending.clear(); self.cells.clear();
        self.pending_bytes = 0; self.cell_bytes = 0; self.overflowed = true;
    }
    fn recount(&mut self) { self.cell_bytes = self.cells.iter().map(run_bytes).sum(); }
    fn retire(&mut self, mut touched: impl FnMut(&Run) -> bool) {
        self.cells.retain(|r| { if !touched(r) { return true; } self.cell_bytes -= run_bytes(r); false });
        self.pending.retain(|r| { if !touched(r) { return true; } self.pending_bytes -= run_bytes(r); false });
    }
    fn trim_bottom_padding(&mut self, page: usize, left: i64, top: i64, right: i64, bottom: i64) {
        let mut changed = false;
        for r in self.cells.iter_mut().chain(&mut self.pending) {
            if r.page != page || r.x >= right || r.right() <= left || r.y >= bottom || r.bottom() <= top || top <= r.row() { continue; }
            let keep = (top - r.y) as usize * r.erase_width();
            if r.bitmap.len() != r.background.len() || keep >= r.bitmap.len()
                || (top < r.row() + 8 && r.bitmap[keep..].iter().zip(&r.background[keep..]).any(|(a, b)| a != b))
                || !r.bitmap[..keep].iter().zip(&r.background[..keep]).any(|(a, b)| a != b) { continue; }
            r.background_height = (top - r.y) as u8;
            r.background.truncate(keep); r.bitmap.truncate(keep); changed = true;
        }
        if changed { self.recount(); self.pending_bytes = self.pending.iter().map(run_bytes).sum(); }
    }
    fn refresh_cell(&mut self, fb: &Fb, target: &Run) {
        let Some(first) = self.cells.iter().position(|r| r.same_cell(target)) else { return; };
        let mut bounds = (target.x.max(0), target.y.max(0), target.right().min(320), target.bottom().min(200));
        let mut selected = Vec::new();
        for (i, r) in self.cells.iter().enumerate().skip(first) {
            if r.page != target.page || r.x >= bounds.2 || r.right() <= bounds.0 || r.y >= bounds.3 || r.bottom() <= bounds.1 { continue; }
            bounds = (bounds.0.min(r.x.max(0)), bounds.1.min(r.y.max(0)), bounds.2.max(r.right().min(320)), bounds.3.max(r.bottom().min(200)));
            selected.push(i);
        }
        let (left, top, right, bottom) = bounds;
        let stride = (right - left) as usize;
        let mut pixels: Vec<_> = (top..bottom).flat_map(|y| (left..right).map(move |x| fb.get(target.page, y as i32, x as i32))).collect();
        let mut removed = false;
        for &i in selected.iter().rev() {
            let r = &mut self.cells[i];
            let mut now = vec![0; r.erase_width() * r.erase_height()];
            for dy in 0..r.erase_height() { for dx in 0..r.erase_width() {
                let (x, y) = (r.x + dx as i64, r.y + dy as i64);
                if (left..right).contains(&x) && (top..bottom).contains(&y) { now[dy * r.erase_width() + dx] = pixels[(y - top) as usize * stride + (x - left) as usize]; }
            } }
            if !r.refresh(now) { r.width = 0; removed = true; continue; }
            for dy in 0..r.erase_height() { for dx in 0..r.erase_width() {
                let (x, y) = (r.x + dx as i64, r.y + dy as i64);
                if (left..right).contains(&x) && (top..bottom).contains(&y) { pixels[(y - top) as usize * stride + (x - left) as usize] = r.background[dy * r.erase_width() + dx]; }
            } }
        }
        if removed { self.cells.retain(|r| r.width != 0); self.recount(); }
    }
    fn refresh(&mut self, fb: &Fb) {
        let mut pages = HashMap::new();
        let mut keep = vec![false; self.cells.len()];
        for (run, keep) in self.cells.iter_mut().zip(&mut keep).rev() {
            let page = pages.entry(run.page).or_insert_with(|| fb.indexed(run.page));
            let (left, top, width) = (run.x, run.y, run.erase_width());
            let now = (0..run.erase_height()).flat_map(|dy| (0..width).map(move |dx| (left + dx as i64, top + dy as i64)))
                .map(|(x, y)| if (0..320).contains(&x) && (0..200).contains(&y) { page[y as usize * 320 + x as usize] } else { 0 }).collect();
            if !run.refresh(now) { continue; }
            *keep = true;
            for dy in 0..run.erase_height() { for dx in 0..run.erase_width() {
                let (x, y) = (run.x + dx as i64, run.y + dy as i64);
                if (0..320).contains(&x) && (0..200).contains(&y) { page[y as usize * 320 + x as usize] = run.background[dy * run.erase_width() + dx]; }
            } }
        }
        let mut keep = keep.into_iter(); self.cells.retain(|_| keep.next().unwrap_or(false));
        self.pending.retain(|r| !r.covered || r.refresh(r.pixels(fb)));
        self.pending_bytes = self.pending.iter().map(run_bytes).sum();
        self.recount();
    }
    fn replace_background(&mut self, target: &mut Run) {
        let Some(i) = self.cells.iter().position(|r| r.same_cell(target)) else { return; };
        let old = self.cells.remove(i);
        self.cell_bytes -= run_bytes(&old);
        let mut clean = old.background.clone();
        for r in self.cells.iter_mut().skip(i) {
            if r.page != old.page { continue; }
            for y in r.y.max(old.y)..r.bottom().min(old.bottom()) {
                for x in r.x.max(old.x)..r.right().min(old.right()) {
                    let at = (y - r.y) as usize * r.erase_width() + (x - r.x) as usize;
                    let under = (y - old.y) as usize * old.erase_width() + (x - old.x) as usize;
                    let ink = r.bitmap[at] != r.background[at];
                    r.background[at] = clean[under];
                    if ink { clean[under] = r.bitmap[at]; } else { r.bitmap[at] = clean[under]; }
                }
            }
        }
        for y in 0..old.erase_height().min(target.erase_height()) { for x in 0..old.erase_width().min(target.erase_width()) {
            let at = y * target.erase_width() + x;
            target.background[at] = clean[y * old.erase_width() + x];
        } }
    }
}
#[derive(Clone)]
struct Face { id: u64, widths: [u8; 256], descend: [bool; 256] }
#[derive(Default)]
struct State { fonts: HashMap<usize, Face>, next_id: u64, ready: Option<(Vec<u32>, TextLog)> }
thread_local! {
    static ENABLED: Cell<bool> = const { Cell::new(false) };
    static IN_GLYPH: Cell<bool> = const { Cell::new(false) };
    static GENERATION: Cell<u64> = const { Cell::new(0) };
    static STATE: RefCell<State> = RefCell::new(State::default());
    static DIAG: Cell<[u32; 3]> = const { Cell::new([0; 3]) };
}
pub fn diagnostics() -> [u32; 3] { DIAG.with(|d| d.replace([0; 3])) }
fn count(i: usize) { DIAG.with(|d| { let mut v = d.get(); v[i] = v[i].saturating_add(1); d.set(v); }); }
#[inline]
pub fn enabled() -> bool { ENABLED.with(Cell::get) }
#[inline]
pub(super) fn writes_enabled() -> bool { enabled() && !IN_GLYPH.with(Cell::get) }
pub fn set_enabled(on: bool) {
    if ENABLED.with(|v| v.replace(on)) != on {
        GENERATION.with(|v| v.set(v.get().wrapping_add(1)));
        STATE.with(|s| s.borrow_mut().ready = None);
    }
}
pub fn clear(fb: &Fb) {
    if !enabled() { return; }
    *fb.textlog.borrow_mut() = None;
    STATE.with(|s| s.borrow_mut().ready = None);
}
pub(super) fn written(fb: &Fb, raster: Raster, page: usize, y: i64, x: i64, h: i64, w: i64) {
    if !writes_enabled() || h <= 0 || w <= 0 { return; }
    let mut state = fb.textlog.borrow_mut();
    let Some(surface) = state.as_mut() else { return; };
    surface.current();
    let (right, bottom) = (x.saturating_add(w), y.saturating_add(h));
    if raster == Raster::Wrap && (x < 0 || y < 0 || right > 320 || bottom > 200) {
        count(1); surface.retire(|_| true); return;
    }
    let (left, top, right, bottom) = (x.max(0), y.max(0), right.min(320), bottom.min(200));
    if left >= right || top >= bottom { return; }
    surface.trim_bottom_padding(page & 7, left, top, right, bottom);
    surface.retire(|r| r.page == page & 7 && r.x < right && r.right() > left && r.y < bottom && r.bottom() > top);
}
pub(super) fn decorated(fb: &Fb, raster: Raster, page: usize, y: i64, x: i64, h: i64, w: i64) {
    if !writes_enabled() || h <= 0 || w <= 0 { return; }
    let (right, bottom) = (x.saturating_add(w), y.saturating_add(h));
    if raster == Raster::Wrap && (x < 0 || y < 0 || right > 320 || bottom > 200) { written(fb, raster, page, y, x, h, w); return; }
    let mut state = fb.textlog.borrow_mut();
    let Some(surface) = state.as_mut() else { return; };
    surface.current();
    let (left, top, right, bottom) = (x.max(0), y.max(0), right.min(320), bottom.min(200));
    if left >= right || top >= bottom { return; }
    let touched = |r: &Run| r.page == page & 7 && r.x < right && r.right() > left && r.y < bottom && r.bottom() > top;
    for r in surface.cells.iter_mut().chain(&mut surface.pending) { if touched(r) { r.covered = true; } }
    surface.retire(|r| touched(r) && r.bitmap == r.background && r.bytes.first() != Some(&b' '));
}
pub(super) fn xor(fb: &Fb, page: usize, y: i64, x: i64, h: i64, w: i64, colour: u8, dither: bool) {
    if !enabled() || IN_GLYPH.with(Cell::get) || h <= 0 || w <= 0 { return; }
    let mut state = fb.textlog.borrow_mut();
    let Some(surface) = state.as_mut() else { return; };
    surface.current();
    let (right, bottom) = (x.saturating_add(w), y.saturating_add(h));
    for r in surface.cells.iter_mut().chain(&mut surface.pending) {
        if r.page != page & 7 || r.x >= right || r.right() <= x || r.y >= bottom || r.bottom() <= y || r.background.is_empty() { continue; }
        let mut delta = None;
        let mut uniform = true;
        for dy in 0..r.erase_height() { for dx in 0..r.erase_width() {
            let (px, py) = (r.x + dx as i64, r.y + dy as i64);
            let inside = (x..right).contains(&px) && (y..bottom).contains(&py) && (0..320).contains(&px) && (0..200).contains(&py);
            let mask = if !inside { 0 } else if dither && colour >= 16 && (px + py) & 1 != 0 { colour >> 4 } else { colour & 15 };
            let at = dy * r.erase_width() + dx;
            if r.bitmap[at] != r.background[at] {
                if let Some(d) = delta { uniform &= d == mask; } else { delta = Some(mask); }
            }
            r.bitmap[at] ^= mask; r.background[at] ^= mask;
        } }
        if uniform {
            let delta = delta.unwrap_or(if (y..bottom).contains(&r.row()) { colour & 15 } else { 0 });
            r.colour ^= delta; r.shadow = r.shadow.map(|c| c ^ delta);
        } else {
            r.width = 0;
        }
    }
    surface.retire(|r| r.width == 0);
}
pub(super) fn written_bytes(fb: &Fb, address: usize, width: usize) {
    if !enabled() || IN_GLYPH.with(Cell::get) || width == 0 { return; }
    let mut state = fb.textlog.borrow_mut();
    let Some(surface) = state.as_mut() else { return; };
    surface.current();
    let start = address & 0xffff;
    let end = start + width.min(65536);
    surface.retire(|r| r.onscreen() && (r.y.max(0)..r.bottom().min(200)).any(|y| {
        let left = r.page * super::fb::PAGE + y as usize * 40 + r.x.max(0) as usize / 8;
        let right = r.page * super::fb::PAGE + y as usize * 40 + (r.right().min(320) as usize + 7) / 8;
        (left < end.min(65536) && right > start) || (end > 65536 && left < end - 65536)
    }));
}
pub(super) struct Suppress(bool);
impl Drop for Suppress { fn drop(&mut self) { IN_GLYPH.with(|v| v.set(self.0)); } }
pub(super) fn suppress() -> Option<Suppress> { enabled().then(|| Suppress(IN_GLYPH.with(|v| v.replace(true)))) }
pub(super) fn register(font: &Font) {
    if !enabled() { return; }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let key = font.glyphs.as_ptr() as usize;
        let id = s.fonts.get(&key).map(|f| f.id).unwrap_or_else(|| { s.next_id += 1; s.next_id });
        s.fonts.insert(key, Face { id, widths: font.widths, descend: font.descend });
    });
}
pub(super) fn forget(font: &Font) {
    if GENERATION.with(Cell::get) != 0 { STATE.with(|s| { s.borrow_mut().fonts.remove(&(font.glyphs.as_ptr() as usize)); }); }
}
pub(super) struct Capture { run: Run }
impl Drop for Capture { fn drop(&mut self) { IN_GLYPH.with(|v| v.set(false)); } }

pub(super) fn begin(fb: &Fb, font: &Font, page: usize, c: usize, y: i64, x: i64, colour: u8, shadow: Option<u8>) -> Option<Capture> {
    if !enabled() { return None; }
    register(font);
    let face = STATE.with(|s| s.borrow().fonts[&(font.glyphs.as_ptr() as usize)].clone());
    start(fb, face, page, c, y, x, colour, shadow)
}
pub(super) fn layout(fb: &Fb, entries: &[Entry], page: usize, c: usize, y: i32, x: i32, colour: u8) -> Option<Capture> {
    if !enabled() || IN_GLYPH.with(Cell::get) { return None; }
    let face = STATE.with(|s| s.borrow().fonts.get(&(entries.as_ptr() as usize)).cloned())?;
    let y = y as i64 - if *face.descend.get(c)? { 2 } else { 0 };
    start(fb, face, page, c, y, x as i64, colour, None)
}
fn start(fb: &Fb, face: Face, page: usize, c: usize, y: i64, x: i64, colour: u8, shadow: Option<u8>) -> Option<Capture> {
    let mut state = fb.textlog.borrow_mut();
    let surface = state.get_or_insert_with(Surface::default);
    surface.current();
    if surface.overflowed { return None; }
    let mut run = Run { page: page & 7, x, y, colour: colour & 15, shadow: shadow.map(|c| c & 15),
        bytes: vec![u8::try_from(c).ok()?], font_id: face.id, width: *face.widths.get(c)?, descend: face.descend[c],
        background_width: face.widths[c] as u16 + u16::from(shadow.is_some()),
        background_height: (if (0xD0..=0xD9).contains(&c) { 12 } else { 8 })
            + if face.descend[c] { 2 } else { 0 } + u8::from(shadow.is_some()),
        background: Vec::new(), bitmap: Vec::new(), covered: false };
    if let Some(old) = surface.cells.iter().find(|old| old.same_cell(&run)) {
        run.background_width = run.background_width.max(old.background_width);
        run.background_height = run.background_height.max(old.background_height);
    }
    if (0..200).contains(&run.y) { run.background_height = run.background_height.min((200 - run.y) as u8); }
    let bytes = std::mem::size_of::<Run>() + 1 + if run.onscreen() { run.erase_width() * run.erase_height() * 2 } else { 0 };
    if surface.events >= MAX_EVENTS || surface.cells.len() >= MAX_EVENTS
        || surface.pending_bytes + surface.cell_bytes + bytes * 2 > MAX_LOG_BYTES
        || surface.frame_bytes + bytes * 2 > MAX_LOG_BYTES {
        surface.overflow(); return None;
    }
    run.background = run.pixels(fb);
    surface.refresh_cell(fb, &run);
    let icon = (c as u8) < 0x20;
    if run.onscreen() && !icon { surface.replace_background(&mut run); }
    IN_GLYPH.with(|v| v.set(true));
    Some(Capture { run })
}
pub(super) fn finish(fb: &Fb, capture: Option<Capture>, colour: u8) {
    let Some(mut capture) = capture else { return; };
    capture.run.bitmap = capture.run.pixels(fb);
    capture.run.colour = colour & 15;
    let mut state = fb.textlog.borrow_mut();
    let surface = state.get_or_insert_with(Surface::default);
    let icon = capture.run.bytes.first().is_some_and(|&b| b < 0x20);
    for old in surface.cells.iter_mut().chain(&mut surface.pending) { if icon && old.same_cell(&capture.run) && old.bytes.first().is_some_and(|&b| b >= 0x20) { old.covered = true; } }
    surface.retire(|old| old.same_cell(&capture.run) && !(icon && old.covered && old.bytes.first().is_some_and(|&b| b >= 0x20)));
    surface.events += 1;
    surface.frame_bytes += run_bytes(&capture.run) * 2;
    surface.pending_bytes += run_bytes(&capture.run);
    surface.pending.push(capture.run.clone());
    if capture.run.onscreen() { surface.cell_bytes += run_bytes(&capture.run); surface.cells.push(capture.run.clone()); }
}
pub fn frame(fb: &Fb, page: usize) -> TextLog {
    if !enabled() { return TextLog::default(); }
    let mut state = fb.textlog.borrow_mut();
    let surface = state.get_or_insert_with(Surface::default);
    surface.current();
    surface.events = 0; surface.frame_bytes = 0;
    if surface.overflowed {
        surface.overflowed = false;
        return TextLog { overflowed: true, page, regs: fb.regs, ..TextLog::default() };
    }
    surface.refresh(fb);
    surface.pending_bytes = 0;
    TextLog { runs: std::mem::take(&mut surface.pending),
        visible: surface.cells.iter().filter(|r| r.page == page).cloned().collect(), overflowed: false, page, regs: fb.regs }
}
pub(super) fn publish(fb: &Fb, page: usize, rgb: &[u32]) {
    if !enabled() { return; }
    let log = frame(fb, page);
    STATE.with(|s| s.borrow_mut().ready = Some((rgb.to_vec(), log)));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{data::ly::Rec, present::{ly::{self, Pen}, palette, prim::Raster, text::{self, Cursor}}};
    fn font() -> Font {
        let mut entries = vec![Vec::new(); 256];
        for c in [b'A', b'g', b'B', b'C', 1, 0xb1] {
            entries[c as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 4, x2: 3 }];
        }
        entries[b'B' as usize][0].colour = 7;
        let mut flags = [0; 256]; flags[b'g' as usize] = 8;
        Font::new(entries, [6; 256], &flags, 8)
    }
    fn scene(on: bool) -> (Vec<u8>, Cursor, TextLog) {
        set_enabled(on);
        let mut fb = Fb::new(); let font = font();
        for page in 0..2 { fb.fill(Raster::Clip, page, 0, 0, 200, 320, 3, 0); }
        let mut cursor = Cursor { r: Raster::Clip, page: 1, x: 10, y: 30, pen: 4, colour: 4,
            shadow: true, bg: 1, tab: 9, tabs: [20, 40], window_x: 10, ops: ly::OPS };
        text::draw(&mut fb, Raster::Wrap, 0, &font, b"AgB", 10, 10, 4, Some(2));
        text::glyph(&mut fb, Raster::Clip, 0, &font, b'A', 20, 10, &mut Pen::new(5), 5, None);
        cursor.text(&mut fb, &font, b"A\tg|A"); cursor.glyph(&mut fb, &font, b'B');
        cursor.counted_glyph(&mut fb, &font, 4, 1, 1, 50, 10, &[0, 1], &[0, 8]);
        cursor.wrapped(&mut fb, &font, b"AA AA AA", 70, 10, 15, 14, 8, 5, 3);
        cursor.at(i64::MAX, i64::MAX); cursor.glyph(&mut fb, &font, b'g');
        let log = frame(&fb, 0);
        if !on { assert!(fb.textlog.borrow().is_none()); }
        (fb.snapshot(), cursor, log)
    }
    #[test]
    fn restored_surface_survives_incremental_redraw() {
        set_enabled(true);let mut original=Fb::new();let font=font();
        text::draw(&mut original,Raster::Clip,0,&font,b"Ag",10,10,15,None);
        let rgb=original.rgb(0,&palette::ega_dac());let saved=capture(&original);
        let mut resumed=Fb::new();resumed.planes=original.planes.clone();resumed.regs=original.regs;
        restore(&resumed,&saved,&rgb).unwrap();
        assert_eq!(take_frame(&rgb).unwrap().visible,saved.as_ref().unwrap().last.as_ref().unwrap().visible);
        text::draw(&mut original,Raster::Clip,0,&font,b"B",30,10,14,None);
        let expected=original.rgb(0,&palette::ega_dac());let expected_text=capture(&original);
        text::draw(&mut resumed,Raster::Clip,0,&font,b"B",30,10,14,None);
        assert_eq!(resumed.rgb(0,&palette::ega_dac()),expected);
        assert_eq!(capture(&resumed),expected_text,"unchanged cells and new glyph must agree");
        set_enabled(false);
    }
    #[test]
    fn all_text_entry_points_leave_planes_and_cursor_byte_identical() {
        assert!(!enabled());
        let (off, off_cursor, empty) = scene(false);
        let (on, on_cursor, log) = scene(true);
        assert_eq!(on, off); assert_eq!(on_cursor, off_cursor); assert!(empty.runs.is_empty());
        assert_eq!(log.runs.iter().take(3).map(|r| r.bytes[0]).collect::<Vec<_>>(), b"AgB");
        assert_eq!((log.runs[0].page, log.runs[0].x, log.runs[0].y, log.runs[0].colour, log.runs[0].shadow), (0, 10, 10, 4, Some(2)));
        assert!(log.runs[1].descend); assert_eq!(log.runs[1].row(), 12);
        assert_eq!(log.runs[2].colour, 7); assert!(log.runs[0].background.iter().all(|&c| c == 3));
        assert_eq!(log.runs.iter().filter(|r| r.bytes == [1]).count(), 1);
        assert_eq!(log.runs.iter().filter(|r| r.bytes == [0xb1]).count(), 1);
        assert!(!log.runs.iter().any(|r| r.bytes == [b'(']));
        assert!(log.runs.iter().all(|r| r.font_id == log.runs[0].font_id));
        set_enabled(false);
    }
    #[test]
    fn retained_cells_erasure_highlight_redraw_page_copy_and_handoff() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        fb.fill(Raster::Clip, 0, 0, 0, 200, 320, 3, 0);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 4, Some(1));
        let first = frame(&fb, 0); assert_eq!(first.runs.len(), 1); assert_eq!(first.visible.len(), 1);
        let second = frame(&fb, 0); assert!(second.runs.is_empty()); assert_eq!(second.visible, first.visible);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, Some(1));
        let redraw = frame(&fb, 0); assert!(redraw.visible[0].background.iter().all(|&c| c == 3));
        fb.fill(Raster::Clip, 0, 16, 16, 8, 6, 15, 24);
        let highlight = frame(&fb, 0); assert_eq!(highlight.visible[0].colour, 6 ^ 15);
        let r = &highlight.visible[0];
        for y in 0..r.erase_height() { for x in 0..r.erase_width() {
            assert_eq!(r.background[y * r.erase_width() + x], if y < 8 && x < 6 { 3 ^ 15 } else { 3 });
        } }
        fb.fill(Raster::Clip, 0, 16, 16, 6, 6, 15, 24);
        let short_highlight = frame(&fb, 0); assert_eq!(short_highlight.visible[0].colour, 6);
        assert_eq!(short_highlight.visible[0].background[0], 3);
        assert_eq!(short_highlight.visible[0].background[7 * short_highlight.visible[0].erase_width()], 3 ^ 15);
        fb.copy(0, 0, 0, 200, 40, 1, 0, 0);
        let copy = frame(&fb, 1); assert_eq!(copy.visible.len(), 1); assert_eq!(copy.visible[0].page, 1);
        fb.copy(1, 0, 0, 200, 40, usize::MAX, 0, 0);
        assert_eq!(frame(&fb, 7).visible.len(), 1);
        fb.fill(Raster::Clip, 0, 16, 16, 8, 6, 2, 0);
        assert!(frame(&fb, 0).visible.is_empty()); assert_eq!(frame(&fb, 1).visible.len(), 1);
        let rgb = fb.rgb(1, &palette::ega_dac());
        assert_eq!(take_frame(&rgb).unwrap().visible.len(), 1); assert!(take_frame(&rgb).is_none());
        fb.rgb(1, &palette::ega_dac()); assert!(take_frame(&[0]).is_none());
        let snap = fb.snapshot(); fb.restore(&snap); assert!(frame(&fb, 1).visible.is_empty());
        set_enabled(false);
    }
    #[test]
    fn direct_font_layout_is_logged_but_artwork_is_not() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        ly::layout(&mut fb, Raster::Clip, 0, &font.glyphs, b'g' as usize, 12, 10, &mut Pen::new(6));
        let art = font.glyphs.clone();
        ly::layout(&mut fb, Raster::Clip, 0, &art, b'A' as usize, 40, 10, &mut Pen::new(6));
        let log = frame(&fb, 0); assert_eq!(log.runs.len(), 1);
        assert_eq!((log.runs[0].y, log.runs[0].row(), log.runs[0].bytes.as_slice()), (10, 12, b"g".as_slice()));
        set_enabled(false);
    }
    #[test]
    fn consecutive_menus_replace_events_and_capture_the_new_underlay() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        fb.fill(Raster::Clip, 0, 10, 10, 30, 80, 3, 0);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"AAA", 12, 12, 3, None);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"BBB", 22, 12, 7, None);
        fb.fill(Raster::Clip, 0, 10, 10, 30, 80, 3, 0);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"CC", 12, 12, 6, None);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"g", 22, 12, 6, Some(1));
        let log = frame(&fb, 0);
        for runs in [&log.runs, &log.visible] {
            assert_eq!(runs.iter().flat_map(|r| r.bytes.iter().copied()).collect::<Vec<_>>(), b"CCg");
            assert!(runs.iter().all(|r| r.background.iter().all(|&c| c == 3)));
        }
        let shadow = log.visible.last().unwrap();
        assert_eq!((shadow.erase_width(), shadow.erase_height(), shadow.background.len()), (7, 11, 77));
        text::draw(&mut fb, Raster::Clip, 0, &font, b"B", 12, 12, 7, None);
        let log = frame(&fb, 0);
        assert_eq!(log.visible.iter().filter(|r| r.x == 12 && r.y == 12).count(), 1);
        assert!(log.visible.last().unwrap().background.iter().all(|&c| c == 3));
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 12, 12, 6, None);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"C", 12, 12, 6, None);
        assert_eq!(frame(&fb, 0).runs.iter().map(|r| r.bytes[0]).collect::<Vec<_>>(), b"C");
        set_enabled(false);
    }
    #[test]
    fn a_highlight_outline_keeps_the_rows_space() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A B", 16, 16, 6, None);
        fb.rect(Raster::Clip, 0, 15, 15, 23, 40, 9, 0);
        let got: Vec<(u8, bool)> = frame(&fb, 0).visible.iter().map(|r| (r.bytes[0], r.covered)).collect();
        assert_eq!(got, [(b'A', true), (b' ', true), (b'B', true)], "the outline keeps the gap");
        fb.fill(Raster::Clip, 0, 15, 22, 10, 6, 9, 0);
        assert!(!frame(&fb, 0).visible.iter().any(|r| r.bytes[0] == b' '), "a fill over the space retires it");
        set_enabled(false);
    }
    #[test]
    fn a_fill_through_the_shadow_row_keeps_the_glyph() {
        set_enabled(true);
        let mut entries = vec![Vec::new(); 256]; entries[b'A' as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 7, x2: 3 }];
        let font = Font::new(entries, [6; 256], &[0; 256], 8);
        for (top, kept) in [(28, true), (26, false)] {
            let mut fb = Fb::new();
            text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 20, 16, 6, Some(2));
            let r = &frame(&fb, 0).visible[0]; let w = r.erase_width();
            assert!((0..w).any(|x| r.bitmap[8 * w + x] != r.background[8 * w + x]), "the shadow has ink on row 8");
            fb.fill(Raster::Clip, 0, top, 0, 10, 320, 1, 0);
            let got = frame(&fb, 0).visible;
            assert_eq!(got.len() == 1, kept, "a fill from row {top}"); if kept { assert_eq!(got[0].erase_height(), 8); }
        }
        set_enabled(false);
    }
    #[test]
    fn every_non_text_writer_retires_touched_cells_even_when_pixels_match() {
        set_enabled(true);
        let font = font();
        for writer in 0..9 {
            let mut fb = Fb::new();
            text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
            text::draw(&mut fb, Raster::Clip, 0, &font, b"C", 50, 50, 6, None);
            match writer {
                0 => fb.pixel(Raster::Clip, 0, 16, 16, 6, 0),
                1 => fb.line(Raster::Clip, 0, 16, 16, 16, 17, 6, 0),
                2 => fb.fill(Raster::Clip, 0, 16, 16, 1, 1, 6, 0),
                3 => fb.rect(Raster::Clip, 0, 16, 16, 17, 17, 6, 0),
                4 => { let art = font.glyphs.clone(); ly::draw(&mut fb, Raster::Clip, 0, &art, b'A' as usize, 16, 16); }
                5 => fb.blit(0, &[6], 1, 16, 16, 0, 0, 1, 1, None),
                6 => fb.copy(1, 16, 2, 1, 1, 0, 16, 2),
                7 => fb.copy_rect(1, 0, 16, 16, 1, 1),
                _ => super::super::pic::upload(&mut fb, &[0xff], 1, 16 * 40 + 2, 40, 1, 1, 0, 15),
            }
            let log = frame(&fb, 0);
            for list in [&log.visible, &log.runs] {
                let got: Vec<(u8, bool)> = list.iter().map(|r| (r.bytes[0], r.covered)).collect();
                assert!(got.contains(&(b'C', false)) && got.iter().all(|&(b, c)| b == b'C' || (b == b'A' && c)), "writer {writer}: {got:?}");
                if matches!(writer, 0 | 2 | 3) { assert!(got.contains(&(b'A', true)), "writer {writer}: a decoration's touch keeps the glyph covered"); }
                if writer == 1 { assert!(!got.iter().any(|&(b, _)| b == b'A'), "writer {writer}: a line through the glyph retires it as before"); }
            }
        }
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
        fb.rect(Raster::Clip, 0, 15, 21, 21, 26, 9, 0);
        let log = frame(&fb, 0);
        assert_eq!(log.visible.iter().map(|r| (r.bytes[0], r.covered)).collect::<Vec<_>>(), [(b'A', true)], "touched beside the ink: covered, kept");
        assert_eq!(log.runs.iter().map(|r| (r.bytes[0], r.covered)).collect::<Vec<_>>(), [(b'A', true)]);
        fb.fill(Raster::Clip, 0, 16, 16, 2, 2, 9, 0);
        assert!(frame(&fb, 0).visible.iter().any(|r| r.bytes[0] == b'A' && r.covered), "a corner under the cursor: still covered");
        fb.fill(Raster::Clip, 0, 16, 16, 4, 3, 0, 0);
        assert!(frame(&fb, 0).visible.is_empty(), "a mostly hidden letter retires the cell");
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
        fb.fill(Raster::Clip, 0, 16, 16, 8, 6, 6, 0);
        assert!(frame(&fb, 0).visible.is_empty(), "a flood of the ink colour retires the cell");
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
        fb.fill(Raster::Clip, 0, 16, 16, 5, 4, 6, 0);
        assert!(frame(&fb, 0).visible.iter().any(|r| r.bytes[0] == b'A' && r.covered), "an identical rewrite of the ink keeps the letter, covered");
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
        text::draw(&mut fb, Raster::Clip, 0, &font, &[1], 16, 16, 9, None);
        let got: Vec<(u8, bool)> = frame(&fb, 0).visible.iter().map(|r| (r.bytes[0], r.covered)).collect();
        assert!(got.contains(&(b'A', true)) && got.contains(&(1, false)), "{got:?}");
        text::draw(&mut fb, Raster::Clip, 0, &font, b"B", 16, 16, 6, None);
        let got: Vec<u8> = frame(&fb, 0).visible.iter().map(|r| r.bytes[0]).collect();
        assert_eq!(got, b"B", "a letter over the cell still replaces everything there: {got:?}");
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 16, 16, 6, None);
        fb.blit(0, &[1], 1, 16, 16, 0, 0, 1, 1, Some(1));
        assert_eq!(frame(&fb, 0).visible.len(), 1);
        let _ = fb.plane_mut(0);
        assert!(frame(&fb, 0).visible.is_empty());
        text::draw(&mut fb, Raster::Clip, 0, &font, b"C", 16, 16, 6, None);
        fb.fill(Raster::Clip, 0, 0, 0, 200, 320, 0, 0);
        let log = frame(&fb, 0); assert!(log.visible.is_empty() && log.runs.is_empty());
        set_enabled(false);
    }
    #[test]
    fn artwork_below_ink_preserves_text_without_restoring_over_the_art() {
        set_enabled(true);
        let mut fb = Fb::new(); let font = font();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"g", 108, 98, 6, None);
        let before = fb.indexed(0);
        fb.blit(0, &[15; 24], 24, 117, 96, 0, 0, 1, 24, None);
        let log = frame(&fb, 0); assert_eq!(log.visible.len(), 1);
        let r = &log.visible[0]; assert_eq!(r.bytes, b"g"); assert_eq!(r.bottom(), 117);
        assert_eq!(r.background.len(), r.erase_width() * r.erase_height());
        for y in 108..117 { for x in 98..104 { assert_eq!(fb.get(0, y, x), before[y as usize * 320 + x as usize]); } }
        fb.pixel(Raster::Clip, 0, 110, 98, 6, 0);
        assert!(frame(&fb, 0).visible.iter().any(|r| r.bytes == b"g" && r.covered));
        fb.fill(Raster::Clip, 0, 108, 98, 9, 6, 6, 0);
        assert!(frame(&fb, 0).visible.is_empty());
        set_enabled(false);
    }
    #[test]
    fn bottom_menu_text_keeps_its_face_across_page_copies() {
        set_enabled(true);
        let mut fb = Fb::new(); let font = font();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"Ag", 191, 200, 6, None);
        fb.copy(0, 141, 24, 59, 16, 1, 141, 24);
        let log = frame(&fb, 1);
        assert_eq!(log.visible.iter().map(|r| r.bytes[0]).collect::<Vec<_>>(), b"Ag");
        assert!(log.visible.iter().all(|r| r.bottom() <= 200));
        set_enabled(false);
    }
    #[test]
    fn partial_panel_copy_keeps_only_complete_matching_glyphs() {
        set_enabled(true);
        let font = font();
        for same in [false, true] {
            let mut fb = Fb::new();
            text::draw(&mut fb, Raster::Clip, 0, &font, b"g", 181, 200, 6, None);
            if same { text::draw(&mut fb, Raster::Clip, 1, &font, b"g", 181, 200, 6, None); }
            fb.copy(0, 54, 24, 130, 16, 1, 54, 24);
            let log = frame(&fb, 1);
            assert_eq!(log.visible.len(), 1, "a partial glyph cannot stand in for a complete matching cell; its cut strip survives, flagged by carrying no text");
            assert_eq!((log.visible[0].bytes.is_empty(), log.visible[0].erase_height()), (!same, if same { 10 } else { 3 }));
        }
        set_enabled(false);
    }
    #[test]
    fn a_cut_line_survives_a_rectangle_copy_as_a_flagged_strip_without_changing_the_frame() {
        let font = font();
        let run = |on: bool, rows: i32| {
            set_enabled(on);
            let mut fb = Fb::new();
            fb.fill(Raster::Clip, 1, 0, 0, 200, 320, 3, 0);
            text::draw(&mut fb, Raster::Clip, 1, &font, b"AgB", 100, 16, 15, Some(2));
            fb.copy_rect(1, 0, 0, 0, rows, 320);
            let log = frame(&fb, 0);
            let rgb = fb.rgb(0, &palette::ega_dac());
            let again = frame(&fb, 0).visible.len();
            set_enabled(false);
            (fb, rgb, log, again)
        };
        let (off, off_rgb, _, _) = run(false, 103);
        let (fb, rgb, log, again) = run(true, 103);
        assert_eq!(fb.snapshot(), off.snapshot(), "the log never writes planes"); assert_eq!(rgb, off_rgb, "the presented frame is identical");
        assert_eq!(log.visible.len(), 3, "the three glyphs cut three rows in survive as strips");
        for r in &log.visible {
            assert!(r.bytes.is_empty(), "a strip carries no text: the flag");
            assert_eq!((r.y, r.erase_height(), r.erase_width(), r.width as usize), (100, 3, 7, 7));
            for (i, &c) in r.bitmap.iter().enumerate().filter(|(i, _)| i % 7 < 6) { assert_eq!(c, fb.get(0, 100 + (i / 7) as i32, r.x as i32 + (i % 7) as i32), "the strip is the copied ink (its shadow column is under the next glyph's layer)"); }
            assert!(r.background.iter().all(|&c| c == 3), "the strip's underlay is the page fill");
        }
        assert_eq!(again, 3, "strips survive the refresh of a quiet frame");
        let (_, _, whole, _) = run(true, 120);
        assert_eq!(whole.visible.iter().map(|r| r.bytes.clone()).collect::<Vec<_>>(), vec![b"A".to_vec(), b"g".to_vec(), b"B".to_vec()], "a complete copy still carries whole glyphs");
        let (_, _, blank, _) = run(true, 100);
        assert!(blank.visible.is_empty(), "a copy ending above the ink keeps no strip");
    }
    #[test]
    fn diagonal_miss_should_keep_text() {
        set_enabled(true);
        let mut fb = Fb::new(); let font = font();
        fb.fill(Raster::Clip, 0, 0, 0, 200, 320, 0x21, 0);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 10, 80, 15, None);
        let before = fb.indexed(0);
        fb.line(Raster::Clip, 0, 0, 0, 100, 100, 6, 0);
        let after = fb.indexed(0);
        for y in 10..18 { for x in 80..86 { assert_eq!(before[y * 320 + x], after[y * 320 + x]); } }
        let log = frame(&fb, 0);
        assert_eq!(log.visible.len(), 1, "line never touches the glyph cell");
        assert_eq!(log.runs.len(), 1);
        set_enabled(false);
    }
    #[test]
    fn wrapped_out_of_page_copy_keeps_identical_cells_and_drops_changed_ones() {
        set_enabled(true);
        let font = font();
        for (dst, dy, dx, page, y) in [(0, 0, 40, 0, 1), (0, 1, -40, 0, 0), (0, -1, 40, 0, 0),
            (0, 204, 32, 1, 0), (7, 204, 32, 0, 0), (0, 1638, 16, 0, 0)] {
            for same in [true, false] {
                let mut fb = Fb::new();
                fb.fill(Raster::Clip, page, 0, 0, 200, 320, 0x21, 0);
                text::draw(&mut fb, Raster::Clip, page, &font, b"A", y, 0, 15, None);
                text::draw(&mut fb, Raster::Clip, page, &font, b"C", 100, 100, 15, None);
                let row: Vec<_> = (0..8).map(|x| if same { fb.get(page, y, x) } else { 7 }).collect();
                fb.blit(3, &row, 8, 0, 0, 0, 0, 1, 8, None);
                let before = fb.snapshot();
                fb.copy(3, 0, 0, 1, 1, dst, dy, dx);
                assert_eq!(fb.snapshot() == before, same, "{dst}:{dy},{dx}");
                let log = frame(&fb, page);
                let kept: Vec<_> = log.visible.iter().map(|r| r.bytes[0]).collect();
                assert_eq!(kept, if same { b"AC".to_vec() } else { b"C".to_vec() }, "same={same} copy to {dst}:{dy},{dx}");
                assert_eq!(diagnostics()[0], 1);
            }
        }
        set_enabled(false);
    }
    #[test]
    fn line_retirement_matches_raster_coverage() {
        let lines = [(0, 0, 100, 100), (100, 100, 0, 0), (0, 100, 100, 0), (100, 0, 0, 100),
            (40, 0, 60, 100), (60, 100, 40, 0), (0, 40, 100, 60), (100, 60, 0, 40),
            (0, 60, 100, 40), (100, 40, 0, 60), (50, 0, 50, 100), (0, 50, 100, 50), (50, 50, 50, 50),
            (0, 0, 50, 100), (0, 0, 100, 50)];
        for raster in [Raster::Clip, Raster::Wrap] { for (y1, x1, y2, x2) in lines {
            set_enabled(false);
            let mut mask = Fb::new();
            mask.line(raster, 0, y1, x1, y2, x2, 1, 0);
            set_enabled(true);
            let mut fb = Fb::new(); let font = font(); let mut expected = Vec::new();
            for y in (0..110).step_by(10) { for x in (0..110).step_by(10) {
                text::draw(&mut fb, raster, 0, &font, b"A", y, x, 0, Some(0));
                if (y..y + 9).all(|py| (x..x + 7).all(|px| mask.get(0, py, px) == 0)) { expected.push((y as i64, x as i64)); }
            } }
            let before = fb.snapshot();
            fb.line(raster, 0, y1, x1, y2, x2, 0, 0);
            assert_eq!(fb.snapshot(), before);
            let log = frame(&fb, 0);
            for runs in [&log.runs, &log.visible] {
                assert_eq!(runs.iter().map(|r| (r.y, r.x)).collect::<Vec<_>>(), expected, "{raster:?}: ({y1},{x1}) -> ({y2},{x2})");
            }
        } }
        set_enabled(false);
    }
    #[test]
    fn clipped_and_empty_copies_preserve_untouched_text() {
        set_enabled(true);
        let mut fb = Fb::new(); let font = font();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 1, 0, 15, None);
        fb.copy_rect(1, 0, 0, 320, 1, 8);
        fb.copy(1, 0, 0, 0, 1, 0, 0, 40);
        fb.copy(1, 0, 0, 1, 0, 0, 0, 40);
        let log = frame(&fb, 0);
        assert_eq!(log.visible.len(), 1);
        assert_eq!(log.runs.len(), 1);
        set_enabled(false);
    }
    #[test]
    fn toggles_discard_old_annotations_but_preserve_live_font_registration() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        ly::layout(&mut fb, Raster::Clip, 0, &font.glyphs, b'A' as usize, 10, 10, &mut Pen::new(6));
        let rgb = fb.rgb(0, &palette::ega_dac());
        set_enabled(false);
        assert!(take_frame(&rgb).is_none());
        set_enabled(true);
        ly::layout(&mut fb, Raster::Clip, 0, &font.glyphs, b'C' as usize, 30, 10, &mut Pen::new(6));
        let log = frame(&fb, 0);
        assert_eq!(log.runs.iter().map(|r| r.bytes[0]).collect::<Vec<_>>(), b"C");
        assert_eq!(log.visible.iter().map(|r| r.bytes[0]).collect::<Vec<_>>(), b"C");
        set_enabled(false);
        let key = font.glyphs.as_ptr() as usize;
        drop(font);
        STATE.with(|s| assert!(!s.borrow().fonts.contains_key(&key)));
        set_enabled(true);
        assert!(frame(&fb, 0).visible.is_empty());
        set_enabled(false);
    }
    #[test]
    fn event_and_byte_budgets_fall_back_and_recover_next_frame() {
        set_enabled(true);
        let mut font = font(); let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 10, 10, 6, None);
        for _ in 0..20_000 { text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 1000, 1000, 6, None); }
        {
            let surface = fb.textlog.borrow(); let surface = surface.as_ref().unwrap();
            assert!(surface.overflowed); assert!(surface.pending.is_empty()); assert!(surface.cells.is_empty());
            assert!(surface.pending.capacity() <= MAX_EVENTS);
        }
        let log = frame(&fb, 0);
        assert!(log.overflowed && log.runs.is_empty() && log.visible.is_empty());
        text::draw(&mut fb, Raster::Clip, 0, &font, b"C", 30, 10, 6, None);
        let log = frame(&fb, 0);
        assert!(!log.overflowed); assert_eq!(log.visible[0].bytes, b"C");
        font.widths[b'A' as usize] = 255;
        let mut events = 0;
        loop {
            text::draw(&mut fb, Raster::Clip, 0, &font, b"A", 60, 0, 6, None);
            events += 1;
            if fb.textlog.borrow().as_ref().unwrap().overflowed { break; }
            assert!(events < MAX_EVENTS);
        }
        assert!(events < MAX_EVENTS);
        assert!(frame(&fb, 0).overflowed);
        set_enabled(false);
    }
    #[test]
    fn local_refresh_matches_full_refresh_with_overlapping_cells_and_xor() {
        set_enabled(true);
        let font = font(); let mut fb = Fb::new();
        fb.fill(Raster::Clip, 0, 0, 0, 200, 320, 3, 0);
        for (y, x) in [(10, 10), (15, 13), (20, 16), (80, 50)] {
            text::draw(&mut fb, Raster::Clip, 0, &font, b"A", y, x, 6, Some(1));
        }
        fb.fill(Raster::Clip, 0, 10, 10, 6, 6, 15, 24);
        let mut surface = fb.textlog.borrow_mut(); let surface = surface.as_mut().unwrap();
        let mut reference = Surface { cells: surface.cells.clone(), ..Surface::default() };
        let target = surface.cells[0].clone();
        surface.refresh_cell(&fb, &target); reference.refresh(&fb);
        assert_eq!(surface.cells, reference.cells);
        set_enabled(false);
    }
}
pub fn take_frame(rgb: &[u32]) -> Option<TextLog> {
    if !enabled() { return None; }
    STATE.with(|s| s.borrow_mut().ready.take().filter(|(image, _)| image == rgb).map(|(_, log)| log))
}

pub(super) fn copy(fb: &Fb, raster: Raster, src: usize, sy: i64, sx: i64, h: i64, w: i64, dst: usize, dy: i64, dx: i64) {
    if !enabled() || h <= 0 || w <= 0 { return; }
    let (src, dst) = (src & 7, dst & 7);
    let mut state = fb.textlog.borrow_mut();
    let Some(surface) = state.as_mut() else { return; };
    surface.current();
    if surface.overflowed { return; }
    if raster == Raster::Wrap && (dx < 0 || dy < 0 || dx.saturating_add(w) > 320 || dy.saturating_add(h) > 200) {
        count(0); return;
    }
    surface.refresh(fb);
    let inside = |r: &Run, page, y: i64, x: i64| r.page == page && r.x >= x && r.y >= y
        && r.right() <= x.saturating_add(w) && r.bottom() <= y.saturating_add(h);
    let copies: Vec<_> = surface.cells.iter().filter_map(|r| {
        if r.page != src || r.x >= sx.saturating_add(w) || r.right() <= sx || r.y >= sy.saturating_add(h) || r.bottom() <= sy { return None; }
        let whole = inside(r, src, sy, sx);
        let mut r = r.clone(); r.page = dst; r.x = r.x.saturating_add(dx - sx); r.y = r.y.saturating_add(dy - sy);
        if !whole {
            let exact = (0..r.erase_height()).all(|row| (0..r.erase_width()).all(|col| {
                let (x, y) = (r.x + col as i64, r.y + row as i64);
                (0..320).contains(&x) && (0..200).contains(&y) && r.bitmap[row * r.erase_width() + col] == if (dx..dx.saturating_add(w)).contains(&x) && (dy..dy.saturating_add(h)).contains(&y) {
                    fb.get(src, (y - dy + sy) as i32, (x - dx + sx) as i32)
                } else { fb.get(dst, y as i32, x as i32) }
            }));
            if !exact { return cut(&r, dx, dy, w, h); }
        }
        Some(r)
    }).collect();
    if surface.cells.len() + copies.len() > MAX_EVENTS
        || surface.pending_bytes + surface.cell_bytes + copies.iter().map(run_bytes).sum::<usize>() > MAX_LOG_BYTES {
        surface.overflow(); return;
    }
    surface.retire(|r| r.page == dst && r.x < dx.saturating_add(w) && r.right() > dx && r.y < dy.saturating_add(h) && r.bottom() > dy);
    surface.cells.extend(copies);
    surface.recount();
}
fn cut(r: &Run, dx: i64, dy: i64, w: i64, h: i64) -> Option<Run> {
    let (l, t) = (r.x.max(dx), r.y.max(dy));
    let (right, bottom) = (r.right().min(dx.saturating_add(w)), r.bottom().min(dy.saturating_add(h)));
    if l >= right || t >= bottom || r.bitmap.len() != r.background.len() || !r.onscreen() { return None; }
    let stride = r.erase_width();
    let strip = |v: &[u8]| -> Vec<u8> { (t - r.y..bottom - r.y).flat_map(|row| v[row as usize * stride + (l - r.x) as usize..row as usize * stride + (right - r.x) as usize].to_vec()).collect() };
    let (background, bitmap) = (strip(&r.background), strip(&r.bitmap));
    if background == bitmap { return None; }
    Some(Run { x: l, y: t, width: (right - l).min(255) as u8, background_width: (right - l) as u16, background_height: (bottom - t) as u8,
        bytes: Vec::new(), background, bitmap, ..r.clone() })
}
