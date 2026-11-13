mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
use super::*;
fn put(table: &mut toml::Value, key: &str, value: toml::Value) { table.as_table_mut().unwrap().insert(key.into(),value); }
fn gamefiles()->Option<PathBuf> { oracle_env::gamefiles() }
fn root()->PathBuf {Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")}
#[test]
fn autofire_scenarios_prove_the_actual_target_range_and_ammo() {
    use ground::{npc_turn, round, state::Mem};
    let Some(dir)=gamefiles() else {return};
    for (name,distance,bursts) in [("short",4,true),("far",8,false),("boxed",8,true)] {
        for enabled in [true,false] {
            let mut prepared=load(&root().join(format!("scenarios/efate_auto_{name}.toml")),&dir).unwrap();
            let g=&mut prepared.game; g.balance.npc_selective_autofire=enabled;
            assert_eq!(round::squares(round::npc_cell(g,14),round::traveller_cell(g,0)),distance);
            assert_eq!(round::sees(g,14),Some(0),"{name}: target visible before any movement");
            npc_turn::turn(g,14);
            let s=&g.squad.as_ref().unwrap().p1;
            let full=!enabled||bursts;
            assert_eq!(g.mem.read(Mem::npc(14).add(0x1E),1),if full {40} else {49},"{name}: {:?}",s.log);
            assert_eq!(s.log.iter().any(|l|l.contains("(burst, 10 rounds)")),full);
            assert_eq!(s.log.iter().any(|l|l.contains("is boxed (no free orthogonal neighbour)")),enabled&&name=="boxed");
        }
    }
}
#[test]
fn reload_installs_prepared_counter_including_saved_ticks() {
    use ground::Tooling;
    let Some(dir)=gamefiles() else {return};
    let mut before=load(&root().join("scenarios/m7_starport_before_save.toml"),&dir).unwrap();
    let mut saved=mt1::save::Saved::capture(&mut before.game,mt1::save::Mode::Ground,env!("MT_BUILD_HASH"));
    saved.now=100; saved.ticks=123; saved.bios_ticks=100u32.to_le_bytes();
    let tmp=root().join(format!("target/scenario-clock-{}",std::process::id())); fs::create_dir_all(&tmp).unwrap();
    fs::write(tmp.join("clock.mtsav"),saved.encode()).unwrap();
    let saved_path=tmp.join("clock.toml");
    fs::write(&saved_path,"[meta]\nformat = 1.3\nname = \"clock\"\nversion = \"1\"\nauthor = \"test\"\npurpose = \"test\"\n[save]\nfile = \"clock.mtsav\"\n").unwrap();
    for path in [root().join("scenarios/efate_quiet.toml"),saved_path] {
        let mut fresh=load(&path,&dir).unwrap();
        if path.starts_with(&tmp) { assert_eq!((fresh.game.clock.now(),fresh.game.clock.ticks()),(100,123)); }
        let mut live=load(&path,&dir).unwrap();
        live.game.balance=ground::rules::balance::Options {grenade_range:false,npc_selective_autofire:false};
        let balance=live.game.balance;
        live.game.clock=Box::new(Counter::at(500));
        live.game.clock.set(900);
        let request=Request::default(); *request.borrow_mut()=Some(path.clone());
        let mut loader=Loader::new(request,dir.clone(),Extensions::default());
        assert!(loader.poll(&mut live.game));
        assert_eq!(live.game.balance,balance,"reloading the clock also retains rule preferences");
        for _ in 0..3 {
            assert_eq!((live.game.clock.now(),live.game.clock.ticks()),
                (fresh.game.clock.now(),fresh.game.clock.ticks()),"{}: complete clock state after reload/advance",path.display());
            live.game.clock.wait_tick(); fresh.game.clock.wait_tick();
        }
    }
}
#[test]
fn desktop_reload_retains_shared_clock_and_balance_preferences() {
    use ground::Tooling;
    #[derive(Clone)] struct Shared(std::rc::Rc<std::cell::RefCell<Counter>>);
    impl mt_engine::sim::tick::Clock for Shared {
        fn now(&mut self)->u32 { self.0.borrow_mut().now() }
        fn set(&mut self,n:u32) { self.0.borrow_mut().set(n) }
        fn wait_tick(&mut self) { self.0.borrow_mut().wait_tick() }
    }
    let Some(dir)=gamefiles() else {return};
    let path=root().join("scenarios/efate_quiet.toml");
    let mut live=load(&path,&dir).unwrap();
    let prepared_now=live.game.clock.now();
    let mut shared=Shared(std::rc::Rc::new(std::cell::RefCell::new(Counter::at(500))));
    live.game.clock=Box::new(shared.clone());
    live.game.balance=ground::rules::balance::Options {grenade_range:false,npc_selective_autofire:false};
    let balance=live.game.balance;
    let request=Request::default(); *request.borrow_mut()=Some(path);
    let mut loader=Loader::desktop(request,dir,Extensions::default());
    assert!(loader.poll(&mut live.game));
    assert_eq!(live.game.balance,balance);
    assert_eq!((live.game.clock.now(),shared.now()),(prepared_now,prepared_now));
    shared.set(901);
    assert_eq!(live.game.clock.now(),901,"the window and game still share one clock");
    live.game.clock.wait_tick();
    assert_eq!(live.game.clock.now(),shared.now());
    assert_ne!(live.game.clock.now(),901,"the reloaded game ticks the shared clock");
}
fn sample(dir:&Path)->PathBuf {
    let path=root().join("scenarios/saves/m7_sample.mtsav");
    let mut before=load(&root().join("scenarios/m7_starport_before_save.toml"),dir).unwrap();
    let bytes=mt1::save::Saved::capture(&mut before.game,mt1::save::Mode::Ground,env!("MT_BUILD_HASH")).encode();
    let tmp=path.with_extension(format!("tmp{}",std::process::id()));
    fs::write(&tmp,&bytes).unwrap();fs::rename(&tmp,&path).unwrap();path
}
#[test]
fn every_library_scenario_reads_back_before_first_poll() {
    let Some(dir)=gamefiles() else {eprintln!("SKIP scenario live files: MT1_GAMEFILES unset");return};
    sample(&dir);
    let mut count=0;
    for entry in fs::read_dir(root().join("scenarios")).unwrap() {
        let path=entry.unwrap().path();if path.extension().is_none_or(|e|e!="toml") {continue}
        let loaded=load(&path,&dir).unwrap_or_else(|e|panic!("{e}"));
        assert!(!loaded.proof.is_empty());
        let rec=loaded.record();let source=validate::document(&fs::read_to_string(&path).unwrap()).unwrap();
        if source.get("save").is_some() {
            assert_eq!(loaded.proof[0].0,"save.file");
            assert_eq!(rec,load(&root().join("scenarios/m7_starport_before_save.toml"),&dir).unwrap().record(),"{}",path.display());
            count+=1;continue
        }
        assert_eq!(&rec["party"]["system"],source["ship"].get("system").unwrap_or(&source["place"]["world"]));
        assert_eq!(&rec["party"]["planet"],source["ship"].get("world").unwrap_or(&source["place"]["area"]));
        assert_eq!(rec["t"]["level"],source["place"]["level"]);
        let selected=source["squad"]["selected"].as_i64().unwrap_or(0);
        let cell=array(&source["squad"],"travellers").find(|t|source["squad"]["mode"]=="broken" && t["slot"].as_i64()==Some(selected) && t.get("cell").is_some()).map_or(&source["place"]["cell"],|t|&t["cell"]);
        assert_eq!(rec["t"]["row"].as_i64(),cell[0].as_i64().map(|x|x*2),"{}",path.display());
        assert_eq!(rec["t"]["col"].as_i64(),cell[1].as_i64().map(|x|x*2),"{}",path.display());
        count+=1;
    }
    assert_eq!(count,64);
}

#[test]
fn format_1_5_party_damage_needs_the_format_and_reaches_the_repair_desk() {
    let text=std::fs::read_to_string(root().join("scenarios/h9e_repair_efate.toml")).unwrap();
    assert!(text.contains("format = 1.5") && validate::document(&text).is_ok());
    let old=text.replace("format = 1.5","format = 1.4");
    assert!(validate::document(&old).unwrap_err().contains("party.damage requires 1.5"));
    let Some(dir)=gamefiles() else {return};
    let g=load(&root().join("scenarios/h9e_repair_efate.toml"),&dir).unwrap().game;
    assert_eq!((0..5).map(|i|g.mem.party_u8(0xC2+i)).collect::<Vec<_>>(),[3,0,2,0,1],"C2..C5 sections, C6 hull: what shipyard::repair lists");
}
#[test]
fn format_1_4_squad_ground_grenades_and_starport_area_apply() {
    use mt1::ground::{round, state::Mem};
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let broken=open("efate_broken"); let g=&broken.game; let s=g.squad.as_ref().expect("[squad] installs the squad state");
    assert_ne!(g.mem.combat(),0); assert_eq!((g.mem.leader(),g.mem.selected_char()),(0,0));
    assert_eq!((0..5).map(|i|round::traveller_cell(g,i)).collect::<Vec<_>>(),[(22,5),(22,6),(23,5),(23,6),(22,7)]);
    let e=s.p1.encounter.as_ref().unwrap(); assert_eq!((e.def.as_str(),e.round,e.side,s.encounter,s.round),("efate_quickstart_ambush",1,round::Side::Party,true,1));
    assert!(s.p1.defs.iter().any(|d|d.id=="efate_quickstart_ambush" && d.locked() && !d.allow_save)); assert!(round::locked(g));
    assert!(s.p1.turns.iter().all(|t|!t.done && !t.attacked && t.mp==round::MOVE_WALK));
    assert_eq!(g.mem.party_u8(0x102+0x14+1),8,"Dex 8"); assert_eq!(g.mem.party_u8(0x102+0x2A+10),2,"Combat Rifleman 2");
    assert_eq!(broken.record()["scenario"]["squad"]["encounter"]["id"],"efate_quickstart_ambush");
    assert!(broken.proof.iter().any(|(k,_)|k=="t.walkers[4].x") && broken.proof.iter().any(|(k,v)|k=="squad.encounter" && v["round"]==1));
    let orders=open("efate_orders"); let w=Mem::walker(1);
    assert_eq!((orders.game.mem.read(w.add(22),1),orders.game.mem.read(w.add(5),1)),(14,14),"a 14-step route east and record 14 as the standing target");
    assert!((1..14).all(|k|orders.game.mem.read(w.add(23+k),1)==orders.game.mem.read(w.add(24),1)) && orders.game.mem.read(w.add(23),1)!=orders.game.mem.read(w.add(24),1),"one diagonal step, then 13 steps east");
    assert_eq!((orders.game.mem.x(),orders.game.mem.y()),(44,10),"broken: the party position is the selected walker's (Traveller 1 at (22, 5))");
    let lag=open("efate_melee_lag"); assert_eq!((lag.game.mem.leader(),lag.game.mem.x(),lag.game.mem.y()),(1,44,12),"Traveller 2 selected at (22, 6): set_leader moved the cursor");
    let acted=open("efate_acted"); assert!(acted.game.squad.as_ref().unwrap().p1.turns[0].done);
    let grenade=open("efate_grenade"); assert_eq!(grenade.game.mem.party_u8(0x102+0xC9),2,"explosive grenades at char + 0xC9 (party_fire's record + 0x1CB)");
    let port=open("efate_start_port"); assert_eq!(port.game.mem.outdoors(),0,"DS:6D20 = 0 inside the starport area"); assert!(port.game.squad.is_none(),"no [squad]: the running mode decides");
    assert!(port.extensions.encounter_defs.iter().any(|e|e["id"]=="efate_quickstart_ambush" && e["trigger"]["rect"][0][0]==33));
    let pile=open("efate_pile"); let party=pile.game.mem.party();
    let ground: Vec<(u16,u16,u16)>=(0..128).map(|n|{let at=0x602+9*n;(u16::from_le_bytes([party[at],party[at+1]]),u16::from_le_bytes([party[at+4],party[at+5]]),u16::from_le_bytes([party[at+6],party[at+7]]))}).filter(|&(item,_,_)|item!=0).collect();
    assert!(ground.iter().filter(|&&(_,r,c)|(r,c)==(6,22)).count()>=2,"the revolver and the emeralds lie on (6, 22): {ground:?}");
    assert!(pile.proof.iter().any(|(k,v)|k.starts_with("party.objects[") && k.ends_with(".id") && *v==61));
    let juke=open("efate_juke"); let s=juke.game.squad.as_ref().unwrap();
    assert_eq!((s.pools[&14],s.p1.npcs[&14].engaged,s.p1.encounter.as_ref().unwrap().round),(3,true,2)); assert!(s.p1.def("juke").is_some());
    let first=open("efate_npc_first"); assert_eq!(round::npc_fighter(&first.game,14).tactics,2); assert_eq!(round::pools(&first.game),(0,6));
    let unmapped=open("efate_unmapped"); assert_eq!(round::npc_fighter(&unmapped.game,14).source,mt1::ground::rules::adapters::Source::Derived);
    let alerted=open("efate_alerted"); let s=alerted.game.squad.as_ref().unwrap(); assert!(s.p1.encounter.is_none()); assert_eq!(s.p1.npcs[&14].alerted,Some(10)); assert_eq!(round::alerted_count(&alerted.game),1);
    let save=open("m8c_save_encounter"); assert!(save.game.squad.as_ref().unwrap().p1.defs.iter().any(|d|d.id=="efate_quickstart_ambush" && d.allow_save));
    let pack=open("efate_pack"); let inv=|c:u16|pack.game.mem.party_u16(0x102+0xCC+2*c); let id=|w:u16|pack.game.mem.party_u8(0xA82+5*(w-1));
    assert!(inv(1)==0 && inv(5)==0 && inv(4)!=0 && inv(6)!=0 && id(inv(4))==61 && id(inv(6))==4 && id(inv(0))==45,"pack cells by name: med kit 0, emeralds 4, pistol 6, the rest empty");
    let text=fs::read_to_string(root().join("scenarios/efate_broken.toml")).unwrap().replace("\r\n","\n");
    for (from,to,expect) in [("format = 1.4","format = 1.3","requires 1.4"),("mode = \"broken\"","mode = \"grouped\"","cells need mode = broken"),("id = \"efate_quickstart_ambush\"\nround","id = \"nowhere\"\nround","squad/encounter"),
        ("cell = [22, 6]","cell = [22, 5]","two Travellers on one cell"),("[[squad.travellers]]\nslot = 4","[[squad.npc]]\nrecord = 14\nrole = \"ninja\"\n\n[[squad.travellers]]\nslot = 4","unknown role"),
        ("round = 1","round = 2\nsurprised = \"party\"","surprise belongs to round 1")] {
        let err=prepare(&text.replace(from,to),Path::new("fault"),&dir).err().unwrap_or_else(||panic!("{from} -> {to} accepted")); assert!(err.contains(expect),"{from}: {err}");
    }
}

#[test]
fn save_scenario_restores_exactly_and_rejects_hostile_files() {
    use ground::Tooling;
    let Some(dir)=gamefiles() else {return};
    let file=sample(&dir);let bytes=fs::read(&file).unwrap();
    let mut after=load(&root().join("scenarios/m7_after_restore.toml"),&dir).unwrap();
    let saved=mt1::save::Saved::decode(&bytes).unwrap();
    assert_eq!(saved.mode,mt1::save::Mode::Ground);assert_eq!(after.game.mem.ds,saved.ds);assert_eq!(after.game.rand.lcg.seed,saved.seed);
    assert_eq!(after.game.restore_pending.as_ref(),Some(&saved));
    assert!(after.extensions.pending_space.is_empty() && !after.extensions.encounter_defs.is_empty());
    assert!(mt1::save::resume(&mut after.game));assert!(after.game.restore_pending.is_none());
    let tmp=root().join(format!("target/scenario-save-{}",std::process::id()));fs::create_dir_all(&tmp).unwrap();
    let toml=|file:&str|format!("[meta]\nformat = 1.3\nname = \"s\"\nversion = \"1\"\nauthor = \"a\"\npurpose = \"test\"\n[save]\nfile = \"{file}\"\n");
    fs::write(tmp.join("junk.mtsav"),b"junk").unwrap();
    let mut flip=bytes.clone();flip[5000]^=1;fs::write(tmp.join("flip.mtsav"),&flip).unwrap();
    let mut short=bytes.clone();short.truncate(bytes.len()/2);fs::write(tmp.join("short.mtsav"),&short).unwrap();
    let mut newer=bytes.clone();newer[8..10].copy_from_slice(&(mt1::save::VERSION+1).to_le_bytes());fs::write(tmp.join("newer.mtsav"),&newer).unwrap();
    for (name,text,expect) in [
        ("missing",toml("nothere.mtsav"),"; make it first: load 'Starport before save' and save (or run the --scenario-check --save command)"),("junk",toml("junk.mtsav"),"truncated"),("flip",toml("flip.mtsav"),"checksum"),
        ("short",toml("short.mtsav"),"truncated"),("newer",toml("newer.mtsav"),"newer"),
        ("with-place",format!("{}[place]\nworld = 2\narea = 0\nlevel = 0\ncell = [1, 1]\n",toml("junk.mtsav")),"a [save] scenario"),
        ("old-format",toml("junk.mtsav").replace("1.3","1.2"),"meta/format"),
    ] {
        let path=tmp.join(format!("{name}.toml"));fs::write(&path,text).unwrap();
        let err=load(&path,&dir).err().unwrap();assert!(err.contains(expect),"{name}: {err}");
    }
    let mut live=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();let before=live.record();
    let request=Request::default();*request.borrow_mut()=Some(tmp.join("flip.toml"));
    let mut loader=Loader::new(request,dir.clone(),Extensions::default());
    assert!(!loader.poll(&mut live.game));assert_eq!(live.record(),before);
    *loader.request.borrow_mut()=Some(root().join("scenarios/m7_after_restore.toml"));
    assert!(loader.poll(&mut live.game));assert_eq!(live.game.mem.ds,saved.ds);assert!(live.game.restore_pending.is_some());
    assert!(mt1::save::resume(&mut live.game));
    fs::remove_dir_all(tmp).unwrap();
}

#[test]
fn space_block_bytes_read_back_and_pending_targets_are_not_s_state() {
    let Some(dir)=gamefiles() else {return};
    let text=include_str!("../../../../scenarios/m6_encounter_one_hostile.toml");
    let text=text.replace("sections = [0, 0, 0, 0]","sections = [1, 99, 100, 255]")
        .replace("hull = 0","hull = 42").replace("jump_drive = 1","jump_drive = 2");
    let prepared=prepare(&text,Path::new("space-block"),&dir).unwrap();
    let r=prepared.record();
    assert_eq!(r["party"]["system"],2);assert_eq!(r["party"]["planet"],255);
    assert_eq!(r["party"]["ship_damage"],json!([1,99,100,255]));assert_eq!(r["party"]["hull_damage"],42);
    assert_eq!(prepared.game.mem.party()[0xc2..0xc7],[1,99,100,255,42]);
    assert_eq!(r["party"]["fuel"],124);assert_eq!(prepared.game.mem.party_u8(0xc1)&3,2);
    assert!(prepared.require_playable().is_ok());
    assert!(r.get("s").is_none());assert!(r["rng"]["S"].is_null());assert_eq!(r["prog"],"T");
    assert_eq!(r["scenario"]["pending_srand"]["S"],864);
    let pending=&prepared.extensions.pending_space;
    assert_eq!(pending["s.ship_count"],1);assert_eq!(pending["s.ships"].as_array().unwrap().len(),18);
    assert_eq!(pending["s.ships"][0]["active"],102);assert_eq!(pending["s.ships"][17]["active"],-1);
    assert_eq!(pending["space_encounter_group"]["count"],1);
    assert!(prepared.proof.iter().all(|(k,_)|!k.starts_with("s.") && !k.starts_with("space_")));

    let mut game=prepared.game;let mut p=Pokes::new();let mut e=Extensions::default();
    p.set(&mut game,"party.ship_flags",0x7d).unwrap();
    space::stage(&json!({"ship":{"jump_drive":2}}),&mut game,&mut p,&mut e).unwrap();
    assert_eq!(game.mem.party_u8(0xc1),0x7e);assert!(p.proof(&game).is_ok());
}

#[test]
fn encounter_object_launches_one_live_hostile_and_clears_unused_records() {
    use mt_engine::{io::{Dir, Scripted}, sim::tick::Counter};
    let Some(dir)=gamefiles() else {return};
    let loaded=load(&root().join("scenarios/m6_encounter_one_hostile.toml"),&dir).unwrap();
    let mut sp=mt1::space::Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(),loaded.game.mem.party().to_vec(),vec![0;0x2000],
        Box::new(Scripted::new([])),Box::new(Counter::at(1234)),Box::new(Dir(dir))).unwrap();
    for i in 0..4 {for off in (0..30).step_by(2) {sp.s.set_enemy(i,off,99);}}
    space::apply_pending(&mut sp,&loaded.extensions.pending_space).unwrap();
    assert_eq!(sp.s.enemy(0,0),170); assert_eq!(sp.s.enemy(0,14),11); assert_eq!(sp.s.enemy(0,16),300);
    for i in 1..4 {assert_eq!(sp.s.enemy(i,0),0);assert_eq!(sp.s.enemy(i,16),0);}
    assert_eq!(sp.s.word(mt1::space::state::ENCOUNTER),1);
}

#[test]
fn space_faults_reject_before_apply_and_menu_loads_valid_space() {
    use ground::Tooling;
    let Some(dir)=gamefiles() else {return};
    let good=include_str!("../../../../scenarios/m6_encounter_one_hostile.toml");
    for (from,to,field) in [
        ("format = 1.2","format = 1.1","meta/format"),
        ("system = 2","system = 8","ship/system"),
        ("world = 255","world = 254","ship/world"),
        ("world = 255","world = 5\nsystem_typo = 1","ship/system_typo"),
        ("orbit = -1","orbit = 79","ship/orbit"),
        ("heading = 0","heading = 8","ship/heading"),
        ("position = [170, 148]","position = [32768, 148]","ship/position"),
        ("velocity = [0, 0]","velocity = [0, -32769]","ship/velocity"),
        ("fuel = 124","fuel = 125","party/fuel"),
        ("jump_drive = 1","jump_drive = 3","ship/jump_drive"),
        ("sections = [0, 0, 0, 0]","sections = [0, 0, 0]","ship/damage/sections"),
        ("hull = 0","hull = 256","ship/damage/hull"),
        ("S = 864","S = 65536","seeds/S"),
        ("direction = 2","direction = true","ship/ships/0/direction"),
        ("encounter = 1","encounter = 2","ship/encounter"),
        ("station = \"pilot\"","station = 16","ship/station"),
        ("screen = \"station\"","screen = \"flight\"","ship/station"),
    ] {
        let err=prepare(&good.replace(from,to),Path::new("negative-space"),&dir).err().unwrap();
        assert!(err.contains(field),"{field}: {err}");
    }
    let mut live=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    let before=live.record();let request=Request::default();
    let invalid=root().join(format!("target/scenario-invalid-space-{}.toml",std::process::id()));
    fs::write(&invalid,good.replace("system = 2","system = 8")).unwrap();
    *request.borrow_mut()=Some(invalid.clone());
    let mut loader=Loader::new(request,dir,Extensions::default());
    assert!(!loader.poll(&mut live.game));assert_eq!(live.record(),before);
    fs::remove_file(invalid).unwrap();
    *loader.request.borrow_mut()=Some(root().join("scenarios/m6_landing_louzy.toml"));
    assert!(loader.poll(&mut live.game));
    assert_eq!(live.game.mem.party_u8(6),3);
    assert_eq!(loader.extensions.pending_space["s.current_object"],5);
    assert_eq!(loader.extensions.pending_space["space_entry"],"flight");
}

#[test]
fn space_table_limit_and_format_compatibility() {
    let good=include_str!("../../../../scenarios/m6_encounter_one_hostile.toml");
    let base=validate::document(good).unwrap();
    let check=|v:&Value|validate::document(&toml::to_string(v).unwrap());
    let mut v=base.clone();let marker=v["ship"]["ships"][0].clone();
    v["ship"]["ships"]=json!(vec![marker.clone();18]);v["ship"]["encounter"]=json!(18);
    assert!(check(&v).is_ok());
    v["ship"]["ships"]=json!(vec![marker;19]);assert!(check(&v).unwrap_err().contains("ship/ships"));
    let juke=include_str!("../../../../scenarios/m4c_efate_juke.toml");
    assert!(validate::document(&juke.replace("format = 1.1","format = 1.2")).is_ok());
    let old=include_str!("../../../../scenarios/m4b_medkit_in_pack.toml");
    assert!(validate::document(&format!("{old}\n[ship]\n")).is_ok());
}
#[test]
fn symbolic_space_fields_encode_oracle_targets_and_clear_unused_slots() {
    let Some(dir)=gamefiles() else {return};
    let mut loaded=load(&root().join("scenarios/m6_jumpscreen_louzy.toml"),&dir).unwrap();
    assert_eq!(loaded.game.mem.party_u8(7),0);
    assert_eq!(loaded.game.mem.party_u8(0xc1)&3,1);
    assert_eq!(loaded.game.mem.party()[0xac..0xc0],[163,164,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0]);
    let mut v=validate::document(include_str!("../../../../scenarios/m6_encounter_one_hostile.toml")).unwrap();
    let schema:Value=serde_json::from_str(include_str!("../../../../schema/scenario.schema.json")).unwrap();
    let check=|v:&Value|validate::check(&schema,&schema,v,"").and_then(|_|space::validate(v));
    let mut p=Pokes::new();let mut e=Extensions::default();
    for n in 0..8 {
        v["ship"]["heading"]=json!(n);v["ship"]["ships"][0]["direction"]=json!(n);
        check(&v).unwrap();space::stage(&v,&mut loaded.game,&mut p,&mut e).unwrap();
        assert_eq!(e.pending_space["s.heading"],90+n);
        assert_eq!(e.pending_space["s.direction"],1+n);
        assert_eq!(e.pending_space["s.view_heading"],n);
        assert_eq!(e.pending_space["s.ships"][0]["active"],100+n);
    }
    v["ship"].as_object_mut().unwrap().remove("encounter");
    v["ship"].as_object_mut().unwrap().remove("encounter_group");
    v["ship"]["ships"][0]["direction"]=json!(-1);
    check(&v).unwrap();space::stage(&v,&mut loaded.game,&mut p,&mut e).unwrap();
    assert_eq!(e.pending_space["s.ships"][0]["active"],-1);
    for (name,id) in [("pilot",10),("gunner_1",11),("gunner_2",12),("computer",13),("engineering",14),("comms",15),("crew",17)] {
        v["ship"]["station"]=json!(name);check(&v).unwrap();
        space::stage(&v,&mut loaded.game,&mut p,&mut e).unwrap();
        assert_eq!(e.pending_space["s.station_id"],id);assert_eq!(e.pending_space["s.station_active"],1);
    }
    let mut programs=Vec::new();
    for id in [0,15] {for loaded in [false,true] {for running in [false,true] {programs.push(json!({"id":id,"loaded":loaded,"running":running}));}}}
    v["ship"]["programs"]=json!(programs);check(&v).unwrap();
    space::stage(&v,&mut loaded.game,&mut p,&mut e).unwrap();
    assert_eq!(loaded.game.mem.party()[0xac..0xb4],[0,32,128,160,15,47,143,175]);
    assert_eq!(loaded.game.mem.party()[0xb4..0xc0],[0;12]);
    v["ship"]["programs"]=json!([]);v["ship"]["ships"]=json!([]);check(&v).unwrap();
    space::stage(&v,&mut loaded.game,&mut p,&mut e).unwrap();
    assert_eq!(loaded.game.mem.party()[0xac..0xc0],[0;20]);
    assert_eq!(e.pending_space["s.ship_count"],0);
    assert!(e.pending_space["s.ships"].as_array().unwrap().iter().all(|r|r==&json!({"x":0,"y":0,"active":-1,"dstx":0,"dsty":0})));
    assert!(p.proof(&loaded.game).is_ok());
}

#[test]
fn medkit_police_vehicle_and_spawn_have_live_owners() {
    let Some(dir)=gamefiles() else {return};
    let med=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    let r=med.record();assert_eq!(r["party"]["chars"][0]["hits"],8);
    let inv=r["party"]["chars"][0]["inventory"].as_array().unwrap();
    assert!(inv.iter().filter_map(Value::as_u64).filter(|&n|n>0 && n<=256).any(|n|r["party"]["objects"][n as usize-1]["id"]==45));
    let police=load(&root().join("scenarios/m5_police_provoked.toml"),&dir).unwrap().record();
    assert_eq!(police["party"]["system"],0);assert_eq!(police["t"]["world_law"],12);assert_eq!(police["t"]["offences"],2);
    let vehicle=load(&root().join("scenarios/m4b_vehicle_efate_l4.toml"),&dir).unwrap().record();
    assert_eq!(vehicle["t"]["level"],4);assert_eq!(vehicle["t"]["vehicle"],1);assert_eq!(vehicle["t"]["vehicle_condition"],100);
    let juke=load(&root().join("scenarios/m4c_efate_juke.toml"),&dir).unwrap().record();
    let thug=&juke["t"]["monsters"][16];
    assert_eq!(thug["level"],0);assert_eq!(thug["x"],68);assert_eq!(thug["y"],68);assert_eq!(thug["weapon"],4);assert_eq!(thug["attitude"].as_u64().unwrap()&3,0);
    assert_eq!(juke["scenario"]["npc_sheets"]["17"]["role"],"thug");
    let sheet=&juke["scenario"]["npc_sheets"]["17"]["sheet"];
    assert_eq!(sheet,&Assets::load(&dir).unwrap().roles["role"]["thug"]);
}
#[test]
fn malformed_references_do_not_touch_running_game() {
    let Some(dir)=gamefiles() else {return};
    let path=root().join("scenarios/m4b_medkit_in_pack.toml");
    let good=fs::read_to_string(&path).unwrap();
    let current=load(&path,&dir).unwrap();let before=current.record();
    for text in [good.replace("[meta]","[meta]\ncolour = 'red'"),good.replace("id = 45","id = 250"),
        good.replace("cell = [32, 9]","cell = [999, 9]"),good.replace("world = 2","world = 1\narea_typo = 0"),
        good.replace("hits = 8","hits = 255")] {
        assert!(prepare(&text,&path,&dir).is_err());assert_eq!(current.record(),before);
    }
    let escape=fs::read_to_string(root().join("scenarios/m4c_efate_escape_zone.toml")).unwrap();
    let mut doc:toml::Value=toml::from_str(&escape).unwrap();
    doc["encounter"][0]["participants"]=toml::Value::Array(vec!["nobody".into()]);
    let err=prepare(&toml::to_string(&doc).unwrap(),&path,&dir).err().unwrap();
    assert!(err.contains("encounter road_block") && err.contains("participant"),"{err}");
}
#[test]
fn late_failure_and_success_use_the_same_transaction_boundary() {
    use ground::Tooling;
    let Some(dir)=gamefiles() else {return};
    let mut live=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    live.game.balance.grenade_range=false; live.game.balance.npc_selective_autofire=false;
    let balance=live.game.balance;
    let before=live.record();
    let request=Request::default();
    *request.borrow_mut()=Some(root().join("scenarios/does-not-exist.toml"));
    let mut loader=Loader::new(request.clone(),dir.clone(),Extensions::default());
    assert!(!loader.poll(&mut live.game));assert_eq!(live.record(),before);
    *request.borrow_mut()=Some(root().join("scenarios/m5_police_provoked.toml"));
    assert!(loader.poll(&mut live.game));assert_eq!(live.game.mem.party_u8(6),0);
    assert_eq!(live.game.balance,balance,"scenario replacement retains the running rule preferences");
}

#[test]
fn optional_scalar_fields_and_pending_seeds_have_readback() {
    let Some(dir)=gamefiles() else {return};
    let text=include_str!("../../../../scenarios/m4b_medkit_in_pack.toml");
    let mut v:toml::Value=toml::from_str(text).unwrap();
    put(&mut v["party"],"credits",u32::MAX.into());put(&mut v["party"],"fuel",124.into());
    put(&mut v["party"],"cargo",toml::Value::Array(vec![4.into()]));
    put(&mut v["party"]["members"][0],"oxygen",255.into());
    put(&mut v["party"]["members"][0],"skills",toml::from_str::<toml::Value>("70 = 15").unwrap());
    put(&mut v,"time",toml::from_str::<toml::Value>("date = 65535").unwrap());
    put(&mut v,"seeds",toml::from_str::<toml::Value>("T = 65535\nS = 7\nCG = 8\nSTARTUP = 9").unwrap());
    put(&mut v,"starport",toml::from_str::<toml::Value>("tech = 15\nlaw = 0\noffices = {casino = true, general = false}").unwrap());
    let prepared=prepare(&toml::to_string(&v).unwrap(),Path::new("scalar-test"),&dir).unwrap();
    let r=prepared.record();
    assert_eq!(r["party"]["credits"],u32::MAX);assert_eq!(r["party"]["clock"],65535);
    assert_eq!(r["party"]["cargo"][0],4);assert_eq!(r["party"]["cargo"][19],0);
    assert_eq!(r["party"]["chars"][0]["skills"][70],15);assert_eq!(r["party"]["chars"][0]["oxygen"],255);
    assert_eq!(r["rng"]["T"],65535);assert_eq!(r["scenario"]["pending_srand"],json!({"S":7,"CG":8,"STARTUP":9}));
    assert_eq!(r["t"]["world_tech"],15);assert_eq!(r["t"]["world_law"],0);
    assert_eq!(r["t"]["office_open"][12],1);assert_eq!(r["t"]["office_open"][18],0);
}

#[test]
fn npc_remove_clears_cached_occupancy_and_sets_kill_not_reward() {
    let Some(dir)=gamefiles() else {return};
    let mut v:toml::Value=toml::from_str(include_str!("../../../../scenarios/m4b_vehicle_efate_l4.toml")).unwrap();
    put(&mut v["npcs"],"set",toml::Value::Array(vec![toml::from_str("record = 14\npresent = false").unwrap()]));
    let prepared=prepare(&toml::to_string(&v).unwrap(),Path::new("remove-test"),&dir).unwrap();
    let g=&prepared.game;let n=Mem::npc(14);let id=g.mem.read(n,2) as u16;
    assert_eq!(g.mem.read(n.add(2),1),20);
    assert_ne!(g.mem.party_u8(8+id/8)&(1<<(id%8)),0);
    assert_eq!(g.mem.party_u8(0x3a+id/8)&(1<<(id%8)),0);
    let width=g.mem.u8(0x6097) as usize*20;let height=g.mem.u8(0x6096) as usize*12;
    let map=g.mem.pointer(0x606e);
    assert!((0..width*height).all(|i|g.mem.read(map.add((i*3+2) as i32),1)&63!=14));
}

#[test]
fn npc_move_swaps_cells_without_stale_occupants() {
    let Some(dir)=gamefiles() else {return};
    let base=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    let read=|k:i16| {let n=Mem::npc(k);[base.game.mem.read(n.add(32),2)/2,base.game.mem.read(n.add(34),2)/2]};
    let a=read(14);let b=read(15);
    let mut v:toml::Value=toml::from_str(include_str!("../../../../scenarios/m4b_medkit_in_pack.toml")).unwrap();
    put(&mut v["npcs"],"set",toml::Value::Array(vec![
        toml::from_str(&format!("record = 14\ncell = [{},{}]",b[0],b[1])).unwrap(),
        toml::from_str(&format!("record = 15\ncell = [{},{}]",a[0],a[1])).unwrap()]));
    let loaded=prepare(&toml::to_string(&v).unwrap(),Path::new("swap-test"),&dir).unwrap();
    let p=Pokes::new();
    for (k,rc) in [(14,b),(15,a)] {
        assert_eq!(p.read(&loaded.game,&monster(k,"x")).unwrap(),rc[0]*2);
        assert_eq!(p.read(&loaded.game,&monster(k,"y")).unwrap(),rc[1]*2);
        let at=occupancy(&loaded.game,rc[0] as i16,rc[1] as i16).unwrap();
        assert_eq!(p.read(&loaded.game,&at).unwrap()&63,k as u32);
    }
}

#[test]
fn readback_detects_corruption_and_late_allocation_errors_are_transactional() {
    let Some(dir)=gamefiles() else {return};
    let mut live=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    let mut p=Pokes::new();p.set(&mut live.game,"party.credits",123).unwrap();
    live.game.mem.set_party_u8(0x88,124);
    assert!(p.proof(&live.game).unwrap_err().contains("party.credits"));
    let before=live.record();
    let mut v:toml::Value=toml::from_str(include_str!("../../../../scenarios/m4b_medkit_in_pack.toml")).unwrap();
    put(&mut v["party"]["items"][0],"armor",toml::from_str("id = 31").unwrap());
    let err=prepare(&toml::to_string(&v).unwrap(),Path::new("full-slot-test"),&dir).err().unwrap();
    assert!(err.contains("no free inventory cell"),"{err}");assert_eq!(live.record(),before);
}

#[test]
fn encounters_override_game_definitions_and_drop_on_level_change() {
    use ground::Tooling;
    let Some(dir)=gamefiles() else {return};
    let mut v:toml::Value=toml::from_str(include_str!("../../../../scenarios/m4c_efate_ambush.toml")).unwrap();
    put(&mut v,"encounter",toml::Value::Array(vec![toml::from_str("id = 'efate_quickstart_ambush'\nstart = 'sight'\nend = 'open'").unwrap()]));
    let mut prepared=prepare(&toml::to_string(&v).unwrap(),Path::new("override-test"),&dir).unwrap();
    assert_eq!(prepared.extensions.encounter_defs.len(),2);
    assert!(prepared.extensions.encounter_defs.iter().any(|e|e["id"]=="efate_quickstart_ambush" && e["end"]=="open"));
    assert!(prepared.extensions.encounter_defs.iter().any(|e|e["id"]=="assassin_szen_sanai" && e["end"]=="locked"));
    let mut loader=Loader::new(Request::default(),dir,prepared.extensions);
    assert!(!loader.poll(&mut prepared.game));
    assert!(walk::event_at(&mut prepared.game,2,0,4));
    assert!(!loader.poll(&mut prepared.game));assert!(loader.extensions.encounter_defs.is_empty());
    assert!(walk::event_at(&mut prepared.game,2,0,0));
    assert!(!loader.poll(&mut prepared.game));assert_eq!(loader.extensions.encounter_defs.len(),2);
    assert!(loader.extensions.encounter_defs.iter().all(|e|e["end"]=="locked"));
}

#[test]
fn new_targets_use_the_lockstep_sidecar_writer_and_record_reader() {
    use crate::lockstep::{schema,symbolic};
    let Some(dir)=gamefiles() else {return};
    let mut loaded=load(&root().join("scenarios/m4b_medkit_in_pack.toml"),&dir).unwrap();
    let fields=symbolic::catalog();
    let mut record=loaded.record();
    for (target,value) in [("t.world_tech",7),("t.monsters[16].hits",33),("t.office_open[12]",0),
        ("t.maps[0].cells[200].attr",14)] {
        symbolic::write(&mut loaded.game,&fields,&format!("{target}+0"),value).unwrap();
        if target=="t.world_tech" {record["t"].as_object_mut().unwrap().shift_remove("world_tech");}
        if target=="t.monsters[16].hits" {record["t"]["monsters"][16].as_object_mut().unwrap().shift_remove("hits");}
        symbolic::emit_requested(&mut record["t"],&loaded.game,&fields,target);
        assert_eq!(schema::byte(&record,target),Some(value));
    }
}
#[test]
fn orders_on_the_efate_scenarios() {
    use mt1::ground::{orders, round, walk, state::Mem};
    use mt1::ui::{Intent, Target, Unit, Verb};
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let ammo=|g:&Ground,who:i32|(g.mem.read(walk::item_ptr(g,who,21).add(1),1),g.mem.read(walk::item_ptr(g,who,21).add(2),1));
    let hover=|g:&mut Ground,t:Target|{orders::hover(g,Some(t));g.squad.as_ref().unwrap().orders.hover.clone()};
    let mut b=open("efate_broken"); let g=&mut b.game;
    assert_eq!(hover(g,Target::Unit(Unit::Npc(14))),"Hostile (thug, NPC 14): Short: Routine 7+, DM +3 (skill 2, Dex +1), needs 4+ (91%); Dennar in the line!","P20 R-16: Traveller 4 at (23, 6) stands on the line to record 14 at short range: the hover warns");
    let m=orders::menu_for(g,Target::Unit(Unit::Npc(14)));
    assert_eq!((m.items[m.default].label.as_str(),m.items.iter().find(|i|i.label=="Full auto").unwrap().grey.as_deref(),m.items.iter().find(|i|i.label=="Target").unwrap().grey.as_deref()),("Attack",Some("switch to full auto first (Q)"),None));
    assert_eq!(ammo(g,0),(3,20));
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    let s=g.squad.as_ref().unwrap(); assert!(s.p1.log.iter().any(|l|l.contains(" fires the 9mm advanced combat rifle at thug (NPC 14): short: 2D ")),"{:?}",s.p1.log);
    assert_eq!(ammo(g,0),(3,19)); assert!(s.p1.turns[0].attacked && !s.p1.turns[0].done); assert_eq!(g.mem.read(Mem::walker(0).add(5),1),14,"f5 = the standing target"); assert_eq!(g.mem.party_fired(),1);
    assert_eq!(hover(g,Target::Unit(Unit::Npc(14))),"Hostile (thug, NPC 14): no Attack left");
    orders::intent(g,Intent::Select(2)); assert_eq!(g.mem.leader(),2); assert_eq!(hover(g,Target::Unit(Unit::Traveller(2))),round::name(g,2));
    let from=round::traveller_cell(g,2);
    let to=(0..40i16).flat_map(|r|(0..60i16).map(move|c|(r,c))).find(|&c|orders::find_path(g,2,from,c).is_some_and(|p|p.len()>=11&&p.len()<=20)).expect("a cell a run away");
    let text=hover(g,Target::Cell(to)); assert!(text.starts_with("Run ")&&text.ends_with("squares: -2 to be hit, -2 to your shots"),"{text}");
    let n=g.squad.as_ref().unwrap().orders.path.len() as i32;
    let path=g.squad.as_ref().unwrap().orders.path.clone();
    orders::intent(g,Intent::Verb{verb:Verb::Move,target:Target::Cell(to)});
    let t=g.squad.as_ref().unwrap().p1.turns[2];
    if round::traveller_cell(g,2)!=to { let at=round::traveller_cell(g,2); let next=path.iter().map(|p|p.0).find(|&c|c!=at&&path.iter().position(|p|p.0==c)>path.iter().position(|p|p.0==at)).unwrap(); let cell=mt1::ground::commands::cell(g,next.0,next.1);
        panic!("stopped at {at:?} of {path:?}: event {} next {next:?} bytes {:?} view {:?} try_move {} log {:?}",g.mem.event(),(g.mem.read(cell,1),g.mem.read(cell.add(1),1),g.mem.read(cell.add(2),1)),(g.mem.view_x(),g.mem.view_y()),walk::try_move(g,next.0,next.1),g.squad.as_ref().unwrap().p1.log); }
    assert_eq!((round::traveller_cell(g,2),t.speed,t.moved,t.mp),(to,2,true,20-n));
    assert_eq!(hover(g,Target::Cell((0,0))),"no path"); assert_eq!(g.squad.as_ref().unwrap().orders.path,vec![((0,0),orders::PATH_BLOCKED)]);
    let mut d=open("efate_dry"); let g=&mut d.game;
    assert_eq!(ammo(g,0),(1,0)); let m=orders::menu_for(g,Target::Unit(Unit::Npc(14))); assert_eq!((m.items[m.default].label.as_str(),m.items[0].grey.as_deref()),("Reload",Some("chamber empty")));
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    assert_eq!(ammo(g,0),(0,20)); let s=g.squad.as_ref().unwrap(); assert!(s.p1.turns[0].attacked); assert!(s.p1.log.iter().any(|l|l.contains(" reloads (20 rounds, 0 magazines left)")),"{:?}",s.p1.log);
    let mut w=open("efate_wall"); let g=&mut w.game;
    assert_eq!(hover(g,Target::Unit(Unit::Npc(14))),"Hostile (thug, NPC 14): no line of sight");
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    assert_eq!(ammo(g,0),(3,20)); assert!(!g.squad.as_ref().unwrap().p1.turns[0].attacked);
    let mut l=open("efate_melee_lag"); let g=&mut l.game; assert_eq!(g.mem.leader(),1);
    let from=round::traveller_cell(g,1); let step=[(from.0-1,from.1),(from.0,from.1-1),(from.0-1,from.1-1),(from.0+1,from.1+1),(from.0-1,from.1+1)].into_iter().find(|&c|orders::find_path(g,1,from,c).is_some()).unwrap();
    orders::intent(g,Intent::Verb{verb:Verb::Move,target:Target::Cell(step)}); assert_eq!(round::traveller_cell(g,1),step);
    assert_eq!(orders::fire_check(g,1,14).err().as_deref(),Some("moved: high recoil"));
    let mut o=open("efate_orders"); let g=&mut o.game;
    orders::intent(g,Intent::Select(1)); assert_eq!((g.mem.leader(),g.mem.read(Mem::walker(1).add(22),1),g.mem.read(Mem::walker(1).add(5),1)),(1,14,14));
    let mut s=open("efate_shop"); let g=&mut s.game; if g.squad.is_none() { g.squad=Some(Default::default()) }
    let m=orders::menu_for(g,Target::Unit(Unit::Npc(17))); assert_eq!(m.items[m.default].label,"Converse");
    orders::intent(g,Intent::Verb{verb:Verb::Converse,target:Target::Unit(Unit::Npc(17))}); assert_eq!(g.mem.key(),0x2E);
    let mut a=open("efate_acted"); let g=&mut a.game;
    assert_eq!(hover(g,Target::Unit(Unit::Npc(14))),"Hostile (thug, NPC 14): has acted"); assert_eq!(hover(g,Target::Unit(Unit::Traveller(0))),format!("{}: has acted",round::name(g,0)));
}
#[test]
fn fire_on_the_efate_scenarios() {
    use mt1::ground::{fire, orders, round, walk, state::Mem};
    use mt1::ui::{Intent, Target, Unit, Verb};
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let ammo=|g:&Ground,who:i32|g.mem.read(walk::item_ptr(g,who,21).add(2),1);
    let hover=|g:&mut Ground,t:Target|{orders::hover(g,Some(t));g.squad.as_ref().unwrap().orders.hover.clone()};
    let logged=|g:&Ground,s:&str|g.squad.as_ref().unwrap().p1.log.iter().filter(|l|l.contains(s)).count();
    let mut a=open("efate_auto"); let g=&mut a.game;
    let m=orders::menu_for(g,Target::Unit(Unit::Npc(14))); assert_eq!(m.items.iter().find(|i|i.label=="Full auto").unwrap().grey.as_deref(),Some("switch to full auto first (Q)"));
    g.squad.as_mut().unwrap().orders.full_auto[0]=true; assert_eq!(ammo(g,0),30);
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    assert_eq!((logged(g," fires the 9mm submachinegun at thug (NPC 14): short: 2D "),logged(g,"(burst, 10 rounds)"),ammo(g,0)),(1,1,20),"{:?}",g.squad.as_ref().unwrap().p1.log);
    assert!(logged(g,"burst on thug (NPC 15)")>=1||!round::npc_present(g,15),"a bonus roll on the adjacent record 15; {:?}",g.squad.as_ref().unwrap().p1.log);
    let mut gr=open("efate_grenade"); let g=&mut gr.game;
    assert_eq!(g.mem.party_u8(0x1CB),2,"two explosive grenades (char + 0xC9)");
    g.mem.write(Mem::walker(0).add(3),1,1);
    g.balance.grenade_range=false;
    let text=hover(g,Target::Cell((24,8))); assert!(text.starts_with("explosive grenade: Medium: Formidable 15+"),"OFF restores the printed row: {text}");
    g.balance.grenade_range=true;
    let text=hover(g,Target::Cell((24,8))); assert!(text.starts_with("explosive grenade: Short: Difficult 11+")&&text.ends_with("goes off at the end of the turn"),"{text}");
    assert_eq!(g.squad.as_ref().unwrap().orders.path.len(),8,"the danger ring");
    let m=orders::menu_for(g,Target::Cell((24,8))); assert_eq!((m.items[m.default].label.as_str(),m.items[m.default].grey.as_deref()),("Throw grenade here",None));
    orders::intent(g,Intent::Verb{verb:Verb::Move,target:Target::Cell((24,8))});
    assert_eq!((g.mem.party_u8(0x1CB),logged(g," throws an explosive grenade at (24, 8): short: 2D "),logged(g,"goes off"),g.squad.as_ref().unwrap().fire.pending.len()),(1,1,0,1),"{:?}",g.squad.as_ref().unwrap().p1.log);
    orders::intent(g,Intent::EndTurn);
    assert_eq!(logged(g,"explosive grenade goes off at"),1,"{:?}",g.squad.as_ref().unwrap().p1.log);
    assert!(logged(g,"blast (1 sq, Pen 4) on thug")>=1,"a roll per NPC within radius 1 at half Pen; {:?}",g.squad.as_ref().unwrap().p1.log);
    let mut l=open("efate_gl"); let g=&mut l.game;
    let text=hover(g,Target::Unit(Unit::Npc(14))); assert!(text.contains("Difficult 11+")&&text.contains("Forward Observer")&&text.ends_with("lands at the start of your next turn"),"{text}");
    assert_eq!(ammo(g,0),3);
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    assert_eq!((ammo(g,0),logged(g," fires the 4cm RAM GL-10 at ("),logged(g,"a marker on (")),(2,1,1),"{:?}",g.squad.as_ref().unwrap().p1.log);
    assert_eq!(mt1::ui::scene::Squad::capture(g).markers.len(),1);
    for _ in 0..12 { if g.squad.as_ref().unwrap().p1.encounter.as_ref().is_some_and(|e|e.round>=2) {break} orders::intent(g,Intent::EndTurn) }
    orders::intent(g,Intent::Select(0));
    assert_eq!((logged(g,"4cm RAM GL-10 lands at ("),g.squad.as_ref().unwrap().fire.pending.len()),(1,0),"{:?}",g.squad.as_ref().unwrap().p1.log);
    assert!(logged(g,"blast (")>=1,"the radius-9 rolls on whoever stands inside; {:?}",g.squad.as_ref().unwrap().p1.log);
    let mut i=open("efate_broken_interior"); let g=&mut i.game;
    g.mem.set_party_u8(0xA82,17);
    assert_eq!(fire::launch_check(g,0,round::npc_cell(g,14)).err().as_deref(),Some("not indoors"));
    let mut c=open("efate_cover"); let g=&mut c.game;
    let text=hover(g,Target::Unit(Unit::Npc(14))); assert!(text.contains("cover")&&text.contains("Difficult 11+"),"{text}");
    assert!(fire::in_cover(g,round::traveller_cell(g,0),round::npc_cell(g,14)));
    let mut k=open("efate_knife"); let g=&mut k.game;
    let text=hover(g,Target::Unit(Unit::Npc(15))); assert!(text.starts_with("Hostile (thug, NPC 15): Close: ")&&text.contains("block"),"{text}");
    let m=orders::menu_for(g,Target::Unit(Unit::Npc(15))); assert_eq!(m.items.iter().find(|i|i.label=="Melee").unwrap().grey,None);
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(15))});
    assert_eq!(logged(g," attacks thug (NPC 15) with the dagger: close: 2D "),1,"{:?}",g.squad.as_ref().unwrap().p1.log);
    assert!(g.squad.as_ref().unwrap().p1.turns[0].done,"the attacker's turn ends with the attack");
    let mut w=open("efate_wall"); let g=&mut w.game;
    assert_eq!(hover(g,Target::Unit(Unit::Npc(14))),"Hostile (thug, NPC 14): no line of sight");
    assert!(fire::line_cells(g,round::traveller_cell(g,0),round::npc_cell(g,14)).len()<5,"the wall cells (24, 10) / (24, 11) end the line");
}

#[test]
fn ui_on_the_efate_scenarios() {
    use mt1::ground::{orders, round, rules};
    use mt1::ui::{scene::Squad, Intent, Scene, ScreenKind, Target, Unit, Verb};
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let hover=|g:&mut Ground,k:u8|{orders::hover(g,Some(Target::Unit(Unit::Npc(k))));Squad::capture(g)};
    let mut b=open("efate_broken"); let g=&mut b.game;
    let sq=hover(g,14); assert_eq!(&sq.preview[..2],["Hostile (thug, NPC 14)","Short: Routine 7+"],"{:?}",sq.preview);
    assert!(sq.preview[2].starts_with("DM +3: skill 2, Dex +1")&&sq.preview[3]=="needs 4+ on 2D (91%)"&&sq.preview[4].contains(" pen: ")&&sq.preview.iter().any(|l|l.contains("in the line")),"{:?}",sq.preview);
    let s=sq.sight.unwrap(); assert_eq!((s.from,s.to,s.los,s.band),((22,5),round::npc_cell(g,14),true,rules::Band::Short));
    g.mem.set_party_u8(0x136,0);
    let sq=hover(g,14); assert!(sq.preview[1]=="Short: Difficult 11+"&&sq.preview[2].contains("unskilled"),"{:?}",sq.preview);
    g.mem.set_party_u8(0x136,2);
    let scene=Scene::capture(g,ScreenKind::Ground,None);
    assert_eq!((scene.squad.pips[0],scene.squad.mp[0],scene.squad.pools,scene.squad.full_auto[0]),([true,true],10,(0,0),false));
    let (id,mags,rounds)=scene.party.hand(0,scene.walkers[0].hand()).expect("the hand weapon"); assert_eq!((rules::weapon(id).map(|w|w.name),mags,rounds),(Some("9mm advanced combat rifle"),3,20));
    orders::intent(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    let f=Squad::capture(g).fx; assert_eq!((f.unit,f.shots.len(),f.shots[0].0),(Some(Unit::Traveller(0)),1,(22,5)),"{f:?}");
    let seq=f.seq; let n0=Squad::capture(g).log_n; let start=round::npc_cell(g,14);
    orders::intent(g,Intent::EndTurn);
    let sq=Squad::capture(g); let f=sq.fx; assert_eq!((f.unit,f.walk[0]),(Some(Unit::Npc(14)),start),"{f:?}");
    assert!(f.seq>seq&&!f.shots.is_empty()&&sq.log_n>n0+1,"record 14 fired: {f:?} {:?}",sq.log);
    if sq.log.iter().any(|l|l.contains("damage to ")) { assert!(f.hits.iter().any(|h|matches!(h.0,Unit::Traveller(_))),"the hit digit on the Traveller: {f:?}") }
    assert!(sq.strip.starts_with("Your move: "),"{}",sq.strip);
    let mut c=open("efate_cover"); let g=&mut c.game; let sq=hover(g,14); assert!(sq.sight.unwrap().cover&&sq.preview[2].contains("cover"),"{:?}",sq.preview);
    let mut w=open("efate_wall"); let g=&mut w.game; let sq=hover(g,14); assert!(!sq.sight.unwrap().los&&sq.preview[1]=="no line of sight","{:?}",sq.preview);
    let before=(sq.preview.clone(),sq.sight); g.mem.set_view_y(g.mem.view_y()+20); let sq=hover(g,14); assert_eq!((sq.preview,sq.sight),before,"the view origin plays no part");
    let mut o=open("efate_orders"); let g=&mut o.game; let d=Squad::capture(g).destinations[1].expect("a destination"); assert_eq!(round::squares(round::traveller_cell(g,1),d),14);
}
#[test]
fn fight_scenarios_open_with_a_conscious_traveller() {
    let Some(dir)=gamefiles() else {return};
    let mut seen=0;
    for folder in ["scenarios","tests/atoms/scenarios"] { for e in std::fs::read_dir(root().join(folder)).unwrap() {
        let path=e.unwrap().path(); if path.extension().and_then(|s|s.to_str())!=Some("toml") {continue}
        let text=std::fs::read_to_string(&path).unwrap(); if !text.contains("[squad.encounter]")||!text.contains("to_act = \"party\"") {continue}
        let Ok(n)=load(&path,&dir) else {continue}; seen+=1;
        assert!((0..5).any(|i|mt1::ground::round::conscious(&n.game,i)),"{}: no conscious Traveller",path.display());
    }}
    assert!(seen>0);
}
#[test]
fn npc_turn_on_the_efate_scenarios() {
    use mt1::ground::round;
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let log=|g:&Ground|g.squad.as_ref().unwrap().p1.log.clone();
    let fired=|g:&Ground,k:u8|log(g).iter().any(|l|l.contains(&format!("(NPC {k}) fires the"))||l.starts_with(&format!("NPC {k} fires the")));
    let acted=|g:&Ground,k:u8|fired(g,k)||log(g).iter().any(|l|l.contains(&format!("(NPC {k}) closes in"))||l.contains(&format!("(NPC {k}) has no ammunition"))||l.contains(&format!("(NPC {k}) holds fire")));
    let mut n=open("efate_npc_turn"); let g=&mut n.game;
    round::pass(g,&[]);
    for k in [14u8,15,16] { assert!(acted(g,k),"NPC {k}: {:?}",log(g)) }
    assert!(log(g).iter().any(|l|l.contains("(NPC 14) fires the 9mm advanced combat rifle at ")&&l.contains(": skill 0, 2D ")),"{:?}",log(g));
    assert_eq!(g.squad.as_ref().unwrap().p1.encounter.as_ref().map(|e|e.round),Some(2),"{:?}",log(g));
    let mut u=open("efate_unmapped"); let g=&mut u.game;
    round::end_turn(g);
    assert!(log(g).iter().any(|l|l.starts_with("NPC 14 fires the 9mm advanced combat rifle at ")&&l.contains(": skill 1 (derived), 2D ")),"{:?}",log(g));
    let mut m=open("efate_morale"); let g=&mut m.game;
    round::end_turn(g);
    assert!(log(g).iter().any(|l|l.starts_with("Morale (2 of 3 down, Low): Formidable 15+, Leader 0: 2D ")),"{:?}",log(g));
    let mut w=open("efate_wall"); let g=&mut w.game;
    let (saw,from)=(round::sees(g,14).is_some(),round::npc_cell(g,14));
    round::end_turn(g);
    assert!(acted(g,14),"{:?}",log(g));
    if !saw { assert!(round::npc_cell(g,14)!=from,"no line of sight: it walks ({from:?}): {:?}",log(g)) }
    let mut j=open("efate_juke"); let g=&mut j.game;
    for _ in 0..5 { if fired(g,14) { break } round::end_turn(g) }
    assert!(fired(g,14),"{:?}",log(g));
    let mut a=open("efate_alerted"); let g=&mut a.game;
    assert!(!round::in_encounter(g)); let c0=round::npc_cell(g,14).1;
    for _ in 0..10 { if round::in_encounter(g) { break } round::walked(g,1); round::pass(g,&[]); }
    let c1=round::npc_cell(g,14).1;
    assert!(c0-c1>=3,"record 14 closed in: {c0} -> {c1}: {:?}",log(g));
    if round::in_encounter(g) { assert!(log(g).iter().any(|l|l.starts_with("No surprise: they were expecting you")),"{:?}",log(g)) }
}

#[test]
fn rules_on_the_efate_scenarios() {
    use mt1::ground::{keymap, orders, round, standing, tactics, rules::adapters, state::{Mem, W_TARGET}};
    use mt1::ui::{Intent, Target, Unit};
    let Some(dir)=gamefiles() else {return};
    let open=|name:&str|load(&root().join(format!("scenarios/{name}.toml")),&dir).unwrap_or_else(|e|panic!("{e}"));
    let log=|g:&Ground|g.squad.as_ref().unwrap().p1.log.clone();
    let logged=|g:&Ground,s:&str|log(g).iter().filter(|l|l.contains(s)).count();
    let strip=|g:&Ground|g.squad.as_ref().unwrap().p1.strip.clone();
    let hover=|g:&Ground|g.squad.as_ref().unwrap().orders.hover.clone();
    let name=|g:&Ground,i:u8|round::name(g,i);
    let mut b=open("efate_broken"); let g=&mut b.game;
    orders::select(g,2); g.bios_key=keymap::BIOS_SHIFT_H; assert!(round::key(g,keymap::SCAN_H)); g.bios_key=0;
    orders::perform(g,orders::Op::HoldRun,Target::Portrait(2));
    let t=g.squad.as_ref().unwrap().p1.turns[2]; assert!(t.hold&&t.speed==2,"{t:?} {}",hover(g));
    orders::select(g,0); orders::intent(g,Intent::EndTurn);
    let n3=name(g,2);
    assert_eq!(logged(g,&format!("Interrupt ({n3} on ")),1,"{:?}",log(g));
    let roll=log(g).iter().find(|l|l.starts_with("Interrupt (")).cloned().unwrap(); assert!(roll.contains(", speed 2): 2D ")&&roll.contains(" + 2 = ")&&roll.contains(" vs 7: "),"{roll}");
    if roll.ends_with("success") {
        assert_eq!(strip(g),format!("{n3} interrupts: your move")); assert_eq!(round::selected(g),2);
        assert!(g.squad.as_ref().unwrap().p1.encounter.as_ref().unwrap().interrupt.is_some()&&!g.squad.as_ref().unwrap().p1.npcs.get(&14).is_some_and(|n|n.acted==false));
        let shots=logged(g,"(NPC 14) fires the"); orders::intent(g,Intent::EndTurn);
        assert!(logged(g,"(NPC 14) fires the")>shots||logged(g,"(NPC 14) has no ammunition")>0||logged(g,"(NPC 14) holds fire")>0,"record 14 finished its turn after the interrupt: {:?}",log(g));
        assert!(g.squad.as_ref().unwrap().p1.encounter.as_ref().unwrap().interrupt.is_none()&&g.squad.as_ref().unwrap().p22.paused.is_none());
    } else { assert!(roll.ends_with("fails, the hold stays")&&g.squad.as_ref().unwrap().p1.turns[2].hold,"{roll}"); }
    let mut h=open("efate_hold"); let g=&mut h.game;
    assert_eq!(round::traveller_cell(g,2),(30,14));
    orders::select(g,2); tactics::hold(g,1); assert!(g.squad.as_ref().unwrap().p1.turns[2].hold,"{}",hover(g));
    for _ in 0..8 { if logged(g,"hold released")>0 { break } let next=(0..5u8).find(|&i|i!=2&&round::conscious(g,i)&&!g.squad.as_ref().unwrap().p1.turns[i as usize].done); let Some(i)=next else { break }; orders::select(g,i); orders::intent(g,Intent::EndTurn); }
    let n3=name(g,2);
    assert_eq!(logged(g,"Interrupt ("),0,"no NPC in Traveller 3's sight: no roll; {:?}",log(g));
    assert!(logged(g,&format!("{n3}: hold released"))==1&&strip(g)==format!("Your move: {n3}"),"{:?} {}",log(g),strip(g));
    for k in [14u8,15,16] { assert!(g.squad.as_ref().unwrap().p1.npcs.get(&k).is_some_and(|n|n.acted),"NPC {k} acted first: {:?}",log(g)) }
    orders::intent(g,Intent::EndTurn); assert_eq!(g.squad.as_ref().unwrap().p1.encounter.as_ref().map(|e|e.round),Some(2),"{:?}",log(g));
    let mut t=open("efate_tactics"); let g=&mut t.game;
    assert_eq!(tactics::pool(g),3,"{}",strip(g));
    assert_eq!((tactics::hud(g).as_str(),tactics::cap(g),round::traveller_fighter(g,0).tactics,round::traveller_fighter(g,2).tactics),("Tactics 3",2,2,1));
    orders::hover(g,Some(Target::Unit(Unit::Npc(14)))); let before=hover(g); assert!(before.contains("Routine 7+")||before.contains("Simple")||before.contains("Difficult"),"{before}");
    assert!(round::key(g,keymap::SCAN_KP_PLUS)); assert!(round::key(g,keymap::SCAN_KP_PLUS));
    assert_eq!((tactics::declared(g),tactics::left(g),tactics::hud(g).as_str()),(2,1,"Tactics 1"));
    orders::hover(g,None); orders::hover(g,Some(Target::Unit(Unit::Npc(14)))); assert!(hover(g).contains("+2 tactics"),"{}",hover(g));
    assert!(round::key(g,keymap::SCAN_KP_PLUS)); assert_eq!(hover(g),"Tactics: maximum 2 (the highest single level)");
    let m=orders::menu_for(g,Target::Unit(Unit::Npc(14))); let it=m.items.iter().find(|i|i.label.starts_with("Tactics")).unwrap(); assert_eq!((it.label.as_str(),it.grey.as_deref()),("Tactics +3",Some("maximum 2 (the highest single level)")));
    orders::intent(g,Intent::Verb{verb:mt1::ui::Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    let n1=name(g,0);
    assert_eq!((logged(g,&format!("{n1} spends 2 tactical points (1 left this round)")),tactics::declared(g),tactics::left(g)),(1,0,1),"{:?}",log(g));
    let mut o=open("efate_orders"); let g=&mut o.game;
    let n2=name(g,1);
    assert_eq!((orders::read_route(g,1).len(),g.mem.read(Mem::walker(1).add(W_TARGET),1)),(14,14));
    assert_eq!(standing::stance(g,1),"crosshair");
    let armed=adapters::traveller_weapon(g,1).is_some();
    orders::intent(g,Intent::EndTurn);
    assert!(strip(g).starts_with(&format!("Your move: {n2}. Continue (O): "))&&strip(g).ends_with("walk 10"),"{}",strip(g));
    let from=round::traveller_cell(g,1);
    assert!(!round::key(g,round::KEY_SPACE),"Space fires nothing"); assert!(round::key(g,round::KEY_O));
    assert_eq!(logged(g,&format!("{n2} continues")),1,"{:?}",log(g));
    if armed { assert!(logged(g,&format!("{n2} fires the"))==1||logged(g,&format!("{n2} reloads"))==1,"the standing target's shot (or the reload when dry): {:?}",log(g)) }
    assert!(logged(g,&format!("{n2} walks "))==1&&round::squares(round::traveller_cell(g,1),from)>=1&&g.squad.as_ref().unwrap().p1.turns[1].done,"{:?}",log(g));
    let (walked,left)=(round::squares(round::traveller_cell(g,1),from),orders::read_route(g,1).len());
    assert!(walked+left as i32>=10&&left<=4,"10 of the 14 this round: walked {walked}, {left} to go; {:?}",log(g));
    let mut o=open("efate_orders"); let g=&mut o.game;
    orders::select(g,1); assert!(round::key(g,keymap::SCAN_BACKSPACE)); assert_eq!(g.mem.read(Mem::walker(1).add(W_TARGET),1),0); assert_eq!(standing::stance(g,1),"boot");
    orders::select(g,0); orders::intent(g,Intent::EndTurn);
    assert!(!round::key(g,round::KEY_SPACE)); assert!(round::key(g,round::KEY_O));
    assert_eq!((logged(g,&format!("{n2} fires")),logged(g,&format!("{n2} walks "))),(0,1),"{:?}",log(g));
    let mut o=open("efate_orders"); let g=&mut o.game;
    let n1=name(g,0); let from=round::traveller_cell(g,0);
    let to=(-6i16..=6).flat_map(|dr|(-6i16..=6).map(move|dc|(from.0+dr,from.1+dc))).find(|&c|round::squares(c,from)>=5&&orders::find_path(g,0,from,c).is_some_and(|p|p.len()<=10)).expect("a square about 6 away with a path");
    let m=orders::menu_for(g,Target::Cell(to)); let a=m.items.iter().find(|i|i.label=="Advance").unwrap(); assert_eq!((a.key,a.grey.as_deref()),(b'A',None),"{:?}",a);
    orders::perform(g,orders::Op::Advance,Target::Cell(to));
    assert!(logged(g,&format!("{n1} walks "))==1,"{:?}",log(g));
    assert!(logged(g,&format!("{n1} fires the"))==1||logged(g,&format!("{n1} advances: nobody in sight"))==1,"{:?}",log(g));
    let mut b=open("efate_broken"); let g=&mut b.game;
    assert!(round::key(g,3)); assert_eq!(round::selected(g),1);
    assert!(round::key(g,keymap::SCAN_TAB)); let first=g.mem.read(Mem::walker(1).add(W_TARGET),1); assert!(first!=0,"{}",hover(g));
    assert!(round::key(g,keymap::SCAN_TAB)); let second=g.mem.read(Mem::walker(1).add(W_TARGET),1); assert!(second!=0&&second!=first,"two figures: {first} {second}; {}",hover(g));
    assert!(round::key(g,0x21));
    if adapters::traveller_weapon(g,1).is_some() { assert!(logged(g,&format!("{} fires the",name(g,1)))==1||logged(g,&format!("{} reloads",name(g,1)))==1,"{:?} {}",log(g),hover(g)) } else { assert_eq!(hover(g),"no weapon in hand") }
    let mut h=open("efate_hold"); let g=&mut h.game;
    orders::select(g,2); g.bios_key=keymap::BIOS_SHIFT_TAB; assert!(round::key(g,keymap::SCAN_TAB)); g.bios_key=0;
    assert_ne!(round::selected(g),2); assert!(!round::key(g,keymap::SCAN_ESC));
}

#[test]
fn same_calls_and_write_set_on_efate_broken() {
    use mt1::ground::{orders, round, trace::{Call, Trace}, state::{Mem, NPCS, NPC_SIZE, PARTY_PTR, WALKERS, WALKER_SIZE}};
    use mt1::ui::{Intent, Target, Unit, Verb};
    use std::collections::BTreeSet;
    let Some(dir)=gamefiles() else {return};
    let mut b=load(&root().join("scenarios/efate_broken.toml"),&dir).unwrap_or_else(|e|panic!("{e}")); let g=&mut b.game;
    g.rand.lcg.seed=11; g.scene=None;
    let (mut calls,mut own,mut frame_names)=(Vec::<Call>::new(),Vec::<(u16,u16,Vec<u8>)>::new(),Vec::<&'static str>::new());
    let mut step=|g:&mut Ground,intent:Intent|{
        g.mem.trace=Trace::default(); g.mem.trace.lean=true; g.mem.trace.on=true; g.mem.trace_enter("squad_step",&[]);
        orders::intent(g,intent);
        g.mem.trace.exit(None);
        let frames=std::mem::take(&mut g.mem.trace.done); g.mem.trace=Trace::default();
        let outer=frames.iter().find(|f|f.name=="squad_step").expect("the step frame");
        let mut w=outer.writes.clone();
        for f in frames.iter().filter(|f|f.name!="squad_step") { frame_names.push(f.name); for c in &f.calls { calls.push(c.clone()) } for x in &f.writes { if let Some(i)=w.iter().position(|o|o==x) { w.remove(i); } } }
        calls.extend(outer.calls.iter().cloned()); own.extend(w);
    };
    let who=round::selected(g); let from=round::traveller_cell(g,who);
    step(g,Intent::Verb{verb:Verb::Fire,target:Target::Unit(Unit::Npc(14))});
    let ring: Vec<(i16,i16)>=(-3i16..=3).flat_map(|dr|(-3i16..=3).map(move |dc|(dr,dc))).filter(|&(dr,dc)|dr.abs()==3||dc.abs()==3).map(|(dr,dc)|(from.0+dr,from.1+dc)).collect();
    let to=ring.into_iter().find(|&c|orders::find_path(g,who,from,c).is_some_and(|p|!p.is_empty())).expect("a reachable square 3 away");
    step(g,Intent::Verb{verb:Verb::Move,target:Target::Cell(to)});
    for _ in 0..3 { step(g,Intent::EndTurn) }
    let log=&g.squad.as_ref().unwrap().p1.log;
    let names: Vec<&str>=calls.iter().map(|c|c.name).collect();
    let hits: Vec<&Call>=calls.iter().filter(|c|c.name=="take_hit").collect();
    assert!(!hits.is_empty()&&hits.iter().all(|c|c.args.len()==3&&c.args[1]==100&&c.args[0]<5),"take_hit(who, 100, dmg) for every Traveller hit; {names:?} {log:?}");
    let moves: Vec<&Call>=calls.iter().filter(|c|c.name=="move_party").collect();
    assert!(moves.len()>=6&&moves.iter().all(|c|c.args.len()==3&&c.args[2]<=8&&c.args[0].wrapping_add(1)<=2&&c.args[1].wrapping_add(1)<=2),"move_party(dx, dy, direction) twice a square for 3 squares; {moves:?}");
    assert!(names.contains(&"set_leader"),"set_leader when the next Traveller's turn comes; {names:?}");
    for c in &calls { assert!(!matches!(c.name,"party_fire"|"monster_attack"|"skill_roll"),"the 1990 attack path is never called in squad mode: {}",c.name) }
    for f in &frame_names { assert!(!matches!(*f,"party_fire"|"monster_attack"|"skill_roll"),"{f}") }
    let seen: BTreeSet<&str>=names.iter().copied().filter(|n|matches!(*n,"take_hit"|"char_dies"|"drop_all"|"npc_dies"|"npc_can_step"|"break_party"|"set_leader"|"move_party")).collect();
    eprintln!("P9b.3 same calls seen: {seen:?} ({} calls, framed routines {:?})",calls.len(),frame_names.iter().collect::<BTreeSet<_>>());
    let party=g.mem.pointer(PARTY_PTR); let n_npcs=g.mem.npc_count() as u16;
    let (mut bad,mut party_set,mut npc_set,mut walker_set)=(BTreeSet::new(),BTreeSet::new(),BTreeSet::new(),BTreeSet::new());
    for (seg,off,bytes) in &own { for i in 0..bytes.len() as u16 { let o=off.wrapping_add(i);
        if *seg==party.seg { let rel=o.wrapping_sub(party.off); party_set.insert(rel);
            let member=rel>=0x102&&rel<0x102+5*256; let m=if member {(rel-0x102)%256} else {0xFFFF};
            let ok=matches!(rel,0x00|0x08|0x81|0x88|0xA0..=0xAB) || (member&&(matches!(m,0x23..=0x25|0x28|0xC9..=0xCB)||(0x79..=0x79+69).contains(&m))) || (rel>=0xA82&&matches!((rel-0xA82)%5,1|2));
            if !ok { bad.insert(format!("party+0x{rel:X}")); } }
        else if *seg==Mem::DS && (NPCS..NPCS+NPC_SIZE*n_npcs).contains(&o) { let rel=(o-NPCS)%NPC_SIZE; npc_set.insert(rel); if !matches!(rel,0x02|0x16|0x17|0x1E|0x20..=0x28) { bad.insert(format!("npc{}+0x{rel:X}",(o-NPCS)/NPC_SIZE)); } }
        else if *seg==Mem::DS && (WALKERS..WALKERS+WALKER_SIZE*5).contains(&o) { let rel=(o-WALKERS)%WALKER_SIZE; walker_set.insert(rel); if !matches!(rel,2|5|8..=19|22..=84) { bad.insert(format!("walker{}+0x{rel:X}",(o-WALKERS)/WALKER_SIZE)); } }
    } }
    let hex=|s:&BTreeSet<u16>|s.iter().map(|o|format!("{o:X}")).collect::<Vec<_>>();
    eprintln!("P9b.4 squad writes: party {:?} npc {:?} walker {:?}",hex(&party_set),hex(&npc_set),hex(&walker_set));
    assert!(bad.is_empty(),"writes outside the enumerated set: {bad:?}; log {log:?}");
}
#[test]
fn a_dark_level_needs_a_lit_light_and_efate_door_is_lit() {
    let Some(dir)=gamefiles() else {return};
    let a=super::assets::Assets::load(&dir).unwrap();
    let text=std::fs::read_to_string(root().join("scenarios/efate_door.toml")).unwrap();
    let v=validate::document(&text).unwrap(); a.validate(&v).unwrap();
    let mut dark=v.clone(); dark["party"]["items"][0]["pack"]=serde_json::json!([]);
    assert!(a.validate(&dark).unwrap_err().contains("dark level"),"a level 2 placement without a light is refused");
    let mut off=v.clone(); off["party"]["items"][0]["pack"][0]["charge"]=serde_json::json!(0);
    assert!(a.validate(&off).is_err(),"a torch turned off does not light the room");
    let o=load(&root().join("scenarios/efate_door.toml"),&dir).unwrap(); let g=&o.game;
    assert_eq!(g.mem.party_u16(0xF82),1,"one light turned on (the 1990 light count)");
    let s=mt1::ui::Scene::capture(g,mt1::ui::ScreenKind::Ground,None); assert!(!s.art.dark,"the room is drawn");
}

#[test]
fn the_conversation_shows_its_1990_page_and_then_the_map_again() {
    use mt1::{ground::state::Mem, ui::{Scene, ScreenKind}};
    use std::{cell::RefCell, rc::Rc};
    let Some(dir)=gamefiles() else {return};
    struct Keys(Vec<u16>, Rc<RefCell<Vec<ScreenKind>>>);
    impl mt_engine::io::Input for Keys {
        fn next_key(&mut self)->u16 { if self.0.is_empty() {27} else {self.0.remove(0)} }
        fn poll(&mut self,_:&mut dyn mt_engine::sim::tick::Clock)->mt_engine::io::Event { mt_engine::io::Event::Key(self.next_key()) }
        fn scene(&mut self,s:&dyn std::any::Any) { if let Some(s)=s.downcast_ref::<Scene>() { self.1.borrow_mut().push(s.screen) } }
    }
    let mut l=load(&root().join("tests/atoms/scenarios/atom_101_tavern.toml"),&dir).unwrap_or_else(|e|panic!("{e}"));
    let g=&mut l.game; let seen=Rc::new(RefCell::new(Vec::new()));
    g.harness=false; g.input=Box::new(Keys(vec![0x114],seen.clone())); attach_scene(g); seen.borrow_mut().clear();
    let id=(1..64).find(|&i|{let k=g.mem.read(Mem::npc(i).add(9),1); k!=0 && !(25..54).contains(&k)}).expect("a person on the level");
    let (cell,_)=mt1::ground::commands::facing_cell(g); g.mem.write(cell.add(2),id as u32,1);
    mt1::ground::commands_talk::talk(g);
    let seen=seen.borrow();
    assert!(seen.len()>2 && seen[..seen.len()-1].iter().filter(|&&k|k==ScreenKind::Picture).count()>2,"the conversation is shown as the 1990 page: {seen:?}");
    assert_eq!(seen.last(),Some(&ScreenKind::Ground),"leaving the conversation returns the map: {seen:?}");
}
