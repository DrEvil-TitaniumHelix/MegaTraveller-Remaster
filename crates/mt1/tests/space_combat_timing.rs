use mt1::space::{Space, state::*, combat::*, tick::{self, Hooks}};
use mt_engine::{io::{Dir, Scripted}, sim::tick::{Clock, Counter}};
use std::{cell::Cell, rc::Rc};

mod oracle_env;

struct Time(Rc<Cell<u32>>);
impl Clock for Time {
    fn now(&mut self) -> u32 { self.0.get() }
    fn set(&mut self, n: u32) { self.0.set(n); }
    fn wait_tick(&mut self) { self.0.set(self.0.get().wrapping_add(1)); }
}
struct EscapeAfterModal(Rc<Cell<u32>>);
struct NoInput;
impl Hooks for NoInput { fn update_cursor(&mut self, _sp: &mut Space) {} }
impl Hooks for EscapeAfterModal {
    fn update_cursor(&mut self, sp: &mut Space) { self.0.set(1000); sp.keyboard_isr(1); }
}
fn space() -> Option<Space> {
    let files = oracle_env::gamefiles()?;
    let mut sp = Space::new(&std::fs::read(files.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![],
        Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(files))).unwrap();
    sp.r.initialize(&mut sp.s);
    sp.hooks = Some(Box::new(NoInput));
    for i in 0..15 { sp.s.set_array(tick::ACTIVE, i, -1); }
    for i in 0..4 { sp.s.set_array(TARGET_Y, i, -1); }
    sp.s.set_word(ENCOUNTER, 1); sp.s.set_word(STATION_ACTIVE, 1); sp.s.set_word(STATION, 11);
    sp.s.set_word(HEADING, 0); sp.s.set_word(TACTICAL_X, 0); sp.s.set_word(TACTICAL_Y, 0);
    Some(sp)
}

#[test]
fn strict_enemy_deadline_and_modal_no_catch_up() {
    let Some(mut sp) = space() else { return; };
    let now = Rc::new(Cell::new(100)); sp.clock = Box::new(Time(now.clone()));
    sp.s.set_array(TARGET_Y, 0, 0); sp.s.set_enemy(0, 0, 110); sp.s.set_enemy(0, 16, 1500);
    sp.s.set_enemy(0, 20, 3); sp.s.set_enemy(0, 22, 3); sp.s.set_enemy(0, 26, 8); sp.s.set_enemy(0, 28, 8);
    sp.s.set_byte(grid_address(3, 3), 1); sp.s.set_dword(tick::DEADLINES + 32, 100);
    assert_eq!(sp.redraw_tactical_radar(99), 0);
    assert_eq!((sp.s.enemy(0, 20), sp.s.dword(tick::DEADLINES + 32)), (3, 100));
    now.set(101); assert_eq!(sp.run_tasks(-1), 1);
    assert_eq!((sp.s.enemy(0, 20), sp.s.dword(tick::DEADLINES + 32)), (4, 245));
    sp.hooks = Some(Box::new(EscapeAfterModal(now.clone())));
    sp.s.set_word(CHARGE, 0); sp.s.set_word(tick::ACTIVE + 18, 1); sp.s.set_dword(tick::DEADLINES + 36, 102);
    let before = sp.ev.reads; sp.show_tactical_map().unwrap();
    assert_eq!(now.get(), 1000); assert_eq!(sp.ev.reads, before, "6743 reads no game clock");
    assert_eq!((sp.s.enemy(0, 20), sp.s.word(CHARGE)), (4, 0), "modal map polls no jobs");
    sp.hooks = Some(Box::new(NoInput)); sp.s.set_byte(KEY, 1);
    sp.run_tasks(-1);
    assert_eq!((sp.s.enemy(0, 20), sp.s.dword(tick::DEADLINES + 32)), (5, 1144));
    assert_eq!((sp.s.word(CHARGE), sp.s.dword(tick::DEADLINES + 36)), (5, 1026), "only one overdue recharge");
    sp.run_tasks(-1); assert_eq!(sp.s.enemy(0, 20), 5);
    sp.s.set_word(STATION, 15); now.set(2000); sp.run_tasks(-1);
    assert_eq!(sp.s.enemy(0, 20), 5, "communications excludes enemy gate");
    assert_eq!(sp.s.word(CHARGE), 10, "communications still recharges");
}

#[test]
fn five_tick_latch_recharge_and_countdown_boundaries() {
    let Some(mut sp) = space() else { return; };
    let now = Rc::new(Cell::new(100)); sp.clock = Box::new(Time(now.clone()));
    sp.s.set_array(TARGET_Y, 0, 0); sp.s.set_enemy(0, 0, 110); sp.s.set_enemy(0, 16, 1500);
    for off in [20, 22, 26, 28] { sp.s.set_enemy(0, off, 0); }
    sp.advance_tactical_enemies();
    assert_eq!((sp.s.word(tick::ACTIVE + 12), sp.s.dword(tick::DEADLINES + 24)), (1, 105));
    sp.s.set_word(STATION, 15); now.set(104); sp.run_tasks(-1); assert_eq!(sp.s.word(tick::ACTIVE + 12), 1);
    now.set(105); sp.run_tasks(-1); assert_eq!(sp.s.word(tick::ACTIVE + 12), -1);
    sp.s.set_word(CHARGE, 98); sp.s.set_word(CHARGE + 2, 25); sp.s.set_word(tick::ACTIVE + 18, 1);
    sp.s.set_dword(tick::DEADLINES + 36, 105); sp.run_tasks(-1);
    assert_eq!((sp.s.word(CHARGE), sp.s.word(CHARGE + 2)), (100, 30));
    assert_eq!(sp.s.dword(tick::DEADLINES + 36), 131);
    sp.s.set_word(0x1CB, 0); sp.s.set_dword(tick::SNAPSHOT, 105);
    assert_eq!(sp.update_encounter_countdown(0, true), 1);
    now.set(429); assert_eq!(sp.update_encounter_countdown(9, false), 0);
    assert_eq!(sp.s.dword(tick::SNAPSHOT), 65535);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
