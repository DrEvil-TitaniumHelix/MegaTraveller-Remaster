#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Slots { size: usize, data: Vec<u8>, used: Vec<bool> }
impl Slots {
    pub fn new(size: usize, cap: usize) -> Option<Slots> {
        fn zeroed<T: Clone>(n: usize, z: T) -> Option<Vec<T>> { let mut v = Vec::new(); v.try_reserve_exact(n).ok()?; v.resize(n, z); Some(v) }
        Some(Slots { size, data: zeroed(size.checked_mul(cap)?, 0u8)?, used: zeroed(cap, false)? })
    }
    pub fn size(&self) -> usize { self.size }
    pub fn cap(&self) -> usize { self.used.len() }
    pub fn get(&self, i: usize) -> Option<&[u8]> { (i < self.cap()).then(|| &self.data[i * self.size..(i + 1) * self.size]) }
    pub fn get_mut(&mut self, i: usize) -> Option<&mut [u8]> { let s = self.size; (i < self.cap()).then(move || &mut self.data[i * s..(i + 1) * s]) }
    pub fn put(&mut self, i: usize, rec: &[u8]) -> bool {
        if rec.len() != self.size { return false }
        match self.get_mut(i) { Some(s) => { s.copy_from_slice(rec); self.used[i] = true; true } None => false }
    }
    pub fn clear(&mut self, i: usize) { if let Some(u) = self.used.get_mut(i) { *u = false } }
    pub fn mark(&mut self, i: usize, used: bool) { if let Some(u) = self.used.get_mut(i) { *u = used } }
    pub fn first_free(&self) -> Option<usize> { self.used.iter().position(|&u| !u) }
    pub fn count(&self) -> usize { self.used.iter().filter(|&&u| u).count() }
    pub fn occupied(&self) -> impl Iterator<Item = (usize, &[u8])> { self.used.iter().enumerate().filter(|(_, &u)| u).map(move |(i, _)| (i, self.get(i).unwrap())) }
    pub fn load(&mut self, recs: impl IntoIterator<Item = Vec<u8>>) -> Result<(), usize> {
        let mut bad = 0;
        for (i, r) in recs.into_iter().enumerate() { if !self.put(i, &r) { bad += 1 } }
        if bad == 0 { Ok(()) } else { Err(bad) }
    }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn slots() {
        assert!(Slots::new(usize::MAX / 2 + 1, 2).is_none());
        assert!(Slots::new(usize::MAX, 1).is_none()); assert!(Slots::new(0, usize::MAX).is_none());
        let mut s = Slots::new(4, 3).unwrap(); assert_eq!((s.size(), s.cap()), (4, 3));
        assert!(s.put(1, &[1, 2, 3, 4])); assert!(!s.put(1, &[1])); assert!(!s.put(3, &[0; 4]));
        assert_eq!((s.first_free(), s.count()), (Some(0), 1));
        assert!(s.put(0, &[9; 4])); assert_eq!(s.first_free(), Some(2));
        assert_eq!(s.occupied().map(|(i, r)| (i, r[0])).collect::<Vec<_>>(), [(0, 9), (1, 1)]);
        s.clear(0); assert_eq!(s.first_free(), Some(0)); assert_eq!(s.get(0).unwrap(), [9; 4]); assert!(s.get(3).is_none());
        s.mark(2, true); s.mark(3, true); assert_eq!(s.count(), 2); assert_eq!(s.get(2).unwrap(), [0; 4]); s.mark(2, false); assert_eq!(s.count(), 1);
        assert_eq!(s.load([vec![0; 4], vec![0; 4], vec![0; 4], vec![0; 4]]), Err(1)); assert_eq!(s.count(), 3); assert_eq!(s.first_free(), None);
    }
}
