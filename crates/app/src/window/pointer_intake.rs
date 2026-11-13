use std::time::{Duration, Instant};
use winit::{event::{ElementState, MouseButton, WindowEvent}, window::Window};

#[derive(Default)]
pub(super) struct FocusClick { lost: bool, armed: Option<Instant>, release: Option<MouseButton> }
impl FocusClick {
    pub const GRACE: Duration = Duration::from_millis(400);
    pub fn swallow(&mut self, event: &WindowEvent, now: Instant) -> bool {
        match event {
            WindowEvent::Focused(false) => { self.lost = true; self.armed = None; false }
            WindowEvent::Focused(true) => { self.lost = false; self.armed = Some(now); false }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                let guard = self.lost || self.armed.is_some_and(|t| now.duration_since(t) <= Self::GRACE);
                self.lost = false; self.armed = None;
                if guard { self.release = Some(*button); }
                guard
            }
            WindowEvent::MouseInput { state: ElementState::Released, button, .. } => { if self.release == Some(*button) { self.release = None; true } else { false } }
            _ => false,
        }
    }
}

pub(super) fn accepts(faithful: bool, mouse: bool, event: &WindowEvent) -> bool {
    faithful || mouse || !matches!(event,
        WindowEvent::CursorMoved { .. } | WindowEvent::CursorEntered { .. } | WindowEvent::CursorLeft { .. }
        | WindowEvent::MouseInput { .. } | WindowEvent::MouseWheel { .. } | WindowEvent::Touch(_)
        | WindowEvent::PinchGesture { .. } | WindowEvent::PanGesture { .. } | WindowEvent::DoubleTapGesture { .. }
        | WindowEvent::RotationGesture { .. } | WindowEvent::TouchpadPressure { .. }
        | WindowEvent::DroppedFile(_) | WindowEvent::HoveredFile(_) | WindowEvent::HoveredFileCancelled)
}

pub(super) fn attach(window: &Window) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    native::attach(window)?;
    let _ = window;
    Ok(())
}

#[cfg(target_os = "windows")]
mod native {
    use super::*;
    use std::cell::Cell;
    use windows_sys::Win32::{Foundation::{HWND, LPARAM, LRESULT, WPARAM}, UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass}, WindowsAndMessaging::*}};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    #[derive(Default)]
    struct ActivationClick { release: Cell<u32> }
    impl ActivationClick {
        fn message(&self, msg: u32, lp: LPARAM) -> Option<LRESULT> {
            if msg == WM_MOUSEACTIVATE && lp as u16 == HTCLIENT as u16 {
                let release = match (lp as usize >> 16) as u32 {
                    WM_LBUTTONDOWN => WM_LBUTTONUP, WM_RBUTTONDOWN => WM_RBUTTONUP,
                    WM_MBUTTONDOWN => WM_MBUTTONUP, WM_XBUTTONDOWN => WM_XBUTTONUP, _ => return None,
                };
                self.release.set(release);
                return Some(MA_ACTIVATEANDEAT as LRESULT);
            }
            if msg == self.release.get() && msg != 0 { self.release.set(0); return Some(0); }
            if matches!(msg, WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN) { self.release.set(0); }
            None
        }
    }
    const ID: usize = 0x4d385646;
    unsafe extern "system" fn subclass(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM, id: usize, data: usize) -> LRESULT {
        let state = unsafe { &*(data as *const ActivationClick) };
        if let Some(result) = state.message(msg, lp) { return result; }
        if msg == WM_NCDESTROY {
            unsafe { RemoveWindowSubclass(hwnd, Some(subclass), id); drop(Box::from_raw(data as *mut ActivationClick)); }
        }
        unsafe { DefSubclassProc(hwnd, msg, wp, lp) }
    }
    pub(super) fn attach(window: &Window) -> Result<(), String> {
        let RawWindowHandle::Win32(handle) = window.window_handle().map_err(|e| e.to_string())?.as_raw() else { return Ok(()) };
        let state = Box::into_raw(Box::<ActivationClick>::default());
        if unsafe { SetWindowSubclass(handle.hwnd.get() as HWND, Some(subclass), ID, state as usize) } == 0 {
            unsafe { drop(Box::from_raw(state)); }
            return Err("activation-click subclass failed".into());
        }
        Ok(())
    }
    #[test]
    fn focus_click_cannot_reach_the_enabled_game_menu() {
        use megatraveller::{look::Look, panels::{self, Component, UiEvent, Button}};
        use mt_engine::{io::Event, play::{Decision, Kind, Option as Opt}};
        let mut d = Decision::new(Kind::Ground, "t", "6923");
        d.options.push(Opt::new("cmd:game", "Game", vec![Event::Key(0x122)]));
        let mut ui = panels::UiState { decision: Some(d), ..Default::default() };
        ui.component_input.size = (320, panels::toolbar::HEIGHT);
        let scene = mt1::ui::Scene::program(mt1::ui::ScreenKind::Ground);
        let toolbar = panels::toolbar::Toolbar::new(Default::default());
        let state = ActivationClick::default();
        let look: Look = toml::from_str("mouse = false").unwrap(); assert!(look.mouse);
        assert_eq!(state.message(WM_MOUSEACTIVATE, ((WM_LBUTTONDOWN << 16) | HTCLIENT) as LPARAM), Some(MA_ACTIVATEANDEAT as LRESULT));
        let mut clicks = Vec::new();
        for message in [WM_LBUTTONUP, WM_LBUTTONDOWN, WM_LBUTTONUP] {
            if state.message(message, 0).is_none() && message == WM_LBUTTONUP {
                clicks.extend(toolbar.input(&scene, &ui, &UiEvent::Up(Button::Left), (10, 6)));
            }
            if message == WM_LBUTTONDOWN { assert!(clicks.is_empty(), "the activating release was swallowed"); }
        }
        assert_eq!(clicks, megatraveller::mouse::decision::raw(&[Event::Key(0x122)]), "only the second click opens Game");
    }
    #[test]
    fn focus_click_eats_activation_and_release_in_both_mouse_settings() {
        for mouse in [false, true] {
            for (down, up) in [(WM_LBUTTONDOWN, WM_LBUTTONUP), (WM_RBUTTONDOWN, WM_RBUTTONUP), (WM_MBUTTONDOWN, WM_MBUTTONUP)] {
                let state = ActivationClick::default();
                assert_eq!(state.message(WM_MOUSEACTIVATE, ((down << 16) | HTCLIENT) as LPARAM), Some(MA_ACTIVATEANDEAT as LRESULT), "mouse={mouse}");
                assert_eq!(state.message(up, 0), Some(0));
                assert_eq!(state.message(down, 0), None);
                assert_eq!(state.message(up, 0), None);
                assert_eq!(state.message(WM_MOUSEACTIVATE, ((down << 16) | HTMENU) as LPARAM), None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::{dpi::PhysicalPosition, event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, TouchPhase}};
    #[test]
    fn mouse_switch_filters_all_pointer_edges_but_not_native_window_events() {
        let device_id = DeviceId::dummy();
        for event in [WindowEvent::CursorMoved { device_id, position: PhysicalPosition::new(40.0, 80.0) },
            WindowEvent::MouseInput { device_id, state: ElementState::Pressed, button: MouseButton::Left },
            WindowEvent::MouseInput { device_id, state: ElementState::Released, button: MouseButton::Left },
            WindowEvent::MouseWheel { device_id, delta: MouseScrollDelta::LineDelta(0.0, 1.0), phase: TouchPhase::Moved },
            WindowEvent::CursorLeft { device_id }, WindowEvent::DroppedFile("gamefiles".into())] {
            assert!(!accepts(false, false, &event));
            assert!(accepts(false, true, &event));
            assert!(accepts(true, false, &event));
        }
        for event in [WindowEvent::Focused(true), WindowEvent::Focused(false), WindowEvent::RedrawRequested, WindowEvent::CloseRequested] {
            assert!(accepts(false, false, &event));
        }
    }
}

#[cfg(all(test, target_os = "windows"))]
mod desktop_tests {
    use super::super::*;
    use winit::event::{DeviceId, TouchPhase};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    struct DesktopInput(Rc<RefCell<Win>>);
    impl Input for DesktopInput {
        fn next_key(&mut self) -> Key { self.0.borrow_mut().next_key() }
        fn poll(&mut self, clock: &mut dyn Clock) -> IoEvent { self.0.borrow_mut().poll(clock) }
        fn arrived(&mut self) -> bool { self.0.borrow_mut().arrived() }
        fn mouse(&mut self) -> MouseState { self.0.borrow_mut().mouse() }
        fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.0.borrow_mut().intent() }
    }

    #[test]
    #[ignore = "requires a Windows desktop; run in its own test process"]
    fn native_mouse_off_intake_preserves_state_and_frame_and_focus_click_is_eaten() {
        let path = std::env::temp_dir().join(format!("mt-mouse-off-{}.toml", std::process::id()));
        let mut win = Win::with_settings(3, false, false, path.clone()).unwrap();
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let mut prepared = crate::scenario::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml"), &dir).unwrap();
        win.panel_assets(&mut mt_engine::io::Dir(dir));
        win.scene(&Scene::capture(&prepared.game, mt1::ui::ScreenKind::Ground, None));
        win.app.prepare_layout(); win.app.render().unwrap();
        let before = win.app.output.clone();
        let main = win.app.window.clone().unwrap(); let id = main.id();
        let proxy = win.events.create_proxy(); let device_id = DeviceId::dummy();
        let desktop = Rc::new(RefCell::new(win));
        prepared.game.input = Box::new(DesktopInput(desktop.clone()));
        prepared.game.harness = false;
        prepared.game.poll();
        let state_hash = mt1::play::hash(&mut prepared.game);
        for at in [(20.0, 20.0), (150.0, 330.0), (900.0, 600.0)] {
            let mut events = vec![WindowEvent::CursorMoved { device_id, position: PhysicalPosition::new(at.0, at.1) },
                WindowEvent::MouseWheel { device_id, delta: MouseScrollDelta::LineDelta(0.0, 1.0), phase: TouchPhase::Moved }];
            for button in [MouseButton::Left, MouseButton::Right, MouseButton::Middle] { for state in [ElementState::Pressed, ElementState::Released] {
                events.push(WindowEvent::MouseInput { device_id, state, button });
            } }
            for e in events { proxy.send_event(Event::TestWindow(id, e)).unwrap(); }
            desktop.borrow_mut().pump_for(Duration::from_millis(20));
            prepared.game.poll();
            assert!(mt1::ground::squad::poll(&mut prepared.game).is_empty());
            assert_eq!(mt1::play::hash(&mut prepared.game), state_hash);
        }
        let mut win = desktop.borrow_mut();
        assert!(win.app.input.events.is_empty() && win.app.intents.is_empty());
        assert!(win.app.mouse_window_pos.is_none() && win.app.ui.hover.is_none() && win.app.ui.capture.is_none());
        assert_eq!(win.mouse(), MouseState { buttons: 0, col: -1, row: -1 });
        assert_eq!(mt1::play::hash(&mut prepared.game), state_hash);
        win.app.render().unwrap(); assert_eq!(win.app.output, before);

        let RawWindowHandle::Win32(handle) = main.window_handle().unwrap().as_raw() else { panic!() };
        let hwnd = handle.hwnd.get() as windows_sys::Win32::Foundation::HWND;
        assert!(!unsafe { GetMenu(hwnd) }.is_null(), "the OS menu remains attached");
        for mouse in [false, true] {
            win.app.look.mouse = mouse;
            assert_eq!(unsafe { SendMessageW(hwnd, WM_MOUSEACTIVATE, hwnd as usize, ((WM_LBUTTONDOWN << 16) | HTCLIENT) as isize) }, MA_ACTIVATEANDEAT as isize);
            unsafe { SendMessageW(hwnd, WM_LBUTTONUP, 0, (20 << 16) | 20); }
            win.pump_for(Duration::from_millis(10));
            assert!(win.app.input.events.is_empty() && win.app.intents.is_empty());
        }
        proxy.send_event(Event::TestWindow(id, WindowEvent::CursorMoved { device_id, position: PhysicalPosition::new(60.0, 60.0) })).unwrap();
        win.pump_for(Duration::from_millis(20));
        assert_eq!(win.app.mouse_window_pos, Some((60.0, 60.0)));
        drop(win); drop(prepared); drop(desktop); std::fs::remove_file(path).unwrap();
    }

    struct WelcomeClick { win: Rc<RefCell<Win>>, seen: Vec<mt_engine::play::Decision>, click: Option<(mt_engine::play::Decision, &'static str)> }
    impl WelcomeClick {
        fn click(&mut self) {
            let Some((d, label)) = self.click.take() else { return };
            let mut win = self.win.borrow_mut();
            win.pump_for(Duration::from_millis(50)); win.app.prepare_layout();
            {
                let i = d.options.iter().position(|o| o.label == label).unwrap_or_else(|| panic!("{label} in {:?}", d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>()));
                let rows = megatraveller::mouse::decision::native_rows(&win.app.ui.frame.text.visible, &d);
                eprintln!("visible runs {} rows {:?}", win.app.ui.frame.text.visible.len(), rows);
                let (id, local) = match rows.iter().find(|(k, _)| *k == i) { Some((_, r)) => ("Picture", (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2)), None => ("Choices", (10, 27 + i as i32 * 12)) };
                let main = win.app.window.clone().unwrap(); let m = win.app.metrics(main.id()).unwrap();
                let Host::Dock(host) = win.app.layout.host(id).unwrap_or_else(|| panic!("{id} host")) else { panic!("{id} docked") };
                let (x, y) = m.project(host, local, &win.app.look).unwrap();
                eprintln!("{label}: {id} host {host:?} local {local:?}");
                let RawWindowHandle::Win32(h) = main.window_handle().unwrap().as_raw() else { panic!() };
                let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
                let lp = (((y as i32) << 16) | (x as i32 & 0xffff)) as isize;
                unsafe { PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp); PostMessageW(hwnd, WM_LBUTTONDOWN, 1, lp); PostMessageW(hwnd, WM_LBUTTONUP, 0, lp); }
                eprintln!("click: window px ({x:.0},{y:.0}) = 1990 {local:?} scene {:?} metrics {:?}", win.app.scene.as_ref().map(|s| s.screen), m);
                win.pump_for(Duration::from_millis(60));
                eprintln!("after click: route pending={} ui.decision={:?} raw events={}", win.app.decision.pending(), win.app.ui.decision.as_ref().map(|d| d.context.routine.clone()), win.app.input.events.len());
            }
        }
    }
    impl Input for WelcomeClick {
        fn wants_decision(&self) -> bool { self.win.borrow().wants_decision() }
        fn decide(&mut self, d: &mt_engine::play::Decision) {
            self.win.borrow_mut().decide(d); self.seen.push(d.clone());
            let n = self.seen.len(); let mut win = self.win.borrow_mut();
            eprintln!("decision {n}: {}.{} {:?}", d.context.program, d.context.routine, d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>());
            if n == 2 { self.click = Some((d.clone(), "Start New Game")); } else {
                let label = match n { 1 => "Start game", 3 => "back", _ => "Return to DOS" };
                let o = d.options.iter().find(|o| o.id == label || o.label == label).unwrap_or_else(|| panic!("no {label} in {:?}", d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>()));
                let input = o.input.clone(); let a: &mut App = &mut win.app; a.decision.queue(&mut a.ui, &input);
            }
        }
        fn next_key(&mut self) -> Key {
            self.click();
            let mut w = self.win.borrow_mut();
            for i in 0..200 {
                { let a: &mut App = &mut w.app;
                  a.decision.restore(&mut a.ui);
                  if let Some(k) = a.decision.next_key() { return k }
                  if let Some(k) = a.input.next_key() { a.decision.hardware(&mut a.ui, IoEvent::Key(k)); return k } }
                w.pump_for(Duration::from_millis(16));
            }
            panic!("the click produced no key in 3 s: decisions {:?}, raw events {:?}", self.seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>(), w.app.input.events);
        }
        fn poll(&mut self, clock: &mut dyn Clock) -> IoEvent {
            self.click(); self.win.borrow_mut().poll(clock)
        }
        fn key_pending(&mut self) -> bool { self.click(); self.win.borrow_mut().key_pending() }
        fn arrived(&mut self) -> bool { self.win.borrow_mut().arrived() }
        fn flush(&mut self) { self.win.borrow_mut().flush() }
        fn present(&mut self, rgb: &[u32]) { self.win.borrow_mut().present(rgb) }
        fn scene(&mut self, s: &dyn std::any::Any) { self.win.borrow_mut().scene(s) }
        fn mouse(&mut self) -> MouseState { self.win.borrow_mut().mouse() }
        fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.win.borrow_mut().intent() }
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn welcome_click_through_the_real_window() {
        use mt_engine::{data::{exepack, ly}, io::{Dir, Files, Overlay}};
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let path = std::env::temp_dir().join(format!("mt-welcome-click-{}.toml", std::process::id()));
        if let Some(look) = std::env::var_os("MT_TEST_LOOK") { std::fs::copy(look, &path).unwrap(); }
        let mut win = Win::with_settings(if path.exists() { 0 } else { 3 }, false, false, path.clone()).unwrap();
        win.panel_assets(&mut Dir(dir.clone()));
        let win = Rc::new(RefCell::new(win));
        let files = Dir(dir.clone()); let read = |name| files.read(name).unwrap();
        let mut overlay = Overlay::new(Some(&dir)); overlay.hide("UCHARS.DAT");
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default();
        struct Report(Rc<RefCell<Vec<mt_engine::play::Decision>>>, WelcomeClick);
        impl Input for Report {
            fn wants_decision(&self) -> bool { self.1.wants_decision() }
            fn decide(&mut self, d: &mt_engine::play::Decision) { self.1.decide(d); *self.0.borrow_mut() = self.1.seen.clone(); }
            fn next_key(&mut self) -> Key { self.1.next_key() }
            fn poll(&mut self, c: &mut dyn Clock) -> IoEvent { self.1.poll(c) }
            fn key_pending(&mut self) -> bool { self.1.key_pending() }
            fn arrived(&mut self) -> bool { self.1.arrived() }
            fn flush(&mut self) { self.1.flush() }
            fn present(&mut self, rgb: &[u32]) { self.1.present(rgb) }
            fn scene(&mut self, s: &dyn std::any::Any) { self.1.scene(s) }
            fn mouse(&mut self) -> MouseState { self.1.mouse() }
            fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.1.intent() }
        }
        let mut input = Report(seen.clone(), WelcomeClick { win: win.clone(), seen: Vec::new(), click: None });
        use mt_engine::data::{pic, Variant}; use mt1::screens::title::{self, TcRand, Title};
        let exe = exepack::unpack(&read("STARTUP.EXE")).unwrap().exe;
        let mut t = Title::new(&exe, ly::decode(&read("TITLE.LY")).unwrap(), ly::decode(&read("FONT.LY")).unwrap(),
            pic::decode(Variant::Mt1, &read("TIT.PIC")).unwrap(), pic::decode(Variant::Mt1, &read("ZOD.PIC")).unwrap()).unwrap();
        let mut clock = mt_engine::sim::tick::Counter::at(1234); let mut rng = TcRand { seed: 1234 }; let dac = mt_engine::present::palette::ega_dac();
        assert_eq!(t.run(&mut || rng.next(), &mut clock, &mut input, |t, i| { i.present(&t.fb.rgb(t.display, &dac)); i.scene(&Scene::program(mt1::ui::ScreenKind::Startup)) }), title::Exit::Game, "the title click");
        let mut cg = mt1::screens::cg::Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe, ly::decode(&read("FONT.LY")).unwrap(),
            ly::decode(&read("CG.LY")).unwrap(), Box::new(input), Box::new(clock), Box::new(overlay)).unwrap();
        cg.snapshots = true;
        let _ = mt1::screens::cg::run(&mut cg);
        let seen = seen.borrow();
        assert!(seen.len() >= 3, "the click never advanced the WELCOME menu: {:?}", seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>());
        assert!(seen[2].options.iter().any(|o| o.label == "Create Party Disk"), "Start New Game click reached the disk menu; got {:?}", seen[2].options.iter().map(|o| o.label.clone()).collect::<Vec<_>>());
        drop(cg); let _ = std::fs::remove_file(path);
    }
    struct RowClicks { win: Rc<RefCell<Win>>, seen: Rc<RefCell<Vec<mt_engine::play::Decision>>>, script: VecDeque<&'static str>, pending: Option<(mt_engine::play::Decision, &'static str)>, log: Rc<RefCell<Vec<String>>>, done: bool,
        command_keys: VecDeque<Vec<IoEvent>> }
    impl RowClicks {
        fn new(win: Rc<RefCell<Win>>, seen: Rc<RefCell<Vec<mt_engine::play::Decision>>>, log: Rc<RefCell<Vec<String>>>, script: &[&'static str]) -> Self {
            RowClicks { win, seen, script: script.iter().copied().collect(), pending: None, log, done: false, command_keys: VecDeque::new() }
        }
    }
    #[derive(Debug)] struct ScriptDone;
    thread_local! { static POLLS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
    impl RowClicks {
        fn click(&mut self) {
            let Some((d, label)) = self.pending.take() else { return };
            let mut win = self.win.borrow_mut();
            win.pump_for(Duration::from_millis(50)); win.app.prepare_layout();
            let button = label.strip_prefix("button:").map(|b| megatraveller::mouse::decision::PANEL_BUTTONS.iter().find(|(name, _, _)| *name == b).unwrap_or_else(|| panic!("no panel button {b}")).1);
            let i = if button.is_some() { 0 } else if let Some(l) = &d.list { l.rows.iter().position(|r| r.label == label || r.id == label).unwrap_or_else(|| panic!("{label} in list {:?}", l.rows.iter().map(|r| r.id.clone()).collect::<Vec<_>>())) }
                else { d.options.iter().position(|o| o.label == label || o.id == label).unwrap_or_else(|| panic!("{label} in {:?}", d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>())) };
            if d.context.program == "title" && std::env::var_os("MT_TITLE_DIAG").is_some() {
                let a: &mut App = &mut win.app;
                let scene = a.scene.clone(); let modal = megatraveller::mouse::decision::modal(&a.ui);
                let hit = scene.as_ref().and_then(|s| a.layout.hit(s, None, (160, 100)).map(|h| h.0));
                let direct = scene.as_ref().map_or_else(Vec::new, |sc| panels::controls::dispatch(&a.layout, "Picture", sc, &mut a.ui, &UiEvent::Up(Button::Left), (160, 100)));
                let line = format!("title diag: scene {:?} modal {modal} ui.decision {:?} hit {hit:?} capture {:?} mouse_window_pos {:?} direct Picture dispatch -> {} intents {:?}",
                    scene.as_ref().map(|s| s.screen), a.ui.decision.as_ref().map(|d| d.kind), a.ui.capture, a.mouse_window_pos, direct.len(), direct);
                eprintln!("{line}"); self.log.borrow_mut().push(line);
                if !direct.is_empty() { let events: Vec<IoEvent> = direct.into_iter().filter_map(|i| match i { Intent::Raw(e) => Some(e), _ => None }).collect(); a.decision.queue(&mut a.ui, &events); self.log.borrow_mut().push("title: answered by the direct Picture dispatch".into()); return }
            }
            let rows = megatraveller::mouse::decision::native_rows(&win.app.ui.frame.text.visible, &d);
            let at = match rows.iter().find(|(k, _)| *k == i) {
                _ if button.is_some() => { let r = button.unwrap(); (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2) }
                Some((_, r)) => (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2),
                None if d.kind == mt_engine::play::Kind::Ack => (160, 100),
                None => {
                    let input = d.options[i].input.clone(); let a: &mut App = &mut win.app; a.decision.queue(&mut a.ui, &input);
                    self.log.borrow_mut().push(format!("{}.{} {label}: no drawn row, key route {input:?}", d.context.program, d.context.routine)); return
                }
            };
            let main = win.app.window.clone().unwrap(); let m = win.app.metrics(main.id()).unwrap();
            let scene_now = win.app.scene.clone().unwrap_or_else(|| Scene::program(mt1::ui::ScreenKind::Picture));
            let (id, host, local) = win.app.layout.ids().into_iter().filter(|id| win.app.layout.visible(id, &scene_now)).find_map(|id| {
                let Host::Dock(host) = win.app.layout.host(id)? else { return None };
                match win.app.layout.placement(id)?.origin { Origin::Classic1990(o) if o.contains(at) => Some((id, host, (at.0 - o.x, at.1 - o.y))), _ => None }
            }).unwrap_or_else(|| panic!("no visible 1990 host at {at:?} for {label}"));
            let (x, y) = m.project(host, local, &win.app.look).unwrap();
            let RawWindowHandle::Win32(h) = main.window_handle().unwrap().as_raw() else { panic!() };
            let hwnd = h.hwnd.get() as windows_sys::Win32::Foundation::HWND;
            let lp = (((y as i32) << 16) | (x as i32 & 0xffff)) as isize;
            unsafe { PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp); PostMessageW(hwnd, WM_LBUTTONDOWN, 1, lp); PostMessageW(hwnd, WM_LBUTTONUP, 0, lp); }
            let t0 = std::time::Instant::now(); let mut retried = false;
            while t0.elapsed() < Duration::from_millis(2000) {
                win.pump_for(Duration::from_millis(20)); if win.app.decision.pending() || win.app.ui.decision.is_none() { break }
                if !retried && t0.elapsed() > Duration::from_millis(1000) {
                    retried = true; eprintln!("  {label}: no answer after 1 s, clicking again at the same pixel");
                    unsafe { PostMessageW(hwnd, WM_MOUSEMOVE, 0, lp); PostMessageW(hwnd, WM_LBUTTONDOWN, 1, lp); PostMessageW(hwnd, WM_LBUTTONUP, 0, lp); }
                }
            }
            let line = format!("{}.{} {label}: click window px ({x:.0},{y:.0}) = 1990 {at:?} on {id}; after {} ms: route pending={} ui.decision={:?} screen {:?}",
                d.context.program, d.context.routine, t0.elapsed().as_millis(), win.app.decision.pending(), win.app.ui.decision.as_ref().map(|d| d.context.routine.clone()), win.app.scene.as_ref().map(|s| s.screen));
            eprintln!("{line}"); self.log.borrow_mut().push(line);
        }
        fn read(&mut self) {
            self.click();
            if self.done { std::panic::resume_unwind(Box::new(ScriptDone)) }
        }
    }
    impl Input for RowClicks {
        fn wants_decision(&self) -> bool { self.win.borrow().wants_decision() }
        fn decide(&mut self, d: &mt_engine::play::Decision) {
            self.win.borrow_mut().decide(d); self.seen.borrow_mut().push(d.clone());
            if matches!(d.kind, mt_engine::play::Kind::Ground | mt_engine::play::Kind::Space | mt_engine::play::Kind::CombatTurn) {
                let n = self.seen.borrow().len(); if n % 50 == 1 { eprintln!("command wait {n}: {}.{} hw queue {} store pending {}", d.context.program, d.context.routine, self.win.borrow().app.input.events.len(), self.win.borrow().app.decision.pending()); }
                if let Some(keys) = self.command_keys.pop_front() { eprintln!("command wait {n}: {}.{} -> keys {keys:?}", d.context.program, d.context.routine); let mut w = self.win.borrow_mut(); let a: &mut App = &mut w.app; a.input.events.extend(keys); }
                else if self.script.is_empty() && self.pending.is_none() { self.done = true; }
                if n > 400 { self.done = true; } return
            }
            eprintln!("decision {}: {}.{} {:?} {:?}", self.seen.borrow().len(), d.context.program, d.context.routine, d.kind, d.options.iter().filter(|o| o.enabled).map(|o| o.label.clone()).take(14).collect::<Vec<_>>());
            match self.script.pop_front() { Some(label) => self.pending = Some((d.clone(), label)), None => self.done = true }
        }
        fn next_key(&mut self) -> Key {
            self.read();
            let mut w = self.win.borrow_mut();
            for _ in 0..200 {
                { let a: &mut App = &mut w.app; a.decision.restore(&mut a.ui);
                  if let Some(k) = a.decision.next_key() { return k }
                  if let Some(k) = a.input.next_key() { a.decision.hardware(&mut a.ui, IoEvent::Key(k)); return k } }
                w.pump_for(Duration::from_millis(16));
            }
            panic!("the click produced no key in 3 s: {:?}", self.log.borrow())
        }
        fn poll(&mut self, clock: &mut dyn Clock) -> IoEvent {
            self.read(); let e = self.win.borrow_mut().poll(clock); if e == IoEvent::Idle { clock.wait_tick(); }
            let n = { let mut w = self.win.borrow_mut(); w.app.ui.log_scroll += 0; self.seen.borrow().len() };
            POLLS.with(|c| { let k = c.get() + 1; c.set(k); if k % 400 == 0 { let w = self.win.borrow(); eprintln!("poll {k}: event {e:?} decisions {n} screen {:?} ui.decision {:?} hw {} store pending {}", w.app.scene.as_ref().map(|s| s.screen), w.app.ui.decision.as_ref().map(|d| d.context.routine.clone()), w.app.input.events.len(), w.app.decision.pending()); } });
            e
        }
        fn key_pending(&mut self) -> bool { self.read(); self.win.borrow_mut().key_pending() }
        fn arrived(&mut self) -> bool { self.win.borrow_mut().arrived() }
        fn flush(&mut self) { self.win.borrow_mut().flush() }
        fn present(&mut self, rgb: &[u32]) { self.win.borrow_mut().present(rgb) }
        fn scene(&mut self, s: &dyn std::any::Any) { self.win.borrow_mut().scene(s) }
        fn mouse(&mut self) -> MouseState { self.win.borrow_mut().mouse() }
        fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.win.borrow_mut().intent() }
    }
    fn hidden_window(dir: &PathBuf) -> (Rc<RefCell<Win>>, PathBuf) {
        let path = std::env::temp_dir().join(format!("mt-row-clicks-{}-{:?}.toml", std::process::id(), std::thread::current().id()));
        if let Some(look) = std::env::var_os("MT_TEST_LOOK") { std::fs::copy(look, &path).unwrap(); }
        let mut win = Win::with_settings(if path.exists() { 0 } else { 3 }, false, false, path.clone()).unwrap();
        win.panel_assets(&mut mt_engine::io::Dir(dir.clone()));
        (Rc::new(RefCell::new(win)), path)
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn horizontal_menu_click_through_the_real_window() {
        use mt_engine::{data::{exepack, ly, pic, Variant}, io::{Dir, Files, Overlay}};
        use mt1::screens::title::{self, TcRand, Title};
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let (win, path) = hidden_window(&dir);
        let files = Dir(dir.clone()); let read = |name| files.read(name).unwrap();
        let mut overlay = Overlay::new(Some(&dir)); overlay.hide("UCHARS.DAT");
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default(); let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut input = RowClicks { win: win.clone(), seen: seen.clone(), script: ["Start game", "Start New Game", "Create Party Disk", "Create New Character", "No"].into(), pending: None, log: log.clone(), done: false, command_keys: VecDeque::new() };
        let exe = exepack::unpack(&read("STARTUP.EXE")).unwrap().exe;
        let mut t = Title::new(&exe, ly::decode(&read("TITLE.LY")).unwrap(), ly::decode(&read("FONT.LY")).unwrap(),
            pic::decode(Variant::Mt1, &read("TIT.PIC")).unwrap(), pic::decode(Variant::Mt1, &read("ZOD.PIC")).unwrap()).unwrap();
        let mut clock = mt_engine::sim::tick::Counter::at(1234); let mut rng = TcRand { seed: 1234 }; let dac = mt_engine::present::palette::ega_dac();
        assert_eq!(t.run(&mut || rng.next(), &mut clock, &mut input, |t, i| { i.present(&t.fb.rgb(t.display, &dac)); i.scene(&Scene::program(mt1::ui::ScreenKind::Startup)) }), title::Exit::Game, "the title click");
        let mut cg = mt1::screens::cg::Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe, ly::decode(&read("FONT.LY")).unwrap(),
            ly::decode(&read("CG.LY")).unwrap(), Box::new(input), Box::new(clock), Box::new(overlay)).unwrap();
        cg.snapshots = true;
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| { let _ = mt1::screens::cg::run(&mut cg); }));
        if let Err(e) = r { if !e.is::<ScriptDone>() { std::panic::resume_unwind(e) } }
        let seen = seen.borrow(); let log = log.borrow();
        for l in log.iter() { eprintln!("LOG {l}"); }
        let yn = seen.iter().position(|d| d.context.routine == "56A6" && d.options.iter().any(|o| o.label == "No")).expect("the Yes / No menu was reached by clicks");
        assert!(seen.len() > yn + 1, "the click on No answered the horizontal menu: {:?}", seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>());
        assert!(log.iter().all(|l| !l.contains("no drawn row")), "every scripted row was drawn and clicked: {log:?}");
        drop(cg); let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn ground_menu_click_through_the_real_window() {
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let (win, path) = hidden_window(&dir);
        let prepared = crate::scenario::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml"), &dir).unwrap();
        let mut g = prepared.game;
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default(); let log: Rc<RefCell<Vec<String>>> = Rc::default();
        g.input = Box::new(RowClicks { win: win.clone(), seen: seen.clone(), script: ["Sound off"].into(), pending: None, log: log.clone(), done: false, command_keys: VecDeque::new() });
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(1234)); g.scene = Some(mt1::ui::SceneHook::default());
        g.tooling = Some(Box::new(crate::scenario::Loader::new(Default::default(), dir.clone(), prepared.extensions)));
        if g.squad.is_none() { g.squad = Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap())); }
        { let mut w = win.borrow_mut(); let a: &mut App = &mut w.app; a.input.events.extend([IoEvent::Key(0x122), IoEvent::KeyUp(0x122)]); }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = r { if !e.is::<ScriptDone>() { std::panic::resume_unwind(e) } }
        let seen = seen.borrow(); let log = log.borrow();
        for l in log.iter() { eprintln!("LOG {l}"); }
        let menu = seen.iter().position(|d| d.context.routine == "AF79").expect("the Game menu was reached");
        assert!(log.iter().any(|l| l.contains("AF79 Sound off: click") && l.contains("on Side panel")), "Sound off was clicked on the Side panel: {log:?}");
        assert!(seen.len() > menu + 1 && seen[menu + 1].context.routine != "AF79", "the click answered the menu: {:?}", seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>());
        drop(g); let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn starport_desk_click_through_the_real_window() {
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let (win, path) = hidden_window(&dir);
        let prepared = crate::scenario::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml"), &dir).unwrap();
        let mut g = prepared.game;
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default(); let log: Rc<RefCell<Vec<String>>> = Rc::default();
        g.input = Box::new(RowClicks { win: win.clone(), seen: seen.clone(), script: ["Bank"].into(), pending: None, log: log.clone(), done: false, command_keys: VecDeque::new() });
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(1234)); g.scene = Some(mt1::ui::SceneHook::default());
        g.tooling = Some(Box::new(crate::scenario::Loader::new(Default::default(), dir.clone(), prepared.extensions)));
        if g.squad.is_none() { g.squad = Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap())); }
        { let mut w = win.borrow_mut(); let a: &mut App = &mut w.app; a.input.events.extend([IoEvent::Key(mt_engine::io::keys::LEFT), IoEvent::KeyUp(mt_engine::io::keys::LEFT)]); }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = r { if !e.is::<ScriptDone>() { std::panic::resume_unwind(e) } }
        let seen = seen.borrow(); let log = log.borrow();
        for l in log.iter() { eprintln!("LOG {l}"); }
        let desk = seen.iter().position(|d| d.context.routine == "13FF2").expect("the desk was reached");
        assert!(seen.len() > desk + 1 && seen[desk + 1].context.routine != "13FF2", "the Bank cell click answered the desk: {:?}", seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>());
        assert!(log.iter().any(|l| l.contains("13FF2 Bank: click")), "the Bank cell was clicked: {log:?}");
        drop(g); let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn hospital_click_through_the_real_window() {
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let (win, path) = hidden_window(&dir);
        let prepared = crate::scenario::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/h9c_hospital_efate.toml"), &dir).unwrap();
        let mut g = prepared.game;
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default(); let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut input = RowClicks::new(win.clone(), seen.clone(), log.clone(), &["list:1199B/0", "button:ESC", "list:1199B/0", "list:11571/0"]);
        input.command_keys.push_back(vec![IoEvent::Key(mt_engine::io::keys::LEFT), IoEvent::KeyUp(mt_engine::io::keys::LEFT)]);
        g.input = Box::new(input);
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(1234)); g.scene = Some(mt1::ui::SceneHook::default());
        g.tooling = Some(Box::new(crate::scenario::Loader::new(Default::default(), dir.clone(), prepared.extensions)));
        if g.squad.is_none() { g.squad = Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap())); }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = r { if !e.is::<ScriptDone>() { std::panic::resume_unwind(e) } }
        let seen = seen.borrow(); let log = log.borrow();
        for l in log.iter() { eprintln!("LOG {l}"); }
        let routines: Vec<&str> = seen.iter().filter(|d| d.kind != mt_engine::play::Kind::Ground).map(|d| d.context.routine.as_str()).collect();
        assert_eq!(routines, ["1199B", "11571", "1199B", "11571"], "payer row, ESC button, payer row, patient row: {log:?}");
        assert!(log.iter().all(|l| !l.contains("no drawn row")), "every scripted row was drawn and clicked: {log:?}");
        assert_eq!(g.mem.party_u8(0x127), g.mem.party_u8(0x125), "Traveller 1 was cured by the clicked row");
        drop(g); let _ = std::fs::remove_file(path);
    }
    #[test]
    #[ignore = "requires a Windows desktop + MT1_GAMEFILES; run in its own test process"]
    fn fight_item_menu_click_through_the_real_window() {
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let (win, path) = hidden_window(&dir);
        let prepared = crate::scenario::load(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml"), &dir).unwrap();
        let mut g = prepared.game;
        let seen: Rc<RefCell<Vec<mt_engine::play::Decision>>> = Rc::default(); let log: Rc<RefCell<Vec<String>>> = Rc::default();
        let mut input = RowClicks::new(win.clone(), seen.clone(), log.clone(), &["Take", "Back"]);
        let item_key = vec![IoEvent::Key(0x117), IoEvent::KeyUp(0x117)];
        input.command_keys.push_back(item_key.clone()); input.command_keys.push_back(item_key);
        g.input = Box::new(input);
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(1234)); g.scene = Some(mt1::ui::SceneHook::default());
        g.tooling = Some(Box::new(crate::scenario::Loader::new(Default::default(), dir.clone(), prepared.extensions)));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = r { if !e.is::<ScriptDone>() { std::panic::resume_unwind(e) } }
        let seen = seen.borrow(); let log = log.borrow();
        for l in log.iter() { eprintln!("LOG {l}"); }
        let menus: Vec<usize> = seen.iter().enumerate().filter(|(_, d)| d.context.routine == "AF79").map(|(i, _)| i).collect();
        assert!(menus.len() >= 2, "the Item menu opened twice: {:?}", seen.iter().map(|d| format!("{:?}:{}", d.kind, d.context.routine)).collect::<Vec<_>>());
        for (m, label) in menus.iter().zip(["Take", "Back"]) {
            assert!(log.iter().any(|l| l.contains(&format!("AF79 {label}: click")) && l.contains("on Side panel")), "{label} was clicked on the Side panel: {log:?}");
            assert!(seen.len() > m + 1 && seen[m + 1].context.routine != "AF79", "the {label} click answered the menu: {log:?}");
        }
        drop(g); let _ = std::fs::remove_file(path);
    }
}

#[cfg(test)]
mod focus_click_tests {
    use super::*;
    use winit::event::DeviceId;
    fn press(b: MouseButton) -> WindowEvent { WindowEvent::MouseInput { device_id: DeviceId::dummy(), state: ElementState::Pressed, button: b } }
    fn release(b: MouseButton) -> WindowEvent { WindowEvent::MouseInput { device_id: DeviceId::dummy(), state: ElementState::Released, button: b } }
    #[test]
    fn first_click_after_focus_loss_is_dropped_and_the_second_acts() {
        let (l, r) = (MouseButton::Left, MouseButton::Right); let t0 = Instant::now(); let mut g = FocusClick::default();
        assert!(!g.swallow(&press(l), t0) && !g.swallow(&release(l), t0), "no focus change: the click acts");
        assert!(!g.swallow(&WindowEvent::Focused(false), t0));
        assert!(g.swallow(&press(l), t0 + Duration::from_secs(3)) && g.swallow(&release(l), t0 + Duration::from_secs(3)), "the activating click is dropped");
        assert!(!g.swallow(&press(l), t0 + Duration::from_secs(4)) && !g.swallow(&release(l), t0 + Duration::from_secs(4)), "the second click acts");
        g.swallow(&WindowEvent::Focused(false), t0); assert!(!g.swallow(&WindowEvent::Focused(true), t0 + Duration::from_secs(5)));
        let t = t0 + Duration::from_secs(5) + FocusClick::GRACE; assert!(g.swallow(&press(r), t) && g.swallow(&release(r), t));
        g.swallow(&WindowEvent::Focused(false), t0); g.swallow(&WindowEvent::Focused(true), t0 + Duration::from_secs(6));
        let t = t0 + Duration::from_secs(6) + FocusClick::GRACE + Duration::from_millis(1); assert!(!g.swallow(&press(l), t) && !g.swallow(&release(l), t));
        g.swallow(&WindowEvent::Focused(false), t0); assert!(g.swallow(&press(l), t0)); assert!(!g.swallow(&release(r), t0)); assert!(g.swallow(&release(l), t0)); assert!(!g.swallow(&release(l), t0));
    }
}
