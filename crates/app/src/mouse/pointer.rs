use crate::{look::Look, panels::{self, render::Metrics, viewport::Camera, Host, Layout, Origin, Rect, UiState}};
use mt1::ui::{ComponentId, Scene, ScreenKind, Target, Unit};
use mt_engine::io::MouseState;

pub fn live_mouse(component: Option<(ComponentId, Origin, (i32, i32))>) -> MouseState {
    let absent = MouseState { buttons: 0, col: -1, row: -1 };
    let Some((id, Origin::Classic1990(r), at)) = component else { return absent };
    if id == "Viewport" || !Rect::new(0, 0, r.w, r.h).contains(at) { return absent }
    let Some(col) = r.x.checked_add(at.0).and_then(|v| i16::try_from(v).ok()) else { return absent };
    let Some(row) = r.y.checked_add(at.1).and_then(|v| i16::try_from(v).ok()) else { return absent };
    MouseState { buttons: 0, col, row }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapCell { pub col: i16, pub row: i16 }
impl MapCell { pub fn row_col(self) -> (i16, i16) { (self.row, self.col) } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Element {
    Map(Option<MapCell>), Traveller(u8), Initiative(Unit),
    Pixel(i32, i32),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ComponentHit { pub id: ComponentId, pub local: (i32, i32), pub element: Element }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hit { Outside, Background, Component(ComponentHit) }

pub fn window(layout: &Layout, scene: &Scene, ui: &UiState, metrics: Metrics, look: &Look,
    host: Option<u64>, at: (f64, f64)) -> Hit {
    if !at.0.is_finite() || !at.1.is_finite() || at.0 < 0.0 || at.1 < 0.0
        || at.0 >= metrics.pixels.out_width as f64 || at.1 >= metrics.pixels.out_height as f64 { return Hit::Outside }
    let Some(pixel) = metrics.unwarp(at, look, false) else { return Hit::Background };
    let Some((id, _)) = layout.hit_mapped(scene, host, pixel, |r| {
        let p = metrics.rect(r); Rect::new(p.x as i32, p.y as i32, p.width as u32, p.height as u32)
    }) else { return Hit::Background };
    let Some(h) = layout.host(id) else { return Hit::Background };
    let rect = match h { Host::Dock(r) => r, Host::Window { size, .. } => Rect::new(0, 0, size.0, size.1) };
    let local = metrics.local(pixel, rect, look);
    Hit::Component(ComponentHit { id, local, element: element(id, scene, ui, h.size(), local, layout.is_classic()) })
}

pub fn element(id: ComponentId, scene: &Scene, ui: &UiState, size: (u32, u32), at: (i32, i32), classic: bool) -> Element {
    if id == "Viewport" && scene.screen == ScreenKind::Ground { return Element::Map(map_cell(scene, ui, size, at, classic)) }
    if id == "Party strip" || id == "Picture" && scene.screen == ScreenKind::Record {
        if let Some(i) = panels::picture::portrait(at) { return Element::Traveller(i) }
    }
    if id == "Initiative order" {
        use panels::Component;
        let c = panels::initiative::Initiative::new(std::sync::Arc::default());
        if let Some(Target::Unit(u)) = c.input(scene, ui, &panels::UiEvent::Move, at).into_iter()
            .find_map(|i| match i { mt1::ui::Intent::Hover(t) => t, _ => None }) { return Element::Initiative(u) }
    }
    Element::Pixel(at.0, at.1)
}

pub fn hostile(scene: &Scene, k: u8) -> bool {
    scene.npcs.get(k.wrapping_sub(1) as usize).is_some_and(|n| n.attitude() & 3 == 0 || n.was_hit())
}
pub fn line(scene: &Scene, ui: &UiState) -> Option<((i16, i16), (i16, i16), u8)> {
    let (to, colour) = match ui.hover? {
        Target::Cell(c) => (c, 10),
        Target::Unit(Unit::Npc(k)) => (panels::controls::position(scene, Unit::Npc(k)).map(|p| ((p.0 - 0.5) as i16, (p.1 - 0.5) as i16))?, if hostile(scene, k) { 12 } else { 10 }),
        _ => return None,
    };
    if to.0 < 0 || to.1 < 0 || to.0 as u16 >= scene.map.rows || to.1 as u16 >= scene.map.cols { return None }
    let p = panels::controls::position(scene, Unit::Traveller(scene.squad.selected))?;
    Some((((p.0 - 0.5) as i16, (p.1 - 0.5) as i16), to, colour))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shape { Walk, Fire, Talk, Door, Item }
pub fn shape(scene: &Scene, ui: &UiState) -> Option<Shape> {
    if !scene.squad.active || ui.menu.is_some() || !ui.components.pointer.is_some_and(|(id, _)| id == "Viewport") { return None }
    Some(match ui.hover? {
        Target::Cell(c) => if crate::mouse::walk::door(scene, c) || scene.map.cell(c.0 as i32, c.1 as i32).is_some_and(|b| mt1::ground::orders::trigger_name(b[0], b[1]).is_some()) { Shape::Door } else { Shape::Walk },
        Target::Unit(Unit::Npc(k)) => if hostile(scene, k) { Shape::Fire } else { Shape::Talk },
        Target::Object(_) => Shape::Item,
        _ => return None,
    })
}

pub fn camera(scene: &Scene, ui: &UiState, size: (u32, u32), classic: bool) -> Camera {
    if classic { Camera { top: scene.view.0 as f32 * 8.0, left: scene.view.1 as f32 * 8.0, zoom: 1.0 } }
    else { Camera::new(scene, ui, size) }
}

pub fn map_cell(scene: &Scene, ui: &UiState, size: (u32, u32), at: (i32, i32), classic: bool) -> Option<MapCell> {
    if ui.fixed_screen && !panels::viewport::screen_rect(size).contains(at) { return None; }
    if !Rect::new(0, 0, size.0, size.1).contains(at) { return None }
    let world = if ui.fixed_screen {
        let (r, c) = panels::viewport::Viewport::fixed_world(scene, (!classic).then_some(ui), size, at)?; (r as f32, c as f32)
    } else { camera(scene, ui, size, classic).world(at) };
    let (row, col) = ((world.0 / 16.0).floor(), (world.1 / 16.0).floor());
    if !row.is_finite() || !col.is_finite() || row < 0.0 || col < 0.0
        || row >= scene.map.rows as f32 || col >= scene.map.cols as f32
        || row > i16::MAX as f32 || col > i16::MAX as f32 { return None }
    Some(MapCell { col: col as i16, row: row as i16 })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn live_mouse_never_exposes_map_or_held_pad_and_preserves_picture_rows() {
        let absent = MouseState { buttons: 0, col: -1, row: -1 };
        assert_eq!(live_mouse(None), absent);
        assert_eq!(live_mouse(Some(("New", Origin::New, (0, 0)))), absent);
        for (id, rect) in [("Party strip", panels::CLASSIC[0]), ("Side panel", panels::CLASSIC[1]),
            ("Combat log", panels::CLASSIC[2]), ("Viewport", panels::CLASSIC[3]), ("Picture", panels::FRAME)] {
            for y in 0..rect.h as i32 { for x in 0..rect.w as i32 {
                let m = live_mouse(Some((id, Origin::Classic1990(rect), (x, y))));
                assert_eq!(m.buttons, 0, "even over cols 198..265 / rows 141..171");
                if id == "Viewport" { assert_eq!(m, absent); }
                else { assert_eq!((m.col, m.row), ((rect.x + x) as i16, (rect.y + y) as i16)); }
            } }
            for at in [(-1, 0), (0, -1), (rect.w as i32, 0), (0, rect.h as i32), (i32::MAX, i32::MIN)] {
                assert_eq!(live_mouse(Some((id, Origin::Classic1990(rect), at))), absent);
            }
        }
        assert_eq!(live_mouse(Some(("Picture", Origin::Classic1990(Rect::new(i32::MAX, 0, 4, 4)), (1, 0)))), absent);
        assert_eq!(live_mouse(Some(("Picture", Origin::Classic1990(Rect::new(0, i32::MAX, 4, 4)), (0, 1)))), absent);
    }
    #[test]
    fn line_runs_selected_to_pointer_green_on_ground_red_on_a_thug_never_off_map() {
        let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.pos = (8, 8);
        let m = std::sync::Arc::make_mut(&mut s.map); m.rows = 12; m.cols = 20; m.stride = 20; m.cells = [1, 0, 0].repeat(12 * 20);
        m.cells[(2 * 20 + 7) * 3 + 2] = 14; m.cells[(2 * 20 + 9) * 3 + 2] = 17;
        s.npcs = (0..32).map(|_| mt1::ui::scene::Npc([0; 46])).collect(); s.npcs[16].0[13] = 2;
        for (k, (r, c)) in [(14, (2, 7)), (17, (2, 9))] { s.npcs[k - 1].0[32..34].copy_from_slice(&(r * 2i16).to_le_bytes()); s.npcs[k - 1].0[34..36].copy_from_slice(&(c * 2i16).to_le_bytes()); }
        let hover = |t| UiState { hover: t, ..UiState::default() };
        assert_eq!(line(&s, &hover(Some(Target::Cell((6, 9))))), Some(((4, 4), (6, 9), 10)));
        assert_eq!(line(&s, &hover(Some(Target::Unit(Unit::Npc(14))))), Some(((4, 4), (2, 7), 12)));
        assert_eq!(line(&s, &hover(Some(Target::Unit(Unit::Npc(17))))), Some(((4, 4), (2, 9), 10)));
        assert_eq!(line(&s, &hover(Some(Target::Unit(Unit::Traveller(0))))), None);
        assert_eq!(line(&s, &hover(Some(Target::Cell((12, 0))))), None);
        assert_eq!(line(&s, &hover(Some(Target::Cell((0, -1))))), None);
        assert_eq!(line(&s, &hover(None)), None);
        let over = |t| { let mut u = hover(t); u.components.pointer = Some(("Viewport", (10, 10))); u };
        assert_eq!(shape(&s, &over(Some(Target::Cell((6, 9))))), Some(Shape::Walk));
        assert_eq!(shape(&s, &over(Some(Target::Unit(Unit::Npc(14))))), Some(Shape::Fire));
        assert_eq!(shape(&s, &over(Some(Target::Unit(Unit::Npc(17))))), Some(Shape::Talk));
        assert_eq!(shape(&s, &over(Some(Target::Object(3)))), Some(Shape::Item));
        assert_eq!(shape(&s, &over(Some(Target::Unit(Unit::Traveller(1))))), None);
        assert_eq!(shape(&s, &hover(Some(Target::Cell((6, 9))))), None, "not over the Viewport");
        let mut u = over(Some(Target::Cell((6, 9)))); u.menu = Some((Target::Cell((0, 0)), (0, 0))); assert_eq!(shape(&s, &u), None);
        std::sync::Arc::make_mut(&mut s.map).interior = b'I'; let i = (6 * 20 + 9) * 3; std::sync::Arc::make_mut(&mut s.map).cells[i..i + 3].copy_from_slice(&[1, 236, 0]);
        assert_eq!(shape(&s, &over(Some(Target::Cell((6, 9))))), Some(Shape::Door));
        let i = (3 * 20 + 3) * 3; std::sync::Arc::make_mut(&mut s.map).cells[i..i + 3].copy_from_slice(&[0, 38, 0]);
        assert_eq!(shape(&s, &over(Some(Target::Cell((3, 3))))), Some(Shape::Door));
        std::sync::Arc::make_mut(&mut s.map).cells[i] = 1; assert_eq!(shape(&s, &over(Some(Target::Cell((3, 3))))), Some(Shape::Walk));
    }
    #[test]
    fn maps_beyond_the_old_picture_without_clamping_off_map_positions() {
        let mut s = Scene::program(ScreenKind::Ground); let m = std::sync::Arc::make_mut(&mut s.map); m.rows = 100; m.cols = 100;
        let ui = UiState { camera: (40.0, 50.0), zoom: 1.0, ..UiState::default() };
        assert_eq!(map_cell(&s, &ui, (640, 400), (0, 0), false), Some(MapCell { col: 30, row: 27 }));
        assert_eq!(map_cell(&s, &ui, (640, 400), (639, 399), false), Some(MapCell { col: 69, row: 52 }));
        assert_eq!(map_cell(&s, &ui, (640, 400), (-1, 0), false), None);
        assert_eq!(map_cell(&s, &ui, (640, 400), (640, 0), false), None);
        let edge = UiState { camera: (0.0, 0.0), ..ui.clone() };
        assert_eq!(map_cell(&s, &edge, (640, 400), (0, 0), false), None);
        let far = UiState { camera: (10000.0, 10000.0), ..ui.clone() };
        assert_eq!(map_cell(&s, &far, (640, 400), (639, 399), false), None);
        s.view = (7, 9);
        assert_eq!(map_cell(&s, &ui, (192, 112), (0, 0), true), Some(MapCell { row: 3, col: 4 }));
        assert_eq!(map_cell(&s, &ui, (192, 112), (191, 111), true), Some(MapCell { row: 10, col: 16 }));
    }
}
