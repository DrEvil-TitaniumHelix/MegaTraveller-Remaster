use super::*;
use std::io::Cursor;
use mt_engine::io::Event;
#[derive(Clone)]
pub(super) struct Output(pub(super) Rc<RefCell<Vec<u8>>>);
impl Write for Output {
    fn write(&mut self,b:&[u8])->io::Result<usize>{self.0.borrow_mut().extend_from_slice(b);Ok(b.len())}
    fn flush(&mut self)->io::Result<()>{Ok(())}
}
fn observation()->Observation {
    let mut o=Observation::program(mt1::ui::ScreenKind::Ground,123);
    o.scene.party.0=vec![0;0xF86];o.scene.party.0[6]=2;o.scene.party.0[7]=0;o.scene.pos=(64,18);o
}
#[test]
fn batch_stops_at_changed_state_even_after_the_last_pick() {
    for (condition,reason) in [("area","area"),("system","system"),("program","program"),("encounter","combat"),("message","message")] {
        for final_pick in [false,true] {
            let picks=if final_pick {json!(["go"])}else{json!(["go","never-apply"])};
            let mut stop=json!({condition:true});if condition!="encounter"{stop["combat"]=json!(false)}
            let request=json!({"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"choose_until","arguments":{"decision":"d:1","picks":picks,"stop":stop}}});
            let output=Output(Rc::default());let observed=Rc::new(RefCell::new(Observed{state:Some(observation()),..Default::default()}));
            let host=Rc::new(RefCell::new(Host{reader:Box::new(Cursor::new(format!("{request}\n").into_bytes())),writer:Box::new(output.clone()),
                dir:PathBuf::new(),exe_dir:PathBuf::new(),snapshots:PathBuf::new(),initialized:true,observed:observed.clone(),root:None,pending:None}));
            let service=host.clone();let mut play=Play::with_service(move|p|service.borrow_mut().service(p));
            let mut d=Decision::new(Kind::Menu,"t","test");d.options.push(mt_engine::play::Option::new("go","Go",vec![Event::Key(13)]));
            play.decide(&d);assert_eq!(play.next_key(),13);
            assert_eq!(play.poll(&mut mt_engine::sim::tick::Counter::default()),Event::KeyUp(13));
            {
                let mut seen=observed.borrow_mut();let o=seen.state.as_mut().unwrap();
                match condition {
                    "area"=>o.scene.party.0[7]=1,
                    "system"=>o.scene.party.0[6]=3,
                    "program"=>d.context.program="s".into(),
                    "encounter"=>o.flight=Some(mt1::play::observation::Flight{position:(0,0),velocity:(0,0),heading:0,thrust:0,station:0,encounter:1}),
                    "message"=>seen.messages.push("A newly observed game message".into()),_=>unreachable!(),
                }
            }
            play.decide(&d);
            let exit=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||play.next_key())).unwrap_err();assert!(matches!(exit.downcast_ref::<Control>(),Some(Control::Eof)));
            let bytes=output.0.borrow();let responses:Vec<_>=bytes.split(|&b|b==b'\n').filter(|b|!b.is_empty()).map(|b|serde_json::from_slice::<Value>(b).unwrap()).collect();
            assert_eq!(responses.len(),1);assert_eq!(responses[0]["result"]["isError"],false,"{condition}: {}",responses[0]);
            let body:Value=serde_json::from_str(responses[0]["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(body["reason"],reason);assert_eq!(body["applied"],1);assert_eq!(play.journal().entries.len(),1);assert_eq!(play.pending().unwrap().id,"d:2");
        }
    }
}

#[test]
fn observer_encoding_does_not_mutate_state_and_base64_handles_tails() {
    for (raw,expected) in [(b"".as_slice(),""),(b"f","Zg=="),(b"fo","Zm8="),(b"foo","Zm9v"),(b"foob","Zm9vYg==")] {assert_eq!(frame::base64(raw),expected)}
    assert!(frame::png(&[]).is_err());
    let o=observation();let before=format!("{o:?}");let observed=Observed{state:Some(o),..Default::default()};
    let d=Decision::new(Kind::Ground,"t","test");let s=state::value(&d,&observed);
    assert_eq!(s["position"],json!({"system":2,"world":2,"area":0,"level":0,"cell":[32,9]}));assert_eq!(s["time"]["tick"],123);
    assert_eq!(format!("{:?}",observed.state.as_ref().unwrap()),before);
}

#[test]
fn anchor_requires_loader_initialization_for_the_captured_level() {
    let mut g=mt1::ground::Ground::from_ds(vec![0;65536],vec![0;0xF86],Box::new(mt_engine::io::Scripted::new([])),
        Box::new(mt_engine::sim::tick::Counter::default()),Box::new(mt_engine::io::Overlay::new(None)));
    g.mem.set_party_u8(6,2);g.mem.set_party_u8(7,3);g.mem.set_level(1);
    let image=mt1::save::Saved::capture(&mut g,mt1::save::Mode::Ground,"").encode();
    assert!(!boot::anchor_ready(&image,None),"first loop precedes loader initialization");
    assert!(!boot::anchor_ready(&image,Some((2,3,0))),"new level precedes its loader poll");
    assert!(boot::anchor_ready(&image,Some((2,3,1))));
    assert!(!boot::anchor_ready(b"invalid",Some((2,3,1))));
}

#[test]
fn host_continuation_accepts_v1_images_and_preserves_story() {
    let mut extensions=crate::scenario::Extensions::default();
    extensions.pending_srand.insert("s".into(),1234);
    extensions.pending_space.insert("fuel".into(),json!(124));
    let legacy=serde_json::to_vec(&extensions).unwrap();
    let mut host:HostState=serde_json::from_slice(&legacy).unwrap();
    assert!(host.story.unlocked.is_empty());
    host.story.unlocked.insert("J3".into());
    let restored:HostState=serde_json::from_slice(&serde_json::to_vec(&host).unwrap()).unwrap();
    assert_eq!(restored.story.unlocked,host.story.unlocked);
    assert_eq!(serde_json::to_value(restored.extensions).unwrap(),serde_json::to_value(extensions).unwrap());
}

#[test]
fn money_events_include_individual_cash_even_when_shared_balance_is_unchanged() {
    let before=json!({"credits":500,"party":[{"slot":0,"credits":100},{"slot":2,"credits":250}]});
    let after=json!({"credits":500,"party":[{"slot":2,"credits":250},{"slot":0,"credits":70}]});
    assert_eq!(state::events(&before,&after),vec![json!({"type":"money","slot":0,"before":100,"after":70})]);
    assert!(state::events(&after,&after).is_empty());
    let mut bank=after.clone();bank["credits"]=json!(600);
    assert_eq!(state::events(&after,&bank),vec![json!({"type":"money","before":500,"after":600})]);
}

#[test]
fn squad_health_observation_reads_pools_and_initial_rules_without_mutation() {
    use mt1::ground::{state::{Far,Mem,MAP_PTR},squad,rules::compendium::Compendium};
    let mut g=mt1::ground::Ground::from_ds(vec![0;65536],vec![0;Mem::PARTY_SIZE],Box::new(mt_engine::io::Scripted::new([])),
        Box::new(mt_engine::sim::tick::Counter::default()),Box::new(mt_engine::io::Overlay::new(None)));
    g.mem.alloc(0x8000,20*12*3);g.mem.set_pointer(MAP_PTR,Far::new(0,0x8000));g.mem.set_u8(0x6D2B,20);
    g.mem.alloc(0x9100,46*140);g.mem.set_pointer(0x6D6D,Far::new(0,0x9100));g.mem.set_party_u8(6,2);
    g.scene=Some(mt1::ui::SceneHook::default());g.squad=Some(squad::State::with_compendium(Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap()));
    g.squad.as_mut().unwrap().encounter=true;
    for (k,picture,hp,col) in [(14,41,12,15),(9,55,255,16),(6,200,19,17)] {
        let n=Mem::npc(k);g.mem.write(n.add(9),picture,1);g.mem.write(n.add(10),hp,1);
        g.mem.write(n.add(32),18,2);g.mem.write(n.add(34),col*2,2);
        g.mem.write(Far::new(((9*20+col)*3+2) as u16,0x8000),k as u32,1);
    }
    let observe=|g:&mt1::ground::Ground| {
        let before=mt1::ui::Scene::capture(g,mt1::ui::ScreenKind::Ground,None);
        let squad=g.squad.clone();let seed=g.rand.lcg.seed;
        let o=Observation::ground(g,123);let d=Decision::new(Kind::CombatTurn,"t","fixture");
        let value=state::value(&d,&Observed{state:Some(o),..Default::default()});
        assert_eq!(g.squad,squad,"observation must not seed pools or log derived records");assert_eq!(g.rand.lcg.seed,seed);
        assert_eq!(format!("{before:?}"),format!("{:?}",mt1::ui::Scene::capture(g,mt1::ui::ScreenKind::Ground,None)));
        value
    };
    let hits=|v:&Value,k|v["combat"]["npcs"].as_array().unwrap().iter().find(|n|n["record"]==k).unwrap()["hits"].clone();
    let initial=observe(&g);let role=g.squad.as_ref().unwrap().compendium.roles["thug"].hits;let full=role.0+role.1;
    assert_eq!(hits(&initial,14),full);assert_eq!(hits(&initial,9),Value::Null);assert_eq!(hits(&initial,6),19);
    assert!(!squad::npc_take(&mut g,14,2));let wounded=observe(&g);assert_eq!(hits(&wounded,14),full-2);
    assert_eq!(g.mem.read(Mem::npc(14).add(10),1),12,"the native field is not current squad health");
    assert!(!squad::npc_take(&mut g,9,2));assert_eq!(hits(&observe(&g),9),Value::Null);
    assert!(!squad::npc_take(&mut g,6,3));assert_eq!(hits(&observe(&g),6),16);
}
