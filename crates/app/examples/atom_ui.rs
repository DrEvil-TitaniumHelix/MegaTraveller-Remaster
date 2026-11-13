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

struct Driver {
    scene: Option<Scene>, last_click: Option<(&'static str, (i32, i32))>, ui: UiState, layout: Layout, assets: Arc<Assets>, look: Look, size: [u32; 2],
    events: VecDeque<Event>, intents: VecDeque<Intent>, store: megatraveller::mouse::decision::Store,
    replay: panels::replay::Replay, epoch: Instant, clock: VirtualClock,
    commands: Box<dyn BufRead>, replies: Box<dyn Write>, waiting: Option<u64>,
    save_dir: PathBuf, last_rgb: Vec<u32>,
}
impl Driver {
    fn new(dir: &Path, look_path: &Path, size: [u32; 2], clock: VirtualClock, commands: Box<dyn BufRead>, replies: Box<dyn Write>, save_dir: PathBuf) -> Self {
        if std::env::var_os("MT_MINIMAP_BELOW").is_some() { megatraveller::panels::components::MINIMAP_BELOW.store(true, std::sync::atomic::Ordering::Relaxed); }
        let assets = Arc::new(Assets::load(&mut Dir(dir.into())));
        let mut layout = Layout::default(); panels::components::register(&mut layout, assets.clone());
        let mut look = Look::load_for_layout(look_path, &layout);
        look.font = Look::default().font; look.bold = Look::default().bold;
        let mut ui = UiState { fixed_screen: true, ..Default::default() };
        look.layout.restore(&layout, &mut ui);
        for id in layout.ids() { if let Some(size) = look.layout.components.get(id).and_then(|c| c.size) { layout.set_size(id, size); } }
        Self { scene: None, last_click: None, ui, layout, assets, look, size, events: VecDeque::new(), intents: VecDeque::new(), store: Default::default(),
            replay: Default::default(), epoch: Instant::now(), clock, commands, replies, waiting: None, save_dir, last_rgb: vec![] }
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
            if key != 32 && !self.scene.as_ref().is_some_and(|s| megatraveller::mouse::walk::cancel_after_replay(UiEvent::Key(key), s, &self.ui, &self.intents)) { return; }
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
            if let Some(a) = window::key_press(&mut keys, code, text, &self.look, false, false, false)? {
                a.apply(&mut self.look);
                if a == crate::menu::Action::Minimap { let id = megatraveller::panels::components::MINIMAP_ID; let k = self.look.layout.components.get(id).map_or(panels::HostKind::Docked, |s| s.host); self.layout.set_kind(&mut self.ui, id, k); }
            }
            for key in keys { self.route(key); }
        } else if !self.ui.plus.open && !panels::textbox::active(&self.ui) {
            window::enqueue(&mut keys, code, text.as_deref())?;
            if let Some(key) = keys.pop_front() { self.events.push_back(Event::KeyUp(key)); }
        }
        Ok(())
    }
    fn panel_rows(&mut self, scene: &Scene) -> Vec<(&'static str, Vec<(String, (i32, i32))>)> {
        self.prepare();
        let mut out = Vec::new();
        for (id, _, surface) in self.layout.draw(scene, &self.ui) {
            let mut runs: Vec<&mt_engine::present::textlog::Run> = surface.text.visible.iter().filter(|r| !r.bytes.is_empty()).collect();
            runs.sort_by_key(|r| (r.y / 4, r.x));
            let mut rows: Vec<(String, (i32, i32))> = Vec::new(); let mut last: Option<(i64, i64)> = None;
            for r in runs {
                let c = r.bytes[0] as char;
                match last { Some((y, x1)) if (y - r.y).abs() <= 3 && r.x <= x1 + 12 => { let row = rows.last_mut().unwrap(); if r.x > x1 + 2 { row.0.push(' '); } row.0.push(c); }
                    _ => rows.push((c.to_string(), (r.x as i32, r.y as i32))) }
                last = Some((r.y, r.x + r.width as i64));
            }
            if !rows.is_empty() { out.push((id, rows)); }
        }
        out
    }
    fn click(&mut self, cmd: &Value) -> Result<(), String> {
        self.pulse(); self.prepare();
        let Some(scene) = self.scene.clone() else { return Err("no scene yet".into()) };
        let button = if cmd["button"] == "right" { panels::Button::Right } else { panels::Button::Left };
        let only = cmd["component"].as_str().map(str::to_string);
        let (id, at): (&'static str, (i32, i32)) = if let (Some(c), Some(at)) = (only.as_deref(), cmd["at"].as_array()) {
            let id = self.layout.ids().into_iter().find(|i| *i == c).ok_or_else(|| format!("no component {c}"))?;
            (id, (at.first().and_then(Value::as_i64).unwrap_or(0) as i32, at.get(1).and_then(Value::as_i64).unwrap_or(0) as i32))
        } else if let Some(text) = cmd["text"].as_str() {
            let needle = text.to_ascii_lowercase(); let mut found = None;
            'rows: for (id, rows) in self.panel_rows(&scene) { if only.as_deref().is_some_and(|c| c != id) { continue } for (row, at) in rows { if let Some(i) = row.to_ascii_lowercase().find(&needle) {
                let ahead = row[..i].chars().filter(|c| *c != ' ').count() as i32 * 6 + row[..i].chars().filter(|c| *c == ' ').count() as i32 * 6; found = Some((id, (at.0 + ahead + 3, at.1 + 4))); break 'rows } } }
            found.ok_or_else(|| format!("no drawn text {text:?}; rows: {:?}", self.panel_rows(&scene).iter().map(|(id, r)| (id, r.iter().map(|x| x.0.clone()).collect::<Vec<_>>())).collect::<Vec<_>>()))?
        } else if let Some(cell) = cmd["cell"].as_array() {
            let (r, c) = (cell.first().and_then(Value::as_i64).unwrap_or(0) as f32, cell.get(1).and_then(Value::as_i64).unwrap_or(0) as f32);
            let Some(panels::Host::Dock(h)) = self.layout.host("Viewport") else { return Err("no docked viewport".into()) };
            let cam = panels::viewport::Camera::new(&scene, &self.ui, (h.w, h.h));
            ("Viewport", (((c * 16. + 8. - cam.left) * cam.zoom) as i32, ((r * 16. + 8. - cam.top) * cam.zoom) as i32))
        } else {
            let at = (cmd["x"].as_i64().ok_or("click needs text, cell or x / y")? as i32, cmd["y"].as_i64().unwrap_or(0) as i32);
            self.layout.hit(&scene, None, at).ok_or("nothing under the point")?
        };
        self.ui.components.pointer = Some((id, at)); self.last_click = Some((id, at));
        if let Some(path) = cmd["path"].as_str() {
            let m = panels::render::Metrics::for_ui(self.size, &self.look, self.scene.as_ref(), &self.ui);
            let px = match self.layout.host(id) { Some(panels::Host::Dock(r)) => m.project(r, at, &self.look), _ => None };
            std::fs::write(path, serde_json::to_vec(&json!({"id": id, "at": [at.0, at.1], "px": px.map(|(x, y)| [x, y])})).unwrap_or_default()).map_err(|e| e.to_string())?;
        }
        let events = if cmd["hover"] == true { vec![UiEvent::Move] } else { vec![UiEvent::Move, UiEvent::Down(button), UiEvent::Up(button)] };
        for e in events {
            let i = panels::controls::dispatch(&self.layout, id, &scene, &mut self.ui, &e, at); self.component(id, i);
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
        let panels_text: Vec<Value> = match self.scene.clone() { Some(s) => self.panel_rows(&s).into_iter().map(|(id, rows)| json!({"id": id, "rows": rows.into_iter().map(|r| r.0).collect::<Vec<_>>()})).collect(), None => Vec::new() };
        let value = json!({"ok":result.is_ok(), "error":result.err(), "backend":"ui", "ms":self.clock.ms(), "font":face, "size":self.size,
            "map_rect":map_rect, "panels":panels_text,
            "cells":self.scene.as_ref().map(|s| json!({"party": (0..5u8).filter_map(|i| panels::controls::position(s, mt1::ui::Unit::Traveller(i)).map(|p| [((p.0 - 0.5) as i32), ((p.1 - 0.5) as i32)])).collect::<Vec<_>>(),
                "npcs": s.npcs.iter().enumerate().filter_map(|(i, _)| panels::controls::position(s, mt1::ui::Unit::Npc(i as u8 + 1)).map(|p| [i as i32 + 1, (p.0 - 0.5) as i32, (p.1 - 0.5) as i32])).collect::<Vec<_>>(),
                "hover": format!("{:?}", self.ui.hover), "hint": self.ui.hint, "click": self.last_click.map(|(id, at)| json!([id, at.0, at.1])), "menu": self.ui.menu.is_some(), "label": s.squad.label, "log": s.squad.log.iter().rev().take(4).cloned().collect::<Vec<_>>()})),
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
                "click" => self.click(&cmd),
                "text_runs" => match cmd["path"].as_str() {
                    Some(path) => { let runs: Vec<Value> = self.ui.frame.text.visible.iter().map(|r| json!([r.x, r.y, r.bytes.iter().map(|&b| b as char).collect::<String>(), r.width, r.covered, r.colour])).collect();
                        std::fs::write(path, serde_json::to_vec(&json!({"runs": runs})).unwrap_or_default()).map_err(|e| e.to_string()) }
                    None => Err("runs needs a path".into()) },
                "hosts" => match cmd["path"].as_str() {
                    Some(path) => {
                        let scene = self.scene.clone().unwrap_or_else(|| Scene::program(ScreenKind::Picture));
                        let m = panels::render::Metrics::for_ui(self.size, &self.look, self.scene.as_ref(), &self.ui);
                        let hosts: Vec<Value> = self.layout.ids().into_iter().filter(|id| self.layout.visible(id, &scene)).filter_map(|id| match self.layout.host(id)? {
                            panels::Host::Dock(r) => { let p = m.rect(r); Some(json!({"id": id, "rect": [p.x, p.y, p.width, p.height], "logical": [r.x, r.y, r.w, r.h]})) } _ => None }).collect();
                        std::fs::write(path, serde_json::to_vec(&json!({"hosts": hosts})).unwrap_or_default()).map_err(|e| e.to_string())
                    }
                    None => Err("hosts needs path".into())
                },
                "rows" => match cmd["path"].as_str() {
                    Some(path) => {
                        let rows: Vec<Value> = self.ui.decision.as_ref().map(|d| megatraveller::mouse::decision::native_rows(&self.ui.frame.text.visible, d).into_iter().map(|(i, r)| {
                            let label = match &d.list { Some(l) => l.rows.get(i).map(|r| r.label.clone()), None => d.options.get(i).map(|o| o.label.clone()) };
                            json!({"i": i, "label": label, "rect": [r.x, r.y, r.w, r.h]}) }).collect()).unwrap_or_default();
                        std::fs::write(path, serde_json::to_vec(&json!({"rows": rows})).unwrap_or_default()).map_err(|e| e.to_string())
                    }
                    None => Err("rows needs path".into())
                },
                "runs" => match cmd["path"].as_str() {
                    Some(path) => {
                        let f = &self.ui.frame.text;
                        let runs: Vec<Value> = f.visible.iter().filter(|r| r.page == f.page).map(|r| json!([r.x, r.y, r.bytes.first().copied().unwrap_or(0), r.colour, r.font_id, r.width, r.shadow, r.background.first().copied(), r.background.iter().all(|&b| Some(b) == r.background.first().copied()), r.covered])).collect();
                        std::fs::write(path, serde_json::to_vec(&json!({"page": f.page, "runs": runs})).unwrap_or_default()).map_err(|e| e.to_string())
                    }
                    None => Err("runs needs path".into())
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
    if let Err(e) = run() { eprintln!("atom_ui: {e}"); std::process::exit(1); }
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
    let mut size = [1531, 1221];
    while let Some(a) = args.next() {
        if a == "--build-id" { println!("{}", env!("MT_BUILD_HASH")); return Ok(()); }
        let v = args.next().ok_or_else(|| format!("{a} needs a value"))?;
        match a.as_str() {
            "--gamefiles" => dir = Some(PathBuf::from(v)), "--snapshot" => snapshot = Some(PathBuf::from(v)),
            "--look" => look = v.into(), "--store" => store = v.into(),
            "--size" => { let (w, h) = v.split_once('x').ok_or("size must be WIDTHxHEIGHT")?; size = [w.parse().map_err(|_|"bad width")?, h.parse().map_err(|_|"bad height")?]; },
            _ => return Err(format!("unknown argument {a}")),
        }
    }
    if size.contains(&0) || size.iter().any(|n| *n > 8192) { return Err("size must be 1..8192".into()); }
    let dir = dir.ok_or("--gamefiles or MT1_GAMEFILES required")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    mod oracle_env { include!("../../mt1/tests/oracle_env/mod.rs"); }
    struct Commands<F> { bytes: std::io::Cursor<Vec<u8>>, next: F }
    impl<F: FnMut() -> Vec<Value>> std::io::Read for Commands<F> {
        fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
            if self.bytes.position() as usize == self.bytes.get_ref().len() {
                self.bytes = std::io::Cursor::new((self.next)().iter().map(|c| format!("{c}\n")).collect::<String>().into_bytes());
            }
            std::io::Read::read(&mut self.bytes, out)
        }
    }
    struct Replies(Rc<RefCell<Vec<u8>>>);
    impl Write for Replies {
        fn write(&mut self,bytes:&[u8])->std::io::Result<usize> {self.0.borrow_mut().extend_from_slice(bytes);Ok(bytes.len())}
        fn flush(&mut self)->std::io::Result<()> {Ok(())}
    }
    #[test]
    fn g7_encounter_keyboard_and_menu_routes_return_to_flight() {
        let Some(dir)=oracle_env::gamefiles() else {return};
        let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out=root.join(format!("target/atom-ui-g7-{}",std::process::id()));std::fs::create_dir_all(&out).unwrap();
        let mut steps:Vec<_>=["move:southeast:5","move:east:15","move:northeast:5","move:north:10"].iter().map(|p|json!({"pick":p})).collect();
        steps.push(json!({"snapshot":"078","frame":true}));
        let route=out.join("route.json");
        std::fs::write(&route,json!({"start":root.join("tests/atoms/scenarios/atom_encounter.toml"),"start_tick":1234,"steps":steps}).to_string()).unwrap();
        assert_eq!(megatraveller::mcp::executor::execute(&route,false,&out.join("prepared"),dir.clone(),out.clone()),0);
        for menu in [false,true] {
            mt_engine::present::textlog::set_enabled(true);
            let mut keys=vec!["V","DOWN","ENTER"];
            if menu {
                keys.extend(["PAGEDOWN";5]); keys.extend(["SPACE";4]); keys.push("T");
                keys.extend(["PAGEDOWN";5]); keys.push("SPACE");
            } else { keys.extend(["RIGHT";4]); keys.extend(["T","ENTER"]); }
            keys.push("ESC");
            let mut commands=vec![json!({"op":"wait","ms":500})];
            for key in keys {
                commands.extend([json!({"op":"down","key":key}),json!({"op":"wait","ms":60}),
                    json!({"op":"up","key":key}),json!({"op":"wait","ms":140})]);
            }
            commands.extend([json!({"op":"wait","ms":3000}),json!({"op":"quit"})]);
            let commands=commands.iter().map(|c|format!("{c}\n")).collect::<String>();
            let replies=Rc::new(RefCell::new(vec![]));let clock=VirtualClock(Rc::default());
            let input=Driver::new(&dir,&out.join("look.toml"),[640,480],clock.clone(),Box::new(std::io::Cursor::new(commands)),Box::new(Replies(replies.clone())),out.join("saves"));
            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||resume_snapshot(&out.join("prepared/snapshots/named-078.json"),&dir,&out,input,clock)));
            assert!(matches!(result,Err(e) if e.is::<Stop>()),"menu={menu}: driver must stay alive");
            let replies=String::from_utf8(replies.borrow().clone()).unwrap();
            let replies:Vec<Value>=replies.lines().map(|line|serde_json::from_str(line).unwrap()).collect();
            assert!(replies.iter().any(|r|r["decision"]["routine"]=="955C"),"menu={menu}: T must enter aim mode");
            assert!(replies.iter().any(|r|r["state"]["text"].as_str().is_some_and(|s|s.to_ascii_lowercase().contains("enemy's cargo"))),
                "menu={menu}: the shot must destroy the hostile and offer its cargo before Esc");
            assert_eq!(replies.last().unwrap()["decision"]["routine"],"4FCB","menu={menu}: one shot destroys the hostile and returns to flight");
        }
    }
    #[test]
    fn g6_space_quit_and_party_defeat_reach_the_real_title() {
        let Some(dir)=oracle_env::gamefiles() else {return};
        let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (id,scenario,pre,keys) in [
            ("082","scenarios/m6_efate_orbit_ready.toml",vec!["move:southeast:5","move:east:15","move:northeast:5","move:north:10"],vec!["T","Q","Y"]),
            ("083","tests/atoms/scenarios/atom_last_stand.toml",vec![],vec!["SPACE","E","E"]),
        ] {
            let out=root.join(format!("target/atom-ui-end-{id}-{}",std::process::id()));std::fs::create_dir_all(&out).unwrap();
            let route=out.join("route.json");let mut steps:Vec<_>=pre.iter().map(|p|json!({"pick":p})).collect();steps.push(json!({"snapshot":id,"frame":true}));
            std::fs::write(&route,json!({"start":root.join(scenario),"start_tick":1234,"steps":steps}).to_string()).unwrap();
            assert_eq!(megatraveller::mcp::executor::execute(&route,false,&out.join("prepared"),dir.clone(),out.clone()),0);
            let mut commands=vec![json!({"op":"wait","ms":500})];
            for key in keys {commands.extend([json!({"op":"down","key":key}),json!({"op":"wait","ms":60}),json!({"op":"up","key":key}),json!({"op":"wait","ms":1940})]);}
            commands.extend([json!({"op":"wait","ms":10000}),json!({"op":"quit"})]);
            let commands=commands.iter().map(|c|format!("{c}\n")).collect::<String>();
            let replies=Rc::new(RefCell::new(vec![]));let clock=VirtualClock(Rc::default());
            let commands: Box<dyn BufRead> = if id == "083" {
                let seen = replies.clone(); let mut batches = 0;
                Box::new(std::io::BufReader::new(Commands { bytes: std::io::Cursor::new(b"{\"op\":\"wait\",\"ms\":500}\n".to_vec()), next: move || {
                    let bytes = seen.borrow();
                    let last: Value = serde_json::from_slice(bytes.split(|&b| b == b'\n').filter(|b| !b.is_empty()).last().unwrap()).unwrap();
                    if last["state"]["screen"] == "Startup" { return vec![json!({"op":"quit"})] }
                    batches += 1; assert!(batches <= 20, "the last-stand fight did not end: {last}");
                    let keys = if last["decision"]["kind"] == "ack" { vec!["SPACE"] } else { vec!["SPACE", "E"] };
                    keys.into_iter().flat_map(|key| [json!({"op":"down","key":key}), json!({"op":"wait","ms":60}),
                        json!({"op":"up","key":key}), json!({"op":"wait","ms":1940})]).collect()
                } }))
            } else { Box::new(std::io::Cursor::new(commands)) };
            let input=Driver::new(&dir,&out.join("look.toml"),[640,480],clock.clone(),commands,Box::new(Replies(replies.clone())),out.join("saves"));
            let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||resume_snapshot(&out.join(format!("prepared/snapshots/named-{id}.json")),&dir,&out,input,clock)));
            assert!(matches!(result,Err(e) if e.is::<Stop>()),"{id}: session must stay alive");
            let replies=String::from_utf8(replies.borrow().clone()).unwrap();
            std::fs::write(out.join("replies.jsonl"), &replies).unwrap();
            let last:Value=serde_json::from_str(replies.lines().last().unwrap()).unwrap();
            assert!(replies.lines().all(|line| serde_json::from_str::<Value>(line).unwrap()["ok"] == true), "driver command failed: {replies}");
            if id == "083" {
                let shown = replies.lines().map(|line| serde_json::from_str::<Value>(line).unwrap()).take_while(|r| r["state"]["screen"] != "Startup")
                    .any(|r| r["decision"]["kind"] == "ack" && r["state"]["text"].as_str().is_some_and(|t| t.contains("Your entire party is unconscious") && t.contains("failed.")));
                assert!(shown, "the loss must be shown, waiting for a key, before the title");
            }
            assert_eq!(last["state"]["screen"],"Startup","{id}: real title, same driver");
        }
    }
    #[test]
    fn g6_save_quit_title_continue_retains_the_input_and_player_slot() {
        let Some(dir)=oracle_env::gamefiles() else {return};
        let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let out=root.join(format!("target/atom-ui-lifecycle-{}",std::process::id()));std::fs::create_dir_all(&out).unwrap();
        let route=out.join("route.json");
        std::fs::write(&route,json!({"start":root.join("scenarios/m7_starport_before_save.toml"),"start_tick":1234,
            "steps":[{"pick":"move:west:5"},{"snapshot":"081","frame":true}]}).to_string()).unwrap();
        assert_eq!(megatraveller::mcp::executor::execute(&route,false,&out.join("prepared"),dir.clone(),out.clone()),0);
        let mut commands=vec![json!({"op":"wait","ms":500}), json!({"op":"click","text":"save"}), json!({"op":"wait","ms":200})];
        for (step, key) in ["ENTER","ESC","ESC","ENTER","SPACE","DOWN","ENTER","SPACE"].into_iter().enumerate() {
            commands.extend([json!({"op":"down","key":key}),json!({"op":"wait","ms":60}),
                json!({"op":"up","key":key}),json!({"op":"wait","ms":if step == 0 { 4000 } else { 1940 }})]);
        }
        commands.push(json!({"op":"quit"}));
        let commands=commands.iter().map(|c|format!("{c}\n")).collect::<String>();
        let replies=Rc::new(RefCell::new(vec![]));let clock=VirtualClock(Rc::default());
        let input=Driver::new(&dir,&out.join("look.toml"),[640,480],clock.clone(),Box::new(std::io::Cursor::new(commands)),Box::new(Replies(replies.clone())),out.join("saves"));
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||resume_snapshot(&out.join("prepared/snapshots/named-081.json"),&dir,&out,input,clock)));
        assert!(matches!(result,Err(e) if e.is::<Stop>()));
        let replies=String::from_utf8(replies.borrow().clone()).unwrap();
        std::fs::write(out.join("replies.jsonl"), &replies).unwrap();
        let replies:Vec<Value>=replies.lines().map(|line|serde_json::from_str(line).unwrap()).collect();
        assert!(replies.iter().all(|r| r["ok"] == true), "driver command failed: {replies:?}");
        assert!(replies.iter().any(|r| r["state"]["text"].as_str().is_some_and(|t| t.contains("Save to which position?"))), "Save must offer the position chooser");
        assert!(replies.iter().any(|r|r["state"]["screen"]=="Startup"),"Quit must reach the real title");
        assert_eq!(replies.last().unwrap()["decision"]["routine"],"13FF2","Continue must restore the saved starport");
        assert!(out.join("saves").join(mt1::save::FILE).is_file());
    }
    #[test]
    fn t007_snapshot_renders_modern_nonflat_frame() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
        let out = root.join(format!("target/atom-ui-test-{}", std::process::id())); std::fs::create_dir_all(&out).unwrap();
        let route = out.join("route.json");
        std::fs::write(&route, json!({"start":root.join("scenarios/efate_broken.toml"),"start_tick":1234,"steps":[{"snapshot":"007","frame":true}]}).to_string()).unwrap();
        assert_eq!(megatraveller::mcp::executor::execute(&route, false, &out.join("prepared"), dir.clone(), out.clone()), 0);
        let png = out.join("frame.png");
        let commands = format!("{}\n{}\n", json!({"op":"capture","path":png}), json!({"op":"quit"}));
        let clock = VirtualClock(Rc::default());
        let input = Driver::new(&dir, &out.join("look.toml"), [1531,1221], clock.clone(), Box::new(std::io::Cursor::new(commands)), Box::new(std::io::sink()), out.join("saves"));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| launch::file_desktop(&out.join("prepared/snapshots/named-007.json"), &dir, &out, Default::default(), move |s| {
            clock.0.borrow_mut().ticks = s.ticks; Ok((Box::new(input), Box::new(clock)))
        })));
        assert!(matches!(result, Err(e) if e.is::<Stop>()));
        let bytes = std::fs::read(png).unwrap(); assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(u32::from_be_bytes(bytes[16..20].try_into().unwrap()),1531); assert_eq!(u32::from_be_bytes(bytes[20..24].try_into().unwrap()),1221);
        let mut counts = [0usize;256]; for b in &bytes[48..bytes.len()-20] { counts[*b as usize] += 1; }
        assert!(counts.iter().filter(|n| **n > 1000).count() > 8);
        assert!(*counts.iter().max().unwrap() < bytes.len() * 95 / 100);
    }
    #[test]
    fn zz_skip_report() { oracle_env::report() }
}
