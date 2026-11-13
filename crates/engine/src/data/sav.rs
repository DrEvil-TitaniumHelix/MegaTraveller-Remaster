use super::bin::{check, parse, write_all, Rd, Record, Result, Wr};
use super::chars::Char;
use super::Variant;

pub const SIZE: usize = 0xF86;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavHdr {
    pub alive_mask: u8, pub order: [u8; 5], pub world: u8, pub area: u8, pub object_bits: [u8; 50], pub npc_bits: [u8; 50], pub spawn_bits: [u8; 21],
    pub flag_81: u8, pub air_counter: u8, pub atmosphere_timer: u8, pub hw_config: [u8; 4], pub money: u32, pub ship_name: [u8; 20],
    pub stations: [u8; 12], pub programs: [u8; 20], pub ship_model: u8, pub ship_flags: u8, pub ship_damage: [u8; 4], pub hull_damage: u8,
    pub f_c7: u8, pub gun_bays: [u8; 2], pub cargo: [u8; 20], pub supplies: [u8; 20], pub f_f2: [u8; 10], pub fuel: u8, pub fuel_refined: u8,
    pub ship_stats: [u8; 4],
}
impl Record for SavHdr {
    const SIZE: usize = 0x102;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(SavHdr {
            alive_mask: r.u8()?, order: r.arr()?, world: r.u8()?, area: r.u8()?, object_bits: r.arr()?, npc_bits: r.arr()?, spawn_bits: r.arr()?,
            flag_81: r.u8()?, air_counter: r.u8()?, atmosphere_timer: r.u8()?, hw_config: r.arr()?, money: r.u32()?, ship_name: r.arr()?,
            stations: r.arr()?, programs: r.arr()?, ship_model: r.u8()?, ship_flags: r.u8()?, ship_damage: r.arr()?, hull_damage: r.u8()?,
            f_c7: r.u8()?, gun_bays: r.arr()?, cargo: r.arr()?, supplies: r.arr()?, f_f2: r.arr()?, fuel: r.u8()?, fuel_refined: r.u8()?,
            ship_stats: r.arr()?,
        })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.u8(self.alive_mask); w.bytes(&self.order); w.u8(self.world); w.u8(self.area); w.bytes(&self.object_bits); w.bytes(&self.npc_bits); w.bytes(&self.spawn_bits);
        w.u8(self.flag_81); w.u8(self.air_counter); w.u8(self.atmosphere_timer); w.bytes(&self.hw_config); w.u32(self.money); w.bytes(&self.ship_name);
        w.bytes(&self.stations); w.bytes(&self.programs); w.u8(self.ship_model); w.u8(self.ship_flags); w.bytes(&self.ship_damage); w.u8(self.hull_damage);
        w.u8(self.f_c7); w.bytes(&self.gun_bays); w.bytes(&self.cargo); w.bytes(&self.supplies); w.bytes(&self.f_f2); w.u8(self.fuel); w.u8(self.fuel_refined);
        w.bytes(&self.ship_stats)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavObj { pub item: u8, pub qty: u8, pub charge: u8, pub f3: u8, pub f4: u8 }
impl Record for SavObj {
    const SIZE: usize = 5;
    fn read(r: &mut Rd) -> Result<Self> { Ok(SavObj { item: r.u8()?, qty: r.u8()?, charge: r.u8()?, f3: r.u8()?, f4: r.u8()? }) }
    fn write(&self, w: &mut Vec<u8>) { w.u8(self.item); w.u8(self.qty); w.u8(self.charge); w.u8(self.f3); w.u8(self.f4) }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Sav { pub hdr: SavHdr, pub chars: Vec<Char>, pub unreferenced_902: [u8; 384], pub objects: Vec<SavObj>, pub lights: u16, pub clock: u16 }
pub fn decode(v: Variant, d: &[u8]) -> Result<Sav> {
    let Variant::Mt1 = v;
    check("sav size", d.len() as i64, SIZE as i64)?;
    parse(d, |r| Ok(Sav { hdr: SavHdr::take(r)?, chars: Char::take_n(r, 8)?, unreferenced_902: r.arr()?, objects: SavObj::take_n(r, 256)?, lights: r.u16()?, clock: r.u16()? }))
}
pub fn encode(v: Variant, p: &Sav) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    check("sav chars", p.chars.len() as i64, 8)?; check("sav objects", p.objects.len() as i64, 256)?;
    let mut w = Vec::with_capacity(SIZE);
    p.hdr.write(&mut w); write_all(&mut w, &p.chars); w.bytes(&p.unreferenced_902); write_all(&mut w, &p.objects); w.u16(p.lights); w.u16(p.clock); Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    fn sl<const N: usize>(d: &[u8], a: usize) -> [u8; N] { d[a..a + N].try_into().unwrap() }
    #[test]
    fn layout_and_round_trip() {
        let d: Vec<u8> = (0..SIZE).map(|i| i as u8).collect();
        let s = decode(Variant::Mt1, &d).unwrap();
        assert_eq!(s.hdr, SavHdr {
            alive_mask: d[0], order: sl(&d, 1), world: d[6], area: d[7], object_bits: sl(&d, 8), npc_bits: sl(&d, 0x3a), spawn_bits: sl(&d, 0x6c),
            flag_81: d[0x81], air_counter: d[0x82], atmosphere_timer: d[0x83], hw_config: sl(&d, 0x84), money: u32::from_le_bytes(sl(&d, 0x88)), ship_name: sl(&d, 0x8c),
            stations: sl(&d, 0xa0), programs: sl(&d, 0xac), ship_model: d[0xc0], ship_flags: d[0xc1], ship_damage: sl(&d, 0xc2), hull_damage: d[0xc6],
            f_c7: d[0xc7], gun_bays: sl(&d, 0xc8), cargo: sl(&d, 0xca), supplies: sl(&d, 0xde), f_f2: sl(&d, 0xf2), fuel: d[0xfc], fuel_refined: d[0xfd],
            ship_stats: sl(&d, 0xfe),
        });
        assert_eq!(s.chars.len(), 8);
        for (i, c) in s.chars.iter().enumerate() { assert_eq!(*c, parse(&d[0x102 + 256 * i..0x202 + 256 * i], Char::read).unwrap()); }
        assert_eq!(s.unreferenced_902[..], d[0x902..0xa82]);
        assert_eq!(s.objects.len(), 256);
        for (i, o) in s.objects.iter().enumerate() { let p = 0xa82 + 5 * i; assert_eq!(*o, SavObj { item: d[p], qty: d[p + 1], charge: d[p + 2], f3: d[p + 3], f4: d[p + 4] }); }
        assert_eq!((s.lights, s.clock), (u16::from_le_bytes(sl(&d, 0xf82)), u16::from_le_bytes(sl(&d, 0xf84))));
        assert_eq!(encode(Variant::Mt1, &s).unwrap(), d);
        assert!(decode(Variant::Mt1, &d[..SIZE - 1]).is_err());
        let mut short = s.clone(); short.chars.pop(); assert!(encode(Variant::Mt1, &short).is_err());
        let mut long = s; long.objects.push(SavObj { item: 0, qty: 0, charge: 0, f3: 0, f4: 0 }); assert!(encode(Variant::Mt1, &long).is_err());
    }
}
