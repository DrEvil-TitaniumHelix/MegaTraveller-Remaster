use super::*;
use std::time::Instant;
mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }

pub(super) fn assets() -> PathBuf { PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES")) }
pub(super) fn root(dir: &Path) -> Root {
    Root { kind: RootKind::NewGame, image: None, start_tick: 1234, scenario: Some("quickstart".into()), gamefiles: boot::fingerprint(dir).unwrap() }
}

pub(super) fn record(dir:&Path,base:Root,mut choose:impl FnMut(&Decision)->Option<String>+'static)->(Snapshot,Snapshot,Vec<u32>) {
    let output=Rc::new(RefCell::new(None));let out=output.clone();let saved=base.clone();
    let p=Play::with_service(move|p| {
        let d=p.pending().unwrap();
        if let Some(pick)=choose(d) {p.choose(Choice::new(d.id.clone(),pick.as_str())).unwrap();return}
        let snapshot=p.snapshot(&saved,"acceptance","Acceptance","now",env!("MT_BUILD_HASH")).unwrap();
        let proof=Snapshot {root:saved.clone(),journal:p.journal().clone(),..snapshot.clone()};
        *out.borrow_mut()=Some((snapshot,proof,p.frame().to_vec()));SessionExit::AwaitingDecision.leave();
    });
    let observed:Rc<RefCell<Observed>>=Rc::default();
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&base,dir,Path::new("."),Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed))).unwrap_err();
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)),"{:?}",exit.downcast_ref::<SessionExit>());
    let result=output.borrow_mut().take().unwrap();result
}

pub(super) fn scenario_root(dir:&Path,name:&str)->Root {
    Root {scenario:Some(format!("scenario:{}",Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios").join(format!("{name}.toml")).display())),..root(dir)}
}

#[test]
fn scenario_npc_forms_restore_their_sheets_and_exact_frame() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let folder=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/fx-npcsnap/{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&folder).unwrap();
    for (name,npc) in [
        ("role","\n[[npc]]\nid = 'snapshot_person'\nrole = 'thug'\nrecord = 20\nat = [32, 10]\nattitude = 'neutral'\n"),
        ("set","\n[[npcs.set]]\nrecord = 20\ncell = [32, 10]\nlevel = 0\nhostile = false\n"),
    ] {
        let path=folder.join(format!("{name}.toml"));
        std::fs::write(&path,format!("{}{npc}",include_str!("../../../../scenarios/efate_quiet.toml"))).unwrap();
        let prepared=crate::scenario::load(&path,&dir).unwrap();
        let expected=serde_json::to_value(&prepared.extensions).unwrap();
        assert_eq!(prepared.extensions.npc_sheets.contains_key(&20),name=="role");
        let base=Root {scenario:Some(format!("scenario:{}",path.display())),..root(&dir)};
        let (snapshot,_,frame)=record(&dir,base,|d| {assert_eq!(d.kind,Kind::Ground);None});
        assert_eq!(snapshot.root.kind,RootKind::Anchor);
        assert!(snapshot.journal.entries.is_empty());
        let saved=mt1::save::Saved::decode(snapshot.root.image.as_deref().unwrap()).unwrap();
        let host:HostState=serde_json::from_slice(&saved.play.unwrap().host).unwrap();
        assert_eq!(serde_json::to_value(&host.extensions).unwrap(),expected);
        let legacy:HostState=serde_json::from_slice(&serde_json::to_vec(&prepared.extensions).unwrap()).unwrap();
        assert_eq!(serde_json::to_value(legacy.extensions).unwrap(),expected);
        let (restored,shown)=replay(&dir,snapshot.clone());
        assert_eq!(restored.hash,snapshot.hash);assert_eq!(shown,frame);
        let (again,shown)=replay(&dir,restored);
        assert_eq!(again.hash,snapshot.hash);assert_eq!(shown,frame);
        first_present(&dir,snapshot,&frame);
    }
}

#[test]
fn journal_history_before_an_image_survives_snapshot_restore() {
    let dir=assets(); let base=scenario_root(&dir,"efate_quiet");
    let folder=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8v-int-r1/journal-{}-{}",std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let observed:Rc<RefCell<Observed>>=Rc::default();
    let mut history=mt1::ui::Scene::program(mt1::ui::ScreenKind::Ground);
    history.party.0=vec![0;0xF86];history.party.0[0]=1;
    observed.borrow_mut().story.observe(&history);
    assert!(observed.borrow().story.unlocked.contains("J3"));
    let host=Rc::new(RefCell::new(Host {reader:Box::new(io::Cursor::new(vec![])),writer:Box::new(io::sink()),
        dir:dir.clone(),exe_dir:PathBuf::from("."),snapshots:folder.clone(),initialized:true,observed:observed.clone(),root:Some(base.clone()),pending:None}));
    let output:Rc<RefCell<Option<Value>>>=Rc::default();let out=output.clone();let h=host.clone();
    let p=Play::with_service(move|p| {
        assert_eq!(p.pending().unwrap().kind,Kind::Ground);
        assert_eq!(h.borrow().observation(p)["position"]["system"],2);
        let before=h.borrow_mut().call("journal",&json!({}),json!(1),Some(p)).unwrap().unwrap();
        *out.borrow_mut()=Some(before);
        let saved=h.borrow_mut().call("snapshot_save",&json!({"name":"History"}),json!(2),Some(p)).unwrap().unwrap();
        h.borrow_mut().call("snapshot_restore",&json!({"id":saved["id"]}),json!(3),Some(p)).unwrap();
        panic!("restore must restart the native session");
    });
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&base,&dir,Path::new("."),
        Box::new(Answerer {play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed.clone()))).unwrap_err();
    let Control::Restart(root,Some(snapshot))=*exit.downcast::<Control>().expect("snapshot_restore restart") else {panic!("expected snapshot")};
    assert_eq!(root.kind,RootKind::Anchor);assert!(snapshot.journal.entries.is_empty());
    *observed.borrow_mut()=Observed::default();host.borrow_mut().root=Some(root.clone());
    let expected=output.borrow_mut().take().unwrap();let h=host.clone();
    let mut p=Play::with_service(move|p| {
        let after=h.borrow_mut().call("journal",&json!({}),json!(4),Some(p)).unwrap().unwrap();
        assert_eq!(after,expected,"full image restore lost a pre-anchor story unlock");
        SessionExit::AwaitingDecision.leave();
    });
    p.restore(snapshot).unwrap();
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&root,&dir,Path::new("."),
        Box::new(Answerer {play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed))).unwrap_err();
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
}

#[test]
fn live_save_prompt_uses_handoff_slot() {
    struct Live {keys:mt_engine::io::Scripted,dir:PathBuf}
    impl Input for Live {
        fn next_key(&mut self)->u16 {self.keys.next_key()}
        fn poll(&mut self,c:&mut dyn mt_engine::sim::tick::Clock)->Event {self.keys.poll(c)}
        fn live_save_location(&self)->Option<(&Path,&'static str)> {Some((&self.dir,"live-window"))}
    }
    let Some(dir)=oracle_env::gamefiles() else {return};
    let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    let mut g=crate::scenario::load(&path,&dir).unwrap().game;
    let saves=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8v-v1/live-save-{}",std::process::id()));
    g.input=Box::new(Live{keys:mt_engine::io::Scripted::new([b'A' as u16]),dir:saves.clone()});
    assert!(g.save_slot.is_none());
    mt1::ground::starport::save_stop(&mut g,false);
    let saved=mt1::save::Saved::decode(&std::fs::read(saves.join(mt1::save::FILE)).expect("live save prompt must write the normal slot")).unwrap();
    assert_eq!(saved.mode,mt1::save::Mode::Starport);assert!(saved.squad.is_none()&&saved.play.is_none());
}

#[derive(Debug)] struct Presented;
pub(super) fn first_present(dir:&Path,snapshot:Snapshot,expected:&[u32]) {
    struct Screen {seen:Rc<RefCell<Vec<u32>>>}
    impl Input for Screen {
        fn next_key(&mut self)->u16 {panic!("first present must precede input")}
        fn present(&mut self,frame:&[u32]) {*self.seen.borrow_mut()=frame.to_vec();std::panic::resume_unwind(Box::new(Presented))}
    }
    let folder=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/m8v-v1/window").join(format!("{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    crate::play_store::save(&folder,&snapshot).unwrap();
    std::fs::write(folder.join("frame-hash.txt"),launch::frame_hash(expected)).unwrap();
    let seen:Rc<RefCell<Vec<u32>>>=Rc::default();let out=seen.clone();
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||launch::file(&folder.join(format!("{}.json",snapshot.id)),dir,Path::new("."),move|start|Ok((Box::new(Screen{seen:out}),Box::new(mt_engine::sim::tick::Counter::at(start.ticks))))))).unwrap_err();
    assert!(exit.is::<Presented>());assert_eq!(launch::frame_hash(&seen.borrow()),launch::frame_hash(expected));
    assert_eq!(seen.borrow().len(),64000);
}

#[test]
#[ignore = "cross-build snapshot check; requires MT_PLAY_SNAPSHOT and MT_PLAY_FRAME_HASH"]
fn open_external_snapshot_headlessly() {
    struct Screen(String);
    impl Input for Screen {
        fn next_key(&mut self)->u16 {panic!("frame expected first")}
        fn present(&mut self,frame:&[u32]) {assert_eq!(launch::frame_hash(frame),self.0);std::panic::resume_unwind(Box::new(Presented))}
    }
    let path=PathBuf::from(std::env::var_os("MT_PLAY_SNAPSHOT").expect("MT_PLAY_SNAPSHOT"));
    let expected=std::env::var("MT_PLAY_FRAME_HASH").expect("MT_PLAY_FRAME_HASH");
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||launch::file(&path,&assets(),Path::new("."),move|s|Ok((Box::new(Screen(expected)),Box::new(mt_engine::sim::tick::Counter::at(s.ticks))))))).unwrap_err();
    assert!(exit.is::<Presented>());
}

fn first_live_choice(dir:&Path,snapshot:Snapshot,pick:&str)->String {
    struct Live {play:Play,pick:Option<String>,out:Rc<RefCell<Option<String>>>}
    impl Input for Live {
        fn wants_decision(&self)->bool {self.play.wants_decision()}
        fn decide(&mut self,d:&Decision) {
            self.play.decide(d);
            if let Some(pick)=self.pick.take() {let d=self.play.pending().unwrap().clone();let pick=if d.options.iter().any(|o|o.id==pick) {pick} else {d.options.iter().find(|o|o.label.eq_ignore_ascii_case(&pick)).map(|o|o.id.clone()).unwrap_or(pick)};self.play.choose(Choice::new(d.id.clone(),pick.as_str())).unwrap()}
            else {*self.out.borrow_mut()=Some(self.play.pending().unwrap().state.clone());std::panic::resume_unwind(Box::new(Presented))}
        }
        fn next_key(&mut self)->u16 {self.play.next_key()}
        fn key_pending(&mut self)->bool {self.play.key_pending()}
        fn poll(&mut self,c:&mut dyn mt_engine::sim::tick::Clock)->Event {self.play.poll(c)}
        fn intent(&mut self)->Option<Box<dyn Any>> {self.play.intent()}
        fn mouse(&mut self)->MouseState {self.play.mouse()}
        fn set_mouse_position(&mut self,x:i16,y:i16) {self.play.set_mouse_position(x,y)}
    }
    let expected:Rc<RefCell<Option<String>>>=Rc::default();let out=expected.clone();let mut choice=Some(pick.to_string());
    let mut p=Play::with_service(move|p| {
        if let Some(pick)=choice.take() {let d=p.pending().unwrap().clone();let pick=if d.options.iter().any(|o|o.id==pick) {pick} else {d.options.iter().find(|o|o.label.eq_ignore_ascii_case(&pick)).map(|o|o.id.clone()).unwrap_or(pick)};p.choose(Choice::new(d.id.clone(),pick.as_str())).unwrap()}
        else {*out.borrow_mut()=Some(p.pending().unwrap().state.clone());std::panic::resume_unwind(Box::new(Presented))}
    });p.restore(snapshot.clone()).unwrap();let observed:Rc<RefCell<Observed>>=Rc::default();
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&snapshot.root,dir,Path::new("."),Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed))).unwrap_err();
    assert!(exit.is::<Presented>());
    let got:Rc<RefCell<Option<String>>>=Rc::default();let out=got.clone();let pick=pick.to_string();let sequence=snapshot.sequence;
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||launch::run(snapshot,dir,Path::new("."),move|start| {
        let mut p=Play::new();p.start_sequence(sequence-1).unwrap();
        Ok((Box::new(Live{play:p,pick:Some(pick),out}),Box::new(mt_engine::sim::tick::Counter::at(start.ticks))))
    }))).unwrap_err();
    assert!(exit.is::<Presented>(),"{:?}",exit.downcast_ref::<SessionExit>());assert_eq!(*got.borrow(),*expected.borrow());
    let result=expected.borrow_mut().take().unwrap();result
}

#[test]
fn quickstart_self_recorded_110_choices_restore_and_root_proof() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let mut count=0;let mut saw_combat=false;
    let (snapshot,proof,frame)=record(&dir,root(&dir),move|d| {
        if count>=110 {assert!(saw_combat);return None} count+=1;
        if d.kind==Kind::CombatTurn {saw_combat=true;}
        let enabled=|id:&str|d.options.iter().find(|o|o.enabled&&o.id==id).map(|o|o.id.clone());
        enabled("first:party").or_else(||enabled("back")).or_else(||{
            d.options.iter().find(|o|o.enabled&&o.id.starts_with("menu:traveller/")).map(|o|o.id.clone())
        }).or_else(||enabled("cmd:game")).or_else(||d.options.iter().find(|o|o.enabled&&o.label.contains("Efate")).map(|o|o.id.clone()))
          .or_else(||d.default.clone()).or_else(||d.options.iter().find(|o|o.enabled).map(|o|o.id.clone()))
    });
    assert_eq!(proof.journal.entries.len(),110);assert_eq!(snapshot.root.kind,RootKind::Anchor);
    let start=Instant::now();let (restored,shown)=replay(&dir,snapshot.clone());
    assert_eq!(restored.hash,snapshot.hash);assert_eq!(shown,frame);assert!(start.elapsed().as_secs_f64()<5.);
    let start=Instant::now();let (restored,shown)=replay(&dir,proof);
    assert_eq!(restored.hash,snapshot.hash);assert_eq!(shown,frame);assert!(start.elapsed().as_secs_f64()<60.);
    first_present(&dir,snapshot.clone(),&frame);
    first_live_choice(&dir,snapshot,"end_turn");
}

#[test]
fn ground_and_nested_menu_window_frames_and_tamper_rejection() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let base=scenario_root(&dir,"efate_quiet");
    let (ground,_,frame)=record(&dir,base.clone(),|_|None);
    assert_eq!(ground.root.kind,RootKind::Anchor);first_present(&dir,ground.clone(),&frame);
    let mut count=0;
    let (nested,proof,frame)=record(&dir,base,move|d| {
        count+=1;if count==1 {assert_eq!(d.kind,Kind::Ground);return Some("cmd:game".into())}
        assert!(!d.kind.is_command());None
    });
    assert_eq!(nested.root.kind,RootKind::Anchor);assert_eq!(nested.journal.entries.len(),1);
    let start=Instant::now();assert_eq!(replay(&dir,nested.clone()).0.hash,nested.hash);
    eprintln!("NESTED_RESTORE tail={} elapsed_ms={:.3}",nested.journal.entries.len(),start.elapsed().as_secs_f64()*1000.);
    assert_eq!(replay(&dir,proof).0.hash,nested.hash);first_present(&dir,nested.clone(),&frame);
    first_live_choice(&dir,nested,"back");
    for fault in 0..4 {
        let mut broken=ground.clone();
        match fault {
            0=>broken.root.gamefiles="wrong".into(),
            1=>broken.root.image.as_mut().unwrap()[100]^=1,
            2=>{let mut s=mt1::save::Saved::decode(broken.root.image.as_deref().unwrap()).unwrap();s.seed^=1;broken.root.image=Some(s.encode());},
            _=>broken.journal.hash_version=3,
        }
        let error=launch::run(broken,&dir,Path::new("."),|_|panic!("window opened before validation")).unwrap_err();
        assert!(!error.is_empty());
    }
}

fn replay(dir:&Path, snapshot:Snapshot) -> (Snapshot,Vec<u32>) {
    let root=snapshot.root.clone(); let base=root.clone(); let output=Rc::new(RefCell::new(None)); let out=output.clone();
    let mut p=Play::with_service(move |p| {
        *out.borrow_mut()=Some((p.snapshot(&base,"restored","Restored","now",env!("MT_BUILD_HASH")).unwrap(),p.frame().to_vec()));
        SessionExit::AwaitingDecision.leave();
    });
    p.restore(snapshot).unwrap(); let observed:Rc<RefCell<Observed>>=Rc::default();
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&root,dir,Path::new("."),Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed)));
    let exit=exit.unwrap_err();
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)),"{:?}",exit.downcast_ref::<SessionExit>());
    let result=output.borrow_mut().take().unwrap(); result
}

#[test]
fn real_efate_combat_payload_under_10kb() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml");
    let prepared = crate::scenario::load(&path,&dir).unwrap();
    let d = mt1::play::turns::decision(&prepared.game).unwrap();
    let bytes = serde_json::to_vec(&public_decision(&d)).unwrap().len();
    assert!(bytes < 10_000, "combat.turn serialized to {bytes} bytes");
}

#[test]
fn reachable_movement_matches_native_paths_and_parameterized_picks() {
    use mt1::{ground::{orders,round},ui::{Intent,Target,Verb}};
    let Some(dir)=oracle_env::gamefiles() else {return};
    let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml");
    let mut g=crate::scenario::load(&path,&dir).unwrap().game;
    let who=round::selected(&g);let from=round::traveller_cell(&g,who);
    let reachable=orders::reachable(&g,who,3);
    for r in from.0-4..=from.0+4 {for c in from.1-4..=from.1+4 {
        let native=orders::find_path(&g,who,from,(r,c)).is_some_and(|p|p.len()<=3);
        assert_eq!(reachable.binary_search(&(r,c)).is_ok(),native,"cell {r},{c}");
    }}
    let d=mt1::play::turns::decision(&g).unwrap();let movement=d.movement.as_ref().unwrap();
    let (name,cell)=movement.directions.iter().map(|(n,(r,c))|(n,(from.0+r,from.1+c))).find(|(_,c)|movement.cells.binary_search(c).is_ok()).unwrap();
    let mut p=Play::new();p.decide(&d);let before=p.pending().unwrap().state.clone();
    for pick in ["move:north:0","move:north:65536","move:north:-1","move:unknown:1","move:cell/-1,0","menu:cell/1,9999","move:cell/no,2"] {
        assert!(p.choose(Choice::new("d:1",pick)).is_err(),"{pick}");assert_eq!(p.pending().unwrap().state,before);assert!(p.journal().entries.is_empty());
    }
    p.choose(Choice::new("d:1",format!("move:{name}:1").as_str())).unwrap();
    assert_eq!(*p.intent().unwrap().downcast::<Intent>().unwrap(),Intent::Verb{verb:Verb::Move,target:Target::Cell(cell)});
    let mut p=Play::new();p.decide(&d);p.choose(Choice::new("d:1","menu:cell/0,0")).unwrap();
    assert_eq!(*p.intent().unwrap().downcast::<Intent>().unwrap(),Intent::OpenMenu{target:Target::Cell((0,0)),at:(0,0)});
    g.squad.as_mut().unwrap().p1.turns[who as usize].mp=0;
    let d=mt1::play::turns::decision(&g).unwrap();assert!(d.movement.as_ref().unwrap().cells.is_empty());
    let mut p=Play::new();p.decide(&d);assert!(p.choose(Choice::new("d:1",format!("move:cell/{},{}",cell.0,cell.1).as_str())).is_err());
}

#[test]
fn batch_refusal_has_no_decision_or_state_payload() {
    let output = tests::Output(Rc::default());
    let mut host = Host { reader:Box::new(io::Cursor::new(vec![])), writer:Box::new(output.clone()), dir:PathBuf::new(),exe_dir:PathBuf::new(),snapshots:PathBuf::new(),initialized:true,
        observed:Rc::default(),root:None,pending:Some(Pending { batch:true,picks:vec![json!("invalid")].into(),stop:json!({"combat":false}),limit:1,..Pending::start(json!(1)) }) };
    let mut p=Play::new(); p.decide(&Decision::new(Kind::CombatTurn,"t",env!("MT_BUILD_HASH")));
    assert!(!host.advance(&mut p));
    let response:Value=serde_json::from_slice(&output.0.borrow()).unwrap();
    let body:Value=serde_json::from_str(response["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(response["result"]["isError"],true);
    assert_eq!(body["reason"],"refused"); assert_eq!(body["applied"],0);
    assert!(body.get("decision").is_none() && body.get("state").is_none(),"{body}");
    assert!(p.journal().entries.is_empty()); assert_eq!(p.sequence(),1);
}

#[test]
fn space_snapshot_has_image() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m6_efate_orbit_ready.toml");
    let prepared=crate::scenario::load(&path,&dir).unwrap();let mut g=prepared.game;
    g.loop_vars.initialized=true;g.boot_calibrating=false;g.scene=Some(mt1::ui::SceneHook::default());
    g.tooling=Some(Box::new(crate::scenario::Loader::new(Default::default(),dir.clone(),prepared.extensions)));
    let root=root(&dir); let saved_root=root.clone();
    let output=Rc::new(RefCell::new(None));let out=output.clone();
    let next=Rc::new(RefCell::new(None));let after=next.clone();let mut first=true;
    g.input=Box::new(Play::with_service(move|p| {
        if first {
            first=false;assert_eq!(p.pending().unwrap().kind,Kind::Space);
            *out.borrow_mut()=Some((p.snapshot(&saved_root,"space","Space","now",env!("MT_BUILD_HASH")).unwrap(),p.frame().to_vec()));
            p.choose(Choice::new(p.pending().unwrap().id.clone(),"cmd:stations")).unwrap();
        } else {*after.borrow_mut()=Some((p.snapshot(&saved_root,"station","Station","now",env!("MT_BUILD_HASH")).unwrap(),p.frame().to_vec()));SessionExit::AwaitingDecision.leave();}
    }));
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||mt1::space::launch(&mut g))).unwrap_err();
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
    let (snapshot,frame)=output.borrow_mut().take().unwrap();assert_eq!(snapshot.root.kind,RootKind::Anchor);
    let start=Instant::now();let (restored,shown)=replay(&dir,snapshot.clone());
    assert_eq!(restored.hash,snapshot.hash);assert_eq!(frame,shown);assert!(start.elapsed().as_secs_f64()<5.);
    first_present(&dir,snapshot.clone(),&frame);
    let (station,frame)=next.borrow_mut().take().unwrap();
    assert_eq!(first_live_choice(&dir,snapshot,"cmd:stations"),station.hash);
    assert_eq!(replay(&dir,station.clone()).0.hash,station.hash);
    first_present(&dir,station,&frame);
}

#[test]
#[ignore = "historical trace benchmark; requires MT_PLAY_TRACE and BYO gamefiles"]
fn historical_101_decisions() {
    let trace = std::fs::read_to_string(std::env::var_os("MT_PLAY_TRACE").expect("MT_PLAY_TRACE")).unwrap();
    let rows: Vec<Value> = trace.lines().map(|s| serde_json::from_str(s).unwrap()).filter(|v: &Value| v.get("branch").is_none() && v["decision"].as_str().and_then(|s|s.strip_prefix("d:")).and_then(|s|s.parse::<usize>().ok()).is_some_and(|n|n<=101)).collect();
    assert_eq!(rows.len(), 101);
    let dir = assets(); let root = root(&dir);
    let result = Rc::new(RefCell::new(None)); let captured = result.clone(); let saved_root = root.clone();
    let start = Instant::now(); let mut last = start; let mut max_payload = 0;let mut worst:Option<Snapshot>=None;
    let p = Play::with_service(move |p| {
        let d = p.pending().unwrap();
        let payload = serde_json::to_vec(&public_decision(d)).unwrap().len(); max_payload = max_payload.max(payload);
        eprintln!("DECISION {} {} wall_ms={:.3} bytes={payload} anchor={:?}", d.id, d.kind.as_str(), last.elapsed().as_secs_f64()*1000., p.latest_anchor().map(|a|a.sequence)); last = Instant::now();
        let tail=p.sequence()-1-p.latest_anchor().map_or(0,|a|a.sequence);
        if worst.as_ref().is_none_or(|s|tail>s.journal.entries.len() as u64) {worst=Some(p.snapshot(&saved_root,"worst","Longest native replay tail","now",env!("MT_BUILD_HASH")).unwrap());}
        if p.sequence() == 102 {
            let snapshot = p.snapshot(&saved_root, "historical", "S2", "now", env!("MT_BUILD_HASH")).unwrap();
            eprintln!("HISTORICAL elapsed_ms={:.3} max_payload={max_payload} replay_tail={} root={:?}", start.elapsed().as_secs_f64()*1000., snapshot.journal.entries.len(), snapshot.root.kind);
            let proof=Snapshot {root:saved_root.clone(),journal:p.journal().clone(),..snapshot.clone()};
            *captured.borrow_mut() = Some((snapshot,proof,p.frame().to_vec(),worst.take().unwrap())); SessionExit::AwaitingDecision.leave();
        }
        let row = &rows[p.sequence() as usize - 1]; assert_eq!(d.id, row["decision"]);
        p.choose(Choice::new(d.id.clone(), pick(&row["pick"],d).unwrap())).unwrap();
    });
    let observed: Rc<RefCell<Observed>> = Rc::default();
    let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| boot::run(&root,&dir,Path::new("."),Box::new(Answerer { play:p, observed:observed.clone(), anchors:true,heartbeat:None }),observed)));
    assert!(matches!(exit.unwrap_err().downcast_ref::<SessionExit>(), Some(SessionExit::AwaitingDecision)));
    let (snapshot,proof,frame,worst)=result.borrow_mut().take().unwrap();
    let start=Instant::now(); let (restored,restored_frame)=replay(&dir,snapshot.clone()); let restore=start.elapsed();
    assert_eq!(restored.hash,snapshot.hash); assert_eq!(restored_frame,frame);
    eprintln!("RESTORE elapsed_ms={:.3} tail={}",restore.as_secs_f64()*1000.,snapshot.journal.entries.len());
    assert!(restore.as_secs_f64()<5.);
    let start=Instant::now(); let (from_root,root_frame)=replay(&dir,proof); let root_time=start.elapsed();
    assert_eq!(from_root.hash,snapshot.hash); assert_eq!(root_frame,frame);
    eprintln!("ROOT_REPLAY elapsed_ms={:.3}",root_time.as_secs_f64()*1000.);
    assert!(root_time.as_secs_f64()<60.);
    let start=Instant::now();let (restored,_)=replay(&dir,worst.clone());assert_eq!(restored.hash,worst.hash);
    eprintln!("WORST_TAIL_RESTORE decision={} tail={} elapsed_ms={:.3}",worst.sequence,worst.journal.entries.len(),start.elapsed().as_secs_f64()*1000.);
    assert!(start.elapsed().as_secs_f64()<5.);
}
