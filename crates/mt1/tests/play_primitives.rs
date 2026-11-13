mod oracle_env;
use mt1::{ground::{self, Ground, state::Mem, starport::{self, shop_menu}}, screens::cg::Screens, ui::{CgUi, Menu}};
use mt_engine::{io::{keys, Dir, Event, Input, Key, Overlay}, play::{Choice, Decision, Kind, Pick, Play, SessionExit}, sim::tick::{Clock, Counter}};
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

struct Native(VecDeque<Event>);
impl Native {
    fn keys(keys: impl IntoIterator<Item=Key>) -> Self { Self(keys.into_iter().flat_map(|k|[Event::Key(k),Event::KeyUp(k)]).collect()) }
}
impl Input for Native {
    fn next_key(&mut self) -> Key { loop { match self.0.pop_front().expect("native script exhausted") {Event::Key(k)=>return k,_=>{}} } }
    fn key_pending(&mut self) -> bool { self.0.iter().any(|e|matches!(e,Event::Key(_))) }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event { { let e=self.0.pop_front().unwrap_or(Event::Idle); if e==Event::Idle {clock.wait_tick();} e } }
}
fn data(picks: impl IntoIterator<Item=Pick>) -> Box<dyn Input> {
    Box::new(Play::scripted(picks.into_iter().enumerate().map(|(i,p)|Choice::new(format!("d:{}",i+1),p))))
}
fn id(s: &str) -> Pick { Pick::Id(s.into()) }
fn ground(input: Box<dyn Input>) -> Ground {
    let mut g=Ground::from_ds(vec![0;65536],vec![0;Mem::PARTY_SIZE],input,Box::new(Counter::at(123)),Box::new(Overlay::new(None)));
    for b in b' '..=b'Z' { if let Some(scan)=mt_engine::io::scan::make_code(b as Key) {g.mem.set_u8(ground::input::ASCII+scan as u16,b);} }
    g.mem.set_i16(0x6E00,240); g.mem.set_i16(0x6E06,160); g.mem.set_i16(0x6E04,16);
    g.mem.set_i16(0x503E,1); g.mem.set_u32(0x4BDE,u32::MAX);
    g
}
fn pair(keys: &[Key], picks: Vec<Pick>, mut action: impl FnMut(&mut Ground)->String) {
    let mut native=ground(Box::new(Native::keys(keys.iter().copied()))); let mut play=ground(data(picks));
    assert_eq!(action(&mut native),action(&mut play));
    assert_eq!(mt1::play::hash(&mut native),mt1::play::hash(&mut play));
}
#[test]
fn ack_and_named_keys_keep_native_irq_and_clock() {
    for (choice,key) in [("continue",32),("back",27)] {
        pair(&[key],vec![id(choice)],|g|ground::input::menu_getkey(g).to_string());
        pair(&[key],vec![id(choice)],|g|ground::input::wait_scan(g).to_string());
    }
    for (choice,key) in [("fuel",70),("all",65),("pay",80),("back",27)] {
        pair(&[key],vec![id(choice)],|g| {
            mt1::play_prompts::named(g,"16374",&[("fuel","Fuel",70),("all","All",65),("pay","Pay",80),("back","Cancel",27)],&[]);
            ground::input::menu_getkey(g).to_string()
        });
    }
}
#[test]
fn text_number_and_cancel_equal_native() {
    pair(&[b'4' as Key,b'2' as Key,13],vec![Pick::Number(42)],|g|format!("{:?}",starport::input_field(g,8,false)));
    pair(&[b'A' as Key,b'B' as Key,b'7' as Key,13],vec![Pick::Text("AB7".into())],|g|format!("{:?}",starport::input_field(g,8,true)));
    pair(&[27],vec![id("back")],|g|format!("{:?}",starport::input_field(g,8,false)));
}
#[test]
fn invalid_text_is_transactional_and_emits_no_input() {
    let observed=Rc::new(RefCell::new(false)); let got=observed.clone();
    let input=Play::with_service(move |p| {
        let before=p.pending().unwrap().state.clone(); let decision=p.pending().unwrap().id.clone();
        for pick in [Pick::Text("TOOLONG".into()),Pick::Text("a".into()),Pick::Text("!".into())] {
            assert!(p.choose(Choice::new(&decision,pick)).is_err()); assert_eq!(p.pending().unwrap().state,before);
            assert!(p.journal().entries.is_empty());
        }
        *got.borrow_mut()=true; p.choose(Choice::new(&decision,Pick::Text("A2".into()))).unwrap();
    });
    assert_eq!(starport::input_field(&mut ground(Box::new(input)),4,true),Some(b"A2".to_vec()));
    assert!(*observed.borrow());
}
fn shop(g: &mut Ground, rows: usize, select: bool, sign: i8) -> String {
    let names=["Heading","One","Two","Three","Heading2","Four","Five"].map(String::from); let mut quantities=[0;7];
    let choice=shop_menu::run(g,shop_menu::Menu {title:"Fixture",names:&names,prices:&[0,5,10,15,0,20,25],quantities:&mut quantities,
        limits:&[9;7],rows,select_only:select as u8,money:1000,max_load:6,weights:None,kind:0,sign,colors:None});
    format!("{choice}:{quantities:?}")
}
#[test]
fn buy_two_third_priced_row_paging_and_exit_equal_native() {
    for rows in [3,7] {
        pair(&[keys::DOWN,keys::DOWN,b'+' as Key,b'+' as Key,13],vec![Pick::Row {row:"list:11571/3".into(),qty:2}],|g|shop(g,rows,false,1));
        pair(&[27],vec![id("exit")],|g|shop(g,rows,false,1));
        pair(&[keys::DOWN,keys::DOWN,keys::DOWN,13],vec![Pick::Row {row:"list:11571/5".into(),qty:1}],|g|shop(g,rows,true,1));
    }
    pair(&[b'+' as Key,13],vec![Pick::Row {row:"list:11571/1".into(),qty:1}],|g|shop(g,7,false,-1));
}
#[test]
fn payer_selection_and_exit_equal_native() {
    for (pick,keys) in [(Pick::Row {row:"list:1199B/1".into(),qty:1},vec![keys::DOWN,13]),(id("exit"),vec![27])] {
        pair(&keys,vec![pick],|g| {
            for i in 0..2 {g.mem.set_party_u8(0x127+256*i,10);g.mem.set_party_u8(0x102+256*i,b'A'+i as u8);starport::put_cash(g,i as i16,100);}
            starport::ui::payer(g,b"Who pays",255,1).to_string()
        });
    }
}
#[test]
fn character_and_save_prompt_equal_native() {
    for (choice,key) in [("back",27),("cmd:equip",b'U' as Key),("cmd:char/3",keys::F1+3),("action:5",b'6' as Key)] {
        pair(&[key],vec![id(choice)],|g|ground::input::character_scan(g,0).to_string());
    }
    for restore in [false,true] {for (choice,key) in [("continue",32),("back",27)] {
        pair(&[key],vec![id(choice)],|g| {starport::save_stop(g,restore);format!("{}",g.restore_pending.is_some())});
    }}
}
fn gamefiles() -> Option<PathBuf> { oracle_env::gamefiles() }
fn cg(input: Box<dyn Input>) -> Option<Screens> {
    let dir=gamefiles()?; let read=|name|std::fs::read(dir.join(name)).unwrap();
    let mut ui=Screens::new(mt_engine::data::exepack::unpack(&read("CG.DAT")).unwrap().exe,
        mt_engine::data::ly::decode(&read("FONT.LY")).unwrap(),mt_engine::data::ly::decode(&read("CG.LY")).unwrap(),
        input,Box::new(Counter::at(123)),Box::new(Dir(dir.clone()))).unwrap();
    let mut files=Overlay::new(Some(&dir)); files.hide("UCHARS.DAT"); ui.overlay=Some(files); Some(ui)
}
#[test]
fn cg_vertical_horizontal_yesno_text_ack_equal_native_hash() {
    for horizontal in [false,true] {for i in 0..3 {
        let mut native_keys=vec![if horizontal {keys::RIGHT} else {keys::DOWN};i]; native_keys.push(13);
        let routine=if horizontal {"56a6"} else {"519a"};
        let Some(mut a)=cg(Box::new(Native::keys(native_keys))) else {return};
        let mut b=cg(data([id(&format!("menu:{routine}.40:20/{i}"))])).unwrap();
        let m=Menu {items:&["Alpha","Beta","Gamma"],highlight:4,row:40,col:20,allow_unique:1,border:2,color:0,extra:&[1,0,3],initial:2};
        assert_eq!(if horizontal {a.hmenu(&m)} else {a.menu(&m)},i as i16);
        assert_eq!(if horizontal {b.hmenu(&m)} else {b.menu(&m)},i as i16);
        assert_eq!(a.play_hash(),b.play_hash());
    }}
    for (pick,native) in [("yes",vec![13]),("no",vec![keys::DOWN,13])] {
        let mut a=cg(Box::new(Native::keys(native))).unwrap();let mut b=cg(data([id(pick)])).unwrap();
        assert_eq!(a.yes_no(&["Yes","No"]),b.yes_no(&["Yes","No"]));assert_eq!(a.play_hash(),b.play_hash());
    }
    let mut a=cg(Box::new(Native::keys([65,100,97,13,32]))).unwrap();let mut b=cg(data([Pick::Text("Ada".into()),id("continue")])).unwrap();
    assert_eq!(a.name(15),b.name(15));assert_eq!(a.play_hash(),b.play_hash());
    assert_eq!(a.read_key(),b.read_key());assert_eq!(a.play_hash(),b.play_hash());
}
#[test]
fn deleting_a_pool_character_asks_first_in_product_and_never_in_1990() {
    let run=|input:Box<dyn Input>| { let mut ui=cg(input)?; let mut flow=mt1::chargen::tables::Cg::new(ui.ds.clone());
        assert_eq!(mt1::chargen::pool::load_pool(&mut flow,&mut ui,"DCHARS.DAT",true),0); let n=flow.pool_count();
        mt1::chargen::pool::delete_character(&mut flow,&mut ui); Some(n-flow.pool_count()) };
    let Some(native)=run(Box::new(Native::keys([13]))) else {return};
    assert_eq!(native,1,"1990: Enter on a name deletes at once");
    let name=id("menu:519a.55:30/0");
    assert_eq!(run(data([name.clone(),id("no")])),Some(0),"No keeps the character");
    assert_eq!(run(data([name.clone(),id("yes")])),Some(1),"Yes deletes");
    for erase in [false,true] {
        let seen=Rc::new(RefCell::new(None)); struct Spy(Rc<RefCell<Option<String>>>,Play);
        impl Input for Spy { fn next_key(&mut self)->Key {self.1.next_key()} fn wants_decision(&self)->bool {true}
            fn decide(&mut self,d:&Decision) {*self.0.borrow_mut()=d.default.clone();self.1.decide(d)} }
        let mut q=cg(Box::new(Spy(seen.clone(),Play::scripted([Choice::new("d:1",id("no"))])))).unwrap();
        assert!(!if erase {q.yes_no(&["Yes","No"])} else {q.confirm("Delete Jurgen for good?")});
        assert_eq!(seen.borrow().as_deref(),Some("no"));
    }
}
#[test]
fn cg_pixel_width_is_rejected_before_playback() {
    let p=Play::with_service(|p| {
        let d=p.pending().unwrap().clone(); let text="W".repeat(d.input.as_ref().unwrap().max_len-1);
        assert!(p.choose(Choice::new(&d.id,Pick::Text(text))).is_err());assert_eq!(p.pending().unwrap().state,d.state);
        p.choose(Choice::new(&d.id,Pick::Text("Ada".into()))).unwrap();
    });
    let Some(mut s)=cg(Box::new(p)) else {return};assert_eq!(s.name(15),Some("Ada".into()));
}
fn space(input: Box<dyn Input>) -> Option<mt1::space::Space> {
    let dir=gamefiles()?;let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
    let mut sp=mt1::space::Space::new(&bytes,vec![0;mt1::space::state::PARTY_SIZE],vec![],input,Box::new(Counter::at(123)),Box::new(Dir(dir))).unwrap();
    sp.s.set_byte(mt1::space::state::KEY,0);sp.s.set_byte(mt1::space::state::PENDING_KEY,0);Some(sp)
}
#[test]
fn space_selector_station_and_quit_equal_native_hash() {
    for i in 0..12 {
        let mut keys=if i<10 {vec![keys::F1+i as u16]} else {vec![keys::DOWN;i]};if i>=10 {keys.push(13)}
        let Some(mut a)=space(Box::new(Native::keys(keys))) else {return};let mut b=space(data([id(&format!("menu:d7b4/{i}"))])).unwrap();
        let boxes:Vec<_>=(0..12).map(|n|[10+n*12,20,10,40]).collect();
        assert_eq!(a.select_rect(&boxes,0,6,8,None),i as i16);assert_eq!(b.select_rect(&boxes,0,6,8,None),i as i16);
        assert_eq!(mt1::space::play::hash(&mut a),mt1::space::play::hash(&mut b));
    }
    for (pick,key) in [("back",27),("cmd:assign",65),("cmd:stations",86),("cmd:up",keys::UP)] {
        let mut a=space(Box::new(Native::keys([key]))).unwrap();let mut b=space(data([id(pick)])).unwrap();
        assert_eq!(a.translate_station_key(),b.translate_station_key());assert_eq!(mt1::space::play::hash(&mut a),mt1::space::play::hash(&mut b));
    }
    for (pick,key) in [("yes",89),("no",78)] {
        let mut a=space(Box::new(Native::keys([key]))).unwrap();let mut b=space(data([id(pick)])).unwrap();
        assert_eq!(a.confirm_quit(),b.confirm_quit());assert_eq!(mt1::space::play::hash(&mut a),mt1::space::play::hash(&mut b));
    }
}
#[test]
fn space_hash_covers_state_and_excludes_evidence() {
    let Some(mut sp)=space(Box::new(Native::keys([]))) else {return};let baseline=mt1::space::play::hash(&mut sp);
    sp.ev.cursor+=1;sp.screen="evidence";assert_eq!(baseline,mt1::space::play::hash(&mut sp));
    sp.s.image_mut()[0]^=1;assert_ne!(baseline,mt1::space::play::hash(&mut sp));
    sp.s.image_mut()[0]^=1;sp.mouse.x+=1;assert_ne!(baseline,mt1::space::play::hash(&mut sp));
}
#[test]
fn list_invalid_quantity_disabled_row_and_money_are_transactional() {
    let p=Play::with_service(|p| {
        let d=p.pending().unwrap().clone(); assert_eq!(d.kind,Kind::List);
        for (row,qty) in [("list:11571/0",1),("list:11571/3",6),("list:11571/3",0)] {
            assert!(p.choose(Choice::new(&d.id,Pick::Row {row:row.into(),qty})).is_err());assert_eq!(p.pending().unwrap().state,d.state);
        }
        p.choose(Choice::new(&d.id,"exit")).unwrap();
    });
    let mut g=ground(Box::new(p));g.mem.set_i16(0x6E0A,2);shop(&mut g,3,false,1);
}

#[test]
fn no_registration_for_native_input() {
    struct NativeOnly;
    impl Input for NativeOnly { fn next_key(&mut self)->Key {27} fn poll(&mut self,_:&mut dyn Clock)->Event {Event::Key(27)} fn decide(&mut self,_:&Decision) {panic!("native input registered a decision")} }
    let mut g=ground(Box::new(NativeOnly));assert_eq!(ground::input::wait_scan(&mut g),1);
}

#[test]
fn missing_choice_stops_at_typed_wait() {
    let mut g=ground(Box::new(Play::new()));
    let error=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||ground::input::menu_getkey(&mut g))).unwrap_err();
    assert_eq!(error.downcast_ref::<SessionExit>(),Some(&SessionExit::AwaitingDecision));
}

#[test]
fn flight_wait_grants_use_exact_ticks_and_commands_reach_quit() {
    use mt1::space::{state::*,tick};
    let seen=Rc::new(RefCell::new(Vec::new()));let observations=seen.clone();
    let input=Play::with_service(move |p| {
        let d=p.pending().unwrap().clone(); observations.borrow_mut().push((d.context.routine.clone(),d.kind,p.journal().clone()));
        let pick=match p.sequence() {1=>"wait:5",2=>"cmd:right",3=>"cmd:quit",4=>"yes",n=>panic!("unexpected decision {n}: {d:?}")};
        p.choose(Choice::new(d.id,pick)).unwrap();
    });
    let Some(mut sp)=space(Box::new(input)) else {return};
    sp.s.set_word(CURRENT,-1);sp.s.set_word(ENCOUNTER,-1);sp.s.set_word(SHIP_COUNT,0);sp.s.set_word(mt1::space::system::COUNT,0);
    sp.s.set_word(VIEW,0);sp.s.set_word(DIRECTION,1);sp.s.set_word(VX,0);sp.s.set_word(VY,0);
    for i in 0..15 {sp.s.set_word(tick::ACTIVE+2*i,-1);}
    assert_eq!(sp.in_system_flight(),-1);
    assert_eq!(sp.clock.ticks(),128,"incidental scheduler polls must not advance virtual time");
    assert_eq!(sp.s.word(DIRECTION),8,"arrow make must survive to the flight dispatcher");
    let seen=seen.borrow();assert_eq!(seen.len(),4);assert_eq!(seen[0].1,Kind::Space);
    assert_eq!(seen[1].2.entries[0].grant,mt_engine::play::Grant {requested:5,used:5,remaining:0});
    assert_eq!(seen[3].0,"4ED0");
}

#[test]
fn casino_one_spin_equals_native_keys_on_virtual_clock() {
    let mut events=VecDeque::from([Event::Key(32),Event::KeyUp(32)]);events.extend([Event::Idle;46]);events.extend([Event::Key(27),Event::KeyUp(27)]);
    let mut native=ground(Box::new(Native(events)));let mut play=ground(data([id("spin"),id("back")]));
    let run=|g:&mut Ground| {
        g.mem.set_u32(0x6D13,1_000_000);starport::put_cash(g,0,1000);g.mem.set_party_u8(0,1);
        starport::casino::run(g);format!("{}:{}",starport::cash(g,0),g.rand.lcg.seed)
    };
    assert_eq!(run(&mut native),run(&mut play));assert_eq!(mt1::play::hash(&mut native),mt1::play::hash(&mut play));
}


const CHARGEN_TRANSCRIPT: &[Key] = &[13,13,13,13,13,13,13,13,keys::DOWN,13,keys::RIGHT,13,65,100,97,13];
fn generate_character(ui:&mut Screens)->[u8;256] {
    ui.rng.srand(0x1267);
    let mut flow=mt1::chargen::tables::Cg::new(ui.ds.clone());flow.set_pool_count(0);flow.set_next_slot(0);
    mt1::chargen::pool::generate(&mut flow,ui).unwrap();
    assert_eq!(flow.pool_count(),1);ui.pool_record.unwrap()
}
fn chargen_choices(corrupt_cash:bool)->Box<dyn Input> {
    let service=Play::with_service(move |p| {
        let d=p.pending().unwrap().clone(); assert!(p.sequence()<200,"chargen did not reach the pool");
        let pick=if d.kind==Kind::Text {
            Pick::Text("Ada".into())
        } else {
            let i=if d.context.table.as_deref()==Some("120,150") || d.context.table.as_deref()==Some("100,175") {1} else {0};
            let option=d.options.get(i).filter(|o|o.id!="back").unwrap_or(&d.options[0]);
            Pick::Id(option.id.clone())
        };
        p.choose(Choice::new(&d.id,pick)).unwrap();
    });
    struct CashRoute {play:Play,corrupt:bool}
    impl Input for CashRoute {
        fn data_mode(&self)->bool {self.play.data_mode()}
        fn wants_decision(&self)->bool {self.play.wants_decision()}
        fn decide(&mut self,d:&Decision) {
            let mut d=d.clone();
            if self.corrupt && d.options.iter().any(|o|o.id=="menu:56a6.100:175/1") {
                let benefits=d.options[0].input.clone();
                d.options.iter_mut().find(|o|o.id=="menu:56a6.100:175/1").unwrap().input=benefits;
            }
            self.play.decide(&d);
        }
        fn next_key(&mut self)->Key {self.play.next_key()}
        fn poll(&mut self,c:&mut dyn Clock)->Event {self.play.poll(c)}
        fn key_pending(&mut self)->bool {self.play.key_pending()}
        fn mouse(&mut self)->mt_engine::io::MouseState {self.play.mouse()}
    }
    Box::new(CashRoute {play:service,corrupt:corrupt_cash})
}
fn assert_chargen_matches_transcript(corrupt_cash:bool) {
    let Some(mut native)=cg(Box::new(Native::keys(CHARGEN_TRANSCRIPT.iter().copied()))) else {return};
    let mut play=cg(chargen_choices(corrupt_cash)).unwrap();
    assert_eq!(generate_character(&mut native),generate_character(&mut play),"independent native chargen transcript differs");
    assert_eq!(native.play_hash(),play.play_hash());
}
#[test]
fn complete_data_chargen_matches_native_pool_record() {assert_chargen_matches_transcript(false);}
#[test]
fn chargen_independent_transcript_detects_cash_routed_to_benefits() {
    if gamefiles().is_none() {return}
    let failure=std::panic::catch_unwind(||assert_chargen_matches_transcript(true)).expect_err("Cash incorrectly routed to Benefits escaped detection");
    assert!(failure.downcast_ref::<String>().is_some_and(|s|s.contains("independent native chargen transcript differs")),"wrong-route control failed for an unrelated reason");
}

#[test]
fn title_start_and_quit_match_native_frame_rng_and_time() {
    use mt1::screens::title::{Title,TcRand};
    use mt_engine::data::{exepack,ly,pic,Variant};
    let Some(dir)=gamefiles() else {return};let read=|name|std::fs::read(dir.join(name)).unwrap();
    let new=||Title::new(&exepack::unpack(&read("STARTUP.EXE")).unwrap().exe,
        ly::decode(&read("TITLE.LY")).unwrap(),ly::decode(&read("FONT.LY")).unwrap(),
        pic::decode(Variant::Mt1,&read("TIT.PIC")).unwrap(),pic::decode(Variant::Mt1,&read("ZOD.PIC")).unwrap()).unwrap();
    for (pick,key) in [("start",13),("quit",27)] {
        let (mut a,mut b)=(new(),new());let (mut ca,mut cb)=(Counter::at(123),Counter::at(123));
        let (mut ra,mut rb)=(TcRand {seed:123},TcRand {seed:123});
        let mut native=Native::keys([key]);let mut play=data([id(pick)]);
        assert_eq!(a.run(&mut ||ra.next(),&mut ca,&mut native,|_,_|{}),b.run(&mut ||rb.next(),&mut cb,play.as_mut(),|_,_|{}));
        assert_eq!(ca,cb);assert_eq!(ca.ticks(),159);assert_eq!(ra.seed,rb.seed);assert_eq!(a.fb.planes,b.fb.planes);
    }
}

#[test]
fn virtual_space_busy_wait_and_jump_animation_finish() {
    use mt1::space::{self,state::*};
    let Some(mut sp)=space(data([id("menu:d7b4/0")])) else {return};
    sp.wait_ticks(5);assert_eq!(sp.clock.ticks(),128);
    sp.s.party=mt1::ground::party::initial();sp.s.world=vec![0;0x2000];
    sp.s.set_pbyte(6,2);sp.s.set_pbyte(7,255);sp.s.set_pbyte(0xFC,124);sp.s.set_pbyte(0xC1,1);sp.s.set_pbyte(0xC0,2);
    for (i,p) in [163,0,164,1,5].into_iter().enumerate() {sp.s.set_pbyte(0xAC+i as u16,p);}
    sp.r.initialize(&mut sp.s);space::system::load(&mut sp.s,sp.files.as_mut(),2).unwrap();
    sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);sp.s.set_word(MOVING,0);sp.s.set_word(THRUST,0);
    let tables=space::jump::Tables::from_ds(sp.s.image()).unwrap();
    let menu=space::jump::menu(&sp.s.party.as_slice().try_into().unwrap(),&tables).unwrap();let dest=tables.system_for_row[menu.rows[0] as usize];
    sp.jump_screen().unwrap();
    assert_eq!(sp.s.pbyte(6),dest);assert_eq!(sp.s.pbyte(0xFC),62);assert!(sp.ev.view>=100);assert_eq!(sp.clock.ticks()-128,140);
}

#[test]
fn station_controller_and_scratch_match_native() {
    struct BlockingDelivery;
    impl mt1::space::tick::Hooks for BlockingDelivery {
        fn update_cursor(&mut self, sp: &mut mt1::space::Space) {
            if sp.s.byte(mt1::space::state::KEY)==0 {sp.poll_input();}
            sp.update_software_cursor();
        }
    }
    use mt1::space::{self,stations,state::*,tick,host::{StationHost,TOP_FRAME}};
    for station in [10,13,14,15,17] {
        let Some(mut native)=space(Box::new(Native::keys([27]))) else {return};let mut play=space(data([id("back")])).unwrap();
        native.hooks=Some(Box::new(BlockingDelivery));
        let run=|sp:&mut space::Space| {
            sp.s.set_word(PREVIOUS_STATION,station);sp.s.set_word(ENCOUNTER,-1);sp.s.set_word(THRUST,0);sp.s.set_pbyte(0xC0,0);
            for i in 0..15 {sp.s.set_word(tick::ACTIVE+2*i,-1);}
            let mut h=StationHost::new(sp,vec![0;65536],TOP_FRAME);
            stations::invoke(&mut h,"dispatch_ship_stations",&[],TOP_FRAME).unwrap().unwrap();h.scratch
        };
        assert_eq!(run(&mut native),run(&mut play));assert_eq!(space::play::hash(&mut native),space::play::hash(&mut play));
    }
}


#[test]
fn flight_native_scheduled_irqs_and_data_have_equal_final_hash() {
    use mt1::space::{Space,state::*,tick};
    struct FlightDelivery {polls:usize, events:VecDeque<Event>}
    impl tick::Hooks for FlightDelivery {
        fn update_cursor(&mut self,sp:&mut Space) {
            self.polls+=1;assert!(self.polls<500,"native flight stalled");
            if self.polls%10==0 && sp.s.byte(KEY)==0 {
                loop {match self.events.pop_front().expect("native flight script exhausted") {
                    Event::Idle=>{sp.clock.wait_tick();break},
                    Event::Key(k)=>{sp.keyboard_isr(mt_engine::io::scan::make_code(k).unwrap());break},
                    Event::KeyUp(k)=>sp.keyboard_isr(mt_engine::io::scan::make_code(k).unwrap()|0x80),
                    _=>unreachable!(),
                }}
            }
            sp.update_software_cursor();
        }
    }
    let Some(mut a)=space(Box::new(Native::keys([]))) else {return};
    let mut b=space(data([id("wait:5"),id("cmd:right"),id("cmd:quit"),id("yes")])).unwrap();
    let mut events=VecDeque::from([Event::Idle;5]);events.extend([Event::Key(keys::RIGHT),Event::KeyUp(keys::RIGHT),Event::Key(81),Event::KeyUp(81),Event::Key(89)]);
    a.hooks=Some(Box::new(FlightDelivery {polls:0,events}));
    for sp in [&mut a,&mut b] {
        sp.s.set_word(CURRENT,-1);sp.s.set_word(ENCOUNTER,-1);sp.s.set_word(SHIP_COUNT,0);sp.s.set_word(mt1::space::system::COUNT,0);
        sp.s.set_word(VIEW,0);sp.s.set_word(DIRECTION,1);sp.s.set_word(VX,0);sp.s.set_word(VY,0);
        for i in 0..15 {sp.s.set_word(tick::ACTIVE+2*i,-1);}
        assert_eq!(sp.in_system_flight(),-1);
    }
    assert_eq!(mt1::space::play::hash(&mut a),mt1::space::play::hash(&mut b));
}

#[test]
fn inventory_mouse_only_cell_matches_native_callback() {
    struct MouseNative(Option<mt_engine::io::MouseEvent>);
    impl Input for MouseNative {
        fn next_key(&mut self)->Key {panic!("mouse-only script")}
        fn poll(&mut self,_:&mut dyn Clock)->Event {Event::Mouse(self.0.take().unwrap())}
        fn mouse(&mut self)->mt_engine::io::MouseState {
            if self.0.is_some() {mt_engine::io::MouseState::default()} else {mt_engine::io::MouseState {buttons:0,col:21,row:11}}
        }
    }
    let event=mt_engine::io::MouseEvent {event:4,buttons:0,row:11,col:21};
    let mut a=ground(Box::new(MouseNative(Some(event))));let mut b=ground(data([id("inventory:0")]));
    for g in [&mut a,&mut b] {
        let at=ground::state::HOTSPOTS_CHAR;
        for (offset,value) in [(0,10),(2,20),(4,8),(6,8)] {g.mem.set_i16(at+offset,value);}
        g.mem.set_u8(at+8,0x96);assert_eq!(ground::input::character_scan(g,0),0x96);
    }
    assert_eq!(mt1::play::hash(&mut a),mt1::play::hash(&mut b));
}

#[test]
fn zero_stock_cannot_queue_an_uncommittable_purchase() {
    let p=Play::with_service(|p| {
        let d=p.pending().unwrap().clone();assert_eq!(d.kind,Kind::List);
        for qty in [0,1] {assert!(p.choose(Choice::new(&d.id,Pick::Row {row:"list:11571/0".into(),qty})).is_err());}
        p.choose(Choice::new(&d.id,"exit")).unwrap();
    });
    let mut g=ground(Box::new(p));g.mem.set_i16(0x6E0A,2);
    assert_eq!(shop_menu::run(&mut g,shop_menu::Menu {title:"Sold out",names:&["Item".into()],prices:&[5],quantities:&mut [0],limits:&[0],rows:1,
        select_only:0,money:100,max_load:0,weights:None,kind:0,sign:1,colors:None}),0);
}


#[test]
fn selling_a_whole_item_does_not_require_the_seller_to_afford_its_price() {
    pair(&[13],vec![Pick::Row {row:"list:11571/0".into(),qty:1}],|g| {
        shop_menu::run(g,shop_menu::Menu {title:"Sell",names:&["Weapon".into()],prices:&[5000],quantities:&mut [255],limits:&[0],rows:1,
            select_only:2,money:1,max_load:0,weights:None,kind:0,sign:1,colors:None}).to_string()
    });
}


#[test]
fn station_rectangle_selector_uses_native_navigation_and_scratch() {
    use mt1::space::{stations::{self,Host,STACK},host::{StationHost,TOP_FRAME},tick,state::*};
    struct BlockingDelivery;
    impl tick::Hooks for BlockingDelivery {
        fn update_cursor(&mut self,sp:&mut mt1::space::Space) {
            if sp.s.byte(KEY)==0 {sp.poll_input();}sp.update_software_cursor();
        }
    }
    for i in [0,7,11] {
        let mut keys=if i<10 {vec![keys::F1+i as u16]} else {vec![keys::DOWN;i]};if i>=10 {keys.push(13)}
        let Some(mut a)=space(Box::new(Native::keys(keys))) else {return};let mut b=space(data([id(&format!("menu:1c1c/{i}"))])).unwrap();
        a.hooks=Some(Box::new(BlockingDelivery));
        let run=|sp:&mut mt1::space::Space| {
            sp.s.set_word(ENCOUNTER,-1);sp.s.set_word(STATION_ACTIVE,0);
            for job in 0..15 {sp.s.set_word(tick::ACTIVE+2*job,-1);}
            let mut h=StationHost::new(sp,vec![0;65536],TOP_FRAME);
            for row in 0..12 {for (j,value) in [10+12*row,20,10,40].into_iter().enumerate() {h.write(0xE000+8*row+2*j as i32,STACK,value as u32,2);}}
            let value=stations::invoke(&mut h,"station_menu",&[0xE000,STACK,12,0,6,8,0,0,1],TOP_FRAME).unwrap().unwrap().signed();
            assert_eq!(value,i as i32);h.scratch
        };
        assert_eq!(run(&mut a),run(&mut b));assert_eq!(mt1::space::play::hash(&mut a),mt1::space::play::hash(&mut b));
    }
}


#[test]
fn named_bill_hash_covers_uncommitted_caller_locals() {
    let capture=|money:u32| {
        struct Capture(Rc<RefCell<String>>);
        impl Input for Capture {
            fn next_key(&mut self)->Key {13} fn wants_decision(&self)->bool {true}
            fn decide(&mut self,d:&Decision) {let mut p=Play::new();p.decide(d);*self.0.borrow_mut()=p.pending().unwrap().state.clone();}
        }
        let out=Rc::new(RefCell::new(String::new()));let mut g=ground(Box::new(Capture(out.clone())));
        mt1::play_prompts::named_state(&mut g,"bill",&[("pay","Pay",80),("back","Cancel",27)],&money.to_le_bytes(),&[]);
        let hash=out.borrow().clone();hash
    };
    assert_eq!(capture(10),capture(10));assert_ne!(capture(10),capture(11));
}


#[test]
fn quantity_limits_respect_native_signed_balance_overflow() {
    for (money,price,sign) in [(u32::MAX,1,1),(i32::MAX as u32-1,2,-1)] {
        let p=Play::with_service(|p| {
            let d=p.pending().unwrap().clone();assert_eq!(d.list.as_ref().unwrap().rows[0].limit,0);
            assert!(p.choose(Choice::new(&d.id,Pick::Row {row:"list:11571/0".into(),qty:1})).is_err());
            p.choose(Choice::new(&d.id,"exit")).unwrap();
        });
        let mut g=ground(Box::new(p));g.mem.set_i16(0x6E0A,2);
        assert_eq!(shop_menu::run(&mut g,shop_menu::Menu {title:"Overflow",names:&["Item".into()],prices:&[price],quantities:&mut [0],limits:&[5],rows:1,
            select_only:0,money,max_load:0,weights:None,kind:0,sign,colors:None}),0);
    }
}

#[test]
fn boarding_countdown_advances_without_consuming_a_choice() {
    use mt1::space::{self,state::*,tick};
    struct BoundedCursor;
    impl tick::Hooks for BoundedCursor {
        fn update_cursor(&mut self,sp:&mut space::Space) {
            assert!(sp.ev.cursor<=400,"boarding clock stalled");
            sp.poll_input();sp.update_software_cursor();
        }
    }
    let Some(mut native)=space(Box::new(Native::keys([]))) else {return};
    let mut play=space(Box::new(Play::with_service(|_|panic!("countdown consumed a choice")))).unwrap();
    for sp in [&mut native,&mut play] {
        sp.hooks=Some(Box::new(BoundedCursor));sp.s.set_dword(tick::SNAPSHOT,123);sp.s.set_word(0x1CB,0);
        assert_eq!(sp.update_encounter_countdown(0,false),0);
        assert_eq!(sp.clock.ticks(),447);assert_eq!(sp.s.byte(KEY),0);assert_eq!(sp.s.byte(PENDING_KEY),0);
    }
    assert_eq!(space::play::hash(&mut native),space::play::hash(&mut play));
}

#[test]
fn data_space_wait_reaches_the_native_sentinel_deadline_without_representing_frames() {
    use mt1::space::tick;
    struct WaitOnly { frames: Rc<RefCell<usize>> }
    impl Input for WaitOnly {
        fn data_mode(&self) -> bool { true }
        fn next_key(&mut self) -> Key { panic!("native timed wait consumed a key") }
        fn present(&mut self, _: &[u32]) {
            *self.frames.borrow_mut() += 1;
            assert!(*self.frames.borrow() < 16, "native timed wait repeatedly renders unchanged frames");
        }
        fn poll(&mut self, _: &mut dyn Clock) -> Event { panic!("native timed wait consumed player input") }
    }
    let frames=Rc::new(RefCell::new(0));
    let Some(mut sp)=space(Box::new(WaitOnly {frames:frames.clone()})) else {return};
    sp.clock=Box::new(Counter::at(2028));
    sp.s.set_dword(tick::SNAPSHOT,65535);
    *frames.borrow_mut()=0;
    sp.s.set_byte(mt1::space::state::KEY,0x14);
    sp.poll_input();
    assert_eq!(*frames.borrow(),0,"ignored ship-task poll presented a frame");
    assert_eq!(sp.clock.ticks(),2028);
    assert_eq!(sp.s.byte(mt1::space::state::KEY),0x14);
    sp.s.set_byte(mt1::space::state::KEY,0);
    let cursors=sp.ev.cursor;
    assert_eq!(sp.wait_with_input(17),None);
    assert_eq!(sp.clock.ticks(),65552);
    assert_eq!(sp.ev.cursor-cursors,63524);
    assert_eq!(*frames.borrow(),0);
    assert_eq!(sp.s.dword(tick::SNAPSHOT),65535);
}

#[test]
fn space_time_grant_presents_at_the_decision_not_at_every_granted_poll() {
    struct Counted { play: Play, frames: Rc<RefCell<usize>> }
    impl Input for Counted {
        fn data_mode(&self)->bool {true}
        fn wants_decision(&self)->bool {self.play.wants_decision()}
        fn decide(&mut self,d:&Decision) {self.play.decide(d)}
        fn next_key(&mut self)->Key {self.play.next_key()}
        fn key_pending(&mut self)->bool {self.play.key_pending()}
        fn poll(&mut self,c:&mut dyn Clock)->Event {self.play.poll(c)}
        fn present(&mut self,rgb:&[u32]) {*self.frames.borrow_mut()+=1;self.play.present(rgb)}
    }
    let frames=Rc::new(RefCell::new(0));
    let play=Play::scripted([Choice::new("d:1","wait:2")]);
    let Some(mut sp)=space(Box::new(Counted {play,frames:frames.clone()})) else {return};
    *frames.borrow_mut()=0;
    mt1::space::play::flight(&mut sp);
    assert_eq!(*frames.borrow(),1,"a pending decision must have its current frame");
    mt1::space::play::flight(&mut sp);
    assert_eq!(*frames.borrow(),1,"a time grant must not repeatedly present frames without a decision");
    assert_eq!(sp.clock.ticks(),125,"both granted ticks still elapse");
}

#[test]
fn enemy_flash_preserves_pending_station_decision() {
    use mt1::space::{self,state::*,tick};
    let calls=Rc::new(RefCell::new(0));let answered=calls.clone();
    let input=Play::with_service(move |p| {*answered.borrow_mut()+=1;let d=p.pending().unwrap().id.clone();p.choose(Choice::new(d,"back")).unwrap();});
    let Some(mut play)=space(Box::new(input)) else {return};
    let mut native=space(Box::new(Native::keys([]))).unwrap();
    space::play::station(&mut play,"121C");
    for sp in [&mut native,&mut play] {
        sp.s.set_dword(tick::SNAPSHOT,123);
        sp.resolve_enemy_shot(9,100,100);
        assert_eq!(sp.clock.ticks(),126,"enemy flash must wait all three ticks");
        assert_eq!((sp.s.byte(KEY),sp.s.byte(PENDING_KEY)),(0,0));
    }
    assert_eq!(*calls.borrow(),0,"timed wait answered the station decision");
    assert_eq!(space::play::hash(&mut native),space::play::hash(&mut play));
    assert_eq!(play.translate_station_key(),27);assert_eq!(*calls.borrow(),1);assert_eq!(play.clock.ticks(),126);
}

#[test]
fn busy_wait_exact_ticks_include_zero_negative_and_wrapping_deadlines() {
    for (start,n,elapsed) in [(123,0,0),(123,-1,0),(123,5,5),(u32::MAX-1,3,0),(u32::MAX-2,2,2)] {
        let Some(mut sp)=space(data([])) else {return};sp.clock=Box::new(Counter::at(start));
        sp.wait_ticks(n);assert_eq!(sp.clock.ticks(),start.wrapping_add(elapsed),"start={start} n={n}");
        let now=sp.clock.ticks();assert_eq!(sp.ticks(),now);assert_eq!(sp.ticks(),now,"ordinary reads must be observational");
    }
}

#[test]
fn character_context_tracks_open_record_without_changing_ground_selection() {
    for (open,expected) in [(0,vec![Some(0),Some(1)]),(2,vec![Some(2),Some(1)])] {
        let seen=Rc::new(RefCell::new(vec![]));let observed=seen.clone();
        let input=Play::with_service(move |p| {
            let d=p.pending().unwrap().clone();observed.borrow_mut().push(d.context.who);
            assert!(p.sequence()<=2);p.choose(Choice::new(d.id,if p.sequence()==1 {"cmd:char/1"} else {"back"})).unwrap();
        });
        let mut g=ground(Box::new(input));let mut native=ground(Box::new(Native::keys([keys::F1+1,keys::ESC])));
        for g in [&mut g,&mut native] {for i in 0..5 {g.mem.set_party_u8(0x127+256*i,10);}}
        let selected=g.mem.selected_char();ground::commands_character::run(&mut g,open);
        assert_eq!(*seen.borrow(),expected);assert_eq!(g.mem.selected_char(),selected);
        ground::commands_character::run(&mut native,open);assert_eq!(mt1::play::hash(&mut native),mt1::play::hash(&mut g));
    }
}

#[test]
fn scheduled_enemy_flash_defers_choice_until_its_native_wait_finishes() {
    use mt1::space::{self,state::*,tick};
    struct Shot {inside:Rc<RefCell<bool>>,fired:Rc<RefCell<bool>>}
    impl tick::Hooks for Shot {
        fn redraw_tactical(&mut self,sp:&mut space::Space,_:i16)->Option<i16> {
            *self.inside.borrow_mut()=true;
            sp.resolve_enemy_shot(9,100,100);
            assert_eq!(sp.clock.ticks(),126);assert_eq!((sp.s.byte(KEY),sp.s.byte(PENDING_KEY)),(0,0));
            *self.inside.borrow_mut()=false;*self.fired.borrow_mut()=true;Some(0)
        }
    }
    let inside=Rc::new(RefCell::new(false));let fired=Rc::new(RefCell::new(false));let guard=inside.clone();
    let input=Play::with_service(move |p| {
        assert!(!*guard.borrow(),"scheduler's enemy flash consumed the station choice");
        let d=p.pending().unwrap().id.clone();p.choose(Choice::new(d,"back")).unwrap();
    });
    let Some(mut sp)=space(Box::new(input)) else {return};
    sp.s.set_word(STATION_ACTIVE,1);sp.s.set_word(STATION,10);sp.s.set_word(TACTICAL_DRAWN,0);sp.s.set_word(ENCOUNTER,1);
    for i in 0..10 {sp.s.set_word(tick::ACTIVE+2*i,-1);}
    sp.s.set_dword(tick::SNAPSHOT,123);sp.hooks=Some(Box::new(Shot {inside,fired:fired.clone()}));
    space::play::station(&mut sp,"121C");sp.run_tasks(5);
    assert!(*fired.borrow());assert_eq!(sp.clock.ticks(),126);assert_eq!(sp.translate_station_key(),27);
}

#[test]
fn time_only_wait_preserves_queued_route_and_native_key_latches() {
    use mt1::space::{state::*,tick};
    let mut p=Play::new();p.decide(&mt1::play_prompts::keys_decision("s","4707",Kind::Menu,&[("back","Back",keys::ESC)]));
    p.choose(Choice::new("d:1","back")).unwrap();
    let Some(mut sp)=space(Box::new(p)) else {return};sp.s.set_dword(tick::SNAPSHOT,123);
    sp.s.set_byte(KEY,0x48);sp.s.set_byte(PENDING_KEY,0x1C);
    assert_eq!(sp.wait_with_input(3),None);assert_eq!(sp.clock.ticks(),126);
    assert_eq!((sp.s.byte(KEY),sp.s.byte(PENDING_KEY)),(0x48,0x1C));
    sp.s.set_byte(KEY,0);sp.s.set_byte(PENDING_KEY,0);
    assert_eq!(sp.translate_station_key(),27);assert_eq!(sp.clock.ticks(),126);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
