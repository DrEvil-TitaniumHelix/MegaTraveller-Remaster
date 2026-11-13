mod oracle_env;
use mt1::space::{Space, state::State};
use mt_engine::{io::{Dir, Scripted}, sim::tick::Counter};
use std::{path::PathBuf, process::Command};
use std::collections::VecDeque;
use mt1::space::{guns::{StationUi, Rect, Result as UiResult}, tick::Hooks};

struct Keys(VecDeque<u8>);
impl Hooks for Keys {
    fn update_cursor(&mut self, sp: &mut Space) {
        if sp.s.byte(0x483A) == 0 { sp.keyboard_isr(self.0.pop_front().expect("control key tape exhausted")); }
    }
}
struct Ui(VecDeque<i16>);
impl StationUi for Ui {
    fn call(&mut self, _sp: &mut Space, name: &str, _args: &[i16]) -> UiResult<i16> {
        match name { "redraw_active_station" | "handle_station_key" => Ok(0), _ => Err(format!("unexpected service {name}")) }
    }
    fn select(&mut self, _sp: &mut Space, _boxes: &[Rect], _count: i16, _selected: i16, _hotkeys: Option<&[u8]>, _boarding: bool) -> UiResult<i16> {
        self.0.pop_front().ok_or("selection tape exhausted".into())
    }
}

#[test]
fn original_combat_state() {
    let Some((oracle, files)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/m6-fixtures/combat");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "120", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("tools/m6_dump_combat.py")).arg(oracle).arg(&out).output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let files = PathBuf::from(files);
    let mut sp = Space::new(&std::fs::read(files.join("S.DAT")).unwrap(), vec![0; 0xF86], vec![],
        Box::new(Scripted::new([])), Box::new(Counter::at(100000)), Box::new(Dir(files))).unwrap();
    for row in std::fs::read_to_string(out.join("cases.tsv")).unwrap().lines() {
        let columns: Vec<_> = row.split('\t').collect();
        let dir = out.join(columns[0]);
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        sp.s = State::from_ds(read("input-ds.bin"), read("input-party.bin"), vec![]).unwrap();
        let controlled = matches!(columns[1], "gun" | "board");
        let mut ui = Ui(VecDeque::new());
        if controlled {
            let keys = std::fs::read_to_string(dir.join("keys.txt")).unwrap();
            sp.hooks = Some(Box::new(Keys(keys.split_whitespace().map(|n| n.parse().unwrap()).collect())));
            ui.0 = std::fs::read_to_string(dir.join("selections.txt")).unwrap().split_whitespace().map(|n| n.parse().unwrap()).collect();
        }
        let args: Vec<i16> = columns[3..].iter().map(|s| s.parse().unwrap()).collect();
        let value = match columns[1] {
            "player" => sp.resolve_player_shot(args[0], args[1], args[2], args[3]),
            "enemy" => sp.resolve_enemy_shot(args[0], args[1], args[2]),
            "advance" => sp.advance_tactical_enemies(),
            "gun" => { sp.operate_gun_station(args[0], &mut ui).unwrap(); 0 }
            "board" => { sp.board_enemy_cargo(args[0], &mut ui).unwrap(); 0 }
            "land-missiles" => { sp.return_missiles_to_supplies(); 0 }
            _ => panic!("unknown fixture"),
        };
        if !controlled && columns[1] != "land-missiles" { assert_eq!(value as u16, columns[2].parse::<u16>().unwrap(), "case {} return", columns[0]); }
        let ds = read("end-ds.bin"); let party = read("end-party.bin");
        let first = sp.s.image().iter().zip(&ds).enumerate().find_map(|(i,(a,b))| {
            let gameplay = (0x188..0x1CD).contains(&i) || (0x343A..0x4619).contains(&i) || i == 0x4626 || i == 0x463C || i == 0x483A || (0x395..0x39F).contains(&i);
            (a != b && (!controlled || gameplay)).then_some(i)
        });
        assert_eq!(first, None, "case {} {} DS mismatch: {:?}", columns[0], columns[1], first.map(|i| (format!("{i:X}"), sp.s.image()[i], ds[i])));
        assert_eq!(sp.s.party, party, "case {} shared party", columns[0]);
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
