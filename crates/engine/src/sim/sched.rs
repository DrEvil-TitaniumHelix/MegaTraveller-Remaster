pub const SECOND: u32 = 18;
pub const MESSAGE_AUTO_CLOSE: u32 = 180;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Job { pub deadline: u32 }
impl Job {
    pub fn at(now: u32, period: u32) -> Job { Job { deadline: now.wrapping_add(period) } }
    pub fn due(&self, now: u32) -> bool { now > self.deadline }
    pub fn arm(&mut self, now: u32, period: u32) { self.deadline = now.wrapping_add(period) }
    pub fn fire(&mut self, now: u32, period: u32) -> bool { if self.due(now) { self.arm(now, period); true } else { false } }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn deadlines() {
        let mut j = Job::at(100, SECOND); assert_eq!(j.deadline, 118);
        assert!(!j.fire(118, SECOND));
        assert!(j.fire(119, SECOND)); assert_eq!(j.deadline, 137);
        assert!(!j.fire(137, SECOND));
        let w = Job::at(u32::MAX - 5, SECOND); assert_eq!(w.deadline, 12); assert!(w.due(u32::MAX)); assert!(!w.due(12)); assert!(w.due(13));
        let mut t = Job::default(); assert_eq!(t.deadline, 0); assert!(t.due(1)); t.arm(50, MESSAGE_AUTO_CLOSE); assert_eq!(t.deadline, 230);
    }
}
