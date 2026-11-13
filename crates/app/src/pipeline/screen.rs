use crate::look::{Look, Screen};
use super::{background, scale::PictureRect};

pub fn apply(rgb: &[u32], width: usize, height: usize, look: &Look) -> Vec<u32> {
    let rect = PictureRect { width, height, out_width: width, out_height: height, ..PictureRect::default() };
    compose(rgb, rect, look, &mut CrtCache::default())
}

#[derive(Default)]
pub(super) struct CrtCache {
    width: usize,
    height: usize,
    samples: Vec<CrtSample>,
    #[cfg(test)]
    builds: usize,
}
#[derive(Clone, Copy)]
struct CrtSample {
    column: u32,
    row: u32,
}
impl CrtCache {
    pub(super) fn prepare(&mut self, width: usize, height: usize) -> bool {
        if self.width == width && self.height == height && !self.samples.is_empty() { return true; }
        let Some(len) = width.checked_mul(height).filter(|&n| n != 0 && n <= 64 * 1024 * 1024) else { return false; };
        self.width = 0; self.height = 0; self.samples.clear();
        if self.samples.try_reserve_exact(len).is_err() { return false; }
        let half_w = width as f32 * 0.5;
        let half_h = height as f32 * 0.5;
        let mut columns = Vec::new();
        let mut gains = Vec::new();
        if columns.try_reserve_exact(width).is_err() || gains.try_reserve_exact(height).is_err() { return false; }
        columns.extend((0..width).map(|x| {
            let cx = x as f32 + 0.5 - half_w;
            let xn = cx / half_w;
            (cx, 1.0 + 0.035 * xn * xn)
        }));
        gains.extend((0..height).map(|y| if scan_gain(y, height) == 192 { 1 << 31 } else { 0 }));
        for y in 0..height {
            let cy = y as f32 + 0.5 - half_h;
            let yn = cy / half_h;
            let stretch_x = 1.0 + 0.035 * yn * yn;
            for &(cx, stretch_y) in &columns {
                let sx = half_w + cx * stretch_x;
                let sy = half_h + cy * stretch_y;
                let sample = if sx < 0.0 || sy < 0.0 || sx >= width as f32 || sy >= height as f32 {
                    CrtSample { column: u32::MAX, row: 0 }
                } else {
                    let (sx, sy) = (sx as usize, sy as usize);
                    CrtSample { column: sx as u32 | gains[sy], row: (sy * width) as u32 }
                };
                self.samples.push(sample);
            }
        }
        self.width = width; self.height = height;
        #[cfg(test)] { self.builds += 1; }
        true
    }
}

pub(super) fn compose(rgb: &[u32], rect: PictureRect, look: &Look, cache: &mut CrtCache) -> Vec<u32> {
    let (width, height) = (rect.width, rect.height);
    if width.checked_mul(height) != Some(rgb.len()) || rgb.is_empty() {
        return background::compose(rect, look, |_, _| 0);
    }
    match look.screen {
        Screen::None => background::compose(rect, look, |x, y| rgb[y * width + x]),
        Screen::Scanlines => background::compose(rect, look, |x, y| dim(rgb[y * width + x], scan_gain(y, height))),
        Screen::Phosphor => {
            let radius = (width / 320).max(1);
            background::compose(rect, look, |x, y| {
                let row = &rgb[y * width..(y + 1) * width];
                let near = x.checked_sub(radius).map_or(0, |i| row[i]);
                let far = x.checked_sub(radius * 2).map_or(0, |i| row[i]);
                let channels = [16, 8, 0].map(|shift| {
                    (((row[x] >> shift) & 255) + (((near >> shift) & 255) >> 2) + (((far >> shift) & 255) >> 3)).min(255)
                });
                channels[0] << 16 | channels[1] << 8 | channels[2]
            })
        }
        Screen::Crt => {
            if !cache.prepare(width, height) { return Vec::new(); }
            let rx = (width / 640).max(1);
            let row_step = (height / 400).max(1) * width;
            let last_row = (height - 1) * width;
            background::compose(rect, look, |x, y| {
                let CrtSample { column, row } = cache.samples[y * width + x];
                if column == u32::MAX { return 0; }
                let gain = if column & (1 << 31) != 0 { 192 } else { 256 };
                let sx = (column & 0x7fff_ffff) as usize;
                let row = row as usize;
                let pixel = rgb[row + sx];
                let left = rgb[row + sx.saturating_sub(rx)];
                let right = rgb[row + (sx + rx).min(width - 1)];
                let up = rgb[row.saturating_sub(row_step) + sx];
                let down = rgb[(row + row_step).min(last_row) + sx];
                dim(glow(pixel, left, right, up, down), gain)
            })
        }
    }
}

#[inline]
fn glow(pixel: u32, left: u32, right: u32, up: u32, down: u32) -> u32 {
    let halo_rb = ((left & 0xff00ff) + (right & 0xff00ff) + (up & 0xff00ff) + (down & 0xff00ff)) >> 5;
    let rb = (pixel & 0xff00ff) + (halo_rb & 0xff00ff);
    let overflow = rb & 0x1000100;
    let rb = (rb | (overflow - (overflow >> 8))) & 0xff00ff;
    let halo_g = ((left & 0xff00) + (right & 0xff00) + (up & 0xff00) + (down & 0xff00)) >> 5;
    let g = ((pixel & 0xff00) + (halo_g & 0xff00)).min(0xff00);
    rb | g
}

#[inline]
fn scan_gain(y: usize, height: usize) -> u32 {
    let dark = if height < 400 { y % 2 == 1 } else { ((y * 2 + 1) * 200 / height) % 2 == 1 };
    if dark { 192 } else { 256 }
}

#[inline]
fn dim(color: u32, gain: u32) -> u32 {
    let rb = (((color & 0xff00ff) * gain + 0x800080) >> 8) & 0xff00ff;
    let g = (((color & 0x00ff00) * gain + 0x008000) >> 8) & 0x00ff00;
    rb | g
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_scanlines_and_spatial_bleed() {
        let rgb = [0xff8040; 12];
        assert_eq!(apply(&rgb, 3, 4, &Look::default()), rgb);
        let look = Look { screen: Screen::Scanlines, ..Look::default() };
        assert_eq!(apply(&rgb, 3, 4, &look), [0xff8040, 0xff8040, 0xff8040, 0xbf6030, 0xbf6030, 0xbf6030,
            0xff8040, 0xff8040, 0xff8040, 0xbf6030, 0xbf6030, 0xbf6030]);
        assert_eq!(scan_gain(0, 1), 256);
        assert_eq!((scan_gain(0, 400), scan_gain(1, 400), scan_gain(2, 400)), (256, 192, 256));
        assert_eq!(dim(0xffffff, 256), 0xffffff);
        let look = Look { screen: Screen::Phosphor, ..Look::default() };
        assert_eq!(apply(&[0xff8040, 0, 0, 0, 0, 0], 3, 2, &look), [0xff8040, 0x3f2010, 0x1f1008, 0, 0, 0]);
        assert_eq!(apply(&[0xffffff; 3], 3, 1, &look), [0xffffff; 3]);
    }
    #[test]
    fn crt_curves_edges_and_spreads_light() {
        let look = Look { screen: Screen::Crt, ..Look::default() };
        let out = apply(&vec![0xffffff; 101 * 101], 101, 101, &look);
        for corner in [0, 100, 101 * 100, 101 * 101 - 1] { assert_eq!(out[corner], 0); }
        assert_eq!(out[50 * 101 + 50], 0xffffff);
        assert!(out[49 * 101 + 50] < out[50 * 101 + 50]);
        let mut impulse = vec![0; 101 * 101];
        impulse[50 * 101 + 50] = 0xffffff;
        let out = apply(&impulse, 101, 101, &look);
        assert!(out[50 * 101 + 49] > 0 && out[49 * 101 + 50] > 0);
        assert_eq!(out[40 * 101 + 40], 0);
        assert_eq!(glow(0, 0xffffff, 0xffffff, 0xffffff, 0xffffff), 0x1f1f1f);
        assert_eq!(glow(0xffffff, 0xffffff, 0xffffff, 0xffffff, 0xffffff), 0xffffff);
        assert_eq!(glow(0xff00ff, 0x00ff00, 0x00ff00, 0x00ff00, 0x00ff00), 0xff1fff);
    }
    #[test]
    fn crt_parallel_rows_match_scalar_reference() {
        let (width, height) = (901, 603);
        let rgb: Vec<_> = (0..width * height).map(|i| (i as u32).wrapping_mul(0x12345) & 0xffffff).collect();
        let actual = apply(&rgb, width, height, &Look { screen: Screen::Crt, ..Look::default() });
        let (half_w, half_h) = (width as f32 * 0.5, height as f32 * 0.5);
        for y in 0..height { for x in 0..width {
            let cx = x as f32 + 0.5 - half_w;
            let cy = y as f32 + 0.5 - half_h;
            let xn = cx / half_w;
            let yn = cy / half_h;
            let sx = half_w + cx * (1.0 + 0.035 * yn * yn);
            let sy = half_h + cy * (1.0 + 0.035 * xn * xn);
            let mut expected = 0;
            if sx >= 0.0 && sy >= 0.0 && sx < width as f32 && sy < height as f32 {
                let (sx, sy) = (sx as usize, sy as usize);
                let center = rgb[sy * width + sx];
                let taps = [rgb[sy * width + sx.saturating_sub(1)], rgb[sy * width + (sx + 1).min(width - 1)],
                    rgb[sy.saturating_sub(1) * width + sx], rgb[(sy + 1).min(height - 1) * width + sx]];
                let gain = if ((sy * 2 + 1) * 200 / height) % 2 == 1 { 192 } else { 256 };
                for shift in [0, 8, 16] {
                    let halo: u32 = taps.iter().map(|p| (p >> shift) & 255).sum();
                    let c = (((center >> shift) & 255) + halo / 32).min(255);
                    expected |= ((c * gain + 128) / 256) << shift;
                }
            }
            assert_eq!(actual[y * width + x], expected, "pixel {x},{y}");
        }}
    }
    #[test]
    fn every_effect_handles_tiny_invalid_and_odd_frames() {
        for screen in [Screen::None, Screen::Scanlines, Screen::Crt, Screen::Phosphor] {
            let look = Look { screen, ..Look::default() };
            for (w, h) in [(0, 0), (0, usize::MAX), (usize::MAX, 0), (1, 1), (1, 17), (17, 1), (13, 9), (317, 197)] {
                let len = w * h;
                let rgb: Vec<_> = (0..len).map(|i| (i as u32).wrapping_mul(0x234567) & 0xffffff).collect();
                let copy = rgb.clone();
                let out = apply(&rgb, w, h, &look);
                assert_eq!(rgb, copy); assert_eq!(out.len(), len);
                assert!(out.iter().all(|&p| p <= 0xffffff));
                assert_eq!(apply(&vec![0; len], w, h, &look), vec![0; len]);
            }
            assert_eq!(apply(&[0xffffff], 2, 2, &look), [0; 4]);
            assert!(apply(&[], usize::MAX, 2, &look).is_empty());
            assert!(apply(&[], 8193, 8193, &look).is_empty());
        }
    }
    #[test]
    fn cached_geometry_is_reused_and_resize_refreshes_it() {
        let mut cache = CrtCache::default();
        let look = Look { screen: Screen::Crt, background: crate::look::Background::Custom,
            custom_background: [0x12, 0x34, 0x56], ..Look::default() };
        for (builds, w, h) in [(1, 101, 97), (1, 101, 97), (2, 97, 101), (3, 101, 97)] {
            let rect = PictureRect { x: 3, y: 5, width: w, height: h, out_width: w + 7, out_height: h + 11 };
            for color in [0x00ff00, 0xff00ff, 0] {
                let rgb = vec![color; w * h];
                let out = compose(&rgb, rect, &look, &mut cache);
                assert_eq!(cache.builds, builds);
                let picture = apply(&rgb, w, h, &look);
                let mut expected = background::paint(rect.out_width, rect.out_height, &look);
                for y in 0..h {
                    let start = (y + rect.y) * rect.out_width + rect.x;
                    expected[start..start + w].copy_from_slice(&picture[y * w..(y + 1) * w]);
                }
                assert_eq!(out, expected);
            }
            let rgb = vec![0x123456; w * h];
            for screen in [Screen::None, Screen::Scanlines, Screen::Phosphor] {
                compose(&rgb, rect, &Look { screen, ..look.clone() }, &mut cache);
                assert_eq!(cache.builds, builds);
            }
        }
        assert!(!cache.prepare(usize::MAX, 2));
        assert!(!cache.prepare(0, 1));
        assert_eq!(cache.builds, 3);
        assert!(cache.prepare(101, 97));
        assert_eq!(cache.builds, 3);
    }
}
