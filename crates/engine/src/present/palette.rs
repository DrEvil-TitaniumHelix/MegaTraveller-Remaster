use crate::data::pal::Pal;

pub type Dac = [[u8; 3]; 256];
pub const DEFAULT_REGS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 16, 17, 18, 19, 20, 21, 22, 23];

pub fn ega_dac() -> Dac {
    let mut d = [[0u8; 3]; 256];
    for i in 0..64u8 {
        let mut rgb = [2, 1, 0].map(|b| 42 * ((i >> b) & 1) + if i & 16 != 0 { 21 } else { 0 });
        if i & 23 == 6 { rgb[1] = 21 }
        d[i as usize] = rgb;
    }
    d
}
pub fn expand(v: u8) -> u8 { (v << 2) | (v >> 4) }
pub fn colors(regs: &[u8; 16], dac: &Dac) -> [u32; 16] {
    regs.map(|r| { let [r, g, b] = dac[r as usize].map(expand); (r as u32) << 16 | (g as u32) << 8 | b as u32 })
}
pub fn regs(p: &Pal) -> Option<[u8; 16]> { (p.vals.len() >= 16).then(|| std::array::from_fn(|i| p.vals[i] & 63)) }

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn dac_and_colors() {
        let d = ega_dac();
        assert_eq!(d[0], [0, 0, 0]); assert_eq!(d[1], [0, 0, 42]); assert_eq!(d[4], [42, 0, 0]); assert_eq!(d[6], [42, 21, 0]);
        assert_eq!(d[7], [42, 42, 42]); assert_eq!(d[16], [21, 21, 21]); assert_eq!(d[23], [63, 63, 63]); assert_eq!(d[22], [63, 63, 21]);
        assert_eq!(d[8], [0, 0, 0]); assert_eq!(d[63], [63, 63, 63]); assert_eq!(d[64], [0, 0, 0]);
        assert_eq!(expand(63), 255); assert_eq!(expand(42), 170); assert_eq!(expand(21), 85);
        let c = colors(&DEFAULT_REGS, &d);
        assert_eq!(c[6], 0xAA5500); assert_eq!(c[15], 0xFFFFFF); assert_eq!(c[9], 0x5555FF);
        let pal = |v: Vec<u8>| Pal { vals: v, sep: " ".into(), end: "".into() };
        assert_eq!(regs(&pal(vec![0xff; 16])).unwrap(), [63u8; 16]);
        assert_eq!(regs(&pal((64..81).collect())).unwrap(), std::array::from_fn(|i| i as u8));
        assert!(regs(&pal(vec![1; 15])).is_none());
    }
}
