mod oracle_env;
use mt1::space::{Space, sounds::{ENABLED, LOADED}, state::PARTY_SIZE};
use mt_engine::{io::{Dir, Input, Key}, sim::tick::{Clock, Counter}};
use std::{cell::RefCell, path::PathBuf, process::Command, rc::Rc};

type Plays = Rc<RefCell<Vec<(u16, u32, Vec<u8>, u8)>>>;
struct Output(Plays);
impl Input for Output {
    fn next_key(&mut self) -> Key { panic!("audio read input") }
    fn audio(&mut self, n: u16, hz: u32, levels: &[u8], rest: u8) { self.0.borrow_mut().push((n, hz, levels.to_vec(), rest)); }
}

#[test]
fn original_samples_and_space_state_invariance() {
    let Some((oracle, gamefiles)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let file = root.join("target/m6-sounds/samples.bin");
    let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "90", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("tools/m6_dump_sounds.py")).arg(oracle).arg(&gamefiles).arg(&file).output().unwrap();
    assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    let bytes = std::fs::read(file).unwrap(); let mut at = 0;
    let gamefiles = PathBuf::from(gamefiles);
    let s_dat = std::fs::read(gamefiles.join("S.DAT")).unwrap();
    let plays: Plays = Rc::default();
    let mut clock = Counter::default(); clock.set(12345);
    let mut sp = Space::new(&s_dat, vec![0; PARTY_SIZE], vec![13; 0x1300], Box::new(Output(plays.clone())), Box::new(clock), Box::new(Dir(gamefiles))).unwrap();
    sp.sounds.load(&sp.s, sp.files.as_ref());
    sp.s.set_byte(LOADED, 1); sp.s.set_byte(ENABLED, 1);
    sp.s.set_byte(mt1::space::state::KEY, 0x39);
    let mut count = 0;
    while at < bytes.len() {
        let word = |off| u16::from_le_bytes([bytes[at + off], bytes[at + off + 1]]);
        let (device, index) = (word(0), word(2));
        let n = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize; at += 8;
        sp.s.set_word(0x1A28, if device == 8 { 8 } else { -1 });
        let before = (sp.s.image().to_vec(), sp.s.party.clone(), sp.s.world.clone(), sp.ev, sp.last_ticks, sp.clock.now());
        assert!(sp.play_sound(index));
        assert_eq!(before, (sp.s.image().to_vec(), sp.s.party.clone(), sp.s.world.clone(), sp.ev, sp.last_ticks, sp.clock.now()));
        let (actual_index, hz, levels, rest) = plays.borrow_mut().pop().unwrap();
        assert_eq!((actual_index, hz, rest), (index, 5013, 0));
        assert_eq!(levels, bytes[at..at + n], "device {device} sample {index} play {count}");
        at += n; count += 1;
    }
    assert_eq!(count, 12);
    println!("PASS S sounds: 12 real clips, exact samples; full S DS/party/world/events/clock unchanged");
}

#[test]
fn zz_skip_report() { oracle_env::report() }
