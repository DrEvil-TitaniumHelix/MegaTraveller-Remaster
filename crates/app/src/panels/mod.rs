pub mod picture;
pub mod assets;
pub mod viewport;
pub mod combat_log;
pub mod combat_bar;
pub mod minimap;
pub mod initiative;
pub mod context_menu;
pub mod toolbar;
pub mod choices;
pub mod textbox;
pub mod plus;
pub mod cargo;
pub mod buy;
pub mod tooltip;
pub mod party_strip;
pub mod side_panel;
pub mod place;
pub mod replay;
pub mod components;
pub mod controls;
pub mod keyboard;
pub mod settings;
#[cfg(test)]
mod regression_tests;
mod layout;
pub use layout::CLASSIC;
pub mod render;
use crate::pipeline::{Frame, TextLog};
use mt1::ui::{ComponentId, Intent, Preset, Scene, ScreenKind, Target, Unit};
pub use picture::Picture;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub struct Rect { pub x: i32, pub y: i32, pub w: u32, pub h: u32 }
impl Rect { pub const fn new(x: i32, y: i32, w: u32, h: u32) -> Rect { Rect { x, y, w, h } } }
pub const FRAME: Rect = Rect::new(0, 0, 320, 200);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Surface { pub w: usize, pub h: usize, pub indexed: Vec<u8>, pub regs: [u8; 16], pub text: TextLog }
impl Surface {
    pub fn new(w: u32, h: u32) -> Surface {
        let (w, h) = (w.min(crate::look::MAX_WINDOW_SIDE) as usize, h.min(crate::look::MAX_WINDOW_SIDE) as usize);
        let mut indexed = Vec::new();
        if indexed.try_reserve_exact(w * h).is_err() { return Surface { w: 0, h: 0, indexed, regs: mt_engine::present::palette::DEFAULT_REGS, text: TextLog::default() }; }
        indexed.resize(w * h, 0);
        Surface { w, h, indexed, regs: mt_engine::present::palette::DEFAULT_REGS, text: TextLog::default() }
    }
    pub fn get(&self, x: i32, y: i32) -> Option<u8> {
        (x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h).then(|| self.indexed[y as usize * self.w + x as usize])
    }
    pub fn put(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h { self.indexed[y as usize * self.w + x as usize] = c }
    }
    pub fn blit(&mut self, src: &[u8], stride: usize, src_h: usize, from: Rect, x: i32, y: i32) {
        let (stride, src_h) = (stride.min(i32::MAX as usize) as i64, src_h.min(i32::MAX as usize) as i64);
        let (sx, sy, dx, dy) = (from.x as i64, from.y as i64, x as i64, y as i64);
        let left = 0.max(-sx).max(-dx); let top = 0.max(-sy).max(-dy);
        let right = i64::from(from.w).min(stride - sx).min(self.w as i64 - dx);
        let bottom = i64::from(from.h).min(src_h - sy).min(self.h as i64 - dy);
        for r in top..bottom { for c in left..right {
            let source = (sy + r) as usize * stride as usize + (sx + c) as usize;
            let dest = (dy + r) as usize * self.w + (dx + c) as usize;
            if let (Some(pixel), Some(out)) = (src.get(source), self.indexed.get_mut(dest)) { *out = *pixel; }
        } }
    }
    pub fn draw(&mut self, s: &Surface, x: i32, y: i32) {
        self.blit(&s.indexed, s.w, s.h, Rect::new(0, 0, s.w as u32, s.h as u32), x, y);
        if self.text.visible.is_empty() && self.text.runs.is_empty() { self.text.page = s.text.page; self.text.regs = s.text.regs }
        let shift = |r: &mt_engine::present::textlog::Run| { let mut r = r.clone(); r.page = self.text.page; r.x = r.x.saturating_add(x as i64); r.y = r.y.saturating_add(y as i64); r };
        self.text.runs.extend(s.text.runs.iter().map(shift)); self.text.visible.extend(s.text.visible.iter().map(shift));
        self.text.overflowed |= s.text.overflowed;
    }
    pub fn cover(&mut self, s: &Surface, x: i32, y: i32) {
        let outside = |r: &mt_engine::present::textlog::Run| { let (a, b) = crate::pipeline::text::ink(r).unwrap_or((r.row(), r.row() + 7));
            r.x + i64::from(r.width) <= x as i64 || r.x >= x as i64 + s.w as i64 || b < y as i64 || a >= y as i64 + s.h as i64 };
        self.text.visible.retain(outside); self.text.runs.retain(outside);
        self.draw(s, x, y);
    }
    pub fn into_frame(self) -> Frame {
        let mut f = Frame { indexed: [0; 64000], regs: self.regs, text: self.text };
        for y in 0..200.min(self.h) { let n = 320.min(self.w); f.indexed[y * 320..y * 320 + n].copy_from_slice(&self.indexed[y * self.w..y * self.w + n]) }
        f
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dock { Top, Bottom, Left, Right, Fill, Float(Anchor) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Anchor { Pointer, Centre }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Size { pub preferred: (u32, u32), pub min: (u32, u32) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Origin { Classic1990(Rect), New }
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub dock: Dock, pub size: Size, pub order: u8,
    pub screens: &'static [ScreenKind],
    pub when: fn(&Scene) -> bool,
    pub origin: Origin, pub optional: bool,
    pub undockable: bool,
    pub title: &'static str,
}
pub fn always(_: &Scene) -> bool { true }

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum UiEvent { Move, Down(Button), Up(Button), DoubleClick(Button), Wheel(i32), Key(mt_engine::io::Key) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Button { Left, Right, Middle }

#[derive(Clone)]
pub struct UiState {
    pub keyboard: keyboard::State,
    pub decision: Option<mt_engine::play::Decision>,
    pub walk: std::cell::RefCell<crate::mouse::walk::Walk>,
    pub plus: crate::plus::State,
    pub frame: Box<Frame>,
    pub fixed_screen: bool,
    pub ground_camera: std::sync::Arc<std::sync::Mutex<viewport::GroundCamera>>,
    pub camera: (f32, f32), pub zoom: f32, pub follow: Option<Unit>,
    pub hover: Option<Target>, pub menu: Option<(Target, (i32, i32))>, pub focus: Option<Unit>,
    pub hint: Option<&'static str>,
    pub buy: Option<(String, usize)>,
    pub info: std::cell::Cell<Option<(Target, u8)>>,
    pub capture: Option<ComponentId>,
    pub log_scroll: i32, pub log_lines: u8, pub log_expanded: bool,
    pub minimap_zoom: f32, pub preset: Preset,
    pub debug_initiative: bool,
    pub menu_host: Option<ComponentId>,
    pub dock_preview: Option<ComponentId>,
    pub grip_hover: Option<ComponentId>,
    pub hosts: Vec<(ComponentId, HostKind)>,
    pub component_input: controls::InputContext,
    pub components: controls::State,
    pub replay: Option<(u32, f32)>,
    pub encounters: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HostKind { Docked, Window, Hidden }
impl Default for UiState {
    fn default() -> Self {
        UiState { keyboard: Default::default(), decision: None, buy: None, walk: Default::default(), plus: Default::default(), frame: Default::default(), fixed_screen: false, ground_camera: Default::default(), camera: (0.0, 0.0), zoom: 1.0, follow: None, hover: None, menu: None, focus: None, hint: None, info: Default::default(), capture: None,
            log_scroll: 0, log_lines: 3, log_expanded: false, minimap_zoom: 1.0, preset: Preset::Modern, debug_initiative: false,
            menu_host: None, dock_preview: None, grip_hover: None, hosts: Vec::new(),
            component_input: controls::InputContext::default(), components: controls::State::default(), replay: None, encounters: 0 }
    }
}

pub trait Component {
    fn visible(&self, _ui: &UiState) -> bool { true }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface);
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Host { Dock(Rect), Window { id: u64, size: (u32, u32) } }
impl Host { pub fn size(&self) -> (u32, u32) { match *self { Host::Dock(r) => (r.w, r.h), Host::Window { size, .. } => size } } }

struct Entry { component: Box<dyn Component>, placement: Placement, host: Host, suppressed: bool, initial_size: Size, overlay_window: Option<u64> }
#[derive(Default)]
pub struct Layout { entries: Vec<Entry>, classic: bool, log_header_extra: usize }
impl Layout {
    pub fn is_classic(&self) -> bool { self.classic }
    pub fn register(&mut self, component: Box<dyn Component>, placement: Placement) -> ComponentId {
        let id = placement.title;
        self.entries.push(Entry { component, placement, host: Host::Dock(FRAME), suppressed: false, initial_size: placement.size, overlay_window: None }); id
    }
    pub fn ids(&self) -> Vec<ComponentId> { self.entries.iter().map(|e| e.placement.title).collect() }
    pub fn placement(&self, id: ComponentId) -> Option<&Placement> { self.entries.iter().find(|e| e.placement.title == id).map(|e| &e.placement) }
    pub fn host(&self, id: ComponentId) -> Option<Host> { self.entries.iter().find(|e| e.placement.title == id).map(|e| e.host) }
    pub fn host_window(&self, id: ComponentId) -> Option<u64> {
        let e = self.entries.iter().find(|e| e.placement.title == id)?;
        e.overlay_window.or(match e.host { Host::Window { id, .. } => Some(id), _ => None })
    }
    pub fn set_host(&mut self, id: ComponentId, host: Host) { if let Some(e) = self.entries.iter_mut().find(|e| e.placement.title == id) { e.host = host } }
    pub fn arrange(&mut self, main: (u32, u32)) {
        self.arrange_for(&Scene::program(ScreenKind::Ground), &UiState::default(), main);
    }
    fn shown(e: &Entry, scene: &Scene) -> bool { (e.placement.screens.is_empty() || e.placement.screens.contains(&scene.screen)) && (e.placement.when)(scene) }
    pub fn draw(&self, scene: &Scene, ui: &UiState) -> Vec<(ComponentId, Host, Surface)> { self.draw_for(scene, ui, None) }
    pub(super) fn draw_source(&self, id: ComponentId, scene: &Scene, ui: &UiState, size: (u32, u32)) -> Surface {
        let mut out = Surface::new(size.0, size.1); out.regs = ui.frame.regs;
        if let Some(e) = self.entries.iter().find(|e| e.placement.title == id) { e.component.draw(scene, ui, &mut out); }
        out
    }
    fn draw_for(&self, scene: &Scene, ui: &UiState, window: Option<Option<u64>>) -> Vec<(ComponentId, Host, Surface)> {
        let mut entries: Vec<_> = self.entries.iter().filter(|e| !e.suppressed && Self::shown(e, scene) && e.component.visible(ui) && window.is_none_or(|window| {
            let owner = e.overlay_window.or(match e.host { Host::Window { id, .. } => Some(id), _ => None }); owner == window
        })).collect();
        entries.sort_by_key(|e| (matches!(e.placement.dock, Dock::Float(_)), e.placement.order));
        entries.into_iter().map(|e| {
            let (w, h) = e.host.size(); let mut s = Surface::new(w, h); s.regs = ui.frame.regs;
            if ui.fixed_screen && e.placement.title == "Viewport" { e.component.draw(scene, ui, &mut s); }
            else if self.classic { if let Origin::Classic1990(rect) = e.placement.origin { Picture::new(rect).draw(scene, ui, &mut s); } else { e.component.draw(scene, ui, &mut s); } }
            else { e.component.draw(scene, ui, &mut s); }
            if ui.grip_hover == Some(e.placement.title) && e.placement.undockable {
                let r = grip_rect(e.placement.dock, e.host);
                for y in 0..r.h as i32 { for x in 0..r.w as i32 { s.put(r.x + x, r.y + y, if (x + y) % 2 == 0 { 7 } else { 8 }); } }
            }
            let host = e.overlay_window.map_or(e.host, |id| Host::Window { id, size: e.host.size() });
            (e.placement.title, host, s)
        }).collect()
    }
    pub fn draw_host(&self, scene: &Scene, ui: &UiState, window: Option<u64>) -> Vec<(ComponentId, Rect, Surface)> {
        self.draw_for(scene, ui, Some(window)).into_iter().filter_map(|(id, host, surface)| {
            let e = self.entries.iter().find(|e| e.placement.title == id)?;
            let r = if let Some(parent) = e.overlay_window {
                if window != Some(parent) { return None; }
                let Host::Dock(r) = e.host else { return None; }; r
            } else { match (window, host) {
                (None, Host::Dock(r)) => r,
                (Some(w), Host::Window { id, size }) if w == id => Rect::new(0, 0, size.0, size.1),
                _ => return None,
            } };
            Some((id, r, surface))
        }).collect()
    }
    pub fn compose(&self, scene: &Scene, ui: &UiState, main: (u32, u32)) -> Surface {
        let mut out = Surface::new(main.0, main.1); out.regs = ui.frame.regs;
        let surfaces = self.draw_host(scene, ui, None);
        let is_float = |id| (self.classic && matches!(id, "Commands" | "Flight")) || self.placement(id).is_some_and(|p| matches!(p.dock, Dock::Float(_)));
        for (id, r, s) in &surfaces { if !is_float(*id) { out.draw(s, r.x, r.y); } }
        let covers = |rect| surfaces.iter().any(|(id, r, _)| !is_float(*id) && *r == rect && self.placement(id).is_some_and(|p| p.origin == Origin::Classic1990(rect)));
        if self.classic && (covers(FRAME) || CLASSIC.into_iter().all(covers)) { out.text = ui.frame.text.clone(); picture::flag_hotkeys(&mut out.text, ui);
            if scene.screen == mt1::ui::ScreenKind::Space { picture::flag_flight_keys(&mut out.text, ui); } }
        for (id, r, s) in &surfaces { if is_float(*id) { out.cover(s, r.x, r.y); } }
        out
    }
    pub fn input(&self, id: ComponentId, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !crate::mouse::decision::pointer_allowed(scene, id, e, at) { return Vec::new(); }
        self.entries.iter().find(|x| x.placement.title == id).map_or_else(Vec::new, |x| {
            if self.classic && id == "Viewport" && scene.squad.active {
                let mut classic = ui.clone(); classic.preset = Preset::Classic; classic.camera = viewport::Viewport::oracle_camera(scene); classic.zoom = 1.0; classic.follow = None;
                classic.component_input.size = (192, 112);
                let intents = x.component.input(scene, &classic, e, at);
                *ui.walk.borrow_mut() = classic.walk.into_inner(); return intents;
            }
            if self.classic { if let Origin::Classic1990(rect) = x.placement.origin { return Picture::new(rect).input(scene, ui, e, at); } }
            x.component.input(scene, ui, e, at)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::io::{Event, MouseEvent};
    fn frame() -> Frame {
        let mut f = Frame::default();
        for (i, p) in f.indexed.iter_mut().enumerate() { *p = ((i * 7 + i / 320) % 16) as u8 }
        f.regs[3] = 0x3F; f
    }
    fn scene() -> Scene { Scene::program(ScreenKind::Ground) }
    #[test]
    fn component_is_object_safe_and_picture_composes_the_frame_identically() {
        let boxed: Box<dyn Component> = Box::new(Picture::full());
        let mut layout = Layout::default(); let id = layout.register(boxed, Picture::placement("Picture", FRAME));
        assert_eq!((id, layout.ids()), ("Picture", vec!["Picture"]));
        let ui = UiState { frame: Box::new(frame()), ..UiState::default() };
        layout.arrange((320, 200));
        let out = layout.compose(&scene(), &ui, (320, 200)).into_frame();
        assert_eq!(out.indexed[..], ui.frame.indexed[..]); assert_eq!(out.regs, ui.frame.regs); assert_eq!(out.text, ui.frame.text);
        layout.register(Box::new(Picture::new(Rect::new(0, 0, 320, 54))), Placement { screens: &[ScreenKind::Cg], ..Picture::placement("Strip", Rect::new(0, 0, 320, 54)) });
        layout.register(Box::new(Picture::new(Rect::new(0, 0, 320, 54))), Placement { when: |s| s.squad.encounter, ..Picture::placement("Initiative", Rect::new(0, 0, 320, 54)) });
        assert_eq!(layout.draw(&scene(), &ui).len(), 1);
        let mut cg = scene(); cg.screen = ScreenKind::Cg; cg.squad.encounter = true;
        assert_eq!(layout.draw(&cg, &ui).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), ["Picture", "Strip", "Initiative"]);
    }
    #[test]
    fn host_blind_dock_and_window_give_the_same_pixels() {
        let ui = UiState { frame: Box::new(frame()), ..UiState::default() };
        let strip = Rect::new(0, 0, 320, 54);
        for (rect, size) in [(FRAME, (320, 200)), (strip, (320, 54)), (Rect::new(192, 54, 128, 146), (128, 146)), (strip, (400, 80))] {
            let mut layout = Layout::default(); layout.register(Box::new(Picture::new(rect)), Picture::placement("P", rect));
            layout.set_host("P", Host::Dock(Rect::new(17, 23, size.0, size.1)));
            let dock = layout.draw(&scene(), &ui).remove(0).2;
            layout.set_host("P", Host::Window { id: 7, size });
            let window = layout.draw(&scene(), &ui).remove(0).2;
            assert_eq!(dock, window); assert_eq!((dock.w, dock.h), (size.0 as usize, size.1 as usize));
            assert_eq!(dock.get(0, 0), Some(ui.frame.indexed[rect.y as usize * 320 + rect.x as usize]));
            if size.0 > rect.w { assert_eq!(dock.get(rect.w as i32, 0), Some(0)) }
        }
    }
    #[test]
    fn picture_input_is_raw_in_1990_coordinates_and_surface_clips() {
        let ui = UiState::default(); let s = scene();
        let p = Picture::new(Rect::new(192, 54, 128, 146));
        assert_eq!(p.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 20)), [Intent::Raw(Event::Mouse(MouseEvent { event: mt_engine::io::mouse::LEFT_RELEASE, buttons: 0, col: 202, row: 74 }))]);
        assert_eq!(p.input(&s, &ui, &UiEvent::Up(Button::Right), (0, 0)), [Intent::Raw(Event::Mouse(MouseEvent { event: mt_engine::io::mouse::RIGHT_RELEASE, buttons: 0, col: 192, row: 54 }))]);
        assert_eq!(p.input(&s, &ui, &UiEvent::Key(27), (0, 0)), [Intent::Raw(Event::Key(27))]);
        assert!(p.input(&s, &ui, &UiEvent::Up(Button::Left), (200, 0)).is_empty());
        assert!(p.input(&s, &ui, &UiEvent::Move, (1, 1)).is_empty() && p.input(&s, &ui, &UiEvent::Wheel(1), (1, 1)).is_empty());
        let mut out = Surface::new(4, 3); out.put(-1, 0, 9); out.put(4, 0, 9); out.put(3, 2, 5); assert_eq!((out.get(3, 2), out.get(4, 2), out.indexed.iter().filter(|&&c| c != 0).count()), (Some(5), None, 1));
        out.blit(&[1, 2, 3, 4], 2, 2, Rect::new(-1, -1, 4, 4), 1, 1); assert_eq!(out.indexed, [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 2]);
        assert_eq!(Surface::new(u32::MAX, 1).w, crate::look::MAX_WINDOW_SIDE as usize);
        let big = Surface::new(400, 300).into_frame(); assert_eq!(big.indexed.len(), 64000);
    }
}

pub fn grip_rect(dock: Dock, host: Host) -> Rect {
    let (w, h) = host.size(); let bar = w.min(32); let height = h.min(3);
    if matches!(host, Host::Window { .. }) { return Rect::new(((w - bar) / 2) as i32, 0, bar, height); }
    match dock {
        Dock::Right => Rect::new(0, (h.saturating_sub(32) / 2) as i32, w.min(3), h.min(32)),
        Dock::Left => Rect::new(w.saturating_sub(3) as i32, (h.saturating_sub(32) / 2) as i32, w.min(3), h.min(32)),
        Dock::Top => Rect::new(((w - bar) / 2) as i32, h.saturating_sub(height) as i32, bar, height),
        _ => Rect::new(((w - bar) / 2) as i32, 0, bar, height),
    }
}

#[cfg(test)]
mod f9_tests {
    #[test]
    fn ui_state_is_small_enough_to_copy_on_the_stack() {
        let n = std::mem::size_of::<super::UiState>();
        assert!(n < 4096, "UiState is {n} bytes; box the big field");
    }
}
