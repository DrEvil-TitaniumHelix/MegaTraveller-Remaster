use std::fs;
#[cfg(not(feature = "dev"))]
use std::process::{Command, Stdio};
const MARKERS: &[&str] = &["MT_DEV_ONLY_BRIDGE_IMPLEMENTATION_V1", "BUGBRIDGE v1", "Send to the fixer", "MT_FOREMAN_PANE", "Fix branch discarded"];

#[test]
fn executable_contains_bridge_markers_only_with_dev_feature() {
    let exe = std::env::var_os("MT_BRIDGE_EXE").unwrap_or_else(|| env!("CARGO_BIN_EXE_megatraveller").into());
    let bytes = fs::read(exe).unwrap();
    assert_eq!(megatraveller::plus::bridge::AVAILABLE, cfg!(feature = "dev"));
    for marker in MARKERS {
        let found = bytes.windows(marker.len()).any(|b| b == marker.as_bytes());
        assert_eq!(found, cfg!(feature = "dev"), "binary bridge marker: {marker}");
    }
}
#[cfg(not(feature = "dev"))]
#[test]
fn runtime_flag_cannot_enable_bridge_in_player_build() {
    let result = Command::new(env!("CARGO_BIN_EXE_megatraveller")).arg("--dev").stdin(Stdio::null()).output().unwrap();
    assert_eq!(result.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&result.stderr).contains("unknown argument --dev"));
}
