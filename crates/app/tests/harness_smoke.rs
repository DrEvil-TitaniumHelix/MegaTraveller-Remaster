#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use std::process::Command;

#[test]
fn script_dump_layout() {
    let Some(gf) = oracle_env::gamefiles() else { return };
    let d = std::env::temp_dir().join(format!("mt-harness-smoke-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("keys.txt"), "# smoke\n13\n0x150\n").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_megatraveller")).args(["--faithful", "--script"]).arg(d.join("keys.txt")).arg("--dump").arg(d.join("dump")).args(["--seed", "4711", "--gamefiles"]).arg(&gf).output().unwrap();
    assert!(out.status.success(), "exit {:?}\n{}{}", out.status.code(), String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    let step = |n: usize| d.join("dump").join(format!("step_{n:04}"));
    for n in 0..3 {
        let s = step(n);
        assert_eq!(std::fs::read(s.join("planes.bin")).unwrap().len(), 4 * 65536, "step {n} planes");
        assert_eq!(std::fs::read(s.join("regs.bin")).unwrap().len(), 16, "step {n} regs");
        assert_eq!(std::fs::read(s.join("work.bin")).unwrap().len(), 57, "step {n} work");
    }
    assert!(!step(3).exists(), "no step beyond the script");
    let j = |n: usize| std::fs::read_to_string(step(n).join("step.json")).unwrap();
    assert!(j(0).starts_with("{\"key\":null,\"rand\":[],\"rand_state\":4711,"), "{}", j(0));
    assert!(j(0).contains("[\"menu\",[\"Start New Game\",\"Continue Old Game\",\"Get Started Quick\",\"Return to DOS\"]],[\"text\",\"Start New Game\"]"), "{}", j(0));
    assert!(j(1).starts_with("{\"key\":13,") && j(1).contains("\"Use My Party Disk\""), "{}", j(1));
    assert!(j(2).starts_with("{\"key\":336,") && j(2).contains("\"text\":[]"), "{}", j(2));
    let regs = std::fs::read(step(0).join("regs.bin")).unwrap();
    assert_eq!(regs, [0, 56, 7, 63, 4, 5, 62, 1, 2, 57, 58, 59, 60, 61, 20, 3], "the CG palette from CG.DAT DS:3692");
    std::fs::remove_dir_all(&d).unwrap();
}

#[test]
fn zz_skip_report() { oracle_env::report() }
