pub mod background;
pub mod palette;
pub mod scale;
pub mod screen;
pub mod text;
use crate::look::{Look, MAX_WINDOW_SIDE};
use std::cell::RefCell;
pub use text::TextLog;

thread_local! { static CRT_CACHE: RefCell<screen::CrtCache> = RefCell::new(screen::CrtCache::default()); }

#[derive(Clone)]
pub struct Frame {
    pub indexed: [u8; 64000],
    pub regs: [u8; 16],
    pub text: TextLog,
}
impl Default for Frame {
    fn default() -> Self {
        Self { indexed: [0; 64000], regs: mt_engine::present::palette::DEFAULT_REGS, text: TextLog::default() }
    }
}
impl Frame {
    pub fn from_rgb(rgb: &[u32]) -> Self {
        let mut frame = Self::default();
        if let Some(log) = mt_engine::present::textlog::take_frame(rgb) { frame.regs = log.regs; frame.text = log; }
        let colors = mt_engine::present::palette::colors(&frame.regs, &mt_engine::present::palette::ega_dac());
        for (dst, &src) in frame.indexed.iter_mut().zip(rgb) {
            *dst = colors.iter().position(|&c| c == src).unwrap_or(0) as u8;
        }
        frame
    }
}
pub fn render(frame: &Frame, look: &Look, out_w: usize, out_h: usize) -> Vec<u32> {
    let rect = scale::picture_rect(out_w, out_h, look);
    if rect.width == 0 || rect.height == 0 { return background::paint(out_w, out_h, look); }
    let indexed = text::background(&frame.indexed, &frame.text, look);
    let colors = palette::map(&indexed, &frame.regs, look);
    let picture = scale::scale(&colors, rect.width, rect.height, look);
    let mut out = CRT_CACHE.with(|cache| {
        if let Ok(mut cache) = cache.try_borrow_mut() { screen::compose(&picture, rect, look, &mut cache) }
        else { screen::compose(&picture, rect, look, &mut screen::CrtCache::default()) }
    });
    text::overdraw(&mut out, rect, &indexed, &frame.text, look);
    out
}
pub(crate) fn pixel_count(w: usize, h: usize) -> Option<usize> {
    if w > MAX_WINDOW_SIDE as usize || h > MAX_WINDOW_SIDE as usize { return None; }
    w.checked_mul(h)
}
pub fn buffer(w: usize, h: usize, color: u32) -> Vec<u32> {
    let Some(len) = pixel_count(w, h) else { return Vec::new() };
    let mut out = Vec::new();
    if out.try_reserve_exact(len).is_ok() { out.resize(len, color); }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rgb_boundary_and_tiny_surfaces() {
        let colors = mt_engine::present::palette::colors(&mt_engine::present::palette::DEFAULT_REGS, &mt_engine::present::palette::ega_dac());
        let rgb: Vec<_> = (0..64000).map(|i| colors[i % 16]).collect();
        let frame = Frame::from_rgb(&rgb);
        assert_eq!(render(&frame, &Look { shape: crate::look::Shape::Square, ..Look::default() }, 320, 200), rgb);
        for (w, h) in [(0, 0), (0, 17), (1, 1), (319, 199), (usize::MAX, 1)] {
            let out = render(&frame, &Look::default(), w, h);
            assert_eq!(out.len(), pixel_count(w, h).unwrap_or(0));
        }
    }
    #[test]
    fn renders_at_window_cap() {
        let side = MAX_WINDOW_SIDE as usize;
        let look = Look { scale: crate::look::ScaleMode::Fill, ..Look::default() };
        let mut frame = Frame::default(); frame.indexed.fill(15);
        let color = mt_engine::present::palette::colors(&frame.regs, &mt_engine::present::palette::ega_dac())[15];
        let out = render(&frame, &look, side, side);
        assert_eq!(out.len(), side * side);
        let rect = scale::picture_rect(side, side, &look);
        assert_eq!(out[0], 0); assert_eq!(out[side * side - 1], 0);
        for y in rect.y..rect.y + rect.height {
            assert!(out[y * side + rect.x..y * side + rect.x + rect.width].iter().all(|&p| p == color));
        }
        assert!(buffer(side + 1, 1, 0).is_empty()); assert!(buffer(1, side + 1, 0).is_empty());
    }
}
