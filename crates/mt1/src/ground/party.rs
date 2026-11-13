use crate::chargen::tables::{Cg, Ds};
use mt_engine::sim::rng::Rng;

fn word(b: &[u8], p: usize) -> u16 { u16::from_le_bytes([b[p], b[p + 1]]) }
fn put(b: &mut [u8], p: usize, value: u16) { b[p..p + 2].copy_from_slice(&value.to_le_bytes()) }

pub fn initial() -> Vec<u8> {
    let mut b = vec![0; super::state::Mem::PARTY_SIZE];
    for i in 0..256 { b[0xA82 + 5 * i] = 0xFF }
    b
}

pub fn selected(cg: &Cg) -> Vec<Option<[u8; 256]>> {
    cg.party.iter().map(|&slot| slot.checked_sub(1)
        .and_then(|s| cg.pool.get(s as usize)).and_then(|r| r.try_into().ok())).collect()
}

pub fn block(before: &[u8], party: &[Option<[u8; 256]>], ds: &Ds, rng: &mut Rng) -> Option<Vec<u8>> {
    if before.len() != super::state::Mem::PARTY_SIZE { return None }
    let mut b = before.to_vec();
    let glyph = |item: u16| ds.u8(0x16BF + 3 * item as usize);
    b[0] = 0;
    for (p, value) in [(0xA82, 0x56), (0xA87, 0x57), (0xA8C, 0x59)] { b[p] = value }
    let mut count = 3u16;
    for member in 0..5 {
        let rec = 0x102 + 256 * member;
        let inv = rec + 0xCC;
        if let Some(source) = party.get(member).and_then(Option::as_ref) {
            b[0] |= 1 << member;
            b[rec..rec + 256].copy_from_slice(source);
            let mut icon = 0x28;
            for cell in 0..19 {
                let item = word(&b, inv + 2 * cell);
                if item != 0xFFFF && glyph(item) == 0x23 {
                    let first = word(&b, inv);
                    put(&mut b, inv + 2 * cell, first); put(&mut b, inv, item);
                    icon = 0x29; break;
                }
            }
            b[0xA82 + 5 * count as usize] = icon;
            count += 1;
            put(&mut b, rec + 0xF2, count);
            b[rec + 0x26] = 45; b[rec + 0xC9] = 2;
        } else {
            b[rec..rec + 256].fill(0); b[rec + 0x28] = 2;
        }
        b[member + 1] = member as u8; b[member + 0xA0] = member as u8 + 1;
    }
    for (p, value) in [(0xC0, 2), (0xC8, 2), (0xC1, 0x81), (0xAC, 3), (0xAE, 4),
                       (0xAF, 1), (0xB0, 5), (0xFC, 125), (0x82, 14), (0x83, 30)] { b[p] = value }
    for member in 0..5 {
        let rec = 0x102 + member * 256;
        if b[rec + 0x25] == 0 { continue }
        for cell in 0..19 {
            let inv = rec + 0xCC + 2 * cell;
            let item = word(&b, inv);
            if item as i16 <= 0 { continue }
            let (obj, kind) = (0xA82 + count as usize * 5, glyph(item));
            b[obj] = kind;
            let mut equipped = false;
            if kind == 0x23 || kind == 0x20 {
                b[obj + 1] = 5; b[obj + 2] = if kind == 0x23 { 40 } else { 30 };
                if word(&b, rec + 0xF4) == 0 {
                    put(&mut b, rec + 0xF4, count + 1); put(&mut b, inv, 0); equipped = true;
                }
            } else if kind != 0x39 {
                b[obj + 1] = (rng.next() % 4 + 2) as u8; b[obj + 2] = (rng.next() % 4) as u8;
                if word(&b, rec + 0xF6) == 0 {
                    put(&mut b, rec + 0xF6, count + 1); put(&mut b, inv, 0);
                    if word(&b, inv + 2) == 0xFFFF { put(&mut b, inv + 2, 0) }
                    equipped = true;
                }
            }
            if !equipped { put(&mut b, inv, count + 1) }
            count += 1;
        }
    }
    b[8..0x6C].fill(0);
    for (p, value) in [(0x1EE, 1), (0x1F0, 2), (0x1F2, 3), (0x88, 198), (0x8A, 0)] { put(&mut b, p, value) }
    b[0x80] = 1; b[6] = 2;
    for i in 0..11 { b[0x8C + i] = ds.u8(0x2A1B + i) }
    Some(b)
}
#[cfg(test)]
mod t {
    #[test]
    fn rejects_bad_block_before_consuming_rng() {
        let ds = crate::chargen::tables::Ds::from_exe(vec![0; 0x10000]).unwrap();
        let mut rng = mt_engine::sim::rng::Rng::new(7);
        assert!(super::block(&[0; 100], &[], &ds, &mut rng).is_none());
        assert_eq!(rng.state, 7); assert!(rng.trace.is_empty());
    }
}
