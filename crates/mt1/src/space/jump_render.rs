use super::jump::Call;
use mt_engine::{data::ly::Entry, present::{fb::Fb, ly::{self, Pen}, prim::Raster, text::{self, Font}}};

pub struct Surface<'a> {
    pub fb: &'a mut Fb,
    pub layouts: &'a [Entry],
    pub font: &'a Font,
    pub active_page: usize,
    pub page: usize,
    pub color: u8,
    pub y: i16,
    pub x: i16,
}
impl Surface<'_> {
    pub fn apply(&mut self, call: &Call) -> bool {
        match call {
            Call::BackBuffer => self.page = self.active_page ^ 1,
            Call::Layout(index, y, x) => ly::layout(self.fb, Raster::Wrap, self.page, self.layouts,
                *index as u16 as usize, *y as i32, *x as i32, &mut Pen::new(self.color)),
            Call::Labels(labels) => for label in labels {
                if label.flags == 255 { break; }
                self.color = label.flags & 15; self.y = label.y; self.x = label.x;
                self.draw_text(&label.text);
            },
            Call::Color(color) => self.color = *color as u8,
            Call::TextPosition(y, x) => { self.y = *y; self.x = *x; }
            Call::Text(value) => self.draw_text(value),
            Call::Fill(y, x, h, w, color) => self.fb.fill(Raster::Wrap, self.page,
                *y as i32, *x as i32, *h as i32, *w as i32, *color as u8, 0),
            Call::Flip => { self.active_page ^= 1; self.page = self.active_page ^ 1; }
            Call::Copy(y, x, h, w) => self.fb.copy(self.active_page, *y as i32, *x as i32, *h as i32,
                *w as i32, self.active_page ^ 1, *y as i32, *x as i32),
            Call::Restore(y, x, h, w) => self.fb.copy(self.active_page ^ 1, *y as i32, *x as i32, *h as i32,
                *w as i32, self.active_page, *y as i32, *x as i32),
            Call::Palette(index, value) => self.fb.set_reg(*index as usize, *value as u8),
            _ => return false,
        }
        true
    }
    fn draw_text(&mut self, value: &[u8]) {
        self.x = text::draw(self.fb, Raster::Wrap, self.page, self.font, value,
            self.y as i32, self.x as i32, self.color, None) as i16;
    }
}
