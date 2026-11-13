#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rng {
    pub state: u32,
    pub trace: Vec<u16>,
    pub force: Option<u16>,
}
pub const TRACE_CAP: usize = 1 << 20;
impl Rng {
    pub fn new(seed: u16) -> Rng { Rng { state: seed as u32, trace: vec![], force: None } }
    pub fn srand(&mut self, seed: u16) { self.state = self.force.unwrap_or(seed) as u32 }
    pub fn next(&mut self) -> u16 {
        self.state = self.state.wrapping_mul(214013).wrapping_add(2531011);
        let v = ((self.state >> 16) & 0x7FFF) as u16;
        if self.trace.len() >= TRACE_CAP { self.trace.clear() }
        self.trace.push(v); v
    }
    pub fn take_trace(&mut self) -> Vec<u16> { std::mem::take(&mut self.trace) }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn ms_lcg() {
        let mut r = Rng::new(4711);
        let s = 4711u32.wrapping_mul(214013).wrapping_add(2531011);
        assert_eq!(r.next(), ((s >> 16) & 0x7FFF) as u16); assert_eq!(r.state, s);
        let mut f = Rng::new(0); f.force = Some(31337); f.srand(12345); assert_eq!(f.state, 31337);
        f.srand(0xFFFF); assert_eq!(f.state, 31337); f.force = None; f.srand(0xFFFF); assert_eq!(f.state, 0xFFFF);
        for _ in 0..3 { f.next(); } assert_eq!(f.take_trace().len(), 3); assert!(f.trace.is_empty());
        let mut c = Rng::new(0); c.trace = vec![0; TRACE_CAP]; c.next(); assert_eq!((c.trace.len(), c.state), (1, 2531011));
        let mut z = Rng::new(0); let v: Vec<u16> = (0..3).map(|_| z.next()).collect(); assert_eq!(v, [38, 7719, 21238]);
    }
}
