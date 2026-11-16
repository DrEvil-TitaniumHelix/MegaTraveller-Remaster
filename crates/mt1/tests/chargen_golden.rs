mod oracle_env;
#[path = "../../engine/tests/common_m3/mod.rs"]
mod common_m3;
use mt1::chargen::pool::main_menu;
use mt1::chargen::tables::{Cg, Ds};
use mt1::record::Work;
use mt1::ui::{CgUi, Menu, MenuCb, SelectCb};
use mt_engine::io::{dump::Text, keys, Files, Key, Overlay};
use mt_engine::sim::rng::Rng;
use std::collections::VecDeque;
use std::panic::{catch_unwind, panic_any, resume_unwind, AssertUnwindSafe};
use std::path::{Path, PathBuf};

struct Exhausted;
#[derive(Default)]
struct Fixture { seed: u16, disk: bool, groups: Vec<Vec<Key>>, steps: Vec<(Vec<u16>, u32, Vec<u8>, Vec<(String, Vec<u8>)>)>, widths: Vec<u8>, pool: Option<(usize, Vec<u8>)>, skills: Vec<(u8, u8)>, uchars: Option<Vec<u8>> }
fn hex(s: &str) -> Vec<u8> { (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect() }
fn parse(p: &Path) -> Fixture {
    let mut f = Fixture::default();
    for line in std::fs::read_to_string(p).unwrap().lines() {
        let w: Vec<&str> = line.split(' ').collect();
        match w[0] {
            "run" => { f.seed = w[1].parse().unwrap(); f.disk = w[4] == "1" }
            "step" => { if w[1] != "0" { f.groups.push(w[3].split(',').map(|k| k.parse().unwrap()).collect()) } f.steps.push((vec![], w[5].parse().unwrap(), vec![], vec![])) }
            "rand" => f.steps.last_mut().unwrap().0 = w[1..].iter().filter(|s| !s.is_empty()).map(|s| s.parse().unwrap()).collect(),
            "work" => f.steps.last_mut().unwrap().2 = hex(w[1]),
            "text" => f.steps.last_mut().unwrap().3.push((w[1].to_string(), hex(w.get(2).copied().unwrap_or("")))),
            "widths" => f.widths = hex(w[1]),
            "pool" => f.pool = Some((w[1].parse().unwrap(), hex(w[2]))),
            "skills" => f.skills = w[1].split(',').filter(|s| !s.is_empty()).map(|s| { let (a, b) = s.split_once(':').unwrap(); (a.parse().unwrap(), b.parse().unwrap()) }).collect(),
            "uchars" => f.uchars = Some(hex(w[1])),
            _ => {}
        }
    }
    f
}

struct Fake<'a> {
    ds: &'a Ds, rng: Rng, work: Work, files: Overlay, keys: VecDeque<Key>, groups: VecDeque<Vec<Key>>, text: Vec<Text>, widths: Vec<u8>,
    steps: Vec<(Vec<u16>, u32, Work, Vec<Text>)>, pool_rec: Option<[u8; 256]>, height: i16, spacing: i16, windows: i16, current: i16,
}
impl<'a> Fake<'a> {
    fn width(&self, s: &str) -> i32 { s.chars().map(|c| if (c as u32) < 128 { self.widths[c as usize] as i32 } else { 0 }).sum() }
    fn read_key(&mut self) -> Key {
        if let Some(k) = self.keys.pop_front() { return k }
        self.steps.push((self.rng.take_trace(), self.rng.state, self.work, std::mem::take(&mut self.text)));
        match self.groups.pop_front() { Some(g) => { self.keys.extend(g); self.keys.pop_front().unwrap_or(keys::ENTER) } None => panic_any(Exhausted) }
    }
    fn find_initial(items: &[&str], key: Key, allow_unique: i16) -> i16 {
        let k = (key as u8 as char).to_ascii_lowercase();
        let (mut found, mut ambiguous) = (-1i16, false);
        for (i, it) in items.iter().enumerate() {
            if it.chars().next().map(|c| c.to_ascii_lowercase()) == Some(k) { if found != -1 { ambiguous = true; break } found = i as i16 }
        }
        found + if found != -1 && (ambiguous || allow_unique == 0) { 1000 } else { 0 }
    }
    fn wrap(&mut self, s: &str) {
        let mut text: Vec<char> = s.chars().collect();
        loop {
            let find = |t: &[char], from: usize| t[from.min(t.len())..].iter().position(|&c| c == ' ').map(|p| p + from);
            let whole = self.width(&text.iter().collect::<String>());
            let Some(mut next) = (if whole < 213 { None } else { find(&text, 0) }) else { if !text.is_empty() { self.text.push(Text::Text(text.iter().collect())) } return };
            loop {
                let last = next;
                let n2 = find(&text, last + 1);
                let width = self.width(&text[..n2.unwrap_or(text.len())].iter().collect::<String>());
                if n2.is_none() && width < 213 { self.text.push(Text::Text(text.iter().collect())); return }
                if width > 213 {
                    self.text.push(Text::Text(text[..last].iter().collect()));
                    let mut p = last; while p < text.len() && text[p] == ' ' { p += 1 }
                    text = text[p..].to_vec(); break;
                }
                match n2 { Some(n) => next = n, None => { self.text.push(Text::Text(text.iter().collect())); return } }
            }
        }
    }
    fn title(&mut self, m: &Menu) { if m.border == 3 { if let Some(&p) = m.extra.get(3) { let t = self.ds.str(p as u16 as usize); self.text.push(Text::Text(t)) } } }
    fn hmenu_keys(&mut self, m: &Menu, sel: &mut i16, on_change: &mut Option<MenuCb>) -> bool {
        let n = m.items.len() as i16;
        loop {
            let k = self.read_key();
            match k {
                13 => return true, 27 => return false,
                32..=126 => { let f = Self::find_initial(m.items, k, m.allow_unique); if f != -1 && f - *sel != 1000 { *sel = if f >= 1000 { f - 1000 } else { f }; if let Some(c) = on_change { c(self, *sel) } if f < 1000 { return true } } }
                keys::RIGHT => if n - *sel != 1 { *sel += 1; if let Some(c) = on_change { c(self, *sel) } },
                keys::LEFT => if *sel > 0 { *sel -= 1; if let Some(c) = on_change { c(self, *sel) } },
                _ => {}
            }
        }
    }
}
impl<'a> CgUi for Fake<'a> {
    fn rng(&mut self) -> &mut Rng { &mut self.rng }
    fn work(&mut self) -> &mut Work { &mut self.work }
    fn files(&mut self) -> &mut dyn Files { &mut self.files }
    fn pool_added(&mut self, rec: &[u8; 256]) { self.pool_rec = Some(*rec) }
    fn menu_cb(&mut self, m: &Menu, mut on_change: Option<MenuCb>, mut on_select: Option<MenuCb>) -> i16 {
        self.text.push(Text::Menu(m.items.iter().map(|s| s.to_string()).collect()));
        self.title(m);
        for it in m.items { self.text.push(Text::Text(it.to_string())) }
        let (n, mut sel) = (m.items.len() as i16, if m.initial != -1 { m.initial } else { 0 });
        if let Some(c) = &mut on_change { c(self, sel) }
        if sel != 0 { sel = 0; if let Some(c) = &mut on_change { c(self, sel) } }
        loop {
            let k = self.read_key();
            match k {
                13 => break, 27 => { sel = -1; break }
                32..=126 => { let f = Self::find_initial(m.items, k, m.allow_unique); if f != -1 && f - sel != 1000 { sel = if f >= 1000 { f - 1000 } else { f }; if let Some(c) = &mut on_change { c(self, sel) } if f < 1000 { break } } }
                keys::UP => { sel -= 1; if sel < 0 { sel = n - 1 } if let Some(c) = &mut on_change { c(self, sel) } }
                keys::DOWN => { sel += 1; if sel >= n { sel = 0 } if let Some(c) = &mut on_change { c(self, sel) } }
                _ => {}
            }
        }
        if let Some(c) = &mut on_select { c(self, sel) }
        sel
    }
    fn hmenu_cb(&mut self, m: &Menu, mut on_change: Option<MenuCb>, mut on_select: Option<MenuCb>) -> i16 {
        self.title(m);
        for it in m.items { self.text.push(Text::Text(it.to_string())) }
        let mut sel = if m.initial != -1 { m.initial } else { 0 };
        if let Some(c) = &mut on_change { c(self, sel) }
        if sel != 0 { sel = 0; if let Some(c) = &mut on_change { c(self, sel) } }
        if !self.hmenu_keys(m, &mut sel, &mut on_change) { return -1 }
        if let Some(c) = &mut on_select { c(self, sel) }
        sel
    }
    fn message(&mut self, s: &str) { self.text.push(Text::Message(s.into())); self.wrap(s) }
    fn text(&mut self, s: &str) { self.text.push(Text::Text(s.into())) }
    fn name(&mut self, size: usize) -> Option<String> {
        let (mut buf, mut x) = (Vec::<char>::new(), 1i32);
        loop {
            let k = self.read_key();
            match k {
                13 => break, 27 => return None,
                8 => if let Some(c) = buf.pop() { x -= self.width(&c.to_string()) },
                32..=126 => { let c = k as u8 as char; let w = self.width(&c.to_string()); if buf.len() < size - 1 && x + w < 80 - 2 { buf.push(c); x += w } }
                _ => {}
            }
        }
        Some(buf.into_iter().collect())
    }
    fn draw_text_at(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, s: &str) { self.text.push(Text::Text(s.into())) }
    fn fill_rect(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) {}
    fn open_window(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) -> i16 { self.windows += 1; self.windows }
    fn close_window(&mut self, _: i16) {}
    fn select_window(&mut self, w: i16) { self.current = w }
    fn current_window(&self) -> i16 { self.current }
    fn reset_text(&mut self) {}
    fn set_text_color(&mut self, _: i16) {}
    fn set_text_mode(&mut self, _: i16) {}
    fn set_text_spacing(&mut self, s: i16) { self.spacing = s }
    fn set_line_height(&mut self, h: i16) { self.height = h }
    fn line_height(&mut self) -> i16 { let d = self.height - self.spacing; if d < 5 { 8 } else { d } }
    fn set_alignment(&mut self, _: i16) {}
    fn set_center(&mut self, _: i16) {}
    fn get_center(&mut self) -> i16 { 0 }
    fn draw_centered(&mut self, s: &str, _: i16) { self.text.push(Text::Text(s.into())) }
    fn string_width(&mut self, s: &str) -> i16 { self.width(s) as i16 }
    fn get_text_origin(&mut self) -> (i16, i16) { (0, 0) }
    fn read_key(&mut self) -> Key { Fake::read_key(self) }
    fn start_timer(&mut self) {}
    fn delay(&mut self, _: i16) -> bool { false }
    fn fatal_error(&mut self, s: &str) -> ! { panic!("fatal_error: {s}") }
    fn move_to(&mut self, _: i16, _: i16) {}
    fn get_text_pos(&mut self) -> (i16, i16) { (1, 1) }
    fn clear_window(&mut self) {}
    fn init_windows(&mut self) {}
    fn draw_hline(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16) {}
    fn draw_vline(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16) {}
    fn draw_border(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) {}
    fn put_char(&mut self, _: u8) {}
    fn char_width(&mut self, c: u8) -> i16 { self.widths.get(c as usize).copied().unwrap_or(0) as i16 }
    fn hmenu_repeat(&mut self, m: &Menu, on_select: SelectCb) -> i16 {
        self.title(m);
        for it in m.items { self.text.push(Text::Text(it.to_string())) }
        loop {
            let mut sel = 0i16;
            if !self.hmenu_keys(m, &mut sel, &mut None) { return -1 }
            if !on_select(self, sel) { return sel }
        }
    }
}

fn cg_image(oracle: &Path) -> Option<Ds> {
    if let Some(gf) = std::env::var_os("MT1_GAMEFILES") { if let Ok(b) = std::fs::read(PathBuf::from(gf).join("CG.DAT")) { return Ds::load(&b) } }
    std::fs::read(oracle.join("work/mt1/CG.exe")).ok().and_then(Ds::from_exe)
}
fn both_env() -> bool { std::env::var_os("MT1_GAMEFILES").is_some() && std::env::var_os("MT1_ORACLE").is_some() }
fn flow_fixtures() -> Option<PathBuf> {
    let Some(oracle) = oracle_env::oracle() else { return None };
    if !oracle.join("tools/golden/cg_golden.py").is_file() {
        assert!(std::env::var_os("MT1_ORACLE").is_none(), "MT1_ORACLE is set but {} has no tools/golden/cg_golden.py", oracle.display());
        println!("SKIP: oracle not found at {}", oracle.display()); return None
    }
    let out = common_m3::workspace().join("target/m3-flow-fixtures");
    let r = match std::process::Command::new("python").arg(common_m3::workspace().join("tools/m3_dump_flow.py")).arg(&oracle).arg("-").arg(&out).output() {
        Ok(r) => r,
        Err(e) => { assert!(!both_env(), "python not runnable with both env vars set: {e}"); println!("SKIP: python not runnable: {e}"); return None }
    };
    let so = String::from_utf8_lossy(&r.stdout).into_owned();
    assert!(r.status.success(), "m3_dump_flow.py failed:\n{so}{}", String::from_utf8_lossy(&r.stderr));
    if so.contains("PARTIAL") {
        assert!(!(std::env::var_os("MT1_GAMEFILES").is_some() && std::env::var_os("MT1_ORACLE").is_some()), "helper dropped the runs with both env vars set:\n{so}");
        println!("{so}"); return None
    }
    Some(out)
}
fn bytes(t: &Text) -> (String, Vec<u8>) {
    let b = |s: &str| s.chars().map(|c| c as u32 as u8).collect::<Vec<u8>>();
    match t { Text::Message(s) => ("message".into(), b(s)), Text::Text(s) => ("text".into(), b(s)), Text::Menu(i) => ("menu".into(), i.iter().map(|s| b(s)).collect::<Vec<_>>().join(&0u8)) }
}

#[test]
fn eight_sessions_match_the_original() {
    let Some(dir) = flow_fixtures() else { return };
    let Some(oracle) = oracle_env::oracle() else { return };
    let ds = cg_image(&oracle).expect("CG.DAT (MT1_GAMEFILES) or the oracle's work/mt1/CG.exe");
    let dchars = std::env::var_os("MT1_GAMEFILES").and_then(|g| std::fs::read(PathBuf::from(g).join("DCHARS.DAT")).ok())
        .or_else(|| std::fs::read(oracle.join("orig/dos/MegaTra1/DCHARS.DAT")).ok()).expect("DCHARS.DAT");
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |i| if !i.payload().is::<Exhausted>() { prev(i) }));
    let mut runs: Vec<PathBuf> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("run_")).collect();
    runs.sort();
    assert!(!runs.is_empty(), "no run fixtures in {}", dir.display());
    if both_env() && std::env::var_os("MT3_FLOW_RUNS").is_none() { assert_eq!(runs.len(), 8, "the 8 cg_golden RUNS") }
    let mut table = vec![];
    for run in &runs {
        let fx = parse(run);
        let mut files = Overlay::new(None);
        files.write("DCHARS.DAT", &dchars).unwrap();
        if fx.disk { files.write("UCHARS.DAT", &dchars).unwrap() }
        let mut fake = Fake { ds: &ds, rng: Rng::new(fx.seed), work: [0; 57], files, keys: VecDeque::new(), groups: fx.groups.iter().cloned().collect(), text: vec![], widths: fx.widths.clone(), steps: vec![], pool_rec: None, height: 8, spacing: 0, windows: 0, current: 0 };
        let mut cg = Cg::new(ds.clone());
        match catch_unwind(AssertUnwindSafe(|| main_menu(&mut cg, &mut fake))) {
            Err(p) if p.is::<Exhausted>() => {}
            Err(p) => resume_unwind(p),
            Ok(e) => panic!("{}: the flow left CG ({e:?}) before the script ended", run.display()),
        }
        let name = run.file_name().unwrap().to_string_lossy().into_owned();
        let mut differ = None::<String>;
        for (i, (want, got)) in fx.steps.iter().zip(fake.steps.iter()).enumerate() {
            let fail = if want.0 != got.0 { Some(format!("rand: original {:?}, rust {:?}", want.0, got.0)) }
                else if want.1 != got.1 { Some(format!("rand_state: original {}, rust {}", want.1, got.1)) }
                else if let Some(b) = (0..57).filter(|b| !(53..55).contains(b)).find(|&b| want.2[b] != got.2[b]) { Some(format!("work +{b}: original {:#04x}, rust {:#04x}", want.2[b], got.2[b])) }
                else {
                    let g: Vec<(String, Vec<u8>)> = got.3.iter().map(bytes).collect();
                    let show = |e: &(String, Vec<u8>)| format!("[{} {:?}]", e.0, String::from_utf8_lossy(&e.1));
                    (0..want.3.len().max(g.len())).find(|&k| want.3.get(k) != g.get(k)).map(|k| format!("text entry {k}: original {}, rust {}", want.3.get(k).map_or("<none>".into(), show), g.get(k).map_or("<none>".into(), show)))
                };
            if let Some(f) = fail { differ = Some(format!("step {i} {f}")); break }
        }
        if differ.is_none() && fx.steps.len() != fake.steps.len() { differ = Some(format!("steps: original {}, rust {}", fx.steps.len(), fake.steps.len())) }
        if differ.is_none() {
            if let Some((_, rec)) = &fx.pool {
                let got: Vec<(u8, u8)> = cg.skills.iter().map(|n| (n.id, n.level as u8)).collect();
                if got != fx.skills { differ = Some(format!("skill list: original {:?}, rust {:?}", fx.skills, got)) }
                else if fake.pool_rec.map(|r| r.to_vec()) != Some(rec.clone()) { differ = Some("pool record differs".into()) }
                else if fx.uchars.is_some() && fake.files.read("UCHARS.DAT") != fx.uchars { differ = Some("UCHARS.DAT differs".into()) }
            }
        }
        println!("| {name} | seed {} | {} steps | pool {} | {} |", fx.seed, fx.steps.len(), fx.pool.is_some(), differ.clone().unwrap_or("0 differ".into()));
        table.push((name, differ));
    }
    let bad: Vec<_> = table.iter().filter(|(_, d)| d.is_some()).collect();
    assert!(bad.is_empty(), "chargen flow differs from the original: {bad:?}");
    println!("PASS chargen flow: {} sessions, every step equal", table.len());
}

#[test]
fn disabled_unused_skills_are_never_granted() {
    use mt1::chargen::skills::{add_skill, choose_skills};
    use mt1::record::{set_no_unused_skills, UNUSED_SKILLS, W_SERVICE, W_STAT};
    let Some(oracle) = oracle_env::oracle() else { return };
    let Some(ds) = cg_image(&oracle) else { println!("SKIP: no CG.DAT (MT1_GAMEFILES) and no oracle work/mt1/CG.exe"); return };
    let unused = |id: u8| UNUSED_SKILLS.contains(&ds.u8(0x118B + 8 * id as usize));
    let fake = |seed: u16, groups: VecDeque<Vec<Key>>| Fake { ds: &ds, rng: Rng::new(seed), work: [0; 57], files: Overlay::new(None), keys: VecDeque::new(), groups, text: vec![], widths: vec![0; 128], steps: vec![], pool_rec: None, height: 8, spacing: 0, windows: 0, current: 0 };
    let run = |on: bool| {
        set_no_unused_skills(on);
        let (mut awards, mut ui) = (Cg::new(ds.clone()), fake(0, VecDeque::new()));
        for id in (0..95u8).filter(|&id| ds.skill_type(id) == 0) { assert_eq!(add_skill(&mut awards, &mut ui, id, 1) == 0, on && unused(id), "award of skill {id:#04x}, option {on}") }
        let mut granted = awards.skills.iter().filter(|n| unused(n.id)).count();
        for service in 0..5u8 { for seed in 0..40u16 {
            let mut pick = Rng::new(seed ^ 0x5A5A);
            let mut ui = fake(seed, (0..400).map(|_| { let mut g = vec![keys::DOWN; (pick.next() % 12) as usize]; g.push(13); g }).collect());
            ui.work[W_SERVICE] = service; ui.work[W_STAT + 4] = 12;
            let mut cg = Cg::new(ds.clone()); cg.earned = 60;
            assert!(choose_skills(&mut cg, &mut ui).is_ok());
            assert!(cg.skills.iter().map(|n| n.level as i32).sum::<i32>() > 0, "service {service} seed {seed} granted nothing");
            granted += cg.skills.iter().filter(|n| unused(n.id)).count();
        } }
        set_no_unused_skills(false);
        granted
    };
    let off = run(false);
    assert!(off > 0, "with the option off the same sessions do grant unused skills");
    assert_eq!(run(true), 0, "with the option on no unused skill is granted");
    println!("PASS disable unused skills: 200 sessions of 60 rolls, {off} unused skills off, 0 on");
}

#[test]
fn record_16_rewrites_the_draft_message() {
    let Some(oracle) = oracle_env::oracle() else { return };
    let Some(ds) = cg_image(&oracle) else { println!("SKIP: no CG.DAT (MT1_GAMEFILES) and no oracle work/mt1/CG.exe"); return };
    let (mut r15, mut r16) = (ds.image()[0x2740..0x2840].to_vec(), ds.image()[0x2840..0x2940].to_vec());
    r15[..15].fill(0); r15[..3].fill(1); r15[0x40] = 17;
    let orig_title = ds.str(0x27EC); let new_title = b"Jobs"; assert!(orig_title.len() >= new_title.len(), "{orig_title:?}");
    r15[0xAC..0xAC + new_title.len()].copy_from_slice(new_title); r15[0xAC + new_title.len()] = 0;
    let want_title: String = new_title.iter().map(|&b| b as char).collect(); assert_ne!(want_title, orig_title);
    let new = b"Conscripted. Report for duty now.\0"; r16[0x38..0x38 + new.len()].copy_from_slice(new);
    let want: String = r16[0x38..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
    let orig = ds.str(0x2878); assert!(orig.starts_with("You have been drafted") && want != orig, "{orig:?}");
    let mut file = vec![17u8]; for _ in 0..15 { file.extend_from_slice(&[0; 256]) } file.extend_from_slice(&r15); file.extend_from_slice(&r16);
    let mut files = Overlay::new(None); files.write("UCHARS.DAT", &file).unwrap();
    let keys: Vec<Vec<Key>> = [13, 13, 100, 13, 13, 100, 13, 13, 100, 13, 13, 99, 13, 13, 13, 109, 13].iter().map(|&k| vec![k as Key]).collect();
    let ui_ds = ds.clone();
    let mut fake = Fake { ds: &ui_ds, rng: Rng::new(4711), work: [0; 57], files, keys: VecDeque::new(), groups: keys.into(), text: vec![], widths: vec![0; 128], steps: vec![], pool_rec: None, height: 8, spacing: 0, windows: 0, current: 0 };
    let mut cg = Cg::new(ds);
    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |i| if !i.payload().is::<Exhausted>() { prev(i) }));
    match catch_unwind(AssertUnwindSafe(|| main_menu(&mut cg, &mut fake))) { Err(p) if p.is::<Exhausted>() => {} Err(p) => resume_unwind(p), Ok(e) => panic!("left CG: {e:?}") }
    assert_eq!((cg.pool_count(), cg.next_slot(), cg.ds.str(0x2878)), (14, 2, want.clone()));
    assert_eq!((ui_ds.str(0x27EC), ui_ds.str(0x2878), ui_ds.u8(0x2780)), (want_title.clone(), want.clone(), 14));
    let msgs: Vec<&String> = fake.steps.iter().flat_map(|s| &s.3).chain(&fake.text).filter_map(|t| if let Text::Message(m) = t { Some(m) } else { None }).collect();
    assert!(msgs.contains(&&want), "rewritten draft message not shown: {msgs:?}");
    assert!(!msgs.contains(&&orig), "original draft message shown: {msgs:?}");
    let texts: Vec<&String> = fake.steps.iter().flat_map(|s| &s.3).chain(&fake.text).filter_map(|t| if let Text::Text(m) = t { Some(m) } else { None }).collect();
    assert!(texts.contains(&&want_title), "rewritten service title not drawn through the UI's clone: {texts:?}");
    assert!(!texts.contains(&&orig_title), "original service title drawn: {texts:?}");
    println!("PASS record 16 at DS:2840: {want:?} displayed");
}

#[test]
fn zz_skip_report() { oracle_env::report() }
