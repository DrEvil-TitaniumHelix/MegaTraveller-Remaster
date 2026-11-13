use super::bin::{check, fit, parse, write_all, Rd, Record, Result, Wr};
use super::Variant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Char {
    pub name: [u8; 20], pub attr: [u8; 6], pub attr_base: [u8; 6], pub service: u8, pub rank: u8, pub age: u8, pub hits_max: u8, pub hits_split: u8,
    pub hits: u8, pub oxygen: u8, pub pension: u8, pub flags: u8, pub skill0: u8, pub skills: [u8; 71], pub skills_unmapped: [u8; 8],
    pub skill_progress: [u8; 80], pub state_c9: u8, pub f_ca: u8, pub f_cb: u8, pub inventory: [u16; 19], pub actor: u16, pub armor: u16,
    pub weapon: u16, pub f_f8: u16, pub f_fa: u16, pub credits: u32,
}
impl Record for Char {
    const SIZE: usize = 256;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(Char {
            name: r.arr()?, attr: r.arr()?, attr_base: r.arr()?, service: r.u8()?, rank: r.u8()?, age: r.u8()?, hits_max: r.u8()?, hits_split: r.u8()?,
            hits: r.u8()?, oxygen: r.u8()?, pension: r.u8()?, flags: r.u8()?, skill0: r.u8()?, skills: r.arr()?, skills_unmapped: r.arr()?,
            skill_progress: r.arr()?, state_c9: r.u8()?, f_ca: r.u8()?, f_cb: r.u8()?, inventory: r.u16s(19)?.try_into().unwrap(), actor: r.u16()?, armor: r.u16()?,
            weapon: r.u16()?, f_f8: r.u16()?, f_fa: r.u16()?, credits: r.u32()?,
        })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.bytes(&self.name); w.bytes(&self.attr); w.bytes(&self.attr_base); w.u8(self.service); w.u8(self.rank); w.u8(self.age); w.u8(self.hits_max); w.u8(self.hits_split);
        w.u8(self.hits); w.u8(self.oxygen); w.u8(self.pension); w.u8(self.flags); w.u8(self.skill0); w.bytes(&self.skills); w.bytes(&self.skills_unmapped);
        w.bytes(&self.skill_progress); w.u8(self.state_c9); w.u8(self.f_ca); w.u8(self.f_cb); for &c in &self.inventory { w.u16(c); } w.u16(self.actor); w.u16(self.armor);
        w.u16(self.weapon); w.u16(self.f_f8); w.u16(self.f_fa); w.u32(self.credits)
    }
}
impl Char {
    pub fn zero() -> Char { parse(&[0u8; 256], Char::read).unwrap() }
}
pub fn decode(v: Variant, d: &[u8]) -> Result<Vec<Char>> {
    let Variant::Mt1 = v;
    parse(d, |r| { let n = r.u8()? as usize; check("chars file size", d.len() as i64, (1 + 256 * n) as i64)?; Char::take_n(r, n) })
}
pub fn encode(v: Variant, p: &[Char]) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    let mut w = Vec::with_capacity(1 + 256 * p.len()); w.u8(fit("chars count", p.len())?); write_all(&mut w, p); Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    fn sl<const N: usize>(d: &[u8], a: usize) -> [u8; N] { d[a..a + N].try_into().unwrap() }
    #[test]
    fn record_layout_and_round_trip() {
        let d: [u8; 256] = std::array::from_fn(|i| i as u8);
        let c = parse(&d, Char::read).unwrap();
        assert_eq!(c, Char {
            name: sl(&d, 0), attr: sl(&d, 0x14), attr_base: sl(&d, 0x1a), service: d[0x20], rank: d[0x21], age: d[0x22], hits_max: d[0x23], hits_split: d[0x24],
            hits: d[0x25], oxygen: d[0x26], pension: d[0x27], flags: d[0x28], skill0: d[0x29], skills: sl(&d, 0x2a), skills_unmapped: sl(&d, 0x71),
            skill_progress: sl(&d, 0x79), state_c9: d[0xc9], f_ca: d[0xca], f_cb: d[0xcb], inventory: std::array::from_fn(|i| u16::from_le_bytes(sl(&d, 0xcc + 2 * i))),
            actor: u16::from_le_bytes(sl(&d, 0xf2)), armor: u16::from_le_bytes(sl(&d, 0xf4)), weapon: u16::from_le_bytes(sl(&d, 0xf6)), f_f8: u16::from_le_bytes(sl(&d, 0xf8)),
            f_fa: u16::from_le_bytes(sl(&d, 0xfa)), credits: u32::from_le_bytes(sl(&d, 0xfc)),
        });
        assert_eq!(c.to_bytes(), d);
        let e = encode(Variant::Mt1, &[c.clone(), Char::zero()]).unwrap();
        assert_eq!(e.len(), 513);
        assert_eq!(decode(Variant::Mt1, &e).unwrap(), [c, Char::zero()]);
        assert!(decode(Variant::Mt1, &e[..300]).is_err());
        assert!(encode(Variant::Mt1, &vec![Char::zero(); 256]).is_err());
    }
}
