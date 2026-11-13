mod oracle_env;
use mt1::space::{Space, Events, state::State};
use mt_engine::{io::{Dir, Scripted}, sim::tick::Counter};
use std::{path::PathBuf, process::Command};

#[test]
fn original_encounter_frames() {
    let Some((oracle, files)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/m6-fixtures/encounter-frames");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "240", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("tools/m6_dump_encounter_frames.py")).arg(oracle).arg(&out).output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let files = PathBuf::from(files);
    let mut sp = Space::new(&std::fs::read(files.join("S.DAT")).unwrap(), vec![0; 0xF86], vec![],
        Box::new(Scripted::new([])), Box::new(Counter::at(100000)), Box::new(Dir(files))).unwrap();
    for row in std::fs::read_to_string(out.join("cases.tsv")).unwrap().lines() {
        let columns: Vec<_> = row.split('\t').collect(); let dir = out.join(columns[0]);
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        sp.s = State::from_ds(read("input-ds.bin"), read("input-party.bin"), vec![]).unwrap();
        sp.ev = Events::default();
        let before = read("input-vram.bin"); sp.r.fb.planes.copy_from_slice(&before[..262144]);
        sp.r.pen.colour = sp.s.byte(0x2A2A); sp.r.pen.op = sp.s.byte(0x2A14);
        match columns[1] {
            "map" => { sp.r.update_minimap_quadrant(&mut sp.s); sp.update_system_minimap(); }
            "radar" => { sp.redraw_tactical_radar(99); }
            "tactical-map" => { sp.s.set_byte(0x483A, 1); sp.show_tactical_map().unwrap(); }
            "gun-view" => { sp.update_gun_view_targets(columns[3].parse().unwrap()); }
            _ => panic!("unknown fixture"),
        }
        let expected = read("end-vram.bin");
        let first = sp.r.fb.planes.iter().zip(&expected).position(|(a,b)| a != b);
        assert_eq!(first, None, "case {} {} VRAM: {:?}", columns[0], columns[1], first.map(|i| (i / 65536, format!("{:X}", i % 65536), sp.r.fb.planes[i], expected[i])));
        let rands: u32 = std::fs::read_to_string(dir.join("rands.txt")).unwrap().parse().unwrap();
        assert_eq!(sp.ev.rands, rands, "case {} RNG calls", columns[0]);
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
