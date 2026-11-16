mod hosts;
mod plus;
mod waits;
mod pointer_intake;
#[cfg(target_os = "windows")]
mod plus_button;
use hosts::{ComponentWindow, GripDrag};
use crate::menu::{Action, Menus};
use megatraveller::{look::{clamp_window_size, Filter, Look, Screen, Shape, MAX_WINDOW_SIDE}, panels::{self, Layout, UiState, Host, HostKind, UiEvent, Button, Rect, Dock, Origin, render::Metrics}, pipeline::{self, Frame}};
use mt1::ui::{ComponentId, Intent, Scene};
use mt_engine::io::{audio::Speaker, scan, Input, Key, mouse, Event as IoEvent, MouseEvent, MouseState};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::sync::{Arc, Mutex};
use mt_engine::sim::tick::Clock;
use softbuffer::{Context, Surface};
use std::{cell::RefCell, collections::VecDeque, num::NonZeroU32, path::PathBuf, rc::Rc, time::{Duration, Instant}};
use winit::{application::ApplicationHandler, dpi::{PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent}, event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    platform::{modifier_supplement::KeyEventExtModifierSupplement, pump_events::{EventLoopExtPumpEvents, PumpStatus}},
    window::{Fullscreen, Window, WindowId}};

#[cfg_attr(not(test), derive(Debug))]
enum Event {
    Menu(muda::MenuId),
    #[cfg(test)]
    Test(Box<dyn FnOnce(&mut App) + Send>),
    #[cfg(test)]
    TestWindow(WindowId, WindowEvent),
}
#[cfg(test)]
impl std::fmt::Debug for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Menu(id) => f.debug_tuple("Menu").field(id).finish(), Self::Test(_) => f.write_str("Test"), Self::TestWindow(..) => f.write_str("TestWindow") }
    }
}

fn window_title(detail: Option<&str>) -> String {
    let milestone = option_env!("MT_MILESTONE").unwrap_or("dev");
    let commit = env!("MT_BUILD_HASH");
    let mut title = format!("MegaTraveller 1 Plus - {milestone} ({commit})");
    if let Some(detail) = detail { title.push_str(" - "); title.push_str(detail); }
    title
}

const MAX_INPUT_EVENTS: usize = 256;
const BIOS_KEY_BUFFER: usize = 15;
const MAX_TEXT_BYTES: usize = 1024;

#[derive(Default)]
struct NativeInput { events: VecDeque<IoEvent>, error: Option<String> }
impl NativeInput {
    fn push(&mut self, event: IoEvent) {
        if self.error.is_some() { return }
        if matches!(event, IoEvent::Key(_)) && self.events.iter().filter(|e| matches!(e, IoEvent::Key(_))).count() >= BIOS_KEY_BUFFER { return }
        if self.events.len() >= MAX_INPUT_EVENTS { self.error = Some("native input event budget exceeded".into()); }
        else { self.events.push_back(event); }
    }
    fn pop(&mut self) -> Option<IoEvent> { self.events.pop_front() }
    fn next_key(&mut self) -> Option<Key> {
        while let Some(event) = self.pop() { if let IoEvent::Key(key) = event { return Some(key) } }
        None
    }
    fn key_pending(&mut self) -> bool {
        while self.events.front().is_some_and(|e| !matches!(e, IoEvent::Key(_))) { self.pop(); }
        !self.events.is_empty()
    }
    fn keys(&mut self, keys: VecDeque<Key>, release: bool) {
        for key in keys.into_iter().take(if release { 1 } else { MAX_INPUT_EVENTS }) {
            self.push(if release { IoEvent::KeyUp(key) } else { IoEvent::Key(key) });
        }
    }
}

pub(crate) fn enqueue(queue: &mut VecDeque<Key>, code: Option<KeyCode>, text: Option<&str>) -> Result<(), String> {
    if text.is_some_and(|s| s.len() > MAX_TEXT_BYTES) { return Err("native input text byte budget exceeded".into()) }
    use KeyCode::*;
    let mapped = match code {
        Some(ScrollLock) => return Ok(()),
        Some(Enter | NumpadEnter) => Some((scan::ENTER, Some(13))), Some(Escape) => Some((scan::ESC, Some(27))),
        Some(Backspace) => Some((scan::BACKSPACE, Some(8))), Some(Tab) => Some((scan::TAB, Some(9))),
        Some(ArrowUp) => Some((scan::UP, None)), Some(ArrowDown) => Some((scan::DOWN, None)),
        Some(ArrowLeft) => Some((scan::LEFT, None)), Some(ArrowRight) => Some((scan::RIGHT, None)),
        Some(Home) => Some((scan::HOME, None)), Some(End) => Some((scan::END, None)),
        Some(PageUp) => Some((scan::PGUP, None)), Some(PageDown) => Some((scan::PGDN, None)),
        Some(Insert) => Some((scan::INS, None)), Some(Delete) => Some((scan::DEL, None)),
        Some(F12) => Some((0x58, None)),
        Some(F1) => Some((scan::F1, None)), Some(F2) => Some((0x3c, None)), Some(F3) => Some((0x3d, None)),
        Some(F4) => Some((0x3e, None)), Some(F5) => Some((0x3f, None)), Some(F6) => Some((0x40, None)),
        Some(F7) => Some((0x41, None)), Some(F8) => Some((0x42, None)), Some(F9) => Some((0x43, None)), Some(F10) => Some((scan::F10, None)),
        _ => None,
    };
    if let Some((scan, ascii)) = mapped {
        let duplicate = ascii.is_some_and(|a| text.is_some_and(|s| s.chars().any(|c| c as u32 == u32::from(a))));
        if !duplicate { if let Some(key) = scan::bios_key(scan, ascii) {
            if queue.len() >= MAX_INPUT_EVENTS { return Err("native input key budget exceeded".into()) }
            queue.push_back(key);
        } }
    }
    if let Some(text) = text { for c in text.chars() {
        if let Ok(byte) = u8::try_from(c as u32) { if byte != 0 { if let Some(key) = scan::bios_key(0, Some(byte)) {
            if queue.len() >= MAX_INPUT_EVENTS { return Err("native input key budget exceeded".into()) }
            queue.push_back(key);
        } } }
    } }
    Ok(())
}

fn native_key(code: PhysicalKey, logical: &winit::keyboard::Key, faithful: bool) -> Option<KeyCode> {
    match code {
        PhysicalKey::Code(KeyCode::Numpad7) if !faithful && *logical == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home) => Some(KeyCode::Home),
        PhysicalKey::Code(code) => Some(code), _ => None,
    }
}
pub(crate) fn key_text<'a>(text: Option<&'a str>, unmodified: &'a winit::keyboard::Key, modifiers: ModifiersState, faithful: bool) -> Option<&'a str> {
    text.or_else(|| match unmodified {
        winit::keyboard::Key::Character(s) if !faithful && modifiers.is_empty() && matches!(s.as_str(), "1" | "2" | "3" | "4" | "5") => Some(s.as_str()),
        _ => None,
    })
}
pub(crate) fn key_press(queue: &mut VecDeque<Key>, code: Option<KeyCode>, text: Option<&str>, look: &Look,
    ctrl: bool, faithful: bool, repeat: bool) -> Result<Option<Action>, String> { key_press_in(queue, code, text, look, ctrl, faithful, repeat, false) }
pub(crate) fn key_press_in(queue: &mut VecDeque<Key>, code: Option<KeyCode>, text: Option<&str>, look: &Look,
    ctrl: bool, faithful: bool, repeat: bool, choosing: bool) -> Result<Option<Action>, String> {
    let action = if !faithful && code == Some(KeyCode::F11) {
        Some(if look.fullscreen { Action::Windowed } else { Action::Fullscreen })
    } else if !faithful && ctrl && code == Some(KeyCode::KeyS) {
        Some(Action::Shape(if look.shape == Shape::Classic { Shape::Square } else { Shape::Classic }))
    } else if !faithful && !ctrl && (code == Some(KeyCode::KeyN) || code.is_none() && text.is_some_and(|t| t.eq_ignore_ascii_case("n"))) && !choosing && look.layout.preset == megatraveller::panels::settings::LayoutPreset::Modern {
        Some(Action::Minimap)
    } else { None };
    if action.is_some() { return Ok(if repeat { None } else { action }); }
    enqueue(queue, code, text)?; Ok(None)
}

fn render_size(size: PhysicalSize<u32>) -> PhysicalSize<u32> {
    let [w, h] = clamp_window_size([size.width, size.height]); PhysicalSize::new(w, h)
}

fn picture_to_game(x: f64, y: f64, rect: (usize, usize, usize, usize), look: &Look) -> Option<(i16,i16)> {
    let (left,top,w,h) = rect;
    if w > MAX_WINDOW_SIDE as usize || h > MAX_WINDOW_SIDE as usize { return None }
    mouse::window_to_game(x,y,rect)?;
    let (mut sx,mut sy) = ((x-left as f64) as usize,(y-top as f64) as usize);
    if look.screen == Screen::Crt {
        let (half_w,half_h) = (w as f32 * 0.5,h as f32 * 0.5);
        let (cx,cy) = (sx as f32 + 0.5 - half_w,sy as f32 + 0.5 - half_h);
        let (xn,yn) = (cx / half_w,cy / half_h);
        let (px,py) = (half_w + cx * (1.0 + 0.035 * yn * yn),half_h + cy * (1.0 + 0.035 * xn * xn));
        if px < 0.0 || py < 0.0 || px >= w as f32 || py >= h as f32 { return None }
        sx = px as usize; sy = py as usize;
    }
    let sample = |dst: usize, len: usize, source: usize| {
        if look.filter == Filter::Nearest { return dst * source / len }
        let n = if look.filter == Filter::Sharp { (len/source).max(1) } else { 1 };
        let expanded = source*n;
        let p = ((dst as f64 + 0.5) * expanded as f64 / len as f64 - 0.5).max(0.0);
        let a = p.floor() as usize;
        let b = (a+1).min(expanded-1);
        if ((p-a as f64)*256.0).round() >= 128.0 { b/n } else { a.min(expanded-1)/n }
    };
    Some((sample(sx,w,320) as i16,sample(sy,h,200) as i16))
}

fn game_to_picture(col: i16, row: i16, rect: (usize,usize,usize,usize), look: &Look) -> Option<(f64,f64)> {
    let (left,top,w,h) = rect;
    if w == 0 || h == 0 || !(0..320).contains(&col) || !(0..200).contains(&row) { return None }
    let (tx,ty) = ((col as f64+0.5)/160.0-1.0,(row as f64+0.5)/100.0-1.0);
    let (mut x,mut y) = (tx,ty);
    if look.screen == Screen::Crt {
        for _ in 0..12 { (x,y) = (tx/(1.0+0.035*y*y),ty/(1.0+0.035*x*x)); }
    }
    Some((left as f64+(x+1.0)*w as f64*0.5,top as f64+(y+1.0)*h as f64*0.5))
}

struct App {
    surface: Option<Surface<Rc<Window>, Rc<Window>>>,
    menus: Option<Rc<RefCell<Menus>>>,
    window: Option<Rc<Window>>,
    platform: Platform,
    look: Look,
    path: PathBuf,
    faithful: bool,
    mouse_override: bool,
    plus_assets: Arc<panels::assets::Assets>,
    plus_scene: Option<Scene>,
    plus_evidence: megatraveller::plus::bug::Evidence,
    scene: Option<Scene>, ui: UiState, layout: Layout, intents: VecDeque<Intent>,
    replay: megatraveller::panels::replay::Replay,
    components: Vec<ComponentWindow>, pointer_window: Option<WindowId>, grip_drag: Option<GripDrag>,
    capture_button: Option<Button>, capture_rect: Option<Rect>, last_click: Option<(ComponentId, Button, Instant, (i32, i32))>,
    rgb: Vec<u32>,
    output: Vec<u32>,
    dirty: bool,
    input: NativeInput,
    decision: megatraveller::mouse::decision::Store,
    mouse_pos: Option<(i16, i16)>, mouse_window_pos: Option<(f64, f64)>, mouse_buttons: u16, focus_click: pointer_intake::FocusClick,
    requested_size: Option<[u32; 2]>, created: Option<Instant>, user_resized: bool,

    modifiers: ModifiersState,
    title: String,
    closed: bool,
    tones: Vec<megatraveller::tone::Tone>,
    repeat: u32,
    error: Option<String>,
    geometry_dirty: bool,
    last_fonts: Instant,
    last_edge_pan: Instant,
    fit_initial: bool,
    scenario_request: Option<crate::scenario::Request>,
    scenario_chord: bool,
    accept_drops: bool, dropped: VecDeque<PathBuf>,
    setup_clicks: VecDeque<(i16, i16)>,
}
impl App {
    fn scenario_shortcut(&mut self, code: Option<KeyCode>, state: ElementState, repeat: bool) -> bool {
        if self.faithful || code != Some(KeyCode::KeyL) { return false; }
        if state == ElementState::Released {
            return std::mem::take(&mut self.scenario_chord);
        }
        if !self.modifiers.control_key() || !self.modifiers.shift_key() { return false; }
        self.scenario_chord = true;
        if !repeat {
            if let (Some(window), Some(menu)) = (&self.window, self.menus.as_ref().and_then(|m| m.borrow().scenario_menu.clone())) {
                self.platform.scenario_menu(window, &menu);
            }
        }
        true
    }
    fn save(&mut self) {
        if !self.faithful {
            let mut look = self.look.clone();
            if !self.user_resized && !look.fullscreen { if let Some(r) = self.requested_size { look.window_size = r; } }
            if let Err(error) = look.save(&self.path) { eprintln!("look.toml: {error}"); }
        }
        self.geometry_dirty = false;
    }
    fn change(&mut self, action: Action) {
        if self.faithful { return; }
        if let Action::Plus(tab) = action { self.plus_action(mt1::ui::scene::PlusAction::Open); if let Some(tab) = tab { self.plus_action(mt1::ui::scene::PlusAction::Tab(tab)); } return; }
        if let Action::Scenario(path) = &action {
            if let Some(request) = &self.scenario_request { *request.borrow_mut() = Some(path.clone()); }
            self.open_test_for(path);
            return;
        }
        if action == Action::Quit { self.save(); self.closed = true; return; }
        if let Action::GameKey(scan) = action { self.route_keys(VecDeque::from([mt_engine::io::keys::ext(scan)])); return; }
        if action == Action::Follow {
            if let Some(intent) = self.scene.as_ref().and_then(|scene| action.intent(scene)) { self.apply_intents(vec![intent]); }
            return;
        }
        if let Action::Host(id, kind) = action { if !self.layout.set_kind(&mut self.ui, id, kind) { return; } }
        if action == Action::Minimap {
            let id = megatraveller::panels::components::MINIMAP_ID;
            let kind = if self.ui.host_kind(id) == HostKind::Hidden { HostKind::Docked } else { HostKind::Hidden };
            if !self.layout.set_kind(&mut self.ui, id, kind) { return; }
        }
        if action == Action::DebugInitiative { self.ui.debug_initiative = !self.ui.debug_initiative; }
        if matches!(action, Action::LogLines(_)) { self.ui.log_expanded = false; }
        if action == Action::ResetLayout {
            self.layout.reset_sizes();
            for id in self.layout.ids() { self.layout.set_kind(&mut self.ui, id, HostKind::Docked); }
            self.ui.log_expanded = false; self.ui.log_scroll = 0; self.ui.capture = None; self.grip_drag = None;
            self.ui.debug_initiative = false; self.ui.menu = None; self.ui.menu_host = None; self.ui.dock_preview = None;
        }
        if matches!(action, Action::Host(..) | Action::ResetLayout | Action::Layout(..) | Action::Minimap) {
            self.ui.component_input.drag = None; self.ui.components.pointer = None;
            self.ui.capture = None; self.capture_button = None; self.capture_rect = None;
        }
        let fullscreen = self.look.fullscreen;
        action.apply(&mut self.look);
        mt1::record::set_no_unused_skills(self.look.disable_unused_skills);
        if action == Action::ClassicCombat && self.intents.len() < MAX_INPUT_EVENTS { self.intents.push_back(Intent::ClassicCombat(self.look.classic_combat)); }
        if matches!(action, Action::GrenadeRange | Action::NpcSelectiveAutofire) && self.intents.len() < MAX_INPUT_EVENTS { self.intents.push_back(Intent::BalanceOptions(self.look.balance_options())); }
        self.ui.preset = self.look.layout.preset.into(); self.ui.log_lines = self.look.layout.log_lines;
        if self.look.fullscreen != fullscreen {
            if let Some(window) = &self.window { self.platform.fullscreen(window, self.look.fullscreen); }
        }
        if let Some(menus) = &self.menus { let mut menus = menus.borrow_mut(); menus.sync(&self.look); menus.debug_checked(self.ui.debug_initiative); }
        self.save(); self.redraw_all();
    }
    fn picture(&self) -> (usize, usize, usize, usize) {
        let Some(window) = &self.window else { return (0, 0, 0, 0) };
        if self.faithful { let s = render_size(window.inner_size()); let (k, x, y) = faithful_fit(s.width as usize, s.height as usize); return (x, y, 320 * k, 200 * k) }
        let size = render_size(window.inner_size());
        let r = pipeline::scale::picture_rect(size.width as usize, size.height as usize, &self.look);
        (r.x, r.y, r.width, r.height)
    }
    fn pointer(&self) -> Option<(i16, i16)> {
        if self.faithful { return self.mouse_window_pos.and_then(|(x, y)| mouse::window_to_game(x, y, self.picture())) }
        self.product_pointer().or_else(|| if self.scene.is_none() { self.mouse_window_pos.and_then(|(x,y)| picture_to_game(x, y, self.picture(), &self.look)) } else { None })
    }
    fn render(&mut self) -> Result<(), String> {
        let (Some(window), Some(surface)) = (&self.window, &mut self.surface) else { return Ok(()) };
        let size = render_size(window.inner_size());
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return Ok(()) };
        if self.dirty {
            self.output = if self.faithful {
                faithful_frame(&self.rgb, size.width as usize, size.height as usize)
            } else {
                if let Some(scene) = &self.scene {
                    let mut scene = scene.clone(); scene.squad.encounter |= self.ui.debug_initiative;
                    panels::render::main(&mut self.layout, &scene, &self.ui, &self.look, [size.width, size.height])
                } else { panels::render::main(&mut self.layout, &Scene::program(mt1::ui::ScreenKind::Picture), &self.ui, &self.look, [size.width, size.height]) }

            };
            self.dirty = false;
        }
        if self.output.is_empty() { return Ok(()); }
        surface.resize(w, h).map_err(|e| format!("surface resize: {e}"))?;
        let mut buffer = surface.buffer_mut().map_err(|e| format!("surface buffer: {e}"))?;
        if buffer.len() != self.output.len() { self.dirty = true; window.request_redraw(); return Ok(()); }
        buffer.copy_from_slice(&self.output);
        if let Some(dir) = std::env::var_os("MT_FRAME_DUMP") {
            static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0); let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if let Ok(png) = megatraveller::plus::png::rgb(size.width, size.height, &self.output) { let d = PathBuf::from(dir); let _ = std::fs::write(d.join(format!("frame_{n:04}.png")), &png); let _ = std::fs::write(d.join("latest.png"), &png); }
        }
        window.pre_present_notify();
        buffer.present().map_err(|e| format!("surface present: {e}"))
    }
    fn create(&mut self, events: &ActiveEventLoop) -> Result<(), String> {
        if self.fit_initial { self.look.window_size = megatraveller::look::default_window_size(); }
        if let Some([x, y]) = self.look.window_pos {
            if !events.available_monitors().any(|m| {
                let p = m.position(); let s = m.size();
                i64::from(x) >= i64::from(p.x) && i64::from(y) >= i64::from(p.y)
                    && i64::from(x) < i64::from(p.x) + i64::from(s.width) && i64::from(y) < i64::from(p.y) + i64::from(s.height)
            }) { self.look.window_pos = None; }
        }
        self.look.window_size = clamp_window_size(self.look.window_size);
        if !self.faithful { self.look.window_size = megatraveller::look::lock_aspect(self.look.window_size, None, megatraveller::look::default_window_size()); }
        let mut attrs = Window::default_attributes().with_title(&self.title).with_resizable(true)
            .with_inner_size(PhysicalSize::new(self.look.window_size[0], self.look.window_size[1]))
            .with_max_inner_size(PhysicalSize::new(MAX_WINDOW_SIDE, MAX_WINDOW_SIDE));
        if let Some([x,y]) = self.look.window_pos { attrs = attrs.with_position(PhysicalPosition::new(x,y)); }
        if self.faithful {
            attrs = attrs.with_min_inner_size(PhysicalSize::new(320,200));
        }
        #[cfg(test)]
        { attrs = attrs.with_visible(false); }
        attrs = self.platform.attributes(attrs, &self.title, &self.look, self.menus.as_ref())?;
        let window = Rc::new(events.create_window(attrs).map_err(|e| format!("window: {e}"))?);
        self.platform.attach(&window, self.menus.as_ref())?;
        if !self.faithful { pointer_intake::attach(&window)?; } else { window.set_cursor_visible(false); }
        let _ = window.request_inner_size(PhysicalSize::new(self.look.window_size[0], self.look.window_size[1]));
        self.requested_size = Some(self.look.window_size); self.created = Some(Instant::now()); self.user_resized = false;
        let context = Context::new(window.clone()).map_err(|e| format!("surface context: {e}"))?;
        self.surface = Some(Surface::new(&context, window.clone()).map_err(|e| format!("surface: {e}"))?);
        window.set_ime_allowed(true);
        if self.look.fullscreen { self.platform.fullscreen(&window, true); }
        window.request_redraw(); self.window = Some(window); Ok(())
    }
}
impl ApplicationHandler<Event> for App {
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.window.is_none() { if let Err(error) = self.create(events) { self.error = Some(error); events.exit(); } }
    }
    fn user_event(&mut self, events: &ActiveEventLoop, event: Event) {
        match event {
            Event::Menu(id) => {
                let action = self.menus.as_ref().and_then(|m| m.borrow().action(&id));
                if let Some(action) = action { self.change(action); }
            }
            #[cfg(test)]
            Event::Test(run) => run(self),
            #[cfg(test)]
            Event::TestWindow(id, event) => self.window_event(events, id, event),
        }
        if self.closed { events.exit(); }
    }
    fn window_event(&mut self, events: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if !pointer_intake::accepts(self.faithful, self.look.mouse || self.mouse_override, &event) { return; }
        let swallowed = !self.faithful && self.focus_click.swallow(&event, Instant::now());
        let main = self.window.as_ref().is_some_and(|w| w.id() == id);
        if std::env::var_os("MT_POINTER_TRACE").is_some() && matches!(event, WindowEvent::MouseInput { .. } | WindowEvent::CursorMoved { .. } | WindowEvent::Focused(_) | WindowEvent::CursorEntered { .. } | WindowEvent::CursorLeft { .. }) {
            eprintln!("pointer: {event:?} swallowed={swallowed} main={main} mouse_pos={:?} window_pos={:?} scene={}", self.mouse_pos, self.mouse_window_pos, self.scene.is_some());
        }
        if swallowed { return; }
        if !main { self.component_event(events, id, event); return; }
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => { self.save(); self.components.clear(); self.closed = true; events.exit(); }
            WindowEvent::Resized(size) => {
                let bounded = if self.faithful || self.look.fullscreen || !self.window.as_ref().is_some_and(|w| self.platform.windowed(w)) { render_size(size) }
                    else { let [w, h] = megatraveller::look::lock_aspect([size.width, size.height], Some(self.look.window_size), megatraveller::look::default_window_size()); PhysicalSize::new(w, h) };
                if size != bounded && size.width != 0 && size.height != 0 {
                    if let Some(window) = &self.window {
                        if self.platform.windowed(window) { let _ = window.request_inner_size(bounded); }
                    }
                }
                if size.width != 0 && size.height != 0 && !self.faithful && !self.look.fullscreen
                    && self.window.as_ref().is_some_and(|w| self.platform.windowed(w)) {
                    self.look.window_size = [bounded.width, bounded.height]; self.geometry_dirty = true;
                    if self.created.is_some_and(|t| t.elapsed() > Duration::from_secs(2)) { self.user_resized = true; }
                }
                self.dirty = true;
                if let Some(window) = &self.window { window.request_redraw(); }
            }
            WindowEvent::Moved(position) => {
                if !self.faithful && !self.look.fullscreen {
                    if let Some(position) = self.platform.position(position) { self.look.window_pos = Some(position); self.geometry_dirty = true; }
                }
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::Focused(false) => self.modifiers = ModifiersState::empty(),
            WindowEvent::KeyboardInput { event, is_synthetic, .. } if event.state == ElementState::Pressed && !is_synthetic => {
                let code = native_key(event.physical_key, &event.logical_key, self.faithful);
                if self.plus_key(code, event.text.as_deref(), ElementState::Pressed, event.repeat, false) { return; }
                if self.text_key(code, event.text.as_deref(), ElementState::Pressed) { return; }
                if self.scenario_shortcut(code, ElementState::Pressed, event.repeat) { return; }
                let mut keys = VecDeque::new();
                if !self.faithful && self.modifiers.shift_key() && code == Some(KeyCode::Tab) {
                    keys.push_back(mt_engine::io::keys::ext(scan::TAB)); self.route_keys(keys); return;
                }
                let unmodified = event.key_without_modifiers();
                let choosing = self.scene.as_ref().is_some_and(|s| s.screen == mt1::ui::ScreenKind::Ground && s.squad.strip.contains("N = they go first"));
                let on_ground = self.scene.as_ref().is_some_and(|s| s.screen == mt1::ui::ScreenKind::Ground);
                match key_press_in(&mut keys, code, key_text(event.text_with_all_modifiers().or(event.text.as_deref()), &unmodified, self.modifiers, self.faithful),
                    &self.look, self.modifiers.control_key(), self.faithful || !on_ground && code == Some(KeyCode::KeyN), event.repeat, choosing) {
                    Ok(Some(action)) => self.change(action),
                    Ok(None) if event.physical_key == PhysicalKey::Code(KeyCode::Numpad7) || (self.modifiers.control_key() && code == Some(KeyCode::Home)) => self.input.keys(keys, false),
                    Ok(None) => self.route_keys(keys), Err(e) => self.error = Some(e),
                }
            }
            WindowEvent::KeyboardInput { event, is_synthetic, .. } if event.state == ElementState::Released && !is_synthetic => {
                let code = native_key(event.physical_key, &event.logical_key, self.faithful);
                if self.plus_key(code, None, ElementState::Released, event.repeat, false) { return; }
                if self.text_key(code, None, ElementState::Released) { return; }
                if self.scenario_shortcut(code, ElementState::Released, event.repeat) { return; }
                let mut keys = VecDeque::new();
                match enqueue(&mut keys, code, event.text_with_all_modifiers().or(event.text.as_deref())) {
                    Ok(()) => self.input.keys(keys, true), Err(e) => self.error = Some(e),
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if self.text_commit(&text) { return; }
                let mut keys = VecDeque::new();
                match enqueue(&mut keys, None, Some(&text)) {
                    Ok(()) => self.route_keys(keys), Err(e) => self.error = Some(e),
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.pointer_window = Some(id); self.mouse_window_pos = Some((position.x, position.y));
                if !self.faithful { self.pointer_event(id, UiEvent::Move); }
                self.mouse_pos = self.pointer().or(if self.faithful { self.mouse_pos } else { None });
                if let (Some(w), Some(scene)) = (self.window_by_id(id), self.scene.as_ref()) {
                    use {winit::window::CursorIcon as C, megatraveller::mouse::pointer::Shape as S};
                    w.set_cursor(match megatraveller::mouse::pointer::shape(scene, &self.ui) { None => C::Default, Some(S::Walk) => C::Move, Some(S::Fire) => C::Crosshair, Some(S::Talk) => C::Help, Some(S::Door) => C::Pointer, Some(S::Item) => C::Grab });
                }
            }
            WindowEvent::CursorLeft { .. } => { if !self.faithful { self.mouse_pos = None; } self.ui.components.pointer = None; if self.ui.capture.is_none() && self.grip_drag.is_none() { self.mouse_window_pos = None; self.ui.grip_hover = None; self.redraw_all(); } }
            WindowEvent::MouseInput { state, button, .. } => {
                if self.accept_drops { if state == ElementState::Released && button == MouseButton::Left { if let Some(at) = self.pointer() { if self.setup_clicks.len() < 8 { self.setup_clicks.push_back(at) } } } }
                else if !self.faithful && self.scene.is_none() { self.wait_button(state, button); }
                else if self.faithful {
                    self.mouse_pos = self.pointer().or(self.mouse_pos);
                    if let Some(event) = mouse_button(&mut self.mouse_buttons, self.mouse_pos, state, button) { self.input.push(IoEvent::Mouse(event)) }
                } else { self.product_button(id, state, button); }
            }
            WindowEvent::MouseWheel { delta, .. } if !self.faithful => self.wheel(id, delta),
            WindowEvent::DroppedFile(path) => if self.accept_drops { if self.dropped.len() < 8 { self.dropped.push_back(path) } }
                else { eprintln!("{}", mt1::gamefiles::find_game_dir(&path).map_or(format!("{}: no MegaTraveller 1 folder (T.DAT) in it", path.display()), |d| mt1::gamefiles::verify(&d).text(&d) + "\n(an in-game drop only checks; to install, start with the gamefiles folder incomplete and drop it there)")) },
            WindowEvent::RedrawRequested => if let Err(error) = self.render() { self.error = Some(error); events.exit(); },
            _ => {}
        }
    }
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        #[cfg(feature = "dev")]
        self.bridge_poll();
        if !self.faithful { if let Err(error) = self.sync_hosts(events) { self.error = Some(error); events.exit(); } }
        if !self.faithful && self.last_edge_pan.elapsed() >= Duration::from_millis(50) {
            self.last_edge_pan = Instant::now(); self.edge_pan();
        }
        if !self.faithful { self.replay_tick(); }
        if !self.faithful && megatraveller::panels::party_strip::expire(&self.ui) { self.redraw_all(); }
        if self.last_fonts.elapsed() >= Duration::from_secs(1) {
            if let Some(menus) = &self.menus { if let Err(error) = menus.borrow_mut().refresh_fonts() { eprintln!("font menu: {error}"); } }
            self.last_fonts = Instant::now();
            if self.geometry_dirty { self.save(); }
        }
        if self.closed { events.exit(); }
    }
}

pub struct Win { events: EventLoop<Event>, app: App, pub closed: bool, speaker: Option<Arc<Mutex<Speaker>>>, _stream: Option<cpal::Stream> }
static SOUND_CALLS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SOUND_NONZERO: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static SOUND_DEAD: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SOUND_STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static SOUND_PEAK: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
pub(crate) fn sound_log(line: &str) {
    use std::io::Write;
    let path = std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("sound.log"))).unwrap_or_else(|| "sound.log".into());
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) { let _ = writeln!(f, "{line}"); }
}
fn open_speaker() -> Option<(Arc<Mutex<Speaker>>, cpal::Stream)> {
    use std::sync::atomic::Ordering::Relaxed;
    fn build<T: cpal::SizedSample + cpal::FromSample<f32>>(device: &cpal::Device, config: &cpal::SupportedStreamConfig, speaker: Arc<Mutex<Speaker>>) -> Result<cpal::Stream, cpal::Error> {
        let channels = (config.channels() as usize).max(1);
        let mut mono = Vec::new();
        device.build_output_stream(config.config(), move |out: &mut [T], _: &cpal::OutputCallbackInfo| {
            mono.resize(out.len() / channels, 0.0);
            if let Ok(mut s) = speaker.lock() { s.fill(&mut mono) } else { mono.fill(0.0) }
            SOUND_CALLS.fetch_add(1, Relaxed);
            let (mut nonzero, mut peak) = (0, 0f32); for &v in &mono { if v.abs() > 0.002 { nonzero += 1; } peak = peak.max(v.abs()); }
            if nonzero > 0 { SOUND_NONZERO.fetch_add(nonzero, Relaxed); SOUND_PEAK.fetch_max((peak * 1000.0) as u32, Relaxed); }
            for (frame, &v) in out.chunks_mut(channels).zip(&mono) { frame.fill(T::from_sample(v)) }
        }, |e| { eprintln!("audio: {e}"); sound_log(&format!("stream error: {e}")); SOUND_DEAD.store(true, Relaxed) }, None)
    }
    let open = || -> Result<(Arc<Mutex<Speaker>>, cpal::Stream), String> {
        let device = cpal::default_host().default_output_device().ok_or("no output device")?;
        let config = device.default_output_config().map_err(|e| e.to_string())?;
        let first = !SOUND_STARTED.swap(true, Relaxed);
        if first { let _ = std::fs::remove_file(std::env::current_exe().ok().and_then(|p| p.parent().map(|d| d.join("sound.log"))).unwrap_or_else(|| "sound.log".into())); }
        sound_log(&format!("device: {} | {:?} {} Hz {} ch | menu tone + 1990 effects through cpal {:?}", device.description().map(|d| d.name().to_string()).unwrap_or_else(|e| format!("? {e}")), config.sample_format(), config.sample_rate(), config.channels(), cpal::default_host().id()));
        if first { std::thread::spawn(|| { let t0 = Instant::now(); loop { std::thread::sleep(Duration::from_secs(5)); sound_log(&format!("t={}s callbacks {} nonzero samples {} peak {:.3}", t0.elapsed().as_secs(), SOUND_CALLS.load(Relaxed), SOUND_NONZERO.load(Relaxed), SOUND_PEAK.load(Relaxed) as f32 / 1000.0)); } }); }
        let speaker = Arc::new(Mutex::new(Speaker::new(config.sample_rate())));
        use cpal::SampleFormat as F;
        let (d, c, s) = (&device, &config, speaker.clone());
        let stream = match config.sample_format() {
            F::F32 => build::<f32>(d, c, s), F::F64 => build::<f64>(d, c, s),
            F::I8 => build::<i8>(d, c, s), F::I16 => build::<i16>(d, c, s), F::I32 => build::<i32>(d, c, s), F::I64 => build::<i64>(d, c, s),
            F::U8 => build::<u8>(d, c, s), F::U16 => build::<u16>(d, c, s), F::U32 => build::<u32>(d, c, s), F::U64 => build::<u64>(d, c, s),
            f => return Err(format!("unsupported sample format {f}")),
        }.map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        Ok((speaker, stream))
    };
    open().map_err(|e| { eprintln!("audio: {e}; the game runs silent"); sound_log(&format!("NO SOUND: {e}")) }).ok()
}
impl Win {
    pub fn classic_combat(&self) -> bool { self.app.look.classic_combat }
    pub fn disable_unused_skills(&self) -> bool { self.app.look.disable_unused_skills }
    pub fn autosave(&self) -> bool { self.app.look.autosave != megatraveller::look::MenuSound::Off }
    pub fn balance_options(&self) -> mt1::ground::rules::balance::Options { self.app.look.balance_options() }
    pub fn new(scale: usize, square: bool, faithful: bool, mouse: bool) -> Result<Self, String> {
        let mut win = Self::with_settings(scale, square, faithful, Look::path())?;
        win.app.mouse_override = mouse;
        Ok(win)
    }
    fn with_settings(scale: usize, square: bool, faithful: bool, path: PathBuf) -> Result<Self, String> {
        let fit_initial = !faithful && scale == 0 && !path.exists();
        let mut layout = Layout::default();
        if !faithful { panels::components::register(&mut layout, Arc::default()); }
        let mut look = if faithful { Look::faithful() } else { Look::load_for_layout(&path, &layout) };
        if faithful { let n = if scale == 0 { 3 } else { scale.clamp(1, 32) } as u32; look.window_size = [320 * n, 200 * n]; }
        if !faithful {
            if scale != 0 { look.window_size = [(320 * scale.clamp(1, 32)) as u32, (240 * scale.clamp(1, 32)) as u32]; }
            if square { look.shape = Shape::Square; }
        }
        look.window_size = clamp_window_size(look.window_size);
        Platform::init()?;
        let menus = if faithful { None } else { Some(Rc::new(RefCell::new(Menus::new(&look).map_err(|e| format!("menu: {e}"))?))) };
        let mut builder = EventLoop::<Event>::with_user_event();
        Platform::configure(&mut builder, menus.clone());
        let events = builder.build().map_err(|e| format!("event loop: {e}"))?;
        let proxy = events.create_proxy();
        if !faithful { muda::MenuEvent::set_event_handler(Some(move |event: muda::MenuEvent| { let _ = proxy.send_event(Event::Menu(event.id)); })); }
        let mut ui = UiState::default();
        ui.fixed_screen = !faithful;
        if !faithful {
            look.layout.restore(&layout, &mut ui);
            for id in layout.ids() { if let Some(size) = look.layout.components.get(id).and_then(|c| c.size) { layout.set_size(id, size); } }
            if let Some(menus) = &menus { menus.borrow_mut().components(&layout).map_err(|e| format!("View menu: {e}"))?; menus.borrow_mut().sync(&look); }
        }
        let mut win = Self { events, app: App {
            surface: None, window: None, menus, platform: Platform::default(), look, path, faithful, mouse_override: false,
            plus_assets: Arc::default(), plus_scene: None, plus_evidence: Default::default(),
            scene: None, ui, layout, intents: VecDeque::new(), replay: Default::default(),
            components: Vec::new(), pointer_window: None, grip_drag: None, capture_button: None, capture_rect: None, last_click: None, rgb: vec![0;64000], output: Vec::new(), dirty: true,
            input: NativeInput::default(), decision: Default::default(), mouse_pos: None, mouse_window_pos: None, mouse_buttons: 0, focus_click: Default::default(), requested_size: None, created: None, user_resized: false, modifiers: ModifiersState::empty(), title: window_title(None), closed: false,
            error: None, geometry_dirty: false, last_fonts: Instant::now(), last_edge_pan: Instant::now(),
            fit_initial, scenario_request: None, scenario_chord: false, accept_drops: false, dropped: VecDeque::new(), setup_clicks: VecDeque::new(), tones: Vec::new(), repeat: 0,
        }, closed: false, speaker: None, _stream: None };
        if let Some((speaker, stream)) = open_speaker() { win.speaker = Some(speaker); win._stream = Some(stream); }
        win.events.pump_app_events(Some(Duration::ZERO), &mut win.app);
        if let Some(error) = win.app.error.take().or_else(|| win.app.input.error.take()) { return Err(error); }
        mt_engine::present::textlog::set_enabled(!faithful);
        Ok(win)
    }
    pub fn scenarios(&mut self, dir: &std::path::Path, request: crate::scenario::Request) -> Result<(), String> {
        if self.app.faithful { return Ok(()); }
        if let Some(menus) = &self.app.menus { menus.borrow_mut().scenarios(dir).map_err(|e| format!("Scenario menu: {e}"))?; }
        self.app.scenario_request = Some(request);
        Ok(())
    }
    fn pump_for(&mut self, timeout: Duration) { self.pump_once(timeout); }
    fn pump_once(&mut self, timeout: Duration) {
        self.app.platform.pump(self.app.window.as_ref(), &mut self.app.look, &mut self.app.geometry_dirty, &mut self.app.closed);
        let status = self.events.pump_app_events(Some(timeout), &mut self.app);
        if SOUND_DEAD.swap(false, std::sync::atomic::Ordering::Relaxed) { self._stream = None; if let Some((speaker, stream)) = open_speaker() { self.speaker = Some(speaker); self._stream = Some(stream); } }
        for tone in self.app.tones.drain(..) { if let Some(s) = &self.speaker { if let Ok(mut s) = s.lock() { s.push_tone(megatraveller::tone::HZ, &megatraveller::tone::render(self.app.look.menu_sound, tone)) } } }
        if let Some(error) = self.app.error.take().or_else(|| self.app.input.error.take()) { eprintln!("{error}"); self.app.save(); std::process::exit(1); }
        if self.app.closed || matches!(status, PumpStatus::Exit(_)) {
            self.closed = true; self.app.save(); std::process::exit(0);
        }
    }
    pub fn accept_drops(&mut self, on: bool) { self.app.accept_drops = on; }
    pub fn take_dropped(&mut self) -> Option<PathBuf> { self.pump_for(Duration::from_millis(16)); self.app.dropped.pop_front() }
    pub fn set_title(&mut self, detail: &str) {
        self.app.title = window_title(Some(detail));
        if let Some(window) = &self.app.window { window.set_title(&self.app.title); self.app.platform.title(&self.app.title); }
    }
    pub fn panel_assets(&mut self, files: &mut dyn mt_engine::io::Files) {
        if self.app.faithful { return }
        let assets = Arc::new(megatraveller::panels::assets::Assets::load(files));
        let mut layout = Layout::default(); megatraveller::panels::components::register(&mut layout, assets.clone());
        self.app.plus_assets = assets;
        self.app.look.layout.restore(&layout, &mut self.app.ui);
        for id in layout.ids() { if let Some(size) = self.app.look.layout.components.get(id).and_then(|c| c.size) { layout.set_size(id, size); } }
        self.app.layout = layout;
        self.app.prepare_layout(); self.app.redraw_all();
    }
}
fn faithful_mouse(pos: Option<(i16, i16)>, buttons: u16) -> MouseState { let (col, row) = pos.unwrap_or((160, 100)); MouseState { buttons, col, row } }
fn faithful_fit(w: usize, h: usize) -> (usize, usize, usize) { let k = (w / 320).min(h / 200).max(1); (k, w.saturating_sub(320 * k) / 2, h.saturating_sub(200 * k) / 2) }
fn faithful_frame(rgb: &[u32], w: usize, h: usize) -> Vec<u32> {
    let mut out = pipeline::buffer(w, h, 0); if out.is_empty() || rgb.len() < 64000 { return out }
    let (k, ox, oy) = faithful_fit(w, h);
    for y in oy..h.min(oy + 200 * k) { let row = &rgb[(y - oy) / k * 320..][..320];
        for x in ox..w.min(ox + 320 * k) { out[y * w + x] = row[(x - ox) / k]; } }
    out
}
fn mouse_button(buttons: &mut u16, pos: Option<(i16,i16)>, state: ElementState, button: MouseButton) -> Option<MouseEvent> {
    let (down, release) = match button { MouseButton::Left => (mouse::LEFT_DOWN, mouse::LEFT_RELEASE), MouseButton::Right => (mouse::RIGHT_DOWN, mouse::RIGHT_RELEASE), _ => return None };
    if state == ElementState::Pressed { *buttons |= down; None }
    else { *buttons &= !down; pos.map(|(col,row)| MouseEvent { event: release, buttons: *buttons, col, row }) }
}
impl Input for Win {
    fn wants_decision(&self) -> bool { !self.app.faithful && self.app.decision.wants(&self.app.ui) }
    fn decide(&mut self, d: &mt_engine::play::Decision) {
        self.app.decision.record(&mut self.app.ui, d);
        let follow_up = self.app.ui.walk.borrow_mut().at_decision(d.kind, self.app.scene.as_ref());
        if !follow_up.is_empty() { self.app.apply_intents(follow_up); }
        self.app.redraw_all();
    }
    fn diagnostics(&self) -> bool { !self.app.faithful }
    fn anchor(&mut self, name: &str, image: &[u8]) {
        if !self.app.faithful && name == "window:t.6923" { self.app.plus_evidence.ground_anchor = Some(image.to_vec()); }
    }
    fn closed(&self) -> bool { self.closed }
    fn next_key(&mut self) -> Key {
        if !self.app.faithful { self.pump_for(Duration::ZERO); }
        loop {
            self.app.decision.restore(&mut self.app.ui);
            if let Some(key) = self.app.decision.next_key() { return key; }
            if let Some(key) = self.app.input.next_key() { self.app.decision.hardware(&mut self.app.ui, IoEvent::Key(key)); return key; }
            self.pump_for(Duration::from_millis(16));
        }
    }
    fn key_pending(&mut self) -> bool { self.pump_for(Duration::ZERO); self.app.decision.key_pending() || self.app.input.key_pending() }
    fn arrived(&mut self) -> bool { !self.app.decision.delivered && !self.app.input.events.is_empty() }
    fn flush(&mut self) { self.pump_for(Duration::ZERO); self.app.input.events.clear(); }
    fn poll(&mut self, _clock: &mut dyn Clock) -> IoEvent {
        self.app.decision.restore(&mut self.app.ui);
        self.pump_for(Duration::ZERO);
        if let Some(event) = self.app.decision.pop() { return event }
        if let Some(event) = self.app.input.pop() { self.app.decision.hardware(&mut self.app.ui, event); return event }
        self.pump_for(Duration::from_millis(1)); IoEvent::Idle
    }
    fn mouse(&mut self) -> MouseState {
        if self.app.faithful { return faithful_mouse(self.app.mouse_pos, self.app.mouse_buttons) }
        let component = self.app.ui.components.pointer.and_then(|(id, at)| {
            self.app.layout.placement(id).map(|p| (id, p.origin, at))
        }).or_else(|| self.app.scene.is_none().then(|| self.app.pointer()).flatten()
            .map(|(col, row)| ("Picture", Origin::Classic1990(panels::FRAME), (col as i32, row as i32))));
        megatraveller::mouse::pointer::live_mouse(component)
    }
    fn set_mouse_position(&mut self, col: i16, row: i16) {
        if !self.app.faithful { return }
        if self.app.faithful { return }
        if self.app.scene.is_some() { self.app.reposition(col, row); return; }
        let Some((x,y)) = game_to_picture(col, row, self.app.picture(), &self.app.look) else { return };
        let position = PhysicalPosition::new(x,y);
        self.app.mouse_window_pos = Some((x,y)); self.app.mouse_pos = self.app.pointer();
        if let Some(window) = &self.app.window { let _ = window.set_cursor_position(position); }
    }
    fn present(&mut self, rgb: &[u32]) {
        if rgb.len() != 64000 { return; }
        let frame = (!self.app.faithful).then(|| Frame::from_rgb(rgb));
        let text_changed = frame.as_ref().is_some_and(|f| f.text.visible != self.app.ui.frame.text.visible);
        if self.app.rgb != rgb || text_changed {
            self.app.decision.refresh();
            self.app.rgb.copy_from_slice(rgb);
            if let Some(frame) = frame { if let Some(f) = self.app.replay.frame(frame) { *self.app.ui.frame = f; } }
            self.app.redraw_all();
        }
    }
    fn scene(&mut self, s: &dyn std::any::Any) {
        if self.app.faithful { return }
        if let Some(state) = s.downcast_ref::<mt1::play::diagnostic::State>() {
            panels::plus::map_seen_set(state.map.clone());
            panels::plus::scenario_error_set(state.scenario_error.clone());
            self.app.plus_evidence.state = Some(state.clone()); return;
        }
        if let Some(s) = s.downcast_ref::<Scene>() {
            self.app.decision.refresh();
            if self.app.scene.as_ref().is_some_and(|old| old.screen != s.screen) && !self.app.decision.fresh() { self.app.ui.decision = None; }
            self.app.decision.settle();
            self.app.plus_evidence.scene(s);
            self.app.ui.plus.journal.observe(s);
            self.app.plus_scene = Some(s.clone());
            self.app.replay.ms = self.app.look.npc_playback_ms;
            if let Some(e) = self.app.replay.arrive(s.clone(), &self.app.ui.frame, Instant::now()) { self.app.show_scene(e); }
        }
    }
    fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.app.intents.pop_front().map(|i| Box::new(i) as Box<dyn std::any::Any>) }
    fn audio(&mut self, _n: u16, hz: u32, levels: &[u8], rest: u8) {
        let levels = levels.to_vec();
        if let Some(s) = &self.speaker { if let Ok(mut s) = s.lock() { s.push(hz, levels, rest) } }
    }
}
impl Win {
    pub fn faithful(&self) -> bool { self.app.faithful }
    pub fn take_click(&mut self) -> Option<(i16, i16)> { self.app.setup_clicks.pop_front() }
    pub fn choose_folder(&mut self) -> Option<PathBuf> {
        let chosen = choose_folder(self.app.window.as_deref());
        self.app.setup_clicks.clear(); self.app.redraw_all(); chosen
    }
    pub fn setup_text(&mut self, runs: Vec<mt_engine::present::textlog::Run>) {
        if self.app.faithful { return }
        self.app.ui.frame.text.visible = runs.clone(); self.app.ui.frame.text.runs = runs; self.app.redraw_all();
    }
}
#[cfg(not(target_os = "windows"))]
fn choose_folder(_: Option<&Window>) -> Option<PathBuf> { None }
#[cfg(target_os = "windows")]
fn choose_folder(window: Option<&Window>) -> Option<PathBuf> {
    use windows_sys::Win32::UI::Shell::{SHBrowseForFolderW, SHGetPathFromIDListW, BROWSEINFOW};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let owner = window.and_then(|w| w.window_handle().ok()).and_then(|h| if let RawWindowHandle::Win32(h) = h.as_raw() { Some(h.hwnd.get()) } else { None }).unwrap_or(0);
    let title: Vec<u16> = "Choose the folder of your original MegaTraveller 1 game (the folder that contains the file TRAV.COM)".encode_utf16().chain([0]).collect();
    let mut name = [0u16; 260];
    let info = BROWSEINFOW { hwndOwner: owner as _, pidlRoot: std::ptr::null_mut(), pszDisplayName: name.as_mut_ptr(), lpszTitle: title.as_ptr(),
        ulFlags: 0x1 | 0x40 | 0x200, lpfn: None, lParam: 0, iImage: 0 };
    let list = unsafe { SHBrowseForFolderW(&info) };
    if list.is_null() { return None }
    let mut path = [0u16; 260];
    let ok = unsafe { SHGetPathFromIDListW(list, path.as_mut_ptr()) };
    let len = path.iter().position(|&c| c == 0).unwrap_or(0);
    (ok != 0 && len > 0).then(|| PathBuf::from(String::from_utf16_lossy(&path[..len])))
}
impl Drop for Win { fn drop(&mut self) { self.app.save(); mt_engine::present::textlog::set_enabled(false); } }

#[derive(Default)]
struct Platform {
    #[cfg(target_os = "linux")]
    host: Option<LinuxHost>,
    #[cfg(target_os = "macos")]
    menu_delegate: Option<objc2::rc::Retained<mac_font_menu::Delegate>>,
}
impl Platform {
    fn init() -> Result<(), String> {
        #[cfg(target_os = "linux")]
        {
            gtk::gdk::set_allowed_backends("x11");
            gtk::init().map_err(|e| format!("GTK/X11 (or XWayland) is required for the native menu: {e}"))?;
        }
        Ok(())
    }
    fn configure(builder: &mut winit::event_loop::EventLoopBuilder<Event>, menus: Option<Rc<RefCell<Menus>>>) {
        #[cfg(target_os = "windows")]
        {
            use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, WM_INITMENUPOPUP};
            use winit::platform::windows::EventLoopBuilderExtWindows;
            #[cfg(test)]
            builder.with_any_thread(true);
            builder.with_msg_hook(move |msg| {
                if unsafe { (*(msg as *const MSG)).message } == WM_INITMENUPOPUP {
                    if let Some(menus) = &menus { if let Ok(mut menus) = menus.try_borrow_mut() {
                        if let Err(error) = menus.refresh_fonts() { eprintln!("font menu: {error}"); }
                    } }
                }
                false
            });
        }
        #[cfg(target_os = "linux")]
        { use winit::platform::x11::EventLoopBuilderExtX11; builder.with_x11(); let _ = menus; }
        #[cfg(target_os = "macos")]
        {
            use winit::platform::macos::EventLoopBuilderExtMacOS;
            builder.with_default_menu(false);
            let _ = menus;
        }
    }
    fn attributes(&mut self, attrs: winit::window::WindowAttributes, title: &str, look: &Look, menus: Option<&Rc<RefCell<Menus>>>) -> Result<winit::window::WindowAttributes, String> {
        #[cfg(target_os = "linux")]
        if let Some(menus) = menus {
            use gtk::prelude::*;
            use winit::platform::x11::WindowAttributesExtX11;
            let host = LinuxHost::new(title, look, menus)?;
            let attrs = attrs.with_decorations(false).with_position(PhysicalPosition::new(0, 0))
                .with_embed_parent_window(host.socket.id() as u32);
            self.host = Some(host); return Ok(attrs);
        }
        let _ = (title, look, menus); Ok(attrs)
    }
    fn attach(&mut self, window: &Rc<Window>, menus: Option<&Rc<RefCell<Menus>>>) -> Result<(), String> {
        #[cfg(target_os = "windows")]
        if let Some(menus) = menus {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let RawWindowHandle::Win32(handle) = window.window_handle().map_err(|e| e.to_string())?.as_raw() {
                unsafe { menus.borrow().root.init_for_hwnd(handle.hwnd.get()) }.map_err(|e| format!("menu attachment: {e}"))?;
                plus_button::attach(handle.hwnd.get() as _);
            }
        }
        #[cfg(target_os = "macos")]
        if let Some(menus) = menus {
            let app = muda::Submenu::new("MegaTraveller 1 Plus", true);
            app.append(&muda::MenuItem::with_id("quit", "Quit MegaTraveller 1 Plus", true, None)).map_err(|e| e.to_string())?;
            menus.borrow().root.prepend(&app).map_err(|e| e.to_string())?;
            menus.borrow().root.init_for_nsapp();
            self.menu_delegate = Some(mac_font_menu::attach(menus)?);
        }
        #[cfg(target_os = "linux")]
        if let Some(host) = &self.host {
            use gtk::prelude::*;
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let RawWindowHandle::Xlib(handle) = window.window_handle().map_err(|e| e.to_string())?.as_raw() {
                host.socket.add_id(handle.window as gtk::xlib::Window);
            }
            let child = window.clone();
            host.socket.connect_focus_in_event(move |_, _| { child.focus_window(); gtk::glib::Propagation::Proceed });
        }
        let _ = (window, menus); Ok(())
    }
    fn scenario_menu(&self, window: &Window, menu: &muda::Submenu) {
        use muda::ContextMenu;
        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(h) = handle.as_raw() {
                    unsafe { menu.show_context_menu_for_hwnd(h.hwnd.get(), None); }
                }
            }
        }
        #[cfg(target_os = "macos")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::AppKit(h) = handle.as_raw() {
                    unsafe { menu.show_context_menu_for_nsview(h.ns_view.as_ptr(), None); }
                }
            }
        }
        #[cfg(target_os = "linux")]
        if let Some(host) = &self.host { menu.show_context_menu_for_gtk_window(&host.window, None); }
        let _ = window;
    }
    fn fullscreen(&self, window: &Window, fullscreen: bool) {
        #[cfg(target_os = "linux")]
        if let Some(host) = &self.host { use gtk::prelude::*; if fullscreen { host.window.fullscreen(); } else { host.window.unfullscreen(); } return; }
        window.set_fullscreen(fullscreen.then(|| Fullscreen::Borderless(window.current_monitor())));
    }
    fn windowed(&self, window: &Window) -> bool {
        #[cfg(target_os = "linux")]
        if let Some(host) = &self.host { use gtk::prelude::*; return !host.window.window().is_some_and(|w| w.state().intersects(gtk::gdk::WindowState::FULLSCREEN | gtk::gdk::WindowState::MAXIMIZED)); }
        window.fullscreen().is_none() && !window.is_maximized()
    }
    fn position(&self, position: PhysicalPosition<i32>) -> Option<[i32; 2]> {
        #[cfg(target_os = "linux")]
        if self.host.is_some() { return None; }
        Some([position.x, position.y])
    }
    fn title(&self, title: &str) {
        #[cfg(target_os = "linux")]
        if let Some(host) = &self.host { use gtk::prelude::*; host.window.set_title(title); }
        let _ = title;
    }
    fn pump(&mut self, window: Option<&Rc<Window>>, look: &mut Look, geometry_dirty: &mut bool, closed: &mut bool) {
        #[cfg(target_os = "linux")]
        {
            use gtk::prelude::*;
            while gtk::events_pending() { gtk::main_iteration_do(false); }
            if let Some(host) = &self.host {
                *closed |= host.closed.get();
                if !*closed {
                    if let Some(window) = window {
                        let factor = host.socket.scale_factor().max(1);
                        let size = render_size(PhysicalSize::new((host.socket.allocated_width().max(1) as u32).saturating_mul(factor as u32),
                            (host.socket.allocated_height().max(1) as u32).saturating_mul(factor as u32)));
                        if window.inner_size() != size { let _ = window.request_inner_size(size); }
                    }
                    if !look.fullscreen && !host.window.is_maximized() {
                        let (x,y) = host.window.position(); let pos = Some([x,y]);
                        if look.window_pos != pos { look.window_pos = pos; *geometry_dirty = true; }
                    }
                }
            }
        }
        let _ = (window, look, geometry_dirty, closed);
    }
}
#[cfg(target_os = "linux")]
struct LinuxHost { window: gtk::Window, socket: gtk::Socket, closed: Rc<std::cell::Cell<bool>> }
#[cfg(target_os = "linux")]
impl LinuxHost {
    fn new(title: &str, look: &Look, menus: &Rc<RefCell<Menus>>) -> Result<Self, String> {
        use gtk::prelude::*;
        let window = gtk::Window::new(gtk::WindowType::Toplevel);
        window.set_title(title);
        let vbox = gtk::Box::new(gtk::Orientation::Vertical, 0); window.add(&vbox);
        menus.borrow().root.init_for_gtk_window(&window, Some(&vbox)).map_err(|e| e.to_string())?;
        let socket = gtk::Socket::new(); socket.set_can_focus(true);
        vbox.pack_end(&socket, true, true, 0);
        let factor = window.scale_factor().max(1) as u32;
        window.set_default_size((look.window_size[0] / factor).max(1) as i32, (look.window_size[1] / factor).max(1) as i32 + 25);
        if let Some([x,y]) = look.window_pos { window.move_(x,y); }
        let closed = Rc::new(std::cell::Cell::new(false)); let closing = closed.clone();
        window.connect_delete_event(move |_, _| { closing.set(true); gtk::glib::Propagation::Stop });
        if let Some(bar) = menus.borrow().root.clone().gtk_menubar_for_gtk_window(&window) {
            for child in bar.children() {
                if let Ok(item) = child.downcast::<gtk::MenuItem>() {
                    let menus = Rc::downgrade(menus);
                    item.connect_select(move |_| { if let Some(menus) = menus.upgrade() {
                        if let Err(error) = menus.borrow_mut().refresh_fonts() { eprintln!("font menu: {error}"); }
                    } });
                }
            }
        }
        window.show_all(); socket.realize();
        Ok(Self { window, socket, closed })
    }
}
#[cfg(target_os = "linux")]
impl Drop for LinuxHost { fn drop(&mut self) { use gtk::prelude::*; self.window.close(); } }

#[cfg(target_os = "macos")]
mod mac_font_menu {
    use super::Menus;
    use muda::ContextMenu;
    use objc2::{define_class, msg_send, DefinedClass, MainThreadOnly};
    use objc2::{rc::Retained, runtime::ProtocolObject};
    use objc2_app_kit::{NSMenu, NSMenuDelegate};
    use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol};
    use std::{cell::RefCell, rc::{Rc, Weak}};
    define_class!(
        #[unsafe(super = NSObject)]
        #[thread_kind = MainThreadOnly]
        #[ivars = Weak<RefCell<Menus>>]
        pub struct Delegate;
        unsafe impl NSObjectProtocol for Delegate {}
        unsafe impl NSMenuDelegate for Delegate {
            #[unsafe(method(menuNeedsUpdate:))]
            fn update(&self, _menu: &NSMenu) {
                if let Some(menus) = self.ivars().upgrade() { if let Ok(mut menus) = menus.try_borrow_mut() {
                    if let Err(error) = menus.refresh_fonts() { eprintln!("font menu: {error}"); }
                } }
            }
        }
    );
    pub fn attach(menus: &Rc<RefCell<Menus>>) -> Result<Retained<Delegate>, String> {
        let mtm = MainThreadMarker::new().ok_or("native menu must be created on the main thread")?;
        let allocated = Delegate::alloc(mtm).set_ivars(Rc::downgrade(menus));
        let delegate: Retained<Delegate> = unsafe { msg_send![super(allocated), init] };
        let ptr = menus.borrow().text.ns_menu();
        let menu = unsafe { (ptr as *const NSMenu).as_ref() }.ok_or("missing native Text menu")?;
        menu.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Ok(delegate)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn titles_include_compile_time_identity_and_optional_detail() {
        let milestone = option_env!("MT_MILESTONE").unwrap_or("dev");
        let commit = env!("MT_BUILD_HASH");
        let title = super::window_title(None);
        assert_eq!(title, format!("MegaTraveller 1 Plus - {milestone} ({commit})"));
        for detail in ["Combat log", "Minimap", "TITLE.LY [1/2] 0 - Enter draws"] {
            assert_eq!(super::window_title(Some(detail)), format!("{title} - {detail}"));
        }
    }

    #[test]
    fn ground_mouse_release_and_faithful() {
        use super::*;
        let mut buttons = 0;
        assert_eq!(mouse_button(&mut buttons, Some((20,30)), ElementState::Pressed, MouseButton::Left), None);
        assert_eq!(buttons, 1);
        assert_eq!(mouse_button(&mut buttons, Some((20,30)), ElementState::Pressed, MouseButton::Right), None);
        assert_eq!(buttons, 3);
        assert_eq!(mouse_button(&mut buttons, Some((20,30)), ElementState::Released, MouseButton::Left), Some(MouseEvent { event: 4, buttons: 2, col: 20, row: 30 }));
        assert_eq!(mouse_button(&mut buttons, Some((20,30)), ElementState::Released, MouseButton::Right), Some(MouseEvent { event: 16, buttons: 0, col: 20, row: 30 }));
        mouse_button(&mut buttons, Some((20,30)), ElementState::Pressed, MouseButton::Left);
        assert_eq!(mouse_button(&mut buttons, None, ElementState::Released, MouseButton::Left), None); assert_eq!(buttons, 0);
        assert_eq!(mouse_button(&mut buttons, Some((20,30)), ElementState::Released, MouseButton::Middle), None);
        mouse_button(&mut buttons, Some((20,30)), ElementState::Pressed, MouseButton::Left);
        let at = mouse::window_to_game(3.0 * 250.0 + 1.0, 3.0 * 150.0 + 1.0, (0, 0, 960, 600));
        assert_eq!(faithful_mouse(at, buttons), MouseState { buttons: 1, col: 250, row: 150 });
        assert_eq!(faithful_mouse(None, 0), MouseState { buttons: 0, col: 160, row: 100 });
    }
    #[test]
    fn ground_mouse_unscales_every_look_shape() {
        use super::*;
        use megatraveller::look::{Shape, ScaleMode, Filter, Screen};
        for shape in [Shape::Square, Shape::Classic] {
            for scale_mode in [ScaleMode::Integer, ScaleMode::Fill] {
                for (w,h) in [(320,200),(640,480),(1000,800),(1920,1080),(200,100),(321,777)] {
                    let mut look = Look::default(); look.shape = shape; look.scale = scale_mode;
                    look.filter = Filter::Nearest; look.screen = Screen::None;
                    let r = pipeline::scale::picture_rect(w,h,&look); let rect = (r.x,r.y,r.width,r.height);
                    let source: Vec<_> = (0..64000).collect();
                    let scaled = pipeline::scale::scale(&source,r.width,r.height,&look);
                    for y in (0..r.height).step_by(17) { for x in (0..r.width).step_by(19) {
                        let (col,row) = picture_to_game((r.x+x) as f64+0.9,(r.y+y) as f64+0.1,rect,&look).unwrap();
                        assert_eq!(row as u32*320+col as u32,scaled[y*r.width+x],"{shape:?} {scale_mode:?} {w}x{h}");
                    } }
                    for (x,y) in [(r.x as f64-0.01,r.y as f64),((r.x+r.width) as f64,r.y as f64),
                        (r.x as f64,(r.y+r.height) as f64),(f64::NAN,1.0),(1.0,f64::INFINITY)] {
                        assert_eq!(picture_to_game(x,y,rect,&look),None);
                    }
                }
            }
        }
    }
    #[test]
    fn ground_mouse_crt_matches_visible_f1_and_tube_mask() {
        use megatraveller::look::ScaleMode;
        let mut look = Look { screen:Screen::Crt,filter:Filter::Nearest,..Look::default() };
        let source: Vec<_> = (0..64000).map(|i| if i%320 < 46 { 0xff0000 } else { 0x00ff00 }).collect();
        let scaled = pipeline::scale::scale(&source,640,480,&look);
        let rendered = pipeline::screen::apply(&scaled,640,480,&look);
        let pixel = rendered[24*640+93]; assert!(pixel>>16 > (pixel>>8)&255);
        let pos = picture_to_game(93.0,24.0,(0,0,640,480),&look).unwrap(); assert_eq!(pos,(43,8));
        let regions = [mouse::Hotspot { top:0,left:6,height:50,width:40,id:0x3b },
            mouse::Hotspot { top:0,left:46,height:50,width:40,id:0x3c }];
        assert_eq!(mouse::hit(regions,pos.0,pos.1),Some(0x3b));
        for shape in [Shape::Classic,Shape::Square] { for scale in [ScaleMode::Integer,ScaleMode::Fill] {
            for (w,h) in [(640,480),(997,631),(200,100)] {
                look.shape=shape;look.scale=scale;
                let r=pipeline::scale::picture_rect(w,h,&look);let rect=(r.x,r.y,r.width,r.height);
                let tube=pipeline::screen::apply(&vec![0xffffff;r.width*r.height],r.width,r.height,&look);
                for y in (0..r.height).step_by(7) { for x in (0..r.width).step_by(7) {
                    assert_eq!(picture_to_game((r.x+x) as f64,(r.y+y) as f64,rect,&look).is_some(),tube[y*r.width+x]!=0);
                } }
                let mut buttons=1;
                assert_eq!(mouse_button(&mut buttons,picture_to_game(r.x as f64,r.y as f64,rect,&look),
                    ElementState::Released,MouseButton::Left),None);assert_eq!(buttons,0);
            }
        } }
    }
    #[test]
    fn ground_mouse_reposition_survives_curvature_resize_and_filters() {
        for screen in [Screen::None,Screen::Scanlines,Screen::Crt,Screen::Phosphor] {
            for filter in [Filter::Nearest,Filter::Sharp,Filter::Soft] {
                let look=Look { screen,filter,..Look::default() };
                for rect in [(0,0,640,480),(23,19,960,600),(20,40,960,720)] {
                    for (col,row) in [(0,0),(43,8),(193,46),(192,47),(210,175),(319,199)] {
                        let (x,y)=game_to_picture(col,row,rect,&look).unwrap();
                        assert_eq!(picture_to_game(x,y,rect,&look),Some((col,row)),"{screen:?} {filter:?} {rect:?}");
                    }
                }
            }
        }
    }
    #[test]
    fn native_input_releases_cannot_starve_behind_fresh_keys() {
        let mut input=NativeInput::default();
        let release=IoEvent::Mouse(MouseEvent {event:16,buttons:0,col:43,row:8});
        input.push(IoEvent::Key(0x148)); input.push(IoEvent::KeyUp(0x148)); input.push(release);
        let mut seen=Vec::new();
        for _ in 0..100_000 {
            input.push(IoEvent::Key(65));let event=input.pop().unwrap();
            if seen.len()<3 { seen.push(event); }
        }
        assert_eq!(seen,[IoEvent::Key(0x148),IoEvent::KeyUp(0x148),release]);
        assert_eq!(input.events.len(),3);assert!(input.error.is_none());
    }
    #[test]
    fn native_input_bounds_text_and_all_event_types_and_cg_drain() {
        let mut keys=VecDeque::new();
        assert!(enqueue(&mut keys,None,Some(&"a".repeat(1_000_000))).is_err());assert!(keys.is_empty());
        assert!(enqueue(&mut keys,None,Some(&"a".repeat(MAX_INPUT_EVENTS+1))).is_err());
        assert_eq!(keys.len(),MAX_INPUT_EVENTS);
        let events=[IoEvent::Key(65),IoEvent::KeyUp(65),IoEvent::Mouse(MouseEvent {event:16,buttons:0,col:43,row:8})];
        for event in &events[1..] {
            let mut input=NativeInput::default();
            for _ in 0..MAX_INPUT_EVENTS { input.push(*event); }
            assert!(input.error.is_none());input.push(*event);assert!(input.error.is_some());
            assert_eq!(input.events.len(),MAX_INPUT_EVENTS);
            for _ in 0..MAX_INPUT_EVENTS { assert_eq!(input.pop(),Some(*event)); }
            assert_eq!(input.pop(),None);
        }
        let mut input=NativeInput::default();
        for _ in 0..1000 { input.push(IoEvent::Key(0x14D)); }
        input.push(IoEvent::KeyUp(0x14D));
        assert!(input.error.is_none());assert_eq!(input.events.len(),BIOS_KEY_BUFFER+1);
        for _ in 0..BIOS_KEY_BUFFER { assert_eq!(input.pop(),Some(IoEvent::Key(0x14D))); }
        assert_eq!(input.pop(),Some(IoEvent::KeyUp(0x14D)));assert_eq!(input.pop(),None);
        let mut input=NativeInput::default();
        for event in [events[1],events[2],events[0],IoEvent::Key(13),events[1]] { input.push(event); }
        assert!(input.key_pending());assert_eq!(input.next_key(),Some(65));assert_eq!(input.next_key(),Some(13));
        assert!(!input.key_pending());assert_eq!(input.pop(),None);
    }
    use super::*;
    #[test]
    fn control_character_sequences_and_shortcuts() {
        use KeyCode::*;
        for faithful in [false, true] {
            for (code, text, ctrl, expected) in [
                (Enter, Some("\n"), true, vec![13, 10]),
                (NumpadEnter, Some("\n"), true, vec![13, 10]),
                (Backspace, Some("\x7f"), true, vec![8, 127]),
                (Enter, Some("\r"), false, vec![13]), (Enter, None, false, vec![13]),
                (Backspace, Some("\x08"), false, vec![8]), (Backspace, None, false, vec![8]),
                (Escape, Some("\x1b"), false, vec![27]), (Tab, Some("\t"), false, vec![9]),
                (KeyA, Some("a"), false, vec![97]), (KeyA, Some("A"), false, vec![65]),
                (KeyS, Some("s"), false, vec![115]), (KeyA, Some("\x01"), true, vec![1]),
                (Enter, Some("\r\n"), true, vec![13, 10]), (Enter, Some("\r\r"), false, vec![13, 13]),
                (ArrowUp, Some("x"), false, vec![0x148, 120]), (ScrollLock, Some("x"), false, vec![]),
            ] {
                let mut q = VecDeque::new();
                assert!(key_press(&mut q, Some(code), text, &Look::default(), ctrl, faithful, false).unwrap().is_none());
                assert_eq!(q.into_iter().collect::<Vec<_>>(), expected, "{code:?} {text:?} faithful={faithful}");
            }
            for (code, text, ctrl, expected_action, expected_keys) in [
                (KeyS, Some("\x13"), true, Some(Action::Shape(Shape::Square)), vec![19]),
                (F11, None, false, Some(Action::Fullscreen), vec![]),
            ] {
                let mut look = Look::default(); let mut q = VecDeque::new();
                let action = key_press(&mut q, Some(code), text, &look, ctrl, faithful, false).unwrap();
                assert_eq!(action, if faithful { None } else { expected_action });
                if let Some(action) = action { action.apply(&mut look); }
                assert_eq!(q.iter().copied().collect::<Vec<_>>(), if faithful { expected_keys.clone() } else { vec![] });
                q.clear();
                assert!(key_press(&mut q, Some(code), text, &look, ctrl, faithful, true).unwrap().is_none());
                assert_eq!(q.into_iter().collect::<Vec<_>>(), if faithful { expected_keys } else { vec![] });
                if !faithful {
                    let action = key_press(&mut VecDeque::new(), Some(code), text, &look, ctrl, false, false).unwrap();
                    assert_eq!(action, Some(if code == F11 { Action::Windowed } else { Action::Shape(Shape::Classic) }));
                }
            }
        }
    }
    #[test]
    fn resize_and_surface_share_settings_limit() {
        for (size, expected) in [([16384, 16384], [8192, 8192]), ([u32::MAX, 1], [8192, 1]),
            ([0, 0], [0, 0]), ([0, u32::MAX], [0, 8192]), ([1, 1], [1, 1]), ([997, 631], [997, 631])] {
            assert_eq!(render_size(PhysicalSize::new(size[0], size[1])), PhysicalSize::new(expected[0], expected[1]));
        }
    }
    #[test]
    fn g1b_missing_text_recovers_only_unmodified_product_selection() {
        use winit::keyboard::Key;
        let none = ModifiersState::empty();
        for c in ["1", "2", "3", "4", "5"] {
            let unmodified = Key::Character(c.into());
            assert_eq!(key_text(None, &unmodified, none, false), Some(c));
            assert_eq!(key_text(None, &unmodified, none, true), None);
            assert_eq!(key_text(Some("!"), &unmodified, none, false), Some("!"));
            assert_eq!(key_text(Some(""), &unmodified, none, false), Some(""));
            for modifier in [ModifiersState::SHIFT, ModifiersState::CONTROL, ModifiersState::ALT, ModifiersState::SUPER] {
                assert_eq!(key_text(None, &unmodified, modifier, false), None);
            }
        }
        for c in ["&", "r", "6"] { assert_eq!(key_text(None, &Key::Character(c.into()), none, false), None); }
        assert_eq!(key_text(None, &Key::Dead(Some('^')), none, false), None);
    }
    #[test]
    fn keys_keep_event_order_and_never_duplicate_controls() {
        let mut q = VecDeque::new();
        for (code, text) in [(KeyCode::KeyA, Some("A")), (KeyCode::ArrowDown, None), (KeyCode::Enter, Some("\r")),
            (KeyCode::KeyS, Some("s")), (KeyCode::ArrowLeft, None), (KeyCode::ScrollLock, Some("x")),
            (KeyCode::KeyA, Some("\x01")), (KeyCode::KeyE, Some("é")), (KeyCode::KeyB, Some("𝄞")), (KeyCode::Backspace, Some("\x08"))] {
            enqueue(&mut q, Some(code), text).unwrap();
        }
        assert_eq!(q.into_iter().collect::<Vec<_>>(), [65, 0x150, 13, 115, 0x14b, 1, 233, 8]);
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a Windows desktop; run in its own test process"]
    fn g1b_queued_digit_tap_reaches_native_input() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{PostMessageW, WM_KEYDOWN, WM_KEYUP};
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let path = std::env::temp_dir().join(format!("mt-g1b-key-{}.toml", std::process::id()));
        let mut win = Win::with_settings(0, false, false, path.clone()).unwrap();
        let RawWindowHandle::Win32(handle) = win.app.window.as_ref().unwrap().window_handle().unwrap().as_raw() else { panic!() };
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        for digit in 1..=5 {
            let flags = 1 | ((digit + 1) << 16);
            unsafe {
                assert_ne!(PostMessageW(handle.hwnd.get() as _, WM_KEYDOWN, b'0' as usize + digit, flags as isize), 0);
                assert_ne!(PostMessageW(handle.hwnd.get() as _, WM_KEYUP, b'0' as usize + digit, (flags | (3 << 30)) as isize), 0);
            }
            for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
            assert_eq!(win.app.input.pop(), Some(IoEvent::Key(b'0' as u16 + digit as u16)), "queued digit {digit} disappeared");
            while win.app.input.pop().is_some() {}
        }
        drop(win); let _ = std::fs::remove_file(path);
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a Windows desktop; run separately from the other native tests"]
    fn native_oversized_window_is_clamped_and_drawn() {
        let path = std::env::temp_dir().join(format!("mt-native-cap-{}.toml", std::process::id()));
        std::fs::write(&path, "window_size = [16384, 16384]\n").unwrap();
        let mut win = Win::with_settings(0, false, false, path.clone()).unwrap();
        let window = win.app.window.clone().unwrap();
        win.present(&vec![0xFFFFFF; 64000]);
        for requested in [PhysicalSize::new(16384, 16384), PhysicalSize::new(1, 1), PhysicalSize::new(640, 480)] {
            let previous = win.app.look.window_size;
            let _ = window.request_inner_size(requested);
            for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
            win.app.render().unwrap();
            let size = window.inner_size();
            eprintln!("requested {requested:?}, actual {size:?}, saved {:?}, pixels {}", win.app.look.window_size, win.app.output.len());
            assert!(size.width <= MAX_WINDOW_SIDE && size.height <= MAX_WINDOW_SIDE, "{size:?}");
            if size.width == 0 || size.height == 0 { assert_eq!(win.app.look.window_size, previous); continue; }
            assert_eq!(win.app.output.len(), size.width as usize * size.height as usize);
            assert!(win.app.output.contains(&0xFFFFFF));
            assert_eq!(win.app.look.window_size, [size.width, size.height]);
            win.app.save(); assert_eq!(Look::load(&path).window_size, [size.width, size.height]);
        }
        drop(win); std::fs::remove_file(path).unwrap();
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a Windows desktop; fullscreen transitions can show the test window"]
    fn native_window_menu_resize_and_settings() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMenu, GetMenuItemCount};
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let path = std::env::temp_dir().join(format!("mt-native-{}.toml", std::process::id()));
        let mut win = Win::with_settings(2, false, false, path.clone()).unwrap();
        let request = crate::scenario::Request::default();
        let scenarios = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
        win.scenarios(&scenarios, request.clone()).unwrap();
        let titles: Vec<_> = win.app.menus.as_ref().unwrap().borrow().root.items().iter()
            .filter_map(|item| if let muda::MenuItemKind::Submenu(menu) = item { Some(menu.text()) } else { None }).collect();
        assert_eq!(titles, ["File", "Look", "View", "Scenario", "Help"]);
        win.app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
        assert!(win.app.scenario_shortcut(Some(KeyCode::KeyL), ElementState::Pressed, true));
        assert!(!win.app.scenario_shortcut(Some(KeyCode::Home), ElementState::Pressed, false));
        win.app.modifiers = ModifiersState::empty();
        assert!(win.app.scenario_shortcut(Some(KeyCode::KeyL), ElementState::Released, false));
        assert!(!win.app.scenario_shortcut(Some(KeyCode::KeyL), ElementState::Pressed, false));
        assert!(win.app.input.events.is_empty());
        let selected = scenarios.join("m5_starport_desk_efate.toml");
        win.app.change(Action::Scenario(selected.clone()));
        assert_eq!(request.borrow_mut().take(), Some(selected));
        let window = win.app.window.clone().expect("window was resumed");
        assert_eq!(window.is_visible(), Some(false));
        if let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() {
            assert_eq!(unsafe { GetMenuItemCount(GetMenu(handle.hwnd.get() as _)) }, 5);
        } else { panic!("expected HWND"); }
        win.present(&vec![0xFFFFFF; 64000]);
        for (w, h) in [(1,1), (319,199), (640,480), (997,631)] {
            let _ = window.request_inner_size(PhysicalSize::new(w,h));
            for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
            win.app.render().unwrap();
            let size = window.inner_size();
            assert_eq!(win.app.output.len(), size.width as usize * size.height as usize);
        }
        fn collect(items: Vec<muda::MenuItemKind>, ids: &mut Vec<muda::MenuId>) {
            for item in items { match item {
                muda::MenuItemKind::Submenu(menu) => collect(menu.items(), ids),
                muda::MenuItemKind::Check(item) => ids.push(item.id().clone()), _ => {}
            } }
        }
        let mut ids = Vec::new(); collect(win.app.menus.as_ref().unwrap().borrow().root.items(), &mut ids);
        let proxy = win.events.create_proxy();
        let mut changes = 0;
        for id in ids {
            let action = win.app.menus.as_ref().unwrap().borrow().action(&id).unwrap();
            if matches!(action, Action::Fullscreen | Action::Windowed | Action::Layout(_) | Action::Host(..) | Action::LogLines(_) | Action::ResetLayout | Action::DebugInitiative | Action::Minimap) { continue; }
            let mut expected = win.app.look.clone(); action.apply(&mut expected);
            proxy.send_event(Event::Menu(id)).unwrap(); win.pump_for(Duration::from_millis(5));
            assert_eq!(win.app.look, expected); assert_eq!(Look::load(&path), expected); changes += 1;
        }
        assert_eq!(changes, 27);
        let size = win.app.look.window_size;
        win.app.change(Action::Fullscreen);
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        assert!(window.fullscreen().is_some()); assert!(win.app.look.fullscreen);
        assert_eq!(win.app.look.window_size, size);
        win.app.change(Action::Windowed);
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        assert!(window.fullscreen().is_none()); assert!(!win.app.look.fullscreen);
        assert_eq!(win.app.look.window_size, size); window.set_visible(false);
        drop(win); std::fs::remove_file(path).unwrap();
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a Windows desktop; run separately from the other native tests"]
    fn saved_window_size_survives_a_launch_unchanged() {
        let path = std::env::temp_dir().join(format!("mt-window-size-{}.toml", std::process::id()));
        let mut saved = Look::default(); saved.window_size = [715, 715]; saved.save(&path).unwrap();
        let mut win = Win::with_settings(0, false, false, path.clone()).unwrap();
        for _ in 0..10 { win.pump_for(Duration::from_millis(20)); }
        let want = megatraveller::look::lock_aspect(clamp_window_size([715, 715]), None, megatraveller::look::default_window_size());
        let inner = win.app.window.as_ref().unwrap().inner_size(); win.app.save();
        assert_eq!(Look::load(&path).window_size, want, "the file keeps the requested size (client {inner:?}), so the next launch asks for the same again");
        drop(win); assert_eq!(Look::load(&path).window_size, want, "the close-time save keeps it too");
        let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "requires a Windows desktop; run separately from the other native tests (one winit event loop per process)"]
    fn native_fight_menu_takes_arrows_enter_escape_and_letters() {
        use mt1::ground::orders::{Menu, MenuItem, Op};
        use mt_engine::play::{Decision, Kind};
        let path = std::env::temp_dir().join(format!("mt-fight-menu-{}.toml", std::process::id()));
        let mut win = Win::with_settings(3, false, false, path).unwrap();
        win.panel_assets(&mut mt_engine::io::Dir(std::env::var_os("MT1_GAMEFILES").map(PathBuf::from).unwrap_or_default()));
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.active = true; s.squad.encounter = true; s.combat = true;
        s.squad.menu = Some(Menu { target: mt1::ui::Target::Portrait(0), default: 0, items: vec![
            MenuItem { key: b'S', label: "Select".into(), op: Op::Select(0), grey: None },
            MenuItem { key: b'F', label: "Fire".into(), op: Op::Fire, grey: Some("no Attack left".into()) },
            MenuItem { key: b'R', label: "Reload".into(), op: Op::Reload, grey: None }] });
        win.present(&vec![0; 64000]); win.app.scene = Some(s); win.app.ui.menu = Some((mt1::ui::Target::Portrait(0), (0, 0)));
        win.app.ui.decision = Some(Decision::new(Kind::CombatTurn, "t", "combat.turn"));
        win.app.route_keys(VecDeque::from([mt_engine::io::keys::DOWN, mt_engine::io::keys::DOWN])); assert_eq!(win.app.ui.keyboard.menu_row, Some(2));
        win.app.route_keys(VecDeque::from([mt_engine::io::keys::UP])); assert_eq!(win.app.ui.keyboard.menu_row, Some(1));
        win.app.route_keys(VecDeque::from([13])); assert!(win.app.intents.contains(&Intent::MenuPick(1)), "Enter picks the highlighted row: {:?}", win.app.intents); assert_eq!(win.app.ui.menu, None);
        win.app.intents.clear(); win.app.ui.menu = Some((mt1::ui::Target::Portrait(0), (0, 0)));
        win.app.route_keys(VecDeque::from([b'r' as u16])); assert!(win.app.intents.contains(&Intent::MenuPick(2)), "{:?}", win.app.intents);
        win.app.intents.clear(); win.app.ui.menu = Some((mt1::ui::Target::Portrait(0), (0, 0)));
        win.app.route_keys(VecDeque::from([27])); assert_eq!(win.app.ui.menu, None, "Esc closes");
        assert_eq!(win.app.input.pop(), None, "no key leaked to the 1990 input");
    }
    #[test]
    #[ignore = "requires a desktop; hidden native component windows, run in a separate test process"]
    fn native_component_hosts_roundtrip_grips_and_global_keys() {
        use windows_sys::Win32::UI::WindowsAndMessaging::{GetMenu, PostMessageW, WM_CLOSE};
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let path = std::env::temp_dir().join(format!("mt-component-hosts-{}.toml", std::process::id()));
        let mut saved = Look::default();
        saved.layout.components.insert("Combat log".into(), panels::settings::ComponentSettings { host: HostKind::Window, window_size: Some([600, 180]), window_pos: Some([i32::MAX, i32::MIN]), ..Default::default() });
        saved.layout.components.insert("Minimap".into(), panels::settings::ComponentSettings { host: HostKind::Hidden, size: Some([140, 100]), ..Default::default() });
        saved.save(&path).unwrap();
        let mut win = Win::with_settings(3, false, false, path.clone()).unwrap();
        assert_eq!(win.app.window.as_ref().unwrap().title(), window_title(None));
        win.set_title("viewer status");
        assert_eq!(win.app.window.as_ref().unwrap().title(), window_title(Some("viewer status")));
        win.panel_assets(&mut mt_engine::io::Dir(std::env::var_os("MT1_GAMEFILES").map(PathBuf::from).unwrap_or_default()));
        assert_eq!(win.app.layout.ids().len(), 10);
        assert_eq!(win.app.ui.host_kind("Minimap"), HostKind::Hidden);
        assert_eq!(win.app.layout.placement("Minimap").unwrap().size.preferred, (140, 100));
        win.present(&vec![0xffffff; 64000]); win.scene(&Scene::program(mt1::ui::ScreenKind::Ground));
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        assert_eq!(win.app.components.len(), 1);
        let child = win.app.components[0].window.clone();
        assert_eq!(child.title(), window_title(Some("Combat log")));
        if let RawWindowHandle::Win32(handle) = child.window_handle().unwrap().as_raw() {
            assert!(unsafe { GetMenu(handle.hwnd.get() as _) }.is_null());
        }
        assert_ne!(win.app.look.layout.components["Combat log"].window_pos, Some([i32::MAX, i32::MIN]));
        assert_eq!(win.app.layout.host("Viewport").unwrap().size().1, 146);
        win.app.pointer_window = Some(child.id()); win.app.components[0].cursor = Some((20.0, 20.0));
        win.app.route_keys(VecDeque::from([32])); assert_eq!(win.app.input.pop(), Some(IoEvent::Key(32)));
        win.app.change(Action::Shape(Shape::Square));
        for _ in 0..2 { win.pump_for(Duration::from_millis(5)); }
        win.app.render_component(0).unwrap();
        win.app.save(); assert_eq!(Look::load(&path).layout.components["Combat log"].host, HostKind::Window);
        if let RawWindowHandle::Win32(handle) = child.window_handle().unwrap().as_raw() { unsafe { PostMessageW(handle.hwnd.get() as _, WM_CLOSE, 0, 0); } }
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        assert!(win.app.components.is_empty()); assert_eq!(win.app.ui.host_kind("Combat log"), HostKind::Docked);
        let main = win.app.window.clone().unwrap(); let metrics = win.app.metrics(main.id()).unwrap();
        let host = win.app.layout.host("Party strip").unwrap();
        let Host::Dock(r) = host else { panic!("strip dock") };
        let grip = panels::grip_rect(Dock::Top, host);
        let p = metrics.rect(Rect::new(r.x + grip.x, r.y + grip.y, grip.w, grip.h));
        win.app.mouse_window_pos = Some(((p.x + p.width / 2) as f64, (p.y + p.height / 2) as f64));
        win.app.pointer_window = Some(main.id());
        win.app.product_button(main.id(), ElementState::Pressed, MouseButton::Left); assert!(win.app.grip_drag.is_some());
        win.app.mouse_window_pos = Some((main.inner_size().width as f64 + 30.0, 250.0));
        win.app.pointer_event(main.id(), UiEvent::Move); win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        for _ in 0..2 { win.pump_for(Duration::from_millis(5)); }
        assert_eq!(win.app.ui.host_kind("Party strip"), HostKind::Window); assert_eq!(win.app.components.len(), 1);
        let child = win.app.components[0].window.clone(); let metrics = win.app.metrics(child.id()).unwrap();
        let host = win.app.layout.host("Party strip").unwrap(); let grip = panels::grip_rect(Dock::Top, host); let p = metrics.rect(grip);
        win.app.components[0].cursor = Some(((p.x + p.width / 2) as f64, (p.y + p.height / 2) as f64));
        win.app.product_button(child.id(), ElementState::Pressed, MouseButton::Left); assert!(win.app.grip_drag.is_some());
        let dest = main.inner_position().unwrap(); let origin = child.inner_position().unwrap();
        win.app.components[0].cursor = Some(((dest.x - origin.x + 20) as f64, (dest.y - origin.y + 20) as f64));
        win.app.pointer_event(child.id(), UiEvent::Move); win.app.product_button(child.id(), ElementState::Released, MouseButton::Left);
        for _ in 0..2 { win.pump_for(Duration::from_millis(5)); }
        assert_eq!(win.app.ui.host_kind("Party strip"), HostKind::Docked); assert!(win.app.components.is_empty());
        win.app.change(Action::ResetLayout); assert_eq!(win.app.look.layout, Default::default());
        drop(win); std::fs::remove_file(path).unwrap();
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a desktop; run in a separate test process"]
    fn snapshot_window_first_frame_fits_the_main_stack() {
        use mt1::ui::ScreenKind;
        std::thread::Builder::new().stack_size(1 << 20).spawn(|| {
            let pad = std::hint::black_box([0u8; 160 * 1024]);
            let path = std::env::temp_dir().join(format!("mt-f9-snapshot-{}.toml", std::process::id()));
            let mut win = Win::with_settings(3, false, false, path.clone()).unwrap();
            let mut scene = Scene::program(ScreenKind::Ground); scene.pos = (20, 40); scene.squad.active = true;
            let map = Arc::make_mut(&mut scene.map); map.rows = 80; map.cols = 80; map.cells = vec![0; 80 * 80 * 3];
            let rgb: Vec<_> = (0..64000).map(|i| mt_engine::present::palette::colors(&mt_engine::present::palette::DEFAULT_REGS, &mt_engine::present::palette::ega_dac())[i % 16]).collect();
            win.present(&rgb); win.scene(&scene);
            for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
            win.app.prepare_layout(); win.app.render().unwrap();
            assert!(win.app.scene.is_some());
            drop(win); let _ = std::fs::remove_file(path); std::hint::black_box(pad);
        }).unwrap().join().unwrap();
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a desktop; run in a separate test process"]
    fn native_integrated_components_controls_and_classic() {
        use mt1::ui::{Preset, ScreenKind, Unit};
        let path = std::env::temp_dir().join(format!("mt-integrated-panels-{}.toml", std::process::id()));
        let mut win = Win::with_settings(3, false, false, path.clone()).unwrap();
        win.app.ui.fixed_screen = false;
        win.panel_assets(&mut mt_engine::io::Dir(std::env::var_os("MT1_GAMEFILES").map(PathBuf::from).unwrap_or_default()));
        let mut scene = Scene::program(ScreenKind::Ground); scene.pos = (20, 40);
        let map = Arc::make_mut(&mut scene.map); map.rows = 80; map.cols = 80; map.cells = vec![0; 80 * 80 * 3];
        let rgb: Vec<_> = (0..64000).map(|i| mt_engine::present::palette::colors(&mt_engine::present::palette::DEFAULT_REGS, &mt_engine::present::palette::ega_dac())[i % 16]).collect();
        win.present(&rgb); win.scene(&scene);
        for _ in 0..3 { win.pump_for(Duration::from_millis(5)); }
        let main = win.app.window.clone().unwrap();
        let point = |app: &mut App, id, at| {
            app.prepare_layout();
            let Host::Dock(r) = app.layout.host(id).unwrap() else { panic!("dock") };
            app.mouse_window_pos = app.metrics(main.id()).unwrap().project(r, at, &app.look);
            app.pointer_window = Some(main.id());
        };
        point(&mut win.app, "Viewport", (40, 40));
        assert_eq!(win.app.ui.components.viewport_size, Some(win.app.layout.host("Viewport").unwrap().size()));
        win.app.pointer_event(main.id(), UiEvent::Wheel(-1)); assert_eq!(win.app.ui.zoom, 0.5);
        win.app.ui.follow = None; win.app.ui.camera = panels::viewport::Viewport::oracle_camera(&scene);
        point(&mut win.app, "Viewport", (70, 50));
        let expected = win.app.product_pointer().unwrap();
        win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        assert_eq!(win.app.input.pop(), Some(IoEvent::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: expected.0, row: expected.1 })));
        let free_pointer = win.app.mouse_window_pos;
        win.set_mouse_position(193, 46);
        assert_eq!(win.app.mouse_window_pos, free_pointer, "the sim cannot warp the modern OS pointer");
        assert_eq!(win.mouse(), MouseState { buttons: 0, col: -1, row: -1 }, "the viewport never enters the 1990 live clip");
        win.app.reposition(expected.0, expected.1); assert_eq!(win.app.product_pointer(), Some(expected));
        win.app.ui.zoom = 1.0;
        win.app.ui.follow = Some(Unit::Traveller(0)); point(&mut win.app, "Viewport", (40, 40));
        win.app.product_button(main.id(), ElementState::Pressed, MouseButton::Middle);
        assert_eq!(win.app.ui.component_input.drag.unwrap().camera, (10.5, 20.5));
        point(&mut win.app, "Viewport", (56, 56)); win.app.pointer_event(main.id(), UiEvent::Move);
        assert_eq!(win.app.ui.camera, (9.0, 19.0)); assert_eq!(win.app.ui.follow, None);
        win.app.product_button(main.id(), ElementState::Released, MouseButton::Middle);
        assert!(win.app.ui.component_input.drag.is_none()); assert!(win.app.ui.capture.is_none());
        point(&mut win.app, "Viewport", (0, 40));
        let before = win.app.ui.camera; win.app.pointer_event(main.id(), UiEvent::Move);
        assert_eq!(win.app.ui.camera, before);
        win.app.edge_pan(); assert_eq!(win.app.ui.camera, (before.0, before.1 - 1.0));
        win.app.mouse_window_pos = Some((-10.0, -10.0)); win.app.pointer_event(main.id(), UiEvent::Move);
        assert!(win.app.ui.components.pointer.is_none());
        let before = win.app.ui.camera; win.app.edge_pan(); assert_eq!(win.app.ui.camera, before);
        point(&mut win.app, "Viewport", (40, 40)); win.app.route_keys(VecDeque::from([mt_engine::io::keys::HOME]));
        assert_eq!(win.app.input.pop(), None);
        assert_eq!(win.app.ui.follow, Some(Unit::Traveller(0)));
        win.app.ui.follow = None;
        let action = win.app.menus.as_ref().unwrap().borrow().action(&muda::MenuId::new("follow")).unwrap();
        assert_eq!(action.intent(&scene), Some(Intent::Follow(Some(Unit::Traveller(0)))));
        win.app.change(action);
        assert_eq!(win.app.ui.follow, Some(Unit::Traveller(0)));
        win.app.ui.follow = None; win.app.last_click = None;
        point(&mut win.app, "Party strip", (90, 20));
        win.app.product_button(main.id(), ElementState::Pressed, MouseButton::Left);
        win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        let portrait_click = IoEvent::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: 90, row: 20 });
        assert_eq!(win.app.input.pop(), Some(portrait_click)); assert_eq!(win.app.ui.follow, None);
        let mut record = scene.clone(); record.screen = mt1::ui::ScreenKind::Record; win.scene(&record);
        point(&mut win.app, "Picture", (90, 20));
        win.app.product_button(main.id(), ElementState::Pressed, MouseButton::Left);
        assert_eq!(win.app.ui.follow, Some(Unit::Traveller(2)));
        win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        assert_eq!(win.app.input.pop(), Some(portrait_click)); assert!(win.app.input.pop().is_none());
        win.scene(&scene); assert_eq!(win.app.ui.follow, Some(Unit::Traveller(2)));
        point(&mut win.app, "Combat log", (5, 0)); win.app.product_button(main.id(), ElementState::Pressed, MouseButton::Left);
        point(&mut win.app, "Combat log", (5, -20)); win.app.pointer_event(main.id(), UiEvent::Move);
        assert_eq!(win.app.ui.log_lines, 5);
        win.app.pointer_event(main.id(), UiEvent::Move); assert_eq!(win.app.ui.log_lines, 5);
        win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        assert_eq!(win.app.ui.log_lines, 5); assert_eq!(Look::load(&path).layout.log_lines, 5);
        win.app.ui.log_expanded = true; win.app.change(Action::LogLines(4)); assert!(!win.app.ui.log_expanded);
        win.app.change(Action::Layout(Preset::Classic.into())); win.app.prepare_layout();
        win.app.ui.follow = None;
        win.app.route_keys(VecDeque::from([mt_engine::io::keys::HOME]));
        assert_eq!(win.app.input.pop(), Some(IoEvent::Key(mt_engine::io::keys::HOME)));
        assert_eq!(win.app.ui.follow, None);
        win.app.ui.grip_hover = None;
        win.app.render().unwrap();
        let size = render_size(main.inner_size());
        let mut expected = panels::Surface::new(320, 200);
        panels::Component::draw(&panels::Picture::full(), &scene, &win.app.ui, &mut expected);
        assert_eq!(win.app.output, pipeline::render(&expected.into_frame(), &win.app.look, size.width as usize, size.height as usize));
        point(&mut win.app, "Viewport", (30, 40)); win.app.product_button(main.id(), ElementState::Released, MouseButton::Left);
        assert_eq!(win.app.input.pop(), Some(IoEvent::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: 30, row: 94 })));
        drop(win); std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn home_and_keypad_keep_raw_native_encoding_without_ctrl_follow_binding() {
        use mt_engine::io::keys::HOME;
        let logical = winit::keyboard::Key::Named(winit::keyboard::NamedKey::Home);
        for faithful in [false, true] { for ctrl in [false, true] {
            let mut q = VecDeque::new();
            assert_eq!(key_press(&mut q, Some(KeyCode::Home), None, &Look::default(), ctrl, faithful, false).unwrap(), None);
            assert_eq!(q.pop_front(), Some(HOME)); assert!(q.is_empty());
        }}
        let code = native_key(PhysicalKey::Code(KeyCode::Numpad7), &logical, false);
        let mut q = VecDeque::new(); enqueue(&mut q, code, None).unwrap();
        assert_eq!(q.pop_front(), Some(HOME));
        enqueue(&mut q, code, None).unwrap(); let mut input = NativeInput::default(); input.keys(q, true);
        assert_eq!(input.pop(), Some(IoEvent::KeyUp(HOME)));
        let digit = winit::keyboard::Key::Character("7".into());
        let code = native_key(PhysicalKey::Code(KeyCode::Numpad7), &digit, false);
        let mut q = VecDeque::new(); enqueue(&mut q, code, Some("7")).unwrap();
        assert_eq!(q.pop_front(), Some(b'7' as Key)); assert!(q.is_empty());
        assert_eq!(native_key(PhysicalKey::Code(KeyCode::Numpad7), &logical, true), Some(KeyCode::Numpad7));
    }
    #[cfg(target_os = "windows")]
    #[test]
    #[ignore = "requires a Windows desktop; run separately from the other native test"]
    fn native_faithful_is_raw_and_ignores_settings() {
        use windows_sys::Win32::UI::WindowsAndMessaging::GetMenu;
        use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        let path = std::env::temp_dir().join(format!("mt-faithful-{}.toml", std::process::id()));
        let content = "fullscreen = true\nbold = true\nwindow_size = [700, 500]\nfilter = 'soft'\n";
        std::fs::write(&path, content).unwrap();
        let mut win = Win::with_settings(5, false, true, path.clone()).unwrap();
        let window = win.app.window.clone().unwrap();
        assert_eq!(window.inner_size(), PhysicalSize::new(320,200)); assert!(win.app.menus.is_none());
        assert_eq!(window.title(), window_title(None));
        if let RawWindowHandle::Win32(handle) = window.window_handle().unwrap().as_raw() {
            assert!(unsafe { GetMenu(handle.hwnd.get() as _) }.is_null());
        }
        let rgb: Vec<_> = (0..64000).map(|i| (i * 73) & 0xFFFFFF).collect();
        assert!(!win.app.plus_key(Some(KeyCode::F12), None, ElementState::Pressed, false, false));
        let mut keys = VecDeque::new(); key_press(&mut keys, Some(KeyCode::F12), None, &win.app.look, false, true, false).unwrap();
        assert_eq!(keys, VecDeque::from([0x158])); win.app.route_keys(keys);
        assert_eq!(win.app.input.next_key(), Some(0x158)); assert!(!win.app.ui.plus.open);
        assert!(win.app.layout.ids().is_empty());
        win.app.plus_action(mt1::ui::scene::PlusAction::Open); assert!(!win.app.ui.plus.open);
        win.present(&rgb); win.app.render().unwrap(); assert_eq!(win.app.output, rgb);
        win.app.change(Action::Fullscreen); win.app.change(Action::Bold);
        assert!(!win.app.look.fullscreen && !win.app.look.bold);
        assert_eq!(win.app.look.text_colour, megatraveller::look::TextColour::Original);
        assert_eq!(win.app.look.font, megatraveller::look::FontChoice::Original);
        drop(win); assert_eq!(std::fs::read_to_string(&path).unwrap(), content); std::fs::remove_file(path).unwrap();
    }
}
#[cfg(test)]
mod faithful_frame_tests {
    #[test]
    fn the_faithful_picture_scales_by_whole_pixels() {
        assert_eq!(super::faithful_fit(960, 600), (3, 0, 0)); assert_eq!(super::faithful_fit(1000, 700), (3, 20, 50)); assert_eq!(super::faithful_fit(200, 100), (1, 0, 0));
        let rgb: Vec<u32> = (0..64000).collect(); let out = super::faithful_frame(&rgb, 960, 600);
        assert_eq!((out[0], out[2], out[3], out[3 * 960], out[599 * 960 + 959]), (0, 0, 1, 320, 63999));
        assert_eq!(super::faithful_frame(&rgb, 200, 100)[99 * 200 + 199], 99 * 320 + 199);
    }
}
