use super::fb::{Fb, H, PLANE, W};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Raster {
    Clip,
    Wrap,
    Planar,
}

fn dither(c: u8, y: i64, x: i64) -> u8 { if c < 16 { c } else if (x + y) & 1 == 0 { c & 15 } else { c >> 4 } }

impl Fb {
    pub fn pixel(&mut self, r: Raster, page: usize, y: i32, x: i32, c: u8, op: u8) {
        super::textlog::decorated(self, r, page, y as i64, x as i64, 1, 1);
        self.px(r, page, y as i64, x as i64, c, op)
    }
    pub fn line(&mut self, r: Raster, page: usize, y1: i32, x1: i32, y2: i32, x2: i32, c: u8, op: u8) {
        if super::textlog::writes_enabled() { self.ln::<true>(r, page, y1 as i64, x1 as i64, y2 as i64, x2 as i64, c, op) }
        else { self.ln::<false>(r, page, y1 as i64, x1 as i64, y2 as i64, x2 as i64, c, op) }
    }
    pub fn fill(&mut self, r: Raster, page: usize, y: i32, x: i32, h: i32, w: i32, c: u8, op: u8) {
        let (y, x, w) = (y as i64, x as i64, w as i64);
        if op == 24 { super::textlog::xor(self, page, y, x.min(x + w - 1), h as i64, (w - 1).abs() + 1, c, r == Raster::Clip || w != 1); }
        else if h <= 16 && w.abs() <= 16 { super::textlog::decorated(self, r, page, y, x.min(x + w - 1), h as i64, (w - 1).abs() + 1); }
        else { super::textlog::written(self, r, page, y, x.min(x + w - 1), h as i64, (w - 1).abs() + 1); }
        for row in 0..h as i64 { self.ln::<false>(r, page, y + row, x, y + row, x + w - 1, c, op) }
    }
    pub fn rect(&mut self, r: Raster, page: usize, y1: i32, x1: i32, y2: i32, x2: i32, c: u8, op: u8) {
        let (y1, x1, y2, x2) = (y1 as i64, x1 as i64, y2 as i64, x2 as i64);
        for y in [y1, y2] { super::textlog::decorated(self, r, page, y, x1.min(x2), 1, (x2 - x1).abs() + 1); }
        if y2 > y1 { for x in [x1, x2] { super::textlog::decorated(self, r, page, y1 + 1, x, y2 - y1 - 1, 1); } }
        self.ln::<false>(r, page, y1, x1, y1, x2, c, op); self.ln::<false>(r, page, y2, x1, y2, x2, c, op);
        if y2 > y1 { self.ln::<false>(r, page, y1 + 1, x1, y2 - 1, x1, c, op); self.ln::<false>(r, page, y1 + 1, x2, y2 - 1, x2, c, op) }
    }
    fn px(&mut self, r: Raster, page: usize, y: i64, x: i64, c: u8, op: u8) {
        if r == Raster::Clip && !((0..H as i64).contains(&y) && (0..W as i64).contains(&x)) { return }
        let a = if r == Raster::Planar {
            ((page as i64).wrapping_mul(super::fb::PAGE as i64).wrapping_add(y.wrapping_mul(40)).wrapping_add(x >> 3) & 0xffff) as usize
        } else { Fb::addr64(page, y, x) };
        let bit = 128u8 >> (x & 7);
        for p in 0..4 {
            let at = &mut self.planes[p * PLANE + a];
            let (old, src) = (*at, if c >> p & 1 != 0 { bit } else { 0 });
            let src = match op { 8 => src & old, 16 => src | old & bit, 24 => src ^ old & bit, _ => src };
            *at = old & !bit | src;
        }
    }
    fn ln<const RETIRE: bool>(&mut self, r: Raster, page: usize, mut y1: i64, mut x1: i64, mut y2: i64, mut x2: i64, c: u8, op: u8) {
        if RETIRE && (x1 == x2 || y1 == y2) {
            super::textlog::written(self, r, page, y1.min(y2), x1.min(x2), (y2 - y1).abs() + 1, (x2 - x1).abs() + 1);
        }
        if r == Raster::Wrap && x1 == x2 { for y in y1.min(y2)..=y1.max(y2) { self.px(r, page, y, x1, c, op) } return }
        if x1 > x2 { (x1, x2, y1, y2) = (x2, x1, y2, y1) }
        let (dx, dy, step) = (x2 - x1, (y2 - y1).abs(), if y2 >= y1 { 1 } else { -1 });
        if dy == 0 { if r == Raster::Wrap { self.span(page, y1, x1, x2, c, op) } else { for x in x1..=x2 { self.px(r, page, y1, x, dither(c, y1, x), op) } } }
        else if dx == 0 { for y in y1.min(y2)..=y1.max(y2) { self.px(r, page, y, x1, c, op) } }
        else {
            let (major, minor) = if dy > dx { (dy, dx) } else { (dx, dy) };
            let (mut err, mut y, mut x) = (2 * minor - major, y1, x1);
            for _ in 0..=major {
                if RETIRE { super::textlog::written(self, r, page, y, x, 1, 1); }
                self.px(r, page, y, x, c, op);
                if dy > dx { y += step } else { x += 1 }
                if err >= 0 { if dy > dx { x += 1 } else { y += step } err -= 2 * major }
                err += 2 * minor;
            }
        }
    }
    fn span(&mut self, page: usize, y: i64, left: i64, right: i64, c: u8, op: u8) {
        let start = (page as i64).wrapping_mul(super::fb::PAGE as i64).wrapping_add(y.wrapping_mul(super::fb::ROW as i64)).wrapping_add(left >> 3) & 0xffff;
        let length = (right >> 3) - (left >> 3) + 1;
        if left < 0 || right > 65535 || start + length > 65536 { for x in left..=right { self.px(Raster::Wrap, page, y, x, dither(c, y, x), op) } return }
        let (first, last) = (255u8 >> (left & 7), (255u32 << (7 - (right & 7)) & 255) as u8);
        let combine = |old: u8, ink: u8, mask: u8| { let ink = match op { 8 => ink & old, 16 => ink | old, 24 => ink ^ old, _ => ink }; old & !mask | ink & mask };
        let (s, e) = (start as usize, (start + length - 1) as usize);
        for p in 0..4 {
            let ink = if c < 16 { if c >> p & 1 != 0 { 255 } else { 0 } } else {
                let (even, odd) = if y & 1 != 0 { (0x55, 0xaa) } else { (0xaa, 0x55) };
                (if c >> p & 1 != 0 { even } else { 0 }) | (if (c >> 4) >> p & 1 != 0 { odd } else { 0 })
            };
            let pl = &mut self.planes[p * PLANE..(p + 1) * PLANE];
            if length == 1 { pl[s] = combine(pl[s], ink, first & last); continue }
            pl[s] = combine(pl[s], ink, first); pl[e] = combine(pl[e], ink, last);
            for b in &mut pl[s + 1..e] { *b = match op { 0 => ink, 8 => *b & ink, 16 => *b | ink, _ => *b ^ ink } }
        }
    }
    pub fn blit(&mut self, page: usize, src: &[u8], sw: usize, y: i32, x: i32, sy: i32, sx: i32, h: i32, w: i32, transparent: Option<u8>) {
        if sw == 0 { return }
        let (sw, sh) = (sw as i64, (src.len() / sw) as i64);
        let (y, x, sy, sx, w) = (y as i64, x as i64, sy as i64, sx as i64, w as i64);
        for row in 0..h as i64 {
            if !((0..H as i64).contains(&(y + row)) && (0..sh).contains(&(sy + row))) { continue }
            let (left, right) = (0.max(-x).max(-sx), w.min(W as i64 - x).min(sw - sx));
            if super::textlog::enabled() && left < right {
                let mut start = left;
                while start < right {
                    if transparent == Some(src[((sy + row) * sw + sx + start) as usize]) { start += 1; continue; }
                    let mut end = start + 1;
                    while end < right && transparent != Some(src[((sy + row) * sw + sx + end) as usize]) { end += 1; }
                    super::textlog::written(self, Raster::Clip, page, y + row, x + start, 1, end - start);
                    start = end;
                }
            }
            for i in left..right {
                let c = src[((sy + row) * sw + sx + i) as usize];
                if transparent != Some(c) { self.px(Raster::Clip, page, y + row, x + i, c, 0) }
            }
        }
    }
    pub fn copy_rect(&mut self, src: usize, dst: usize, y: i32, x: i32, h: i32, w: i32) {
        super::textlog::copy(self, Raster::Clip, src, y as i64, x as i64, h as i64, w as i64, dst, y as i64, x as i64);
        let _annotations = super::textlog::suppress();
        let s = self.indexed(src); self.blit(dst, &s, W as usize, y, x, y, x, h, w, None)
    }
}

#[cfg(test)]
mod t {
    use super::*;
    const C: Raster = Raster::Clip;
    const S: Raster = Raster::Wrap;
    const M: i32 = i32::MAX;
    const N: i32 = i32::MIN;
    fn px(fb: &Fb, page: usize, y: i32, x: i32) -> u8 { fb.get(page, y, x) }
    #[test]
    fn rules() {
        let mut fb = Fb::new();
        fb.pixel(C, 0, 0, 320, 5, 0); fb.pixel(C, 0, 200, 0, 5, 0); fb.pixel(C, 0, -1, 0, 5, 0); assert!(fb.planes.iter().all(|&b| b == 0));
        fb.pixel(S, 0, 0, 320, 5, 0); assert_eq!(px(&fb, 0, 1, 0), 5);
        fb.pixel(S, 0, 1, 0, 0x3a, 16); assert_eq!(px(&fb, 0, 1, 0), 15);
        fb.pixel(S, 0, 1, 0, 6, 8); assert_eq!(px(&fb, 0, 1, 0), 6); fb.pixel(S, 0, 1, 0, 3, 24); assert_eq!(px(&fb, 0, 1, 0), 5);
        fb.line(C, 1, 10, 0, 10, 7, 0x12, 0); assert_eq!((0..8).map(|x| px(&fb, 1, 10, x)).collect::<Vec<_>>(), [2, 1, 2, 1, 2, 1, 2, 1]);
        fb.line(C, 1, 11, 0, 11, 3, 0x12, 0); assert_eq!((0..4).map(|x| px(&fb, 1, 11, x)).collect::<Vec<_>>(), [1, 2, 1, 2]);
        fb.line(S, 1, 12, 0, 12, 7, 0x12, 0); fb.line(S, 1, 13, 0, 13, 3, 0x12, 0); assert_eq!((0..4).map(|x| px(&fb, 1, 13, x)).collect::<Vec<_>>(), [1, 2, 1, 2]); assert_eq!(px(&fb, 1, 12, 7), 1);
        fb.line(C, 1, 20, 5, 20, 5, 0x12, 0); fb.line(S, 1, 21, 5, 21, 5, 0x12, 0); assert_eq!((px(&fb, 1, 20, 5), px(&fb, 1, 21, 5)), (1, 2));
        fb.line(S, 1, 30, 9, 40, 3, 4, 0); assert_eq!(px(&fb, 1, 30, 9), 4); assert_eq!(px(&fb, 1, 40, 3), 4); assert_eq!(px(&fb, 1, 35, 6), 4);
        fb.fill(C, 1, 50, 10, 1, 0, 7, 0); assert_eq!((px(&fb, 1, 50, 9), px(&fb, 1, 50, 10), px(&fb, 1, 50, 11)), (7, 7, 0));
        fb.fill(C, 1, 60, 0, -1, 5, 7, 0); assert_eq!(px(&fb, 1, 60, 0), 0);
        fb.rect(C, 1, 70, 0, 69, 5, 9, 0); assert_eq!((px(&fb, 1, 70, 3), px(&fb, 1, 69, 3)), (9, 9));
        fb.rect(C, 1, 80, 2, 84, 0, 9, 0); assert_eq!((px(&fb, 1, 82, 0), px(&fb, 1, 82, 2), px(&fb, 1, 82, 1)), (9, 9, 0));
        let img = [1, 2, 3, 4, 5, 6];
        fb.blit(2, &img, 3, 0, -1, 0, 0, 2, 3, Some(5)); assert_eq!((px(&fb, 2, 0, 0), px(&fb, 2, 0, 1), px(&fb, 2, 1, 0), px(&fb, 2, 1, 1)), (2, 3, 0, 6));
        fb.copy_rect(2, 3, 0, 0, 2, 2); assert_eq!((px(&fb, 3, 0, 1), px(&fb, 3, 1, 1), px(&fb, 3, 0, 2)), (3, 6, 0));
        fb.planes[8192 + 20 * 40..8192 + 20 * 40 + 3].fill(255); fb.line(S, 1, 20, 0, 20, 23, 1, 1); assert_eq!(fb.planes[8192 + 20 * 40..8192 + 20 * 40 + 3], [255, 0, 255]);
        fb.planes[8192 + 21 * 40..8192 + 21 * 40 + 3].fill(255); fb.line(S, 1, 21, -1, 21, 23, 1, 1); assert_eq!(fb.planes[8192 + 21 * 40..8192 + 21 * 40 + 3], [255, 255, 255]);
    }
    #[test]
    fn extreme_arguments_no_panic() {
        let mut fb = Fb::new();
        fb.line(C, 0, M, 1, M, 1, 0x12, 0); fb.line(C, 0, 0, M - 1, 1, M, 1, 0); fb.line(C, 0, M - 1, 0, M, 1, 1, 0);
        fb.fill(C, 0, M, 0, 2, 1, 1, 0); fb.fill(C, 0, 0, M, 1, 1, 1, 0); fb.fill(C, 0, 0, N, 1, 0, 1, 0); fb.rect(C, 0, M - 1, M - 1, M, M, 1, 0);
        fb.pixel(S, 0, M, M, 0, 0); fb.pixel(S, 0, N, N, 0, 16); fb.line(S, 0, M, 0, M, 10, 0, 0); fb.line(S, 0, N, N, N, N + 5, 0, 0);
        fb.blit(0, &[], 0, 0, 0, 0, 0, 1, 0, None); fb.blit(0, &[1], 1, M, 0, 0, 0, 2, 1, None); fb.blit(0, &[1], 1, 0, 0, M, 0, 2, 1, None);
        fb.blit(0, &[1], 1, 0, N, 0, 0, 1, 1, None); fb.blit(0, &[1], 1, 0, N + 1, 0, 0, 1, 1, None); fb.blit(0, &[1], 1, 0, 0, 0, N, 1, 1, None); fb.blit(0, &[1], 1, 5, 5, 0, 0, 2, M, None);
        fb.copy_rect(1, 0, M, 0, 2, 320); fb.copy_rect(1, 0, 0, N, 1, 5);
        assert_eq!(fb.get(0, 5, 5), 1); assert_eq!(fb.get(0, 5, 6), 0);
        assert_eq!(fb.planes.iter().filter(|&&b| b != 0).count(), 1);
    }
    #[test]
    #[ignore = "2^31-step lines: cargo test --release -p mt-engine --lib -- --ignored huge_lines"]
    fn huge_lines() {
        let mut fb = Fb::new();
        fb.line(C, 0, 0, N, 1, 0, 1, 0);
        assert_eq!((px(&fb, 0, 1, 0), px(&fb, 0, 0, 0)), (1, 0)); assert_eq!(fb.planes.iter().filter(|&&b| b != 0).count(), 1);
        fb.line(C, 1, 0, 0, 1 << 30, 1 << 30, 1, 0);
        assert!((0..200).all(|k| px(&fb, 1, k, k) == 1)); assert_eq!(px(&fb, 1, 0, 1), 0);
        fb.line(C, 2, 0, 0, 1 << 29, 1 << 30, 1, 0);
        assert_eq!([(0, 0), (1, 1), (1, 2), (2, 3), (2, 4), (3, 5)].map(|(y, x)| px(&fb, 2, y, x)), [1; 6]); assert_eq!(px(&fb, 2, 2, 2), 0);
        fb.line(C, 3, 0, 0, 0, N, 1, 0);
        assert_eq!(px(&fb, 3, 0, 0), 1); assert_eq!(fb.plane(0)[3 * 8192..4 * 8192].iter().filter(|&&b| b != 0).count(), 1);
    }
}
