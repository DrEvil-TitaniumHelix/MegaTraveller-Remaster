use super::{Dock, Layout, Rect, Surface, UiState};
use crate::{look::{Filter, Look, ScaleMode, Screen, Shape}, pipeline::{self, scale::PictureRect}};
use mt1::ui::{Preset, Scene, ScreenKind};

#[derive(Clone, Copy, Debug)]
pub struct Metrics { pub pixels: PictureRect, pub logical: (u32, u32), pub classic: bool }
impl Metrics {
    pub fn for_ui(size: [u32; 2], look: &Look, scene: Option<&Scene>, ui: &UiState) -> Self {
        if look.layout.preset != super::settings::LayoutPreset::Modern || !scene.is_some_and(|s| super::choices::beside_picture(s, ui)) {
            return Self::main(size, look, scene);
        }
        let [w, h] = crate::look::clamp_window_size(size);
        let logical = super::choices::logical(scene.unwrap(), ui);
        let aspect = if look.shape == Shape::Classic { 1.2 } else { 1.0 };
        let fit = (w as f64 / logical.0 as f64).min(h as f64 / (logical.1 as f64 * aspect));
        let scale = fit;
        let width = ((logical.0 as f64 * scale).round().max(1.0) as usize).min(w as usize);
        let height = ((logical.1 as f64 * aspect * scale).round().max(1.0) as usize).min(h as usize);
        Self { pixels: PictureRect { x: (w as usize - width) / 2, y: (h as usize - height) / 2, width, height,
            out_width: w as usize, out_height: h as usize }, logical, classic: false }
    }
    pub fn main(size: [u32; 2], look: &Look, scene: Option<&Scene>) -> Self {
        let [w, h] = crate::look::clamp_window_size(size);
        let product = Self::pages_fill(size, look);
        let classic = !product || scene.is_none_or(|s| s.screen != ScreenKind::Ground);
        let rect = pipeline::scale::picture_rect(w as usize, h as usize, &Self::page_look(size, look));
        if classic { return Self { pixels: rect, logical: (320, 200), classic }; }
        let required_height = super::components::PARTY_RECT.h + super::party_strip::NAMES_H + super::components::MINIMAP_MIN_H + super::components::SIDE_RECT.h + super::toolbar::HEIGHT
            + super::combat_bar::CombatBar::placement().size.preferred.1;
        let aspect = if look.shape == Shape::Classic { 1.2 } else { 1.0 };
        let fit = (rect.width as f64 / 320.0).min(h as f64 / (required_height as f64 * aspect));
        let fit_1990 = (rect.width as f64 / 320.0).min(h as f64 / ((required_height - super::components::MINIMAP_MIN_H) as f64 * aspect));
        let s = if look.scale == ScaleMode::Integer && fit_1990 >= 1.0 { fit_1990.floor() } else { fit };
        Self::scaled([w, h], s, look, false)
    }
    pub fn pages_fill(size: [u32; 2], look: &Look) -> bool { let [w, h] = crate::look::clamp_window_size(size); look.layout.preset != super::settings::LayoutPreset::Classic && w >= 640 && h >= 480 }
    pub fn page_look(size: [u32; 2], look: &Look) -> Look { if Self::pages_fill(size, look) { Look { scale: ScaleMode::Fill, ..look.clone() } } else { look.clone() } }
    pub fn component(size: [u32; 2], preferred: (u32, u32), look: &Look) -> Self {
        let [w, h] = crate::look::clamp_window_size(size);
        let a = if look.shape == Shape::Classic { 1.2 } else { 1.0 };
        let fit = (w as f64 / preferred.0.max(1) as f64).min(h as f64 / (preferred.1.max(1) as f64 * a));
        let s = if look.scale == ScaleMode::Integer && fit >= 1.0 { fit.floor() } else { fit };
        Self::scaled([w, h], s, look, false)
    }
    fn scaled([w, h]: [u32; 2], s: f64, look: &Look, classic: bool) -> Self {
        let a = if look.shape == Shape::Classic { 1.2 } else { 1.0 };
        let s = s.max(1.0 / 8192.0);
        Self { pixels: PictureRect { width: w as usize, height: h as usize, out_width: w as usize, out_height: h as usize, ..Default::default() },
            logical: (((w as f64 / s).floor() as u32).clamp(1, 8192), ((h as f64 / (s * a)).floor() as u32).clamp(1, 8192)), classic }
    }
    pub fn rect(&self, r: Rect) -> PictureRect {
        let edge = |v: i64, extent: usize, logical: u32| (v.max(0) as u64 * extent as u64 / u64::from(logical.max(1))).min(extent as u64) as usize;
        let x = edge(r.x as i64, self.pixels.width, self.logical.0);
        let y = edge(r.y as i64, self.pixels.height, self.logical.1);
        let right = edge(r.x as i64 + r.w as i64, self.pixels.width, self.logical.0);
        let bottom = edge(r.y as i64 + r.h as i64, self.pixels.height, self.logical.1);
        PictureRect { x: self.pixels.x + x, y: self.pixels.y + y, width: right.saturating_sub(x), height: bottom.saturating_sub(y), ..self.pixels }
    }
    pub fn unwarp(&self, at: (f64, f64), look: &Look, captured: bool) -> Option<(i32, i32)> {
        let p = self.pixels;
        if !at.0.is_finite() || !at.1.is_finite() || p.width == 0 || p.height == 0 { return None; }
        let (x, y) = (at.0 - p.x as f64, at.1 - p.y as f64);
        if captured && (x < 0.0 || y < 0.0 || x >= p.width as f64 || y >= p.height as f64) { return Some((x.clamp(-1e6, 1e6) as i32 + p.x as i32, y.clamp(-1e6, 1e6) as i32 + p.y as i32)); }
        if x < 0.0 || y < 0.0 || x >= p.width as f64 || y >= p.height as f64 { return None; }
        let (mut x, mut y) = (x as usize, y as usize);
        if look.screen == Screen::Crt {
            let (hw, hh) = (p.width as f32 * 0.5, p.height as f32 * 0.5);
            let (cx, cy) = (x as f32 + 0.5 - hw, y as f32 + 0.5 - hh);
            let (xn, yn) = (cx / hw, cy / hh);
            let (sx, sy) = (hw + cx * (1.0 + 0.035 * yn * yn), hh + cy * (1.0 + 0.035 * xn * xn));
            if sx < 0.0 || sy < 0.0 || sx >= p.width as f32 || sy >= p.height as f32 { return None; }
            x = sx as usize; y = sy as usize;
        }
        Some((x as i32 + p.x as i32, y as i32 + p.y as i32))
    }
    pub fn local(&self, pixel: (i32, i32), r: Rect, look: &Look) -> (i32, i32) {
        let p = self.rect(r);
        let sample = |v: i32, start: usize, len: usize, source: u32| {
            let v = i64::from(v) - start as i64;
            if len == 0 || source == 0 { return 0; }
            if v < 0 || v >= len as i64 { return (v * i64::from(source) / len as i64) as i32; }
            if look.filter == Filter::Nearest { return (v * i64::from(source) / len as i64) as i32; }
            let n = if look.filter == Filter::Sharp { (len / source as usize).max(1) } else { 1 };
            let expanded = source as usize * n;
            let f = ((v as f64 + 0.5) * expanded as f64 / len as f64 - 0.5).max(0.0);
            let a = f.floor() as usize;
            let i = if ((f - a as f64) * 256.0).round() >= 128.0 { (a + 1).min(expanded - 1) } else { a.min(expanded - 1) };
            (i / n) as i32
        };
        (sample(pixel.0, p.x, p.width, r.w), sample(pixel.1, p.y, p.height, r.h))
    }
    pub fn project(&self, r: Rect, at: (i32, i32), look: &Look) -> Option<(f64, f64)> {
        let p = self.rect(r); let host = self.pixels;
        if r.w == 0 || r.h == 0 || host.width == 0 || host.height == 0 { return None; }
        let (sx, sy) = (p.x as f64 + (at.0 as f64 + 0.5) * p.width as f64 / r.w as f64,
                        p.y as f64 + (at.1 as f64 + 0.5) * p.height as f64 / r.h as f64);
        let (tx, ty) = (2.0 * (sx - host.x as f64) / host.width as f64 - 1.0, 2.0 * (sy - host.y as f64) / host.height as f64 - 1.0);
        let (mut x, mut y) = (tx, ty);
        if look.screen == Screen::Crt { for _ in 0..12 { (x, y) = (tx / (1.0 + 0.035 * y * y), ty / (1.0 + 0.035 * x * x)); } }
        let out = (host.x as f64 + (x + 1.0) * host.width as f64 * 0.5, host.y as f64 + (y + 1.0) * host.height as f64 * 0.5);
        if look.screen != Screen::Crt { return Some(out); }
        let (fx, fy) = (out.0.floor(), out.1.floor());
        Some([(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0), (-1.0, 0.0), (0.0, -1.0), (-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0)].into_iter()
            .map(|(dx, dy)| (fx + dx + 0.5, fy + dy + 0.5))
            .find(|&c| self.unwarp(c, look, false).is_some_and(|px| self.local(px, r, look) == at)).unwrap_or(out))
    }
    pub fn logical_at(&self, pixel: (i32, i32)) -> (i32, i32) {
        let p = self.pixels;
        (((i64::from(pixel.0) - p.x as i64) * self.logical.0 as i64 / p.width.max(1) as i64) as i32,
         ((i64::from(pixel.1) - p.y as i64) * self.logical.1 as i64 / p.height.max(1) as i64) as i32)
    }
}

pub fn compose(surfaces: impl IntoIterator<Item = (Rect, Surface)>, metrics: Metrics, look: &Look) -> Vec<u32> {
    let mut out = pipeline::background::paint(metrics.pixels.out_width, metrics.pixels.out_height, look);
    let mut text_layers = Vec::new();
    for (r, mut surface) in surfaces {
        let p = metrics.rect(r);
        if p.width == 0 || p.height == 0 || out.is_empty() { continue; }
        pipeline::text::restore_background(&mut surface.indexed, (surface.w, surface.h), &surface.text, look);
        let colors = pipeline::palette::map(&surface.indexed, &surface.regs, look);
        let scaled = pipeline::scale::scale_surface(&colors, surface.w, surface.h, p.width, p.height, look);
        if scaled.len() != p.width * p.height { continue; }
        for y in 0..p.height { let at = (p.y + y) * p.out_width + p.x; out[at..at + p.width].copy_from_slice(&scaled[y * p.width..(y + 1) * p.width]); }
        if look.high_contrast_text() { text_layers.push((p, (surface.w, surface.h), surface.indexed, surface.text)); }
        else { pipeline::text::overdraw_surface(&mut out, p, (surface.w, surface.h), &surface.indexed, &surface.text, look); }
    }
    let base = pipeline::screen::apply(&out, metrics.pixels.out_width, metrics.pixels.out_height, look);
    if text_layers.is_empty() { return base; }
    let mut out = base.clone();
    for (p, size, indexed, text) in text_layers {
        for y in p.y..p.y + p.height {
            let at = y * p.out_width + p.x;
            out[at..at + p.width].copy_from_slice(&base[at..at + p.width]);
        }
        pipeline::text::overdraw_surface(&mut out, p, size, &indexed, &text, look);
    }
    out
}
pub fn main(layout: &mut Layout, scene: &Scene, ui: &UiState, look: &Look, size: [u32; 2]) -> Vec<u32> {
    let metrics = Metrics::for_ui(size, look, Some(scene), ui);
    let mut ui = ui.clone();
    if metrics.classic { ui.preset = Preset::Classic; }
    layout.arrange_for(scene, &ui, metrics.logical);
    super::controls::prepare(layout, &mut ui);
    if metrics.classic {
        let mut out = layout.compose(scene, &ui, (320, 200));
        if let Some((r, s)) = preview(layout, &ui, metrics.logical) { out.draw(&s, r.x, r.y); }
        return pipeline::render(&out.into_frame(), &Metrics::page_look(size, look), size[0] as usize, size[1] as usize);
    }
    let mut surfaces = Vec::new();
    for (id, r, s) in layout.draw_host(scene, &ui, None) {
        if ui.fixed_screen && id == "Viewport" {
            let map = super::viewport::screen_rect((r.w, r.h));
            let mut blank = Surface::new(r.w, r.h); blank.regs = s.regs;
            surfaces.push((r, blank));
            surfaces.push((Rect::new(r.x + map.x, r.y + map.y, map.w, map.h),
                layout.draw_source(id, scene, &ui, super::viewport::SCREEN_PIXELS)));
        } else { surfaces.push((r, s)); }
    }
    if let Some(surface) = preview(layout, &ui, metrics.logical) { surfaces.push(surface); }
    compose(surfaces, metrics, look)
}

fn preview(layout: &Layout, ui: &UiState, logical: (u32, u32)) -> Option<(Rect, Surface)> {
    let p = layout.placement(ui.dock_preview?)?;
    let (w, h) = logical;
    let r = match p.dock {
        Dock::Top => Rect::new(0, 0, w, h.min(2)), Dock::Bottom => Rect::new(0, h.saturating_sub(2) as i32, w, h.min(2)),
        Dock::Left => Rect::new(0, 0, w.min(2), h), _ => Rect::new(w.saturating_sub(2) as i32, 0, w.min(2), h),
    };
    let mut s = Surface::new(r.w, r.h); s.regs = ui.frame.regs; s.indexed.fill(14); Some((r, s))
}

#[cfg(test)]
mod fit_tests {
    use super::*;
    #[test]
    fn product_pages_fill_the_window_at_1035_x_871_and_classic_stays_integer() {
        let look = Look::default(); assert_eq!(look.scale, ScaleMode::Integer);
        for screen in [ScreenKind::Record, ScreenKind::Picture] {
            let m = Metrics::main([1035, 871], &look, Some(&Scene::program(screen)));
            let p = m.pixels; assert!(m.classic && (p.width == 1035 || p.height == 871), "{screen:?}: {p:?}");
            assert!((p.width as i64 * 240 - p.height as i64 * 320).abs() <= 320, "4:3 kept: {p:?}");
            assert!(p.width >= 1035 - 2 || p.height >= 871 - 2);
        }
        let mut classic = Look::default(); classic.layout.preset = super::super::settings::LayoutPreset::Classic;
        let p = Metrics::main([1035, 871], &classic, Some(&Scene::program(ScreenKind::Record))).pixels; assert_eq!((p.width, p.height), (960, 720));
        let p = Metrics::main([500, 400], &look, Some(&Scene::program(ScreenKind::Record))).pixels; assert_eq!((p.width, p.height), (320, 240), "a small window keeps the integer scale");
        let classic = Look { shape: Shape::Classic, scale: ScaleMode::Integer, ..Look::default() };
        let rect = pipeline::scale::picture_rect(1210, 1221, &Metrics::page_look([1210, 1221], &classic)); assert_eq!((rect.x, rect.y, rect.width, rect.height), (0, 157, 1210, 907));
        let m = Metrics::main([1210, 1221], &classic, Some(&Scene::program(ScreenKind::Record))).pixels; assert_eq!((m.x, m.y, m.width, m.height), (0, 157, 1210, 907), "metrics and compositor agree");
    }
}
