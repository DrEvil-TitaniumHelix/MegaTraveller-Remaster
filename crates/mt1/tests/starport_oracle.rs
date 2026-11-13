mod oracle_env;
use mt1::ground::{starport,state::{Ground,Mem}};
use mt_engine::{io::{Dir,Input},sim::tick::Counter};
use std::{path::PathBuf,process::Command};
use std::{cell::RefCell,rc::Rc};
struct Capture {frame:Rc<RefCell<Vec<u8>>>,first:bool}
impl Input for Capture {
    fn next_key(&mut self)->u16 {27}
    fn poll(&mut self,_:&mut dyn mt_engine::sim::tick::Clock)->mt_engine::io::Event {
        if self.first {self.first=false;mt_engine::io::Event::Boundary} else {mt_engine::io::Event::Key(27)}
    }
    fn step(&mut self,s:&mt_engine::io::dump::Step) {let mut out=self.frame.borrow_mut();out.extend_from_slice(s.planes);out.extend_from_slice(s.regs)}
}
struct NoInput;
impl Input for NoInput {fn next_key(&mut self)->u16 {27}}
#[test]
fn starport_frames_and_greeting_rng() {
    let Some((oracle,files))=oracle_env::both() else {return};
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");let out=root.join("target/m5-panel-frames");
    let result=Command::new("python").args(["-B"]).arg(root.join("tools/run_watch.py")).args(["--seconds","120","--mib","1024","--","python","-B"]).arg(root.join("tools/m5_dump_panel.py")).arg(&oracle).arg(&files).arg(&out).output().expect("Python needed for oracle");
    assert!(result.status.success(),"{}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    let segments:Vec<u16>=std::fs::read_to_string(out.join("segments.txt")).unwrap().split_whitespace().map(|s|s.parse().unwrap()).collect();
    let names=std::fs::read_to_string(out.join("manifest.txt")).unwrap();assert_eq!(names.lines().count(),8);
    for name in names.lines() {
        let dir=out.join(name);let read=|f:&str|std::fs::read(dir.join(f)).unwrap();let memory=read("memory.bin");
        let ds:usize=std::fs::read_to_string(dir.join("ds.txt")).unwrap().parse().unwrap();
        let mut g=Ground::from_ds(memory[ds*16..ds*16+65536].to_vec(),memory[0x90000..0x90000+Mem::PARTY_SIZE].to_vec(),Box::new(NoInput),Box::new(Counter::default()),Box::new(Dir(PathBuf::from(&files))));
        for &seg in &segments {if seg!=Mem::DS&&seg!=Mem::PARTY_SEG {g.mem.segs.insert(seg,memory[seg as usize*16..(seg as usize*16+65536).min(memory.len())].to_vec());}}
        assert!(g.fb.restore(&read("start.bin")));g.rand.lcg.seed=g.mem.u32(0x5C20);starport::draw(&mut g);
        let expected=read("end.bin");let got=g.fb.snapshot();
        if expected!=got {std::fs::write(dir.join("got.bin"),&got).unwrap();let at=expected.iter().zip(&got).position(|(a,b)|a!=b);panic!("{name}: first differing video byte {at:?}")}
        assert_eq!(g.mem.u32(0x5C20).to_le_bytes().as_slice(),read("rng.bin"),"{name} greeting RNG");
    }
    let dir=out.join("ammo-label");let read=|f:&str|std::fs::read(dir.join(f)).unwrap();let memory=read("memory.bin");
    let ds:usize=std::fs::read_to_string(dir.join("ds.txt")).unwrap().parse().unwrap();
    let frame=Rc::new(RefCell::new(Vec::new()));
    let mut g=Ground::from_ds(memory[ds*16..ds*16+65536].to_vec(),memory[0x90000..0x90000+Mem::PARTY_SIZE].to_vec(),Box::new(Capture{frame:frame.clone(),first:true}),Box::new(Counter::default()),Box::new(Dir(PathBuf::from(&files))));
    for &seg in &segments {if seg!=Mem::DS&&seg!=Mem::PARTY_SEG {g.mem.segs.insert(seg,memory[seg as usize*16..(seg as usize*16+65536).min(memory.len())].to_vec());}}
    assert!(g.fb.restore(&read("start.bin")));assert_eq!(starport::items::ammo(&mut g,-2,0,30000,0),1);
    let expected=read("end.bin");let got=frame.borrow();
    if expected!=*got {std::fs::write(dir.join("got.bin"),&*got).unwrap();let at=expected.iter().zip(got.iter()).position(|(a,b)|a!=b);panic!("ammo label: first differing video byte {at:?}")}
}

#[test]
fn zz_skip_report() { oracle_env::report() }
