mod oracle_env;
use mt1::ground::{Ground, state::{ds_image, Mem}, input::{self, dialogue, menu}};
use mt_engine::{data::{dict,msg,Variant}, io::{Dir, Event, Input, Key, MouseEvent, MouseState}, sim::tick::{Clock, Counter}};
use std::{collections::VecDeque, path::PathBuf, process::Command};

struct Events { queue: VecDeque<Event>, mouse: MouseState, polls: usize }
impl Events { fn new(events: impl IntoIterator<Item=Event>) -> Self { Self { queue: events.into_iter().collect(), mouse: MouseState::default(), polls: 0 } } }
impl Input for Events {
    fn next_key(&mut self) -> Key { panic!("ground modal called CG next_key") }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        self.polls += 1; assert!(self.polls < 32, "modal exceeded fixture input budget");
        let e = self.queue.pop_front().unwrap_or(Event::Key(27));
        if let Event::Mouse(m) = e { self.mouse = MouseState { col:m.col,row:m.row,buttons:m.buttons } }
        if e == Event::Idle { clock.wait_tick() } e
    }
    fn mouse(&mut self) -> MouseState { self.mouse }
    fn set_mouse_position(&mut self,col:i16,row:i16) { self.mouse.col=col;self.mouse.row=row; }
}
fn host(ds: Vec<u8>, events: impl IntoIterator<Item=Event>, dir: PathBuf) -> Ground {
    Ground::from_ds(ds,vec![0;Mem::PARTY_SIZE],Box::new(Events::new(events)),Box::new(Counter::default()),Box::new(Dir(dir)))
}
fn hex(s:&str)->Vec<u8>{ (0..s.len()/2).map(|i|u8::from_str_radix(&s[2*i..2*i+2],16).unwrap()).collect() }

#[test]
fn ground_input_real_oracle() {
    let Some((oracle,game))=oracle_env::both() else {return };
    let game=PathBuf::from(game); let oracle=PathBuf::from(oracle);
    let ds=ds_image(&std::fs::read(game.join("T.DAT")).unwrap()).unwrap();
    let msgs=msg::decode(Variant::Mt1,&std::fs::read(game.join("TRAV.MSG")).unwrap()).unwrap();
    let dictionary=dict::decode(&std::fs::read(game.join("TRAV.DAT")).unwrap()).unwrap();
    let script=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ground_input_oracle.py");
    let output=Command::new("python").arg("-B").arg(script).arg(&oracle).arg(&game).env("PYTHONDONTWRITEBYTECODE","1").output().expect("PARTIAL: python unavailable");
    assert!(output.status.success(),"PARTIAL: {}",String::from_utf8_lossy(&output.stderr));
    let output=String::from_utf8(output.stdout).unwrap();
    let mut g=host(ds.clone(),[],game.clone());
    let quit=menu::Menu::read(&g.mem,Mem::ds(0x45B3)).unwrap();
    assert_eq!(g.mem.cstr(quit.labels[0]),"Quit","T DS far pointers must be MZ relocated");
    assert_eq!(g.mem.cstr(quit.hotkeys),"QCE");
    for (scan,key,want) in [(1,b'q',input::Flow::Quit),(1,b'c',input::Flow::Continue),(0x22,b'q',input::Flow::Quit)] {
        let mut h=host(ds.clone(),[Event::Key(key as u16)],PathBuf::new());
        assert_eq!(input::dispatch(&mut h,scan,0,0),want);
    }
    let mut h=host(ds.clone(),[Event::Mouse(MouseEvent{event:16,buttons:0,row:175,col:210})],PathBuf::new());
    assert_eq!(input::dispatch(&mut h,1,0,0),input::Flow::Continue);
    let mut h=host(ds.clone(),[Event::Key(b's' as u16)],PathBuf::new());
    let sound=h.mem.sound_enabled();input::dispatch(&mut h,0x22,0,0);assert_eq!(h.mem.sound_enabled(),sound^1);
    for (ptr,key,want) in [(0x44D2,b'b',0),(0x44D2,b'r',1),(0x472A,b'l',0),(0x472A,b's',1)] {
        let mut h=host(ds.clone(),[Event::Key(key as u16)],PathBuf::new());
        assert_eq!(menu::run_menu(&mut h,Mem::ds(ptr),0,0),want);
    }
    for (key,want) in [(b't',0x06CD),(b'u',0x0C61)] {
        let mut h=host(ds.clone(),[Event::Key(key as u16)],PathBuf::new());let mut calls=vec![];
        menu::run_menu_with(&mut h,Mem::ds(0x4568),0,0,&mut |_,callback,_|{calls.push(callback);0});assert_eq!(calls,[want]);
    }
    let mut h=host(ds.clone(),[],game.clone());h.harness=true;dialogue::set_text_box(&mut h,3);
    let expected=dialogue::decode_dialogue(&msgs.msgs[86],&dictionary.words,189,86,0).unwrap();
    dialogue::say_message(&mut h,86,true).unwrap();
    assert!(h.log.is_empty(),"19471 drawing is not a 05C3 message-log entry");
    assert_eq!(h.frames.iter().map(|(name,_)|name.as_str()).collect::<Vec<_>>(),
        ["frame_0000_dismiss_message.bin","frame_0001_print_message.bin","frame_0002_get_message.bin","frame_0003_say_message.bin"]);
    assert_eq!(h.mem.u8(0x6E2A),expected.lines.len() as u8);assert_eq!(h.mem.message_deadline(),180);assert_eq!(h.mem.u8(0x713F)&1,0);
    let message=vec![b'a';300];input::ui::message(&mut h,&message);
    assert_eq!(h.log.iter().rev().find(|t| matches!(t, mt_engine::io::dump::Text::Message(_))),Some(&mt_engine::io::dump::Text::Message("a".repeat(300))));
    h.harness=false;h.log.clear();dialogue::get_message(&mut h,86,true,0).unwrap();
    assert!(h.log.is_empty(),"window dialogue must not accumulate harness text logs");
    let mut counts=[0usize;5];let mut finished=false;
    for line in output.lines() {
        let p:Vec<_>=line.split(' ').collect();
        match p[0] {
            "K"=>{let code=p[1].parse().unwrap();assert_eq!(input::translated_key(&g.mem,code),p[2].parse::<u16>().unwrap(),"{line}");counts[0]+=1;}
            "I"=>{assert_eq!(input::inventory_action(p[2].parse().unwrap(),p[1].parse().unwrap()),(p[3],p[4].parse().unwrap()),"{line}");counts[4]+=1;}
            "H"=>{
                let bytes=hex(p[5]);g.mem.write_bytes(Mem::ds(0x120),&bytes);g.mem.set_u8(input::MOUSE_PENDING,0);
                input::mouse_callback(&mut g,MouseEvent{event:4,buttons:p[4].parse().unwrap(),row:p[2].parse().unwrap(),col:p[3].parse().unwrap()});
                input::special_key(&mut g,0x120,bytes.len()/9);
                assert_eq!((g.mem.key(),g.mem.u8(input::MOUSE_PENDING),g.mem.u16(input::CALLBACK)),(p[6].parse().unwrap(),p[7].parse().unwrap(),p[8].parse().unwrap()),"{line}");counts[1]+=1;
            }
            "R"=>{
                let n:usize=p[1].parse().unwrap();let code:u8=p[2].parse().unwrap();
                let mut h=host(ds.clone(),[],PathBuf::new());
                h.mem.set_i16(input::CALLBACK+4,171);h.mem.set_i16(input::CALLBACK+6,200);
                let rects=menu::rectangles(&h,Mem::ds(0x480B),n);
                let mut codes=[code,1].into_iter();
                let answer=menu::select_with_poll(&mut h,&rects,-2,6,7,Some(b"QCT123"),&mut |g|g.mem.set_key(codes.next().expect("selector input budget")));
                assert_eq!(answer,p[3].parse::<i16>().unwrap(),"{line}");counts[2]+=1;
            }
            "D"=>{
                let i:usize=p[1].parse().unwrap();let d=dialogue::decode_dialogue(&msgs.msgs[i],&dictionary.words,p[2].parse().unwrap(),i as i16,p[3].parse().unwrap()).unwrap();
                let lines=if p[4]=="-"{vec![]}else{p[4].split(',').map(|s|if s=="_"{vec![]}else{hex(s)}).collect()};
                assert_eq!(d.lines,lines,"{line}");assert_eq!(d.short,if p[5]=="-"{None}else{Some(hex(p[5]))},"{line}");counts[3]+=1;
            }
            "PASS"=>{assert_eq!(p[1].parse::<usize>().unwrap(),msgs.msgs.len());finished=true;}
            _=>panic!("unexpected fixture {line}"),
        }
    }
    assert!(finished,"PARTIAL: no final oracle marker");
    assert_eq!(counts,[256,1400,768,msgs.msgs.len()*4,2048]);
    eprintln!("PASS ground input oracle: keys={} hotspots={} selectors={} dialogue={} inventory={}",counts[0],counts[1],counts[2],counts[3],counts[4]);
}

#[test]
fn ground_input_mouse_wait_scroll_and_irq() {
    for (row,result) in [(9,5),(10,5000),(190,5000),(191,6)] {
        let mut g=host(vec![],[Event::Mouse(MouseEvent{event:4,buttons:0,col:30,row})],PathBuf::new());g.mem.set_u8(0x6E2D,1);
        assert_eq!(input::menu_getkey(&mut g),result);assert_eq!(g.mem.i16(0x6E0C),if result==5000{row}else{-1});
        assert_eq!(g.mem.key(),0x6E);
    }
    let mut g=host(vec![],[Event::Mouse(MouseEvent{event:16,buttons:0,col:30,row:25})],PathBuf::new());
    assert_eq!(input::wait_scan(&mut g),1);assert_eq!(g.mem.u16(input::CALLBACK),16);
    g.mem.set_key(0x48);input::keyboard_scan(&mut g,0x46);assert_eq!(g.mem.key(),0x48);
    input::keyboard_scan(&mut g,0x1D);assert_eq!(g.mem.key(),0x48);
    input::keyboard_scan(&mut g,0x9E);assert_eq!(g.mem.key(),0);
    input::keyboard_scan(&mut g,0x18);input::keyboard_scan(&mut g,0xC8);assert_eq!(g.mem.key(),0x18);
    input::keyboard_scan(&mut g,0x80);assert_eq!(g.mem.key(),0x80);
    input::mouse_callback(&mut g,MouseEvent{event:0x14,buttons:0,col:2,row:3});
    assert_eq!(g.mem.key(),0x6E,"combined release bits are not the exact right-release branch");
}

#[test]
fn dialogue_literal_overflow_and_invalid_data() {
    let words=vec![b"one".to_vec(),b"two".to_vec()];
    assert_eq!(dialogue::decode_dialogue(&[0,-33,1,-1],&words,48,5,0).unwrap().lines,[b"\"one! ".to_vec(),b" two".to_vec(),b"<invalid> \"".to_vec()]);
    assert_eq!(dialogue::decode_dialogue(&[0],&words,0,0,0).unwrap().lines,[vec![],b"one".to_vec()]);
    assert_eq!(dialogue::decode_dialogue(&[0],&words,0,5,1).unwrap().short,Some(b"\"...\"".to_vec()));
    assert!(dialogue::decode_dialogue(&[2],&words,192,0,0).is_err());
    assert!(dialogue::decode_dialogue(&[i16::MIN],&words,i16::MIN,0,0).is_ok());
    let mut g=host(vec![],[],PathBuf::new()); assert!(dialogue::get_message(&mut g,0,true,0).is_err());
}

#[test]
fn ground_mouse_clip_repositions_and_preserves_buttons() {
    for (enabled,previous,col,row,expected) in [
        (1,46,192,47,(192,46)), (1,47,192,47,(193,47)),
        (1,-1,20,100,(20,46)), (-1,100,20,100,(193,100)),
        (1,46,193,100,(193,100)), (1,47,192,46,(192,46)),
        (0,0,20,100,(20,100)), (1,100,-1,-1,(-1,-1)),
    ] {
        let event=MouseEvent{event:4,buttons:3,col,row};
        let mut g=host(vec![],[Event::Mouse(event)],PathBuf::new());
        g.mem.set_i16(0x7012,enabled);g.mem.set_i16(0x6FFE,previous);g.poll();
        assert_eq!((g.mem.i16(0x6FFC),g.mem.i16(0x6FFE)),expected);
        let live=g.input.mouse();assert_eq!((live.col,live.row),expected,"driver must be repositioned");
        assert_eq!(g.mem.u16(0x6FF8),3);
        assert_eq!((g.mem.i16(input::CALLBACK+6),g.mem.i16(input::CALLBACK+4)),(col,row),"saved callback is independent");
    }
}

#[test]
fn ground_dispatch_orders_space_targeting_and_unbound_keys() {
    let mut g=host(vec![],[],PathBuf::new());g.mem.trace.on=true;
    g.mem.set_key(15);input::dispatch(&mut g,15,100,0);assert_eq!(g.mem.key(),15);
    input::dispatch(&mut g,0x39,100,1);assert!(g.mem.trace.done.is_empty());
    input::dispatch(&mut g,0x18,100,0);
    assert_eq!(g.mem.trace.done[0].name,"party_order");assert_eq!(g.mem.trace.done[0].args,[1]);
    g.mem.set_combat(1);g.mem.set_orders(1);g.mem.set_leader(2);
    let w=Mem::walker(2);for off in [16,18,20]{g.mem.write(w.add(off),22,2)}
    input::dispatch(&mut g,1,100,0);for off in [16,18,20]{assert_eq!(g.mem.read(w.add(off),2),0)}
    assert_eq!(g.mem.trace.done.len(),1,"escape cancels queued destination before leaving orders");
    input::dispatch(&mut g,1,100,0);assert_eq!(g.mem.trace.done[1].args,[0]);
    input::dispatch(&mut g,0x4E,100,0);assert_eq!(g.mem.read(w.add(2),1),1);
    input::dispatch(&mut g,0x0D,100,0);assert_eq!(g.mem.read(w.add(2),1),0);
    g.mem.set_orders(0);g.mem.set_x(10);g.mem.set_y(12);g.mem.write(w.add(12),20,2);g.mem.write(w.add(14),22,2);
    input::dispatch(&mut g,0x21,100,0);assert_eq!((g.mem.individual(),g.mem.x(),g.mem.y(),g.mem.target_deadline()),(1,20,22,109));
    assert_eq!((g.mem.read(w.add(8),2),g.mem.read(w.add(10),2)),(10,12));
    g.mem.write(w.add(3),2,1);let calls=g.mem.trace.done.len();
    input::dispatch(&mut g,0x1C,100,0);
    assert_eq!((g.mem.individual(),g.mem.x(),g.mem.y(),g.mem.read(w.add(3),1)),(0,10,12,0));
    assert_eq!(g.mem.trace.done[calls].name,"sync_leader_pos");
    input::dispatch(&mut g,0x1C,100,0);assert_eq!((g.mem.individual(),g.mem.x(),g.mem.y()),(1,20,22),"the next Enter aims again");
}

#[test]
fn remaining_m4b_command_only_notes_and_consumes_key() {
    let cases=[
        (0x10,"debug status 6923"),
    ];
    for (key,command) in cases {
        assert_eq!(input::m4b_note(key,true),Some(format!("not yet (M4B): {command}").as_str()));
        for (orders,combat) in [(0,0),(1,0),(0,1),(1,1)] {
            let mut g=host(vec![],[],PathBuf::new());g.harness=true;
            g.mem.set_debug(1);g.mem.set_orders(orders);g.mem.set_combat(combat);g.mem.set_cursor_visible(1);g.mem.set_panel_flag(7);g.mem.set_event(12);
            let ds=g.mem.ds.clone();let party=g.mem.party().to_vec();let frame=g.fb.snapshot();
            g.mem.set_key(key);
            assert_eq!(input::dispatch(&mut g,key,100,0),input::Flow::Continue);
            assert_eq!(g.mem.ds,ds,"{command}");assert_eq!(g.mem.party(),party);
            assert_eq!(g.fb.snapshot(),frame);assert!(g.frames.is_empty() && g.log.is_empty() && g.mem.trace.done.is_empty());assert_eq!(g.now(),0);
        }
    }
    for key in [0x11,0x10,0x43,0x44] { assert_eq!(input::m4b_note(key,false),None) }
}

#[test]
fn selector_blink_strict_deadline_and_mouse_miss() {
    use mt_engine::io::mouse::Hotspot;
    for (ticks,frames) in [(9,2),(10,2),(19,2),(20,4)] {
        let mut g=host(vec![],(0..ticks).map(|_|Event::Idle).chain([Event::Key(13)]),PathBuf::new());g.harness=true;
        let rects=[Hotspot{top:20,left:20,height:10,width:40,id:1}];
        assert_eq!(menu::select(&mut g,&rects,0,6,7,None),0);
        assert_eq!(g.frames.len(),frames,"ticks {ticks}");
        for (i,(_,frame)) in g.frames.iter().enumerate() {
            let mut fb=mt_engine::present::fb::Fb::new();assert!(fb.restore(frame));
            assert_eq!(fb.get(0,20,20),if i%2==0{6}else{7},"ticks {ticks}, frame {i}");
        }
    }
    let events=[Event::Mouse(MouseEvent{event:4,buttons:0,col:100,row:100}),Event::Key(0x14D),Event::Key(13)];
    let mut g=host(vec![],events,PathBuf::new());
    let rects=[Hotspot{top:20,left:20,height:10,width:40,id:1},Hotspot{top:40,left:20,height:10,width:40,id:2}];
    assert_eq!(menu::select(&mut g,&rects,0,6,7,None),1);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
