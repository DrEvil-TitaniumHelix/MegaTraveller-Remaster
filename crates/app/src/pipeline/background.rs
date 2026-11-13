use crate::look::{Background, Look};
use super::scale::PictureRect;
use std::mem::MaybeUninit;

pub fn paint(width: usize, height: usize, look: &Look) -> Vec<u32> {
    super::buffer(width, height, color(look))
}

fn color(look: &Look) -> u32 {
    match look.background {
        Background::Black => 0,
        Background::DarkGrey => 0x181818,
        Background::Custom => {
            let [r, g, b] = look.custom_background.map(u32::from);
            r << 16 | g << 8 | b
        }
    }
}

pub(super) fn compose(rect: PictureRect, look: &Look, sample: impl Fn(usize, usize) -> u32 + Sync) -> Vec<u32> {
    let Some(len) = rect.out_width.checked_mul(rect.out_height).filter(|&n| n <= 64 * 1024 * 1024) else { return Vec::new(); };
    if len == 0 { return Vec::new(); }
    if rect.width == 0 || rect.height == 0
        || rect.x.checked_add(rect.width).is_none_or(|n| n > rect.out_width)
        || rect.y.checked_add(rect.height).is_none_or(|n| n > rect.out_height) {
        return paint(rect.out_width, rect.out_height, look);
    }
    let mut out = Vec::<u32>::new();
    if out.try_reserve_exact(len).is_err() { return out; }
    let pixels = &mut out.spare_capacity_mut()[..len];
    let color = color(look);
    let workers = if len >= 512 * 1024 {
        std::thread::available_parallelism().map_or(1, |n| n.get().min(4)).min(rect.out_height)
    } else { 1 };
    if workers == 1 { write_rows(pixels, 0, rect, color, &sample); }
    else {
        let rows_per_worker = rect.out_height.div_ceil(workers);
        let failed = std::thread::scope(|scope| {
            let mut failed = false;
            for (i, rows) in pixels.chunks_mut(rect.out_width * rows_per_worker).enumerate() {
                let sample = &sample;
                if std::thread::Builder::new().spawn_scoped(scope, move || write_rows(rows, i * rows_per_worker, rect, color, sample)).is_err() {
                    failed = true;
                }
            }
            failed
        });
        if failed { write_rows(pixels, 0, rect, color, &sample); }
    }
    unsafe { out.set_len(len); }
    out
}

fn write_rows(pixels: &mut [MaybeUninit<u32>], first_y: usize, rect: PictureRect, color: u32,
    sample: &(impl Fn(usize, usize) -> u32 + Sync)) {
    for (y, row) in pixels.chunks_exact_mut(rect.out_width).enumerate() {
        let y = y + first_y;
        if y < rect.y || y >= rect.y + rect.height { row.fill(MaybeUninit::new(color)); continue; }
        let (left, rest) = row.split_at_mut(rect.x);
        let (picture, right) = rest.split_at_mut(rect.width);
        left.fill(MaybeUninit::new(color));
        right.fill(MaybeUninit::new(color));
        for (x, dst) in picture.iter_mut().enumerate() { dst.write(sample(x, y - rect.y)); }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn solid_backgrounds_and_empty_surfaces() {
        for (background, color) in [(Background::Black, 0), (Background::DarkGrey, 0x181818), (Background::Custom, 0x123456)] {
            let look = Look { background, custom_background: [0x12, 0x34, 0x56], ..Look::default() };
            for (w, h) in [(1, 1), (1, 7), (11, 1), (9, 7)] { assert_eq!(paint(w, h, &look), vec![color; w * h]); }
            for (w, h) in [(0, 0), (0, usize::MAX), (usize::MAX, 0), (usize::MAX, 2), (8193, 8193)] {
                assert!(paint(w, h, &look).is_empty());
            }
        }
    }
    #[test]
    fn composed_surface_initializes_every_border_and_picture_pixel() {
        let look = Look { background: Background::Custom, custom_background: [0x12, 0x34, 0x56], ..Look::default() };
        for (w, h, x, y, pw, ph) in [(1, 1, 0, 0, 1, 1), (13, 17, 3, 4, 5, 7), (901, 607, 7, 11, 883, 583)] {
            let rect = PictureRect { x, y, width: pw, height: ph, out_width: w, out_height: h };
            let out = compose(rect, &look, |x, y| 0x400000 + (y * pw + x) as u32);
            let expected: Vec<_> = (0..w * h).map(|i| {
                let (px, py) = (i % w, i / w);
                if px >= x && px < x + pw && py >= y && py < y + ph { 0x400000 + ((py - y) * pw + px - x) as u32 }
                else { 0x123456 }
            }).collect();
            assert_eq!(out, expected);
        }
        for (w, h) in [(0, usize::MAX), (usize::MAX, 0), (usize::MAX, 2), (8193, 8193)] {
            let rect = PictureRect { out_width: w, out_height: h, width: 1, height: 1, ..PictureRect::default() };
            assert!(compose(rect, &look, |_, _| panic!("empty surface must not sample")).is_empty());
        }
        let bad = PictureRect { x: usize::MAX, width: 2, height: 3, out_width: 5, out_height: 7, ..PictureRect::default() };
        assert_eq!(compose(bad, &look, |_, _| panic!("invalid rectangle must not sample")), vec![0x123456; 35]);
        let empty = PictureRect { width: 0, height: 3, out_width: 5, out_height: 7, ..PictureRect::default() };
        assert_eq!(compose(empty, &look, |_, _| panic!("empty rectangle must not sample")), vec![0x123456; 35]);
    }
    #[test]
    fn sampler_panic_never_publishes_uninitialized_pixels() {
        for (w, h) in [(3, 5), (901, 607)] {
            let rect = PictureRect { width: w, height: h, out_width: w, out_height: h, ..PictureRect::default() };
            assert!(std::panic::catch_unwind(|| compose(rect, &Look::default(), |x, y| {
                assert!(x != 2 || y != 3, "synthetic sampler failure");
                0xffffff
            })).is_err());
        }
    }
}
