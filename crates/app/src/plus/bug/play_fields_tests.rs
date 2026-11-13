mod oracle_env { include!("../../../../mt1/tests/oracle_env/mod.rs"); }
use super::*;
use mt_engine::{io::{Dir, Event, Input, Overlay, Scripted}, play::{Decision, Kind as DecisionKind, Play, Root, RootKind}, sim::tick::{Clock, Counter}};
use mt1::{ground::{Ground, state::Mem}, play::{self, diagnostic}, save::{Mode, Saved}};
use serde_json::Value;
use std::{any::Any, cell::RefCell, rc::Rc};

struct Observe { evidence: Rc<RefCell<Evidence>>, faithful: bool }
impl Input for Observe {
    fn diagnostics(&self) -> bool { !self.faithful }
    fn next_key(&mut self) -> u16 { 27 }
    fn poll(&mut self, _: &mut dyn Clock) -> Event { Event::Idle }
    fn anchor(&mut self, name: &str, image: &[u8]) {
        assert_eq!(name, "window:t.6923"); self.evidence.borrow_mut().ground_anchor = Some(image.to_vec());
    }
    fn scene(&mut self, state: &dyn Any) {
        if let Some(state) = state.downcast_ref::<diagnostic::State>() { self.evidence.borrow_mut().state = Some(state.clone()); }
    }
}
fn game(faithful: bool) -> (Ground, Rc<RefCell<Evidence>>) {
    let evidence = Rc::new(RefCell::new(Evidence::default()));
    let mut g = Ground::from_ds(vec![0;65536], vec![0;Mem::PARTY_SIZE],
        Box::new(Observe { evidence: evidence.clone(), faithful }), Box::new(Counter::at(123)), Box::new(Overlay::new(None)));
    g.loop_vars.initialized = true; g.mem.set_key(0); g.mem.set_event(-1);
    (g, evidence)
}
fn directory(case: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/m8pint2-r3/reports").join(format!("{case}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
}
fn report(root: &Path, scene: &Scene, fields: &PlayFields, faithful: bool) -> Option<Report> {
    write_at(root, Capture { faithful, kind: Kind::Bug, text: "Capture evidence", rgb: &vec![0x123456;64000],
        scene, journal: &Journal::default(), look: &Look::default(), play: Some(fields) }, 0).unwrap()
}
fn bundle(report: &Report) -> Value { serde_json::from_slice(&fs::read(report.directory.join("bundle.json")).unwrap()).unwrap() }
fn session(g: &mut Ground) -> (Play, Root) {
    let root = Root { kind: RootKind::Save, image: Some(Saved::capture(g, Mode::Ground, "test").encode()), start_tick: 123, scenario: None, gamefiles: "fixture".into() };
    let mut d = Decision::new(DecisionKind::Menu, "t", "fixture"); d.state = play::hash(g);
    d.options.push(mt_engine::play::Option::new("continue", "Continue", vec![Event::Key(65534)]));
    let mut p = Play::new(); p.decide(&d); (p, root)
}
#[test]
fn ground_window_play_fields_save_last_anchor_without_changing_hash() {
    let (mut g, evidence) = game(false); let scene = Scene::program(ScreenKind::Ground);
    let before = play::hash(&mut g); let original = Saved::capture(&mut g, Mode::Ground, "").encode();
    diagnostic::anchor(&mut g);
    let state = diagnostic::ground(&mut g); g.input.scene(&state);
    let fields = play_fields(&scene, &evidence.borrow(), None);
    let root = directory("window"); let r = report(&root, &scene, &fields, false).unwrap(); let b = bundle(&r);
    assert_eq!(b["state_hash"], before); assert_eq!(b["state"], before);
    assert!(b["decision"].is_null()); assert_eq!(b["play_fields_status"], "partial: decision");
    assert_eq!(b["snapshot"], "snapshot.mtsav");
    let bytes = fs::read(r.directory.join("snapshot.mtsav")).unwrap(); assert_eq!(bytes, original);
    let saved = Saved::decode(&bytes).unwrap(); assert_eq!(saved.mode, Mode::Ground); assert!(saved.play.is_none());
    assert_eq!(play::hash(&mut g), before, "capturing and writing must not change an outcome or its hash");
    g.mem.set_party_u8(6, 2); let state = diagnostic::ground(&mut g); g.input.scene(&state);
    let later = play_fields(&scene, &evidence.borrow(), None);
    assert_ne!(later.state_hash, fields.state_hash);
    assert!(matches!(later.snapshot, Some(Image::Window(bytes)) if bytes == original));
}
#[test]
fn data_session_play_fields_export_public_decision_and_loadable_snapshot_root() {
    let (mut g, evidence) = game(false); let scene = Scene::program(ScreenKind::Ground);
    evidence.borrow_mut().state = Some(diagnostic::ground(&mut g)); let (p, origin) = session(&mut g);
    let before = play::hash(&mut g); let decision = p.pending_public(); let journal = p.journal().clone();
    let fields = play_fields(&scene, &evidence.borrow(), Some((&p, &origin)));
    let root = directory("data"); let r = report(&root, &scene, &fields, false).unwrap(); let b = bundle(&r);
    assert_eq!(b["play_fields_status"], "available"); assert_eq!(b["snapshot"], "snapshot.json");
    assert_eq!(b["state_hash"], before); assert_eq!(b["decision"], crate::mcp::public_decision(p.pending().unwrap()));
    assert!(!b["decision"].to_string().contains("65534")); assert!(b["decision"]["options"][0].get("input").is_none());
    let saved = crate::play_store::load(&r.directory, "snapshot").unwrap();
    assert_eq!(saved.root, origin); assert_eq!(saved.hash, p.pending().unwrap().state); assert_eq!(saved.journal, journal);
    assert!(r.directory.join("snapshot.mtsav").is_file());
    assert_eq!(p.pending_public(), decision); assert_eq!(p.journal(), &journal); assert_eq!(play::hash(&mut g), before);
    let pending = play_fields(&scene, &evidence.borrow(), Some((&Play::new(), &origin)));
    assert!(pending.decision.is_null()); assert_eq!(pending.status(), "partial: decision, snapshot");
}
#[test]
fn encounter_play_fields_keep_hash_and_decision_but_never_invent_a_snapshot() {
    let (mut g, evidence) = game(false); diagnostic::anchor(&mut g);
    let anchor = evidence.borrow().ground_anchor.clone(); assert!(anchor.is_some());
    g.squad = Some(mt1::ground::squad::State { encounter: true, ..Default::default() });
    let mut scene = Scene::program(ScreenKind::Ground); scene.squad.encounter = true;
    evidence.borrow_mut().state = Some(diagnostic::ground(&mut g)); let (p, origin) = session(&mut g);
    let before = play::hash(&mut g); diagnostic::anchor(&mut g); assert_eq!(evidence.borrow().ground_anchor, anchor);
    for session in [None, Some((&p, &origin))] {
        let fields = play_fields(&scene, &evidence.borrow(), session);
        assert!(fields.snapshot.is_none(), "encounter: no invented snapshot");
        let r = report(&directory("encounter"), &scene, &fields, false).unwrap(); let b = bundle(&r);
        assert!(b["snapshot"].is_null()); assert!(!r.directory.join("snapshot.mtsav").exists()); assert!(!r.directory.join("snapshot.json").exists());
        assert_eq!(b["state_hash"], before); assert_eq!(b["decision"].is_null(), session.is_none());
        assert_eq!(b["play_fields_status"], if session.is_some() { "partial: snapshot" } else { "partial: decision, snapshot" });
    }
    assert_eq!(play::hash(&mut g), before);
}
#[test]
fn faithful_play_fields_capture_has_no_evidence_or_file_effects() {
    let (mut g, evidence) = game(true); let before = play::hash(&mut g);
    diagnostic::anchor(&mut g); diagnostic::offer_ground(&mut g);
    assert!(evidence.borrow().ground_anchor.is_none()); assert!(evidence.borrow().state.is_none());
    let scene = Scene::program(ScreenKind::Ground); let (p, origin) = session(&mut g);
    let fields = play_fields(&scene, &Evidence { state: Some(diagnostic::ground(&mut g)), ground_anchor: None }, Some((&p, &origin)));
    let root = directory("faithful"); assert!(report(&root, &scene, &fields, true).is_none()); assert!(!root.exists());
    assert_eq!(play::hash(&mut g), before); assert!(!g.input.diagnostics());
    let missing = play_fields(&scene, &Evidence::default(), None);
    assert_eq!(missing.status(), "partial: decision, state_hash, snapshot");
}
#[test]
fn native_ground_boundary_publishes_live_play_fields_after_the_loop_top_anchor() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    #[derive(Debug)]
    struct Stop;
    struct Boundary(Observe);
    impl Input for Boundary {
        fn diagnostics(&self) -> bool { true }
        fn next_key(&mut self) -> u16 { panic!("ground must use poll") }
        fn anchor(&mut self, name: &str, image: &[u8]) { self.0.anchor(name, image); }
        fn scene(&mut self, state: &dyn Any) { self.0.scene(state); }
        fn poll(&mut self, _: &mut dyn Clock) -> Event { std::panic::resume_unwind(Box::new(Stop)); }
    }
    let scenario = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    let mut g = crate::scenario::load(&scenario, &dir).unwrap().game;
    let evidence = Rc::new(RefCell::new(Evidence::default()));
    g.input = Box::new(Boundary(Observe { evidence: evidence.clone(), faithful: false }));
    let stopped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g))).unwrap_err();
    assert!(stopped.is::<Stop>()); let before = play::hash(&mut g);
    let fields = play_fields(&Scene::program(ScreenKind::Ground), &evidence.borrow(), None);
    assert_eq!(fields.state_hash.as_deref(), Some(before.as_str())); assert_eq!(fields.status(), "partial: decision");
    let Some(Image::Window(bytes)) = fields.snapshot else { panic!("outer loop did not publish an M7 image") };
    assert!(Saved::decode(&bytes).unwrap().loop_vars.unwrap().initialized);
    assert_eq!(play::hash(&mut g), before);
}
#[test]
fn space_and_cg_play_fields_use_their_live_hashes_and_clear_previous_roots() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let read = |name| fs::read(dir.join(name)).unwrap();
    let mut sp = mt1::space::Space::new(&read("S.DAT"), vec![0;Mem::PARTY_SIZE], vec![], Box::new(Scripted::new([])),
        Box::new(Counter::at(123)), Box::new(Dir(dir.clone()))).unwrap();
    sp.s.set_word(mt1::space::state::ENCOUNTER, -1);
    let before = mt1::space::play::hash(&mut sp);
    let mut evidence = Evidence { state: Some(diagnostic::space(&mut sp)), ground_anchor: Some(vec![1,2,3]) };
    let fields = play_fields(&Scene::program(ScreenKind::Space), &evidence, None);
    assert_eq!(fields.state_hash.as_deref(), Some(before.as_str())); assert_eq!(mt1::space::play::hash(&mut sp), before);
    sp.s.set_word(mt1::space::state::ENCOUNTER, 0); evidence.state = Some(diagnostic::space(&mut sp));
    assert!(play_fields(&Scene::program(ScreenKind::Space), &evidence, None).snapshot.is_none());
    let mut cg = mt1::screens::cg::Screens::new(mt_engine::data::exepack::unpack(&read("CG.DAT")).unwrap().exe,
        mt_engine::data::ly::decode(&read("FONT.LY")).unwrap(), mt_engine::data::ly::decode(&read("CG.LY")).unwrap(),
        Box::new(Scripted::new([])), Box::new(Counter::at(123)), Box::new(Dir(dir))).unwrap();
    let scene = Scene::program(ScreenKind::Cg); evidence.scene(&scene); assert!(evidence.ground_anchor.is_none()); assert!(evidence.state.is_none());
    let before = cg.play_hash(); evidence.state = Some(diagnostic::cg(&mut cg));
    let fields = play_fields(&scene, &evidence, None);
    assert_eq!(fields.state_hash.as_deref(), Some(before.as_str())); assert_eq!(cg.play_hash(), before);
    assert_eq!(fields.status(), "partial: decision, snapshot");
}
