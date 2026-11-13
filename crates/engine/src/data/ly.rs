use super::bin::{fit, Rd, Record, Result, Wr};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rec { pub op: u8, pub colour: u8, pub y1: i16, pub x1: i16, pub y2: i16, pub x2: i16 }
impl Record for Rec {
    const SIZE: usize = 10;
    fn read(r: &mut Rd) -> Result<Self> { Ok(Rec { op: r.u8()?, colour: r.u8()?, y1: r.i16()?, x1: r.i16()?, y2: r.i16()?, x2: r.i16()? }) }
    fn write(&self, w: &mut Vec<u8>) { w.u8(self.op); w.u8(self.colour); w.i16(self.y1); w.i16(self.x1); w.i16(self.y2); w.i16(self.x2) }
}
pub type Entry = Vec<Rec>;

pub fn decode(d: &[u8]) -> Result<Vec<Entry>> {
    super::bin::parse(d, |r| { let n = r.u8()?; (0..n).map(|_| { let k = r.u8()?; Rec::take_n(r, k as usize) }).collect() })
}
pub fn encode(e: &[Entry]) -> Result<Vec<u8>> {
    let mut w = vec![fit("ly entry count", e.len())?];
    for x in e { w.u8(fit("ly record count", x.len())?); super::bin::write_all(&mut w, x); }
    Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip_synthetic() {
        let e = vec![vec![], vec![Rec { op: 3, colour: 0x10, y1: -1, x1: 2, y2: 300, x2: -300 }]];
        let b = encode(&e).unwrap();
        assert_eq!(b, [2, 0, 1, 3, 0x10, 0xff, 0xff, 2, 0, 44, 1, 0xd4, 0xfe]);
        assert_eq!(decode(&b).unwrap(), e);
        assert!(decode(&b[..12]).is_err());
        assert!(decode(&[1, 0, 0]).is_err());
        assert!(encode(&vec![vec![]; 256]).is_err());
    }
}
