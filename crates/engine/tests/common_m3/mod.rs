#![allow(dead_code, unused_imports)]
#[path = "../common/mod.rs"]
pub mod common;
pub use common::{gamefiles, oracle, workspace};
use std::path::PathBuf;
use std::process::Command;

pub fn fixtures() -> Option<PathBuf> {
    let (oracle, set) = (oracle(), std::env::var_os("MT1_ORACLE").is_some());
    if !oracle.join("tools/golden/cg_spec.py").is_file() { assert!(!set, "MT1_ORACLE is set but {} has no tools/golden/cg_spec.py", oracle.display()); println!("SKIP: oracle not found at {}", oracle.display()); return None }
    let out = workspace().join("target/m3-fixtures");
    let gf = std::env::var_os("MT1_GAMEFILES").map(|p| PathBuf::from(p).to_string_lossy().into_owned()).unwrap_or("-".into());
    let r = match Command::new("python").arg(workspace().join("tools/m3_dump_sim.py")).arg(&oracle).arg(&gf).arg(&out).output() {
        Ok(o) => o,
        Err(e) => { assert!(!set, "MT1_ORACLE is set but python is not runnable: {e}"); println!("SKIP: python not runnable: {e}"); return None }
    };
    let so = String::from_utf8_lossy(&r.stdout).into_owned();
    assert!(r.status.success(), "m3_dump_sim.py failed:\n{so}{}", String::from_utf8_lossy(&r.stderr));
    if so.contains("PARTIAL") {
        let both = std::env::var_os("MT1_GAMEFILES").is_some() && std::env::var_os("MT1_ORACLE").is_some();
        assert!(!both, "helper dropped real cases with both env vars set:\n{so}");
        println!("{so}");
    }
    Some(out)
}
pub fn ints(line: &str) -> Vec<u64> { line.split_whitespace().map(|t| t.parse().unwrap_or_else(|_| panic!("bad int {t:?}"))).collect() }
