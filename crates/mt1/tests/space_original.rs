mod oracle_env;
use mt1::{lockstep::{SpaceEvent, SpaceObserver}, space::{self, Space, state::*, system, tick::{Hooks, SNAPSHOT}}};
use mt_engine::{data::stp, io::{Dir, Scripted}, sim::tick::Counter};
use std::{cell::RefCell, path::PathBuf, process::Command, rc::Rc};
type Trace = Rc<RefCell<Vec<(u16, Vec<i16>)>>>;

struct Reader { b: Vec<u8>, at: usize }
impl Reader {
    fn bytes(&mut self, n: usize) -> Vec<u8> { let b = self.b[self.at..self.at + n].to_vec(); self.at += n; b }
    fn u16(&mut self) -> u16 { let b = self.bytes(2); u16::from_le_bytes([b[0], b[1]]) }
    fn i16(&mut self) -> i16 { self.u16() as i16 }
    fn u32(&mut self) -> u32 { let lo = self.u16() as u32; lo | ((self.u16() as u32) << 16) }
    fn words(&mut self, n: usize) -> Vec<i16> { (0..n).map(|_| self.i16()).collect() }
}
fn fixture(name: &str) -> Option<Reader> {
    let Some((oracle, _)) = oracle_env::both() else { return None };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("target/m6-fixtures/space");
    let path = dir.join(format!("{name}.bin"));
    if !path.is_file() {
        let result = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py")).args(["--seconds", "1800", "--mib", "2048", "--", "python", "-B"])
            .arg(root.join("tools/m6_dump_space.py")).arg(oracle).arg(&dir).arg(name).output().expect("Python required with oracle env");
        assert!(result.status.success(), "{}{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
    }
    Some(Reader { b: std::fs::read(path).unwrap(), at: 0 })
}
fn gamefiles() -> PathBuf { PathBuf::from(std::env::var_os("MT1_GAMEFILES").unwrap()) }
fn space() -> Space {
    let files = gamefiles();
    let s_dat = std::fs::read(files.join("S.DAT")).unwrap();
    Space::new(&s_dat, vec![0; PARTY_SIZE], vec![], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(files))).unwrap()
}

#[test]
fn original_systems() {
    let Some(mut r) = fixture("systems") else { return };
    let mut cases = 0;
    while r.at < r.b.len() {
        let (sys, world, clock) = (r.u16() as u8, r.u16() as u8, r.i16());
        let mut s = State::from_ds(vec![], vec![0; PARTY_SIZE], vec![]).unwrap();
        s.set_pbyte(7, world); s.set_word(WORLD_TIME, clock);
        for (off, n) in [(system::RADIUS, 90), (system::OBJECT_X, 80), (system::OBJECT_Y, 80)] {
            for (i, v) in r.words(n).into_iter().enumerate() { s.set_array(off, i as i16, v); }
        }
        let data = std::fs::read(gamefiles().join(format!("{sys}.STP"))).unwrap();
        system::load_decoded(&mut s, &stp::decode(&data).unwrap(), sys).unwrap();
        let expected = r.words(5);
        let actual: Vec<_> = [system::COUNT, system::DEFAULT_X, system::DEFAULT_Y, X, Y].map(|a| s.word(a)).into();
        assert_eq!(actual, expected, "case {cases} system {sys} world {world} clock {clock}");
        for off in [system::WORLD, system::ORBIT, system::PHASE, system::KIND, system::MOONS, system::SPRITE, system::RADIUS, system::OBJECT_X, system::OBJECT_Y] {
            let expected = r.words(s.word(system::COUNT) as usize);
            let actual: Vec<_> = (0..s.word(system::COUNT)).map(|i| s.array(off, i)).collect();
            assert_eq!(actual, expected, "case {cases} array {off:X}");
        }
        assert_eq!(s.byte(0x147D) as u16, r.u16()); cases += 1;
    }
    assert_eq!(cases, 300);
}

struct Clocks { clocks: Vec<u32>, at: usize, trace: Trace }
impl SpaceObserver for Clocks {
    fn read(&mut self, _sp: &mut Space) -> Option<u32> { let v = self.clocks[self.at]; self.at += 1; self.trace.borrow_mut().push((0, vec![v as i16, (v >> 16) as i16])); Some(v) }
    fn event(&mut self, _sp: &mut Space, _e: SpaceEvent) {}
    fn srand(&mut self, _sp: &mut Space) -> Option<u16> { None }
    fn stop(&mut self, _sp: &mut Space, _reason: &str) {}
}
struct Cursor { count: usize, after: usize, key: u8, radar: i16, trace: Trace }
impl Hooks for Cursor {
    fn update_cursor(&mut self, sp: &mut Space) { if self.count >= self.after { sp.s.set_byte(KEY, self.key); } self.count += 1; self.trace.borrow_mut().push((1, vec![])); }
    fn hide_cursor(&mut self, _sp: &mut Space) { self.trace.borrow_mut().push((2, vec![])); }
    fn show_cursor(&mut self, _sp: &mut Space) { self.trace.borrow_mut().push((3, vec![])); }
    fn redraw_tactical(&mut self, _sp: &mut Space, mode: i16) -> Option<i16> { self.trace.borrow_mut().push((4, vec![mode])); Some(self.radar) }
    fn redraw_active_station(&mut self, _sp: &mut Space) { self.trace.borrow_mut().push((5, vec![])); }
}
#[test]
fn original_scheduler() {
    let Some(mut r) = fixture("scheduler") else { return };
    let mut sp = space();
    let mut cases = 0;
    while r.at < r.b.len() {
        let (function, arg, cursor_after, key, radar, n) = (r.u16(), r.i16(), r.u16(), r.u16() as u8, r.i16(), r.u16());
        let clocks = (0..n).map(|_| r.u32()).collect();
        sp.s = State::from_ds(r.bytes(65536), r.bytes(PARTY_SIZE), vec![]).unwrap();
        let expected_ds = r.bytes(65536); let expected_party = r.bytes(PARTY_SIZE);
        let result = r.i16(); let n = r.u16();
        let trace: Vec<_> = (0..n).map(|_| { let id = r.u16(); let len = r.u16() as usize; (id, r.words(len)) }).filter(|(id, _)| *id <= 5).collect();
        let recorded: Trace = Rc::new(RefCell::new(vec![]));
        sp.observer = Some(Box::new(Clocks { clocks, at: 0, trace: recorded.clone() }));
        sp.hooks = Some(Box::new(Cursor { count: 0, after: cursor_after as usize, key, radar, trace: recorded.clone() }));
        sp.ev = space::Events::default();
        let got = match function {
            0x5EE5 => Some(sp.run_tasks(arg)),
            0x5E3C => { sp.snapshot_clock(); None }
            0x5E49 => sp.wait_with_input(arg),
            _ => panic!("unknown function"),
        };
        if let Some(got) = got { assert_eq!(got, result, "case {cases} function {function:X} return"); }
        sp.observer = None; sp.hooks = None;
        assert_eq!(*recorded.borrow(), trace, "case {cases} function {function:X} ordered clock reads and cursor calls");
        for (i, (&got, &expected)) in sp.s.image().iter().zip(&expected_ds).enumerate() {
            if (0x2792..0x2796).contains(&i) || (0x2A15..0x2A17).contains(&i) || (0x49EE..0x49F0).contains(&i) { continue; }
            assert_eq!(got, expected, "case {cases} function {function:X} DS:{i:X}");
        }
        assert_eq!(sp.s.party, expected_party, "case {cases} function {function:X} party");
        cases += 1;
    }
    assert_eq!(cases, 632);
}

#[test]
fn wait_input_preserves_the_tested_branch() {
    if oracle_env::gamefiles().is_none() { return; }
    struct InputPhase { phase: SpaceEvent, clocks: std::collections::VecDeque<u32> }
    impl SpaceObserver for InputPhase {
        fn read(&mut self, _sp: &mut Space) -> Option<u32> { Some(self.clocks.pop_front().expect("unexpected clock read")) }
        fn event(&mut self, sp: &mut Space, event: SpaceEvent) { if event == self.phase { sp.keyboard_isr(0x4D); } }
        fn srand(&mut self, _sp: &mut Space) -> Option<u16> { None }
        fn stop(&mut self, _sp: &mut Space, _reason: &str) {}
    }
    for late in [false, true] {
        let mut sp = space();
        sp.s.set_byte(KEY, 0); sp.s.set_byte(PENDING_KEY, 0); sp.s.set_dword(SNAPSHOT, 5499);
        sp.observer = Some(Box::new(InputPhase {
            phase: if late { SpaceEvent::WaitKeyTest } else { SpaceEvent::Cursor },
            clocks: [5499, 5500, 5501].into(),
        }));
        assert_eq!(sp.wait_with_input(2), if late { None } else { Some(1) });
        assert_eq!((sp.ev.reads, sp.ev.cursor), (if late { 3 } else { 2 }, 1));
        assert_eq!(sp.s.dword(SNAPSHOT), if late { 65535 } else { 5499 });
        assert_eq!((sp.s.byte(KEY), sp.s.byte(PENDING_KEY)), if late { (0x4D, 0) } else { (0, 0x4D) });
    }
}

#[test]
fn original_render() {
    let Some(mut r) = fixture("render") else { return };
    let mut sp = space();
    let mut cases = 0;
    const RANGES: [(u16, usize); 16] = [(0x188, 4), (0x1B6, 2), (0x1C3, 8), (0x343, 18), (0x347A, 12), (0x34B8, 2), (0x34C8, 6), (0x3578, 160), (0x36D0, 2), (0x3702, 160), (0x37D6, 2), (0x37DC, 160), (0x387C, 2), (0x3A04, 160), (0x3B79, 160), (0x3C19, 160)];
    while r.at < r.b.len() {
        let function = r.u16(); let mode = r.i16();
        sp.s = State::from_ds(r.bytes(65536), r.bytes(PARTY_SIZE), vec![]).unwrap();
        sp.r.fb.planes = r.bytes(4 * 65536);
        let expected_pixels = r.bytes(4 * 65536);
        let expected_game = r.bytes(RANGES.iter().map(|r| r.1).sum::<usize>() + PARTY_SIZE);
        let answer = r.i16();
        match function {
            0x4B30 => sp.hud(mode),
            0x3E49 => assert_eq!(sp.draw_system_view(mode != 0), answer, "case {cases} return"),
            _ => panic!("unknown function"),
        }
        let first = sp.r.fb.planes.iter().zip(&expected_pixels).position(|(a, b)| a != b);
        assert_eq!(first.map(|i| (i / 65536, i % 65536)), None, "case {cases} function {function:X} mode {mode}: first differing (plane, offset)");
        let actual: Vec<u8> = RANGES.iter().flat_map(|&(off, n)| (0..n).map(move |i| off + i as u16)).map(|off| sp.s.byte(off)).chain(sp.s.party.iter().copied()).collect();
        if let Some(i) = actual.iter().zip(&expected_game).position(|(a, b)| a != b) { panic!("case {cases} function {function:X} game byte {i}: {} vs {}", actual[i], expected_game[i]); }
        cases += 1;
    }
    assert_eq!(cases, 160);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
