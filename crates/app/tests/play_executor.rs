use std::{fs, path::PathBuf, process::{Command, Stdio}};
use serde_json::{json, Value};

fn directory(name: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!("mt-play-{name}-{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&p).unwrap(); p
}
fn run(route: Value, name: &str) -> (std::process::Output, PathBuf) {
    let dir = directory(name); let path = dir.join("route.json");
    fs::write(&path, route.to_string()).unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_megatraveller"));
    command.arg("--play").arg(path).arg("--out").arg(dir.join("out"));
    let result = bounded(&mut command);
    (result, dir)
}
fn bounded(command: &mut Command) -> std::process::Output {
    let mut child = command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let until = std::time::Instant::now() + std::time::Duration::from_secs(45);
    while child.try_wait().unwrap().is_none() {
        if std::time::Instant::now() >= until { child.kill().unwrap(); let _ = child.wait(); panic!("headless CLI failed to exit in 45 s"); }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    child.wait_with_output().unwrap()
}
#[test]
fn malformed_route_has_distinct_exit_and_never_reaches_window_setup() {
    let (result, _) = run(json!({"start":"quickstart","start_tick":1234,"steps":[{"pick":false}]}), "bad");
    assert_eq!(result.status.code(), Some(5), "{}", String::from_utf8_lossy(&result.stderr));
}

fn require_live() { assert!(std::env::var_os("MT1_GAMEFILES").is_some(),"MT1_GAMEFILES is required for live executor proof"); }
#[test]
fn missing_assets_cannot_silently_pass_live_test() {
    let out=bounded(Command::new(std::env::current_exe().unwrap()).args(["--exact","real_quickstart_twice_restores_snapshots_frames_and_replays_hashes","--nocapture"]).env_remove("MT1_GAMEFILES"));
    assert!(!out.status.success(),"live test silently passed without assets");
}
fn game_text_samples() -> Vec<String> {
    let dir=PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES is required for the game-text leak check"));
    let words=mt_engine::data::dict::decode(&fs::read(dir.join("TRAV.DAT")).unwrap()).unwrap().words;
    let messages=mt_engine::data::msg::decode(mt_engine::data::Variant::Mt1,&fs::read(dir.join("TRAV.MSG")).unwrap()).unwrap();
    let samples:Vec<_>=messages.msgs.iter().take(5).flat_map(|m|m.windows(3).filter_map(|tokens| {
        let phrase=tokens.iter().map(|&i|usize::try_from(i).ok().and_then(|i|words.get(i)).and_then(|w|String::from_utf8(w.clone()).ok())).collect::<Option<Vec<_>>>()?.join(" ");
        (phrase.len()>=15).then_some(phrase)
    })).collect();
    assert!(samples.len()>=5,"need independent BYO text probes"); samples
}
fn text_free(text:&str) -> bool {
    !text.contains("\"prompt\"") && !text.contains("\"options\"") && !text.contains("Interloper") && game_text_samples().iter().all(|s| {
        let escaped=serde_json::to_string(s).unwrap();!text.contains(s) && !text.contains(&escaped[1..escaped.len()-1])
    })
}
#[test]
fn leak_check_detects_game_text_without_route_label() {
    assert!(!text_free("{\"unexpected\":\"Interloper\"}"),"game text escaped the trace leak check");
    for sample in game_text_samples() {assert!(!text_free(&json!({"unexpected":sample}).to_string()),"BYO text escaped the trace leak check");}
}
fn route(steps: Value) -> Value { json!({"start":"quickstart","start_tick":1234,"steps":steps}) }
fn trace(dir: &std::path::Path) -> Vec<Value> {
    fs::read_to_string(dir.join("out/trace.jsonl")).unwrap().lines().map(|s| serde_json::from_str(s).unwrap()).collect()
}
#[test]
fn refusal_and_expectation_mismatch_have_distinct_exits_and_diagnostics() {
    require_live();
    for (step, code, text) in [(json!({"pick":"missing","label":"Efate introduction"}), 2, "legal"),
        (json!({"pick":"continue","expect_kind":"space","label":"Efate introduction"}), 3, "expected"),
        (json!({"pick":"continue","expect_label":"wrong label","label":"Efate introduction"}), 3, "expected")] {
        let (result, _) = run(route(json!([step,{"replay":true}])), "stop");
        assert_eq!(result.status.code(), Some(code));
        let output = String::from_utf8_lossy(&result.stderr);
        for expected in ["Efate introduction", "d:1", text] { assert!(output.contains(expected), "{output}"); }
    }
}
#[test]
fn real_quickstart_twice_restores_snapshots_frames_and_replays_hashes() {
    require_live();
    let steps = json!([
        {"snapshot":"S1","expect_kind":"ack","frame":true,"label":"Efate introduction"},
        {"pick":"continue","expect_kind":"ack"}, {"pick":"continue","expect_kind":"ack"},
        {"restore":"S1"}, {"pick":"continue","expect_kind":"ack"}, {"replay":true}
    ]);
    let (a, dir_a) = run(route(steps.clone()), "twice-a");
    let (b, dir_b) = run(route(steps), "twice-b");
    for result in [&a, &b] { assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr)); }
    assert_eq!(trace(&dir_a), trace(&dir_b));
    let text = fs::read_to_string(dir_a.join("out/trace.jsonl")).unwrap();
    assert!(text_free(&text));
    for step in 1..=6 { assert!(dir_a.join(format!("out/snapshots/step-{step:04}.json")).is_file()); }
    for step in [1, 6] { assert_eq!(&fs::read(dir_a.join(format!("out/frames/{step:04}.png"))).unwrap()[..8], b"\x89PNG\r\n\x1a\n"); }
    let source = dir_a.join("out/trace.jsonl");
    let output = bounded(Command::new(env!("CARGO_BIN_EXE_megatraveller")).args(["--play","--replay"]).arg(&source)
        .arg("--out").arg(dir_a.join("replay")));
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let mut records = trace(&dir_a);
    let record = records.iter_mut().rfind(|v| v["type"] == "entry").unwrap();
    record["entry"]["before"] = json!("s:0000000000000000");
    let corrupt = dir_a.join("corrupt.jsonl");
    fs::write(&corrupt, records.iter().map(|v| format!("{v}\n")).collect::<String>()).unwrap();
    let output = bounded(Command::new(env!("CARGO_BIN_EXE_megatraveller")).args(["--play","--replay"]).arg(corrupt)
        .arg("--out").arg(dir_a.join("corrupt-out")));
    assert_eq!(output.status.code(), Some(3));
}

#[test]
fn scenario_route_asserts_moves_and_restore_replay_use_the_same_boot() {
    require_live();
    let mut r = route(json!([
        {"assert":{"system":2,"alive_mask":31,"in_space":false,"encounter":null},"expect_kind":"ground","label":"Quiet Efate"},
        {"snapshot":"S1"}, {"pick":"wait:1","expect_kind":"ground"},
        {"restore":"S1"}, {"pick":"wait:1","expect_kind":"ground"}, {"replay":true}
    ]));
    r["start"] = json!(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml"));
    let (result, _) = run(r, "scenario");
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
}

fn saved_frame(path: &std::path::Path) -> Vec<u32> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    let mut at = 8; let mut z = vec![];
    while at < bytes.len() {
        let n = u32::from_be_bytes(bytes[at..at+4].try_into().unwrap()) as usize;
        if &bytes[at+4..at+8] == b"IDAT" { z.extend_from_slice(&bytes[at+8..at+8+n]); }
        at += 12 + n;
    }
    assert_eq!(&z[..2], &[0x78, 0x01]);
    let mut at = 2; let mut raw = vec![];
    loop {
        let last = z[at] & 1 != 0; assert_eq!(z[at] & !1, 0);
        let n = u16::from_le_bytes(z[at+1..at+3].try_into().unwrap());
        assert_eq!(n, !u16::from_le_bytes(z[at+3..at+5].try_into().unwrap()));
        at += 5; raw.extend_from_slice(&z[at..at+n as usize]); at += n as usize;
        if last { break }
    }
    assert_eq!(raw.len(), 200 * 961);
    raw.chunks_exact(961).flat_map(|row| {
        assert_eq!(row[0], 0);
        row[1..].chunks_exact(3).map(|p| ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32)
    }).collect()
}

fn executor_snapshot_opens_at_its_recorded_frame(scenario: &str, kind: &str, pick: &str) {
    use megatraveller::{mcp::launch, play_store};
    use mt_engine::{io::Input, play::RootKind, sim::tick::Counter};
    #[derive(Debug)] struct Presented;
    struct Screen(String);
    impl Input for Screen {
        fn next_key(&mut self) -> u16 { panic!("frame must precede live input") }
        fn present(&mut self, frame: &[u32]) {
            assert_eq!(frame.len(), 64000);
            assert_eq!(launch::frame_hash(frame), self.0);
            std::panic::resume_unwind(Box::new(Presented));
        }
    }
    let mut r = route(json!([
        {"pick":pick,"expect_kind":kind,"after":{"kind":kind}},
        {"snapshot":"boundary","expect_kind":kind,"frame":true},
        {"pick":pick,"expect_kind":kind,"after":{"kind":kind}},
        {"restore":"boundary","expect_kind":kind,"frame":true}
    ]));
    r["start"] = json!(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!("../../scenarios/{scenario}.toml")));
    let (result, dir) = run(r, &format!("handoff-{scenario}"));
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let out = dir.join("out");
    let snapshot = play_store::load(&out.join("snapshots"), "step-0002").unwrap();
    assert_eq!(snapshot.root.kind, RootKind::Anchor, "executor scenario must use V1's fast image path");
    assert!(snapshot.journal.entries.is_empty(), "clean boundary needs no replay choices");
    let stderr=String::from_utf8_lossy(&result.stderr);
    let timing=stderr.lines().find(|line|line.starts_with("restore step 4:")).expect("restore step measurement");
    assert!(timing.ends_with("root Anchor"),"{timing}");
    let seconds: f64=timing.split_whitespace().nth(3).unwrap().parse().unwrap();
    assert!(seconds<5.,"{timing}");eprintln!("{scenario}: {timing}");
    let expected = launch::frame_hash(&saved_frame(&out.join("frames/0002.png")));
    assert_eq!(launch::frame_hash(&saved_frame(&out.join("frames/0004.png"))), expected);
    let gamefiles = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES"));
    let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        launch::file(&out.join("snapshots/step-0002.json"), &gamefiles, &dir, move |start|
            Ok((Box::new(Screen(expected)), Box::new(Counter::at(start.ticks)))))
    })).unwrap_err();
    assert!(exit.is::<Presented>(), "headless launch failed: {exit:?}");
}

#[test]
fn executor_ground_snapshot_first_frame_matches_headless_launch() {
    executor_snapshot_opens_at_its_recorded_frame("efate_quiet", "ground", "wait:1");
}

#[test]
fn executor_mid_fight_snapshot_first_frame_matches_headless_launch() {
    executor_snapshot_opens_at_its_recorded_frame("m8c_save_encounter", "combat.turn", "select:1");
}

#[test]
fn play_rejects_conflicting_modes_and_missing_assets_without_window_setup() {
    for other in ["--mcp","--faithful","--viewer","--ground","--snapshot","--build-id"] {
        let result = bounded(Command::new(env!("CARGO_BIN_EXE_megatraveller")).args(["--play","unused.json",other]));
        assert_eq!(result.status.code(), Some(5));
    }
    let dir = directory("missing-assets"); let path = dir.join("route.json");
    fs::write(&path, route(json!([{"pick":"continue","after":{"kind":"ack"}}])).to_string()).unwrap();
    let result = bounded(Command::new(env!("CARGO_BIN_EXE_megatraveller")).arg("--play").arg(path)
        .arg("--gamefiles").arg(dir.join("does-not-exist")));
    assert_eq!(result.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&result.stderr).contains("gamefiles"));
}
#[test]
fn decision_watchdog_times_out_a_route_without_opening_a_window() {
    require_live();
    let mut r = route(json!([{"pick":"continue","label":"Efate introduction","after":{"kind":"ack"}}])); r["timeout_ms"] = json!(1);
    let (result, _) = run(r, "timeout");
    assert_eq!(result.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&result.stderr).contains("hang"));
}
