use mt1::ground::{state::{Ground, Mem}, trace::Trace};
use mt_engine::{io::{Dir, Scripted}, sim::tick::Counter};
use std::{process::{Command, Stdio}, time::{Duration, Instant}};

#[test]
fn budget_child() {
    let Ok(kind) = std::env::var("MT_TRACE_LIMIT_CASE") else { return };
    let mut trace = Trace::default(); trace.on = true; trace.enter("open", &[]);
    match kind.as_str() {
        "missing_entry" => { trace.exit(None); trace.take_jsonl(); }
        "calls" | "writes" | "nested" => {
            if kind == "nested" { for _ in 0..32 { trace.enter("nested", &[]) } }
            for _ in 0..65536 {
                if kind == "calls" { trace.call("callee", &[0; 1024], None) } else { trace.write(Mem::DS, 0, &[0; 1024]) }
                assert!(trace.take_jsonl().is_empty());
            }
        }
        "frames" | "entries" => {
            let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(std::env::temp_dir())));
            g.harness = true; g.mem.trace.on = true;
            for _ in 0..4096 {
                if kind == "frames" { g.frame("redraw_view") } else { g.mem.trace_enter("entry", &[]); g.mem.trace.exit(None) }
            }
        }
        _ => panic!("unknown case"),
    }
    panic!("budget did not stop recording")
}

#[test]
fn recording_limits_stop_with_error() {
    for kind in ["calls", "writes", "nested", "frames", "entries", "missing_entry"] {
        let mut child = Command::new(std::env::current_exe().unwrap()).args(["--exact", "budget_child", "--nocapture"])
            .env("MT_TRACE_LIMIT_CASE", kind).stdout(Stdio::null()).spawn().unwrap();
        let start = Instant::now();
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() { break status }
            if start.elapsed() > Duration::from_secs(15) { child.kill().unwrap(); child.wait().unwrap(); panic!("{kind}: watchdog expired") }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(3), "{kind}");
    }
}
