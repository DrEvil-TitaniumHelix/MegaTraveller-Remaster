use super::*;
use mt_engine::{io::{Event, Scripted}, play::{Choice, Decision, Kind, Pick, Play}, sim::tick::Counter};

#[test]
fn delay_gate_preserves_data_mode_and_pending_choice() {
    let mut play = Play::scripted([Choice::new("d:1", Pick::Id("continue".into()))]);
    let mut decision = Decision::new(Kind::Ack, "cg", "4c50");
    decision.options.push(mt_engine::play::Option::new("continue", "Continue", vec![Event::Key(13)]));
    play.decide(&decision);
    let pending = play.pending().unwrap().id.clone();
    let mut clock = Counter::at(123);
    let mut timer = Timer::default();
    {
        let mut gated = Gated(&mut play);
        assert!(gated.data_mode());
        assert!(!gated.key_pending());
        assert!(!timer.delay(&mut clock, 2, &mut gated));
    }
    assert_eq!(clock.ticks(), 125);
    assert_eq!(play.pending().unwrap().id, pending);
    assert!(play.journal().entries.is_empty());
    assert_eq!(play.next_key(), 13);
    assert_eq!(play.journal().entries.len(), 1);
    assert!(!Gated(&mut Scripted::new([])).data_mode());
}
