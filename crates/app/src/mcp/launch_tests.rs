use super::*;
use super::restore_tests::{assets,root,scenario_root,record};

#[test]
fn stale_build_is_refused_before_factory() {
    let dir=assets();let (mut s,_,_)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);
    for build in ["stale-deadbeef0000","source archive"] {
        s.build=build.into();
        let error=launch::run(s.clone(),&dir,Path::new("."),|_|Err("factory called".into())).unwrap_err();
        assert!(error.contains(build)&&error.contains(env!("MT_BUILD_HASH")),"{error}");
    }
}

#[test]
fn title_and_chargen_targets_open_on_their_frame() {
    let dir=assets();
    for (start,picks) in [("boot",0),("chargen",0),("chargen",3)] {
        let base=Root{scenario:Some(start.into()),..root(&dir)};let mut n=0;
        let (s,_,frame)=record(&dir,base,move|d|{n+=1;(n<=picks).then(||d.options[0].id.clone())});
        super::restore_tests::first_present(&dir,s,&frame);
    }
}

#[test]
fn message_history_survives_an_image() {
    let dir=assets();let (mut s,_,_)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);
    let observed:Rc<RefCell<Observed>>=Rc::default();
    observed.borrow_mut().messages.push("queued-before-anchor".into());
    observed.borrow_mut().loaded=Some((2,0,0));
    let mut a=Answerer{play:Play::new(),observed,anchors:true,heartbeat:None};
    a.anchor("t.6923",s.root.image.as_ref().unwrap());
    s.root.image=Some(a.play.latest_anchor().unwrap().image.clone());
    let observed:Rc<RefCell<Observed>>=Rc::default();let seen=observed.clone();
    let mut p=Play::with_service(move |_| {
        assert_eq!(seen.borrow().messages,vec!["queued-before-anchor"]);
        SessionExit::AwaitingDecision.leave();
    });p.restore(s.clone()).unwrap();
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&s.root,&dir,Path::new("."),
        Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed)));
    assert!(matches!(result.unwrap_err().downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
}

#[test]
fn text_survives_first_handoff_frame() {
    use mt_engine::present::textlog;
    struct Screen(textlog::TextLog);
    impl Input for Screen {
        fn next_key(&mut self)->u16 {panic!("first frame expected")}
        fn present(&mut self,frame:&[u32]) {
            let mut got=textlog::take_frame(frame).expect("restored text log");
            for r in got.runs.iter_mut().chain(got.visible.iter_mut()) {r.font_id=0;}
            assert_eq!(got.visible,self.0.visible,"first frame text surface");
            SessionExit::AwaitingDecision.leave();
        }
    }
    textlog::set_enabled(true);
    let dir=assets();let (s,_,frame)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);
    let mut expected=textlog::take_frame(&frame).expect("recorded text log");
    assert!(!expected.visible.is_empty());
    for r in expected.runs.iter_mut().chain(expected.visible.iter_mut()) {r.font_id=0;}
    textlog::set_enabled(false);textlog::set_enabled(true);
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||launch::run(s,&dir,Path::new("."),move|start|
        Ok((Box::new(Screen(expected)),Box::new(mt_engine::sim::tick::Counter::at(start.ticks))))))).unwrap_err();
    textlog::set_enabled(false);
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
}

#[test]
fn changed_code_assets_refused_before_window() {
    let dir=assets();let (mut s,_,_)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);
    let altered=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8v-int-r1b/altered-{}",std::process::id()));
    std::fs::create_dir_all(&altered).unwrap();
    for name in mt1::REQUIRED_FILES {std::fs::copy(mt1::gamefiles::find(&dir,name).unwrap(),altered.join(name)).unwrap();}
    let file=altered.join("STARTUP.EXE");let mut bytes=std::fs::read(&file).unwrap();bytes[500]^=1;std::fs::write(file,bytes).unwrap();
    s.root.gamefiles=boot::fingerprint(&altered).unwrap();
    let error=launch::run(s,&altered,Path::new("."),|_|Err("factory called".into())).unwrap_err();
    assert!(error.contains("REFUSED: STARTUP.EXE"),"{error}");
}

#[test]
fn refused_parent_capture_keeps_space_running() {
    let dir=assets();let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m6_efate_orbit_ready.toml");
    let prepared=crate::scenario::load(&path,&dir).unwrap();let mut g=prepared.game;
    g.loop_vars.initialized=true;g.boot_calibrating=true;
    g.tooling=Some(Box::new(crate::scenario::Loader::new(Default::default(),dir,prepared.extensions)));
    g.input=Box::new(Play::with_service(|p| {assert_eq!(p.pending().unwrap().kind,Kind::Space);assert!(p.latest_anchor().is_none());SessionExit::AwaitingDecision.leave();}));
    let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||mt1::space::launch(&mut g)));
    assert!(result.is_err(),"capture refusal aborted launch: {result:?}");
    assert!(matches!(result.unwrap_err().downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
}

#[test]
fn product_station_offers_crew_records() {
    struct Check;
    impl Input for Check {
        fn next_key(&mut self)->u16 {0}
        fn wants_decision(&self)->bool {true}
        fn decide(&mut self,d:&Decision) {assert_eq!(d.options.iter().filter(|o|o.id.starts_with("cmd:char/")).count(),5,"crew records offered");}
    }
    let dir=assets();let files=mt_engine::io::Dir(dir);
    use mt_engine::io::Files;
    let mut sp=mt1::space::Space::new(&files.read("S.DAT").unwrap(),files.read("DEFAULT.SAV").unwrap(),vec![],Box::new(Check),Box::new(mt_engine::sim::tick::Counter::default()),Box::new(files)).unwrap();
    sp.product=true;
    mt1::space::play::station_state(&mut sp,"station",&[]);
}
#[test]
fn product_station_crew_hotkey_opens_and_closes_the_record() {
    let dir=assets();let files=mt_engine::io::Dir(dir);
    use mt_engine::io::Files;use mt1::space::stations::Host;
    let mut sp=mt1::space::Space::new(&files.read("S.DAT").unwrap(),files.read("DEFAULT.SAV").unwrap(),vec![],Box::new(mt_engine::io::Scripted::new([mt_engine::io::keys::ESC])),Box::new(mt_engine::sim::tick::Counter::default()),Box::new(files)).unwrap();
    sp.product=true;sp.s.set_pbyte(0x127,10);
    let mut h=mt1::space::host::StationHost::new(&mut sp,vec![0;65536],mt1::space::host::TOP_FRAME);
    assert!(h.call("crew_record",&[0]).is_ok(),"the record opens and Esc closes it");
}

#[test]
fn scenario_request_reaches_restored_loader() {
    let dir=assets();let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    let prepared=crate::scenario::load(&path,&dir).unwrap();let mut g=prepared.game;
    let request=crate::scenario::Request::default();
    let observed=Rc::new(RefCell::new(Observed{desktop:Some(request.clone()),..Default::default()}));
    boot::attach(&mut g,&dir,prepared.extensions,observed,true);
    *request.borrow_mut()=Some(path);g.mem.set_x(999);
    let mut tooling=g.tooling.take().unwrap();
    assert!(tooling.poll(&mut g),"scenario menu/debug request was ignored");
    assert_ne!(g.mem.x(),999);
}

#[test]
fn game_over_returns_live_session_for_title() {
    let dir=assets();let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m6_efate_orbit_ready.toml");
    let prepared=crate::scenario::load(&path,&dir).unwrap();let mut g=prepared.game;
    g.loop_vars.initialized=true;g.boot_calibrating=false;g.scene=Some(mt1::ui::SceneHook::default());
    g.tooling=Some(Box::new(crate::scenario::Loader::new(Default::default(),dir.clone(),prepared.extensions)));
    let base=root(&dir);let output=Rc::new(RefCell::new(None));let out=output.clone();
    let mut first=true;g.input=Box::new(Play::with_service(move|p| {if first {first=false;let id=p.pending().unwrap().id.clone();p.choose(Choice::new(id,"cmd:launch")).unwrap();return}
        *out.borrow_mut()=Some(p.snapshot(&base,"over","Over","now",env!("MT_BUILD_HASH")).unwrap());SessionExit::AwaitingDecision.leave();}));
    let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||mt1::space::launch(&mut g))).unwrap_err();
    assert!(matches!(exit.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)));
    let snapshot=output.borrow_mut().take().unwrap();
    let result=launch::run_desktop(snapshot,&dir,Path::new("."),Default::default(),|start| {
        struct RunningClock(u32);
        impl mt_engine::sim::tick::Clock for RunningClock {
            fn now(&mut self)->u32 {self.0=self.0.wrapping_add(1);self.0}
            fn set(&mut self,n:u32){self.0=n}
            fn wait_tick(&mut self){self.0=self.0.wrapping_add(1)}
        }
        struct Live {p:Play, visits:usize,polls:usize}
        impl Input for Live {
            fn wants_decision(&self)->bool {self.p.wants_decision()}
            fn decide(&mut self,d:&Decision) {
                self.visits+=1;assert!(self.visits<=10,"quit did not finish");self.p.decide(d);
                let id=if d.kind==Kind::Space {"cmd:quit".to_string()} else {d.options.iter().find(|o|o.enabled && (o.id=="yes" || o.id=="accept" || o.id=="continue")).unwrap_or_else(||panic!("unexpected quit prompt {d:?}")).id.clone()};
                self.p.choose(Choice::new(self.p.pending().unwrap().id.clone(),id.as_str())).unwrap();
            }
            fn next_key(&mut self)->u16 {self.p.next_key()}
            fn key_pending(&mut self)->bool {self.p.key_pending()}
            fn poll(&mut self,c:&mut dyn mt_engine::sim::tick::Clock)->Event {self.polls+=1;assert!(self.polls<100000,"quit stalled");self.p.poll(c)}
        }
        Ok((Box::new(Live{p:Play::new(),visits:0,polls:0}),Box::new(RunningClock(start.ticks))))
    }).unwrap();
    assert!(result.is_some(),"game over discarded the live window instead of returning it to the title loop");
    let g=result.unwrap();assert!(g.game_over);assert!(!g.input.closed());
}

#[test]
fn mcp_refuses_stale_snapshot_build() {
    let dir=assets();let (mut snapshot,_,_)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);
    snapshot.build="stale-mcp-build".into();
    let folder=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8v-int-r1b/stale-mcp-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    crate::play_store::save(&folder,&snapshot).unwrap();
    let mut host=Host{reader:Box::new(io::Cursor::new(vec![])),writer:Box::new(io::sink()),dir,exe_dir:PathBuf::from("."),snapshots:folder,initialized:true,observed:Rc::default(),root:None,pending:None};
    let error=host.call("snapshot_restore",&json!({"id":snapshot.id}),json!(1),None).unwrap_err();
    assert!(error.contains("stale-mcp-build")&&error.contains(env!("MT_BUILD_HASH")),"{error}");
    snapshot.build="source archive".into();assert!(crate::play_store::compatible_build(&snapshot,"source archive").is_err());
    snapshot.build=env!("MT_BUILD_HASH").into();assert!(crate::play_store::compatible_build(&snapshot,env!("MT_BUILD_HASH")).is_ok());
}

#[test]
fn stale_build_precedes_incompatible_image_decode() {
    let dir=assets();let (mut s,_,_)=record(&dir,scenario_root(&dir,"efate_quiet"),|_|None);s.build="older-image-build".into();
    let folder=Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("../../target/m8v-int-r1b/stale-image-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    crate::play_store::save(&folder,&s).unwrap();
    std::fs::write(folder.join(format!("{}.mtsav",s.id)),b"an image from an incompatible decoder").unwrap();
    let error=launch::file(&folder.join(format!("{}.json",s.id)),&dir,Path::new("."),|_|panic!("no stale window")).unwrap_err();
    assert!(error.contains("older-image-build")&&error.contains(env!("MT_BUILD_HASH")),"{error}");
}
