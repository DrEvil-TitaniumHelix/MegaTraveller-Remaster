pub mod keys;
pub mod scan;
pub mod script;
pub mod files;
pub mod dump;
pub mod harness;
pub mod mouse;
pub mod audio;
pub use files::{Dir, Files, Overlay};
pub use harness::Harness;
pub use mouse::{MouseEvent, MouseState};

pub type Key = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    Idle,
    Key(Key),
    KeyUp(Key),
    Mouse(MouseEvent),
    Boundary,
}
pub trait Input {
    fn data_mode(&self) -> bool { false }
    fn diagnostics(&self) -> bool { false }
    fn wants_decision(&self) -> bool { false }
    fn decide(&mut self, _d: &crate::play::Decision) {}
    fn anchor(&mut self, _name: &str, _image: &[u8]) {}
    fn play_checkpoint(&self) -> Option<crate::play::Checkpoint> { None }
    fn resume_play_checkpoint(&mut self, _state: &crate::play::Checkpoint) -> Result<(), String> { Ok(()) }
    fn play_clock(&mut self, clock:Box<dyn crate::sim::tick::Clock>) -> Box<dyn crate::sim::tick::Clock> {clock}
    fn live_save_location(&self) -> Option<(&std::path::Path, &'static str)> {None}
    fn closed(&self) -> bool { false }
    fn clock_checkpoint(&mut self,_name:&str)->Result<Option<u32>,String> {Ok(None)}
    fn take_checkpoint(&mut self,_name:&str)->Option<u32> {None}
    fn elapse_poll(&mut self,clock:&mut dyn crate::sim::tick::Clock)->Result<Event,String> {Ok(self.poll(clock))}
    fn boundary_due(&self) -> bool { false }
    fn streams_steps(&self) -> bool { false }
    fn stream_step_file(&mut self, _name: &str, _bytes: &[u8], _append: bool) {
        eprintln!("input: step streaming is unavailable"); std::process::exit(3)
    }
    fn next_key(&mut self) -> Key;
    fn key_pending(&mut self) -> bool { false }
    fn flush(&mut self) {}
    fn reset(&mut self) {}
    fn refuse_grant(&mut self) {}
    fn step(&mut self, _s: &dump::Step) {}
    fn present(&mut self, _rgb: &[u32]) {}
    fn audio(&mut self, _n: u16, _hz: u32, _levels: &[u8], _rest: u8) {}
    fn end_files(&mut self, _files: &[(&str, &[u8])]) {}
    fn poll(&mut self, clock: &mut dyn crate::sim::tick::Clock) -> Event {
        if self.key_pending() { Event::Key(self.next_key()) } else { clock.wait_tick(); Event::Idle }
    }
    fn arrived(&mut self) -> bool { false }
    fn mouse(&mut self) -> MouseState { MouseState::default() }
    fn set_mouse_position(&mut self, _col: i16, _row: i16) {}
    fn scene(&mut self, _s: &dyn std::any::Any) {}
    fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { None }
}
pub struct Scripted { pub keys: std::collections::VecDeque<Key>, pub exhausted: bool }
impl Scripted { pub fn new(keys: impl IntoIterator<Item = Key>) -> Scripted { Scripted { keys: keys.into_iter().collect(), exhausted: false } } }
impl Input for Scripted {
    fn next_key(&mut self) -> Key { self.keys.pop_front().unwrap_or_else(|| { self.exhausted = true; keys::ESC }) }
    fn poll(&mut self, clock: &mut dyn crate::sim::tick::Clock) -> Event { match self.keys.pop_front() { Some(k) => Event::Key(k), None => { clock.wait_tick(); Event::Idle } } }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn scripted() {
        let mut s = Scripted::new([13]); assert_eq!((s.next_key(), s.exhausted), (13, false)); assert_eq!((s.next_key(), s.exhausted), (27, true)); assert!(!s.key_pending()); s.flush(); s.reset();
        let (mut s, mut c) = (Scripted::new([65]), crate::sim::tick::Counter::default());
        assert_eq!(s.poll(&mut c), Event::Key(65)); assert_eq!(s.poll(&mut c), Event::Idle); assert_eq!(c.now, 1); assert_eq!(s.mouse(), MouseState::default());
    }
}
