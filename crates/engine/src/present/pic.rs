use super::fb::{Fb, PLANE, ROW};
use crate::data::pic::Pic;

pub fn width(p: &Pic) -> usize { p.wbytes4 as usize * 8 }
pub fn consistent(p: &Pic) -> bool { p.px.len() as u64 == p.h as u64 * p.wbytes4 as u64 * 4 }
pub fn unpack(p: &Pic) -> Vec<u8> { p.px.iter().flat_map(|&b| [b >> 4, b & 15]).collect() }
pub fn planar(p: &Pic) -> Vec<u8> {
    if !consistent(p) { return vec![] }
    let (px, w, wb) = (unpack(p), width(p), p.wbytes4 as usize);
    let px = &px;
    (0..4).flat_map(|pl| (0..p.h as usize * wb).map(move |i| (0..8).fold(0u8, |b, k| b | (px[(i / wb) * w + (i % wb) * 8 + k] >> pl & 1) << (7 - k)))).collect()
}
pub fn draw_rect(fb: &mut Fb, page: usize, pic: &Pic, y: i32, x: i32, sy: i32, sx: i32, h: i32, w: i32, transparent: Option<u8>) {
    if consistent(pic) && pic.wbytes4 > 0 { fb.blit(page, &unpack(pic), width(pic), y, x, sy, sx, h, w, transparent) }
}
pub fn draw(fb: &mut Fb, page: usize, pic: &Pic, y: i32, x: i32, transparent: Option<u8>) { draw_rect(fb, page, pic, y, x, 0, 0, pic.h as i32, width(pic) as i32, transparent) }
pub fn icon(fb: &mut Fb, page: usize, small: &Pic, big: &Pic, i: i32, y: i32, x: i32) {
    let (sheet, cols, cw, i) = if i >= 100 { (big, 6i64, 48i64, i as i64 - 100) } else { (small, 12, 24, i as i64) };
    let sat = |v: i64| v.clamp(i32::MIN as i64, i32::MAX as i64) as i32;
    draw_rect(fb, page, sheet, y, x, sat(i.div_euclid(cols) * 15), sat(i.rem_euclid(cols) * cw), 15, cw as i32, None)
}
pub fn upload(fb: &mut Fb, src: &[u8], sstride: usize, dst: usize, dstride: usize, rows: u16, width: u16, op: u8, mask: u8) {
    let (rows, width) = (if rows == 0 { 65536 } else { rows as usize }, if width == 0 { if op != 0 { 65536 } else { 0 } } else { width as usize });
    let si = |row: usize, col: usize| row.wrapping_mul(sstride).wrapping_add(col) & 0xffff;
    if src.len() < 65536 && (0..rows).any(|row| (0..width).any(|col| si(row, col) >= src.len())) { return }
    if mask & 15 != 0 && super::textlog::enabled() {
        for row in 0..rows { super::textlog::written_bytes(fb, dst.wrapping_add(row.wrapping_mul(dstride)), width); }
    }
    for row in 0..rows { for col in 0..width {
        let (v, a) = (src[si(row, col)], dst.wrapping_add(row.wrapping_mul(dstride)).wrapping_add(col) & 0xffff);
        for p in (0..4).filter(|p| mask >> p & 1 != 0) {
            let at = &mut fb.planes[p * PLANE + a];
            *at = match op { 8 => v & *at, 16 => v | *at, 24 => v ^ *at, _ => v }
        }
    } }
}
pub fn atlas(fb: &mut Fb, pic: &Pic, dst: usize) {
    let (pl, n) = (planar(pic), pic.h as usize * pic.wbytes4 as usize);
    if n == 0 || pl.is_empty() { return }
    for p in 0..4 { upload(fb, &pl[p * n..(p + 1) * n], pic.wbytes4 as usize, dst, ROW, pic.h, pic.wbytes4, 0, 1 << p) }
}

#[cfg(test)]
mod t {
    use super::*;
    fn pic(h: u16, wb: u16, px: Vec<u8>) -> Pic { Pic { h, wbytes4: wb, pal: [0; 16], px, tail: vec![] } }
    #[test]
    fn unpack_blit_upload() {
        let p = pic(2, 1, vec![0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc, 0xde, 0xf1]);
        assert_eq!(unpack(&p), [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 1]); assert_eq!(width(&p), 8);
        let pl = planar(&p); assert_eq!(pl.len(), 8); assert_eq!((pl[0], pl[1], pl[7]), (0xaa, 0xab, 0xfe));
        let mut fb = Fb::new();
        draw(&mut fb, 0, &p, 0, -1, Some(1)); assert_eq!((fb.get(0, 0, 0), fb.get(0, 1, 5), fb.get(0, 1, 6), fb.get(0, 0, 7)), (2, 15, 0, 0));
        draw_rect(&mut fb, 1, &p, 5, 5, 1, 2, 1, 3, None); assert_eq!((fb.get(1, 5, 5), fb.get(1, 5, 7), fb.get(1, 5, 8)), (11, 13, 0));
        let small = pic(30, 36, (0..30 * 144).map(|i| if i / 144 >= 15 && i % 144 >= 12 && i % 144 < 24 { 0x33 } else { 0 }).collect());
        let big = pic(15, 36, vec![0x44; 15 * 144]);
        icon(&mut fb, 2, &small, &big, 13, 0, 0); assert_eq!((fb.get(2, 0, 0), fb.get(2, 14, 23), fb.get(2, 0, 24)), (3, 3, 0));
        icon(&mut fb, 3, &small, &big, 101, 0, 0); assert_eq!((fb.get(3, 0, 47), fb.get(3, 0, 48)), (4, 0));
        let mut fb = Fb::new(); fb.planes[2 * PLANE + 0x4000] = 0xf0;
        upload(&mut fb, &[0x0f, 0xff, 0x01], 2, 0x4000, 40, 1, 2, 24, 0b0101);
        assert_eq!((fb.plane(0)[0x4000], fb.plane(0)[0x4001], fb.plane(2)[0x4000], fb.plane(1)[0x4000]), (0x0f, 0xff, 0xff, 0));
        upload(&mut fb, &[0xaa; 2], 1, 0xffff, 1, 2, 1, 0, 15); assert_eq!((fb.plane(3)[0xffff], fb.plane(3)[0]), (0xaa, 0xaa));
        upload(&mut fb, &[0x11; 4], 2, 0x100, 1, 1, 0, 0, 15); assert_eq!(fb.plane(0)[0x100], 0);
        atlas(&mut fb, &p, 0x4000); assert_eq!((fb.plane(0)[0x4000], fb.plane(3)[0x4000 + 40]), (0xaa, 0xfe));
    }
    #[test]
    fn extremes() {
        let mut fb = Fb::new();
        let bad = pic(1, 1, vec![]);
        assert!(!consistent(&bad) && planar(&bad).is_empty());
        draw(&mut fb, 0, &bad, 0, 0, None); draw_rect(&mut fb, 0, &bad, 0, 0, 0, 0, 1, 8, None); atlas(&mut fb, &bad, 0);
        atlas(&mut fb, &pic(0, 1, vec![]), 0);
        let sheet = pic(30, 36, vec![0x33; 30 * 144]);
        icon(&mut fb, 0, &sheet, &sheet, 1_717_987_024, 0, 0);
        icon(&mut fb, 0, &sheet, &sheet, -1, 0, 0);
        icon(&mut fb, 0, &sheet, &sheet, -13, 0, 0);
        icon(&mut fb, 0, &sheet, &sheet, i32::MIN, 0, 0); icon(&mut fb, 0, &sheet, &sheet, i32::MAX, i32::MIN, i32::MAX);
        upload(&mut fb, &[], 1, 0, 1, 1, 1, 0, 15); upload(&mut fb, &[1, 2], 1, 0, 1, 0, 3, 16, 15);
        assert!(fb.planes.iter().all(|&b| b == 0));
        upload(&mut fb, &[0x5a; 65536], usize::MAX, usize::MAX, 7, 3, 1, 0, 1);
        assert_eq!((fb.plane(0)[0xffff], fb.plane(0)[6], fb.plane(0)[13], fb.plane(0)[0]), (0x5a, 0x5a, 0x5a, 0));
        upload(&mut fb, &[0x5a; 65536], 1, usize::MAX, 1, 1, 2, 0, 2);
        assert_eq!((fb.plane(1)[0xffff], fb.plane(1)[0]), (0x5a, 0x5a));
    }
}
