mod oracle_env;
use mt1::space::{Space, state::State};
use mt_engine::{io::{Dir, Input}, sim::tick::Counter};
use std::{cell::RefCell, rc::Rc, path::PathBuf, process::Command};

type Played = Rc<RefCell<Vec<(u16, u32, Vec<u8>, u8)>>>;
struct Audio(Played);
impl Input for Audio {
    fn next_key(&mut self) -> u16 { 27 }
    fn audio(&mut self, id: u16, hz: u32, levels: &[u8], rest: u8) { self.0.borrow_mut().push((id, hz, levels.to_vec(), rest)); }
}

#[test]
fn original_space_effects() {
    let Some((oracle, files)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/m6-fixtures/effects");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "120", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("tools/m6_dump_effects.py")).arg(oracle).arg(&out).output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let files = PathBuf::from(files); let played = Played::default();
    let mut sp = Space::new(&std::fs::read(files.join("S.DAT")).unwrap(), vec![0; 0xF86], vec![],
        Box::new(Audio(played.clone())), Box::new(Counter::default()), Box::new(Dir(files))).unwrap();
    sp.sounds.load(&sp.s, sp.files.as_ref());
    for row in std::fs::read_to_string(out.join("cases.tsv")).unwrap().lines() {
        let columns: Vec<_> = row.split('\t').collect(); let dir = out.join(columns[0]);
        let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
        sp.s = State::from_ds(read("input-ds.bin"), vec![0; 0xF86], vec![]).unwrap();
        let index = columns[1].parse().unwrap(); sp.combat_effect(index);
        assert_eq!(played.borrow_mut().pop().unwrap(), (index, 5013, read("levels.bin"), columns[2].parse().unwrap()));
    }
    sp.s.set_byte(0x4627, 0); sp.combat_effect(1); assert!(played.borrow().is_empty());
    sp.s.set_byte(0x4627, 1); sp.s.set_byte(0x3B78, 0); sp.combat_effect(2); assert!(played.borrow().is_empty());
}

#[test]
fn zz_skip_report() { oracle_env::report() }
