use super::bin::{check, fit, parse, write_all, Error, Rd, Record, Result, Wr};
use super::{container, Variant};

pub type Cell = [u8; 2];
pub fn wrle_dec(r: &mut Rd, need: usize) -> Result<Vec<Cell>> {
    let mut w = Vec::new();
    while w.len() < need {
        let n = r.i16()?;
        if n < 0 { let c: Cell = r.arr()?; w.extend(std::iter::repeat(c).take(-(n as i32) as usize)); }
        else { for _ in 0..n { w.push(r.arr()?); } }
    }
    check("wrle word count", w.len() as i64, need as i64)?;
    Ok(w)
}
pub fn wrle_enc(w: &[Cell], o: &mut Vec<u8>) -> Result<()> {
    let (mut lit, mut i, n) = (Vec::<Cell>::new(), 0, w.len());
    let flush = |lit: &mut Vec<Cell>, o: &mut Vec<u8>| -> Result<()> { if !lit.is_empty() { o.i16(fit("wrle literal count", lit.len())?); for c in lit.iter() { o.bytes(c); } lit.clear(); } Ok(()) };
    while i < n {
        let mut r = 1;
        while i + r < n && w[i + r] == w[i] { r += 1; }
        if r >= 3 || i + r == n {
            flush(&mut lit, o)?;
            o.i16(i16::try_from(-(r as i64)).map_err(|_| Error::Range { what: "wrle run", got: r, bits: 16 })?); o.bytes(&w[i]);
        } else { lit.extend_from_slice(&w[i..i + r]); }
        i += r;
    }
    flush(&mut lit, o)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MapHdr { pub rows: u8, pub cols: u8, pub name: [u8; 8], pub hdr_rest: [u8; 16] }
impl Record for MapHdr {
    const SIZE: usize = 0x1A;
    fn read(r: &mut Rd) -> Result<Self> { Ok(MapHdr { rows: r.u8()?, cols: r.u8()?, name: r.arr()?, hdr_rest: r.arr()? }) }
    fn write(&self, w: &mut Vec<u8>) { w.u8(self.rows); w.u8(self.cols); w.bytes(&self.name); w.bytes(&self.hdr_rest) }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map { pub hdr: MapHdr, pub cells: Vec<Cell> }
impl Map {
    pub fn read(r: &mut Rd) -> Result<Map> {
        let hdr = MapHdr::take(r)?;
        let cells = wrle_dec(r, hdr.rows as usize * 12 * hdr.cols as usize * 20)?;
        Ok(Map { hdr, cells })
    }
    pub fn write(&self, w: &mut Vec<u8>) -> Result<()> { self.hdr.write(w); wrle_enc(&self.cells, w) }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AreaHdr {
    pub f00: [u8; 9], pub scene_ids: [u8; 5], pub uwp: [u8; 7], pub f15: u8, pub f16: u8, pub level_mask: u8, pub f18: u8,
    pub ly_a: u8, pub ly_b: u8, pub spawn_base: u8, pub pic_ids: [u8; 4], pub start_x: u16, pub start_y: u16,
    pub description: [u8; 150], pub slogan: [u8; 150], pub text3: [u8; 80], pub place_names: [[u8; 40]; 5],
}
impl Record for AreaHdr {
    const SIZE: usize = 0x268;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(AreaHdr {
            f00: r.arr()?, scene_ids: r.arr()?, uwp: r.arr()?, f15: r.u8()?, f16: r.u8()?, level_mask: r.u8()?, f18: r.u8()?,
            ly_a: r.u8()?, ly_b: r.u8()?, spawn_base: r.u8()?, pic_ids: r.arr()?, start_x: r.u16()?, start_y: r.u16()?,
            description: r.arr()?, slogan: r.arr()?, text3: r.arr()?,
            place_names: [r.arr()?, r.arr()?, r.arr()?, r.arr()?, r.arr()?],
        })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.bytes(&self.f00); w.bytes(&self.scene_ids); w.bytes(&self.uwp); w.u8(self.f15); w.u8(self.f16); w.u8(self.level_mask); w.u8(self.f18);
        w.u8(self.ly_a); w.u8(self.ly_b); w.u8(self.spawn_base); w.bytes(&self.pic_ids); w.u16(self.start_x); w.u16(self.start_y);
        w.bytes(&self.description); w.bytes(&self.slogan); w.bytes(&self.text3); for p in &self.place_names { w.bytes(p); }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WObj { pub id: u16, pub state: u8, pub x: u16, pub y: u16, pub r#type: u8, pub f8: u8, pub f9: u8, pub fa: u8, pub fb: u8, pub fc: [u8; 14], pub f1a: u16, pub f1c: [u8; 4] }
impl Record for WObj {
    const SIZE: usize = 32;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(WObj { id: r.u16()?, state: r.u8()?, x: r.u16()?, y: r.u16()?, r#type: r.u8()?, f8: r.u8()?, f9: r.u8()?, fa: r.u8()?, fb: r.u8()?, fc: r.arr()?, f1a: r.u16()?, f1c: r.arr()? })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.u16(self.id); w.u8(self.state); w.u16(self.x); w.u16(self.y); w.u8(self.r#type); w.u8(self.f8); w.u8(self.f9); w.u8(self.fa); w.u8(self.fb); w.bytes(&self.fc); w.u16(self.f1a); w.bytes(&self.f1c)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn { pub level: u8, pub item: u8, pub x: u16, pub y: u16 }
impl Record for Spawn {
    const SIZE: usize = 6;
    fn read(r: &mut Rd) -> Result<Self> { Ok(Spawn { level: r.u8()?, item: r.u8()?, x: r.u16()?, y: r.u16()? }) }
    fn write(&self, w: &mut Vec<u8>) { w.u8(self.level); w.u8(self.item); w.u16(self.x); w.u16(self.y) }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Area { pub hdr: AreaHdr, pub level_offs: [u16; 6], pub map_offs: Vec<usize>, pub maps: Vec<Map>, pub objects: Vec<WObj>, pub spawns: Vec<Spawn> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chunk { Area(Area), Inside(Map) }
const INSIDE: &[u8] = b"\x01\x01INSIDE";

impl Chunk {
    pub fn decode(e: &[u8]) -> Result<Chunk> {
        if e.starts_with(INSIDE) { return Ok(Chunk::Inside(parse(e, Map::read)?)); }
        parse(e, |r| {
            let hdr = AreaHdr::take(r)?;
            let level_offs: [u16; 6] = r.u16s(6)?.try_into().unwrap();
            let oo = level_offs[5] as usize;
            let (mut map_offs, mut maps) = (vec![], vec![]);
            while r.p < oo { map_offs.push(r.p); maps.push(Map::read(r)?); }
            check("area object-list offset", r.p as i64, oo as i64)?;
            let n = r.u8()? as usize; let objects = WObj::take_n(r, n)?;
            let n = r.u8()? as usize; let spawns = Spawn::take_n(r, n)?;
            Ok(Chunk::Area(Area { hdr, level_offs, map_offs, maps, objects, spawns }))
        })
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        let mut w = Vec::new();
        match self {
            Chunk::Inside(m) => m.write(&mut w)?,
            Chunk::Area(a) => {
                a.hdr.write(&mut w); for &o in &a.level_offs { w.u16(o); }
                for m in &a.maps { m.write(&mut w)?; }
                w.u8(fit("area object count", a.objects.len())?); write_all(&mut w, &a.objects);
                w.u8(fit("area spawn count", a.spawns.len())?); write_all(&mut w, &a.spawns);
            }
        }
        Ok(w)
    }
}
pub fn decode(v: Variant, d: &[u8]) -> Result<Vec<Chunk>> {
    let Variant::Mt1 = v;
    container::decode(d)?.into_iter().map(Chunk::decode).collect()
}
pub fn encode(v: Variant, w: &[Chunk]) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    container::encode(&w.iter().map(Chunk::encode).collect::<Result<Vec<_>>>()?)
}

#[cfg(test)]
mod t {
    use super::*;
    fn sl<const N: usize>(d: &[u8], a: usize) -> [u8; N] { d[a..a + N].try_into().unwrap() }
    #[test]
    fn record_layouts() {
        let d: [u8; 0x1A] = std::array::from_fn(|i| i as u8);
        let h = parse(&d, MapHdr::read).unwrap();
        assert_eq!(h, MapHdr { rows: d[0], cols: d[1], name: sl(&d, 2), hdr_rest: sl(&d, 0xa) });
        assert_eq!(h.to_bytes(), d);
        let d: [u8; 0x268] = std::array::from_fn(|i| i as u8);
        let h = parse(&d, AreaHdr::read).unwrap();
        assert_eq!(h, AreaHdr { f00: sl(&d, 0), scene_ids: sl(&d, 9), uwp: sl(&d, 0xe), f15: d[0x15], f16: d[0x16], level_mask: d[0x17], f18: d[0x18],
            ly_a: d[0x19], ly_b: d[0x1a], spawn_base: d[0x1b], pic_ids: sl(&d, 0x1c), start_x: u16::from_le_bytes(sl(&d, 0x20)), start_y: u16::from_le_bytes(sl(&d, 0x22)),
            description: sl(&d, 0x24), slogan: sl(&d, 0xba), text3: sl(&d, 0x150), place_names: std::array::from_fn(|i| sl(&d, 0x1a0 + 40 * i)) });
        assert_eq!(h.to_bytes(), d);
        let d: [u8; 32] = std::array::from_fn(|i| i as u8);
        let o = parse(&d, WObj::read).unwrap();
        assert_eq!(o, WObj { id: u16::from_le_bytes(sl(&d, 0)), state: d[2], x: u16::from_le_bytes(sl(&d, 3)), y: u16::from_le_bytes(sl(&d, 5)), r#type: d[7], f8: d[8], f9: d[9],
            fa: d[0xa], fb: d[0xb], fc: sl(&d, 0xc), f1a: u16::from_le_bytes(sl(&d, 0x1a)), f1c: sl(&d, 0x1c) });
        assert_eq!(o.to_bytes(), d);
        let d: [u8; 6] = std::array::from_fn(|i| i as u8);
        let s = parse(&d, Spawn::read).unwrap();
        assert_eq!(s, Spawn { level: d[0], item: d[1], x: u16::from_le_bytes(sl(&d, 2)), y: u16::from_le_bytes(sl(&d, 4)) });
        assert_eq!(s.to_bytes(), d);
    }
    #[test]
    fn wrle_packer_rule() {
        let w: Vec<Cell> = vec![[1, 0], [2, 0], [5, 5], [5, 5], [5, 5], [7, 0], [7, 0]];
        let mut o = vec![]; wrle_enc(&w, &mut o).unwrap();
        assert_eq!(o, [2, 0, 1, 0, 2, 0, 0xfd, 0xff, 5, 5, 0xfe, 0xff, 7, 0]);
        assert_eq!(wrle_dec(&mut Rd::new(&o), 7).unwrap(), w);
        assert!(wrle_dec(&mut Rd::new(&o), 6).is_err());
        assert!(wrle_dec(&mut Rd::new(&o), usize::MAX).is_err());
        let mut o = vec![]; wrle_enc(&vec![[5, 5]; 32768], &mut o).unwrap();
        assert_eq!(o, [0x00, 0x80, 5, 5]);
        assert_eq!(wrle_dec(&mut Rd::new(&o), 32768).unwrap().len(), 32768);
        assert!(matches!(wrle_enc(&vec![[5, 5]; 32769], &mut vec![]), Err(Error::Range { what: "wrle run", got: 32769, bits: 16 })));
        let alt: Vec<Cell> = (0..65536u32).map(|i| [(i & 1) as u8, 0]).collect();
        assert!(matches!(wrle_enc(&alt, &mut vec![]), Err(Error::Range { what: "wrle literal count", .. })));
    }
    #[test]
    fn inside_and_area_round_trip() {
        let hdr = MapHdr { rows: 1, cols: 1, name: *b"INSIDE\0\0", hdr_rest: [0; 16] };
        let m = Map { hdr, cells: vec![[3, 1]; 240] };
        let e = Chunk::Inside(m.clone()).encode().unwrap();
        assert_eq!(e.len(), 0x1A + 4);
        assert_eq!(Chunk::decode(&e).unwrap(), Chunk::Inside(m.clone()));
        let mut g = m.clone(); g.hdr.name = *b"G1\0\0\0\0\0\0";
        let a = Area { hdr: parse(&[0u8; 0x268], AreaHdr::read).unwrap(), level_offs: [0x274, 5237, 0, 0, 0, 0x274 + 0x1E], map_offs: vec![0x274], maps: vec![g],
            objects: vec![WObj { id: 1, state: 2, x: 3, y: 4, r#type: 5, f8: 0, f9: 0, fa: 0, fb: 0, fc: [9; 14], f1a: 0x1234, f1c: [1, 2, 3, 4] }], spawns: vec![Spawn { level: 0, item: 7, x: 8, y: 9 }] };
        let e = Chunk::encode(&Chunk::Area(a.clone())).unwrap();
        assert_eq!(Chunk::decode(&e).unwrap(), Chunk::Area(a.clone()));
        let w = encode(Variant::Mt1, &[Chunk::Area(a.clone()), Chunk::Inside(m)]).unwrap();
        assert_eq!(encode(Variant::Mt1, &decode(Variant::Mt1, &w).unwrap()).unwrap(), w);
        let mut full = a; full.objects = vec![full.objects[0].clone(); 256];
        assert!(Chunk::Area(full).encode().is_err());
    }
}
