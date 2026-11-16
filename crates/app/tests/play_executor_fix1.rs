use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};
use serde_json::{json, Value};
fn dir() -> PathBuf {
    let p = std::env::temp_dir().join(format!("mt-v2-fix1-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    fs::create_dir_all(&p).unwrap(); p
}
fn run(args: &[&str], paths: &[&Path]) -> std::process::Output {
    assert!(std::env::var_os("MT1_GAMEFILES").is_some(), "MT1_GAMEFILES is required for live executor proof");
    let mut c = Command::new(env!("CARGO_BIN_EXE_megatraveller")); c.args(args).arg(paths[0]).arg("--out").arg(paths[1]);
    let mut child = c.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    let deadline = Instant::now()+Duration::from_secs(45);
    while child.try_wait().unwrap().is_none() { if Instant::now()>deadline { child.kill().unwrap(); let _=child.wait(); panic!("CLI hung"); } std::thread::sleep(Duration::from_millis(10)); }
    child.wait_with_output().unwrap()
}
fn route(start: Value, steps: Value) -> (std::process::Output,PathBuf) {
    let p=dir(); let path=p.join("route.json"); fs::write(&path,json!({"start":start,"start_tick":1234,"steps":steps}).to_string()).unwrap();
    (run(&["--play"],&[&path,&p.join("out")]),p)
}
fn read(p: &Path)->Vec<Value> {fs::read_to_string(p.join("out/trace.jsonl")).unwrap().lines().map(|s|serde_json::from_str(s).unwrap()).collect()}
fn replay(rows: &[Value])->(std::process::Output,PathBuf) {
    let p=dir(); let file=p.join("trace.jsonl"); fs::write(&file,rows.iter().map(|r|format!("{r}\n")).collect::<String>()).unwrap();
    (run(&["--play","--replay"],&[&file,&p.join("out")]),p)
}
fn fixture()->Vec<Value> {
    let (r,p)=route(json!("quickstart"),json!([
        {"pick":"continue"},{"snapshot":"S1"},{"pick":"continue"},{"restore":"S1"},{"pick":"continue"},{"replay":true}
    ])); assert!(r.status.success(),"{}",String::from_utf8_lossy(&r.stderr)); read(&p)
}
fn code(r: &std::process::Output, n:i32) {assert_eq!(r.status.code(),Some(n),"stdout={} stderr={}",String::from_utf8_lossy(&r.stdout),String::from_utf8_lossy(&r.stderr))}
fn incomplete(rows: &[Value]) {
    let (r,p)=replay(rows); code(&r,6);
    assert!(String::from_utf8_lossy(&r.stderr).contains("prefix verified to d:"));
    assert!(!read(&p).iter().any(|r|r["type"]=="complete"));
    let (again,_)=replay(&read(&p)); code(&again,6);
}
#[test] fn truncated_trace_never_becomes_complete() {
    let rows=fixture();
    let ats:Vec<_>=rows.iter().enumerate().filter(|(_,r)|r["type"]=="at").map(|(i,_)|i).collect();
    incomplete(&rows[..=ats[0]]); incomplete(&rows[..=ats[1]]);
    incomplete(&rows[..rows.len()-1]);
}
#[test] fn failed_route_replay_stays_nonzero() {
    let (r,p)=route(json!("quickstart"),json!([{"pick":"continue"},{"replay":true,"expect_kind":"space"}])); code(&r,3);
    incomplete(&read(&p));
}
#[test] fn hung_pick_trace_cannot_be_completed_by_replay() {
    let rows=fixture(); let pick=rows.iter().position(|r|r["type"]=="pick").unwrap(); incomplete(&rows[..=pick]);
}
#[test] fn restore_target_state_and_sequence_are_checked() {
    for field in ["state","decision"] { let mut rows=fixture(); let r=rows.iter_mut().find(|r|r["type"]=="restore").unwrap();
        r[field]=json!(if field=="state"{"s:0000000000000000"}else{"d:500"}); let (out,_)=replay(&rows); code(&out,3); }
}
#[test] fn discarded_branch_is_reexecuted() {
    let mut rows=fixture(); let restore=rows.iter().position(|r|r["type"]=="restore").unwrap();
    let entry=(0..restore).rfind(|&i|rows[i]["type"]=="entry").unwrap(); let pick=(0..entry).rfind(|&i|rows[i]["type"]=="pick").unwrap();
    rows[entry]["entry"]["pick"]=json!("bogus"); rows[pick]["pick"]=json!("bogus");
    let (r,_)=replay(&rows); code(&r,3);
}
#[test] fn every_at_and_pick_before_is_checked() {
    for (event,field,value) in [("at","state","s:2222222222222222"),("at","kind","space"),("pick","before","s:0000000000000000")] {
        let mut rows=fixture(); rows.iter_mut().find(|r|r["type"]==event).unwrap()[field]=json!(value);
        let (r,_)=replay(&rows); code(&r,3);
    }
}
#[test] fn complete_must_be_terminal_and_cover_all_route_steps() {
    let original=fixture(); let mut rows=original.clone(); let at=rows.iter().rev().find(|r|r["type"]=="at").unwrap().clone(); rows.push(at);
    let (r,_)=replay(&rows); assert!(!r.status.success());
    for (event,field,value) in [("header","route_steps",json!(99)),("complete","steps",json!(99)),("step","state",json!("s:0000000000000000"))] {
        let mut rows=original.clone();rows.iter_mut().find(|r|r["type"]==event).unwrap()[field]=value;let (r,_)=replay(&rows);code(&r,3);
    }
    let rows:Vec<_>=original.into_iter().filter(|r|r["type"]!="step").collect();let (r,_)=replay(&rows);code(&r,3);
}
#[test] fn mid_route_replay_does_not_certify_later_hash() {
    let (r,_)=route(json!("quickstart"),json!([{"pick":"continue"},{"replay":true},{"pick":"continue"},{"pick":"continue","after":{"kind":"ack"}}]));
    code(&r,0); assert!(!String::from_utf8_lossy(&r.stdout).contains("replay hash-equal"));
}
#[test] fn unchecked_terminal_pick_is_bad_route() {
    let (r,_)=route(json!("quickstart"),json!([{"pick":"continue","expect_kind":"ack"}])); code(&r,5);
}
#[test] fn no_motion_and_wrong_result_kind_fail() {
    let start=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    for step in [json!({"pick":"move:east:300","after":{"kind":"ground","state":{"cell":{"changed":true}}}}),
        json!({"pick":"move:north:300","after":{"kind":"ground"}})] {
        let (r,_)=route(json!(start),json!([step])); code(&r,3);
    }
}
#[test] fn unfilled_label_and_field_stop_before_action() {
    for step in [json!({"snapshot":"S1","label":"UNFILLED: next task"}),json!({"snapshot":"S1","unfilled":true})] {
        let (r,p)=route(json!("quickstart"),json!([step])); code(&r,7);
        assert!(!read(&p).iter().any(|r|r["type"]=="complete"));
    }
}
#[test] fn scenario_header_has_no_absolute_path_and_replays() {
    let start=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    let (r,p)=route(json!(start),json!([{"assert":{"system":2}},{"replay":true}])); code(&r,0);
    let rows=read(&p); let header=rows[0].to_string();
    assert!(!header.contains("C:")&&!header.contains("Users")&&!header.contains("\\\\?"),"{header}");
    let result=run(&["--play","--replay"],&[&p.join("out/trace.jsonl"),&p.join("replay")]); code(&result,0);
}
#[test] fn first_jump_guards_prove_required_state() {
    let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../acceptance/play/routes/quickstart_first_jump.json");
    let r:Value=serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let assertions:Vec<_>=r["steps"].as_array().unwrap().iter().filter_map(|s|s.get("assert")).collect();
    for field in ["conscious","alerted","ship_name","clock_days","panel","programs_loaded","programs_running"] {
        assert!(assertions.iter().any(|a|a.get(field).is_some()),"FIRST_TEST lacks required {field} guard");
    }
}
#[test] #[ignore = "plays the whole first-jump route: about five minutes in a debug build"]
fn first_jump_route_hash_is_unchanged() {
    assert!(std::env::var_os("MT1_GAMEFILES").is_some(), "MT1_GAMEFILES is required for live executor proof");
    let route=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../acceptance/play/routes/quickstart_first_jump.json");
    let r=Command::new(env!("CARGO_BIN_EXE_megatraveller")).arg("--play").arg(route).arg("--out").arg(dir().join("out")).stdin(Stdio::null()).output().unwrap();
    code(&r,0);
    assert!(String::from_utf8_lossy(&r.stdout).contains("final hash s:3f63d5c57bb4c7b1"),"{}",String::from_utf8_lossy(&r.stdout));
}
#[test] fn landing_route_guards_prove_required_state() {
    let path=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../acceptance/play/routes/quickstart_to_landing.json");
    let r:Value=serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
    let steps=r["steps"].as_array().unwrap();
    let assertions:Vec<_>=steps.iter().filter_map(|s|s.get("assert").or_else(||s.pointer("/after/state"))).collect();
    for field in ["conscious","alerted","ship_name","clock_days","panel","programs_loaded","programs_running","fuel","cell","planet"] {
        assert!(assertions.iter().any(|a|a.get(field).is_some()),"landing route lacks required {field} guard");
    }
    assert!(assertions.iter().any(|a|a["in_space"]==json!(true)&&a["planet"]==json!(255)),"no post-jump in-space guard");
    assert!(assertions.iter().any(|a|a["in_space"]==json!(false)&&a["cell"].is_array()),"no on-the-ground guard with a cell");
    assert!(steps.iter().any(|s|s["pick"]=="cmd:land"),"no landing pick");
    assert_eq!(steps.last().unwrap()["replay"],json!(true),"route must end with the root replay");
}
#[test] fn alerted_hunter_is_not_a_clear_ground_state() {
    let start=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_alerted.toml");
    let (r,_)=route(json!(start),json!([{"assert":{"encounter":null,"alerted":0},"expect_kind":"ground"}]));code(&r,3);
    assert!(String::from_utf8_lossy(&r.stderr).contains("alerted"));
}
#[test] fn live_ship_consciousness_and_named_clock_are_read_from_party() {
    let start=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml");
    let (r,_)=route(json!(start),json!([{"snapshot":"Clock"},{"assert":{"ship_name":"Interloper","conscious":{"min":1},"alerted":0,"clock_days":{"since":"Clock","delta":0}}}]));code(&r,0);
}
#[test] fn native_fatal_codes_are_disjoint_from_refused() {
    fn scan(path:&Path,codes:&mut std::collections::BTreeSet<u64>) {
        for entry in fs::read_dir(path).unwrap() {let p=entry.unwrap().path(); if p.is_dir(){scan(&p,codes)}else if p.extension().is_some_and(|s|s=="rs") {
            let text=fs::read_to_string(p).unwrap();
            for suffix in text.split("std::process::exit(").skip(1) { codes.insert(suffix.split(')').next().unwrap().trim().parse().expect("new nonconstant native exit requires audit")); }
        }}
    }
    let root=PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut codes=Default::default();scan(&root.join("crates/mt1/src"),&mut codes);
    assert_eq!(codes,std::collections::BTreeSet::from([1,3]));assert!(!codes.contains(&2));
}
