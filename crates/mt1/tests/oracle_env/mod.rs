use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static SKIPS: AtomicUsize = AtomicUsize::new(0);

fn required() -> bool { std::env::var_os("MT_REQUIRE_ORACLE").is_some() }
fn missing(var: &str, note: &str) {
    SKIPS.fetch_add(1, Ordering::Relaxed);
    if required() { panic!("{var} {note}: MT_REQUIRE_ORACLE is set, so this FAILS instead of skipping (unset MT_REQUIRE_ORACLE to allow skips)") }
    let _ = std::io::stderr().write_all(format!("SKIP: {var} {note}\n").as_bytes());
}
fn workspace() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..") }

#[allow(dead_code)]
pub fn gamefiles() -> Option<PathBuf> {
    match std::env::var_os("MT1_GAMEFILES") {
        Some(p) => Some(PathBuf::from(p)),
        None => { missing("MT1_GAMEFILES", "unset"); None }
    }
}
#[allow(dead_code)]
pub fn oracle() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("MT1_ORACLE") { return Some(PathBuf::from(p)) }
    let default = workspace().join("../MegaTraveller");
    if default.is_dir() { return Some(default) }
    missing("MT1_ORACLE", &format!("unset and {} missing", default.display()));
    None
}
#[allow(dead_code)]
pub fn both() -> Option<(PathBuf, PathBuf)> { Some((oracle()?, gamefiles()?)) }
#[allow(dead_code)]
pub fn report() {
    let n = SKIPS.load(Ordering::Relaxed);
    if n > 0 { let _ = std::io::stderr().write_all(format!("SKIPPED {n} tests for missing oracle settings\n").as_bytes()); }
}
