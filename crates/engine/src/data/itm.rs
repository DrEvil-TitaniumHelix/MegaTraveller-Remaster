use super::bin::{check, fit, parse, write_all, Rd, Record, Result, Wr};
use super::Variant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub name: [u8; 29], pub r#type: u16, pub weight: u16, pub price: u32, pub f25: u8, pub icon: u8, pub ammo_flags: u8, pub charges: u8,
    pub f29: u8, pub f2a: u8, pub tech_level: u8, pub law_level: u8, pub f2d: u8,
}
impl Record for Item {
    const SIZE: usize = 46;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(Item { name: r.arr()?, r#type: r.u16()?, weight: r.u16()?, price: r.u32()?, f25: r.u8()?, icon: r.u8()?, ammo_flags: r.u8()?, charges: r.u8()?,
            f29: r.u8()?, f2a: r.u8()?, tech_level: r.u8()?, law_level: r.u8()?, f2d: r.u8()? })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.bytes(&self.name); w.u16(self.r#type); w.u16(self.weight); w.u32(self.price); w.u8(self.f25); w.u8(self.icon); w.u8(self.ammo_flags); w.u8(self.charges);
        w.u8(self.f29); w.u8(self.f2a); w.u8(self.tech_level); w.u8(self.law_level); w.u8(self.f2d)
    }
}
pub fn decode(v: Variant, d: &[u8]) -> Result<Vec<Item>> {
    let Variant::Mt1 = v;
    parse(d, |r| { let n = r.u16()? as usize; check("itm file size", d.len() as i64, (2 + 46 * n) as i64)?; Item::take_n(r, n) })
}
pub fn encode(v: Variant, p: &[Item]) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    let mut w = Vec::with_capacity(2 + 46 * p.len()); w.u16(fit("itm count", p.len())?); write_all(&mut w, p); Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    fn sl<const N: usize>(d: &[u8], a: usize) -> [u8; N] { d[a..a + N].try_into().unwrap() }
    #[test]
    fn record_layout_and_round_trip() {
        let d: [u8; 46] = std::array::from_fn(|i| i as u8);
        let it = parse(&d, Item::read).unwrap();
        assert_eq!(it, Item { name: sl(&d, 0), r#type: u16::from_le_bytes(sl(&d, 0x1d)), weight: u16::from_le_bytes(sl(&d, 0x1f)), price: u32::from_le_bytes(sl(&d, 0x21)),
            f25: d[0x25], icon: d[0x26], ammo_flags: d[0x27], charges: d[0x28], f29: d[0x29], f2a: d[0x2a], tech_level: d[0x2b], law_level: d[0x2c], f2d: d[0x2d] });
        assert_eq!(it.to_bytes(), d);
        let e = encode(Variant::Mt1, &[it.clone(), it.clone()]).unwrap();
        assert_eq!(e.len(), 2 + 92);
        assert_eq!(decode(Variant::Mt1, &e).unwrap(), [it.clone(), it]);
        assert!(decode(Variant::Mt1, &e[..50]).is_err());
    }
}
