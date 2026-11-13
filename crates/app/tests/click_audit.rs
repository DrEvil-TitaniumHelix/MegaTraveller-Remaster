#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use megatraveller::{mouse::decision::{self, native_rows, Store}, panels::{picture::Picture, Button, Component, UiEvent, UiState}, pipeline::Frame, scenario};
use mt_engine::{data::{exepack, ly, pic, Variant}, io::{keys, Dir, Event, Files, Input, MouseState, Overlay}, play::{Decision, Kind}, present::{palette, textlog}, sim::tick::{Clock, Counter}};
use mt1::{ground::{self, main_loop, Ground}, screens::{cg::Screens, title::{self, TcRand, Title}}, ui::{Intent, Scene, SceneHook, ScreenKind}};
use std::{cell::RefCell, collections::{BTreeMap, BTreeSet, VecDeque}, panic::{catch_unwind, AssertUnwindSafe}, path::{Path, PathBuf}, rc::Rc, time::Instant};

fn files() -> Option<PathBuf> { oracle_env::gamefiles() }

#[derive(Clone, Debug)]
enum Pick { Id(String), Row(String, u32), Text(String), Default, Escape, Auto(usize), Round, Tour, At(i32, i32) }
#[derive(Debug)] struct Stop(String);

#[derive(Default, Debug)]
struct Rec { kind: String, screen: String, options: BTreeSet<String>, found: BTreeSet<String>, missing: BTreeSet<String>, undrawn: BTreeSet<String>, notes: BTreeSet<String>, reached: usize, clicked: usize, keyed: usize }
#[derive(Default)]
struct Table { rows: BTreeMap<String, Rec>, drives: Vec<String>, failures: Vec<String>, coverage: BTreeMap<String, BTreeSet<String>> }
impl Table {
    fn markdown(&self) -> String {
        let mut out = String::new();
        let missing: usize = self.rows.values().map(|r| r.missing.len()).sum();
        out += &format!("STATUS: {} distinct decisions reached, {} MISSING labels, drives: {}\n\n", self.rows.len(), missing, self.drives.join("; "));
        out += "| decision (program.routine, screen) | kind | options | rows found | MISSING labels | not drawn (keyboard-only) | notes |\n|---|---|---|---|---|---|---|\n";
        for (k, r) in &self.rows {
            out += &format!("| {k} ({}) | {} | {} | {} | {} | {} | {} |\n", r.screen, r.kind, r.options.len(), r.found.len(),
                if r.missing.is_empty() { "-".into() } else { r.missing.iter().cloned().collect::<Vec<_>>().join(", ") },
                if r.undrawn.is_empty() { "-".into() } else { r.undrawn.iter().cloned().collect::<Vec<_>>().join(", ") },
                format!("reached {}x, clicked {} keyed {}; {}", r.reached, r.clicked, r.keyed, r.notes.iter().cloned().collect::<Vec<_>>().join("; ")));
        }
        out
    }
}

struct Drive {
    store: Store, ui: UiState, hw: VecDeque<Event>, intents: VecDeque<Intent>, intent_batch: bool, script: VecDeque<Pick>, auto_left: usize, touring: bool,
    table: Rc<RefCell<Table>>, scene: Option<Scene>, last: String, visits: std::collections::HashMap<String, usize>, calls: [usize; 4], tour: VecDeque<String>, rgb: Vec<u32>, probe_off: Option<(String, (i32, i32))>, spent: [std::time::Duration; 4], idle: usize, empty: usize, acts: usize, started: Instant, name: String, policy: fn(&Decision) -> Pick,
}
impl Drive {
    fn with_tour(mut self, cmds: &[&str]) -> Self { self.tour = cmds.iter().map(|s| s.to_string()).collect(); self }
    fn new(name: &str, script: Vec<Pick>, policy: fn(&Decision) -> Pick, table: Rc<RefCell<Table>>) -> Self {
        Self { store: Store::default(), ui: UiState::default(), hw: VecDeque::new(), intents: VecDeque::new(), intent_batch: false, script: script.into(), auto_left: 0, touring: false,
            table, scene: None, last: String::new(), visits: Default::default(), calls: [0; 4], tour: VecDeque::new(), rgb: vec![0; 64000], probe_off: None, spent: [std::time::Duration::ZERO; 4], idle: 0, empty: 0, acts: 0, started: Instant::now(), name: name.into(), policy }
    }
    fn key(&self) -> Option<String> { self.ui.decision.as_ref().map(|d| format!("{}.{}", d.context.program, d.context.routine)) }
    fn audit(&mut self, d: &Decision) {
        let key = format!("{}.{}", d.context.program, d.context.routine);
        self.table.borrow_mut().coverage.entry(self.name.clone()).or_default().insert(key.clone());
        if matches!(d.kind, Kind::Ground | Kind::Space | Kind::CombatTurn) {
            let mut t = self.table.borrow_mut(); let r = t.rows.entry(key).or_default(); r.kind = d.kind.as_str().into(); r.reached += 1;
            if r.reached == 1 { r.notes.insert("command wait (not a drawn choice)".into()); r.screen = self.scene.as_ref().map(|s| format!("{:?}", s.screen)).unwrap_or_else(|| d.context.program.clone()); }
            return
        }
        let vis = &self.ui.frame.text.visible;
        let rows = native_rows(vis, d); let lines = decision::line_texts(vis);
        let joined: Vec<String> = lines.windows(2).map(|w| format!("{}{}", w[0].1, w[1].1)).collect();
        let page = d.context.table.as_deref().and_then(|t| t.strip_prefix("page:")).and_then(|p| { let (a, b) = p.split_once('+')?; let (r, n) = b.split_once('/')?; Some((a.parse::<usize>().ok()?, r.parse::<usize>().ok()?, n.parse::<usize>().ok()?)) });
        let wrong = self.click_every_rect(d);
        let mut t = self.table.borrow_mut(); let r = t.rows.entry(key.clone()).or_default();
        r.kind = d.kind.as_str().into(); r.reached += 1; r.missing.extend(wrong);
        r.screen = self.scene.as_ref().map(|s| format!("{:?}", s.screen)).unwrap_or_else(|| d.context.program.clone());
        let check = |i: usize, label: &str, r: &mut Rec| {
            r.options.insert(label.into());
            if rows.iter().any(|(k, _)| *k == i) { r.found.insert(label.into()); return }
            let n = decision::norm_label(label);
            if let Some((first, per, total)) = page { if i < first || i >= first + per { r.undrawn.insert(format!("{label} [off page: rows {}..{} of {total} shown]", first + 1, (first + per).min(total))); return } }
            if !n.is_empty() && !lines.iter().any(|(_, l)| l.contains(n.as_str())) && !joined.iter().any(|l| l.contains(n.as_str())) { r.undrawn.insert(format!("{label} [absent: '{n}' in no line]")); return }
            r.missing.insert(label.into());
        };
        match d.kind {
            Kind::Menu | Kind::YesNo => for (i, o) in d.options.iter().enumerate().filter(|(_, o)| o.enabled) { check(i, &o.label, r); },
            Kind::List => {
                let l = d.list.as_ref().unwrap();
                for (i, row) in l.rows.iter().enumerate().filter(|(_, r)| r.enabled) { check(i, &row.label, r); }
                for c in ["enter", "up", "down", "+", "-"] { if lines.iter().any(|(_, l)| l == c) { r.notes.insert(format!("panel button {c} drawn (1990 hotspot, raw release)")); } }
            }
            Kind::Ack => {
                let i = d.options.iter().position(|o| Some(&o.id) == d.default.as_ref()).unwrap_or(0);
                if decision::option(d, i).is_empty() { r.missing.insert("click-anywhere".into()); } else { r.found.insert("click-anywhere".into()); }
            }
            Kind::Text | Kind::Number => { r.notes.insert("text wait: keyboard; no click rows".into()); if !rows.is_empty() { r.missing.insert("click must do nothing".into()); } }
            _ => { r.notes.insert("command wait (not a drawn choice)".into()); }
        }
        if r.reached == 1 {
            if !r.missing.is_empty() || rows.is_empty() { for (y, l) in &lines { eprintln!("  [{key}] y={y}: {l}"); } eprintln!("  [{key}] options {:?} table {:?}", d.options.iter().map(|o| (o.id.as_str(), o.label.as_str(), o.enabled)).collect::<Vec<_>>(), d.context.table); }
            eprintln!("  [{key}] screen {} rows {:?}", r.screen, rows.iter().map(|(k, r)| (k, (r.x, r.y, r.w, r.h))).collect::<Vec<_>>());
        }
    }
    fn click_every_rect(&self, d: &Decision) -> Vec<String> {
        if !matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::List) || self.ui.buy.as_ref().is_some_and(|(id, _)| *id == d.id) { return Vec::new() }
        let scene = self.scene.clone().unwrap_or_else(|| Scene::program(ScreenKind::Picture));
        let shop = megatraveller::panels::buy::shop_list(d).is_some();
        d.rects.iter().filter(|(_, r)| r[2] > 0 && r[3] > 0).filter_map(|&(i, r)| {
            let want = match &d.list {
                _ if shop => if d.list.as_ref().and_then(|l| l.rows.get(i)).is_some_and(|r| r.enabled) { vec![Intent::BuyPrompt(Some(i))] } else { return None },
                Some(l) => match l.rows.get(i).filter(|r| r.enabled) { Some(row) => decision::raw(&row.input.iter().chain(&l.submit).copied().collect::<Vec<_>>()), None => return None },
                None => if d.options.get(i).is_some_and(|o| o.enabled) { decision::option(d, i) } else { return None },
            };
            let at = (r[0] + r[2] as i32 / 2, r[1] + r[3] as i32 / 2);
            let got = Picture::full().input(&scene, &self.ui, &UiEvent::Up(Button::Left), at);
            (got != want).then(|| format!("click #{i} {r:?} at {at:?} gave {got:?}, route {want:?}"))
        }).collect()
    }
    fn absorb(&mut self, intents: Vec<Intent>) -> bool {
        let events: Vec<Event> = intents.iter().filter_map(|i| if let Intent::Raw(e) = i { Some(*e) } else { None }).collect();
        let any = !intents.is_empty();
        self.store.queue(&mut self.ui, &events);
        for i in intents { match i {
            Intent::Raw(_) => {},
            Intent::BuyPrompt(row) => self.ui.buy = row.and_then(|row| self.ui.decision.as_ref().map(|d| (d.id.clone(), row))),
            i => self.intents.push_back(i),
        } }
        any
    }
    fn click(&mut self, at: (i32, i32)) -> bool {
        let scene = self.scene.clone().unwrap_or_else(|| Scene::program(ScreenKind::Picture));
        let intents = Picture::full().input(&scene, &self.ui, &UiEvent::Up(Button::Left), at);
        self.absorb(intents)
    }
    fn note(&self, key: &str, s: String) { self.table.borrow_mut().rows.entry(key.into()).or_default().notes.insert(s); }
    fn count(&self, key: &str, clicked: bool) { let mut t = self.table.borrow_mut(); let r = t.rows.entry(key.into()).or_default(); if clicked { r.clicked += 1 } else { r.keyed += 1 } }
    fn act(&mut self, d: &Decision, pick: Pick) -> bool {
        let key = format!("{}.{}", d.context.program, d.context.routine);
        let rows = native_rows(&self.ui.frame.text.visible, d);
        let centre = |r: &megatraveller::panels::Rect| (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2);
        let by_id = |id: &str| d.options.iter().position(|o| o.id == id && o.enabled);
        let mut pick = pick;
        for _ in 0..4 { pick = match pick {
            Pick::Default => {
                if matches!(d.kind, Kind::Text | Kind::Number) { Pick::Text("1".into()) }
                else { match d.default.as_ref().and_then(|id| by_id(id)).or_else(|| d.options.iter().position(|o| o.enabled && !matches!(o.id.as_str(), "back" | "exit"))) { Some(i) => Pick::Id(d.options[i].id.clone()), None => return false } }
            }
            Pick::Escape => match by_id("back").or_else(|| by_id("exit")).or_else(|| d.options.iter().position(|o| o.enabled && o.input == [Event::Key(keys::ESC)])) {
                Some(i) => Pick::Id(d.options[i].id.clone()),
                None => { self.hw.push_back(Event::Key(keys::ESC)); self.hw.push_back(Event::KeyUp(keys::ESC)); self.count(&key, false); return true }
            },
            Pick::Auto(n) => { self.auto_left = n; return self.act(d, (self.policy)(d)) }
            Pick::Tour => {
                self.touring = true;
                if matches!(d.kind, Kind::Ground | Kind::Space | Kind::CombatTurn) {
                    match self.tour.pop_front() { Some(id) => Pick::Id(id), None => self.stop("tour complete") }
                } else { Pick::Round }
            }
            Pick::Round => {
                let sig = format!("{}.{} {:?}", d.context.program, d.context.routine, d.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>());
                let n = self.visits.entry(sig).or_insert(0); let v = *n; *n += 1;
                match d.kind {
                    Kind::Text | Kind::Number => if v == 0 { Pick::Text(if d.kind == Kind::Number { "1".into() } else { "ABC".into() }) } else { Pick::Escape },
                    Kind::Ack => Pick::Default,
                    Kind::List => { let l = d.list.as_ref().unwrap(); let open: Vec<&mt_engine::play::Row> = l.rows.iter().filter(|r| r.enabled).collect();
                        if v >= open.len().min(2) || open.is_empty() { Pick::Escape } else { Pick::Row(open[v].id.clone(), open[v].qty.max(open[v].min_qty)) } }
                    _ => { let open: Vec<usize> = d.options.iter().enumerate().filter(|(_, o)| o.enabled && !matches!(o.id.as_str(), "back" | "exit") && !o.label.contains("My Party Disk") && !o.label.contains("Return to DOS") && !o.label.contains("Quit") && !o.label.contains("Print")).map(|(i, _)| i).collect();
                        if v >= open.len() { Pick::Escape } else { Pick::Id(d.options[open[v]].id.clone()) } }
                }
            }
            Pick::At(x, y) => {
                let before = format!("{}.{} {:?}", d.context.program, d.context.routine, d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>());
                self.probe_off = Some((before, (x, y))); let ok = self.click((x, y)); eprintln!("    {} {key}: off-row click at ({x},{y}) -> raw {ok}", self.name);
                if !ok { self.note(&key, format!("off-row click at ({x},{y}): nothing sent")); self.probe_off = None; return self.act(d, Pick::Default) }
                return true
            }
            p => { pick = p; break }
        }; }
        let Some((events, intents)) = resolve(d, &pick) else { return false };
        eprintln!("    {} {key}: {pick:?}", self.name);
        if d.kind == Kind::Ack { self.count(&key, true); return self.click((160, 100)) }
        let row = match &pick {
            Pick::Id(id) => match &d.list { Some(l) => l.rows.iter().position(|r| r.id == *id), None => by_id(id) },
            Pick::Row(id, qty) => d.list.as_ref().and_then(|l| l.rows.iter().position(|r| r.id == *id && r.qty == *qty)),
            _ => None,
        };
        if let Some((_, r)) = row.filter(|_| matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::List)).and_then(|i| rows.iter().find(|(k, _)| *k == i)) {
            let at = centre(r); self.count(&key, true);
            let before = self.store.playing();
            let mut ok = self.click(at);
            if self.ui.buy.is_some() {
                let scene = self.scene.clone().unwrap_or_else(|| Scene::program(ScreenKind::Picture));
                let buy = megatraveller::panels::buy::Buy::new(Default::default());
                let yes = buy.input(&scene, &self.ui, &UiEvent::Up(Button::Left), (62, 51));
                assert!(!yes.is_empty(), "{key}: the Buy confirmation must answer Yes");
                ok &= self.absorb(yes);
            }
            let mut want = Vec::new(); decision::press(&mut want, &events);
            if !ok || before { self.note(&key, format!("click at {at:?} produced nothing for {pick:?}")); }
            let _ = (want, intents); return ok
        }
        self.count(&key, false);
        if events.iter().any(|e| *e == Event::Idle) { self.store.hardware(&mut self.ui, Event::Idle); self.hw.extend(events); } else { self.store.queue(&mut self.ui, &events); }
        if !intents.is_empty() { self.ui.decision = None; }
        self.intents.extend(intents); true
    }
    fn stop(&self, why: &str) -> ! {
        let msg = format!("{}: {why} (last decision {}; calls next_key/key_pending/poll/intent {:?}; spent audit/act/present/scene {:?}; store pending {} hw {} intents {} decision {:?})", self.name, self.last, self.calls, self.spent, self.store.pending(), self.hw.len(), self.intents.len(), self.ui.decision.as_ref().map(|d| d.context.routine.clone()));
        if !matches!(why, "script finished" | "tour complete") { self.table.borrow_mut().failures.push(msg.clone()); }
        self.table.borrow_mut().drives.push(msg.clone()); std::panic::resume_unwind(Box::new(Stop(msg)))
    }
    fn serve(&mut self) {
        self.store.restore(&mut self.ui);
        if self.started.elapsed().as_secs() > 600 { self.stop("drive exceeded 600 s (wall-clock cap)") }
        if self.store.playing() || !self.hw.is_empty() || !self.intents.is_empty() { self.empty = 0; return }
        let Some(d) = self.ui.decision.clone() else { return };
        self.empty = 0;
        let tag = format!("{}.{} {}", d.context.program, d.context.routine, d.state);
        if tag != self.last {
            if let Some((before, at)) = self.probe_off.take() {
                let now = format!("{}.{} {:?}", d.context.program, d.context.routine, d.options.iter().map(|o| o.label.clone()).collect::<Vec<_>>());
                let same = now == before;
                let note = format!("off-row click at {at:?}: the wait after it is {} ({})", if same { "the SAME menu" } else { "a DIFFERENT wait" }, if same { "no option chosen".to_string() } else { now.clone() });
                eprintln!("    PROBE {note}"); self.note(&before.split(' ').next().unwrap_or("").to_string(), note);
            }
            self.last = tag; let t0 = Instant::now(); self.audit(&d); self.spent[0] += t0.elapsed();
            if !matches!(d.kind, Kind::Ground | Kind::Space | Kind::CombatTurn) { eprintln!("  {} {}.{} {:?} {:?}", self.name, d.context.program, d.context.routine, d.kind, d.options.iter().filter(|o| o.enabled).map(|o| o.label.as_str()).take(12).collect::<Vec<_>>()); }
        }
        for _ in 0..64 {
            let pick = if self.touring { Pick::Tour } else if self.auto_left > 0 { self.auto_left -= 1; (self.policy)(&d) } else { match self.script.pop_front() { Some(p) => p, None => self.stop("script finished") } };
            self.acts += 1; if self.acts > 3000 { self.stop("3000 acts") }
            let t0 = Instant::now(); let acted = self.act(&d, pick.clone()); self.spent[1] += t0.elapsed(); if acted { return }
            self.note(&format!("{}.{}", d.context.program, d.context.routine), format!("pick {pick:?} not offered here"));
        }
        self.stop("no offered pick")
    }
}
impl Input for Drive {
    fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
    fn decide(&mut self, d: &Decision) { self.store.record(&mut self.ui, d); }
    fn next_key(&mut self) -> u16 {
        self.calls[0] += 1;
        loop {
            self.serve();
            if let Some(k) = self.store.next_key() { return k }
            while let Some(e) = self.hw.pop_front() { self.store.hardware(&mut self.ui, e); if let Event::Key(k) = e { return k } }
            self.empty += 1;
            if self.empty > 3 {
                let k = self.key().unwrap_or_default(); eprintln!("  {}: next_key without a decision after {k}: Escape", self.name); self.empty = 0;
                self.hw.push_back(Event::Key(keys::ESC)); self.hw.push_back(Event::KeyUp(keys::ESC)); self.acts += 1; if self.acts > 3000 { self.stop("3000 acts") }
            }
        }
    }
    fn key_pending(&mut self) -> bool { self.calls[1] += 1; if self.calls[1] % 100000 == 0 { eprintln!("    {} key_pending x{} last {}", self.name, self.calls[1], self.last); } self.serve(); self.store.key_pending() || self.hw.iter().any(|e| matches!(e, Event::Key(_))) }
    fn arrived(&mut self) -> bool { !self.store.delivered && !self.hw.is_empty() }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        self.calls[2] += 1; self.serve();
        if let Some(e) = self.store.pop() { self.idle = 0; if e == Event::Idle { clock.wait_tick(); } return e }
        if let Some(e) = self.hw.pop_front() { self.idle = 0; if e == Event::Idle { clock.wait_tick(); } else { self.store.hardware(&mut self.ui, e); } return e }
        self.idle += 1; if self.idle > 20000 { self.stop("20000 idle polls") }
        clock.wait_tick(); Event::Idle
    }
    fn mouse(&mut self) -> MouseState { MouseState { buttons: 0, col: -1, row: -1 } }
    fn intent(&mut self) -> Option<Box<dyn std::any::Any>> {
        self.calls[3] += 1;
        if !self.intent_batch { self.serve(); self.intent_batch = true; }
        let next = self.intents.pop_front();
        if next.is_none() { self.intent_batch = false; }
        next.map(|i| Box::new(i) as Box<dyn std::any::Any>)
    }
    fn present(&mut self, rgb: &[u32]) {
        if rgb.len() != 64000 || self.rgb[..] == rgb[..] { return }
        let t0 = Instant::now(); self.rgb.copy_from_slice(rgb); self.store.refresh(); *self.ui.frame = Frame::from_rgb(rgb); self.spent[2] += t0.elapsed();
    }
    fn scene(&mut self, s: &dyn std::any::Any) {
        if let Some(s) = s.downcast_ref::<Scene>() {
            let t0 = Instant::now(); self.spent[3] += t0.elapsed();
            self.store.refresh();
            if self.scene.as_ref().is_some_and(|old| old.screen != s.screen) && !self.store.fresh() { self.ui.decision = None; }
            self.store.settle(); self.scene = Some(s.clone());
        }
    }
}

fn resolve(d: &Decision, pick: &Pick) -> Option<(Vec<Event>, Vec<Intent>)> {
    use mt_engine::play::{Choice, Pick as P, Play};
    let p = match pick { Pick::Id(s) => P::Id(s.clone()), Pick::Row(r, q) => P::Row { row: r.clone(), qty: *q },
        Pick::Text(t) => if d.kind == Kind::Number { P::Number(t.parse().ok()?) } else { P::Text(t.clone()) }, _ => return None };
    let mut play = Play::new(); play.register(d); let id = play.pending()?.id.clone();
    play.choose(Choice { decision: id, pick: p }).ok()?;
    let mut intents = Vec::new(); while let Some(i) = play.intent() { intents.push(*i.downcast::<Intent>().ok()?); }
    let mut events = Vec::new(); let mut clock = Counter::at(0);
    for _ in 0..70000 { if play.play_checkpoint().is_some() { break } events.push(play.poll(&mut clock)); }
    Some((events, intents))
}
fn explore(d: &Decision) -> Pick {
    match d.kind { Kind::Ack => Pick::Default, _ => Pick::Escape }
}
fn quickstart_tour(d: &Decision) -> Pick {
    match d.kind {
        Kind::CombatTurn => Pick::Id(d.options.iter().find(|o| o.enabled && (o.id == "first:party" || o.id.starts_with("fire:npc/")))
            .map_or("end_turn", |o| o.id.as_str()).into()),
        Kind::Ground => Pick::Tour,
        Kind::Ack => Pick::Default,
        _ => Pick::Round,
    }
}
fn chargen_a(_: &Decision) -> Pick { Pick::Round }
fn chargen_b(d: &Decision) -> Pick {
    match d.kind {
        Kind::YesNo => Pick::Id("no".into()),
        Kind::Menu => d.options.iter().rev().find(|o| o.enabled && o.id != "back").map(|o| Pick::Id(o.id.clone())).unwrap_or(Pick::Default),
        _ => Pick::Default,
    }
}

struct Assets { dir: PathBuf }
impl Assets {
    fn read(&self, name: &str) -> Vec<u8> { Dir(self.dir.clone()).read(name).unwrap() }
    fn files(&self) -> Box<dyn Files> { let mut f = Overlay::new(Some(&self.dir)); f.hide("UCHARS.DAT"); Box::new(f) }
    fn run<R>(&self, table: &Rc<RefCell<Table>>, name: &str, body: impl FnOnce() -> R) {
        if let Some(only) = std::env::var_os("MT_CLICK_DRIVES") { if !name.contains(only.to_str().unwrap()) { return } }
        table.borrow_mut().drives.push(name.into());
        let r = catch_unwind(AssertUnwindSafe(body));
        if let Err(e) = r { if let Some(s) = e.downcast_ref::<Stop>() { eprintln!("{} ended: {}", name, s.0); } else { write(table); std::panic::resume_unwind(e) } }
        else { table.borrow_mut().drives.push(format!("{name}: program ended")); }
    }
    fn boot(&self, table: &Rc<RefCell<Table>>, name: &str, script: Vec<Pick>, policy: fn(&Decision) -> Pick) { self.boot_disk(table, name, script, policy, false) }
    fn boot_disk(&self, table: &Rc<RefCell<Table>>, name: &str, script: Vec<Pick>, policy: fn(&Decision) -> Pick, disk: bool) {
        let mut input = Drive::new(name, script, policy, table.clone());
        self.run(table, name, || {
            let exe = exepack::unpack(&self.read("STARTUP.EXE")).unwrap().exe;
            let mut t = Title::new(&exe, ly::decode(&self.read("TITLE.LY")).unwrap(), ly::decode(&self.read("FONT.LY")).unwrap(),
                pic::decode(Variant::Mt1, &self.read("TIT.PIC")).unwrap(), pic::decode(Variant::Mt1, &self.read("ZOD.PIC")).unwrap()).unwrap();
            let mut clock = Counter::at(1234); let mut rng = TcRand { seed: 1234 }; let dac = palette::ega_dac();
            if t.run(&mut || rng.next(), &mut clock, &mut input, |t, i| { i.present(&t.fb.rgb(t.display, &dac)); i.scene(&Scene::program(ScreenKind::Startup)) }) != title::Exit::Game { return }
            let files: Box<dyn Files> = if disk { let mut f = Overlay::new(Some(&self.dir)); if Dir(self.dir.clone()).read("UCHARS.DAT").is_none() { f.write("UCHARS.DAT", &[0u8; 2048]).unwrap(); } Box::new(f) } else { self.files() };
            let mut cg = Screens::new(exepack::unpack(&self.read("CG.DAT")).unwrap().exe, ly::decode(&self.read("FONT.LY")).unwrap(),
                ly::decode(&self.read("CG.LY")).unwrap(), Box::new(input), Box::new(clock), files).unwrap();
            cg.snapshots = true; let tick = cg.ticks() as u16; cg.rng.srand(tick);
            let _ = mt1::screens::cg::run(&mut cg);
        });
    }
    fn ground(&self, table: &Rc<RefCell<Table>>, name: &str, scenario: Option<&str>, script: Vec<Pick>, policy: fn(&Decision) -> Pick) { self.ground_tour(table, name, scenario, script, policy, GROUND_TOUR) }
    fn space(&self, table: &Rc<RefCell<Table>>, name: &str, path: &str, script: Vec<Pick>) {
        self.run(table, name, || {
            let p = scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(path), &self.dir).unwrap();
            assert!(!p.extensions.pending_space.is_empty(), "{name}: a space scenario is required");
            let mut g = p.game;
            g.input = Box::new(Drive::new(name, script, explore, table.clone()).with_tour(SPACE_TOUR));
            g.clock = Box::new(Counter::at(1234)); g.scene = Some(SceneHook::default());
            g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), self.dir.clone(), p.extensions)));
            mt1::space::launch(&mut g).unwrap();
        });
        if std::env::var("MT_CLICK_DRIVES").is_ok_and(|only| !name.contains(&only)) { return }
        assert!(table.borrow().coverage.get(name).is_some_and(|seen| seen.iter().any(|k| k.starts_with("s."))), "{name}: never reached space");
    }
    fn ground_tour(&self, table: &Rc<RefCell<Table>>, name: &str, scenario: Option<&str>, script: Vec<Pick>, policy: fn(&Decision) -> Pick, tour: &[&str]) {
        let input = Drive::new(name, script, policy, table.clone()).with_tour(tour);
        self.run(table, name, || {
            let mut g = match scenario {
                Some(s) => { let p = scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(s), &self.dir).unwrap(); let mut g = p.game;
                    g.input = Box::new(input); g.clock = Box::new(Counter::at(1234)); g.scene = Some(SceneHook::default());
                    g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), self.dir.clone(), p.extensions))); g }
                None => { let mut g = Ground::new(&self.read("T.DAT"), self.read("DEFAULT.SAV"), Box::new(input), Box::new(Counter::at(1234)), self.files()).unwrap();
                    g.mouse_driver = true; g.scene = Some(SceneHook::default());
                    g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), self.dir.clone(), Default::default()))); g }
            };
            if g.squad.is_none() { g.squad = Some(ground::squad::State::with_compendium(ground::rules::compendium::Compendium::parse(include_str!("../../../data/npcs.toml")).unwrap())); }
            if scenario.is_some() { main_loop::run(&mut g) } else { ground::run(&mut g) }
        });
    }
}
fn out_path() -> PathBuf { std::env::var_os("MT_CLICK_AUDIT").map(PathBuf::from).unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/click_audit.md")) }
fn write(table: &Rc<RefCell<Table>>) {
    let t = table.borrow(); let md = t.markdown();
    let known = ["title.0297", "cg.519A", "cg.56A6", "cg.1AA2", "cg.7290", "cg.5CC2", "t.D2C3", "t.161EE", "t.A8F7", "t.1AC9C", "t.13FF2", "t.15B69", "t.save", "t.restore",
        "t.shipyard.pay", "t.16374", "t.16A2F", "t.11571", "t.1199B", "t.supplies.trade-in", "t.DE5E", "t.AF79", "t.6923", "t.combat.turn",
        "s.4FCB", "s.4707", "s.955C", "s.9FDB", "s.D7B4", "s.6743.select", "s.6743.view", "s.4ED0", "s.B287", "s.notice"];
    let unreached: Vec<&str> = known.iter().copied().filter(|k| !t.rows.contains_key(*k)).collect();
    let guessed = decision::guessed_screens();
    let filter = std::env::var("MT_CLICK_DRIVES").unwrap_or_else(|_| "none (all drives)".into());
    let body = format!("# click audit (click_audit.rs), {}\n\nDRIVE FILTER: {filter}\n\n{md}\nUNREACHED: {}\n\nGUESSED SCREENS ({}): {}\n\nINCOMPLETE DRIVES: {}\n\nAUDIT TABLE READY\n", chrono_now(), if unreached.is_empty() { "none".into() } else { unreached.join(", ") }, guessed.len(), guessed.join(", "), if t.failures.is_empty() { "none".into() } else { t.failures.join("; ") });
    println!("{body}");
    let p = out_path(); let _ = std::fs::create_dir_all(p.parent().unwrap()); std::fs::write(&p, body).unwrap();
}
fn chrono_now() -> String { format!("{:?}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()) }

const GROUND_TOUR: &[&str] = &["cmd:game", "cmd:party", "cmd:inventory", "cmd:vehicle", "cmd:orders", "cmd:char/0", "cmd:char/1", "cmd:reload", "cmd:talk"];
const SPACE_TOUR: &[&str] = &["cmd:stations", "cmd:communications", "cmd:jump", "cmd:land", "cmd:launch", "cmd:quit"];
#[test]
fn every_drawn_choice_is_clickable() {
    let Some(dir) = files() else { return };
    textlog::set_enabled(true);
    let a = Assets { dir: dir.clone() }; let table: Rc<RefCell<Table>> = Rc::default();
    a.boot(&table, "boot A: title, Start New Game, round-robin", vec![Pick::Default, Pick::Id("menu:519a.80:30/0".into()), Pick::Id("menu:519a.80:30/1".into()), Pick::Auto(250)], chargen_a);
    a.boot_disk(&table, "boot C: party disk present, Create Party Disk asks Yes / No (1AA2)", vec![Pick::Default, Pick::Id("menu:519a.80:30/0".into()), Pick::Id("menu:519a.80:30/1".into()), Pick::Id("no".into()), Pick::Id("menu:519a.80:30/1".into()), Pick::Id("yes".into()), Pick::Auto(6)], chargen_a, true);
    a.boot(&table, "boot B: title, Start New Game, no/last", vec![Pick::Default, Pick::Id("menu:519a.80:30/0".into()), Pick::Id("menu:519a.80:30/1".into()), Pick::Auto(250)], chargen_b);
    write(&table);
    a.ground(&table, "quickstart: introduction, ambush and menus", None, vec![Pick::Auto(500)], quickstart_tour);
    write(&table);
    a.ground_tour(&table, "desk: every service", Some("scenarios/m5_starport_desk_efate.toml"), vec![Pick::Id("move:west:5".into()), Pick::Tour], explore, &[]);
    write(&table);
    a.ground(&table, "ground menus: efate_quiet", Some("scenarios/efate_quiet.toml"), vec![Pick::Tour], explore);
    a.ground(&table, "store + payer: m5_weapons_store_efate_l4", Some("scenarios/m5_weapons_store_efate_l4.toml"),
        vec![Pick::Id("move:south:5".into()), Pick::Row("list:11571/0".into(), 1), Pick::Row("list:11571/0".into(), 1), Pick::Row("list:1199B/0".into(), 1), Pick::Row("list:1199B/5".into(), 1), Pick::Tour], explore);
    a.ground(&table, "fuel payer: atom_low_fuel", Some("tests/atoms/scenarios/atom_low_fuel.toml"),
        vec![Pick::Id("move:west:5".into()), Pick::Id("cmd:fuel".into()), Pick::Row("list:11571/0".into(), 1), Pick::Tour], explore);
    a.ground_tour(&table, "shipyard + supplies: m7_starport_before_save", Some("scenarios/m7_starport_before_save.toml"), vec![Pick::Id("move:west:5".into()), Pick::Tour], explore, &[]);
    a.ground_tour(&table, "supplies: computer trade-in", Some("scenarios/m7_starport_before_save.toml"),
        vec![Pick::Id("move:west:5".into()), Pick::Id("cmd:supplies".into()), Pick::Row("list:1199B/0".into(), 1),
            Pick::Row("list:11571/2".into(), 0), Pick::Id("back".into()), Pick::Tour], explore, &[]);
    a.ground_tour(&table, "recruit: atom_recruit", Some("tests/atoms/scenarios/atom_recruit.toml"), vec![Pick::Id("move:west:5".into()), Pick::Tour], explore, &[]);
    a.ground_tour(&table, "casino: m5_casino_rich", Some("scenarios/m5_casino_rich.toml"), vec![Pick::Id("move:west:5".into()), Pick::Tour], explore, &[]);
    write(&table);
    a.space(&table, "space: m6_gasgiant_lowfuel", "scenarios/m6_gasgiant_lowfuel.toml", vec![Pick::Tour]);
    a.space(&table, "space: m6_landing_louzy", "scenarios/m6_landing_louzy.toml", vec![Pick::Tour]);
    a.space(&table, "space: atom_encounter", "tests/atoms/scenarios/atom_encounter.toml", vec![Pick::Tour]);
    a.space(&table, "space: selectors, aim, tactical maps and unmanned station", "tests/atoms/scenarios/atom_encounter.toml",
        ["cmd:station/1", "cmd:target", "back", "cmd:station/3", "cmd:stop", "back", "cmd:tactical", "back",
         "cmd:station/5", "continue", "cmd:assign", "menu:1c1c/4", "menu:1c1c/5", "cmd:identify", "back", "back"].map(|s| Pick::Id(s.into())).into_iter().collect());
    write(&table);
    for s in ["scenarios/efate_broken.toml", "scenarios/m4b_medkit_in_pack.toml", "tests/atoms/scenarios/atom_pile.toml"] {
        a.ground(&table, &format!("atoms: {s}"), Some(s), vec![Pick::Tour], explore);
        write(&table);
    }
    let g = decision::guessed_screens();
    println!("GUESSED SCREENS ({}): {}", g.len(), g.join(", "));
    let t = table.borrow();
    assert!(t.failures.is_empty(), "INCOMPLETE drives:\n{}", t.failures.join("\n"));
    if std::env::var_os("MT_CLICK_DRIVES").is_none() {
        for key in ["t.supplies.trade-in", "s.D7B4", "s.955C", "s.6743.select", "s.6743.view", "s.notice", "s.4FCB", "s.4ED0"] {
            assert!(t.rows.contains_key(key), "required audit screen never reached: {key}");
        }
        let quick = &t.coverage["quickstart: introduction, ambush and menus"];
        for key in ["t.D2C3", "t.combat.turn", "t.6923"] { assert!(quick.contains(key), "quickstart never reached {key}"); }
    }
    let missing: Vec<String> = t.rows.iter().filter(|(_, r)| !r.missing.is_empty()).map(|(k, r)| format!("{k}: {:?}", r.missing)).collect();
    assert!(missing.is_empty(), "MISSING click rows:\n{}", missing.join("\n"));
}

#[test]
fn off_row_click_does_not_answer_welcome_or_yes_no() {
    let Some(dir) = files() else { return };
    textlog::set_enabled(true);
    let a = Assets { dir }; let table: Rc<RefCell<Table>> = Rc::default();
    a.boot(&table, "off-row probe", vec![Pick::Default, Pick::At(130, 60), Pick::Id("menu:519a.80:30/0".into()), Pick::Id("menu:519a.80:30/1".into()), Pick::Id("menu:519a.80:30/0".into()), Pick::At(160, 60), Pick::Id("menu:56a6.118:220/1".into()), Pick::Auto(2)], chargen_a);
    let t = table.borrow();
    assert!(t.failures.is_empty(), "INCOMPLETE drives:\n{}", t.failures.join("\n"));
    let notes: Vec<String> = t.rows.values().flat_map(|r| r.notes.iter().cloned()).filter(|n| n.starts_with("off-row click")).collect();
    eprintln!("{notes:?}");
    assert_eq!(notes.len(), 2, "two probes: {notes:?}");
    assert!(notes.iter().all(|n| n.contains("nothing sent") || n.contains("the SAME menu")), "an off-row click answered a menu: {notes:?}");
}

#[test]
fn zz_skip_report() { oracle_env::report() }
