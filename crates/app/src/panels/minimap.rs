use super::{assets::Assets, controls, viewport::Camera, Button, Component, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene, Unit};
use std::sync::Arc;

pub const HINT: &str = "Area map: the frame is your view";
pub struct Minimap { assets: Arc<Assets> }
impl Minimap { pub fn new(assets: Arc<Assets>) -> Self { Self { assets } } }
#[derive(Clone, Copy)]
struct MapRect { x: f32, y: f32, scale: f32 }
impl MapRect {
    fn new(scene: &Scene, size: (u32, u32)) -> Option<Self> {
        if scene.map.cols == 0 || scene.map.rows == 0 || size.0 == 0 || size.1 == 0 { return None }
        let scale = (size.0 as f32 / scene.map.cols as f32).min(size.1 as f32 / scene.map.rows as f32);
        Some(Self { x: (size.0 as f32 - scene.map.cols as f32 * scale) / 2.0, y: (size.1 as f32 - scene.map.rows as f32 * scale) / 2.0, scale })
    }
    fn point(self, row: f32, col: f32) -> (i32, i32) { ((self.x + col * self.scale).floor() as i32, (self.y + row * self.scale).floor() as i32) }
    fn cell(self, scene: &Scene, at: (i32, i32)) -> (i16, i16) { controls::cell(((at.1 as f32 - self.y) / self.scale, (at.0 as f32 - self.x) / self.scale), scene) }
    fn contains(self, scene: &Scene, at: (i32, i32)) -> bool { at.0 as f32 >= self.x && at.1 as f32 >= self.y && (at.0 as f32) < self.x + scene.map.cols as f32 * self.scale && (at.1 as f32) < self.y + scene.map.rows as f32 * self.scale }
}
fn marker(out: &mut Surface, at: (i32, i32), colour: u8) { for y in -1..=1 { for x in -1..=1 { out.put(at.0.saturating_add(x), at.1.saturating_add(y), colour); } } }
pub fn attitude(flags: u8) -> u8 { match flags & 3 { 0 => 12, 1 => 9, _ => 7 } }
impl Component for Minimap {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        out.indexed.fill(8);
        let Some(rect) = MapRect::new(scene, (out.w as u32, out.h as u32)) else { return };
        for y in 0..out.h { for x in 0..out.w {
            if !rect.contains(scene, (x as i32, y as i32)) { continue }
            let (r, c) = rect.cell(scene, (x as i32, y as i32));
            if let Some([f, tile, _]) = scene.map.cell(r as i32, c as i32) {
                let b = if f & 1 != 0 { self.assets.terrain.get(scene.map.tileset as usize) } else { self.assets.objects.get(4) };
                if let Some(colour) = b.and_then(|b| b.get(tile as i32 % 20 * 16 + 8, tile as i32 / 20 * 16 + 8)) { out.put(x as i32, y as i32, colour); }
            }
        } }
        let on_map = |p: (f32, f32)| p.0 >= 0.0 && p.1 >= 0.0 && p.0 < scene.map.rows as f32 && p.1 < scene.map.cols as f32;
        for (i, n) in scene.npcs.iter().enumerate().take(32) {
            if n.present() { if let Some(p) = controls::position(scene, Unit::Npc(i as u8 + 1)).filter(|&p| on_map(p)) { marker(out, rect.point(p.0, p.1), attitude(n.attitude())); } }
        }
        for i in 0..5 { if scene.party.mask() & (1 << i) != 0 || scene.walkers[i].active() {
            if let Some(p) = controls::position(scene, Unit::Traveller(i as u8)).filter(|&p| on_map(p)) { marker(out, rect.point(p.0, p.1), i as u8 + 1); }
        } }
        let size = if ui.fixed_screen { super::viewport::SCREEN_PIXELS } else { ui.components.viewport_size.unwrap_or((192, 112)) }; let camera = Camera::new(scene, ui, size);
        let a = rect.point(camera.top / 16.0, camera.left / 16.0);
        let b = rect.point((camera.top + size.1 as f32 / camera.zoom) / 16.0, (camera.left + size.0 as f32 / camera.zoom) / 16.0);
        for x in a.0.max(0)..=b.0.min(out.w as i32 - 1) { out.put(x, a.1, 15); out.put(x, b.1, 15); }
        for y in a.1.max(0)..=b.1.min(out.h as i32 - 1) { out.put(a.0, y, 15); out.put(b.0, y, 15); }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let Some(rect) = MapRect::new(scene, ui.component_input.size) else { return Vec::new() };
        let dragging = ui.component_input.drag.is_some_and(|d| d.button == Button::Left);
        if !rect.contains(scene, at) && !dragging { return Vec::new() }
        let cell = rect.cell(scene, at);
        match *e {
            UiEvent::Down(Button::Left) | UiEvent::Up(Button::Left) => vec![Intent::Follow(None), Intent::PanTo(cell)],
            UiEvent::Move if dragging => vec![Intent::Follow(None), Intent::PanTo(cell)],
            UiEvent::Move => vec![Intent::Hover(None), Intent::Hint(Some(HINT))],
            UiEvent::Key(k) => vec![Intent::Raw(mt_engine::io::Event::Key(k))],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fitted_letterboxing_click_and_drag() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); let m = Arc::make_mut(&mut s.map); m.rows = 20; m.cols = 40;
        let c = Minimap::new(Arc::default()); let mut ui = UiState::default(); ui.component_input.size = (100, 100);
        assert!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (50, 10)).is_empty());
        assert_eq!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (50, 50)), [Intent::Follow(None), Intent::PanTo((10, 20))]);
        controls::begin(&mut ui, &UiEvent::Down(Button::Left), (50, 50), (100, 100));
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (i32::MAX, i32::MIN)), [Intent::Follow(None), Intent::PanTo((0, 39))]);
        assert_eq!([attitude(0), attitude(1), attitude(2)], [12, 9, 7]);
    }
    #[test]
    fn whole_level_markers_and_camera_box() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); let m = Arc::make_mut(&mut s.map); m.rows = 10; m.cols = 20; m.cells = [1, 0, 0].repeat(200);
        let mut npc = mt1::ui::scene::Npc([0; 46]); npc.0[32] = 2; npc.0[34] = 2; s.npcs.push(npc);
        let mut assets = Assets::default(); assets.terrain.push(super::super::assets::Bitmap { w: 16, h: 16, pixels: vec![2; 256] });
        let c = Minimap::new(Arc::new(assets)); let mut ui = UiState { camera: (5.0, 10.0), ..UiState::default() }; ui.components.viewport_size = Some((16, 16));
        let mut out = Surface::new(40, 20); c.draw(&s, &ui, &mut out);
        assert_eq!(out.get(3, 3), Some(12)); assert_eq!(out.get(19, 9), Some(15)); assert_eq!(out.get(30, 16), Some(2));
        c.draw(&s, &ui, &mut Surface::new(0, 0));
    }
}
