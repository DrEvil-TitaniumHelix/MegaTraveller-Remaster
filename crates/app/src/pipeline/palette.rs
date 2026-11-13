use crate::look::{Look, Palette};
pub fn map(indexed: &[u8], regs: &[u8; 16], look: &Look) -> Vec<u32> {
    let colors = mt_engine::present::palette::colors(regs, &mt_engine::present::palette::ega_dac());
    let colors = colors.map(|color| {
        let [r, g, b] = [color >> 16, (color >> 8) & 255, color & 255];
        let tint = match look.palette {
            Palette::Ega => return color,
            Palette::Warm => return r << 16 | ((g * 235 + 127) / 255) << 8 | (b * 205 + 127) / 255,
            Palette::Amber => [255, 191, 0],
            Palette::Green => [96, 255, 96],
            Palette::Custom => look.custom_tint,
        };
        let luma = (77 * r + 150 * g + 29 * b + 128) >> 8;
        let [r, g, b] = tint.map(|c| (u32::from(c) * luma + 127) / 255);
        r << 16 | g << 8 | b
    });
    indexed.iter().map(|&i| colors[(i & 15) as usize]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::present::palette::{colors, ega_dac, DEFAULT_REGS};
    #[test]
    fn ega_matches_engine_for_every_index_and_register() {
        let indexed: [u8; 64000] = std::array::from_fn(|i| i as u8);
        for bank in 0..16 {
            let regs = std::array::from_fn(|i| (bank * 16 + i) as u8);
            let expected = colors(&regs, &ega_dac());
            let actual = map(&indexed, &regs, &Look::default());
            assert_eq!(actual.len(), indexed.len());
            for (&index, &pixel) in indexed.iter().zip(&actual) { assert_eq!(pixel, expected[(index & 15) as usize]); }
        }
    }
    #[test]
    fn tint_endpoints_and_luminance() {
        let indexed: [u8; 64000] = std::array::from_fn(|i| (i % 16) as u8);
        for (palette, white) in [(Palette::Warm, 0xffebcd), (Palette::Amber, 0xffbf00),
            (Palette::Green, 0x60ff60), (Palette::Custom, 0x123456)] {
            let look = Look { palette, custom_tint: [0x12, 0x34, 0x56], ..Look::default() };
            let out = map(&indexed, &DEFAULT_REGS, &look);
            assert_eq!(out[0], 0); assert_eq!(out[15], white);
            assert!(out.iter().all(|&p| p <= 0xffffff));
        }
        let look = Look { palette: Palette::Custom, custom_tint: [255; 3], ..Look::default() };
        let gray = map(&indexed, &DEFAULT_REGS, &look);
        assert_eq!((gray[4], gray[2], gray[1]), (0x333333, 0x646464, 0x131313));
        assert_eq!(gray[7], 0xaaaaaa);
        assert!(map(&indexed, &DEFAULT_REGS, &Look { custom_tint: [0; 3], ..look }).iter().all(|&p| p == 0));
    }
}
