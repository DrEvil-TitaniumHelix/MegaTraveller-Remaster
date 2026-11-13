use super::{Button, UiEvent, UiState};
use mt1::ui::{Scene, Unit};

#[derive(Clone, Copy, Debug)]
pub struct Drag { pub button: Button, pub start: (i32, i32), pub camera: (f32, f32), pub log_lines: u8 }
#[derive(Clone, Debug)]
pub struct InputContext { pub size: (u32, u32), pub drag: Option<Drag> }
impl Default for InputContext { fn default() -> Self { Self { size: (192, 112), drag: None } } }
#[derive(Clone, Default)]
pub struct State {
    pub combat: super::combat_bar::State,
    pub log: super::combat_log::History,
    pub viewport_size: Option<(u32, u32)>,
    pub log_size: Option<(u32, u32)>,
    pub log_header_extra: usize,
    pub pointer: Option<(mt1::ui::ComponentId, (i32, i32))>,
    pub picker: std::sync::Arc<std::sync::Mutex<super::party_strip::Note>>,
}
pub fn begin(ui: &mut UiState, e: &UiEvent, at: (i32, i32), size: (u32, u32)) {
    ui.component_input.size = size;
    if let UiEvent::Down(button) = e { ui.component_input.drag = Some(Drag { button: *button, start: at, camera: ui.camera, log_lines: ui.log_lines }); }
}
pub fn end(ui: &mut UiState, e: &UiEvent) {
    if let UiEvent::Up(button) = e { if ui.component_input.drag.is_some_and(|d| d.button == *button) { ui.component_input.drag = None; } }
}
pub fn dispatch(layout: &super::Layout, id: mt1::ui::ComponentId, scene: &Scene, ui: &mut UiState, event: &UiEvent, at: (i32, i32)) -> Vec<mt1::ui::Intent> {
    let Some(host) = layout.host(id) else { return Vec::new() };
    ui.camera = camera(scene, ui);
    if !matches!(event, UiEvent::Key(_)) { ui.components.pointer = Some((id, at)); }
    begin(ui, event, at, host.size());
    let mut intents = layout.input(id, scene, ui, event, at);
    super::combat_bar::track(id, scene, ui, event, at, &mut intents);
    end(ui, event); intents
}
pub fn prepare(layout: &super::Layout, ui: &mut UiState) {
    ui.components.viewport_size = layout.host("Viewport").map(|h| h.size());
    ui.components.log_size = layout.host("Combat log").map(|h| h.size());
    ui.components.log_header_extra = layout.log_header_extra;
    ui.log_scroll = ui.log_scroll.clamp(0, ui.log_oldest());
}
pub fn edge_tick(layout: &super::Layout, scene: &Scene, ui: &mut UiState) -> Vec<mt1::ui::Intent> {
    if scene.screen != mt1::ui::ScreenKind::Ground { return Vec::new() }
    let Some(("Viewport", at)) = ui.components.pointer else { return Vec::new() };
    if ui.component_input.drag.is_some() { return Vec::new() }
    let intents = dispatch(layout, "Viewport", scene, ui, &UiEvent::Move, at);
    intents.into_iter().filter(|i| matches!(i, mt1::ui::Intent::Pan(..) | mt1::ui::Intent::Follow(None))).collect()
}
pub fn observe(scene: &Scene, previous: Option<&Scene>, ui: &mut UiState) {
    super::viewport::GroundCamera::observe(scene, previous, ui);
    if scene.screen != mt1::ui::ScreenKind::Ground { return }
    let entered = previous.is_none_or(|p| p.map.rows != scene.map.rows || p.map.cols != scene.map.cols || p.level != scene.level || p.map.interior != scene.map.interior || p.map.cells.is_empty() || (p.screen != mt1::ui::ScreenKind::Ground && ui.follow.is_none()));
    if entered {
        ui.camera = super::viewport::Viewport::oracle_camera(scene);
        ui.follow = Some(selected(scene)); ui.component_input.drag = None;
    }
    if matches!(ui.follow, Some(Unit::Traveller(_))) && previous.is_some_and(|p| p.leader != scene.leader) {
        ui.follow = Some(selected(scene));
    }
    if ui.follow.is_some() { ui.camera = camera(scene, ui); }
    if scene.squad.encounter && !previous.is_some_and(|p| p.squad.encounter) { ui.encounters = ui.encounters.saturating_add(1); }
    ui.components.log.observe(&ui.frame);
    ui.components.log.observe_squad(&scene.squad);
}
pub fn hint(scene: &Scene, ui: &UiState) -> bool { scene.squad.encounter && ui.encounters <= 1 && scene.squad.round <= 3 }
pub fn zoom(z: f32) -> f32 { if z.is_finite() { z.clamp(0.25, 2.0) } else { 1.0 } }
pub fn next_zoom(z: f32, direction: i32) -> f32 {
    let z = zoom(z); let steps = [0.25, 0.5, 1.0, 2.0];
    if direction > 0 { steps.into_iter().find(|&v| v > z).unwrap_or(2.0) }
    else if direction < 0 { steps.into_iter().rev().find(|&v| v < z).unwrap_or(0.25) } else { z }
}
pub fn selected(scene: &Scene) -> Unit { Unit::Traveller(scene.leader.clamp(0, 4) as u8) }
pub fn position(scene: &Scene, unit: Unit) -> Option<(f32, f32)> {
    let half = match unit {
        Unit::Traveller(i) if i < 5 => {
            if !scene.combat { scene.pos } else { let w = &scene.walkers[i as usize]; if !w.active() { return None } (w.x(), w.y()) }
        }
        Unit::Npc(i) => { let n = scene.npcs.get(i.checked_sub(1)? as usize)?; if !n.present() { return None } (n.x(), n.y()) }
        _ => return None,
    };
    Some((half.0 as f32 * 0.5 + 0.5, half.1 as f32 * 0.5 + 0.5))
}
pub fn camera(scene: &Scene, ui: &UiState) -> (f32, f32) {
    if let Some(p) = ui.follow.and_then(|u| position(scene, u)) { return p }
    let finite = |v: f32, fallback: f32| if v.is_finite() { v.clamp(-32768.0, 32767.0) } else { fallback };
    (finite(ui.camera.0, scene.view.0 as f32 / 2.0 + 3.5), finite(ui.camera.1, scene.view.1 as f32 / 2.0 + 6.0))
}
pub fn cell(p: (f32, f32), scene: &Scene) -> (i16, i16) {
    let clamp = |v: f32, n: u16| v.floor().clamp(0.0, n.saturating_sub(1).min(i16::MAX as u16) as f32) as i16;
    (clamp(p.0, scene.map.rows), clamp(p.1, scene.map.cols))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn discrete_zoom_and_invalid_values() {
        assert_eq!(next_zoom(1.0, 1), 2.0); assert_eq!(next_zoom(0.25, -1), 0.25); assert_eq!(next_zoom(f32::NAN, -1), 0.5);
        assert_eq!(next_zoom(2.0, i32::MAX), 2.0);
    }
    #[test]
    fn first_ground_scene_after_a_starport_screen_follows_the_party() {
        let mut scene = Scene::program(mt1::ui::ScreenKind::Ground); scene.pos = (20, 40);
        let map = std::sync::Arc::make_mut(&mut scene.map); map.rows = 80; map.cols = 80; map.cells = vec![0; 80 * 80 * 3];
        let mut previous = scene.clone(); previous.screen = mt1::ui::ScreenKind::Picture;
        let mut ui = UiState::default();
        observe(&scene, Some(&previous), &mut ui);
        assert_eq!(ui.follow, Some(Unit::Traveller(0))); assert_eq!(ui.camera, (10.5, 20.5));
        ui.follow = None; ui.camera = (3.0, 3.0); previous.screen = mt1::ui::ScreenKind::Ground; observe(&scene, Some(&previous), &mut ui);
        assert_eq!((ui.follow, ui.camera), (None, (3.0, 3.0)));
    }
    #[test]
    fn portrait_follow_survives_record_exit_and_tracks_later_leader_changes() {
        let mut scene = Scene::program(mt1::ui::ScreenKind::Ground);
        let map = std::sync::Arc::make_mut(&mut scene.map); map.rows = 80; map.cols = 80; map.cells = vec![0; 80 * 80 * 3];
        scene.combat = true;
        scene.walkers[2].0[0] = 1;
        scene.walkers[2].0[8..10].copy_from_slice(&20i16.to_le_bytes());
        scene.walkers[2].0[10..12].copy_from_slice(&40i16.to_le_bytes());
        let mut previous = scene.clone(); previous.screen = mt1::ui::ScreenKind::Record;
        let mut ui = UiState { follow: Some(Unit::Traveller(2)), ..UiState::default() };
        observe(&scene, Some(&previous), &mut ui);
        assert_eq!(ui.follow, Some(Unit::Traveller(2))); assert_eq!(ui.camera, (10.5, 20.5));
        previous = scene.clone(); scene.leader = 1; observe(&scene, Some(&previous), &mut ui);
        assert_eq!(ui.follow, Some(Unit::Traveller(1)));
    }
    #[test]
    fn routed_drag_uses_local_size_and_followed_camera_and_stops_on_matching_release() {
        use mt1::ui::{Intent, ScreenKind};
        use super::super::{assets::Assets, components, Host, Layout, Rect};
        let mut scene = Scene::program(ScreenKind::Ground); scene.pos = (20, 40);
        let map = std::sync::Arc::make_mut(&mut scene.map); map.rows = 80; map.cols = 80;
        let mut ui = UiState { camera: (0.0, 0.0), follow: Some(Unit::Traveller(0)), ..UiState::default() };
        let mut layout = Layout::default(); components::register(&mut layout, std::sync::Arc::new(Assets::default()));
        layout.set_host("Viewport", Host::Dock(Rect::new(40, 60, 400, 240)));
        dispatch(&layout, "Viewport", &scene, &mut ui, &UiEvent::Down(Button::Middle), (40, 40));
        assert_eq!(ui.component_input.size, (400, 240)); assert_eq!(ui.component_input.drag.unwrap().camera, (10.5, 20.5));
        assert!(dispatch(&layout, "Viewport", &scene, &mut ui, &UiEvent::Move, (56, 56)).iter().all(|i| matches!(i, Intent::Hover(_))), "no drag pan");
        end(&mut ui, &UiEvent::Up(Button::Left)); assert!(ui.component_input.drag.is_some());
        end(&mut ui, &UiEvent::Up(Button::Middle)); assert!(ui.component_input.drag.is_none());
        dispatch(&layout, "Viewport", &scene, &mut ui, &UiEvent::Move, (399, 80));
        assert!(edge_tick(&layout, &scene, &mut ui).is_empty(), "no edge pan");
        prepare(&layout, &mut ui); assert_eq!(ui.components.viewport_size, Some((400, 240)));
    }
}
