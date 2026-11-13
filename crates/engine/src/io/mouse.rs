pub const LEFT_RELEASE: u16 = 0x04;
pub const RIGHT_RELEASE: u16 = 0x10;
pub const LEFT_DOWN: u16 = 1;
pub const RIGHT_DOWN: u16 = 2;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseEvent { pub event: u16, pub buttons: u16, pub col: i16, pub row: i16 }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MouseState { pub buttons: u16, pub col: i16, pub row: i16 }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Hotspot { pub top: i16, pub left: i16, pub height: i16, pub width: i16, pub id: u8 }
impl Hotspot {
    pub const SIZE: usize = 9;
    pub fn parse(b: &[u8]) -> Option<Hotspot> {
        let w = |i: usize| Some(i16::from_le_bytes([*b.get(i)?, *b.get(i + 1)?]));
        Some(Hotspot { top: w(0)?, left: w(2)?, height: w(4)?, width: w(6)?, id: *b.get(8)? })
    }
    pub fn pack(&self) -> [u8; 9] {
        let mut o = [0; 9];
        for (i, v) in [self.top, self.left, self.height, self.width].iter().enumerate() { o[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes()) }
        o[8] = self.id; o
    }
    pub fn contains(&self, col: i16, row: i16) -> bool {
        self.top <= row && row < self.top.wrapping_add(self.height) && self.left <= col && col < self.left.wrapping_add(self.width)
    }
}
pub fn hit(spots: impl IntoIterator<Item = Hotspot>, col: i16, row: i16) -> Option<u8> { spots.into_iter().find(|h| h.contains(col, row)).map(|h| h.id) }
pub fn table(bytes: &[u8], count: usize) -> impl Iterator<Item = Hotspot> + '_ { bytes.chunks_exact(Hotspot::SIZE).take(count).filter_map(Hotspot::parse) }
pub fn window_to_game(px: f64, py: f64, rect: (usize, usize, usize, usize)) -> Option<(i16, i16)> {
    let (x, y, w, h) = rect;
    if w == 0 || h == 0 || !(px.is_finite() && py.is_finite()) { return None }
    let (dx, dy) = (px - x as f64, py - y as f64);
    if dx < 0.0 || dy < 0.0 || dx >= w as f64 || dy >= h as f64 { return None }
    Some(((dx * 320.0 / w as f64) as i16, (dy * 200.0 / h as f64) as i16))
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn matcher_and_unscale() {
        let t = [Hotspot { top: 0, left: 6, height: 50, width: 40, id: 0x3B }, Hotspot { top: 0, left: 46, height: 50, width: 40, id: 0x3C }, Hotspot { top: 2, left: 208, height: 22, width: 45, id: 0x18 }];
        assert_eq!(hit(t, 6, 0), Some(0x3B)); assert_eq!(hit(t, 45, 49), Some(0x3B)); assert_eq!(hit(t, 46, 0), Some(0x3C));
        assert_eq!(hit(t, 5, 0), None); assert_eq!(hit(t, 10, 50), None); assert_eq!(hit(t, 208, 2), Some(0x18)); assert_eq!(hit(t, 252, 23), Some(0x18)); assert_eq!(hit(t, 253, 23), None);
        let first = [Hotspot { top: 0, left: 0, height: 10, width: 10, id: 1 }, Hotspot { top: 0, left: 0, height: 10, width: 10, id: 2 }];
        assert_eq!(hit(first, 3, 3), Some(1));
        let wrap = Hotspot { top: 32000, left: 0, height: 1000, width: 1, id: 9 }; assert!(!wrap.contains(0, 32500));
        let mut bytes = Vec::new(); for h in t { bytes.extend_from_slice(&h.pack()) } bytes.push(0xEE);
        assert_eq!(table(&bytes, 3).collect::<Vec<_>>(), t.to_vec()); assert_eq!(table(&bytes, 2).count(), 2); assert_eq!(Hotspot::parse(&bytes[..8]), None);
        assert_eq!(Hotspot::parse(&t[0].pack()), Some(t[0]));
        assert_eq!(window_to_game(0.0, 0.0, (0, 0, 320, 200)), Some((0, 0))); assert_eq!(window_to_game(319.9, 199.9, (0, 0, 320, 200)), Some((319, 199)));
        assert_eq!(window_to_game(320.0, 0.0, (0, 0, 320, 200)), None); assert_eq!(window_to_game(100.0, 100.0, (0, 0, 0, 0)), None);
        assert_eq!(window_to_game(10.0 + 2.0 * 193.0, 20.0 + 3.0 * 141.0, (10, 20, 640, 600)), Some((193, 141)));
        assert_eq!(window_to_game(9.0, 20.0, (10, 20, 640, 600)), None); assert_eq!(window_to_game(f64::NAN, 0.0, (0, 0, 320, 200)), None);
    }
}
