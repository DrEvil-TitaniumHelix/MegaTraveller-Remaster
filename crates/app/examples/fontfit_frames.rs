use crate::window;
use megatraveller::{scenario, look::Look, mcp::launch, panels::{self, assets::Assets, Layout, UiState, UiEvent, Dock}, pipeline::Frame};
use mt_engine::{io::{Dir, Event, Files, Input}, play::Decision, sim::tick::{Clock, HZ}};
use mt1::ui::{Intent, Scene, ScreenKind};
use serde_json::{json, Value};
use std::{cell::RefCell, collections::VecDeque, io::{BufRead, Write}, path::{Path, PathBuf}, rc::Rc, sync::Arc, time::{Duration, Instant}};
use winit::keyboard::{KeyCode, ModifiersState};

#[derive(Debug)]
struct Stop;
#[derive(Default)]
struct Time { ms: u64, ticks: u32, offset: u32, paused: bool }
#[derive(Clone)]
struct VirtualClock(Rc<RefCell<Time>>);
impl VirtualClock {
    fn advance(&self, ms: u64) { let mut t = self.0.borrow_mut(); let before = (t.ms as f64 * HZ / 1000.) as u32; t.ms += ms; if !t.paused { t.ticks = t.ticks.wrapping_add((t.ms as f64 * HZ / 1000.) as u32 - before); } }
    fn ms(&self) -> u64 { self.0.borrow().ms }
}
impl Clock for VirtualClock {
    fn now(&mut self) -> u32 { let t = self.0.borrow(); t.ticks.wrapping_add(t.offset) }
    fn ticks(&mut self) -> u32 { self.0.borrow().ticks }
    fn set(&mut self, now: u32) { let mut t = self.0.borrow_mut(); t.offset = now.wrapping_sub(t.ticks); }
    fn wait_tick(&mut self) { self.advance(55); }
    fn busy_wait(&mut self) { self.advance(55); }
}

#[allow(dead_code)]
struct Driver {
    scene: Option<Scene>, ui: UiState, layout: Layout, assets: Arc<Assets>, look: Look, size: [u32; 2],
    events: VecDeque<Event>, intents: VecDeque<Intent>, store: megatraveller::mouse::decision::Store,
    replay: panels::replay::Replay, epoch: Instant, clock: VirtualClock,
    commands: Box<dyn BufRead>, replies: Box<dyn Write>, waiting: Option<u64>,
    save_dir: PathBuf, last_rgb: Vec<u32>, frames: u32,
}
impl Driver {
    fn new(dir: &Path, look_path: &Path, size: [u32; 2], clock: VirtualClock, commands: Box<dyn BufRead>, replies: Box<dyn Write>, save_dir: PathBuf) -> Self {
        let assets = Arc::new(Assets::load(&mut Dir(dir.into())));
        let mut layout = Layout::default(); panels::components::register(&mut layout, assets.clone());
        let mut look = Look::load_for_layout(look_path, &layout);
        look.font = match std::env::var("MT_FONTFIT_FACE").as_deref() {
            Ok("original") => megatraveller::look::FontChoice::Original,
            Ok(n) => megatraveller::look::FontChoice::Builtin(n.parse().unwrap_or(0)), _ => Look::default().font,
        }; look.bold = false;
        let mut ui = UiState { fixed_screen: true, ..Default::default() };
        look.layout.restore(&layout, &mut ui);
        for id in layout.ids() { if let Some(size) = look.layout.components.get(id).and_then(|c| c.size) { layout.set_size(id, size); } }
        Self { scene: None, ui, layout, assets, look, size, events: VecDeque::new(), intents: VecDeque::new(), store: Default::default(),
            replay: Default::default(), epoch: Instant::now(), clock, commands, replies, waiting: None, save_dir, last_rgb: vec![], frames: 0 }
    }
    fn now(&self) -> Instant { self.epoch + Duration::from_millis(self.clock.ms()) }
    fn show(&mut self, e: panels::replay::Entry) {
        let s = e.scene;
        let walk = self.ui.walk.borrow_mut().observe(&s);
        self.ui.frame = e.frame;
        panels::controls::observe(&s, self.scene.as_ref(), &mut self.ui);
        self.ui.replay = e.fx.then_some((s.squad.fx.seq, 0.));
        match (&s.squad.menu, self.ui.menu) {
            (Some(m), None) => self.ui.menu = Some((m.target, (0, 0))),
            (None, Some(_)) => { self.ui.menu = None; self.ui.menu_host = None; }, _ => {}
        }
        let combat = panels::combat_bar::observe(&s, &mut self.ui);
        self.scene = Some(s); self.apply(walk); self.apply(combat);
    }
    fn pulse(&mut self) {
        let (entries, progress) = self.replay.tick(self.now());
        for e in entries { self.show(e); } self.ui.replay = progress;
        if let Some(f) = self.replay.take_held() { *self.ui.frame = f; }
    }
    fn prepare(&mut self) {
        if let Some(s) = &self.scene {
            let m = panels::render::Metrics::for_ui(self.size, &self.look, Some(s), &self.ui);
            let mut ui = self.ui.clone(); if m.classic { ui.preset = mt1::ui::Preset::Classic; }
            self.layout.arrange_for(s, &ui, m.logical);
            panels::controls::prepare(&self.layout, &mut self.ui);
        }
    }
    fn dispatch(&mut self, id: &'static str, key: u16) -> Vec<Intent> {
        let fallback = Scene::program(ScreenKind::Picture);
        panels::controls::dispatch(&self.layout, id, self.scene.as_ref().unwrap_or(&fallback), &mut self.ui, &UiEvent::Key(key), (0, 0))
    }
    fn component(&mut self, id: &'static str, mut intents: Vec<Intent>) {
        if matches!(id, "Commands" | "Choices" | "Combat controls" | "Text input" | "Continue" | "Flight") {
            let raw: Vec<_> = intents.iter().filter_map(|i| if let Intent::Raw(e) = i { Some(*e) } else { None }).collect();
            self.store.queue(&mut self.ui, &raw); intents.retain(|i| !matches!(i, Intent::Raw(_)));
        }
        self.apply(intents);
    }
    fn apply(&mut self, mut intents: Vec<Intent>) {
        if let Some(raw) = megatraveller::mouse::decision::released_route(&intents) {
            self.store.queue(&mut self.ui, &raw); intents.retain(|i| !matches!(i, Intent::Raw(_)));
        }
        megatraveller::mouse::walk::retarget(&mut self.intents, &intents);
        for i in intents {
            match i {
                Intent::Raw(e) => self.events.push_back(e),
                Intent::Plus(a) => self.ui.plus.apply(a),
                _ => {
                    let forward = matches!(i, Intent::Hover(t) if t != self.ui.hover) || matches!(i, Intent::OpenMenu { .. } | Intent::CloseMenu);
                    if !self.ui.apply(i) || forward { self.intents.push_back(i); }
                }
            }
        }
    }
    fn route(&mut self, key: u16) {
        self.prepare();
        if self.ui.plus.open { let i = self.dispatch("Plus", key); self.apply(i); return; }
        if panels::textbox::active(&self.ui) { let i = self.dispatch("Text input", key); self.component("Text input", i); return; }
        if self.scene.is_none() { self.events.push_back(Event::Key(key)); return; }
        if megatraveller::mouse::decision::modal(&self.ui) {
            if self.ui.decision.as_ref().is_some_and(|d| panels::choices::handles_key(d, key)) {
                let i = self.dispatch("Choices", key); self.component("Choices", i);
            } else { self.events.push_back(Event::Key(key)); }
            return;
        }
        if matches!(key, 13 | 27 | 32) && self.replay.active() {
            if let Some(e) = self.replay.flush() { self.show(e); }
            self.ui.replay = None; if let Some(f) = self.replay.take_held() { *self.ui.frame = f; }
            if !self.scene.as_ref().is_some_and(|s| megatraveller::mouse::walk::cancel_after_replay(UiEvent::Key(key), s, &self.ui, &self.intents)) { return; }
        }
        let s = self.scene.as_ref().unwrap();
        if let Some(i) = panels::keyboard::route(s, &mut self.ui, key) { self.apply(i); return; }
        let focused = self.ui.menu.and_then(|_| self.layout.ids().into_iter().rev().find(|id| self.layout.placement(id).is_some_and(|p| matches!(p.dock, Dock::Float(_))) && self.layout.visible(id, s)));
        if let Some(id) = focused { let i = self.dispatch(id, key); if !i.is_empty() { self.component(id, i); return; } }
        let s = self.scene.as_ref().unwrap();
        if key == 27 && self.ui.menu.is_none() {
            let dropped = megatraveller::mouse::walk::drop_moves(&mut self.intents);
            let cancel = self.ui.walk.borrow_mut().cancel(s, (0, 0));
            if let Some(i) = cancel { self.apply(i); return; } if dropped { return; }
        }
        if key != 27 { self.ui.walk.borrow_mut().reset(); }
        if matches!(key, 13 | 27 | 32 | 0x13b..=0x144) { self.events.push_back(Event::Key(key)); return; }
        if s.screen == ScreenKind::Ground && !self.layout.is_classic() {
            if matches!(key, 76 | 108) { self.apply(vec![Intent::LogExpand]); return; }
            if matches!(key, 91 | 93) { self.apply(vec![Intent::LogScroll(if key == 91 { 1 } else { -1 })]); return; }
        }
        let id = if key == mt_engine::io::keys::HOME { Some("Viewport") } else { self.layout.ids().into_iter().find(|id| self.layout.placement(id).is_some_and(|p| p.dock == Dock::Fill) && self.layout.visible(id, s)) };
        let i = id.map_or_else(Vec::new, |id| self.dispatch(id, key));
        if i.is_empty() { self.events.push_back(Event::Key(key)); } else { self.component(id.unwrap(), i); }
    }
    fn key(&mut self, name: &str, down: bool) -> Result<(), String> {
        let (code, text) = named_key(name)?;
        if down && text.as_deref() == Some("?") && !self.ui.plus.open {
            use mt1::ui::scene::PlusAction;
            self.apply(vec![Intent::Plus(PlusAction::Open), Intent::Plus(PlusAction::Tab(1))]); return Ok(());
        }
        let mut keys = VecDeque::new();
        if down {
            let logical = winit::keyboard::Key::Character(text.clone().unwrap_or_default().into());
            let supplied = text.as_deref().filter(|t| !matches!(*t, "1" | "2" | "3" | "4" | "5"));
            let text = window::key_text(supplied, &logical, ModifiersState::empty(), false);
            if let Some(a) = window::key_press(&mut keys, code, text, &self.look, false, false, false)? { a.apply(&mut self.look); }
            for key in keys { self.route(key); }
        } else if !self.ui.plus.open && !panels::textbox::active(&self.ui) {
            window::enqueue(&mut keys, code, text.as_deref())?;
            if let Some(key) = keys.pop_front() { self.events.push_back(Event::KeyUp(key)); }
        }
        Ok(())
    }
    fn render(&mut self) -> Vec<u32> {
        self.pulse();
        let fallback = Scene::program(ScreenKind::Picture);
        panels::render::main(&mut self.layout, self.scene.as_ref().unwrap_or(&fallback), &self.ui, &self.look, self.size)
    }
    fn reply(&mut self, result: Result<(), String>) {
        let face = megatraveller::font_menu::selected(&self.look.font).map(|f| f.family.clone());
        let map_rect = self.scene.as_ref().filter(|s| s.screen == ScreenKind::Ground).and_then(|s| {
            let panels::Host::Dock(host) = self.layout.host("Viewport")? else { return None; };
            let map = panels::viewport::screen_rect((host.w, host.h));
            let m = panels::render::Metrics::for_ui(self.size, &self.look, Some(s), &self.ui);
            let r = m.rect(panels::Rect::new(host.x + map.x, host.y + map.y, map.w, map.h));
            Some([r.x, r.y, r.width, r.height])
        });
        let value = json!({"ok":result.is_ok(), "error":result.err(), "backend":"ui", "ms":self.clock.ms(), "font":face, "size":self.size,
            "map_rect":map_rect,
            "state":{"screen":self.scene.as_ref().map(|s|format!("{:?}",s.screen)), "station":self.scene.as_ref().and_then(|s|s.context.station),
                "text":self.ui.frame.text.visible.iter().flat_map(|r|r.bytes.iter().copied()).map(char::from).collect::<String>(),
                "combat":self.scene.as_ref().map(|s|json!({"round":s.squad.round,"selected":s.squad.selected,
                    "encounter":s.squad.encounter,"acting":s.squad.acting.map(|u|format!("{u:?}")),"strip":s.squad.strip})), "time":{"tick":self.clock.ticks()}},
            "decision":self.ui.decision.as_ref().map(|d|json!({"kind":d.kind.as_str(),"id":d.id,"prompt":d.prompt,
                "program":d.context.program,"routine":d.context.routine,"options":d.options.iter().map(|o|&o.label).collect::<Vec<_>>()}))});
        writeln!(self.replies, "{value}").unwrap(); self.replies.flush().unwrap();
    }
    fn pump(&mut self) {
        loop {
            self.pulse();
            if let Some(end) = self.waiting {
                if self.clock.ms() < end { self.clock.advance(1); return; }
                self.waiting = None; self.reply(Ok(()));
            }
            let mut line = String::new();
            if self.commands.read_line(&mut line).unwrap_or(0) == 0 { std::panic::resume_unwind(Box::new(Stop)); }
            let cmd: Value = match serde_json::from_str(&line) { Ok(c) => c, Err(e) => { self.reply(Err(e.to_string())); continue; } };
            let result = match cmd["op"].as_str().unwrap_or("") {
                "down" | "up" => self.key(cmd["key"].as_str().unwrap_or(""), cmd["op"] == "down"),
                "wait" => match cmd["ms"].as_u64().filter(|n| *n <= 60000) {
                    Some(ms) => { self.waiting = Some(self.clock.ms() + ms); continue; }, None => Err("wait ms must be 0..60000".into())
                },
                "capture" => match cmd["path"].as_str() {
                    Some(path) => { let pixels = self.render(); megatraveller::plus::png::rgb(self.size[0], self.size[1], &pixels).and_then(|png| std::fs::write(path, png)).map_err(|e|e.to_string()) },
                    None => Err("capture needs path".into())
                },
                "quit" => { self.reply(Ok(())); std::panic::resume_unwind(Box::new(Stop)); },
                _ => Err("unknown command".into()),
            };
            self.reply(result);
        }
    }
}
impl Input for Driver {
    fn next_key(&mut self) -> u16 { loop { self.pump(); self.store.restore(&mut self.ui); if let Some(k) = self.store.next_key() { return k; } while let Some(e) = self.events.pop_front() { if let Event::Key(k) = e { self.store.hardware(&mut self.ui, e); return k; } } } }
    fn key_pending(&mut self) -> bool { self.pump(); self.store.key_pending() || self.events.iter().any(|e| matches!(e, Event::Key(_))) }
    fn poll(&mut self, _: &mut dyn Clock) -> Event {
        self.pump(); self.store.restore(&mut self.ui);
        if let Some(e) = self.store.pop() { return e; }
        let e = self.events.pop_front().unwrap_or(Event::Idle); self.store.hardware(&mut self.ui, e); e
    }
    fn arrived(&mut self) -> bool { !self.store.delivered && !self.events.is_empty() }
    fn flush(&mut self) { self.events.clear(); }
    fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
    fn decide(&mut self, d: &Decision) { self.store.record(&mut self.ui, d); let i = self.ui.walk.borrow_mut().at_decision(d.kind, self.scene.as_ref()); self.apply(i); }
    fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.intents.pop_front().map(|i| Box::new(i) as Box<dyn std::any::Any>) }
    fn live_save_location(&self) -> Option<(&Path, &'static str)> { Some((&self.save_dir, env!("MT_BUILD_HASH"))) }
    fn present(&mut self, rgb: &[u32]) {
        let frame = Frame::from_rgb(rgb);
        if std::env::var_os("MT_TEXTLOG_TRACE").is_some() {
            let d = mt_engine::present::textlog::diagnostics();
            eprintln!("textlog frame={} page={} visible={} runs={} overflowed={} wrap_copy_oob={} wrap_write_oob={} overflows={} text={:?}", self.frames, frame.text.page, frame.text.visible.len(), frame.text.runs.len(), frame.text.overflowed, d[0], d[1], d[2], frame.text.visible.iter().flat_map(|r| r.bytes.iter().copied()).map(char::from).take(60).collect::<String>());
            self.frames += 1;
        }
        if self.last_rgb != rgb || frame.text.visible != self.ui.frame.text.visible {
            self.store.refresh(); self.last_rgb = rgb.to_vec(); if let Some(f) = self.replay.frame(frame) { *self.ui.frame = f; }
        }
    }
    fn scene(&mut self, value: &dyn std::any::Any) {
        if let Some(s) = value.downcast_ref::<Scene>() {
            self.store.refresh(); if self.scene.as_ref().is_some_and(|old| old.screen != s.screen) { self.ui.decision = None; }
            self.ui.plus.journal.observe(s); self.replay.ms = self.look.npc_playback_ms;
            if let Some(e) = self.replay.arrive(s.clone(), &self.ui.frame, self.now()) { self.show(e); }
        }
    }
}
fn named_key(name: &str) -> Result<(Option<KeyCode>, Option<String>), String> {
    use KeyCode::*;
    let code = match name {
        "LEFT" => ArrowLeft, "RIGHT" => ArrowRight, "UP" => ArrowUp, "DOWN" => ArrowDown,
        "ESC" => Escape, "ENTER" => Enter, "SPACE" => Space, "TAB" => Tab, "BACKSPACE" => Backspace,
        "PAGEUP" => PageUp, "PAGEDOWN" => PageDown, "HOME" => Home, "END" => End,
        "F1" => F1, "F2" => F2, "F3" => F3, "F4" => F4, "F5" => F5, "F6" => F6,
        "F7" => F7, "F8" => F8, "F9" => F9, "F10" => F10, "F11" => F11,
        _ if name.len() == 1 && name.is_ascii() => return Ok((None, Some(name.to_ascii_lowercase()))),
        _ => return Err(format!("unsupported key {name:?}")),
    };
    Ok((Some(code), (name == "SPACE").then(|| " ".into())))
}

fn boot(dir: &Path, store: &Path, mut input: Box<dyn Input>, mut clock: Box<dyn Clock>) -> Result<(), String> {
    use mt_engine::data::{exepack, ly, pic, Variant};
    let read = |name| Dir(dir.into()).read(name).ok_or_else(||format!("missing {name}"));
    let ly = |name| ly::decode(&read(name)?).map_err(|e|format!("{e:?}"));
    let exe = exepack::unpack(&read("STARTUP.EXE")?).map_err(|e|format!("{e:?}"))?.exe;
    let pic = |name| pic::decode(Variant::Mt1, &read(name)?).map_err(|e|format!("{e:?}"));
    let mut title = mt1::screens::title::Title::new(&exe, ly("TITLE.LY")?, ly("FONT.LY")?, pic("TIT.PIC")?, pic("ZOD.PIC")?).ok_or("invalid title")?;
    loop {
        let mut rng = mt1::screens::title::TcRand { seed: clock.now() as u16 as u32 };
        if title.run(&mut || rng.next(), clock.as_mut(), input.as_mut(), |t, i| {
            i.present(&t.fb.rgb(t.display, &mt_engine::present::palette::ega_dac())); i.scene(&Scene::program(ScreenKind::Startup));
        }) == mt1::screens::title::Exit::Quit { return Ok(()); }
        let exe = exepack::unpack(&read("CG.DAT")?).map_err(|e|format!("{e:?}"))?.exe;
        let mut cg = mt1::screens::cg::Screens::new(exe, ly("FONT.LY")?, ly("CG.LY")?, input, clock, Box::new(Dir(dir.into()))).ok_or("invalid CG")?;
        cg.snapshots = true; let tick = cg.ticks() as u16; cg.rng.srand(tick);
        let (exit, party) = mt1::screens::cg::run(&mut cg);
        let start = match exit {
            mt1::chargen::pool::Exit::Quick => Err(read("DEFAULT.SAV")?),
            mt1::chargen::pool::Exit::Play => Err(mt1::ground::party::block(&mt1::ground::party::initial(), &party, &cg.ds, &mut cg.rng).ok_or("invalid party")?),
            mt1::chargen::pool::Exit::Continue => match mt1::save::continue_old(&mut cg, &store.join("saves")) {
                Some(saved) => Ok(saved),
                None => { input=cg.input; clock=cg.clock; continue; }
            },
            mt1::chargen::pool::Exit::Dos => return Ok(()),
        };
        let restored = start.is_ok();
        let mut g = match start {
            Ok(saved) => mt1::save::ground(saved, cg.input, cg.clock, Box::new(Dir(dir.into()))),
            Err(block) => mt1::ground::Ground::new(&read("T.DAT")?, block, cg.input, cg.clock, Box::new(Dir(dir.into()))).ok_or("invalid ground")?,
        };
        g.save_slot=Some(mt1::save::Slot {dir:store.join("saves"),build:env!("MT_BUILD_HASH")});
        g.scene = Some(mt1::ui::SceneHook::default()); g.mouse_driver = true;
        g.squad = Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../data/npcs.toml"))?));
        g.tooling = Some(Box::new(scenario::Loader::desktop(Default::default(), dir.into(), Default::default())));
        if restored { mt1::ground::main_loop::run(&mut g); } else { mt1::ground::run(&mut g); }
        if !g.game_over || g.input.closed() { return Ok(()); }
        input=g.input; clock=g.clock;
    }
}

pub(crate) fn main() {
    if let Err(e) = run() { eprintln!("fontfit_frames: {e}"); std::process::exit(1); }
}
fn resume_snapshot(snapshot:&Path, dir:&Path, store:&Path, input:Driver, clock:VirtualClock)->Result<(),String> {
    let ended=launch::file_desktop(snapshot, dir, store, Default::default(), move |start| {
        clock.0.borrow_mut().ticks=start.ticks; Ok((Box::new(input),Box::new(clock)))
    })?;
    if let Some(g)=ended { boot(dir,store,g.input,g.clock) } else { Ok(()) }
}
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1); let mut dir = std::env::var_os("MT1_GAMEFILES").map(PathBuf::from);
    let mut snapshot = None; let mut look = PathBuf::from("look.toml"); let mut store = std::env::current_dir().map_err(|e|e.to_string())?;
    let mut size = [1531, 1221]; let (mut route, mut named) = (None, None);
    while let Some(a) = args.next() {
        if a == "--build-id" { println!("{}", env!("MT_BUILD_HASH")); return Ok(()); }
        let v = args.next().ok_or_else(|| format!("{a} needs a value"))?;
        match a.as_str() {
            "--gamefiles" => dir = Some(PathBuf::from(v)), "--snapshot" => snapshot = Some(PathBuf::from(v)),
            "--look" => look = v.into(), "--store" => store = v.into(),
            "--route" => route = Some(PathBuf::from(v)), "--named" => named = Some(v),
            "--size" => { let (w, h) = v.split_once('x').ok_or("size must be WIDTHxHEIGHT")?; size = [w.parse().map_err(|_|"bad width")?, h.parse().map_err(|_|"bad height")?]; },
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if size.contains(&0) || size.iter().any(|n| *n > 8192) { return Err("size must be 1..8192".into()); }
    let dir = dir.ok_or("--gamefiles or MT1_GAMEFILES required")?;
    if let (Some(route), Some(named)) = (route, named) {
        let out = store.join("prepared");
        let code = megatraveller::mcp::executor::execute(&route, false, &out, dir.clone(), store.clone());
        if code != 0 { return Err(format!("route exit {code}")); }
        snapshot = Some(out.join("snapshots").join(format!("named-{named}.json")));
    }
    mt_engine::present::textlog::set_enabled(true);
    let clock = VirtualClock(Rc::default());
    let input = Driver::new(&dir, &look, size, clock.clone(), Box::new(std::io::BufReader::new(std::io::stdin())), Box::new(std::io::stdout()), store.join("saves"));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if let Some(snapshot) = snapshot {
            resume_snapshot(&snapshot, &dir, &store, input, clock)
        } else { boot(&dir, &store, Box::new(input), Box::new(clock)) }
    }));
    match result { Ok(r) => r, Err(e) if e.is::<Stop>() => Ok(()), Err(e) => std::panic::resume_unwind(e) }
}

