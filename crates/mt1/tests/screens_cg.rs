mod oracle_env;
#[path = "../../engine/tests/common_m2/mod.rs"]
mod common_m2;
use common_m2::{workspace, Case};
use oracle_env::{gamefiles, oracle};
use mt1::screens::cg::Screens;
use mt1::ui::{CgUi, Menu};
use mt_engine::data::{exepack, ly};
use mt_engine::io::{Dir, Input, Key};
use mt_engine::present::fb::{PLANE, SNAP};
use mt_engine::sim::tick::Counter;
use std::path::PathBuf;
use std::process::Command;

struct Keys(std::collections::VecDeque<Key>);
impl Input for Keys {
    fn next_key(&mut self) -> Key { self.0.pop_front().unwrap_or_else(|| std::panic::panic_any("key script exhausted (a routine waited for more keys than the oracle did)".to_string())) }
    fn key_pending(&mut self) -> bool { !self.0.is_empty() }
}
fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
fn latin1(s: &str) -> String { hex(s).iter().map(|&b| b as char).collect() }
struct Call { addr: u16, args: Vec<i32>, strs: Vec<Option<String>>, keys: Vec<Key>, rand: Vec<u16> }
struct Spec { calls: Vec<Call>, data: Vec<(String, Vec<String>)> }
fn parse(text: &str) -> Spec {
    let (mut calls, mut data) = (Vec::<Call>::new(), vec![]);
    let mut rand = vec![];
    for line in text.lines() {
        let w: Vec<&str> = line.split(' ').collect();
        match w[0] {
            "rand" => rand = w[1..].iter().filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect(),
            "call" => {
                let (mut args, mut strs) = (vec![], vec![]);
                for t in w[2..].iter().filter(|t| !t.is_empty()) { if let Some(h) = t.strip_prefix("s:") { args.push(0); strs.push(Some(latin1(h))) } else { args.push(t.parse().unwrap()); strs.push(None) } }
                calls.push(Call { addr: u16::from_str_radix(w[1], 16).unwrap(), args, strs, keys: vec![], rand: std::mem::take(&mut rand) });
            }
            "keys" => calls.last_mut().unwrap().keys.extend(w[2..].iter().filter(|s| !s.is_empty()).map(|s| s.parse::<Key>().unwrap())),
            "result" | "text" => {}
            k => data.push((k.to_string(), w[1..].iter().map(|s| s.to_string()).collect())),
        }
    }
    Spec { calls, data }
}
impl Spec {
    fn data(&self, key: &str) -> Vec<String> { self.data.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone()).unwrap_or_default() }
    fn strs(&self, key: &str) -> Vec<String> { self.data(key).iter().map(|h| latin1(h)).collect() }
}

fn fixtures() -> Option<Vec<(Case, Vec<String>, Spec)>> {
    let Some(o) = oracle() else { return None };
    if !o.join("tools/spec/render_cg_paths.py").is_file() { println!("SKIP: oracle not found at {}", o.display()); return None }
    let out = workspace().join("target/m3-fixtures/screens");
    let gf = std::env::var_os("MT1_GAMEFILES").map(|p| PathBuf::from(p).to_string_lossy().into_owned()).unwrap_or("-".into());
    let r = match Command::new("python").arg(workspace().join("tools/m3_dump_screens.py")).arg(&o).arg(&gf).arg(&out).output() {
        Ok(r) => r, Err(e) => { println!("SKIP: python not runnable: {e}"); return None }
    };
    let so = String::from_utf8_lossy(&r.stdout).into_owned();
    assert!(r.status.success(), "m3_dump_screens.py failed:\n{so}{}", String::from_utf8_lossy(&r.stderr));
    if so.contains("PARTIAL") {
        let both = std::env::var_os("MT1_GAMEFILES").is_some() && std::env::var_os("MT1_ORACLE").is_some();
        assert!(!both, "helper dropped real cases with both env vars set:\n{so}");
        println!("{so}");
    }
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("case.txt").is_file()).collect();
    dirs.sort();
    Some(dirs.into_iter().map(|dir| {
        let rd = |f: &str| std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{}/{f}: {e}", dir.display()));
        let (start, end) = (rd("start.bin"), rd("end.bin"));
        assert!(start.len() == SNAP && end.len() == SNAP, "{}: snapshot length", dir.display());
        let name = dir.file_name().unwrap().to_string_lossy().into_owned();
        let state: Vec<String> = String::from_utf8(rd("state.txt")).unwrap().lines().map(String::from).collect();
        let spec = parse(&String::from_utf8(rd("case.txt")).unwrap());
        (Case { name, calls: common_m2::parse_calls(&String::from_utf8(rd("calls.json")).unwrap()), dir, start, end }, state, spec)
    }).collect())
}

fn apply(s: &mut Screens, c: &Call, spec: &Spec) {
    let a = |k: usize| c.args.get(k).copied().unwrap_or(0) as i16;
    let st = |k: usize| c.strs.get(k).cloned().flatten().unwrap_or_default();
    let tail3 = |from: usize| [a(from), a(from + 1), a(from + 2)];
    let items = spec.strs("items"); let items: Vec<&str> = items.iter().map(String::as_str).collect();
    match c.addr {
        0x519A | 0x56A6 => {
            let initial = std::mem::replace(&mut s.initial, -1);
            let m = Menu { items: &items, highlight: a(2), row: a(3), col: a(4), allow_unique: a(5), border: a(8), color: a(9), extra: &c.args[10..].iter().map(|&v| v as i16).collect::<Vec<_>>(), initial };
            let (on_change, on_select) = (c.args[6] as u16, c.args[7] as u16);
            let rows: Vec<Vec<(String, u8)>> = (0..4).map(|t| spec.data.iter().find(|(k, v)| k == "rows" && v.first().map_or(false, |x| x == &t.to_string())).map_or(vec![], |(_, v)| v[1..].iter().map(|e| { let (n, k) = e.rsplit_once(':').unwrap(); (latin1(n), k.parse().unwrap()) }).collect())).collect();
            let labels = spec.strs("labels");
            let rand = c.rand.clone(); let mut ri = 0usize;
            let mut change = |ui: &mut dyn CgUi, i: i16| {
                let s = unsafe { &mut *(ui as *mut dyn CgUi as *mut Screens) };
                if on_change == 0x2E36 { let r: Vec<(&str, u8)> = rows[i as usize].iter().map(|(n, k)| (n.as_str(), *k)).collect(); s.skill_table(&r) }
                if on_change == 0x374A { let l: Vec<&str> = labels.iter().map(String::as_str).collect(); s.skill_table_help(&l, i) }
            };
            let page = spec.calls.first().filter(|f| f.addr == 0x0A36).map(|_| 3i16).unwrap_or(0);
            let mut select = |ui: &mut dyn CgUi, i: i16| -> bool {
                let s = unsafe { &mut *(ui as *mut dyn CgUi as *mut Screens) };
                if on_select == 0x900 && i == 1 {
                    s.select_window(page); s.clear_window();
                    let stats: [u8; 6] = std::array::from_fn(|_| { let (x, y) = (rand[ri], rand[ri + 1]); ri += 2; (2 + x % 6 + y % 6) as u8 });
                    s.attribute_values(page, &stats); return true
                }
                false
            };
            if c.addr == 0x519A { s.menu_cb(&m, (on_change != 0).then_some(&mut change as _), None); }
            else if on_select != 0 { s.hmenu_repeat(&m, &mut select); }
            else { s.hmenu_cb(&m, (on_change != 0).then_some(&mut change as _), None); }
        }
        0x1AA2 => { s.yes_no(&[items[0], items[1]]); }
        0x12B0 => s.message(&st(0)),
        0x1158 => s.show_message(&st(0)),
        0x1BD0 => s.banner(&st(0)),
        0x5CC2 => { s.input_text(a(1) as usize, a(2), a(3), a(4), a(5), a(6), tail3(7)); }
        0x4FFA => { let t = c.strs.get(10).cloned().flatten(); s.open_window_full(a(0), a(1), a(2), a(3), a(4), a(5), a(6), tail3(7), t.as_deref().map(|t| (t, a(11)))); }
        0x501E => s.close_window(a(0)),
        0x0A36 => { let l = spec.strs("labels"); let l: Vec<&str> = l.iter().map(String::as_str).collect(); s.label_panel(&l, a(2), a(3), a(4), a(5), a(6), tail3(7)); }
        0x0B1C => { let w = hex(&spec.data("work")[0]); s.attribute_values(a(0), w[16..22].try_into().unwrap()) }
        0x2DCC => s.flash(a(0), a(1), a(2), a(3), a(4), a(5)),
        0x0C9C => s.skill_totals(a(0), a(1)),
        0x0B7A => s.skill_cursor(a(0)),
        0x1B72 => { let n: Vec<String> = spec.data("names").iter().map(|h| if h == "-" { String::new() } else { latin1(h) }).collect(); let n: Vec<&str> = n.iter().map(String::as_str).collect(); s.party_list(&n) }
        0x6416 => s.layout(a(0), a(1), a(2)),
        0x4A50 => for l in spec.data.iter().filter(|(k, _)| k == "textlist") { let v = &l.1; s.move_to(v[0].parse().unwrap(), v[1].parse().unwrap()); s.set_text_color(v[2].parse().unwrap()); let t = latin1(&v[3]); s.text(&t) },
        0x251E => { let rec = hex(&spec.data("record")[0]); s.character_sheet(a(0) as usize, &rec) }
        0x481A => s.set_text_color(a(0)), 0x4792 => s.set_text_flags(a(0) as u8), 0x479E => s.clear_text_flags(a(0) as u8), 0x47AC => s.set_shadow_color(a(0)),
        0x10E8 => s.draw_text_at(a(0), a(1), a(2), a(3), a(4), &st(5)),
        0x49B8 => s.draw_centered(&st(0), a(1)),
        0x1416 => s.service_emblem(a(0), a(1), a(2)),
        x => panic!("no replay for {x:04X}"),
    }
}

#[test]
fn cg_screens_vs_original() {
    let Some(gf) = gamefiles() else { return };
    let Some(cases) = fixtures() else { return };
    assert!(cases.len() >= 100, "{} cases", cases.len());
    let exe = exepack::unpack(&std::fs::read(gf.join("CG.DAT")).unwrap()).unwrap().exe;
    let load = |n: &str| ly::decode(&std::fs::read(gf.join(n)).unwrap()).unwrap();
    let (mut pass, mut fails) = (0, vec![]);
    for (case, state, spec) in &cases {
        let keys: Vec<Key> = spec.calls.iter().flat_map(|c| c.keys.iter().copied()).collect();
        let mut s = Screens::new(exe.clone(), load("FONT.LY"), load("CG.LY"), Box::new(Keys(keys.into())), Box::new(Counter::default()), Box::new(Dir(gf.clone()))).unwrap();
        s.fb.restore(&case.start);
        for l in state { s.load_state_line(l) }
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| for c in &spec.calls { apply(&mut s, c, spec) }));
        let got = s.fb.snapshot();
        match r {
            Err(p) => fails.push(format!("{}: panic {}", case.name, p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default())),
            Ok(()) => match (0..SNAP).find(|&i| got[i] != case.end[i]) {
                None => { pass += 1; println!("PASS {}", case.name) }
                Some(i) => {
                    let n = (0..SNAP).filter(|&i| got[i] != case.end[i]).count();
                    let (plane, off) = if i < 4 * PLANE { (format!("plane {}", i / PLANE), i % PLANE) } else { ("regs".to_string(), i - 4 * PLANE) };
                    let first: Vec<String> = case.calls.iter().take(12).map(|(n, a)| format!("{n}{a:?}")).collect();
                    fails.push(format!("{}: {plane} offset {off} (row {} byte {}): expected {:#04x} got {:#04x}; {n} bytes differ; oracle calls: {}", case.name, off % 8192 / 40, off % 8192 % 40, case.end[i], got[i], first.join(" ")))
                }
            },
        }
    }
    for f in &fails { println!("FAIL {f}") }
    println!("{pass}/{} CG screen cases byte-equal", cases.len());
    assert!(fails.is_empty(), "{} of {} cases differ", fails.len(), cases.len());
}

#[test]
fn esc_paths_per_c() {
    let Some(gf) = gamefiles() else { return };
    let exe = exepack::unpack(&std::fs::read(gf.join("CG.DAT")).unwrap()).unwrap().exe;
    let load = |n: &str| ly::decode(&std::fs::read(gf.join(n)).unwrap()).unwrap();
    let screens = |keys: Vec<Key>| Screens::new(exe.clone(), load("FONT.LY"), load("CG.LY"), Box::new(Keys(keys.into())), Box::new(Counter::default()), Box::new(Dir(gf.clone()))).unwrap();
    let items = ["One", "Two"];
    let menu = |initial: i16| Menu { items: &items, highlight: 4, row: 80, col: 30, allow_unique: 0, border: 2, color: 0, extra: &[1, 0, 3], initial };
    let mut s = screens(vec![27]);
    let before = s.fb.snapshot();
    let mut got = None;
    assert_eq!(s.menu_cb(&menu(-1), None, Some(&mut |_, i| got = Some(i))), -1);
    assert_eq!((got, s.current_window()), (Some(-1), 0));
    assert_ne!(s.fb.snapshot(), before, "the closed menu window's restore fill (0x12) stays on screen");
    let mut s = screens(vec![27]);
    let mut selected = false;
    assert_eq!(s.hmenu_cb(&menu(-1), None, Some(&mut |_, _| selected = true)), -1);
    assert!(!selected); assert_ne!(s.current_window(), 0, "56A6's window is still open (4C50 frees it at the recovery)");
    let mut s = screens(vec![b'A' as Key, 27]);
    assert_eq!(s.input_text(15, 100, 125, 80, 1, 0, [3, 0, 0]), None);
    let mut s = screens(vec![b'A' as Key, 13]);
    assert_eq!(s.name(0), Some(String::new()));
    let mut s = screens(vec![13]);
    let empty = Menu { items: &[], ..menu(-1) };
    let before = s.fb.snapshot();
    assert_eq!((s.menu(&empty), s.hmenu(&empty)), (-1, -1)); assert_eq!(s.fb.snapshot(), before);
    assert_eq!(s.hmenu(&menu(7)), 0);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
