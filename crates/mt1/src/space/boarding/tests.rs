use super::*;
use crate::space::host::GunUi;
use mt_engine::{io::{keys, Dir, Event, Input}, play::{Choice, Play}, sim::tick::{Clock, Counter}};
use std::collections::VecDeque;
mod oracle_env { include!("../../../tests/oracle_env/mod.rs"); }

struct Native { events: VecDeque<Event>, polls: usize }
impl Native {
    fn new(keys: &[u16]) -> Self {
        Self { events: keys.iter().flat_map(|&k| [Event::Key(k), Event::KeyUp(k)]).collect(), polls: 0 }
    }
}
impl Input for Native {
    fn next_key(&mut self) -> u16 { panic!("boarding uses cursor polls") }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        self.polls += 1; assert!(self.polls <= 400, "boarding clock stalled");
        self.events.pop_front().unwrap_or_else(|| { clock.wait_tick(); Event::Idle })
    }
}
fn space(input: Box<dyn Input>, timed: bool) -> Option<Space> {
    let dir = oracle_env::gamefiles()?;
    let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![],
        input, Box::new(Counter::at(100000)), Box::new(Dir(dir))).unwrap();
    sp.r.initialize(&mut sp.s);
    sp.s.set_byte(KEY, 0); sp.s.set_byte(PENDING_KEY, 0);
    sp.s.set_word(STATION_ACTIVE, 0); sp.s.set_word(STATION, 11);
    sp.s.set_enemy(0, 2, 5); sp.s.set_enemy(0, 24, i16::from(timed));
    sp.s.set_dword(0x343A, 864);
    Some(sp)
}
fn board(sp: &mut Space) {
    sp.board_enemy_cargo(0, &mut GunUi { scratch: vec![0; 65536], bp: 0xF000 }).unwrap();
    assert_eq!(sp.s.byte(0x4626), 0);
    assert_eq!(sp.s.dword(tick::SNAPSHOT), 65535);
}

#[test]
fn timed_and_untimed_cargo_choices_match_native_transfer_and_exit() {
    for timed in [false, true] { for transfer in [false, true] {
        let native = if transfer { vec![b'T' as u16, keys::F1, keys::F1, keys::ESC] } else { vec![keys::ESC] };
        let choices = if transfer { vec!["cmd:transfer", "menu:1c1c/0", "menu:1c1c/0", "back"] } else { vec!["back"] };
        let Some(mut a) = space(Box::new(Native::new(&native)), timed) else { return };
        let mut b = space(Box::new(Play::scripted(choices.into_iter().enumerate().map(|(i, pick)| Choice::new(format!("d:{}",i+1),pick)))), timed).unwrap();
        board(&mut a); board(&mut b);
        assert_eq!(b.s.party, a.s.party, "timed={timed}, transfer={transfer}");
        assert_eq!(b.s.pbyte(0xC6), 0);
        assert_eq!(b.s.pbyte(0xCA), if transfer { 6 } else { 0 }, "same seed/cargo as S:236C oracle probe");
        assert!(b.clock.ticks() < 100324);
    } }
}

#[test]
fn native_timed_boarding_still_expires_after_324_ticks() {
    let Some(mut sp) = space(Box::new(Native::new(&[])), true) else { return };
    board(&mut sp);
    assert_eq!(sp.clock.ticks(), 100324);
    assert_eq!(sp.s.word(0x1CB), 0);
    assert_eq!(sp.s.pbyte(0xC6), 60);
    assert_eq!(sp.s.pbyte(0xCA), 0);
}
