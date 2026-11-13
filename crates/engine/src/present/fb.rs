use super::palette::{self, Dac, DEFAULT_REGS};

pub const W: i32 = 320;
pub const H: i32 = 200;
pub const ROW: usize = 40;
pub const PAGE: usize = 8192;
pub const PLANE: usize = 65536;
pub const SNAP: usize = 4 * PLANE + 16;

pub struct Fb { pub planes: Vec<u8>, pub regs: [u8; 16], pub(super) textlog: std::cell::RefCell<Option<super::textlog::Surface>> }
impl Default for Fb { fn default() -> Fb { Fb::new() } }
impl Fb {
    pub fn new() -> Fb { Fb { planes: vec![0; 4 * PLANE], regs: DEFAULT_REGS, textlog: std::cell::RefCell::new(None) } }
    pub fn plane(&self, p: usize) -> &[u8] { &self.planes[p * PLANE..(p + 1) * PLANE] }
    pub fn plane_mut(&mut self, p: usize) -> &mut [u8] {
        super::textlog::clear(self);
        &mut self.planes[p * PLANE..(p + 1) * PLANE]
    }
    pub fn addr(page: usize, y: i32, x: i32) -> usize { Fb::addr64(page, y as i64, x as i64) }
    pub(super) fn addr64(page: usize, y: i64, x: i64) -> usize { ((page as i64).wrapping_mul(PAGE as i64).wrapping_add(y.wrapping_mul(ROW as i64)).wrapping_add((x & 0xffff) >> 3) & 0xffff) as usize }
    pub fn get(&self, page: usize, y: i32, x: i32) -> u8 {
        let (a, bit) = (Fb::addr(page, y, x), 128u8 >> (x & 7));
        (0..4).map(|p| ((self.planes[p * PLANE + a] & bit != 0) as u8) << p).sum()
    }
    pub fn set_reg(&mut self, i: usize, v: u8) { if i < 16 { self.regs[i] = v & 63 } }
    pub fn indexed(&self, page: usize) -> Vec<u8> {
        let base = page * PAGE;
        (0..64000).map(|i| { let (a, bit) = (base + i / 8, 128u8 >> (i % 8)); (0..4).map(|p| ((self.planes[p * PLANE + a] & bit != 0) as u8) << p).sum() }).collect()
    }
    pub fn rgb(&self, page: usize, dac: &Dac) -> Vec<u32> {
        let c = palette::colors(&self.regs, dac);
        let rgb = self.indexed(page).into_iter().map(|i| c[i as usize]).collect::<Vec<_>>();
        super::textlog::publish(self, page, &rgb);
        rgb
    }
    pub fn copy(&mut self, src: usize, sy: i32, sx: i32, h: i32, w: i32, dst: usize, dy: i32, dx: i32) {
        super::textlog::copy(self, super::prim::Raster::Wrap, src, sy as i64, sx as i64 * 8, h as i64, w as i64 * 8, dst, dy as i64, dx as i64 * 8);
        let at = |page: usize, y: i64, x: i64| ((page as i64).wrapping_mul(PAGE as i64).wrapping_add(y.wrapping_mul(ROW as i64)).wrapping_add(x) & 0xffff) as usize;
        let (sy, sx, dy, dx) = (sy as i64, sx as i64, dy as i64, dx as i64);
        for row in 0..h as i64 { for col in 0..w as i64 {
            let (a, b) = (at(src, sy + row, sx + col), at(dst, dy + row, dx + col));
            for p in 0..4 { self.planes[p * PLANE + b] = self.planes[p * PLANE + a] }
        } }
    }
    pub fn snapshot(&self) -> Vec<u8> { let mut v = Vec::with_capacity(self.planes.len() + self.regs.len()); v.extend_from_slice(&self.planes); v.extend_from_slice(&self.regs); v }
    pub fn restore(&mut self, d: &[u8]) -> bool {
        if d.len() != SNAP { return false }
        *self.textlog.get_mut() = None;
        self.planes = d[..4 * PLANE].to_vec(); self.regs.copy_from_slice(&d[4 * PLANE..]); true
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn addressing_and_snapshot() {
        assert_eq!(Fb::addr(0, 0, 0), 0); assert_eq!(Fb::addr(1, 1, 9), 8192 + 41); assert_eq!(Fb::addr(0, 0, -1), 8191); assert_eq!(Fb::addr(0, -1, 0), 65496);
        assert_eq!(Fb::addr(7, 204, 200), (57344 + 8160 + 25) & 0xffff);
        assert_eq!(Fb::addr(usize::MAX, 0, 0), 57344); assert_eq!(Fb::addr((i64::MAX as usize) / 8192, 1000, 0), 31808);
        assert_eq!(Fb::addr(0, i32::MAX, i32::MIN), ((i32::MAX as i64 * 40 + ((i32::MIN as i64 & 0xffff) >> 3)) & 0xffff) as usize);
        let mut fb = Fb::new();
        fb.planes[0] = 0x80; fb.planes[PLANE * 3] = 0x80; fb.planes[PLANE + 8192 + 40] = 0x01;
        assert_eq!(fb.get(0, 0, 0), 9); assert_eq!(fb.get(1, 1, 7), 2); assert_eq!(fb.get(1, 0, 0), 0);
        let ix = fb.indexed(1); assert_eq!(ix.len(), 64000); assert_eq!(ix[327], 2); assert_eq!(ix.iter().filter(|&&c| c != 0).count(), 1);
        assert_eq!(fb.rgb(0, &palette::ega_dac())[0], 0x5555FF);
        fb.copy(0, 0, 0, 1, 1, 1, 0, 0); assert_eq!(fb.get(1, 0, 0), 9);
        fb.copy(1, 0, 0, 1, 2, 1, 0, 1); assert_eq!(fb.plane(0)[8192..8195], [0x80, 0x80, 0x80]);
        fb.set_reg(3, 0xff); assert_eq!(fb.regs[3], 63); fb.set_reg(17, 1); fb.set_reg(16, 1); fb.set_reg(usize::MAX, 1); assert_eq!(fb.regs, { let mut r = DEFAULT_REGS; r[3] = 63; r });
        fb.copy(usize::MAX, i32::MAX, i32::MIN, 2, 2, 0, i32::MIN, i32::MAX); fb.copy(0, i32::MAX, i32::MAX, 1, 1, 7, i32::MIN, i32::MIN);
        let s = fb.snapshot(); assert_eq!(s.len(), SNAP); assert_eq!(s.capacity(), SNAP);
        let mut g = Fb::new(); g.planes.clear(); assert!(!g.restore(&s[1..])); assert!(g.restore(&s)); assert_eq!(g.snapshot(), s);
    }
}
