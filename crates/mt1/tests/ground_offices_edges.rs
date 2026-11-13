use mt1::ground::{starport::offices, state::{Ground, Mem}};
use mt_engine::{io::{Dir, Event, Input}, sim::tick::{Clock, Counter}};

struct Escape;
impl Input for Escape {
    fn next_key(&mut self) -> u16 { 27 }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        clock.wait_tick();
        Event::Key(27)
    }
}

#[test]
fn tas_malformed_leader_wraps_without_panicking() {
    let mut g = Ground::from_ds(vec![], vec![0; Mem::PARTY_SIZE],
        Box::new(Escape), Box::new(Counter::default()),
        Box::new(Dir(std::path::PathBuf::new())));
    g.mem.set_party_u8(1, 255);
    g.mem.set_party_u8(0x2A, 0);
    offices::tas(&mut g);
    assert_eq!(g.mem.key(), 0);
    assert_eq!(g.mem.party_u8(1), 255);
    assert_eq!(g.mem.i16(0x6E0A), 2);
}
