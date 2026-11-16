use mt_engine::data::{bin::{parse, Record}, chars::{self, Char}, Variant};
use mt_engine::sim::record::Slots;

pub type Work = [u8; 57];
pub const W_STATE: usize = 0; pub const W_NAME: usize = 1; pub const W_STAT: usize = 16; pub const W_SERVICE: usize = 22; pub const W_RANK: usize = 23;
pub const W_AGE: usize = 24; pub const W_HITS_A: usize = 27; pub const W_HITS_B: usize = 28; pub const W_ITEMS: usize = 30; pub const W_CASH: usize = 46;
pub const W_PENSION: usize = 50; pub const W_TAS: usize = 52; pub const W_SKILLS_PTR: usize = 53;
pub const POOL_SLOTS: usize = 15;
pub fn pool() -> Slots { Slots::new(Char::SIZE, POOL_SLOTS).unwrap() }
thread_local! { static NO_UNUSED_SKILLS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) }; }
pub const UNUSED_SKILLS: [u8; 34] = [1, 2, 3, 6, 7, 10, 12, 14, 20, 21, 24, 25, 27, 30, 31, 32, 33, 34, 37, 41, 44, 46, 47, 51, 52, 54, 55, 56, 58, 60, 63, 68, 70, 71];
pub fn set_no_unused_skills(on: bool) { NO_UNUSED_SKILLS.with(|v| v.set(on)) }
pub fn unused_skill(slot: u8) -> bool { NO_UNUSED_SKILLS.with(std::cell::Cell::get) && UNUSED_SKILLS.contains(&slot) }
pub fn shown_skill(slot: u8, level: u8) -> u8 { if unused_skill(slot) { 0 } else { level } }

pub struct CgTables<'a> { ds: &'a [u8] }
impl<'a> CgTables<'a> {
    pub const DS: usize = 0x9240;
    const MIN: usize = 0x1A00;
    pub fn new(exe: &'a [u8]) -> Option<CgTables<'a>> {
        let hdr = u16::from_le_bytes([*exe.get(8)?, *exe.get(9)?]) as usize * 16;
        let ds = exe.get(hdr + Self::DS..)?;
        (ds.len() >= Self::MIN).then_some(CgTables { ds })
    }
    pub fn bytes(&self, r: std::ops::Range<usize>) -> Option<&[u8]> { self.ds.get(r) }
    pub fn saved_index(&self, skill: u8) -> i8 { self.ds[0x1184 + 8 * skill as usize + 7] as i8 }
    pub fn item_type(&self, item: i8) -> u8 { self.ds[(0x16C2 + 3 * item as isize) as usize] }
    pub fn two_cell(&self, item: i8) -> bool { matches!(self.item_type(item), 8 | 12 | 13 | 15) }
}
pub fn add_to_pool(w: &Work, skills: &[(u8, u8)], t: &CgTables, before: &[u8; 256]) -> [u8; 256] {
    let mut r = *before;
    let n = w[W_NAME..W_NAME + 15].iter().position(|&b| b == 0).unwrap_or(15);
    r[..n].copy_from_slice(&w[W_NAME..W_NAME + n]); r[n] = 0;
    r[0x14..0x1A].copy_from_slice(&w[W_STAT..W_STAT + 6]); r[0x1A..0x20].copy_from_slice(&w[W_STAT..W_STAT + 6]);
    r[0x20] = w[W_SERVICE]; r[0x21] = w[W_RANK]; r[0x22] = w[W_AGE];
    r[0x24] = w[W_HITS_A]; r[0x23] = w[W_HITS_A].wrapping_add(w[W_HITS_B]); r[0x25] = r[0x23];
    r[0x26] = 45;
    let pension = u16::from_le_bytes([w[W_PENSION], w[W_PENSION + 1]]) as u32;
    r[0x27] = ((pension + 11) / 240) as u8; r[0x28] = w[W_TAS];
    r[0xC9] = 0; r[0xCA] = 0; r[0xCB] = 0; r[0xFA..0xFC].copy_from_slice(&30u16.to_le_bytes()); r[0xFC..0x100].copy_from_slice(&w[W_CASH..W_CASH + 4]);
    r[0x29..0x79].fill(0); r[0x79..0xC9].fill(0); r[0xCC..0xFA].fill(0);
    for &(skill, level) in skills { let i = t.saved_index(skill); if i != -1 { r[(0x29 + i as isize).rem_euclid(256) as usize] = level } }
    let mut items: [i8; 16] = std::array::from_fn(|i| w[W_ITEMS + i] as i8);
    let (mut inv, mut slot) = ([0u16; 23], 0usize);
    for i in (0..16).rev() {
        let it = items[i];
        if it != -1 && t.two_cell(it) {
            inv[slot] = (it as u16).wrapping_add(1); inv[slot + 1] = 0xFFFF; slot += 2; items[i] = -1;
            if slot >= 19 { break }
            if matches!(slot, 2 | 5 | 8 | 13) { slot += 1 }
        }
    }
    slot = 0;
    for i in (0..16).rev() {
        if slot >= 19 { break }
        while inv[slot] != 0 { slot += 1; if slot >= 19 { break } }
        if slot > 18 { break }
        let it = items[i];
        if it != -1 && !t.two_cell(it) { inv[slot] = (it as u16).wrapping_add(1); slot += 1 }
    }
    for (k, v) in inv.iter().enumerate() { r[0xCC + 2 * k..0xCE + 2 * k].copy_from_slice(&v.to_le_bytes()) }
    r
}
pub fn save_pool(p: &Slots) -> Result<Vec<u8>, mt_engine::data::bin::Error> {
    let recs: Vec<Char> = p.occupied().map(|(_, b)| parse(b, Char::read)).collect::<Result<_, _>>()?;
    chars::encode(Variant::Mt1, &recs)
}
pub const DS_USED: usize = 0x2740; pub const DS_NEXT: usize = 0x277E; pub const DS_COUNT: usize = 0x2780; pub const DS_SAVE_OK: usize = 0x2781; pub const DS_DISK: usize = 0x2783;
pub const DS_POOL: usize = 0x1840;
pub fn pool_count(ds: &[u8]) -> u8 { ds.get(DS_COUNT).copied().unwrap_or(0) }
pub fn next_free(ds: &[u8]) -> u16 { u16::from_le_bytes([ds.get(DS_NEXT).copied().unwrap_or(0), ds.get(DS_NEXT + 1).copied().unwrap_or(0)]) }
pub fn load_pool(p: &mut Slots, ds: &mut [u8], file: &[u8]) -> (u8, u16) {
    let mut at = 0usize;
    let mut read = |n: usize| -> &[u8] { let e = (at + n).min(file.len()); let r = &file[at.min(e)..e]; at = e; r };
    let ds_write = |ds: &mut [u8], off: usize, b: &[u8]| for (k, &v) in b.iter().enumerate() { if let Some(d) = ds.get_mut(off.wrapping_add(k) & 0xFFFF) { *d = v } };
    ds_write(ds, DS_COUNT, read(1));
    let c = pool_count(ds) as usize;
    if c < POOL_SLOTS { ds_write(ds, DS_USED + c, &vec![0u8; POOL_SLOTS - c]) }
    if c != 0 { ds_write(ds, DS_USED, &vec![1u8; c]) }
    let mut i = 0usize;
    while pool_count(ds) as usize > i {
        let r = read(256);
        if i < POOL_SLOTS { if let Some(s) = p.get_mut(i).and_then(|s| s.get_mut(..r.len())) { s.copy_from_slice(r) } ds_write(ds, DS_POOL + (i << 8), r); }
        else { ds_write(ds, DS_POOL + (i << 8), r) }
        i += 1;
    }
    let n = pool_count(ds);
    ds_write(ds, DS_NEXT, &(n as u16).to_le_bytes());
    for k in 0..POOL_SLOTS { p.mark(k, ds.get(DS_USED + k).is_some_and(|&u| u != 0)) }
    (n, next_free(ds))
}
#[cfg(test)]
mod t {
    use super::*;
    fn exe() -> Vec<u8> {
        let mut e = vec![0u8; CgTables::DS + 0x1A00];
        for id in 0..128usize { e[CgTables::DS + 0x1184 + 8 * id + 7] = id as u8 } e[CgTables::DS + 0x1184 + 8 * 5 + 7] = 0xFF;
        e[CgTables::DS + 0x1184 + 8 * 200 + 7] = 0xD6;
        e[CgTables::DS + 0x16C2 + 3 * 3] = 8; e[CgTables::DS + 0x16C2 + 3 * 9] = 15; e
    }
    #[test]
    fn record_conversion_and_pool() {
        let e = exe(); let t = CgTables::new(&e).unwrap();
        assert_eq!((t.saved_index(5), t.saved_index(7), t.two_cell(3), t.two_cell(4)), (-1, 7, true, false));
        assert!(CgTables::new(&e[..0x9000]).is_none()); assert!(CgTables::new(&e[..CgTables::DS + 0x19FF]).is_none());
        for id in 0..=255u8 { t.saved_index(id); } for id in i8::MIN..=i8::MAX { t.item_type(id); }
        assert_eq!(t.bytes(0x1184..0x1186), Some(&[0u8, 0][..])); assert!(t.bytes(0x19FF..0x1A01).is_none());
        let mut w: Work = [0; 57];
        w[W_NAME..W_NAME + 6].copy_from_slice(b"GOLDEN"); w[W_STAT..W_STAT + 6].copy_from_slice(&[7, 8, 9, 10, 11, 12]);
        w[W_SERVICE] = 3; w[W_RANK] = 2; w[W_AGE] = 34; w[W_HITS_A] = 20; w[W_HITS_B] = 250; w[W_ITEMS..W_ITEMS + 16].fill(0xFF);
        w[W_ITEMS] = 4; w[W_ITEMS + 1] = 3; w[W_ITEMS + 15] = 9; w[W_ITEMS + 14] = 1;
        w[W_CASH..W_CASH + 4].copy_from_slice(&40000u32.to_le_bytes()); w[W_PENSION..W_PENSION + 2].copy_from_slice(&10000u16.to_le_bytes()); w[W_TAS] = 1;
        let before: [u8; 256] = std::array::from_fn(|i| i as u8);
        let r = add_to_pool(&w, &[(7, 2), (5, 9), (0x20, 1)], &t, &before);
        assert_eq!(&r[..8], b"GOLDEN\0\x07"); assert_eq!(&r[8..0x14], &before[8..0x14]);
        assert_eq!(&r[0x14..0x20], &[7, 8, 9, 10, 11, 12, 7, 8, 9, 10, 11, 12]);
        assert_eq!(&r[0x20..0x29], &[3, 2, 34, 14, 20, 14, 45, 41, 1]);
        assert_eq!((r[0x29 + 7], r[0x29 + 0x20], r[0x29 + 5]), (2, 1, 0));
        assert_eq!(add_to_pool(&w, &[(200, 3)], &t, &before)[255], 3);
        let inv: Vec<u16> = (0..23).map(|k| u16::from_le_bytes([r[0xCC + 2 * k], r[0xCD + 2 * k]])).collect();
        assert_eq!(&inv[..7], &[10, 0xFFFF, 2, 4, 0xFFFF, 5, 0]); assert!(inv[7..].iter().all(|&v| v == 0));
        assert_eq!((u16::from_le_bytes([r[0xFA], r[0xFB]]), u32::from_le_bytes(r[0xFC..].try_into().unwrap())), (30, 40000));
        assert_eq!((r[0xC9], r[0xCA], r[0xCB]), (0, 0, 0));
        let mut p = pool(); assert!(p.put(2, &r)); assert!(p.put(0, &before));
        let f = save_pool(&p).unwrap();
        assert_eq!(f.len(), 513); assert_eq!(f[0], 2); assert_eq!(&f[1..257], &before); assert_eq!(&f[257..], &r);
        let (mut q, mut ds) = (pool(), vec![0u8; 0x2840]);
        assert_eq!(load_pool(&mut q, &mut ds, &f), (2, 2)); assert_eq!((q.count(), q.get(1).unwrap(), q.first_free()), (2, &r[..], Some(2)));
    }
    #[test]
    fn load_pool_mimics_2736() {
        let rec = |v: u8| [v; 256];
        let file = |c: u8, recs: &[[u8; 256]]| { let mut f = vec![c]; for r in recs { f.extend_from_slice(r) } f };
        let fresh = || { let mut ds = vec![0u8; 0x10000]; ds[DS_DISK] = 0xFF; (pool(), ds) };
        let (mut p, mut ds) = fresh(); let recs: Vec<_> = (1..=15u8).map(rec).collect();
        assert_eq!(load_pool(&mut p, &mut ds, &file(15, &recs)), (15, 15));
        assert_eq!((p.count(), p.first_free(), p.get(14).unwrap()[0], ds[DS_USED + 15], ds[DS_DISK]), (15, None, 15, 0, 0xFF));
        let (mut p, mut ds) = fresh(); let mut r15 = rec(0xAA); r15[0] = 0; r15[0x3E] = 0x34; r15[0x3F] = 0x12; r15[0x40] = 16; r15[0x41] = 0x42; r15[0x43] = 0x43; r15[0xFF] = 0xEE;
        let mut recs: Vec<_> = (1..=15u8).map(rec).collect(); recs.push(r15);
        assert_eq!(load_pool(&mut p, &mut ds, &file(16, &recs)), (16, 16));
        assert_eq!((p.count(), p.first_free(), ds[DS_SAVE_OK], ds[DS_DISK], ds[0x283F], ds[0x2840]), (14, Some(0), 0x42, 0x43, 0xEE, 0));
        assert_eq!(&ds[DS_NEXT..DS_NEXT + 3], &[16, 0, 16]);
        let (mut p, mut ds) = fresh(); let mut r15 = rec(0xAA); r15[0x10] = 0x55; r15[0x40] = 3;
        let mut recs: Vec<_> = (1..=15u8).map(rec).collect(); recs.push(r15); recs.push(rec(0xBB));
        assert_eq!(load_pool(&mut p, &mut ds, &file(17, &recs)), (3, 3)); assert_eq!((ds[0x2750], ds[0x2840], p.count()), (0x55, 0, 15));
        let (mut p, mut ds) = fresh(); recs[15][0x40] = 17;
        assert_eq!(load_pool(&mut p, &mut ds, &file(17, &recs)), (17, 17)); assert_eq!((ds[0x2840], ds[0x293F], ds[0x2940]), (0xBB, 0xBB, 0));
        let (mut p, mut ds) = fresh(); let mut big: Vec<_> = (0..0xE9u8).map(rec).collect(); big[15][0x40] = 0xE9;
        assert_eq!(load_pool(&mut p, &mut ds, &file(18, &big)), (0xE9, 0xE9));
        assert_eq!((ds[0xFF40], ds[0xFFFF], ds[0], ds[0x3F], ds[0x40], ds[0x13F], ds[0x140]), (0xE7, 0xE7, 0xE7, 0xE7, 0xE8, 0xE8, 0));
        let (mut p, mut ds) = fresh(); assert!(p.put(3, &rec(0x77)));
        assert_eq!(load_pool(&mut p, &mut ds, &file(255, &big)), (1, 1));
        assert_eq!((p.count(), p.get(0).unwrap()[0], p.get(3).unwrap()[0], p.get(1).unwrap()[0]), (15, 0, 0x77, 0));
        assert_eq!((ds[DS_SAVE_OK], ds[DS_DISK], ds[0x283E], ds[0x283F]), (1, 1, 1, 0));
        let (mut p, mut ds) = fresh(); assert_eq!(load_pool(&mut p, &mut ds, &file(65, &big)), (1, 1)); assert_eq!(ds[DS_SAVE_OK], 0);
        let (mut p, mut ds) = fresh(); assert_eq!(load_pool(&mut p, &mut ds, &[64]), (64, 64)); assert_eq!((p.count(), p.get(0).unwrap()[0], ds[0x2782]), (15, 0, 0));
        let (mut p, mut ds) = fresh(); ds[DS_COUNT] = 7; ds[DS_USED..DS_USED + 15].fill(9); assert!(p.put(9, &rec(1)));
        assert_eq!(load_pool(&mut p, &mut ds, &[]), (7, 7)); assert_eq!((p.count(), p.first_free(), p.get(9).unwrap()[0]), (7, Some(7), 1));
        let (mut p, mut ds) = fresh(); assert!(p.put(2, &rec(0x99))); let mut f = file(3, &[rec(1), rec(2)]); f.extend_from_slice(&[0x33; 100]);
        assert_eq!(load_pool(&mut p, &mut ds, &f), (3, 3)); assert_eq!((p.count(), p.get(2).unwrap()[99], p.get(2).unwrap()[100]), (3, 0x33, 0x99));
        let (mut p, mut ds) = fresh(); let mut f = file(16, &recs[..15]); f.extend_from_slice(&[0u8; 0x40]);
        assert_eq!(load_pool(&mut p, &mut ds, &f), (16, 16)); assert_eq!((p.count(), p.first_free(), ds[DS_USED + 14]), (0, Some(0), 0));
        let mut tiny = Slots::new(4, 2).unwrap(); assert_eq!(load_pool(&mut tiny, &mut [0u8; 16], &file(2, &[rec(1), rec(2)])), (0, 0));
        assert_eq!(load_pool(&mut pool(), &mut vec![0u8; 0x2780], &file(2, &[rec(1), rec(2)])), (0, 0));
    }
}
