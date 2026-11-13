mod oracle_env;
use mt1::{ground::{self, state::Mem, Ground}, space};
use mt_engine::{io::{Dir, Harness, script, Scripted}, sim::tick::Counter};
use std::{path::PathBuf, process::Command};

#[test]
fn landing_matches_original_mode_b() {
    let Some((oracle, files)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/m6-fixtures/landing");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "120", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("tools/m4a_dump_game_init.py")).arg(oracle).arg(&out).arg("--landing")
        .output().expect("Python required with oracle env");
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let files = PathBuf::from(files);
    for case in 0..4 {
        let dir = out.join(case.to_string());
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        let input = Harness::lines(script::lines(&std::fs::read_to_string(dir.join("script.txt")).unwrap()).unwrap(), dir.join("dump"), true);
        let mut g = Ground::from_ds(vec![], read("party.bin"), Box::new(input), Box::new(Counter::default()), Box::new(Dir(files.clone())));
        g.mem.set_key(0x48); g.mem.set_event(8); g.mem.alloc(0x9200, 32);
        g.mem.segs.insert(0x9100, read("input-items.bin"));
        g.harness = true; g.mouse_driver = case & 1 != 0;
        let address = g.mem.segs[&Mem::PARTY_SEG].as_ptr();
        assert_eq!(space::handoff(&mut g, |party| {
            assert_eq!(party.as_ptr(), address);
            Ok(space::Exit::Land)
        }).unwrap(), space::Exit::Land);
        assert_eq!(g.mem.segs[&Mem::PARTY_SEG].as_ptr(), address);
        std::fs::write(dir.join("got-party.bin"), g.mem.party()).unwrap();
        assert_eq!(g.mem.party().iter().zip(read("end-party.bin")).enumerate().filter_map(|(i, (&a,b))| (a!=b).then_some((i,a,b))).collect::<Vec<_>>(), vec![], "case {case} party (offset, got, ref)");
        let ds = read("end-ds.bin");
        let word = |at| i16::from_le_bytes([ds[at], ds[at+1]]);
        assert_eq!((g.mem.x(), g.mem.y()), (word(0x6B6C), word(0x63F5)), "case {case} landing coordinates");
        std::fs::write(dir.join("got.bin"), g.fb.snapshot()).unwrap();
        assert_eq!(g.fb.snapshot().iter().zip(read("end.bin")).position(|(&a,b)| a!=b), None, "case {case} full VRAM/registers");
        for (name, data) in g.state_buffers() { assert_eq!(data, read(&name), "case {case} {name}"); }
    }
}

#[test]
fn exit_one_keeps_s_changes_without_loading_t() {
    let mut g = Ground::from_ds(vec![], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(mt_engine::io::Overlay::new(None)));
    let address = g.mem.segs[&Mem::PARTY_SEG].as_ptr();
    assert_eq!(space::handoff(&mut g, |party| {
        party[6] = 3; party[7] = 255; party[0xFC] = 61;
        Ok(space::Exit::GameOver)
    }).unwrap(), space::Exit::GameOver);
    assert_eq!(g.mem.segs[&Mem::PARTY_SEG].as_ptr(), address);
    assert_eq!((g.mem.party_u8(6), g.mem.party_u8(7), g.mem.party_u8(0xFC)), (3, 255, 61));
    assert!(!g.landing);
    g.mem.set_event(8);
    ground::walk::world_event(&mut g);
    assert!(g.launch_pending);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
