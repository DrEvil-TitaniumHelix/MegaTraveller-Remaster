use super::*;
use super::route::{Record, Route, Step};
use std::{collections::HashMap, fs::{self, File}, sync::mpsc, thread, time::Duration};

pub const REFUSED: i32 = 2;
pub const MISMATCH: i32 = 3;
pub const HANG: i32 = 4;
pub const BAD_ROUTE: i32 = 5;
pub const INCOMPLETE: i32 = 6;
pub const UNFILLED: i32 = 7;

#[derive(Clone, Debug)]
struct Report { completed: usize, step: usize, label: String, decision: String, expected: String, seen: String }
impl Report {
    fn print(&self, reason: &str) {
        eprintln!("{reason}: furthest step {}; step {} {:?}; decision {}; expected {}; seen {}",
            self.completed, self.step, self.label, self.decision, self.expected, self.seen);
    }
}
enum Pulse { Arm(Report), Decision(String, String), Stop }
struct Watchdog { tx: mpsc::Sender<Pulse>, thread: Option<thread::JoinHandle<()>> }
impl Watchdog {
    fn new(timeout: Duration, report: Report) -> Result<Self, String> {
        let (tx, rx) = mpsc::channel();
        let handle = thread::Builder::new().name("play-watchdog".into()).spawn(move || {
            let mut report = report;
            loop {
                match rx.recv_timeout(timeout) {
                    Ok(Pulse::Arm(next)) => report = next,
                    Ok(Pulse::Decision(id, seen)) => { report.decision = id; report.seen = seen; }
                    Ok(Pulse::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => return,
                    Err(mpsc::RecvTimeoutError::Timeout) => { report.print("hang (decision timeout)"); std::process::exit(HANG); }
                }
            }
        }).map_err(|e| e.to_string())?;
        Ok(Self { tx, thread: Some(handle) })
    }
}
impl Drop for Watchdog {
    fn drop(&mut self) { let _ = self.tx.send(Pulse::Stop); if let Some(t) = self.thread.take() { let _ = t.join(); } }
}
#[derive(Debug)]
enum Stop { Done, Fail(i32, String), Restart(Snapshot) }
fn stop(s: Stop) -> ! { std::panic::resume_unwind(Box::new(s)) }
fn required<T>(r: Result<T, String>) -> T { r.unwrap_or_else(|e| stop(Stop::Fail(BAD_ROUTE, e))) }
fn disk<T>(r: io::Result<T>) -> T { required(r.map_err(|e| e.to_string())) }

struct Runner {
    route: Route, root: Root, session_root: Root, observed: Rc<RefCell<Observed>>, out: PathBuf, trace: File,
    index: usize, report: Report, watch: mpsc::Sender<Pulse>,
    records: Vec<Record>, named: HashMap<String, (String, Vec<Record>)>,
    choosing: Option<String>, returning: bool, replayed: Option<(String,String)>,
    restore_started: Option<std::time::Instant>,
    before: Option<Value>, named_states: HashMap<String,Value>,
    replay_job: Option<super::trace::ReplayFile>,
    replay_kinds: HashMap<String, String>,
}
impl Runner {
    fn write(&mut self, v: Value) { disk(serde_json::to_writer(&mut self.trace, &v).map_err(io::Error::other)); disk(self.trace.write_all(b"\n")); disk(self.trace.flush()); }
    fn at(&mut self, p: &Play) {
        let d = p.pending().unwrap();
        self.report.decision = d.id.clone(); self.report.seen = format!("{} {}", d.kind.as_str(), d.state);
        self.write(json!({"type":"at","decision":d.id,"kind":d.kind.as_str(),"state":d.state}));
    }
    fn arm(&mut self, step: &Step) {
        self.report.completed = self.index; self.report.step = self.index + 1;
        if !step.label.is_empty() { self.report.label = step.label.clone(); }
        self.report.expected = step.expect_kind.clone().unwrap_or_else(|| "next decision / successful control".into());
        let _ = self.watch.send(Pulse::Arm(self.report.clone()));
    }
    fn save(&self, p: &Play, id: &str) {
        let s = required(p.snapshot(&self.session_root, id, id, "route", env!("MT_BUILD_HASH")));
        required(crate::play_store::save(&self.out.join("snapshots"), &s));
    }
    fn finish_step(&mut self, p: &Play, step: &Step) {
        if let Some(after) = &step.after {
            let d = p.pending().unwrap();
            if after.kind != d.kind.as_str() { self.fail_at(p, MISMATCH, format!("after.kind: expected {}, seen {}",after.kind,d.kind.as_str())) }
            if let Some(a) = &after.state { self.check(p,a); }
        }
        let id = format!("step-{:04}", self.index + 1); self.save(p, &id);
        if step.frame || self.index + 1 == self.route.steps.len() {
            disk(fs::write(self.out.join("frames").join(format!("{:04}.png", self.index + 1)), required(frame::png(p.frame()))));
        }
        self.index += 1; self.report.completed = self.index;
        let d = p.pending().unwrap();
        self.write(json!({"type":"step","index":self.index,"decision":d.id,"kind":d.kind.as_str(),"state":d.state}));
        self.before = None;
    }
    fn check(&self, p: &Play, a: &Value) {
        let result = route::state(&self.observed.borrow(),p.pending()).and_then(|s| route::check_state(a,&s,self.before.as_ref(),&self.named_states));
        if let Err(e) = result { self.fail_at(p, MISMATCH, e) }
    }
    fn target(&mut self, target: super::trace::Target) -> Snapshot {
        self.replay_kinds = target.records.iter().map(|r| (r.decision.clone(),r.kind.clone())).collect();
        if !target.at.kind.is_empty() { self.replay_kinds.insert(target.at.decision.clone(),target.at.kind); }
        route::root_snapshot(&self.root,&target.records,&target.at.decision,&target.at.state).unwrap_or_else(|e| stop(Stop::Fail(MISMATCH,e)))
    }
    fn fail_at(&self, p: &Play, code: i32, error: String) -> ! {
        self.save(p, &format!("failed-{:04}", self.index + 1));
        if let Ok(png) = frame::png(p.frame()) { disk(fs::write(self.out.join("frames").join(format!("{:04}.png", self.index + 1)), png)); }
        stop(Stop::Fail(code, error))
    }
    fn service(&mut self, p: &mut Play) {
        self.replay_kinds.clear();
        if let Some(mut job) = self.replay_job.take() {
            if let Some(target) = job.targets.pop_front() {
                let snapshot = self.target(target); self.replay_job = Some(job); stop(Stop::Restart(snapshot))
            }
            let d = p.pending().unwrap();
            for event in job.events { if event["type"] != "prefix_verified" && (job.complete || event["type"] != "complete") { self.write(event); } }
            if job.complete {
                println!("complete: route finished; decision {}; final hash {} (replay hash-equal; all branches verified)",d.id,d.state);
                stop(Stop::Done)
            }
            self.write(json!({"type":"prefix_verified","decision":d.id,"state":d.state}));
            stop(Stop::Fail(INCOMPLETE,format!("prefix verified to {}, route did not finish",d.id)))
        }
        if let Some(kind) = self.choosing.take() {
            let entry = p.journal().entries.last().expect("accepted choice is journaled");
            let record = Record::from_entry(entry, kind);
            self.write(json!({"type":"entry","entry":record})); self.records.push(record);
            self.at(p);
            let step = self.route.steps[self.index].clone(); self.finish_step(p, &step);
        } else if self.returning {
            self.returning = false;
            self.at(p);
            let step = self.route.steps[self.index].clone(); self.finish_step(p, &step);
            if let Some(start)=self.restore_started.take() {
                eprintln!("restore step {}: {:.6} s; root {:?}",self.index,start.elapsed().as_secs_f64(),self.session_root.kind);
            }
        } else { self.at(p); }
        loop {
            if self.index == self.route.steps.len() {
                self.write(json!({"type":"complete","steps":self.index,"state":p.pending().unwrap().state}));
                println!("complete: furthest step {} {:?}; decision {}; final hash {}{}", self.index, self.report.label,
                    p.pending().unwrap().id, p.pending().unwrap().state, if self.replayed.as_ref().is_some_and(|(id,state)| id == &p.pending().unwrap().id && state == &p.pending().unwrap().state) { " (replay hash-equal)" } else { "" });
                stop(Stop::Done)
            }
            let step = self.route.steps[self.index].clone(); self.arm(&step);
            if step.unfilled || step.label.to_ascii_uppercase().contains("UNFILLED") { self.fail_at(p,UNFILLED,"unfilled route step; fill its actions and checks before continuing".into()) }
            let d = p.pending().unwrap();
            if let Some(expected) = &step.expect_kind {
                if expected != d.kind.as_str() { self.fail_at(p, MISMATCH, format!("expected kind {expected}, seen {}", d.kind.as_str())) }
            }
            if let Some(expected) = &step.expect_label {
                let seen = step.pick.as_ref().and_then(|v| {
                    if let Some(id) = v.as_str() { d.options.iter().find(|o| o.id == id).map(|o| o.label.as_str()) }
                    else { v["row"].as_str().and_then(|id| d.list.as_ref()?.rows.iter().find(|r| r.id == id).map(|r| r.label.as_str())) }
                }).or(d.prompt.as_deref()).unwrap_or("");
                if seen != expected { self.fail_at(p, MISMATCH, format!("expected label {expected:?}, seen {seen:?}")) }
            }
            if let Some(value) = &step.pick {
                self.before = route::state(&self.observed.borrow(),p.pending()).ok();
                self.replayed = None;
                let id = d.id.clone(); let kind = d.kind.as_str().to_string(); let before = d.state.clone();
                let result = pick(value, d).and_then(|pick| p.choose(Choice::new(&id, pick)));
                if let Err(error) = result {
                    let d = p.pending().unwrap();
                    let mut legal: Vec<_> = d.options.iter().filter(|o| o.enabled).map(|o| o.id.as_str()).collect();
                    if let Some(list) = &d.list { legal.extend(list.rows.iter().filter(|r| r.enabled).map(|r| r.id.as_str())); }
                    let total = legal.len(); legal.truncate(24);
                    let time = d.time.as_ref().map(|t| format!("; wait:1..{}; move directions {:?}", t.max_ticks, t.directions.keys().collect::<Vec<_>>() )).unwrap_or_default();
                    self.fail_at(p, REFUSED, format!("refused pick {value}: {error}; legal ids {legal:?} ({total} total){time}"))
                }
                self.write(json!({"type":"pick","decision":id,"kind":kind,"pick":value,"before":before}));
                self.choosing = Some(kind); return
            }
            if let Some(name) = &step.snapshot {
                let id = format!("named-{name}"); self.save(p, &id);
                self.named.insert(name.clone(), (id, self.records.clone()));
                if let Ok(state) = route::state(&self.observed.borrow(),p.pending()) { self.named_states.insert(name.clone(),state); }
            } else if let Some(name) = &step.restore {
                self.restore_started=Some(std::time::Instant::now());
                let (id, records) = self.named.get(name).expect("route validated named snapshot").clone();
                let s = required(crate::play_store::load(&self.out.join("snapshots"), &id));
                self.records = records;
                self.replayed = None;
                self.write(json!({"type":"restore","entries":self.records,"decision":format!("d:{}",s.sequence),"state":s.hash}));
                self.returning = true; self.replay_kinds = self.records.iter().map(|r| (r.decision.clone(), r.kind.clone())).collect();
                stop(Stop::Restart(s))
            } else if let Some(a) = &step.assertion {
                self.check(p,a);
            } else if step.replay == Some(true) {
                let d = p.pending().unwrap();
                let s = required(route::root_snapshot(&self.root, &self.records, &d.id, &d.state));
                self.returning = true; self.replayed = Some((d.id.clone(),d.state.clone()));
                self.replay_kinds = self.records.iter().map(|r| (r.decision.clone(), r.kind.clone())).collect();
                self.replay_kinds.insert(d.id.clone(), d.kind.as_str().into());
                stop(Stop::Restart(s))
            }
            self.finish_step(p, &step);
        }
    }
}

fn scenario(start: &str, relative: &Path, exe: &Path) -> Result<String, String> {
    if matches!(start, "boot" | "quickstart") { return Ok(start.into()) }
    let path = PathBuf::from(start);
    let path = if path.is_absolute() { path } else {
        [relative.join(&path), path.clone(), exe.join("scenarios").join(&path)].into_iter().find(|p| p.is_file()).ok_or("scenario path not found")?
    };
    Ok(format!("scenario:{}", path.canonicalize().map_err(|e| e.to_string())?.display()))
}

pub fn execute(path: &Path, replay: bool, out: &Path, dir: PathBuf, exe_dir: PathBuf) -> i32 {
    let result = prepare(path, replay, out, dir, exe_dir);
    match result { Ok(code) => code, Err(error) => { eprintln!("bad route: furthest step 0; decision none; expected valid route and assets; seen {error}"); BAD_ROUTE } }
}
fn prepare(path: &Path, replay: bool, out: &Path, dir: PathBuf, exe_dir: PathBuf) -> Result<i32, String> {
    let mut replay_file = if replay { match super::trace::read(path) {
        Ok(r) => Some(r), Err(e) => { eprintln!("mismatch: trace verification failed: {e}"); return Ok(MISMATCH) }
    } } else { None };
    let route = if let Some(r) = &replay_file {
        Route { start: r.header["start"].as_str().unwrap().into(), start_tick: r.header["start_tick"].as_u64().unwrap() as u32, timeout_ms: r.header["timeout_ms"].as_u64().unwrap(),
            steps: vec![Step { replay: Some(true), label: "Replay from root".into(), ..Step::default() }] }
    } else { Route::read(path)? };
    let report = Report { completed: 0, step: 1, label: route.steps[0].label.clone(), decision: "none (boot)".into(), expected: "first decision".into(), seen: "booting".into() };
    let watch = Watchdog::new(Duration::from_millis(route.timeout_ms), report.clone())?;
    let start = scenario(route.start.strip_prefix("scenario:").unwrap_or(&route.start), path.parent().unwrap_or(Path::new(".")), &exe_dir)?;
    let fingerprint = boot::fingerprint(&dir)?;
    if replay_file.as_ref().is_some_and(|r| r.header["gamefiles"] != fingerprint) { report.print("mismatch: expected trace gamefiles digest; seen different assets"); return Ok(MISMATCH) }
    let root = Root { kind: RootKind::NewGame, image: None, start_tick: route.start_tick, scenario: Some(start.clone()), gamefiles: fingerprint };
    boot::validate(&root, &dir)?;
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let trace = File::options().write(true).create_new(true).open(out.join("trace.jsonl")).map_err(|e| format!("output trace: {e}; use a fresh --out directory"))?;
    fs::create_dir_all(out.join("frames")).map_err(|e| e.to_string())?;
    let portable_start = if let Some(source) = start.strip_prefix("scenario:") {
        let bytes = route::read(Path::new(source))?;
        fs::write(out.join("root.toml"),bytes).map_err(|e|e.to_string())?;
        "scenario:root.toml"
    } else { &start };
    let observed = Rc::new(RefCell::new(Observed::default()));
    let mut runner = Runner { route, root: root.clone(), session_root: root.clone(), observed, out: out.into(), trace, index: 0, report,
        watch: watch.tx.clone(), records: vec![], named: HashMap::new(), choosing: None, returning: false, replayed: None, before: None,
        named_states:HashMap::new(), replay_job:None, replay_kinds: HashMap::new(), restore_started:None };
    let mut header = replay_file.as_ref().map(|r|r.header.clone()).unwrap_or_else(|| json!({"type":"header","format":2,"hash_version":mt_engine::play::HASH_VERSION,"build":env!("MT_BUILD_HASH"),
        "start_tick":root.start_tick,"gamefiles":root.gamefiles,"timeout_ms":runner.route.timeout_ms,"route_steps":runner.route.steps.len()}));
    header["start"] = json!(portable_start);
    serde_json::to_writer(&mut runner.trace, &header).map_err(|e| e.to_string())?;
    runner.trace.write_all(b"\n").map_err(|e| e.to_string())?; runner.trace.flush().map_err(|e| e.to_string())?;
    let mut next = if let Some(mut r) = replay_file.take() {
        let s = if let Some(t) = r.targets.pop_front() {
            let s = match route::root_snapshot(&root,&t.records,&t.at.decision,&t.at.state) {
                Ok(s) => s, Err(e) => { runner.report.print(&format!("mismatch: {e}")); return Ok(MISMATCH) }
            };
            runner.replay_kinds = t.records.iter().map(|r| (r.decision.clone(),r.kind.clone())).collect();
            if !t.at.kind.is_empty() { runner.replay_kinds.insert(t.at.decision,t.at.kind); } Some(s)
        } else { None };
        runner.replay_job = Some(r); s
    } else { None };
    let host = Rc::new(RefCell::new(runner));
    loop {
        let h = host.clone(); let beat = host.clone();
        let observed = { let h = host.borrow(); *h.observed.borrow_mut() = Observed::default(); h.observed.clone() };
        let mut play = Play::with_service(move |p| h.borrow_mut().service(p));
        let boot_root = if let Some(s) = next.take() { let r = s.root.clone(); play.restore(s)?; r } else { root.clone() };
        host.borrow_mut().session_root = boot_root.clone();
        let anchors = true;
        let heartbeat = Some(Box::new(move |p: &Play| {
            let d = p.pending().unwrap(); let mut h = beat.borrow_mut();
            h.report.decision = d.id.clone(); h.report.seen = format!("{} {}", d.kind.as_str(), d.state);
            let _ = h.watch.send(Pulse::Decision(d.id.clone(), format!("{} {}", d.kind.as_str(), d.state)));
            if let Some(kind) = h.replay_kinds.get(&d.id) {
                if kind != d.kind.as_str() { stop(Stop::Fail(MISMATCH, format!("expected kind {kind}, seen {} at {}", d.kind.as_str(), d.id))) }
            }
        }) as Box<dyn FnMut(&Play)>);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(||
            boot::run(&boot_root, &dir, &exe_dir, Box::new(Answerer { play, observed: observed.clone(), anchors, heartbeat }), observed)));
        let (code, error) = match result {
            Ok(Ok(())) => (MISMATCH, "expected next decision; seen session ended before route completion".into()),
            Ok(Err(e)) => (BAD_ROUTE, e),
            Err(payload) => match payload.downcast::<Stop>() {
                Ok(s) => match *s { Stop::Done => return Ok(0), Stop::Restart(s) => { next = Some(s); continue }, Stop::Fail(code, error) => (code, error) },
                Err(payload) => match payload.downcast::<SessionExit>() {
                    Ok(e) => (MISMATCH, format!("expected valid decision/replay; seen {e:?}")),
                    Err(payload) => { let error = payload.downcast_ref::<String>().map(String::as_str).or_else(|| payload.downcast_ref::<&str>().copied()).unwrap_or("unknown panic");
                        (MISMATCH, format!("expected next decision; seen simulation panic: {error}")) }
                }
            }
        };
        host.borrow().report.print(&error); return Ok(code)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "child process deliberately stalls; parent verifies watchdog exit"]
    fn stuck_simulation_child() {
        let report = Report { completed: 0, step: 1, label: "Efate ambush".into(), decision: "d:1".into(), expected: "next decision".into(), seen: "stuck native loop".into() };
        let watch = Watchdog::new(Duration::from_millis(100), report.clone()).unwrap();
        let out = PathBuf::from(std::env::var_os("MT_WATCHDOG_TEST_OUT").expect("run through parent test"));
        let root = Root { kind: RootKind::NewGame, image: None, start_tick: 1234, scenario: Some("quickstart".into()), gamefiles: "fixture".into() };
        let route = Route { start: "quickstart".into(), start_tick: 1234, timeout_ms: 100,
            steps: vec![Step { pick: Some(json!("go")), label: "Efate ambush".into(), ..Step::default() }] };
        let mut runner = Runner { route, root: root.clone(), session_root: root, observed: Rc::default(),
            out: out.clone(), trace: File::create(out.join("trace.jsonl")).unwrap(), index: 0, report, watch: watch.tx.clone(),
            records: vec![], named: HashMap::new(), choosing: None, returning: false, replayed: None, before:None,
            named_states:HashMap::new(),replay_job:None,replay_kinds: HashMap::new(),restore_started:None };
        let mut play = Play::with_service(move |p| runner.service(p));
        let mut d = Decision::new(Kind::Menu, "test", "stuck-after-pick");
        d.options.push(mt_engine::play::Option::new("go", "Go", vec![Event::Key(13)]));
        play.decide(&d); assert_eq!(play.next_key(), 13);
        loop { std::hint::spin_loop(); }
    }
    #[test]
    fn watchdog_survives_simulation_that_never_polls() {
        let dir = std::env::temp_dir().join(format!("mt-watchdog-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        fs::create_dir_all(&dir).unwrap();
        let mut child = std::process::Command::new(std::env::current_exe().unwrap()).args(["--exact", "mcp::executor::tests::stuck_simulation_child", "--ignored", "--nocapture"])
            .env("MT_WATCHDOG_TEST_OUT", &dir)
            .stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
        let until = std::time::Instant::now() + Duration::from_secs(5);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() >= until { child.kill().unwrap(); panic!("watchdog failed to stop stuck simulation"); }
            thread::sleep(Duration::from_millis(10));
        }
        let out = child.wait_with_output().unwrap(); assert_eq!(out.status.code(), Some(HANG));
        let text = String::from_utf8_lossy(&out.stderr); assert!(text.contains("Efate ambush") && text.contains("d:1"), "{text}");
        let records: Vec<Value> = fs::read_to_string(dir.join("trace.jsonl")).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect();
        assert!(records.iter().any(|r| r["type"] == "pick" && r["pick"] == "go" && r["decision"] == "d:1"), "accepted pick must survive a stuck simulation: {records:?}");
    }
}
