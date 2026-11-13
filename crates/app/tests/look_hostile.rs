use megatraveller::look::Look;
use std::{fs, process::Command};

#[test]
fn hostile_settings_subprocess() {
    const CHILD_PATH: &str = "MT_LOOK_HOSTILE_CHILD";
    if let Some(path) = std::env::var_os(CHILD_PATH) {
        assert_eq!(Look::load(path), Look::default());
        return;
    }
    let path = std::env::temp_dir().join(format!("mt-look-hostile-{}.toml", std::process::id()));
    let hostile = format!("unknown = {}0{}", "[".repeat(128), "]".repeat(128));
    assert_eq!(hostile.len(), 267);
    for hostile in [hostile, format!("unknown = {}0{}", "{ a.b.c.d.e.f.g = ".repeat(15), "}".repeat(15))] {
        fs::write(&path, hostile).unwrap();
        let output = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "hostile_settings_subprocess", "--nocapture"])
            .env(CHILD_PATH, &path).output().unwrap();
        fs::remove_file(&path).unwrap();
        assert!(output.status.success(), "child exit {:?}\n{}{}", output.status.code(),
            String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    }
}
