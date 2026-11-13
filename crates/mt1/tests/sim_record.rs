mod oracle_env;
#[path = "../../engine/tests/common_m3/mod.rs"]
mod common_m3;
use mt1::chargen::attributes::roll_attributes;
use mt1::record::{add_to_pool, load_pool, pool, save_pool, CgTables, Work};
use mt_engine::data::exepack;
use mt_engine::sim::rng::Rng;

#[test]
fn attributes_and_pool_match_cg_spec() {
    let Some(dir) = common_m3::fixtures() else { return };
    let mut n = 0;
    for line in std::fs::read_to_string(dir.join("attr.txt")).unwrap().lines().filter(|l| !l.trim().is_empty()) {
        let v = common_m3::ints(line);
        let mut r = Rng::new(v[0] as u16);
        assert_eq!(roll_attributes(&mut r).map(u64::from).to_vec(), v[1..], "seed {}", v[0]);
        assert_eq!(r.trace.len(), 12); n += 1;
    }
    assert_eq!(n, 8); println!("PASS roll_attributes 8 seeds");
    let Some(gf) = oracle_env::gamefiles() else { return };
    if !dir.join("tables.bin").is_file() { println!("SKIP: no pool fixtures (helper PARTIAL)"); return }
    let exe = exepack::unpack(&std::fs::read(gf.join("CG.DAT")).unwrap()).unwrap().exe;
    let t = CgTables::new(&exe).unwrap();
    assert_eq!(t.bytes(0x1184..0x1A00).unwrap(), &std::fs::read(dir.join("tables.bin")).unwrap()[..], "CG.DAT unpack != oracle CG.exe DS tables");
    let mut cases: Vec<_> = std::fs::read_dir(&dir).unwrap().flatten().map(|e| e.path()).filter(|p| p.file_name().unwrap().to_string_lossy().starts_with("pool_")).collect();
    cases.sort();
    assert_eq!(cases.len(), 24);
    let mut first = None;
    for c in &cases {
        let rd = |f: &str| std::fs::read(c.join(f)).unwrap();
        let w: Work = rd("work.bin").try_into().unwrap();
        let before: [u8; 256] = rd("before.bin").try_into().unwrap();
        let skills: Vec<(u8, u8)> = std::fs::read_to_string(c.join("skills.txt")).unwrap().lines().filter(|l| !l.trim().is_empty()).map(|l| { let v = common_m3::ints(l); (v[0] as u8, v[1] as u8) }).collect();
        let got = add_to_pool(&w, &skills, &t, &before);
        let want = rd("record.bin");
        if let Some(i) = (0..256).find(|&i| got[i] != want[i]) { panic!("{}: byte {i:#04x} want {:#04x} got {:#04x}", c.display(), want[i], got[i]) }
        first.get_or_insert(got);
        println!("PASS {}", c.file_name().unwrap().to_string_lossy());
    }
    let (mut p, mut ds) = (pool(), vec![0u8; 0x2840]);
    let (n, next) = load_pool(&mut p, &mut ds, &std::fs::read(gf.join("DCHARS.DAT")).unwrap());
    assert_eq!((n as usize, next as usize), (p.count(), p.count()));
    let slot = p.first_free().unwrap();
    assert!(p.put(slot, &first.unwrap()));
    assert_eq!(save_pool(&p).unwrap(), std::fs::read(dir.join("uchars.bin")).unwrap(), "UCHARS.DAT != cg_spec.save_pool");
    println!("PASS save_pool: {} records + 1 at slot {slot}", p.count() - 1);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
