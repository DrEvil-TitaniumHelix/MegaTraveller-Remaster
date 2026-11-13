#![allow(dead_code, unused_imports)]
#[path = "../common/mod.rs"]
pub mod common;
pub use common::{gamefiles, oracle, workspace};
use mt_engine::present::fb::{Fb, PLANE, SNAP};
use std::path::PathBuf;
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
pub enum Arg { I(i64), S(String) }
impl Arg {
    pub fn i(&self) -> i64 { match self { Arg::I(v) => *v, Arg::S(s) => panic!("int expected, got {s:?}") } }
    pub fn s(&self) -> &str { match self { Arg::S(s) => s, Arg::I(v) => panic!("string expected, got {v}") } }
    pub fn hex(&self) -> Vec<u8> { let s = self.s(); (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
}
pub struct Case { pub name: String, pub dir: PathBuf, pub start: Vec<u8>, pub end: Vec<u8>, pub calls: Vec<(String, Vec<Arg>)> }

pub fn parse_calls(text: &str) -> Vec<(String, Vec<Arg>)> {
    text.lines().map(str::trim).filter(|l| l.starts_with('[') && l.len() > 1).map(|l| {
        let l = l.trim_start_matches('[').trim_end_matches(',').trim_end_matches(']');
        let (mut args, mut cs) = (Vec::new(), l.chars().peekable());
        while let Some(&c) = cs.peek() {
            if c == ',' || c == ' ' { cs.next(); continue }
            if c == '"' { cs.next(); let s: String = cs.by_ref().take_while(|&c| c != '"').collect(); args.push(Arg::S(s)) }
            else { let s: String = cs.by_ref().take_while(|&c| c != ',').collect(); args.push(Arg::I(s.trim().parse().unwrap_or_else(|_| panic!("bad token {s:?} in {l}")))) }
        }
        let name = args.remove(0).s().to_string();
        (name, args)
    }).collect()
}

pub fn fixtures(group: &str, helper: &str) -> Option<Vec<Case>> {
    let oracle = oracle();
    if !oracle.join("tools/spec/render_s_cg.py").is_file() { println!("SKIP: oracle not found at {}", oracle.display()); return None }
    let out = workspace().join("target/m2-fixtures").join(group);
    let gf = std::env::var_os("MT1_GAMEFILES").map(|p| PathBuf::from(p).to_string_lossy().into_owned()).unwrap_or("-".into());
    let r = match Command::new("python").arg(workspace().join("tools").join(helper)).arg(&oracle).arg(&gf).arg(&out).output() {
        Ok(o) => o,
        Err(e) => { println!("SKIP: python not runnable: {e}"); return None }
    };
    assert!(r.status.success(), "{helper} failed:\n{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&out).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.join("calls.json").is_file()).collect();
    dirs.sort();
    Some(dirs.into_iter().map(|dir| {
        let rd = |f: &str| std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{}/{f}: {e}", dir.display()));
        let (start, end) = (rd("start.bin"), rd("end.bin"));
        assert!(start.len() == SNAP && end.len() == SNAP, "{}: snapshot length", dir.display());
        Case { name: dir.file_name().unwrap().to_string_lossy().into_owned(), calls: parse_calls(&String::from_utf8(rd("calls.json")).unwrap()), dir, start, end }
    }).collect())
}

pub fn check(case: &Case, got: &[u8]) {
    if let Some(i) = (0..SNAP).find(|&i| got[i] != case.end[i]) {
        let (plane, off) = if i < 4 * PLANE { (format!("plane {}", i / PLANE), i % PLANE) } else { ("regs".to_string(), i - 4 * PLANE) };
        let n = (0..SNAP).filter(|&i| got[i] != case.end[i]).count();
        panic!("FAIL {}: {plane} offset {off} (page {} row {} byte {}): expected {:#04x} got {:#04x}; {n} bytes differ", case.name, off / 8192, off % 8192 / 40, off % 8192 % 40, case.end[i], got[i]);
    }
    println!("PASS {}", case.name);
}
pub fn replay(cases: &[Case], mut apply: impl FnMut(&mut Fb, &str, &[Arg])) {
    for case in cases {
        let mut fb = Fb::new();
        assert!(fb.restore(&case.start));
        for (name, args) in &case.calls { apply(&mut fb, name, args) }
        check(case, &fb.snapshot());
    }
}

#[test]
fn parse_calls_subset() {
    let c = parse_calls("[\n[\"line\", \"wrap\", 1, -5, 300],\n[\"blit\", \"0aff\", 2]\n]\n");
    assert_eq!(c.len(), 2);
    assert_eq!(c[0], ("line".into(), vec![Arg::S("wrap".into()), Arg::I(1), Arg::I(-5), Arg::I(300)]));
    assert_eq!(c[1].1[0].hex(), [0x0a, 0xff]); assert_eq!(c[1].1[1].i(), 2);
}
