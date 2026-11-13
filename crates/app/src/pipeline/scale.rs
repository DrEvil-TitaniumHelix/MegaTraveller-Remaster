use crate::look::{Filter, Look, ScaleMode, Shape};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PictureRect {
    pub x: usize, pub y: usize, pub width: usize, pub height: usize,
    pub out_width: usize, pub out_height: usize,
}
pub fn picture_rect(out_width: usize, out_height: usize, look: &Look) -> PictureRect {
    let mut rect = PictureRect { out_width, out_height, ..PictureRect::default() };
    if out_width == 0 || out_height == 0 || super::pixel_count(out_width, out_height).is_none() { return rect; }
    let base_h = if look.shape == Shape::Classic { 240 } else { 200 };
    let n = (out_width / 320).min(out_height / base_h);
    let (w, h) = if look.scale == ScaleMode::Integer && n != 0 { (320 * n, base_h * n) }
        else if out_width * base_h <= out_height * 320 { (out_width, (out_width * base_h / 320).max(1)) }
        else { ((out_height * 320 / base_h).max(1), out_height) };
    rect.x = (out_width - w) / 2; rect.y = (out_height - h) / 2; rect.width = w; rect.height = h;
    rect
}
pub fn scale(rgb: &[u32], width: usize, height: usize, look: &Look) -> Vec<u32> {
    scale_surface(rgb, 320, 200, width, height, look)
}
pub fn scale_surface(rgb: &[u32], source_w: usize, source_h: usize, width: usize, height: usize, look: &Look) -> Vec<u32> {
    let Some(len) = width.checked_mul(height).filter(|&n| n <= 64 * 1024 * 1024) else { return Vec::new(); };
    if len == 0 { return Vec::new(); }
    if source_w == 0 || source_h == 0 || source_w.checked_mul(source_h) != Some(rgb.len()) { return super::buffer(width, height, 0); }
    let mut out = Vec::new();
    if out.try_reserve_exact(len).is_err() { return out; }
    if look.filter == Filter::Nearest {
        let mut row = super::buffer(width, 1, 0);
        if row.is_empty() { return out; }
        let mut cached = usize::MAX;
        for y in 0..height {
            let source_y = y * source_h / height;
            if source_y != cached {
                for (x, dst) in row.iter_mut().enumerate() { *dst = rgb[source_y * source_w + x * source_w / width]; }
                cached = source_y;
            }
            out.extend_from_slice(&row);
        }
        return out;
    }
    let nx = if look.filter == Filter::Sharp { (width / source_w).max(1) } else { 1 };
    let ny = if look.filter == Filter::Sharp { (height / source_h).max(1) } else { 1 };
    let mut xs = Vec::new();
    if xs.try_reserve_exact(width).is_err() { return Vec::new(); }
    xs.extend((0..width).map(|x| axis(x, width, source_w, nx)));
    let mut top = super::buffer(width, 1, 0);
    let mut bottom = super::buffer(width, 1, 0);
    if top.is_empty() || bottom.is_empty() { return Vec::new(); }
    let mut cached = (usize::MAX, usize::MAX);
    for y in 0..height {
        let (a, b, t) = axis(y, height, source_h, ny);
        if cached != (a, b) {
            for ((top, bottom), &(l, r, t)) in top.iter_mut().zip(&mut bottom).zip(&xs) {
                *top = mix(rgb[a * source_w + l], rgb[a * source_w + r], t);
                *bottom = mix(rgb[b * source_w + l], rgb[b * source_w + r], t);
            }
            cached = (a, b);
        }
        if a == b || t == 0 { out.extend_from_slice(&top); }
        else { out.extend(top.iter().zip(&bottom).map(|(&a, &b)| mix(a, b, t))); }
    }
    out
}

fn axis(dst: usize, len: usize, source: usize, integer: usize) -> (usize, usize, u32) {
    let expanded = source * integer;
    let position = ((dst as f64 + 0.5) * expanded as f64 / len as f64 - 0.5).max(0.0);
    let left = position.floor() as usize;
    let right = (left + 1).min(expanded - 1);
    (left.min(expanded - 1) / integer, right / integer, ((position - left as f64) * 256.0).round() as u32)
}

#[inline]
fn mix(a: u32, b: u32, t: u32) -> u32 {
    let rb = ((a & 0xff00ff) * (256 - t) + (b & 0xff00ff) * t + 0x800080) >> 8;
    let g = ((a & 0x00ff00) * (256 - t) + (b & 0x00ff00) * t + 0x008000) >> 8;
    (rb & 0xff00ff) | (g & 0x00ff00)
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::look::{Background, Palette, Screen};
    #[test]
    fn rectangles_and_nearest_rows() {
        let mut look = Look::default();
        let r = picture_rect(1000, 800, &look); assert_eq!((r.x, r.y, r.width, r.height), (20, 40, 960, 720));
        look.shape = Shape::Square;
        let r = picture_rect(640, 480, &look); assert_eq!((r.x, r.y, r.width, r.height), (0, 40, 640, 400));
        look.scale = ScaleMode::Fill;
        for (w, h) in [(1,1), (1,999), (999,1), (317, 197), (1920,1080)] {
            let r = picture_rect(w, h, &look); assert!(r.width > 0 && r.height > 0 && r.x + r.width <= w && r.y + r.height <= h);
        }
        let rgb: Vec<_> = (0..64000).map(|i| (i / 320) as u32).collect();
        for n in 1..=6 {
            let out = scale(&rgb, 320 * n, 240 * n, &look);
            for y in 0..240 * n { assert_eq!(out[y * 320 * n], (y * 5 / (6 * n)) as u32); }
        }
    }

    #[test]
    fn pixel_centres_and_independent_channels() {
        assert_eq!(axis(0, 640, 320, 1), (0, 1, 0));
        assert_eq!(axis(1, 640, 320, 1), (0, 1, 64));
        assert_eq!(axis(2, 640, 320, 1), (0, 1, 192));
        assert_eq!(axis(639, 640, 320, 1), (319, 319, 64));
        assert_eq!(axis(1, 640, 320, 2), (0, 1, 0));
        assert_eq!(mix(0xff0000, 0x0000ff, 128), 0x800080);
        assert_eq!(mix(0, 0xffffff, 64), 0x404040);
        for color in [0, 0x123456, 0xffffff] {
            assert_eq!(mix(color, 0xffffff, 0), color);
            assert_eq!(mix(0, color, 256), color);
            assert_eq!(mix(color, color, 137), color);
        }
    }

    #[test]
    fn soft_interpolates_both_axes_and_sharp_keeps_integer_blocks() {
        let rgb: Vec<_> = (0..64000).map(|i| if (i % 320 + i / 320) % 2 == 0 { 0 } else { 0xffffff }).collect();
        let soft = Look { filter: Filter::Soft, ..Look::default() };
        let out = scale(&rgb, 640, 400, &soft);
        assert_eq!(&out[..4], &[0, 0x404040, 0xbfbfbf, 0xbfbfbf]);
        assert_eq!(out[640], 0x404040);
        assert_eq!(out[641], 0x606060);
        let sharp = Look { filter: Filter::Sharp, ..Look::default() };
        assert_eq!(scale(&rgb, 960, 800, &sharp), scale(&rgb, 960, 800, &Look::default()));
        let stripes: Vec<_> = (0..64000).map(|i| if i % 320 % 2 == 0 { 0 } else { 0xffffff }).collect();
        let sharp_out = scale(&stripes, 800, 200, &sharp);
        assert_eq!(&sharp_out[..5], &[0, 0, 0x808080, 0xffffff, 0xe5e5e5]);
        assert_eq!(&scale(&stripes, 800, 200, &soft)[..5], &[0, 0x1a1a1a, 0x808080, 0xe5e5e5, 0xb2b2b2]);
        assert_eq!(scale(&stripes, 160, 100, &soft), vec![0x808080; 16000]);
        assert_eq!(scale(&stripes, 160, 100, &sharp), vec![0x808080; 16000]);
    }

    #[test]
    fn filters_preserve_constants_and_handle_resize_edges() {
        for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] {
            let look = Look { filter, ..Look::default() };
            let rgb = vec![0xabcdef; 64000];
            for (w, h) in [(0, 0), (0, usize::MAX), (usize::MAX, 0), (1, 1), (1, 31), (31, 1), (317, 197), (677, 413)] {
                assert_eq!(scale(&rgb, w, h, &look), vec![0xabcdef; w * h]);
            }
            for (w, h) in [(usize::MAX, 2), (8193, 8193)] { assert!(scale(&rgb, w, h, &look).is_empty()); }
            assert_eq!(scale(&[], 3, 2, &look), [0; 6]);
            let pattern: Vec<_> = (0..64000).map(|i| i as u32 * 257 & 0xffffff).collect();
            assert_eq!(scale(&pattern, 320, 200, &look), pattern);
        }
    }

    #[test]
    fn all_knobs_combine_without_affecting_borders_or_source() {
        let frame = crate::pipeline::Frame { indexed: std::array::from_fn(|i| (i % 16) as u8), ..Default::default() };
        for shape in [Shape::Classic, Shape::Square] { for scale in [ScaleMode::Integer, ScaleMode::Fill] {
            for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] { for screen in [Screen::None, Screen::Scanlines, Screen::Crt, Screen::Phosphor] {
                for palette in [Palette::Ega, Palette::Warm, Palette::Amber, Palette::Green, Palette::Custom] {
                    for (background, border) in [(Background::Black, 0), (Background::DarkGrey, 0x181818), (Background::Custom, 0x123456)] {
                        let look = Look { shape, scale, filter, screen, palette, background, custom_background: [0x12, 0x34, 0x56], ..Look::default() };
                        let out = crate::pipeline::render(&frame, &look, 21, 19);
                        assert_eq!(out.len(), 21 * 19);
                        assert_eq!(&out[..21], &[border; 21]);
                        assert!(out.iter().all(|&p| p <= 0xffffff));
                    }
                }
            }}
        }}
        assert_eq!(frame.indexed, std::array::from_fn(|i| (i % 16) as u8));
    }

    #[test]
    #[ignore = "manual before/after byte comparison of both full-size slowest paths"]
    fn snapshot_slowest_path() {
        use std::{fs, io::{BufWriter, Write}};
        let label = std::env::var("MT_FILTER_SNAPSHOT").expect("set MT_FILTER_SNAPSHOT=before or after");
        assert!(label == "before" || label == "after");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/filter-snapshot").join(&label);
        fs::create_dir_all(&dir).unwrap();
        let frame = crate::pipeline::Frame { indexed: std::array::from_fn(|i| ((i % 320 / 3 + i / 320 / 2) % 16) as u8), ..Default::default() };
        for shape in [Shape::Classic, Shape::Square] {
            let look = Look { shape, scale: ScaleMode::Fill, filter: Filter::Sharp, screen: Screen::Crt, palette: Palette::Amber, ..Look::default() };
            let out = crate::pipeline::render(&frame, &look, 2560, 1440);
            assert_eq!(out.len(), 2560 * 1440);
            if label == "after" {
                let before = fs::read(dir.parent().unwrap().join("before").join(format!("{shape:?}.rgb"))).unwrap();
                assert_eq!(before.len(), out.len() * 4);
                for (i, (&pixel, bytes)) in out.iter().zip(before.chunks_exact(4)).enumerate() {
                    assert_eq!(pixel.to_le_bytes(), bytes, "{shape:?} pixel {i}");
                }
                println!("{shape:?}: before/after byte comparison PASS ({} bytes)", before.len());
            }
            let path = dir.join(format!("{shape:?}.rgb"));
            let mut file = BufWriter::new(fs::File::create_new(&path).unwrap());
            for pixel in out { file.write_all(&pixel.to_le_bytes()).unwrap(); }
            file.flush().unwrap();
            println!("snapshot: {}", path.display());
        }
    }

    #[test]
    #[ignore = "manual release timing, including allocations and final border composition"]
    fn release_budget() {
        use crate::pipeline::{self, palette, screen};
        use std::{hint::black_box, time::Instant};
        assert!(!cfg!(debug_assertions), "run with --release");
        let frame = pipeline::Frame { indexed: std::array::from_fn(|i| ((i % 320 / 3 + i / 320 / 2) % 16) as u8), ..Default::default() };
        for shape in [Shape::Classic, Shape::Square] {
            let look = Look { shape, scale: ScaleMode::Fill, filter: Filter::Sharp, screen: Screen::Crt, palette: Palette::Amber, ..Look::default() };
            pipeline::CRT_CACHE.with(|cache| *cache.borrow_mut() = screen::CrtCache::default());
            let start = Instant::now();
            black_box(pipeline::render(black_box(&frame), black_box(&look), 2560, 1440));
            let cold = start.elapsed().as_secs_f64() * 1000.0;
            for _ in 0..5 { black_box(pipeline::render(black_box(&frame), black_box(&look), 2560, 1440)); }
            let mut timings = Vec::new();
            for _ in 0..40 {
                let start = Instant::now();
                black_box(pipeline::render(black_box(&frame), black_box(&look), 2560, 1440));
                timings.push(start.elapsed().as_secs_f64() * 1000.0);
            }
            timings.sort_by(f64::total_cmp);
            let rect = picture_rect(2560, 1440, &look);
            println!("2560x1440 {shape:?} fill+sharp+CRT+amber: min={:.3} median={:.3} p95={:.3} max={:.3} ms (40 frames; picture {}x{})",
                timings[0], timings[20], timings[37], timings[39], rect.width, rect.height);
            let mut stages = [0.0; 3];
            let mut cache = screen::CrtCache::default();
            let start = Instant::now();
            assert!(cache.prepare(rect.width, rect.height));
            let rebuild = start.elapsed().as_secs_f64() * 1000.0;
            for _ in 0..40 {
                let start = Instant::now();
                let colors = palette::map(black_box(&frame.indexed), &frame.regs, &look);
                stages[0] += start.elapsed().as_secs_f64();
                let start = Instant::now();
                let scaled = scale(black_box(&colors), rect.width, rect.height, &look);
                stages[1] += start.elapsed().as_secs_f64();
                let start = Instant::now();
                let out = screen::compose(black_box(&scaled), rect, &look, &mut cache);
                stages[2] += start.elapsed().as_secs_f64();
                black_box(out);
            }
            println!("stage means ms: palette={:.3} scale={:.3} fused_CRT_borders_output={:.3}; separate background/composite passes=0",
                stages[0] * 25.0, stages[1] * 25.0, stages[2] * 25.0);
            println!("resize-only costs ms: first full render={cold:.3} standalone cache rebuild={rebuild:.3}");
            assert!(timings[37] < 16.0, "{shape:?} cached redraw p95 exceeded 16 ms");
        }
    }
}
