mod oracle_env;
use mt1::ground::{state::{Ground,Mem},starport::shop_menu};
use mt_engine::{io::{Dir,Event,Input},sim::tick::{Clock,Counter}};
use std::{path::PathBuf,process::Command};
struct Boundary;
struct QuantityInput(usize);
impl Input for Boundary {
    fn next_key(&mut self)->u16 {std::panic::resume_unwind(Box::new(Boundary))}
    fn poll(&mut self,_:&mut dyn Clock)->Event {std::panic::resume_unwind(Box::new(Boundary))}
}
impl Input for QuantityInput {
    fn next_key(&mut self)->u16 {std::panic::resume_unwind(Box::new(Boundary))}
    fn poll(&mut self,_:&mut dyn Clock)->Event {
        if self.0==0 {std::panic::resume_unwind(Box::new(Boundary))}
        self.0-=1;Event::Key(b'+' as u16)
    }
}
#[test]
fn shop_menu_matches_original_planes_at_input_boundary() {
    let Some((oracle,files))=oracle_env::both() else {return};
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");let out=root.join("target/m5-shop-frames");
    let output=Command::new("python").arg("-B").arg(root.join("tools/run_watch.py")).args(["--seconds","120","--mib","1536","--","python","-B"]).arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ground_shop_frames.py")).arg(oracle).arg(&files).arg(&out).env("PYTHONDONTWRITEBYTECODE","1").output().unwrap();
    assert!(output.status.success(),"{}{}",String::from_utf8_lossy(&output.stdout),String::from_utf8_lossy(&output.stderr));
    let segments:Vec<usize>=std::fs::read_to_string(out.join("segments.txt")).unwrap().split_whitespace().map(|s|s.parse().unwrap()).collect();
    for mode in 0..3 {for remembered in [0,7] {for quantity in 0..3 {
        let dir=out.join(format!("{mode}-{remembered}"));let memory=std::fs::read(dir.join("memory.bin")).unwrap();
        let ds:usize=std::fs::read_to_string(dir.join("ds.txt")).unwrap().parse().unwrap();
        let mut g=Ground::from_ds(memory[ds*16..ds*16+65536].to_vec(),memory[0x90000..0x90000+Mem::PARTY_SIZE].to_vec(),Box::new(QuantityInput(quantity)),Box::new(Counter::default()),Box::new(Dir(PathBuf::from(&files))));
        for &seg in &segments {if seg!=Mem::DS as usize && seg!=0x9000 {g.mem.segs.insert(seg as u16,memory[seg*16..(seg*16+65536).min(memory.len())].to_vec());}}
        assert!(g.fb.restore(&std::fs::read(dir.join("start.bin")).unwrap()));
        let names=["Heading","food ^5$","Fuel ^50$","supplies ^100$","tools ^20$","armor ^30$","weapon ^40$"].map(String::from);let mut quantities=[0;7];
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||shop_menu::run(&mut g,shop_menu::Menu {title:"Supplies for purchase:",names:&names,prices:&[0,5,50,100,20,30,40],quantities:&mut quantities,limits:&[9;7],rows:3,select_only:0,money:1000,max_load:0,weights:None,kind:0,sign:1,colors:Some(&[6,1,2,3,4,5,6])})));
        assert!(result.is_err_and(|e|e.is::<Boundary>()),"did not reach input boundary");
        let got=g.fb.snapshot();let expected=std::fs::read(dir.join(format!("end-{quantity}.bin"))).unwrap();
        if got!=expected {std::fs::write(dir.join(format!("got-{quantity}.bin")),&got).unwrap();}
        assert_eq!(got.iter().zip(&expected).position(|(a,b)|a!=b),None,"mode {mode}, remembered {remembered}, quantity {quantity}: first differing plane byte");
    }}}
}

#[test]
fn zz_skip_report() { oracle_env::report() }
