use mt1::{chargen::tables::{Cg, Ds}, ground::party};
use mt_engine::sim::rng::Rng;
use std::{fs, path::Path, process::Command};

fn rejects_missing_environment(mut command: impl FnMut() -> Command) {
    for name in ["MT1_ORACLE", "MT1_GAMEFILES"] {
        for value in [None, Some("")] {
            let mut child = command();
            child.env("MT1_ORACLE", "unused-oracle").env("MT1_GAMEFILES", "unused-gamefiles");
            if let Some(value) = value { child.env(name, value); } else { child.env_remove(name); }
            let result = child.output().expect("run proof prerequisite check");
            let log = format!("{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
            assert!(!result.status.success(), "{name}={value:?}: proof succeeded without prerequisites\n{log}");
            assert!(log.contains("FAIL") && log.contains(&format!("{name} is unset or empty")) && !log.contains("SKIP"),
                    "{name}={value:?}: missing explicit prerequisite failure\n{log}");
        }
    }
}

#[test]
fn rust_proof_rejects_missing_environment() {
    rejects_missing_environment(|| {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command.args(["--exact", "original_build_party", "--nocapture", "--test-threads=1"]);
        command
    });
}

#[test]
fn python_proof_rejects_missing_environment() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    rejects_missing_environment(|| {
        let mut command = Command::new("python");
        command.args(["-B", "-u"]).arg(root.join("tools/decode/cg_1d0a.py"))
            .env("PYTHONDONTWRITEBYTECODE", "1").current_dir(root);
        command
    });
}

#[test]
fn original_build_party() {
    for name in ["MT1_ORACLE", "MT1_GAMEFILES"] {
        assert!(std::env::var_os(name).is_some_and(|value| !value.is_empty()),
                "CG 1D0A proof: FAIL: {name} is unset or empty");
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap();
    let out = root.join("target/cg-1d0a-rust");
    let result = Command::new("python").args(["-B", "-u"])
        .arg(root.join("tools/decode/cg_1d0a.py")).arg("--out").arg(&out)
        .env("PYTHONDONTWRITEBYTECODE", "1").current_dir(root).output().expect("run Python byte proof");
    let log = String::from_utf8_lossy(&result.stdout);
    assert!(result.status.success() && log.contains("BYTE PROOF: PASS") && !log.contains("PARTIAL") && !log.contains("SKIP"),
            "{log}\n{}", String::from_utf8_lossy(&result.stderr));
    let manifest = fs::read_to_string(out.join("cases.txt")).unwrap();
    assert_eq!(manifest.lines().count(), 72, "PARTIAL fixture set");
    let mut bytes = 0;
    for name in manifest.lines() {
        let dir = out.join(name);
        let read = |file| fs::read(dir.join(file)).unwrap();
        let before = read("before.bin");
        let pool = read("pool.bin");
        let ds_image = read("ds.bin");
        let state = fs::read_to_string(dir.join("state.txt")).unwrap();
        let mut lines = state.lines();
        let words: Vec<u32> = lines.next().unwrap().split_whitespace().map(|x| x.parse().unwrap()).collect();
        let draws: Vec<u16> = lines.next().unwrap_or("").split_whitespace().map(|x| x.parse().unwrap()).collect();
        let mut exe = vec![0; 0x9240]; exe.extend_from_slice(&ds_image);
        let ds = Ds::from_exe(exe).unwrap();
        let records: Vec<Option<[u8; 256]>> = (0..5).map(|i| (words[0] & (1 << i) != 0)
            .then(|| pool[256*i..256*(i + 1)].try_into().unwrap())).collect();
        let mut cg = Cg::new(Ds::from_exe(vec![0; 0x10000]).unwrap());
        for (i, record) in records.iter().enumerate() {
            if let Some(record) = record {
                cg.pool.put(i, record); cg.party[i] = i as u8 + 1; cg.party_count += 1;
            }
        }
        let selected = party::selected(&cg);
        assert_eq!(selected, records, "{name}: selected pool records, including gaps");
        let mut rng = Rng::new(0); rng.state = words[1];
        let got = party::block(&before, &selected, &ds, &mut rng).unwrap();
        let want = read("party.bin");
        assert_eq!(got.len(), want.len(), "{name}");
        if let Some(i) = got.iter().zip(&want).position(|(a, b)| a != b) {
            panic!("{name}: block byte {i:04X}, Rust {:02X}, ORIGINAL {:02X}", got[i], want[i]);
        }
        assert_eq!(rng.state, words[2], "{name}: final RNG state");
        assert_eq!(rng.trace, draws, "{name}: ordered rand calls");
        if name.starts_with("run_") { assert_eq!(before, party::initial(), "{name}: T 822C launcher block") }
        let mut extra = records.clone(); extra.push(Some([0xFF; 256]));
        let mut rng_extra = Rng::new(0); rng_extra.state = words[1];
        assert_eq!(party::block(&before, &extra, &ds, &mut rng_extra).unwrap(), want);
        assert_eq!(rng_extra, rng);
        bytes += want.len();
    }
    println!("CG 1D0A Rust: 72 cases, {bytes} bytes, RNG stream/state: PASS (0 differ)");
}
