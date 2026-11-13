use mt1::ground::{Ground, state::Fault};
use mt1::screens::cg::Screens;
use mt_engine::io::{script::{self, Line}, Dir, Files, Harness, Key, Overlay};
use mt_engine::sim::tick::Counter;
use std::path::Path;
use std::io::Read;

fn script_text(path: &Path) -> Result<String, String> {
    let read = || -> Result<String, String> {
        let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
        if f.metadata().map_err(|e| e.to_string())?.len() > script::MAX_BYTES as u64 { return Err(format!("script exceeds {} bytes", script::MAX_BYTES)) }
        let mut bytes = Vec::with_capacity(script::MAX_BYTES + 1);
        f.take((script::MAX_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        if bytes.len() > script::MAX_BYTES { return Err(format!("script exceeds {} bytes", script::MAX_BYTES)) }
        String::from_utf8(bytes).map_err(|e| e.to_string())
    };
    read().map_err(|e| format!("{}: {e}", path.display()))
}

pub fn screens(cg_exe: Vec<u8>, font_ly: Vec<mt_engine::data::ly::Entry>, cg_ly: Vec<mt_engine::data::ly::Entry>, keys: Vec<Key>, dump: &Path, seed: Option<u16>, disk: bool, dir: &Path) -> Result<Screens, String> {
    let mut o = Overlay::new(Some(dir));
    if disk { let d = o.read("DCHARS.DAT").ok_or("--disk: DCHARS.DAT missing")?; o.write("UCHARS.DAT", &d).map_err(|e| e.to_string())? } else { o.hide("UCHARS.DAT") }
    let input = Harness::new(keys, dump.to_path_buf(), true);
    let mut s = Screens::new(cg_exe, font_ly, cg_ly, Box::new(input), Box::new(Counter::default()), Box::new(Dir(dir.to_path_buf()))).ok_or("CG.DAT: tables not found")?;
    s.overlay = Some(o); s.rng.force = seed;
    Ok(s)
}
pub fn keys(path: &Path) -> Result<Vec<Key>, String> {
    let t = script_text(path)?;
    script::parse(&t).map_err(|(l, tok)| format!("{}:{l}: bad key {tok:?}", path.display()))
}
pub fn lines(path: &Path) -> Result<Vec<Line>, String> {
    let t = script_text(path)?;
    script::lines(&t).map_err(|(l, tok)| format!("{}:{l}: bad line {tok:?}", path.display()))
}
pub fn ground(t_dat: &[u8], party: Vec<u8>, lines: Vec<Line>, dump: &Path, seed: Option<u16>, dir: &Path) -> Result<Ground, String> {
    let prefix = lines.iter().take_while(|line| matches!(line, Line::Patch { .. } | Line::At { .. })).count();
    if lines[prefix..].iter().any(|line| matches!(line, Line::Patch { .. } | Line::At { .. })) { return Err("fixture patches and checkpoints must precede input lines".into()) }
    let fixture = lines[..prefix].iter().filter_map(|line| match line { Line::Patch { offset, value } => Some((*offset, *value)), _ => None }).collect();
    let lines = lines.into_iter().filter(|line|!matches!(line,Line::Patch{..})).collect::<Vec<_>>();
    let mouse_enabled = lines.iter().any(|line| matches!(line, Line::Mouse(_)));
    let input = Harness::lines(lines, dump.to_path_buf(), true);
    let mut g = Ground::new(t_dat, party, Box::new(input), Box::new(Counter::default()), Box::new(Dir(dir.to_path_buf()))).ok_or("T.DAT: not the EXEPACK program, or the party block is not 0xF86 bytes")?;
    g.fixture = fixture;
    g.overlay = Some(Overlay::new(Some(dir))); g.rand.force = seed; g.mem.trace.on = true; g.harness = true; g.rand.recording = true;
    g.mouse_driver = mouse_enabled;
    g.boot_diagnostics = std::env::var_os("MT4_BOOT_TRACE").is_some();
    if let Some(v) = std::env::var_os("MT4_FAULT") { g.fault_delay = Some(fault_delay(&v.to_string_lossy()).ok_or_else(|| format!("MT4_FAULT: expected delay_entry:<tick>, delay_body:<tick> or elapse_nested:<tick>, got {v:?}"))?) }
    Ok(g)
}
fn fault_delay(s: &str) -> Option<(Fault, u32)> {
    let (kind, tick) = s.split_once(':')?;
    Some((match kind { "delay_entry" => Fault::DelayEntry, "delay_body" => Fault::DelayBody, "elapse_nested" => Fault::ElapseNested, _ => return None }, tick.parse().ok()?))
}

#[cfg(test)]
mod ground_tests {
mod oracle_env { include!("../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    #[test]
    fn faithful_mouse_driver_requires_a_script_mouse_line() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let files = Dir(dir.clone());
        let t = files.read("T.DAT").expect("T.DAT missing");
        let party = files.read("DEFAULT.SAV").expect("DEFAULT.SAV missing");
        let mouse = mt_engine::io::MouseEvent { event: 4, buttons: 0, col: 210, row: 175 };
        for (lines, enabled) in [(vec![Line::Key(13), Line::Ticks(400)], 0), (vec![Line::Ticks(400), Line::Mouse(mouse)], 0xFFFF)] {
            let g = ground(&t, party.clone(), lines, Path::new("unused-dump"), Some(1), &dir).unwrap();
            assert_eq!(g.mem.u16(0x7000), 0);
            assert_eq!(g.mouse_driver, enabled != 0);
        }
    }
    #[test]
    fn ground_step_zero_precedes_exec_and_srand() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let files = Dir(dir.clone());
        let dat = files.read("T.DAT").unwrap(); let party = files.read("DEFAULT.SAV").unwrap();
        let dump = std::env::temp_dir().join(format!("mt-ground-preboot-{}", std::process::id()));
        let mut g = ground(&dat, party.clone(), vec![], &dump, Some(4711), &dir).unwrap();
        g.input = Box::new(Harness::lines([], dump.clone(), false));
        g.clock = Box::new(Counter::at(17)); g.mouse_driver = true;
        mt1::ground::start(&mut g);
        let step = dump.join("step_0000");
        let json = std::fs::read_to_string(step.join("step.json")).unwrap();
        assert!(json.contains("\"rand_state\":1,") && json.contains("\"now\":17,"));
        assert!(json.contains("\"text\":[]"));
        assert_eq!(std::fs::read(step.join("ds.bin")).unwrap(), mt1::ground::state::ds_image(&dat).unwrap());
        assert_eq!(std::fs::read(step.join("party.bin")).unwrap(), vec![0; mt1::ground::state::Mem::PARTY_SIZE]);
        assert_eq!(std::fs::read(step.join("planes.bin")).unwrap(), vec![0; 4 * 65536]);
        assert_eq!(std::fs::read(step.join("regs.bin")).unwrap(), (0..16).collect::<Vec<u8>>());
        assert_eq!(g.mem.party(), party);
        assert_eq!((g.rand.lcg.seed, g.mem.u32(0x5C20), g.mem.u16(0x7000)), (4711, 4711, 0xFFFF));
        assert_eq!(std::fs::read_dir(&dump).unwrap().count(), 1);
        std::fs::remove_dir_all(dump).unwrap();
    }
    #[test]
    fn fault_delay_forms() {
        assert_eq!(fault_delay("delay_entry:1063"), Some((Fault::DelayEntry, 1063))); assert_eq!(fault_delay("delay_body:1200"), Some((Fault::DelayBody, 1200)));
        assert_eq!(fault_delay("elapse_nested:1063"), Some((Fault::ElapseNested, 1063)));
        assert_eq!((fault_delay("delay_entry"), fault_delay("delay_late:1"), fault_delay("delay_body:-1")), (None, None, None));
    }
    #[test]
    fn script_file_limit() {
        let path = std::env::temp_dir().join(format!("mt-script-limit-{}.txt", std::process::id()));
        let f = std::fs::File::create(&path).unwrap(); f.set_len((script::MAX_BYTES + 1) as u64).unwrap(); drop(f);
        assert!(lines(&path).unwrap_err().contains("script exceeds")); assert!(keys(&path).is_err());
        std::fs::write(&path, "ticks --1").unwrap(); assert!(lines(&path).unwrap_err().contains(":1: bad line"));
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn ground_seed_initializes_borland_stream() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let files = Dir(dir.clone());
        let dat = files.read("T.DAT").unwrap(); let party = files.read("DEFAULT.SAV").unwrap();
        let mut streams = Vec::new();
        for seed in [1, 2, 65535] {
            let mut g = ground(&dat, party.clone(), vec![], &std::env::temp_dir().join("mt-seed-unused"), Some(seed), &dir).unwrap();
            assert_eq!(g.rand.lcg.seed, 1);
            g.harness = false;
            mt1::ground::start(&mut g);
            assert_eq!(g.rand.lcg.seed, seed as u32);
            let got: Vec<_> = (0..4).map(|_| g.rand.rand()).collect();
            let mut state = seed as u32;
            let want: Vec<_> = (0..4).map(|_| { state = state.wrapping_mul(22695477).wrapping_add(1); ((state >> 16) & 0x7FFF) as u16 }).collect();
            assert_eq!(got, want); streams.push(got);
        }
        assert_ne!(streams[0], streams[1]); assert_ne!(streams[1], streams[2]);
    }
}
