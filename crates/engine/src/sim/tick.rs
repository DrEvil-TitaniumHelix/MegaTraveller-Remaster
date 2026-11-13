use crate::io::Input;
pub const HZ: f64 = 1193180.0 / 65536.0;
pub trait Clock {
    fn now(&mut self) -> u32;
    fn set(&mut self, now: u32);
    fn wait_tick(&mut self);
    fn busy_wait(&mut self) {}
    fn ticks(&mut self) -> u32 { self.now() }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counter { pub now: u32, pub assigned: Option<u32> }
impl Counter { pub const fn at(now: u32) -> Counter { Counter { now, assigned: None } } }
impl Clock for Counter {
    fn now(&mut self) -> u32 { self.assigned.unwrap_or(self.now) }
    fn set(&mut self, now: u32) { self.assigned = Some(now) }
    fn wait_tick(&mut self) { self.now = self.now.wrapping_add(1); self.assigned = None }
    fn ticks(&mut self) -> u32 { self.now }
}
pub const TICKS_PER_DAY: u32 = 0x1800B0;
#[derive(Default)]
pub struct Wall { offset: u32 }
impl Wall {
    fn raw() -> u32 {
        static BASE: std::sync::OnceLock<(u32, std::time::Instant)> = std::sync::OnceLock::new();
        let (start, at) = *BASE.get_or_init(|| {
            let utc = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
            (((utc.rem_euclid(86400.0) * HZ) as u32) % TICKS_PER_DAY, std::time::Instant::now())
        });
        Self::count(start, at.elapsed().as_secs_f64())
    }
    fn count(start: u32, elapsed_secs: f64) -> u32 { start.wrapping_add((elapsed_secs * HZ) as u32) }
}
impl Clock for Wall {
    fn now(&mut self) -> u32 {
        Self::raw().wrapping_add(self.offset)
    }
    fn set(&mut self, now: u32) { self.offset = now.wrapping_sub(Self::raw()) }
    fn wait_tick(&mut self) { let t = self.now(); while self.now() == t { std::thread::sleep(std::time::Duration::from_millis(1)) } }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Timer { pub start: u32 }
impl Timer {
    pub fn start(&mut self, c: &mut dyn Clock) { self.start = c.now() }
    pub fn delay(&mut self, c: &mut dyn Clock, ticks: i16, input: &mut dyn Input) -> bool {
        input.reset();
        if self.start == 0 { self.start = c.now() }
        let end = c.now().wrapping_add(ticks as i32 as u32);
        let mut cut = false;
        while c.now() < end { if input.key_pending() { input.flush(); input.reset(); cut = true; break } c.wait_tick() }
        self.start = 0; cut
    }
}
#[cfg(test)]
mod t {
    use super::*;
    use crate::io::Scripted;
    struct Pending(u8, u8); impl Input for Pending { fn next_key(&mut self) -> u16 { 13 } fn key_pending(&mut self) -> bool { true } fn flush(&mut self) { self.0 += 1 } fn reset(&mut self) { self.1 += 1 } }
    #[test]
    fn counter_and_delay() {
        let (mut c, mut t, mut no) = (Counter::at(10), Timer::default(), Scripted::new([]));
        t.start(&mut c); assert_eq!(t.start, 10);
        assert!(!t.delay(&mut c, 2, &mut no)); assert_eq!((c.now, t.start), (12, 0));
        assert!(!t.delay(&mut c, -1, &mut no)); assert_eq!(c.now, 12);
        c.now = u32::MAX - 1; assert!(!t.delay(&mut c, 3, &mut no)); assert_eq!(c.now, u32::MAX - 1);
        c.now = 0; let mut p = Pending(0, 0); assert!(t.delay(&mut c, 5, &mut p)); assert_eq!((c.now, p.0, p.1), (0, 1, 2));
        let mut p = Pending(0, 0); assert!(!t.delay(&mut c, 0, &mut p)); assert_eq!((p.0, p.1), (0, 1));
        assert!((HZ - 18.2065).abs() < 1e-4);
    }
    #[test]
    fn counter_assignment_rewinds_and_wraps() {
        let mut c = Counter::at(123);
        c.set(5); assert_eq!((c.now(), c.ticks()), (5, 123));
        c.wait_tick(); assert_eq!((c.now(), c.ticks()), (124, 124));
        c.now = u32::MAX; c.wait_tick(); assert_eq!(c.now(), 0);
        let mut w = Wall::default(); w.set(40);
        assert!(w.now().wrapping_sub(40) <= 1);
        w.wait_tick(); assert!(w.now().wrapping_sub(40) >= 1);
    }
    #[test]
    fn wall_is_time_of_day() {
        assert_eq!((86400.0 * HZ) as u32, TICKS_PER_DAY);
        let mut w = Wall::default(); let a = w.now();
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() % 86400;
        assert!((a as i64 - (secs as f64 * HZ) as i64).rem_euclid(TICKS_PER_DAY as i64) <= HZ as i64 + 1);
        w.wait_tick(); let c = w.now(); assert!(c != a && (c as i64 - a as i64).rem_euclid(TICKS_PER_DAY as i64) <= 2);
    }
    #[test]
    fn wall_counts_through_midnight() {
        use crate::sim::sched::Job;
        let before = Wall::count(TICKS_PER_DAY - 10, 0.0); let job = Job::at(before, 18);
        let after = Wall::count(TICKS_PER_DAY - 10, 60.0);
        assert_eq!(after - before, (60.0 * HZ) as u32); assert!(after > TICKS_PER_DAY && job.due(after));
        assert!(after >= before.wrapping_add(3));
    }
}
