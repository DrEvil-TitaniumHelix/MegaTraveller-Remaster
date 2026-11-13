use crate::pipeline::Frame;
use mt1::ui::Scene;
use std::{collections::VecDeque, time::Instant};

const MAX: usize = 32;
pub struct Entry { pub scene: Scene, pub frame: Box<Frame>, pub fx: bool }
pub struct Replay { queue: VecDeque<Entry>, since: Option<Instant>, shown: u32, seq: u32, held: Option<Box<Frame>>, pub ms: u32 }
impl Default for Replay { fn default() -> Self { Self { queue: VecDeque::new(), since: None, shown: 0, seq: 0, held: None, ms: 250 } } }
fn has_fx(s: &Scene) -> bool { let f = &s.squad.fx; !f.shots.is_empty() || !f.hits.is_empty() || !f.blasts.is_empty() || f.walk.len() > 1 }
impl Replay {
    pub fn active(&self) -> bool { self.since.is_some() || !self.queue.is_empty() }
    pub fn frame(&mut self, f: Frame) -> Option<Frame> { if self.active() { self.held = Some(Box::new(f)); None } else { Some(f) } }
    pub fn arrive(&mut self, s: Scene, current: &Frame, now: Instant) -> Option<Entry> {
        let fx = s.squad.fx.seq != self.seq && has_fx(&s);
        if fx { self.seq = s.squad.fx.seq }
        let frame = self.held.take().unwrap_or_else(|| Box::new(current.clone()));
        let e = Entry { scene: s, frame, fx };
        if !self.active() { return Some(self.start(e, now)) }
        if !fx { if let Some(back) = self.queue.back_mut() { if !back.fx { *back = e; return None } } }
        self.queue.push_back(e);
        if self.queue.len() > MAX { let _ = self.flush(); }
        None
    }
    fn start(&mut self, e: Entry, now: Instant) -> Entry { if e.fx { self.since = Some(now); self.shown = e.scene.squad.fx.seq } else { self.since = None } e }
    pub fn tick(&mut self, now: Instant) -> (Vec<Entry>, Option<(u32, f32)>) {
        let mut show = Vec::new();
        if let Some(since) = self.since {
            let t = now.saturating_duration_since(since).as_millis() as f32 / self.ms.max(1) as f32;
            if t < 1.0 { return (show, Some((self.shown, t))) }
            self.since = None;
        }
        while self.since.is_none() { match self.queue.pop_front() { Some(e) => { let e = self.start(e, now); show.push(e) } None => break } }
        (show, self.since.map(|_| (self.shown, 0.0)))
    }
    pub fn flush(&mut self) -> Option<Entry> { self.since = None; let last = self.queue.pop_back(); self.queue.clear(); last.map(|mut e| { e.fx = false; e }) }
    pub fn take_held(&mut self) -> Option<Frame> { if self.active() { None } else { self.held.take().map(|b| *b) } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    fn scene(seq: u32, shot: bool) -> Scene {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.fx.seq = seq;
        if shot { s.squad.fx.shots.push(((1, 1), (2, 2), 0x78)) } s
    }
    fn frame(tag: u8) -> Frame { let mut f = Frame::default(); f.indexed[0] = tag; f }
    #[test]
    fn effects_scenes_are_shown_one_per_playback_time_and_plain_scenes_collapse() {
        let mut r = Replay::default(); let t0 = Instant::now();
        assert!(r.arrive(scene(0, false), &frame(0), t0).is_some_and(|e| !e.fx), "no effects: shown at once");
        let e = r.arrive(scene(1, true), &frame(1), t0).expect("the first effect Scene shows at once"); assert!(e.fx && r.active());
        assert!(r.frame(frame(2)).is_none(), "a frame during the replay is held");
        assert!(r.arrive(scene(2, true), &frame(1), t0).is_none(), "the second NPC's turn waits");
        assert!(r.arrive(scene(2, false), &frame(1), t0).is_none()); assert!(r.arrive(scene(2, false), &frame(1), t0).is_none());
        assert_eq!(r.queue.len(), 2, "two plain Scenes collapsed into one");
        let (show, p) = r.tick(t0 + Duration::from_millis(100)); assert!(show.is_empty()); assert_eq!(p.map(|p| p.0), Some(1)); assert!(p.unwrap().1 > 0.3 && p.unwrap().1 < 0.5);
        let (show, p) = r.tick(t0 + Duration::from_millis(260)); assert_eq!(show.len(), 1); assert!(show[0].fx); assert_eq!(show[0].frame.indexed[0], 2, "the held frame rides with the next Scene"); assert_eq!(p, Some((2, 0.0)));
        let (show, p) = r.tick(t0 + Duration::from_millis(600)); assert_eq!((show.len(), show[0].fx, p), (1, false, None)); assert!(!r.active()); assert!(r.take_held().is_none());
        assert!(r.arrive(scene(2, true), &frame(3), t0).is_some_and(|e| !e.fx));
        r.arrive(scene(3, true), &frame(3), t0); r.arrive(scene(4, true), &frame(3), t0); r.arrive(scene(4, false), &frame(3), t0);
        assert!(r.frame(frame(9)).is_none()); assert!(r.take_held().is_none(), "held while active");
        let last = r.flush().unwrap(); assert_eq!((last.fx, last.scene.squad.fx.seq), (false, 4)); assert!(!r.active()); assert!(r.flush().is_none());
        assert_eq!(r.take_held().map(|f| f.indexed[0]), Some(9));
        assert!(r.frame(frame(5)).is_some(), "no replay: frames show at once");
    }
}
