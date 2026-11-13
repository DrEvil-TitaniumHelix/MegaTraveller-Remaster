mod oracle_env { include!("../../mt1/tests/oracle_env/mod.rs"); }
use super::scenario;
use mt1::{ground::{main_loop, Ground, Tooling}, save::{Mode, Saved}};
use mt_engine::{io::{keys, Event, Input, Key}, sim::tick::Clock};
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};

struct Walk { events: VecDeque<Event>, done: bool, polls: usize, seen: Rc<RefCell<Vec<String>>>,  list: Rc<RefCell<Vec<(i64, i64, u8)>>> }
impl Walk {
    fn new(seed: u32, n: usize) -> Walk {
        let (mut s, mut events) = (seed, VecDeque::new());
        for _ in 0..n {
            s = s.wrapping_mul(22695477).wrapping_add(1); let r = (s >> 16) & 0x7fff;
            if r % 5 == 0 { let k = [keys::UP, keys::DOWN, keys::LEFT, keys::RIGHT, keys::HOME, keys::PGUP, keys::END, keys::PGDN][(r / 5 % 8) as usize]; events.push_back(Event::Key(k)); events.push_back(Event::KeyUp(k)) }
            else { events.push_back(Event::Idle) }
        }
        Walk { events, done: false, polls: 0, seen: Default::default(), list: Default::default() }
    }
}
impl Input for Walk {
    fn next_key(&mut self) -> Key { keys::ESC }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        self.polls += 1; assert!(self.polls < 400_000, "walk: runaway");
        match self.events.pop_front() { Some(Event::Idle) => { clock.wait_tick(); Event::Idle } Some(e) => e, None => { self.done = true; clock.wait_tick(); Event::Key(keys::ESC) } }
    }
    fn closed(&self) -> bool { self.done }
    fn present(&mut self, rgb: &[u32]) { let Some(log) = mt_engine::present::textlog::take_frame(rgb) else { return }; let t = log.visible.iter().map(|r| String::from_utf8_lossy(&r.bytes).into_owned()).collect::<Vec<_>>().concat();
        if t.contains(mt1::save::SAVE_TITLE) && log.visible.len() >= self.list.borrow().len() { *self.list.borrow_mut() = log.visible.iter().filter_map(|r| Some((r.y, r.x, *r.bytes.first()?))).collect() }
        if !t.is_empty() { self.seen.borrow_mut().push(t) } }
}
struct Probe { polls: usize, at: usize, cont: usize, through_bytes: bool, out: Rc<RefCell<Vec<Saved>>> }
impl Tooling for Probe {
    fn poll(&mut self, g: &mut Ground) -> bool {
        self.polls += 1;
        if self.polls == self.at {
            let s = Saved::capture(g, Mode::Ground, "test"); self.out.borrow_mut().push(s.clone());
            if self.through_bytes { let d = Saved::decode(&s.encode()).unwrap(); assert_eq!(d, s); d.install(g); g.restore_pending = None; }
            return true
        }
        if self.polls == self.at + self.cont { self.out.borrow_mut().push(Saved::capture(g, Mode::Ground, "test")) }
        false
    }
    fn space_launched(&mut self, _: &mut mt1::space::Space) -> Result<(), String> { Err("identity test: S has no 1990 save point".into()) }
}
fn run(path: &std::path::Path, dir: &std::path::Path, seed: u32, through_bytes: bool) -> Vec<Saved> {
    let mut g = scenario::load(path, dir).unwrap_or_else(|e| panic!("{e}")).game;
    g.input = Box::new(Walk::new(seed, 700));
    let out = Rc::new(RefCell::new(vec![]));
    g.tooling = Some(Box::new(Probe { polls: 0, at: 50, cont: 250, through_bytes, out: out.clone() }));
    main_loop::run(&mut g);
    eprintln!("walk {} seed {seed} bytes {}: game_over={} x={} y={} ticks={}", path.file_name().unwrap().to_string_lossy(), through_bytes, g.game_over, g.mem.x(), g.mem.y(), g.clock.ticks());
    drop(g); Rc::try_unwrap(out).ok().unwrap().into_inner()
}
#[test]
fn starport_save_then_continue_old_game_resumes_the_panel() {
    use mt1::{ground::walk, save};
    use mt_engine::{io::Dir, sim::tick::Counter};
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    let saves = std::env::temp_dir().join(format!("mt-save-panel-{}", std::process::id()));
    let seen: Rc<RefCell<Vec<String>>> = Default::default(); mt_engine::present::textlog::set_enabled(true);
    let script = |keys: &[Key]| -> Box<dyn Input> { let mut w = Walk::new(0, 0); w.seen = seen.clone(); for &k in keys { w.events.extend([Event::Idle, Event::Key(k), Event::KeyUp(k), Event::Idle, Event::Idle]) } w.events.extend(std::iter::repeat(Event::Idle).take(60)); Box::new(w) };
    let mut a = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game;
    a.save_slot = Some(save::Slot { dir: saves.clone(), build: "test" });
    a.input = script(&[keys::RIGHT, keys::ENTER, b'A' as Key, keys::ESC]);
    a.mem.set_event(9); walk::world_event(&mut a); a.mem.set_event(-1);
    let file = saves.join(save::FILE); assert!(file.exists(), "no save written");
    mt_engine::present::textlog::set_enabled(false);
    assert!(seen.borrow().iter().any(|t| t.contains("Game saved.")), "no presented frame shows Game saved.: {:?}", seen.borrow());
    let after_a = Saved::capture(&mut a, Mode::Ground, "test");
    let saved = save::Saved::decode(&std::fs::read(&file).unwrap()).unwrap(); assert_eq!(saved.mode, Mode::Starport);
    let mut b = save::ground(saved.clone(), script(&[keys::ESC]), Box::new(Counter::at(saved.ticks)), Box::new(Dir(dir.clone())));
    assert_eq!(b.mem.ds, saved.ds, "save::ground installs the image before the restart's preamble runs");
    assert!(save::resume(&mut b)); assert!(b.restore_pending.is_none());
    let after_b = Saved::capture(&mut b, Mode::Ground, "test");
    std::fs::remove_dir_all(&saves).unwrap();
    let diffs: Vec<usize> = after_a.ds.iter().zip(&after_b.ds).enumerate().filter(|(_, (p, q))| p != q).map(|(o, _)| o).collect();
    eprintln!("starport save: {} DS bytes differ after the panel exit: {:x?}", diffs.len(), diffs.iter().take(24).collect::<Vec<_>>());
    assert_eq!((after_a.segs.get(&0x9000), after_a.seed, a.mem.x(), a.mem.y(), a.mem.event()), (after_b.segs.get(&0x9000), after_b.seed, b.mem.x(), b.mem.y(), b.mem.event()));
    assert!(diffs.iter().all(|&o| (0x4bde..0x4be2).contains(&o) || (0x503a..0x503e).contains(&o) || o == 0x52b4 || o == 0x52f4 || o == 0x6fe5), "DS differs outside render / clock bookkeeping: {:x?}", diffs);
    let ticks = saved.ticks;
    let mut c = save::ground(saved, Box::new(Walk { events: [Event::Idle, Event::Key(keys::ESC), Event::KeyUp(keys::ESC)].into(), done: false, polls: 0, seen: Default::default(), list: Default::default() }), Box::new(Counter::at(ticks)), Box::new(Dir(dir)));
    main_loop::run(&mut c);
    assert!(c.restore_pending.is_none() && c.input.closed());
    let after_c = Saved::capture(&mut c, Mode::Ground, "test");
    let diffs: Vec<usize> = after_b.ds.iter().zip(&after_c.ds).enumerate().filter(|(_, (p, q))| p != q).map(|(o, _)| o).collect();
    eprintln!("continue old game: {} DS bytes differ from the direct resume: {:x?}", diffs.len(), diffs.iter().take(24).collect::<Vec<_>>());
    assert_eq!((after_c.segs.get(&0x9000), after_c.seed, c.mem.x(), c.mem.y(), c.mem.event()), (after_b.segs.get(&0x9000), after_b.seed, b.mem.x(), b.mem.y(), b.mem.event()));
    assert_eq!(after_c.segs, after_b.segs);
    assert!(diffs.iter().all(|&o| o == 0xe3 || (0x503a..0x503e).contains(&o) || o == 0x52b4 || o == 0x52f4 || (0x6b73..0x6b75).contains(&o) || o == 0x6fe5 || o == 0x7010 || (0x707d..0x7098).contains(&o) || (0x7139..0x713d).contains(&o) || (0x7196..0x71a2).contains(&o)), "DS differs outside the main loop's render / input bookkeeping: {:x?}", diffs);
}
#[test]
fn product_save_asks_for_no_drive_faithful_still_does() {
    use mt1::{ground::walk, save};
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    for product in [true, false] {
        let saves = std::env::temp_dir().join(format!("mt-save-nodrive-{}-{product}", std::process::id()));
        let seen: Rc<RefCell<Vec<String>>> = Default::default(); mt_engine::present::textlog::set_enabled(true);
        let mut w = Walk::new(0, 0); w.seen = seen.clone();
        for &k in if product { &[keys::RIGHT, keys::ENTER, keys::ENTER, keys::ESC][..] } else { &[keys::RIGHT, keys::ENTER, keys::ESC] } { w.events.extend([Event::Idle, Event::Key(k), Event::KeyUp(k), Event::Idle, Event::Idle]) }
        let mut g = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game;
        g.save_slot = Some(save::Slot { dir: saves.clone(), build: "test" }); g.input = Box::new(w);
        if product { g.scene = Some(mt1::ui::SceneHook::default()) } else { g.scene = None }
        g.mem.set_event(9); walk::world_event(&mut g);
        mt_engine::present::textlog::set_enabled(false);
        let (seen, written) = (seen.borrow(), saves.join(save::FILE).exists()); let _ = std::fs::remove_dir_all(&saves);
        let has = |t: &str| seen.iter().any(|s| s.contains(t));
        assert_eq!((has("Select drive"), has("Game saved."), written), (!product, product, product), "product {product}: {seen:?}");
        if product { let saved = seen.iter().position(|s| s.contains("Game saved.")).unwrap(); assert!(seen[saved..].iter().any(|s| s.contains("EFATE STARPORT") && !s.contains("Game saved.")), "the starport list returns after Game saved.") }
    }
}
#[test]
fn product_save_and_restore_list_the_positions() {
    use mt1::{ground::walk, save};
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    let saves = std::env::temp_dir().join(format!("mt-save-list-{}", std::process::id())); let _ = std::fs::remove_dir_all(&saves);
    let seen: Rc<RefCell<Vec<String>>> = Default::default(); mt_engine::present::textlog::set_enabled(true);
    let rows: Rc<RefCell<Vec<(i64, i64, u8)>>> = Default::default();
    let mut g = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game;
    g.save_slot = Some(save::Slot { dir: saves.clone(), build: "test" }); g.scene = Some(mt1::ui::SceneHook::default());
    let desk = |g: &mut Ground, event: i8, ks: &[Key]| {
        let mut w = Walk::new(0, 0); w.seen = seen.clone(); w.list = rows.clone();
        for &k in ks { if k != 0 { w.events.extend([Event::Idle, Event::Key(k), Event::KeyUp(k)]) } w.events.extend(std::iter::repeat(Event::Idle).take(if k == 0 { 40 } else { 14 })) }
        g.input = Box::new(w); g.mem.set_event(event); walk::world_event(g); g.mem.set_event(-1);
    };
    let three = saves.join("game3.mtsav");
    desk(&mut g, 9, &[keys::RIGHT, keys::ENTER, keys::DOWN, keys::DOWN, keys::ENTER]);
    let first = std::fs::read(&three).expect("position 3 written"); assert_eq!(Saved::decode(&first).unwrap().mode, Mode::Starport);
    assert!(!saves.join(save::FILE).exists() && !saves.join("game3.mtsav.tmp").exists());
    std::fs::write(&three, b"old").unwrap();
    desk(&mut g, 9, &[keys::RIGHT, keys::ENTER, keys::DOWN, keys::DOWN, keys::ENTER, keys::ENTER]);
    assert_eq!(std::fs::read(&three).unwrap(), b"old", "No keeps the old file");
    desk(&mut g, 9, &[keys::RIGHT, keys::ENTER, keys::DOWN, keys::DOWN, keys::ENTER, keys::DOWN, keys::ENTER]);
    assert!(Saved::decode(&std::fs::read(&three).unwrap()).is_ok(), "Yes replaces it");
    desk(&mut g, 9, &[keys::RIGHT, keys::ENTER, keys::ENTER, 0, keys::ENTER, keys::DOWN, keys::ENTER]);
    assert!(saves.join(save::FILE).exists() && saves.join("game2.mtsav").exists(), "two saves in one visit to the desk");
    assert!(g.restore_pending.is_none());
    desk(&mut g, 20, &[keys::DOWN, keys::DOWN, keys::ENTER]);
    mt_engine::present::textlog::set_enabled(false);
    assert_eq!(g.restore_pending.as_ref().map(|s| s.encode()), Some(std::fs::read(&three).unwrap()), "Restore loads the picked position");
    let _ = std::fs::remove_dir_all(&saves);
    let seen = seen.borrow(); let has = |t: &str| seen.iter().any(|s| s.contains(t));
    for t in ["Save to which position?", "Game 2Empty", "Game 8Empty", "Overwrite position 3?", "Game saved.", "Cr 451,002", "Restore which game?"] { assert!(has(t), "no frame shows {t:?}: {seen:?}") }
    assert!(["Efate starport", "Efate"].iter().any(|place| has(&format!("Game 3{place}Day 0  Cr 451,002  "))), "no frame shows the saved row: {seen:?}");
    let rows = rows.borrow(); let mut ys: Vec<i64> = rows.iter().filter(|r| r.0 >= 5 + 32).map(|r| r.0).collect(); ys.sort(); ys.dedup();
    assert_eq!(ys, (0..8).map(|i| 5 + 32 + i * 8).collect::<Vec<_>>(), "eight rows, one line a position: {rows:?}");
    let starts = |y: i64| { let mut xs: Vec<(i64, u8)> = rows.iter().filter(|r| r.0 == y && r.2 != 5).map(|r| (r.1, r.2)).collect(); xs.sort();
        let mut cols = vec![xs[0]]; for w in xs.windows(2) { if w[1].0 > w[0].0 + 9 { cols.push(w[1]) } } cols };
    let used: Vec<Vec<(i64, u8)>> = [0, 2].map(|i| starts(37 + i * 8)).to_vec();
    assert_eq!(used.iter().map(|u| u.iter().map(|c| c.1).collect::<Vec<_>>()).collect::<Vec<_>>(), [[b'G', b'E', b'D'], [b'G', b'E', b'D']], "label | place | Day ...: {used:?}");
    assert!(used[0].iter().zip(&used[1]).all(|(a, b)| a.0 == b.0), "columns start at one x: {used:?}");
    assert_eq!(starts(37 + 8)[1].0, used[0][1].0, "an Empty row sits in the same columns");
    assert!(rows.iter().all(|r| r.1 < 315), "a row ends inside the box");
    assert!(!has("Select drive"));
}
#[test]
fn quick_save_then_quick_load_round_trip() {
    use mt1::save;
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    let (f6, f9) = (keys::ext(save::KEY_QUICK_SAVE), keys::ext(save::KEY_QUICK_LOAD));
    let run = |product: bool, load: bool| {
        let saves = std::env::temp_dir().join(format!("mt-save-quick-{}-{product}-{load}", std::process::id())); let _ = std::fs::remove_dir_all(&saves);
        let seen: Rc<RefCell<Vec<String>>> = Default::default(); mt_engine::present::textlog::set_enabled(true);
        let mut g = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game;
        g.save_slot = Some(save::Slot { dir: saves.clone(), build: "test" }); g.scene = product.then(mt1::ui::SceneHook::default);
        let start = (g.mem.x(), g.mem.y());
        let mut w = Walk::new(0, 0); w.seen = seen.clone();
        let mut ks = vec![f6, keys::RIGHT, keys::RIGHT]; if load { ks.extend([f9, keys::DOWN, keys::ENTER]) }
        for k in ks { w.events.extend([Event::Idle, Event::Key(k), Event::KeyUp(k)]); w.events.extend(std::iter::repeat(Event::Idle).take(14)) }
        g.input = Box::new(w); main_loop::run(&mut g);
        mt_engine::present::textlog::set_enabled(false);
        let quick = std::fs::read(saves.join(save::QUICK)).ok().map(|b| Saved::decode(&b).unwrap().mode); let _ = std::fs::remove_dir_all(&saves);
        let seen = seen.borrow(); let has = |t: &str| seen.iter().any(|s| s.contains(t));
        (start, (g.mem.x(), g.mem.y()), quick, has("Quick-saved"), has("Load the quick-save?NoYes"))
    };
    let (start, end, quick, said, asked) = run(true, false);
    assert_eq!((quick, said, asked), (Some(Mode::Ground), true, false)); assert_ne!(start, end, "the two steps move the party");
    let (start, end, quick, said, asked) = run(true, true);
    assert_eq!((start, quick, said, asked), (end, Some(Mode::Ground), true, true), "Yes puts the party back where F6 saved it");
    assert_eq!(run(false, true).2, None, "faithful: F6 / F9 are the 1990's (nothing)");
}
#[test]
fn entering_a_starport_autosaves_never_in_a_fight() {
    use mt1::{ground::walk, save};
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    for (fight, on) in [(false, true), (true, true), (false, false)] {
        let saves = std::env::temp_dir().join(format!("mt-save-auto-{}-{fight}-{on}", std::process::id())); let _ = std::fs::remove_dir_all(&saves);
        let mut g = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game;
        g.save_slot = Some(save::Slot { dir: saves.clone(), build: "test" }); g.scene = Some(mt1::ui::SceneHook::default());
        let mut squad = mt1::ground::squad::State::default(); squad.encounter = fight; g.squad = Some(squad);
        let mut w = Walk::new(0, 0); w.events.extend([Event::Idle, Event::Key(keys::ESC), Event::KeyUp(keys::ESC)]); g.input = Box::new(w);
        save::AUTOSAVE.store(on, std::sync::atomic::Ordering::Relaxed);
        g.mem.set_event(9); walk::world_event(&mut g);
        save::AUTOSAVE.store(true, std::sync::atomic::Ordering::Relaxed);
        let list = save::entries(&saves); let _ = std::fs::remove_dir_all(&saves);
        let used: Vec<String> = list.iter().filter(|e| e.used()).map(|e| e.name()).collect();
        let log = g.squad.as_ref().unwrap().p1.log.clone();
        if fight || !on { assert!(used.is_empty() && !log.iter().any(|l| l == "Autosaved"), "fight {fight} on {on}: {used:?} {log:?}"); continue }
        assert_eq!((used, log.last().map(String::as_str)), (vec!["Autosave 1".to_string()], Some("Autosaved")));
        assert!(save::rows(&list)[9][1].ends_with("starport"), "a desk save: {:?}", save::rows(&list)[9]);
    }
}
#[test]
fn product_recruit_asks_for_no_drive_faithful_still_does() {
    use mt1::ground::walk;
    let Some(dir) = oracle_env::gamefiles() else { return };
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/atoms/scenarios/atom_recruit.toml");
    for product in [true, false] {
        let seen: Rc<RefCell<Vec<String>>> = Default::default(); mt_engine::present::textlog::set_enabled(true);
        let mut w = Walk::new(0, 0); w.seen = seen.clone();
        for k in [keys::DOWN, keys::DOWN, keys::ENTER, keys::ESC, keys::ESC] { w.events.extend([Event::Idle, Event::Key(k), Event::KeyUp(k), Event::Idle, Event::Idle]) }
        let mut g = scenario::load(&path, &dir).unwrap_or_else(|e| panic!("{e}")).game; g.input = Box::new(w);
        g.scene = if product { Some(mt1::ui::SceneHook::default()) } else { None };
        g.mem.set_event(9); walk::world_event(&mut g);
        mt_engine::present::textlog::set_enabled(false);
        let seen = seen.borrow(); let has = |t: &str| seen.iter().any(|s| s.contains(t));
        assert_eq!((has("Select drive"), has("disk"), has("Select character to try")), (!product, false, product), "product {product}: {seen:?}");
    }
}
#[test]
#[ignore = "full gate only (~17 min debug): cargo test -p megatraveller identity_over_every -- --ignored"]
fn identity_over_every_scenario_and_seeded_walks() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
    let mut paths: Vec<_> = std::fs::read_dir(&root).unwrap().map(|e| e.unwrap().path()).filter(|p| p.extension().is_some_and(|e| e == "toml")).collect(); paths.sort();
    let (mut states, mut continued) = (0, 0);
    for path in &paths {
        for seed in [1u32, 1990] {
            let (a, b) = (run(path, &dir, seed, false), run(path, &dir, seed, true));
            let name = path.file_name().unwrap().to_string_lossy();
            assert_eq!(a.len(), b.len(), "{name} seed {seed}: capture count");
            if a.is_empty() { eprintln!("save identity: {name} seed {seed}: no loop boundary (the scenario starts inside a modal screen or launches S)"); continue }
            for (i, (x, y)) in a.iter().zip(&b).enumerate() {
                assert_eq!(x.ds == y.ds, true, "{name} seed {seed}: DS differs at capture {i}: {:?}", x.ds.iter().zip(&y.ds).enumerate().filter(|(_, (p, q))| p != q).map(|(o, _)| o).take(8).collect::<Vec<_>>());
                assert_eq!((&x.segs, x.seed, x.ticks, x.now, x.bios_ticks, x.landing, x.sound_option), (&y.segs, y.seed, y.ticks, y.now, y.bios_ticks, y.landing, y.sound_option), "{name} seed {seed}: capture {i}");
                assert_eq!(Saved::decode(&x.encode()).unwrap(), *x);
            }
            states += a.len(); if a.len() == 2 { continued += 1 }
        }
    }
    eprintln!("save identity: {} scenarios, {states} states, {continued} continued walks", paths.len());
    assert!(continued >= 8, "too few walks reached the continuation capture: {continued}");
}
