#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use megatraveller::{mouse::pointer, panels::{self, components, controls, viewport::Camera, Button, Host, Layout, Rect, UiEvent, UiState}, scenario};
use mt1::{ground::{input, main_loop, orders::PATH_WALK, render, round, squad, Ground}, play, ui::{Cell, ComponentId, Intent, Scene, ScreenKind, Verb}};
use mt_engine::{io::{Event, Input, MouseState}, play::Decision, sim::tick::{Clock, Counter}};
use std::{any::Any, cell::RefCell, collections::VecDeque, panic::{catch_unwind, AssertUnwindSafe}, path::{Path, PathBuf}, rc::Rc, time::{Duration, Instant}};

const WINDOW: (u32, u32) = (960, 720);
const LIMIT: Duration = Duration::from_secs(60);
fn files() -> Option<PathBuf> {
    mt_engine::present::textlog::set_enabled(true);
    oracle_env::gamefiles()
}

struct Queue {
    scene: Option<Scene>, decision: Option<Decision>, events: VecDeque<Event>, intents: VecDeque<Intent>,
    live: MouseState, warps: usize, ui_keys: usize, raw_keys: usize, clicks: usize, polls: usize,
    stop_poll: Option<usize>, started: Instant, frame: Option<megatraveller::pipeline::Frame>,
}
impl Default for Queue {
    fn default() -> Self {
        Self { scene: None, decision: None, events: VecDeque::new(), intents: VecDeque::new(), live: pointer::live_mouse(None),
            warps: 0, ui_keys: 0, raw_keys: 0, clicks: 0, polls: 0, stop_poll: None, started: Instant::now(), frame: None }
    }
}
type Shared = Rc<RefCell<Queue>>;
struct MouseInput(Shared);
#[derive(Debug)] struct IdleComplete;
impl Input for MouseInput {
    fn wants_decision(&self) -> bool { true }
    fn decide(&mut self, d: &Decision) { assert!(self.0.borrow().started.elapsed() < LIMIT); self.0.borrow_mut().decision = Some(d.clone()); }
    fn scene(&mut self, s: &dyn Any) { assert!(self.0.borrow().started.elapsed() < LIMIT); if let Some(s) = s.downcast_ref::<Scene>() { self.0.borrow_mut().scene = Some(s.clone()); } }
    fn intent(&mut self) -> Option<Box<dyn Any>> { self.0.borrow_mut().intents.pop_front().map(|i| Box::new(i) as Box<dyn Any>) }
    fn next_key(&mut self) -> u16 { panic!("mouse golden reached an unclicked blocking keyboard wait") }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        let mut q = self.0.borrow_mut();
        assert!(q.started.elapsed() < LIMIT, "mouse session exceeded 60 seconds");
        if q.stop_poll == Some(q.polls) { std::panic::resume_unwind(Box::new(IdleComplete)); }
        q.polls += 1;
        let e = q.events.pop_front().unwrap_or(Event::Idle);
        if e == Event::Idle { clock.wait_tick(); } e
    }
    fn mouse(&mut self) -> MouseState { self.0.borrow().live }
    fn set_mouse_position(&mut self, _: i16, _: i16) { self.0.borrow_mut().warps += 1; }
    fn present(&mut self, rgb: &[u32]) { if rgb.len() == 64000 { self.0.borrow_mut().frame = Some(megatraveller::pipeline::Frame::from_rgb(rgb)); } }
}

struct Session { g: Ground, q: Shared, layout: Layout, ui: UiState }
impl Session {
    fn load(dir: &Path, name: &str) -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios").join(format!("{name}.toml"));
        let prepared = scenario::load(&path, dir).unwrap(); let mut g = prepared.game;
        if g.squad.is_none() {
            let compendium = mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../data/npcs.toml")).unwrap();
            g.squad = Some(squad::State::with_compendium(compendium));
        }
        let q: Shared = Rc::default(); g.input = Box::new(MouseInput(q.clone()));
        g.clock = Box::new(Counter::at(1234)); scenario::attach_scene(&mut g);
        g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), dir.into(), prepared.extensions))); g.tool_poll();
        let mut layout = Layout::default(); components::register(&mut layout, Default::default());
        let mut s = Self { g, q, layout, ui: UiState::default() }; s.observe(); s
    }
    fn scene(&self) -> Scene { self.q.borrow().scene.clone().expect("sim must publish a Scene") }
    fn observe(&mut self) {
        let previous = self.q.borrow().scene.clone();
        render::redraw(&mut self.g);
        let scene = self.scene(); assert_eq!(scene.screen, ScreenKind::Ground); assert!(scene.squad.active);
        controls::observe(&scene, previous.as_ref(), &mut self.ui);
        if scene.squad.menu.is_none() { self.ui.menu = None; self.ui.menu_host = None; }
        let follow_up = panels::combat_bar::observe(&scene, &mut self.ui);
        self.apply_intents(follow_up);
        self.layout.arrange_for(&scene, &self.ui, WINDOW); controls::prepare(&self.layout, &mut self.ui);
        if self.q.borrow().decision.is_none() { play::turns::wait(&mut self.g); play::commands::wait(&mut self.g); }
    }
    fn rect(&self, id: ComponentId) -> Rect {
        match self.layout.host(id).expect("registered component") { Host::Dock(r) => r, _ => panic!("expected docked component") }
    }
    fn local_pixel(&self, id: ComponentId, at: (i32, i32)) -> (i32, i32) {
        let r = self.rect(id); assert!(Rect::new(0, 0, r.w, r.h).contains(at)); (r.x + at.0, r.y + at.1)
    }
    fn cell_pixel(&self, cell: Cell) -> Option<(i32, i32)> {
        let scene = self.scene(); let r = self.rect("Viewport");
        let c = Camera::new(&scene, &self.ui, (r.w, r.h));
        let at = (((cell.1 as f32 * 16.0 + 8.0 - c.left) * c.zoom).round() as i32,
                  ((cell.0 as f32 * 16.0 + 8.0 - c.top) * c.zoom).round() as i32);
        Rect::new(4, 4, r.w.saturating_sub(8), r.h.saturating_sub(8)).contains(at).then_some((r.x + at.0, r.y + at.1))
    }
    fn pointer(&mut self, event: UiEvent, pixel: (i32, i32)) {
        if matches!(event, UiEvent::Key(_)) { self.q.borrow_mut().ui_keys += 1; panic!("keyboard event in mouse script"); }
        let scene = self.scene();
        let mapped = self.layout.hit_mapped(&scene, None, pixel, |r| r);
        let (id, at) = self.ui.capture.map(|id| { let r = self.rect(id); (id, (pixel.0 - r.x, pixel.1 - r.y)) })
            .or(mapped).expect("script pixel must hit a component");
        self.q.borrow_mut().live = pointer::live_mouse(Some((id, self.layout.placement(id).unwrap().origin, at)));
        if matches!(event, UiEvent::Down(_)) { self.ui.capture = Some(id); }
        let intents = controls::dispatch(&self.layout, id, &scene, &mut self.ui, &event, at);
        self.apply_intents(intents);
        if matches!(event, UiEvent::Up(_)) { self.ui.capture = None; }
    }
    fn apply_intents(&mut self, intents: Vec<Intent>) {
        for intent in intents {
            match intent {
                Intent::Raw(e) => {
                    let mut q = self.q.borrow_mut();
                    if matches!(e, Event::Key(_) | Event::KeyUp(_)) { q.raw_keys += 1; panic!("raw key without a clicked Decision option: {e:?}"); }
                    q.events.push_back(e);
                }
                _ => {
                    let forward = matches!(intent, Intent::Hover(t) if t != self.ui.hover) || matches!(intent, Intent::OpenMenu { .. } | Intent::CloseMenu);
                    if !self.ui.apply(intent) || forward { self.q.borrow_mut().intents.push_back(intent); }
                }
            }
        }
    }
    fn pass(&mut self) {
        for _ in 0..4 {
            let intents = squad::poll(&mut self.g); round::pass(&mut self.g, &intents); self.observe();
            if self.q.borrow().intents.is_empty() { return }
        }
        panic!("clicked menu transaction did not settle");
    }
    fn click(&mut self, pixel: (i32, i32)) {
        self.pointer(UiEvent::Move, pixel); self.pointer(UiEvent::Down(Button::Left), pixel); self.pointer(UiEvent::Up(Button::Left), pixel);
        self.q.borrow_mut().clicks += 1; self.pass();
    }
    fn target(&mut self, min: i32, max: i32) -> Cell {
        let scene = self.scene(); let from = traveller(&scene, scene.squad.selected);
        let mut examined = 0;
        for row in 0..scene.map.rows as i16 { for col in 0..scene.map.cols as i16 {
            let cell = (row, col); let distance = round::squares(from, cell);
            if !(min..=max).contains(&distance) { continue; }
            let Some(bytes) = scene.map.cell(row as i32, col as i32) else { continue };
            if bytes[0] >> 1 != 0 || bytes[2] & 63 != 0 { continue; }
            let Some(pixel) = self.cell_pixel(cell) else { continue };
            examined += 1;
            self.pointer(UiEvent::Move, pixel); self.pass();
            let preview = self.scene(); let path = &preview.squad.path;
            if path.last().is_some_and(|&(end, _)| end == cell) && path.iter().all(|&(_, kind)| kind == PATH_WALK) {
                return cell;
            }
        } }
        panic!("no visible passable target {min}..={max} squares away from {from:?}; examined {examined}, rect {:?}, camera {:?}, hover {:?}, path {:?}", self.rect("Viewport"), self.ui.camera, self.scene().squad.hover, self.scene().squad.path)
    }
    fn check(&self) {
        let q = self.q.borrow();
        assert_eq!((q.ui_keys, q.raw_keys), (0, 0), "zero keyboard input, including key-up");
        assert!(!self.g.input.closed()); assert!(q.started.elapsed() < LIMIT);
        assert!(q.intents.is_empty() && q.events.is_empty(), "no unconsumed click input");
        assert!(q.decision.is_some(), "real sim Decision observed");
    }
}
fn traveller(s: &Scene, who: u8) -> Cell {
    if s.combat { let w = &s.walkers[who as usize]; (w.x() / 2, w.y() / 2) } else { (s.pos.0 / 2, s.pos.1 / 2) }
}

#[test]
fn mouse_only_ground_and_combat_golden() {
    let Some(dir) = files() else { return }; let started = Instant::now();
    let mut s = Session::load(&dir, "efate_quiet"); assert!(!s.scene().squad.encounter);
    let from = traveller(&s.scene(), 0); let ground_to = s.target(6, 9);
    assert!(round::squares(from, ground_to) >= 6); assert!(s.scene().squad.path.len() >= 6);
    s.click(s.cell_pixel(ground_to).unwrap()); assert_eq!(traveller(&s.scene(), 0), ground_to, "one click walks the whole route");
    assert_eq!(s.q.borrow().clicks, 1); assert!(!s.scene().squad.encounter); s.check();

    let mut s = Session::load(&dir, "efate_broken"); assert!(s.scene().squad.encounter);
    s.click(s.local_pixel("Party strip", (60, 20))); assert_eq!(s.scene().squad.selected, 1);
    s.click(s.local_pixel("Party strip", (20, 20))); assert_eq!(s.scene().squad.selected, 0);
    let before = s.scene(); let to = s.target(1, 1);
    s.click(s.cell_pixel(to).unwrap()); let moved = s.scene();
    assert_eq!(traveller(&moved, 0), to); assert_ne!(traveller(&before, 0), to);
    assert!(moved.squad.mp[0] < before.squad.mp[0], "walking spends MP");
    let enemy = moved.npcs.iter().enumerate().filter(|(_, n)| n.present() && (n.attitude() & 3 == 0 || n.was_hit()))
        .find_map(|(i, n)| s.cell_pixel((n.x() / 2, n.y() / 2)).map(|p| (i as u8 + 1, p))).expect("visible hostile");
    let ammo = moved.party.hand(0, 0).unwrap().2;
    let seq = moved.squad.fx.seq; let logged = moved.squad.log_n;
    s.click(enemy.1); let fired = s.scene();
    assert_eq!(fired.walkers[0].target(), enemy.0 as i8, "the enemy click selects its native target");
    assert!(fired.squad.fx.seq != seq || fired.squad.log_n > logged && fired.squad.log.iter().any(|l| l.contains("fires")), "click NPC {} advances fire: {:?}", enemy.0, fired.squad.log);
    assert!(!fired.squad.pips[0][1], "shot spends Attack");
    assert!(fired.party.hand(0, 0).unwrap().2 < ammo, "one enemy click fires the weapon");
    let who = (0..5u8).find(|&i| fired.party.alive(i) && fired.squad.pips[i as usize].iter().any(|&p| p)).expect("another open turn");
    s.click(s.local_pixel("Party strip", (20 + 40 * who as i32, 20)));
    let before = s.scene(); let r = panels::party_strip::O_RECT;
    s.click(s.local_pixel("Party strip", (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2)));
    let after = s.scene();
    assert!(after.squad.round > before.squad.round || after.squad.to_act != before.squad.to_act || after.squad.queue != before.squad.queue, "End Turn advances the round/turn queue");
    assert!(after.squad.round > before.squad.round || after.squad.pips[who as usize] == [false, false]);
    s.check(); assert!(started.elapsed() < LIMIT, "whole golden must finish in 60 seconds");
    eprintln!("mouse golden: Efate {from:?} -> {ground_to:?}; combat selected/moved/fired/ended; zero keyboard events; {:?}", started.elapsed());
}

#[test]
fn modern_1990_pad_region_never_walks_while_held() {
    let Some(dir) = files() else { return }; let mut s = Session::load(&dir, "efate_quiet");
    let start = traveller(&s.scene(), 0);
    for row in [145, 155, 165] { for col in [205, 226, 247] {
        let pixel = s.local_pixel("Side panel", (col - components::SIDE_RECT.x, row - components::SIDE_RECT.y));
        s.pointer(UiEvent::Move, pixel); s.pointer(UiEvent::Down(Button::Left), pixel);
        let live = s.q.borrow().live; assert_eq!((live.buttons, live.col, live.row), (0, col as i16, row as i16));
        { let mut q = s.q.borrow_mut(); q.stop_poll = Some(q.polls + 24); }
        let stopped = catch_unwind(AssertUnwindSafe(|| main_loop::run(&mut s.g))).expect_err("bounded idle probe");
        if !stopped.is::<IdleComplete>() { std::panic::resume_unwind(stopped); }
        s.q.borrow_mut().stop_poll = None;
        assert_eq!((s.g.mem.x() / 2, s.g.mem.y() / 2), start, "held pad at ({col}, {row}) moved the party");
        s.pointer(UiEvent::Up(Button::Left), pixel); s.g.poll();
        if s.g.mem.key() == b'n' { input::special_key(&mut s.g, mt1::ground::state::HOTSPOTS_GROUND, mt1::ground::state::HOTSPOTS_GROUND_N); }
        assert!(!(0x47..=0x51).contains(&s.g.mem.key()), "pad release synthesized a movement scan");
        s.observe();
    } }
    let live = s.q.borrow().live; let warps = s.q.borrow().warps;
    s.g.input.set_mouse_position(0, 0);
    assert_eq!(s.q.borrow().warps, warps + 1); assert_eq!(s.q.borrow().live, live, "sim warp cannot move the pointer");
    assert_eq!(traveller(&s.scene(), 0), start); s.check();
}

#[test]
fn press_drag_release_does_not_walk_after_lane_b_merge() {
    let Some(dir) = files() else { return }; let mut s = Session::load(&dir, "efate_quiet");
    let from = traveller(&s.scene(), 0); let to = s.target(6, 9);
    let a = s.cell_pixel(from).unwrap(); let b = s.cell_pixel(to).unwrap();
    s.pointer(UiEvent::Move, a); s.pointer(UiEvent::Down(Button::Left), a);
    s.pointer(UiEvent::Move, b); s.pass(); assert_eq!(traveller(&s.scene(), 0), from);
    s.pointer(UiEvent::Up(Button::Left), b);
    assert!(!s.q.borrow().intents.iter().any(|i| matches!(i, Intent::Verb { verb: Verb::Move, .. })), "drag release emitted a walk");
    s.pass(); assert_eq!(traveller(&s.scene(), 0), from); s.check();
}

fn bind_pending_decision(ui: &mut UiState, d: &Decision) {
    megatraveller::mouse::decision::Store::default().record(ui, d);
}
fn pending_option_click(scene: &Scene, d: &Decision, option: &mt_engine::play::Option, frame: Option<&megatraveller::pipeline::Frame>) -> Vec<Event> {
    use mt_engine::io::Dir;
    let dir = files().expect("BYO assets");
    let assets = panels::assets::Assets::load(&mut Dir(dir));
    let mut layout = Layout::default(); components::register(&mut layout, std::sync::Arc::new(assets));
    let mut ui = UiState::default(); bind_pending_decision(&mut ui, d);
    if let Some(f) = frame { *ui.frame = f.clone(); }
    layout.arrange_for(scene, &ui, WINDOW); controls::prepare(&layout, &mut ui);
    let hosts = layout.draw_host(scene, &ui, None);
    let (id, rect, local) = match d.kind {
        mt_engine::play::Kind::Ground => {
            let (id, rect, surface) = hosts.into_iter().find(|(id, _, _)| *id == "Commands").expect("the Commands component is registered and visible");
            let mut rows = std::collections::BTreeMap::<i64, Vec<(i64, u8)>>::new();
            for run in &surface.text.visible { for (i, b) in run.bytes.iter().enumerate() { rows.entry(run.y).or_default().push((run.x + i as i64 * 6, *b)); } }
            let local = rows.into_iter().find_map(|(y, mut chars)| {
                chars.sort_by_key(|&(x, _)| x);
                let text: String = chars.iter().map(|&(_, b)| b as char).collect();
                text.find(&option.label).map(|i| (chars[i].0 as i32 + 1, y as i32 + 3))
            }).unwrap_or_else(|| panic!("button {:?} has no visible label", option.label));
            (id, rect, local)
        }
        mt_engine::play::Kind::Ack => {
            let (id, rect, _) = hosts.into_iter().find(|(id, _, _)| *id == "Picture").expect("the Picture is visible");
            (id, rect, (160, 100))
        }
        _ => {
            let i = d.options.iter().position(|o| o.id == option.id).unwrap();
            let rows = megatraveller::mouse::decision::native_rows(&ui.frame.text.visible, d);
            let (_, r) = rows.iter().find(|(k, _)| *k == i).unwrap_or_else(|| panic!("option {:?} has no drawn row in the presented frame ({} visible runs; rows {:?})", option.label, ui.frame.text.visible.len(), rows));
            let at = (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2);
            hosts.into_iter().find_map(|(id, rect, _)| match layout.placement(id)?.origin {
                panels::Origin::Classic1990(o) if o.contains(at) => Some((id, rect, (at.0 - o.x, at.1 - o.y))), _ => None,
            }).unwrap_or_else(|| panic!("no visible 1990 host holds the row of {:?} at {at:?}", option.label))
        }
    };
    let pixel = (rect.x + local.0, rect.y + local.1); let mut events = Vec::new();
    for event in [UiEvent::Move, UiEvent::Down(Button::Left), UiEvent::Up(Button::Left)] {
        let (hit, at) = layout.hit_mapped(scene, None, pixel, |r| r).expect("button pixel hit"); assert_eq!(hit, id);
        for intent in controls::dispatch(&layout, hit, scene, &mut ui, &event, at) {
            match intent { Intent::Raw(e) => events.push(e), other => { assert!(ui.apply(other), "unexpected option intent {other:?}"); } }
        }
    }
    events
}

struct DecisionInput {
    mouse: MouseInput, labels: VecDeque<String>, seen: Rc<RefCell<Vec<Decision>>>, pending: Option<Decision>,
}
impl DecisionInput {
    fn new(q: Shared, labels: &[&str], seen: Rc<RefCell<Vec<Decision>>>) -> Self {
        Self { mouse: MouseInput(q), labels: labels.iter().map(|s| s.to_string()).collect(), seen, pending: None }
    }
    fn click(&mut self) {
        let Some(d) = self.pending.take() else { return };
        let Some(label) = self.labels.pop_front() else {
            let i = d.options.iter().position(|o| Some(&o.id) == d.default.as_ref()).unwrap_or(0);
            let events = megatraveller::mouse::decision::option(&d, i).into_iter().filter_map(|i| if let Intent::Raw(e) = i { Some(e) } else { None }).collect::<Vec<_>>();
            assert!(!events.is_empty(), "unexpected Decision after the last scripted click: {}.{}", d.context.program, d.context.routine);
            self.mouse.0.borrow_mut().events.extend(events); return
        };
        let option = d.options.iter().find(|o| o.label.eq_ignore_ascii_case(&label) || o.id == label)
            .unwrap_or_else(|| panic!("{label:?} missing from {}.{}", d.context.program, d.context.routine));
        assert!(option.enabled, "button {label} is disabled");
        let (scene, frame) = { let q = self.mouse.0.borrow(); (q.scene.clone().unwrap_or_else(|| Scene::program(ScreenKind::Startup)), q.frame.clone()) };
        let i = d.options.iter().position(|o| o.id == option.id).unwrap();
        if matches!(d.kind, mt_engine::play::Kind::Menu | mt_engine::play::Kind::YesNo | mt_engine::play::Kind::List)
            && !frame.as_ref().is_some_and(|f| megatraveller::mouse::decision::native_rows(&f.text.visible, &d).iter().any(|(k, _)| *k == i)) {
            let events: Vec<Event> = megatraveller::mouse::decision::option(&d, i).into_iter().filter_map(|i| if let Intent::Raw(e) = i { Some(e) } else { None }).collect();
            audit_option_events(option, &events); self.mouse.0.borrow_mut().events.extend(events); return
        }
        let events = pending_option_click(&scene, &d, option, frame.as_ref());
        audit_option_events(option, &events);
        let mut q = self.mouse.0.borrow_mut(); q.clicks += 1; q.events.extend(events);
    }
}
impl Input for DecisionInput {
    fn wants_decision(&self) -> bool { true }
    fn decide(&mut self, d: &Decision) {
        self.mouse.decide(d);
        if self.pending.is_some() || self.mouse.0.borrow().events.iter().any(|e| matches!(e, Event::Key(_) | Event::Mouse(_))) { return; }
        self.seen.borrow_mut().push(d.clone()); self.pending = Some(d.clone());
    }
    fn scene(&mut self, s: &dyn Any) { self.mouse.scene(s); }
    fn present(&mut self, rgb: &[u32]) { self.mouse.present(rgb); }
    fn key_pending(&mut self) -> bool { self.click(); self.mouse.0.borrow().events.iter().any(|e| matches!(e, Event::Key(_))) }
    fn next_key(&mut self) -> u16 {
        self.click();
        loop { match self.mouse.0.borrow_mut().events.pop_front().expect("no clicked option key") {
            Event::Key(k) => return k, Event::KeyUp(_) => {}, e => panic!("unexpected BIOS input {e:?}"),
        } }
    }
    fn poll(&mut self, c: &mut dyn Clock) -> Event { self.click(); self.mouse.poll(c) }
    fn mouse(&mut self) -> MouseState { self.mouse.mouse() }
    fn set_mouse_position(&mut self, col: i16, row: i16) { self.mouse.set_mouse_position(col, row); }
}
fn audit_option_events(option: &mt_engine::play::Option, events: &[Event]) {
    let makes = |es: &[Event]| es.iter().filter_map(|e| if let Event::Key(k) = e { Some(*k) } else { None }).collect::<Vec<_>>();
    assert_eq!(makes(events), makes(&option.input), "keys must originate in the clicked Decision option");
    let mut held = Vec::new();
    for e in events { match e {
        Event::Key(k) => { assert!(!held.contains(k), "press without break"); held.push(*k); }
        Event::KeyUp(k) => { let i = held.iter().position(|v| v == k).expect("break without press"); held.remove(i); }
        _ => {}
    } }
    assert!(held.is_empty(), "mouse::decision::press must release every synthesized key");
}

#[test]
fn title_start_game_and_get_started_quick_are_clickable() {
    assert!(megatraveller::look::Look::default().mouse);
    assert!(toml::from_str::<megatraveller::look::Look>("mouse = false").unwrap().mouse);
    use mt_engine::{data::{exepack, ly, pic, Variant}, io::{Dir, Files, Overlay}};
    use mt1::screens::{cg::Screens, title::{self, TcRand, Title}};
    let Some(dir) = files() else { return }; let started = Instant::now();
    let files = Dir(dir.clone()); let read = |name| files.read(name).unwrap();
    let exe = exepack::unpack(&read("STARTUP.EXE")).unwrap().exe;
    let mut title = Title::new(&exe, ly::decode(&read("TITLE.LY")).unwrap(), ly::decode(&read("FONT.LY")).unwrap(),
        pic::decode(Variant::Mt1, &read("TIT.PIC")).unwrap(), pic::decode(Variant::Mt1, &read("ZOD.PIC")).unwrap()).unwrap();
    let q: Shared = Rc::default(); let seen: Rc<RefCell<Vec<Decision>>> = Rc::default();
    let mut input = DecisionInput::new(q.clone(), &["Start game", "Get Started Quick"], seen.clone());
    let mut clock = Counter::at(1234); let mut rng = TcRand { seed: 1234 };
    let dac = mt_engine::present::palette::ega_dac();
    assert_eq!(title.run(&mut || rng.next(), &mut clock, &mut input,
        |t, i| { i.present(&t.fb.rgb(t.display, &dac)); i.scene(&Scene::program(ScreenKind::Startup)) }), title::Exit::Game);
    let mut overlay = Overlay::new(Some(&dir)); overlay.hide("UCHARS.DAT");
    let mut cg = Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe, ly::decode(&read("FONT.LY")).unwrap(),
        ly::decode(&read("CG.LY")).unwrap(), Box::new(input), Box::new(clock), Box::new(overlay)).unwrap();
    cg.snapshots = true;
    assert_eq!(mt1::screens::cg::run(&mut cg).0, mt1::chargen::pool::Exit::Quick);
    let decisions = seen.borrow();
    assert_eq!((decisions[0].context.program.as_str(), decisions[0].context.routine.as_str()), ("title", "0297"));
    assert_eq!(decisions.last().unwrap().context.program, "cg");
    assert_eq!((q.borrow().ui_keys, q.borrow().raw_keys), (0, 0), "zero scripted keyboard events");
    assert_eq!(q.borrow().clicks, 2); assert!(!cg.input.closed()); assert!(started.elapsed() < LIMIT);
}

#[test]
fn start_new_game_click_reaches_party_disk_choices() {
    use mt_engine::{data::{exepack, ly}, io::{Dir, Files, Overlay}};
    use mt1::screens::cg::Screens;
    assert!(megatraveller::look::Look::default().mouse);
    assert!(toml::from_str::<megatraveller::look::Look>("mouse = false").unwrap().mouse);
    let Some(dir) = files() else { return };
    let files = Dir(dir.clone()); let read = |name| files.read(name).unwrap();
    let q: Shared = Rc::default(); let seen: Rc<RefCell<Vec<Decision>>> = Rc::default();
    let input = DecisionInput::new(q.clone(), &["Start New Game", "back", "Get Started Quick"], seen.clone());
    let mut overlay = Overlay::new(Some(&dir)); overlay.hide("UCHARS.DAT");
    let mut cg = Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe, ly::decode(&read("FONT.LY")).unwrap(),
        ly::decode(&read("CG.LY")).unwrap(), Box::new(input), Box::new(Counter::at(1234)), Box::new(overlay)).unwrap();
    cg.snapshots = true;
    assert_eq!(mt1::screens::cg::run(&mut cg).0, mt1::chargen::pool::Exit::Quick);
    let decisions = seen.borrow();
    assert!(decisions[1].options.iter().any(|o| o.label == "Create Party Disk"), "Start New Game advanced to the native disk menu");
    assert_eq!((q.borrow().clicks, q.borrow().ui_keys, q.borrow().raw_keys), (2, 0, 0), "two drawn rows clicked; Cancel is the keyboard's");
}

#[test]
fn toolbar_buttons_open_their_native_menus_and_back_is_clickable() {
    use mt_engine::play::Kind;
    let Some(dir) = files() else { return }; let mut s = Session::load(&dir, "efate_quiet");
    for (id, table, row) in [("cmd:game", "45FE", "Sound off"), ("cmd:party", "44D2", "Regroup"), ("cmd:inventory", "4568", "Take"), ("cmd:vehicle", "472A", "Status"), ("cmd:orders", "", "")] {
        let d = play::commands::ground(&s.g); let option = d.options.iter().find(|o| o.id == id).unwrap();
        if !option.enabled { assert!(pending_option_click(&s.scene(), &d, option, None).is_empty()); continue; }
        let events = pending_option_click(&s.scene(), &d, option, None); audit_option_events(option, &events);
        let seen: Rc<RefCell<Vec<Decision>>> = Rc::default(); let mut queue = Queue::default(); queue.events.extend(events); queue.scene = Some(s.scene());
        let q = Rc::new(RefCell::new(queue));
        s.g.input = Box::new(DecisionInput::new(q.clone(), &[row], seen.clone()));
        s.g.poll(); let key = s.g.mem.key(); let now = s.g.clock.now();
        assert_eq!(input::dispatch(&mut s.g, key, now, 0), input::Flow::Continue);
        let decisions = seen.borrow(); let menu = decisions.first().expect("toolbar click must open a menu");
        assert_eq!(menu.kind, Kind::Menu); assert_eq!(menu.context.routine, "AF79");
        assert_eq!(menu.context.table.as_deref(), Some(table), "{id} opened the wrong menu");
        assert_eq!(q.borrow().clicks, 1, "{row} was clicked on its drawn row");
        assert_eq!((q.borrow().ui_keys, q.borrow().raw_keys), (0, 0), "zero scripted keyboard events"); assert!(!s.g.input.closed());
        while !q.borrow().events.is_empty() { s.g.poll(); }
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
