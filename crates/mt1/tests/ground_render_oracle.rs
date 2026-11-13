mod oracle_env;
#[path = "../../engine/tests/common_m2/mod.rs"]
mod common_m2;
use common_m2::{workspace,parse_calls,Case,check};
use mt1::ground::{render,state::{Ground,Far,Mem}};
use mt_engine::{io::{Input,MouseState,Dir},sim::tick::Counter};
use std::{path::PathBuf,process::Command,cell::Cell,rc::Rc};
struct Mouse(Rc<Cell<MouseState>>);
impl Input for Mouse {fn next_key(&mut self)->u16 {13}fn mouse(&mut self)->MouseState {self.0.get()}}

#[test]
fn complete_ground_frames() {
    let Some((oracle,files))=oracle_env::both() else {return};
    let root=workspace();let out=root.join("target/m4a-fixtures/render");
    let result=Command::new("python").arg("-B").arg(root.join("tools/m4a_watch.py")).arg("60").arg("python").arg("-B").arg(root.join("tools/m4a_dump_render.py")).arg(oracle).arg(&files).arg(&out).output().expect("Python required when oracle env is set");
    let stdout=String::from_utf8_lossy(&result.stdout);
    assert!(result.status.success() && !stdout.contains("PARTIAL"),"fixture export failed:\n{stdout}{}",String::from_utf8_lossy(&result.stderr));
    let segments:Vec<usize>=std::fs::read_to_string(out.join("segments.txt")).unwrap().split_whitespace().map(|s|s.parse().unwrap()).collect();
    let mut dirs:Vec<PathBuf>=std::fs::read_to_string(out.join("manifest.txt")).unwrap().lines().map(|n|out.join(n)).collect();dirs.sort();
    assert_eq!(dirs.len(),134,"real cases dropped");
    let mut failures=vec![];
    for dir in dirs {
        let read=|name:&str|std::fs::read(dir.join(name)).unwrap();
        let name=dir.file_name().unwrap().to_string_lossy().into_owned();
        let look=name.starts_with("main-");mt_engine::present::textlog::set_enabled(look);
        let ds:usize=std::fs::read_to_string(dir.join("ds.txt")).unwrap().parse().unwrap();
        let memory=read("memory.bin");let m=Rc::new(Cell::new(MouseState::default()));
        let mut g=Ground::from_ds(memory[ds*16..ds*16+65536].to_vec(),memory[0x90000..0x90000+Mem::PARTY_SIZE].to_vec(),Box::new(Mouse(m.clone())),Box::new(Counter::default()),Box::new(Dir(PathBuf::from(&files))));
        for &seg in &segments {if seg!=Mem::DS as usize && seg!=0x9000 {g.mem.segs.insert(seg as u16,memory[seg*16..(seg*16+65536).min(memory.len())].to_vec());}}
        let calls=parse_calls(&std::fs::read_to_string(dir.join("calls.json")).unwrap());
        let case=Case {name:name.clone(),dir:dir.clone(),start:read("start.bin"),end:read("end.bin"),calls};assert!(g.fb.restore(&case.start));
        let outcome=std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut blink:Option<render::SelectionBlink>=None;
            for (name,a) in &case.calls {
                let n=|i:usize|a[i].i();
                match name.as_str() {
                    "main"=>render::main_screen(&mut g,n(0) as u16),
                    "redraw"=>render::redraw(&mut g),
                    "message"=>render::message(&mut g,&a[0].hex()),
                    "message_repeat"=>{
                        let count=usize::try_from(n(1)).unwrap();assert!(count<=8_421_505,"fixture byte cap");
                        let mut text=vec![n(0) as u8;count];text.extend_from_slice(&a[2].hex());render::message(&mut g,&text);
                    },
                    "dismiss"=>render::dismiss_message(&mut g,n(0) as i16),
                    "menu"=>render::draw_menu(&mut g,Far::new(n(0) as u16,n(1) as u16)),
                    "present_menu"=>render::present_menu(&mut g),
                    "portrait"=>render::shop_portrait(&mut g,n(0) as i16,n(1) as i16),
                    "intro"=>render::intro_page(&mut g,n(0) as usize),
                    "blink_start"=>blink=Some(render::SelectionBlink::new(&mut g,Mem::ds(n(0) as u16),n(1) as usize,n(2) as usize,n(3) as u8,n(4) as u8)),
                    "blink_poll"=>blink.as_mut().unwrap().poll(&mut g),
                    "blink_choose"=>blink.as_mut().unwrap().choose(&mut g,n(0) as usize),
                    "mouse"=>m.set(MouseState {col:n(0) as i16,row:n(1) as i16,buttons:n(2) as u16}),
                    "ticks"=>g.clock=Box::new(Counter::at(n(0) as u32)),
                    "show"=>render::cursor_on(&mut g),"hide"=>render::cursor_off(&mut g),"track"=>render::cursor_track(&mut g),"flip"=>render::flip(&mut g),
                    _=>panic!("unknown operation {name}")
                }
            }
            check(&case,&g.fb.snapshot());
            if look {let log=mt_engine::present::textlog::frame(&g.fb,(g.mem.u16(0x52F4)&1) as usize);assert!(!log.visible.is_empty(),"HUD text must survive page copies for Look fonts");assert!(!log.overflowed);}
            if let Ok(state)=std::fs::read_to_string(dir.join("state.txt")) {for l in state.lines() {let v:Vec<u32>=l.split_whitespace().map(|s|s.parse().unwrap()).collect();assert_eq!(g.mem.read(Mem::ds(v[0] as u16),v[1] as usize),v[2],"{} DS:{:04x}",case.name,v[0]);}}
        }));
        if outcome.is_err() {std::fs::write(dir.join("got.bin"),g.fb.snapshot()).unwrap();failures.push(name);}
        mt_engine::present::textlog::set_enabled(false);
    }
    assert!(failures.is_empty(),"failed {} cases: {failures:?}",failures.len());
}

#[test]
fn zz_skip_report() { oracle_env::report() }
