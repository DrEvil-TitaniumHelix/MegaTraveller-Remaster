mod oracle_env;
#[path = "../../engine/tests/common_m2/mod.rs"]
mod common_m2;
use common_m2::workspace;
use oracle_env::{gamefiles, oracle};
use mt1::screens::title::Title;
use mt_engine::data::{exepack, ly, pic, Variant};
use mt_engine::present::fb::{PLANE, SNAP};
use std::path::PathBuf;
use std::process::Command;

struct Fixture { name: String, start: Vec<u8>, rand: Vec<u32>, keys: Vec<Option<u8>>, checks: Vec<(String, usize, Vec<u8>)> }
fn fixtures() -> Option<Vec<Fixture>> {
    let Some(o) = oracle() else { return None };
    if !o.join("tools/spec/test_render_startup.py").is_file() { println!("SKIP: oracle not found at {}", o.display()); return None }
    let out = workspace().join("target/m3-fixtures/title");
    let gf = std::env::var_os("MT1_GAMEFILES").map(|p| PathBuf::from(p).to_string_lossy().into_owned()).unwrap_or("-".into());
    let r = match Command::new("python").arg(workspace().join("tools/m3_dump_title.py")).arg(&o).arg(&gf).arg(&out).output() {
        Ok(r) => r, Err(e) => { println!("SKIP: python not runnable: {e}"); return None }
    };
    assert!(r.status.success(), "m3_dump_title.py failed:\n{}{}", String::from_utf8_lossy(&r.stdout), String::from_utf8_lossy(&r.stderr));
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&out).unwrap().flatten().map(|e| e.path()).filter(|p| p.join("checks.txt").is_file()).collect();
    dirs.sort();
    Some(dirs.into_iter().map(|dir| {
        let rd = |f: &str| std::fs::read(dir.join(f)).unwrap_or_else(|e| panic!("{}/{f}: {e}", dir.display()));
        let txt = |f: &str| String::from_utf8(rd(f)).unwrap();
        let start = rd("start.bin"); assert_eq!(start.len(), SNAP);
        let checks = txt("checks.txt").lines().map(|l| { let w: Vec<&str> = l.split(' ').collect(); (w[1].to_string(), w[2].parse().unwrap(), rd(&format!("{}.bin", w[0]))) }).collect();
        Fixture { name: dir.file_name().unwrap().to_string_lossy().into_owned(), start, rand: txt("rand.txt").lines().map(|l| l.parse().unwrap()).collect(),
            keys: txt("keys.txt").lines().map(|l| if l == "-" { None } else { Some(l.parse::<u32>().unwrap() as u8) }).collect(), checks }
    }).collect())
}
fn diff(name: &str, label: &str, got: &[u8], want: &[u8]) -> Option<String> {
    (0..4 * PLANE).find(|&i| got[i] != want[i]).map(|i| format!("{name} {label}: plane {} offset {} (page {} row {} byte {}): expected {:#04x} got {:#04x}; {} bytes differ", i / PLANE, i % PLANE, i % PLANE / 8192, i % PLANE % 8192 / 40, i % PLANE % 8192 % 40, want[i], got[i], (0..4 * PLANE).filter(|&i| got[i] != want[i]).count()))
}

#[test]
fn title_vs_original() {
    let Some(gf) = gamefiles() else { return };
    let Some(fx) = fixtures() else { return };
    assert_eq!(fx.len(), 4, "title cases");
    let startup = exepack::unpack(&std::fs::read(gf.join("STARTUP.EXE")).unwrap()).unwrap().exe;
    let load = |n: &str| ly::decode(&std::fs::read(gf.join(n)).unwrap()).unwrap();
    let pic = |n: &str| pic::decode(Variant::Mt1, &std::fs::read(gf.join(n)).unwrap()).unwrap();
    let mut fails = vec![];
    for f in &fx {
        let mut t = Title::new(&startup, load("TITLE.LY"), load("FONT.LY"), pic("TIT.PIC"), pic("ZOD.PIC")).unwrap();
        t.fb.planes.copy_from_slice(&f.start[..4 * PLANE]);
        assert_eq!(&t.fb.regs[..], &f.start[4 * PLANE..], "{}: palette from STARTUP DS:0094", f.name);
        let mut ri = 0; let mut rand = || { let v = f.rand[ri]; ri += 1; v };
        let secret = b"migraine"; let (mut frame, mut step, mut cursor, mut key) = (4usize, 1usize, 0usize, 0usize);
        let mut local = vec![];
        let check = |local: &mut Vec<String>, t: &Title, label: &str, page: usize, want: &[u8]| {
            if let Some(d) = diff(&f.name, label, &t.fb.planes, want) { local.push(d) }
            if t.display != page { local.push(format!("{} {label}: display page {} expected {page}", f.name, t.display)) }
        };
        for (label, page, want) in &f.checks {
            match label.as_str() {
                "starfield" => { t.star_field(&mut rand); check(&mut local, &t, label, *page, want) }
                "copy" => { t.copy_to_front(); check(&mut local, &t, label, *page, want) }
                "title_sprites" | "caption" | "TIT_mask" => {}
                "TIT_image" => { t.title_objects(); check(&mut local, &t, label, *page, want) }
                z if z.starts_with("Zhodani_") => {
                    t.frame(frame); check(&mut local, &t, label, *page, want); frame = (frame + step) & 7;
                    if let Some(k) = f.keys.get(key).copied().flatten() { if cursor < secret.len() && k == secret[cursor] { cursor += 1; if cursor == secret.len() { step = 7 } } }
                    key += 1;
                }
                other => local.push(format!("{}: unknown checkpoint {other}", f.name)),
            }
        }
        println!("{}: {} checkpoints", f.name, f.checks.len());
        fails.extend(local);
    }
    for d in &fails { println!("FAIL {d}") }
    assert!(fails.is_empty(), "{} title checkpoints differ", fails.len());
}

#[test]
fn zz_skip_report() { oracle_env::report() }
