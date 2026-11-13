mod oracle_env;
use mt1::ground::{self,Ground};
use mt_engine::{io::{Dir,Files,Harness,script},sim::tick::Counter};
use std::{path::PathBuf,process::Command};

#[test]
fn game_init_full_vram() {
    let Some((oracle,files))=oracle_env::both() else {return};
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out=root.join("target/m4a-fixtures/game-init");
    let result=Command::new("python").arg("-B").arg(root.join("tools/run_watch.py")).args(["--seconds","120","--mib","1536","--","python","-B"])
        .arg(root.join("tools/m4a_dump_game_init.py")).arg(oracle).arg(&out).output().expect("Python required with oracle env");
    assert!(result.status.success(),"{}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    let files=PathBuf::from(files);let dat=Dir(files.clone()).read("T.DAT").unwrap();
    for case in 0..4 {
        let dir=out.join(case.to_string());let read=|n:&str|std::fs::read(dir.join(n)).unwrap();
        let lines=script::lines(&std::fs::read_to_string(dir.join("script.txt")).unwrap()).unwrap();
        let mut input=Harness::lines(lines,dir.join("dump"),true);input.max_polls=10000;
        let mut g=Ground::new(&dat,read("party.bin"),Box::new(input),Box::new(Counter::default()),Box::new(Dir(files.clone()))).unwrap();
        g.harness=true;g.rand.force=Some(1);g.mouse_driver=case&1!=0;
        ground::start(&mut g);g.boot_calibrating=true;
        assert!(ground::render::game_init(&mut g));g.boot_checkpoint("main_loop");
        let actual=g.fb.snapshot();let expected=read("end.bin");
        std::fs::write(dir.join("got.bin"),&actual).unwrap();
        assert_eq!(actual.iter().zip(&expected).position(|(a,b)|a!=b),None,"case {case} full VRAM/registers");
        assert_eq!(g.mem.party(),read("end-party.bin"),"case {case} party");
        let items=read("items.bin");
        assert_eq!(g.mem.bytes(g.mem.pointer(0x6D6D),items.len()),items,"case {case} 819B/8171 item names and record bytes");
        for (name,data) in g.state_buffers() {
            assert_eq!(data,read(&name),"case {case} named game buffer {name}");
        }
        let mut ranges:Vec<_>=g.mem.segs.iter().map(|(&seg,b)|(seg as usize*16,seg as usize*16+b.len())).collect();
        ranges.push((ground::state::Mem::DS as usize*16,ground::state::Mem::DS as usize*16+65536));ranges.sort();
        assert!(ranges.windows(2).all(|r|r[0].1<=r[1].0),"case {case} nonoverlapping TRACE entry ranges");
        let expected=read("end-ds.bin");
        for (off,n) in [(0x6D2A,1),(0x6248,1),(0x6B72,1),(0x6418,1),(0x6B63,1),(0x6D20,1),(0x6B76,1),
            (0x6D0B,1),(0x5E1F,2),(0x6B68,1),(0x6B92,4),(0x6B6C,2),(0x63F5,2),(0x0110,16),
            (0x4A98,2),(0x4B18,3),(0x6F00,76),(0x4CDE,8),(0x6FC6,10),(0x6B52,4),(0x6FC0,6)] {
            assert_eq!(&g.mem.ds[off..off+n],&expected[off..off+n],"case {case} DS:{off:04X}");
        }
        println!("PASS game-init-full-VRAM 078B2 case={case}");
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
