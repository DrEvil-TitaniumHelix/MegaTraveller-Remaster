use super::*;
use mt1::ui::scene::PlusAction as A;
use panels::plus::ID as PLUS;
const LAST_SHEET: usize = (panels::plus::LAST_TAB - panels::plus::TEST) as usize;

#[cfg(all(test, feature = "dev", target_os = "windows"))]
mod bridge_tests;

impl Win {
    pub fn clock(&self) -> Box<dyn Clock> { Box::new(mt_engine::sim::tick::Wall::default()) }
}
impl App {
    #[cfg(feature = "dev")]
    fn bridge_action(&mut self, action: megatraveller::plus::bridge::Action) {
        if self.faithful || !self.ui.plus.open || self.ui.plus.tab != 0 { return; }
        if let Some(report) = &self.ui.plus.report {
            self.ui.plus.bridge.action(megatraveller::plus::root(), report, action);
            self.redraw_all();
        }
    }
    #[cfg(feature = "dev")]
    pub(super) fn bridge_poll(&mut self) {
        if self.faithful { return; }
        if let Some(report) = &self.ui.plus.report {
            if self.ui.plus.bridge.poll(megatraveller::plus::root(), report, Instant::now()) && self.ui.plus.open { self.redraw_all(); }
        }
    }
    pub(super) fn plus_action(&mut self, action: A) {
        if self.faithful { return; }
        let action = match action { A::Scroll(n) if panels::plus::main_tab(self.ui.plus.tab) == panels::plus::LOG => A::Scroll(n.saturating_neg()), a => a };
        #[cfg(feature = "dev")]
        if action == A::Submit && self.ui.plus.tab == 0 && self.ui.plus.bridge.editing_note {
            self.bridge_action(megatraveller::plus::bridge::Action::Note); return;
        }
        if action == A::Submit && self.ui.plus.tab == 0 {
            use megatraveller::plus::bug::{self, Capture, Kind};
            let fallback = Scene::program(mt1::ui::ScreenKind::Picture);
            let scene = self.plus_scene.as_ref().unwrap_or(&fallback);
            let play = bug::play_fields(scene, &self.plus_evidence, None);
            let result = bug::write(&megatraveller::plus::root(), Capture { faithful: self.faithful, kind: Kind::from_index(self.ui.plus.kind),
                text: &self.ui.plus.draft, rgb: &self.rgb, scene,
                journal: &self.ui.plus.journal, look: &self.look, play: Some(&play) });
            self.ui.plus.status = match result {
                Ok(Some(report)) => {
                    #[cfg(feature = "dev")]
                    self.ui.plus.bridge.saved(&report);
                    let size = self.window.as_ref().map(|w| render_size(w.inner_size()));
                    if let Some(png) = size.and_then(|s| window_png(s.width, s.height, &self.output)) { let _ = std::fs::write(report.directory.join("window.png"), png); }
                    let status = format!("Saved to {}", report.directory.display().to_string().replace('/', "\\")); self.ui.plus.report = Some(report); self.ui.plus.draft.clear(); status
                }
                Ok(None) => String::new(), Err(e) => format!("Report: {e}")
            };
        }
        if action == A::Submit && self.ui.plus.tab == 2 {
            let result = self.ui.plus.journal.add_note(&megatraveller::plus::root(), &self.ui.plus.note, self.faithful);
            self.ui.plus.status = match result { Ok(()) => { self.ui.plus.note.clear(); "Note saved".into() }, Err(e) => format!("Note: {e}") };
        }
        if panels::plus::main_tab(self.ui.plus.tab) == panels::plus::TEST && self.ui.plus.open {
            match action {
                A::Kind(n) => { self.record_test(n == 1); return; }
                A::Submit => { self.record_test(false); return; }
                A::Backspace => { self.ui.plus.status.pop(); self.redraw_all(); return; }
                _ => {}
            }
        }
        let was_open = self.ui.plus.open;
        self.ui.plus.apply(action);
        if let A::Tab(n) = action { self.ui.plus.tab = n.min(panels::plus::LAST_TAB); }
        self.clamp_plus_scroll();
        if self.ui.plus.open && !was_open {
            self.layout.set_host(PLUS, Host::Window { id: 0, size: (panels::plus::WIDTH, panels::plus::HEIGHT) });
            if let Err(e) = self.ui.plus.journal.load_notes(&megatraveller::plus::root(), false) { self.ui.plus.status = format!("Notes: {e}"); }
        }
        self.redraw_all();
    }
    pub(super) fn plus_key(&mut self, code: Option<KeyCode>, text: Option<&str>, state: ElementState, repeat: bool, from_plus: bool) -> bool {
        if self.faithful { return false; }
        if code == Some(KeyCode::F12) {
            if state == ElementState::Pressed && !repeat { self.plus_action(if self.ui.plus.open { A::Close } else { A::Open }); }
            return true;
        }
        if !from_plus {
            if state == ElementState::Pressed && text == Some("?") {
                if !repeat { if !self.ui.plus.open { self.plus_action(A::Open); } self.plus_action(A::Tab(1)); }
                return true;
            }
            return false;
        }
        if state == ElementState::Released { return true; }
        #[cfg(feature = "dev")]
        if self.ui.plus.tab == 0 && self.ui.plus.report.is_some() {
            use megatraveller::plus::bridge::Action as B;
            let bridge = match code { Some(KeyCode::F6) => Some(B::Send), Some(KeyCode::F7) => Some(B::Note), Some(KeyCode::F8) => Some(B::Accept), Some(KeyCode::F9) => Some(B::Discard), _ => None };
            if let Some(action) = bridge { if !repeat { self.bridge_action(action); } return true; }
        }
        let action = match code {
            Some(KeyCode::Escape) => Some(A::Close),
            Some(KeyCode::Tab) => Some(A::Tab(panels::plus::next_tab(self.ui.plus.tab, self.modifiers.shift_key()))),
            Some(KeyCode::ArrowLeft) if self.ui.plus.tab == 0 => Some(A::Kind((self.ui.plus.kind + 2) % 3)),
            Some(KeyCode::ArrowRight) if self.ui.plus.tab == 0 => Some(A::Kind((self.ui.plus.kind + 1) % 3)),
            Some(KeyCode::ArrowLeft | KeyCode::ArrowRight) if panels::plus::main_tab(self.ui.plus.tab) == panels::plus::CHARACTERS => {
                let mask = self.scene.as_ref().map_or(0, |s| s.party.mask());
                Some(A::Tab(panels::plus::CHARACTERS + panels::plus::next_member(mask, panels::plus::member(self.ui.plus.tab), code == Some(KeyCode::ArrowLeft))))
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter) if panels::plus::main_tab(self.ui.plus.tab) == panels::plus::TEST && self.modifiers.shift_key() => Some(A::Kind(1)),
            Some(KeyCode::Enter | KeyCode::NumpadEnter) => Some(A::Submit),
            Some(KeyCode::Backspace) => Some(A::Backspace),
            Some(KeyCode::ArrowUp) => Some(A::Scroll(-1)), Some(KeyCode::ArrowDown) => Some(A::Scroll(1)),
            Some(KeyCode::PageUp) => Some(A::Scroll(-10)), Some(KeyCode::PageDown) => Some(A::Scroll(10)),
            Some(KeyCode::Home) => Some(A::Scroll(-100_000)), Some(KeyCode::End) => Some(A::Scroll(100_000)),
            _ => None,
        };
        if let Some(action) = action { if !repeat || matches!(action, A::Backspace | A::Scroll(_)) { self.plus_action(action); } }
        else if let Some(text) = text { self.plus_text(text); }
        true
    }
    fn clamp_plus_scroll(&mut self) {
        let fallback = Scene::program(mt1::ui::ScreenKind::Picture);
        let (lines, rows) = panels::plus::scroll_lines(self.scene.as_ref().unwrap_or(&fallback), &self.ui, &self.plus_assets);
        self.ui.plus.scroll = self.ui.plus.scroll.min(panels::plus::max_scroll(lines.len(), rows));
    }
    pub(super) fn plus_text(&mut self, text: &str) -> bool {
        if self.faithful || !self.ui.plus.open { return false; }
        if panels::plus::main_tab(self.ui.plus.tab) == panels::plus::TEST {
            if self.ui.plus.status.starts_with("Recorded ") || self.ui.plus.status.starts_with("Test: ") || self.ui.plus.status.starts_with("No test sheet") { self.ui.plus.status.clear(); }
            for c in text.chars().filter(|c| !c.is_control()).take(1024) { if self.ui.plus.status.len() < 1024 { self.ui.plus.status.push(c); } }
            self.redraw_all(); return true;
        }
        for c in text.chars().take(1024) { self.ui.plus.apply(A::Text(c)); }
        self.redraw_all(); true
    }
    fn record_test(&mut self, fail: bool) {
        let all = panels::plus::sheets(); let i = panels::plus::sheet_index(self.ui.plus.tab);
        self.ui.plus.status = match all.get(i) {
            None => "Test: no sheet to record".into(),
            Some(sheet) => {
                let note = if self.ui.plus.status.starts_with("Recorded ") || self.ui.plus.status.starts_with("Test: ") { String::new() } else { self.ui.plus.status.clone() };
                match panels::plus::record(&megatraveller::plus::root(), sheet, !fail, &note, env!("MT_BUILD_HASH"), megatraveller::plus::now()) { Ok(m) => m, Err(e) => format!("Test: {e}") }
            }
        };
        self.redraw_all();
    }
    pub(super) fn open_test_for(&mut self, path: &std::path::Path) {
        if self.faithful { return; }
        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        let found = panels::plus::sheet_for_scenario(&stem);
        if !self.ui.plus.open { self.plus_action(A::Open); }
        self.plus_action(A::Tab(panels::plus::TEST + found.unwrap_or(0).min(LAST_SHEET) as u8));
        if found.is_none() { self.ui.plus.status = format!("No test sheet names the scenario {stem}"); self.redraw_all(); }
    }
    #[cfg(all(test, feature = "dev"))]
    pub(super) fn plus_event(&mut self, event: UiEvent, at: (i32, i32)) {
        #[cfg(feature = "dev")]
        if self.ui.plus.tab == 0 && self.ui.plus.report.is_some() && matches!(event, UiEvent::Up(Button::Left)) {
            if let Some(action) = self.ui.plus.bridge.click(at) { self.bridge_action(action); return; }
        }
        let fallback = Scene::program(mt1::ui::ScreenKind::Picture);
        let scene = self.scene.as_ref().unwrap_or(&fallback);
        let intents = self.layout.input(PLUS, scene, &self.ui, &event, at);
        self.apply_intents(intents);
    }
    pub(super) fn plus_scene_for(&self, id: ComponentId) -> Option<Scene> {
        self.scene.clone().or_else(|| (id == PLUS).then(|| Scene::program(mt1::ui::ScreenKind::Picture)))
    }
    pub(super) fn is_plus_window(&self, id: WindowId) -> bool { self.components.iter().any(|c| c.window.id() == id && c.component == PLUS) }
    pub(super) fn render_look(&self, id: ComponentId) -> Look {
        if id != PLUS { return self.look.clone(); }
        Look { font: megatraveller::font_menu::plus_font(), bold: false, text_colour: megatraveller::look::TextColour::HighContrast,
            layout: panels::settings::LayoutSettings { preset: panels::settings::LayoutPreset::Modern, ..self.look.layout.clone() }, ..self.look.clone() }
    }
}

#[cfg(test)]
mod look_tests {
    use super::*;
    #[test]
    fn plus_composes_in_a_windows_face_whatever_the_game_look_says() {
        let settings = std::env::temp_dir().join(format!("mt-plus-look-{}.toml", std::process::id()));
        let mut win = Win::with_settings(3, false, false, settings.clone()).unwrap();
        win.app.look.font = megatraveller::look::FontChoice::Original; win.app.look.bold = true;
        win.app.look.text_colour = megatraveller::look::TextColour::Original; win.app.look.layout.preset = panels::settings::LayoutPreset::Classic;
        let plus = win.app.render_look(PLUS);
        assert_eq!(plus.font, megatraveller::font_menu::plus_font()); assert!(matches!(plus.font, megatraveller::look::FontChoice::File(ref f) if f == "segoeui.ttf") || plus.font == megatraveller::look::FontChoice::Builtin(1));
        assert!(plus.high_contrast_text() && !plus.bold);
        assert_eq!((plus.shape, plus.screen, plus.scale, plus.filter), (win.app.look.shape, win.app.look.screen, win.app.look.scale, win.app.look.filter), "geometry stays the game's");
        assert_eq!(win.app.render_look("Viewport"), win.app.look, "only the Plus window has its own Look");
        if std::env::var_os("WINDIR").is_some_and(|w| PathBuf::from(w).join("Fonts/segoeui.ttf").is_file()) {
            assert_eq!(megatraveller::font_menu::selected(&plus.font).unwrap().family, "Segoe UI");
        }
        drop(win); let _ = std::fs::remove_file(settings);
    }
}

#[cfg(all(test, target_os = "windows"))]
mod tests {
    use super::*;
    use windows_sys::Win32::{Foundation::HWND, UI::WindowsAndMessaging::*};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    fn hwnd(w: &Window) -> HWND { let RawWindowHandle::Win32(h) = w.window_handle().unwrap().as_raw() else { panic!() }; h.hwnd.get() as HWND }
    fn settle(win: &mut Win) { for _ in 0..6 { win.pump_for(Duration::from_millis(16)); } }
    fn key(h: HWND, vk: usize, scan: isize) { unsafe { PostMessageW(h, WM_KEYDOWN, vk, (scan << 16) | 1); PostMessageW(h, WM_KEYUP, vk, (scan << 16) | 1 | (1 << 30) | (1 << 31)); } }
    fn click(h: HWND, (x, y): (f64, f64)) {
        let lp = (((y as i32) << 16) | (x as i32 & 0xffff)) as isize;
        unsafe { PostMessageW(h, WM_MOUSEMOVE, 0, lp); PostMessageW(h, WM_LBUTTONDOWN, 1, lp); PostMessageW(h, WM_LBUTTONUP, 0, lp); }
    }
    fn plus_window(app: &App) -> Option<Rc<Window>> { app.components.iter().find(|c| c.component == PLUS).map(|c| c.window.clone()) }
    fn plus_point(app: &App, at: (i32, i32)) -> (f64, f64) {
        let w = plus_window(app).unwrap(); let m = app.metrics(w.id()).unwrap();
        let Some(Host::Window { size, .. }) = app.layout.host(PLUS) else { panic!("Plus window host") };
        m.project(Rect::new(0, 0, size.0, size.1), at, &app.look).unwrap()
    }
    #[test]
    #[ignore = "requires a Windows desktop; run in its own test process"]
    fn g1_question_mark_opens_help_on_every_product_screen_and_escape_closes() {
        let (mut win, settings) = fixture();
        for screen in [mt1::ui::ScreenKind::Picture, mt1::ui::ScreenKind::Startup, mt1::ui::ScreenKind::Cg,
            mt1::ui::ScreenKind::Ground, mt1::ui::ScreenKind::Intro, mt1::ui::ScreenKind::Record, mt1::ui::ScreenKind::Space] {
            for kind in [mt_engine::play::Kind::Menu, mt_engine::play::Kind::Ack] {
                win.app.scene = Some(Scene::program(screen));
                win.app.ui.decision = Some(mt_engine::play::Decision::new(kind, "t", "help"));
                assert!(win.app.plus_key(Some(KeyCode::Slash), Some("?"), ElementState::Pressed, false, false), "{screen:?} {kind:?}");
                assert!(win.app.ui.plus.open); assert_eq!(win.app.ui.plus.tab, 1);
                assert!(!win.app.plus_key(Some(KeyCode::Escape), None, ElementState::Pressed, false, false));
                assert!(win.app.ui.plus.open);
                assert!(win.app.plus_key(Some(KeyCode::Escape), None, ElementState::Pressed, false, true));
                assert!(!win.app.ui.plus.open);
                assert!(win.app.input.events.is_empty());
            }
        }
        win.app.faithful = true;
        assert!(!win.app.plus_key(Some(KeyCode::Slash), Some("?"), ElementState::Pressed, false, false));
        drop(win); let _ = std::fs::remove_file(settings);
    }
    fn fixture() -> (Win, PathBuf) {
        let settings = std::env::temp_dir().join(format!("mt-plus-fix1-{}.toml", std::process::id()));
        let mut win = Win::with_settings(3, false, false, settings.clone()).unwrap();
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        win.panel_assets(&mut mt_engine::io::Dir(dir.clone()));
        let scenario = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
        let prepared = crate::scenario::load(&scenario, &dir).unwrap();
        let scene = Scene::capture(&prepared.game, mt1::ui::ScreenKind::Ground, None);
        win.app.scene = Some(scene.clone()); win.app.plus_scene = Some(scene.clone()); win.app.ui.plus.journal.observe(&scene);
        win.app.rgb = prepared.game.fb.rgb(0, &prepared.game.dac);
        *win.app.ui.frame = Frame::from_rgb(&win.app.rgb);
        win.app.prepare_layout();
        (win, settings)
    }
    fn point(app: &mut App, component: ComponentId, at: (i32, i32)) -> WindowId {
        app.prepare_layout();
        let main = app.window.as_ref().unwrap().id();
        let Host::Dock(rect) = app.layout.host(component).unwrap() else { panic!("docked component"); };
        app.pointer_window = Some(main);
        app.mouse_window_pos = app.metrics(main).unwrap().project(rect, at, &app.look);
        main
    }
    #[test]
    #[ignore = "requires a Windows desktop; run in its own test process"]
    fn native_plus_leaves_game_mouse_and_drags_untouched() {
        let (mut win, settings) = fixture();
        win.app.look.layout.preset = panels::settings::LayoutPreset::Classic;
        for button in [MouseButton::Left, MouseButton::Right] {
            let mask = if button == MouseButton::Left { mouse::LEFT_DOWN } else { mouse::RIGHT_DOWN };
            let release = if button == MouseButton::Left { mouse::LEFT_RELEASE } else { mouse::RIGHT_RELEASE };
            let main = point(&mut win.app, "Viewport", (40, 40));
            win.app.product_button(main, ElementState::Pressed, button);
            assert_eq!(win.app.mouse_buttons, mask);
            let pos = win.app.product_pointer().unwrap();
            win.app.plus_action(A::Open);
            assert_eq!(win.app.mouse_buttons, mask, "held game button survives F12");
            win.app.product_button(main, ElementState::Released, button);
            assert_eq!(win.app.input.pop(), Some(IoEvent::Mouse(MouseEvent { event: release, buttons: 0, col: pos.0, row: pos.1 })), "the release reaches the game with Plus open");
            win.app.plus_action(A::Close);
            assert!(win.app.input.events.is_empty());
        }
        win.app.look.layout.preset = panels::settings::LayoutPreset::Modern;
        for (id, button, start, end) in [("Viewport", MouseButton::Middle, (40, 40), (56, 56)), ("Combat log", MouseButton::Left, (40, 1), (40, 25))] {
            let main = point(&mut win.app, id, start);
            win.app.product_button(main, ElementState::Pressed, button);
            assert!(win.app.ui.component_input.drag.is_some(), "{id} drag started");
            win.app.plus_action(A::Open);
            assert!(win.app.ui.component_input.drag.is_some(), "{id} drag continues with Plus open");
            point(&mut win.app, id, end); win.app.pointer_event(main, UiEvent::Move);
            assert!(win.app.ui.component_input.drag.is_some() && win.app.ui.capture == Some(id), "{id} keeps dragging with Plus open");
            win.app.product_button(main, ElementState::Released, button);
            win.app.plus_action(A::Close);
            assert!(win.app.ui.component_input.drag.is_none());
        }
        drop(win); std::fs::remove_file(settings).unwrap();
    }
    #[test]
    #[ignore = "requires a Windows desktop; run in its own test process"]
    fn native_plus_game_boundary_returns_and_clock_runs() {
        let (mut win, settings) = fixture();
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").unwrap());
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
        let mut game = crate::scenario::load(&path, &dir).unwrap().game;
        game.clock = win.clock(); game.clock.set(100);
        win.app.plus_key(Some(KeyCode::F12), None, ElementState::Pressed, false, false);
        assert!(win.app.ui.plus.open);
        win.app.input.push(IoEvent::Key(mt_engine::io::keys::UP)); win.app.input.push(IoEvent::Key(mt_engine::io::keys::UP));
        let proxy = win.events.create_proxy();
        game.input = Box::new(win);
        let start = Instant::now();
        assert!(game.input.key_pending());
        assert_eq!(game.input.next_key(), mt_engine::io::keys::UP);
        assert_eq!(game.input.poll(game.clock.as_mut()), IoEvent::Key(mt_engine::io::keys::UP));
        assert_eq!(game.input.poll(game.clock.as_mut()), IoEvent::Idle);
        proxy.send_event(Event::Test(Box::new(|app| { assert!(app.ui.plus.open); app.input.push(IoEvent::Key(mt_engine::io::keys::DOWN)); }))).unwrap();
        assert_eq!(game.input.next_key(), mt_engine::io::keys::DOWN);
        assert!(start.elapsed() < Duration::from_millis(500), "no Input call waits on Plus");
        let t = game.clock.now(); std::thread::sleep(Duration::from_millis(120));
        assert!(game.clock.now().wrapping_sub(t) >= 1, "game clock runs with Plus open");
        drop(game); std::fs::remove_file(settings).unwrap();
    }
    #[test]
    #[ignore = "requires a Windows desktop; hidden window, run in its own test process"]
    fn native_plus_keys_by_window_tabs_and_main_untouched() {
        let (mut win, settings) = fixture();
        assert!(win.app.plus_key(Some(KeyCode::F12), None, ElementState::Pressed, false, false));
        assert!(win.app.ui.plus.open);
        assert!(win.app.plus_key(Some(KeyCode::F12), None, ElementState::Pressed, true, false)); assert!(win.app.ui.plus.open, "a repeat never toggles");
        assert!(win.app.plus_key(Some(KeyCode::F12), None, ElementState::Released, false, false)); assert!(win.app.ui.plus.open);
        assert!(!win.app.plus_key(Some(KeyCode::KeyA), Some("a"), ElementState::Pressed, false, false));
        assert!(!win.app.plus_key(Some(KeyCode::ArrowRight), None, ElementState::Pressed, false, false));
        assert_eq!(win.app.ui.plus.draft, ""); assert_eq!(win.app.ui.plus.kind, 0);
        assert!(win.app.plus_key(Some(KeyCode::KeyA), Some("a"), ElementState::Pressed, false, true));
        assert!(win.app.plus_key(Some(KeyCode::KeyA), None, ElementState::Released, false, true));
        assert_eq!(win.app.ui.plus.draft, "a"); assert!(win.app.input.events.is_empty());
        win.app.plus_key(Some(KeyCode::ArrowRight), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.kind, 1);
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::JOURNAL);
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::CHARACTERS, "the state's clamp is overridden for the new tabs");
        win.app.plus_key(Some(KeyCode::ArrowRight), None, ElementState::Pressed, false, true); assert!(win.app.ui.plus.tab > panels::plus::CHARACTERS, "Right = the next Traveller");
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::MAP);
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::TEST);
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::HELP);
        win.app.plus_key(Some(KeyCode::Tab), None, ElementState::Pressed, false, true); assert_eq!(win.app.ui.plus.tab, panels::plus::BUG);
        win.app.render().unwrap();
        let size = render_size(win.app.window.as_ref().unwrap().inner_size());
        let scene = win.app.scene.clone().unwrap(); let mut ui = win.app.ui.clone(); ui.plus.open = false;
        let mut layout = Layout::default(); panels::components::register(&mut layout, win.app.plus_assets.clone());
        for id in layout.ids() { if let Some(size) = win.app.look.layout.components.get(id).and_then(|c| c.size) { layout.set_size(id, size); } }
        let expected = panels::render::main(&mut layout, &scene, &ui, &win.app.look, [size.width, size.height]);
        assert_eq!(win.app.output, expected, "the game window is the same with Plus open and closed");
        let main = point(&mut win.app, "Viewport", (40, 40));
        win.app.product_button(main, ElementState::Pressed, MouseButton::Left);
        win.app.product_button(main, ElementState::Released, MouseButton::Left);
        assert!(matches!(win.app.input.pop(), Some(IoEvent::Mouse(_))) || !win.app.intents.is_empty(), "a game click with Plus open");
        assert!(!win.app.plus_key(Some(KeyCode::Escape), None, ElementState::Pressed, false, false)); assert!(win.app.ui.plus.open);
        assert!(win.app.plus_key(Some(KeyCode::Escape), None, ElementState::Pressed, false, true)); assert!(!win.app.ui.plus.open);
        assert!(!win.app.plus_key(Some(KeyCode::ArrowRight), None, ElementState::Pressed, false, false));
        assert!(!win.app.output.is_empty());
        let menus = win.app.menus.clone().unwrap();
        let titles: Vec<String> = menus.borrow().root.items().iter().filter_map(|i| i.as_submenu().map(|s| s.text())).collect();
        assert_eq!(titles.last().map(String::as_str), Some(crate::menu::PLUS_TITLE), "{titles:?}");
        assert!(titles.iter().position(|t| t == "Help").unwrap() < titles.len() - 1);
        for (id, tab) in [("plus-open", None), ("plus-bug", Some(0)), ("plus-journal", Some(2)), ("plus-log", Some(panels::plus::LOG)), ("plus-test", Some(panels::plus::TEST))] {
            let action = menus.borrow().action(&muda::MenuId::new(id)).expect(id);
            assert_eq!(action, Action::Plus(tab));
            win.app.change(action); assert!(win.app.ui.plus.open, "{id} opens Plus");
            if let Some(tab) = tab { assert_eq!(win.app.ui.plus.tab, tab); }
            win.app.plus_action(A::Close);
        }
        drop(win); std::fs::remove_file(settings).unwrap();
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; hidden windows, run in its own test process"]
    fn native_plus_window_through_real_messages() {
        let (mut win, settings) = fixture();
        let main = win.app.window.clone().unwrap(); let mh = hwnd(&main);
        key(mh, 0x7B, 0x58); settle(&mut win);
        assert!(win.app.ui.plus.open, "F12 opens Plus");
        let plus = plus_window(&win.app).expect("the Plus window exists"); let ph = hwnd(&plus);
        assert_ne!(ph, mh); assert!(unsafe { GetParent(ph) }.is_null(), "not a child of the game window");
        assert_eq!(unsafe { GetWindowLongPtrW(ph, GWL_STYLE) } & WS_CHILD as isize, 0);
        let mut clock = win.clock(); let t = clock.now(); std::thread::sleep(Duration::from_millis(120));
        assert!(clock.now().wrapping_sub(t) >= 1, "the game is not paused");
        win.app.look.layout.preset = panels::settings::LayoutPreset::Classic; win.app.prepare_layout();
        let m = win.app.metrics(main.id()).unwrap();
        let Host::Dock(r) = win.app.layout.host("Viewport").unwrap() else { panic!("Viewport dock") };
        click(mh, m.project(r, (40, 40), &win.app.look).unwrap()); settle(&mut win);
        assert!(matches!(win.app.input.pop(), Some(IoEvent::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, .. }))), "the game took the click");
        assert!(win.app.ui.plus.open);
        click(ph, plus_point(&win.app, (100, 100)));
        for (vk, scan, c) in [(0x44usize, 0x20isize, 'd'), (0x4F, 0x18, 'o'), (0x4F, 0x18, 'o'), (0x52, 0x13, 'r')] {
            unsafe { PostMessageW(ph, WM_KEYDOWN, vk, (scan << 16) | 1); PostMessageW(ph, WM_CHAR, c as usize, (scan << 16) | 1); PostMessageW(ph, WM_KEYUP, vk, (scan << 16) | 1 | (1 << 30) | (1 << 31)); }
        }
        settle(&mut win);
        assert_eq!(win.app.ui.plus.draft, "door", "typed characters reach the Plus box");
        click(ph, plus_point(&win.app, (100, 36))); settle(&mut win);
        assert_eq!(win.app.ui.plus.kind, 1, "QoL request selected by click");
        click(ph, plus_point(&win.app, (20, 178))); settle(&mut win);
        let report = win.app.ui.plus.report.clone().expect("Save report wrote a report");
        assert!(report.directory.join("bundle.json").exists());
        assert!(win.app.ui.plus.status.contains(&report.directory.display().to_string()), "status says where: {}", win.app.ui.plus.status);
        assert!(win.app.ui.plus.draft.is_empty());
        let out = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/plus-preview"); std::fs::create_dir_all(&out).unwrap();
        win.app.ui.plus.draft = "Door at the top of the stairs sticks.".into(); win.app.ui.plus.status.clear();
        if let Some(s) = &win.app.scene { let (rows, cols) = (s.map.rows, s.map.cols); let mut seen = vec![0u8; (rows as usize * cols as usize).div_ceil(8)];
            let (r0, c0) = ((s.view.0 >> 1).max(0) as usize, (s.view.1 >> 1).max(0) as usize);
            for r in r0..(r0 + 7).min(rows as usize) { for c in c0..(c0 + 12).min(cols as usize) { let i = r * cols as usize + c; seen[i / 8] |= 1 << (i % 8); } }
            panels::plus::map_seen_set(Some(mt1::play::diagnostic::Map { area: [0, 0, 0], rows, cols, tiles: Vec::new(), party: (s.pos.0 >> 1, s.pos.1 >> 1), seen })); }
        for (tab, name) in [(0u8, "bug"), (1, "help"), (2, "journal"), (panels::plus::CHARACTERS, "characters"), (panels::plus::MAP, "map"), (panels::plus::LOG, "log"), (panels::plus::TEST + panels::plus::sheet_for_scenario("efate_quiet").unwrap_or(0) as u8, "test")] {
            win.app.plus_action(A::Tab(tab));
            let i = win.app.components.iter().position(|c| c.component == PLUS).unwrap();
            let (size, pixels) = win.app.component_pixels(i).unwrap().expect("Plus pixels");
            std::fs::write(out.join(format!("{name}.png")), megatraveller::plus::png::rgb(size.width, size.height, &pixels).unwrap()).unwrap();
        }
        key(ph, 0x1B, 0x01); settle(&mut win);
        assert!(!win.app.ui.plus.open && plus_window(&win.app).is_none(), "Esc closes the Plus window");
        assert!(unsafe { IsWindow(mh) } != 0 && !win.closed);
        let action = win.app.menus.clone().unwrap().borrow().action(&muda::MenuId::new("plus-open")).unwrap();
        win.app.change(action); settle(&mut win);
        let plus = plus_window(&win.app).expect("Plus > Open Plus reopens"); let ph = hwnd(&plus);
        unsafe { SendMessageW(ph, WM_CLOSE, 0, 0); } settle(&mut win);
        assert!(!win.app.ui.plus.open && plus_window(&win.app).is_none(), "the X closes the Plus window");
        assert!(unsafe { IsWindow(mh) } != 0 && !win.closed);
        drop(win); std::fs::remove_file(settings).unwrap();
    }
}

fn window_png(w: u32, h: u32, output: &[u32]) -> Option<Vec<u8>> { (output.len() == (w * h) as usize && !output.is_empty()).then(|| megatraveller::plus::png::rgb(w, h, output).ok()).flatten() }
#[cfg(test)]
mod bug_report_tests {
    #[test]
    fn the_report_keeps_the_window_the_tester_saw() {
        let png = super::window_png(4, 2, &[0x112233; 8]).expect("a drawn window becomes window.png");
        assert_eq!(&png[1..4], b"PNG");
        assert!(super::window_png(4, 2, &[]).is_none() && super::window_png(4, 3, &[0; 8]).is_none(), "a stale or empty frame is not written");
    }
}
