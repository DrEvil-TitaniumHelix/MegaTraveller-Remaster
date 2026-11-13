use std::{cell::RefCell, collections::HashMap, sync::Arc};
use crate::{font_menu::{self, Face}, look::{FontChoice, Look}};
use super::scale::PictureRect;
pub use mt_engine::present::textlog::TextLog;
use mt_engine::present::textlog::Run;
pub(crate) const DIMMED: u8 = 0x80;
pub(crate) const UNDERLINE: u8 = 0x20;
pub(crate) const KEYCAP: u8 = 0x40;

pub fn shown(code: u8) -> Option<char> {
    Some(match code {
        0x20..=0x5A | 0x61..=0x7A => code as char, 0xAF => '/',
        0xB0..=0xB9 => (code - 0x80) as char, 0xD0..=0xD9 => (code - 0xA0) as char, _ => return None,
    })
}
fn character(run: &Run, face: &Face) -> Option<char> {
    let c = shown(*run.bytes.first()?)?;
    face.font.has_glyph(c).then_some(c)
}
pub fn badge(run: &Run, log: &TextLog) -> bool {
    run.bytes.first().is_some_and(|c| (0xB0..=0xB9).contains(c)) && log.visible.iter().any(|i| i.page == run.page
        && i.bytes.first().is_some_and(|&c| c < 0x20) && (i.x - 4..=i.x + i.width as i64 + 4).contains(&run.x) && (i.y - 2..=i.y + 10).contains(&run.y))
}
fn valid(run: &Run, size: (usize, usize)) -> bool {
    run.width > 0 && (run.width as usize..=256).contains(&run.erase_width()) && (1..=15).contains(&run.erase_height())
        && run.x > -(run.erase_width() as i64) && run.x < size.0 as i64 && run.y > -(run.erase_height() as i64) && run.y < size.1 as i64
        && run.background.len() == run.erase_width() * run.erase_height()
}
fn cell_height(run: &Run) -> i64 {
    let height = if run.bytes.first().is_some_and(|c| (0xD0..=0xD9).contains(c)) { 12 } else { 8 };
    run.erase_height().saturating_sub(usize::from(run.descend) * 2).min(height) as i64
}
fn invisible(run: &Run) -> bool {
    run.bytes.first().is_some_and(|&b| b != b' ') && !run.bitmap.is_empty() && run.bitmap == run.background
}
fn bitmap_text(run: &Run) -> bool {
    run.bytes.first().is_some_and(|&c| shown(c).is_some()) && run.bitmap.len() == run.background.len()
}
pub fn contrast_ink(background: u32) -> u32 {
    readable_ink(background, false)
}
pub fn readable_ink(background: u32, dimmed: bool) -> u32 {
    let channel = |shift: u32| {
        let c = ((background >> shift) & 255) as f64 / 255.;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let l = 0.2126 * channel(16) + 0.7152 * channel(8) + 0.0722 * channel(0);
    let dark = (l + 0.05) / 0.05 >= 1.05 / (l + 0.05);
    if !dimmed { return if dark { 0 } else { 0xffffff }; }
    let level = if dark { (l + 0.05) / 3.2 - 0.05 } else { 3.2 * (l + 0.05) - 0.05 };
    let srgb = if level <= 0.0031308 { level * 12.92 } else { 1.055 * level.powf(1. / 2.4) - 0.055 };
    let c = (srgb * 255.).round().clamp(0., 255.) as u32;
    c * 0x010101
}
pub fn background(indexed: &[u8; 64000], log: &TextLog, look: &Look) -> [u8; 64000] {
    let mut out = *indexed;
    restore_background(&mut out, (320, 200), log, look); out
}
pub fn restore_background(out: &mut [u8], size: (usize, usize), log: &TextLog, look: &Look) {
    if size.0.checked_mul(size.1) != Some(out.len()) || log.overflowed { return; }
    let face = font_menu::selected(look.text_font());
    for run in log.visible.iter().rev().filter(|r| r.page == log.page && valid(r, size)) {
        let cut = run.bytes.is_empty() && face.is_some();
        if badge(run, log) { continue; }
        if invisible(run) { continue; }
        if !cut && !face.as_ref().is_some_and(|f| character(run, f).is_some()) && !(look.high_contrast_text() && bitmap_text(run)) { continue; }
        for dy in 0..run.erase_height() { for dx in 0..run.erase_width() {
            let (x, y) = (run.x + dx as i64, run.y + dy as i64);
            if (0..size.0 as i64).contains(&x) && (0..size.1 as i64).contains(&y) {
                let i = dy * run.erase_width() + dx; let at = y as usize * size.0 + x as usize;
                if run.covered && !(run.bitmap.get(i).is_some_and(|b| *b != run.background[i] && out[at] == *b)) { continue; }
                out[at] = run.background[i];
            }
        } }
    }
}
type MaskKey = (usize, char, usize, usize, bool, bool);
struct Mask { alpha: Vec<u8>, spans: Vec<(usize, usize, usize)>, pad: usize }
#[derive(Default)]
struct Masks { entries: HashMap<MaskKey, Arc<Mask>>, bytes: usize }
thread_local! { static MASKS: RefCell<Masks> = RefCell::new(Masks::default()); }

fn mask(face: &Face, c: char, width: usize, height: usize, bold: bool, descend: bool) -> Arc<Mask> {
    let key = (face.font.file_hash(), c, width, height, bold && !face.bold, descend);
    MASKS.with(|cache| {
        let mut cache = cache.borrow_mut();
        if let Some(mask) = cache.entries.get(&key) { return mask.clone(); }
        let pad = width / 4; let stride = width + 2 * pad;
        let mut out = vec![0u8; stride * height];
        let cap = face.font.metrics('H', 32.0).height.max(1) as f32;
        let mut px = (height as f32 * 0.75 * 32.0 / cap).clamp(1.0, 512.0);
        let bounds = face.font.metrics(c, px);
        let extent = bounds.width.max(bounds.height);
        if extent > 1024 { px *= 1024.0 / extent as f32; }
        let bounds = face.font.metrics(c, px);
        if bounds.width > width && width > 0 { px *= width as f32 / bounds.width as f32; }
        let (metrics, alpha) = font_menu::font_operation(|| face.font.rasterize(c, px)).unwrap_or_default();
        if metrics.width > 0 && metrics.height > 0 {
            let baseline = height as f32 * if descend { 0.5 } else { 0.75 };
            let ascent = (metrics.ymin as f32 + metrics.height as f32).max(1.0);
            let vertical = (baseline / ascent).min(1.0);
            let y0 = (baseline - ascent * vertical).max(0.0).round() as usize;
            let y0 = y0.min(height.saturating_sub(1));
            let h = ((metrics.height as f32 * vertical).round() as usize).max(1).min(height - y0);
            let strength = if key.4 { (height / 24).max(1).min(width.saturating_sub(1)) } else { 0 };
            let w = metrics.width.min(width.saturating_sub(strength)).max(1);
            let ascent = (metrics.ymin + metrics.height as i32).clamp(0, metrics.height as i32) as usize;
            let xh = face.font.metrics('x', px).height.min(ascent);
            let body = (0..metrics.width).filter(|&x| (ascent - xh..ascent - xh / 2).any(|y| alpha[y * metrics.width + x] > 127));
            let shift = match (body.clone().min(), body.max()) {
                (Some(a), Some(b)) if c == 'j' && (metrics.width as f32 - (a + b + 1) as f32).abs() / 2.0 > metrics.width as f32 / 8.0 =>
                    (metrics.width as f32 - (a + b + 1) as f32) / 2.0 * w as f32 / metrics.width as f32,
                _ => 0.0,
            };
            let x0 = ((width.saturating_sub(w + strength) / 2) as f32 + shift).round().clamp(-(pad as f32), (width.saturating_sub(w + strength) + pad) as f32);
            let x0 = (x0 as i64 + pad as i64) as usize;
            for y in 0..h { for x in 0..w {
                let a = alpha[y * metrics.height / h * metrics.width + x * metrics.width / w];
                for dx in 0..=strength {
                    let at = (y0 + y) * stride + x0 + x + dx;
                    if let Some(dst) = out.get_mut(at) { *dst = (*dst).max(a); }
                }
            } }
        }
        let mut spans = Vec::new();
        for (y, row) in out.chunks_exact(stride).enumerate() {
            let mut x = 0;
            while x < stride {
                if row[x] == 0 { x += 1; continue; }
                let start = x;
                while x < stride && row[x] != 0 { x += 1; }
                spans.push((y, start, x));
            }
        }
        let bytes = out.len() + spans.len() * std::mem::size_of::<(usize, usize, usize)>();
        if cache.entries.len() >= 2048 || cache.bytes + bytes > 16 * 1024 * 1024 { *cache = Masks::default(); }
        let out = Arc::new(Mask { alpha: out, spans, pad });
        if bytes <= 16 * 1024 * 1024 { cache.bytes += bytes; cache.entries.insert(key, out.clone()); }
        out
    })
}
fn blend(dst: u32, src: u32, a: u8) -> u32 {
    let a = a as u32;
    [0, 8, 16].into_iter().map(|shift| {
        ((((dst >> shift) & 255) * (255 - a) + ((src >> shift) & 255) * a + 127) / 255) << shift
    }).sum()
}
fn edge(p: i64, extent: usize, base: i128) -> i64 { ((p as i128 * extent as i128 + base - 1).div_euclid(base)) as i64 }

pub fn overdraw(out: &mut [u32], rect: PictureRect, indexed: &[u8], log: &TextLog, look: &Look) {
    overdraw_surface(out, rect, (320, 200), indexed, log, look)
}
pub fn overdraw_surface(out: &mut [u32], rect: PictureRect, size: (usize, usize), indexed: &[u8], log: &TextLog, look: &Look) {
    if size.0 == 0 || size.1 == 0 { return; }
    if log.overflowed || (*look.text_font() == FontChoice::Original && !look.high_contrast_text()) || rect.width == 0 || rect.height == 0
        || rect.out_width.checked_mul(rect.out_height) != Some(out.len()) || out.len() > 64 * 1024 * 1024
        || rect.x.checked_add(rect.width).is_none_or(|x| x > rect.out_width)
        || rect.y.checked_add(rect.height).is_none_or(|y| y > rect.out_height) { return; }
    let face = font_menu::selected(look.text_font());
    let mut indices = [0; 64000];
    for (i, c) in indices[..16].iter_mut().enumerate() { *c = i as u8; }
    let colors = super::palette::map(&indices, &log.regs, look);
    let runs: Vec<&Run> = log.visible.iter().filter(|r| r.page == log.page && valid(r, size) && !r.bytes.is_empty() && !badge(r, log) && !invisible(r)).collect();
    let (sx, sy) = (rect.width as f32 / size.0 as f32, rect.height as f32 / size.1 as f32);
    let (shadow_dx, shadow_dy) = ((rect.width / size.0).max(1) as i64, (rect.height / size.1).max(1) as i64);
    for (line, laid) in layout(runs, size, indexed, (sx, sy), shadow_dy) {
        let (row, x_src0, x_src1, adv, h) = (laid.row, laid.x0, laid.x1, laid.adv, laid.h);
        if adv.checked_mul(h).is_none_or(|n| n > 4 * 1024 * 1024) { continue; }
        let (mut x0, y0) = (edge(x_src0, rect.width, size.0 as i128), edge(row, rect.height, size.1 as i128) + laid.dy);
        if line.len() == 1 { x0 -= ((adv as f32 - (x_src1 - x_src0) as f32 * sx) / 2.0).max(0.0).round() as i64; }
        else if ((x_src0 + x_src1) - size.0 as i64).abs() <= 4 { x0 -= ((line.len() * adv) as i64 - ((x_src1 - x_src0) as f32 * sx) as i64) / 2; }
        for (k, run) in line.iter().enumerate() {
            let Some((face, c)) = face.as_ref().and_then(|f| character(run, f).map(|c| (f, c))) else {
                if look.high_contrast_text() && bitmap_text(run) { overdraw_bitmap(out, rect, size, run); }
                continue;
            };
            let mask = mask(face, c, adv, h, look.bold, false);
            let gx = x0 + (k * adv) as i64;
            if run.colour & KEYCAP != 0 {
                let color = colors[(run.colour & 15) as usize]; let cap = |r: Option<&&Run>| r.is_some_and(|r| r.colour & KEYCAP != 0);
                let (left, right) = (!cap(line.get(k.wrapping_sub(1))), !cap(line.get(k + 1)));
                let mut dot = |x: i64, y: i64| { if x < 0 || y < 0 || x >= rect.width as i64 || y >= rect.height as i64 { return }
                    let dst = &mut out[(rect.y + y as usize) * rect.out_width + rect.x + x as usize];
                    *dst = if look.high_contrast_text() { readable_ink(*dst, run.colour & DIMMED != 0) } else { color }; };
                for x in gx..gx + adv as i64 { dot(x, y0 - 1); dot(x, y0 + h as i64); }
                for y in y0 - 1..=y0 + h as i64 { if left { dot(gx, y); } if right { dot(gx + adv as i64 - 1, y); } }
            }
            if run.colour & UNDERLINE != 0 {
                let thick = (h / 12).max(1) as i64; let color = colors[(run.colour & 15) as usize];
                for dy in 0..thick { let y = y0 + h as i64 - 1 - dy; if y < 0 || y >= rect.height as i64 { continue; }
                    for x in gx.max(0)..(gx + adv as i64).min(rect.width as i64) {
                        let dst = &mut out[(rect.y + y as usize) * rect.out_width + rect.x + x as usize];
                        *dst = if look.high_contrast_text() { readable_ink(*dst, run.colour & DIMMED != 0) } else { color };
                    } }
            }
            for shadow in [true, false] {
                if shadow && look.high_contrast_text() { continue; }
                let colour = if shadow { let Some(bg) = run.shadow else { continue; }; bg } else { run.colour };
                let (dx, dy) = if shadow { (shadow_dx, shadow_dy) } else { (0, 0) };
                let color = colors[(colour & 15) as usize];
                for &(my, left, right) in &mask.spans {
                    let y = y0 + my as i64 + dy; let gx = gx - mask.pad as i64;
                    if y < 0 || y >= rect.height as i64 { continue; }
                    let left = (gx + left as i64 + dx).max(0);
                    let right = (gx + right as i64 + dx).min(rect.width as i64);
                    if left >= right { continue; }
                    let offset = my * (adv + 2 * mask.pad) + (left - gx - dx) as usize;
                    let at = (rect.y + y as usize) * rect.out_width + rect.x + left as usize;
                    for (dst, &a) in out[at..at + (right - left) as usize].iter_mut().zip(&mask.alpha[offset..]) {
                        let color = if look.high_contrast_text() { readable_ink(*dst, run.colour & DIMMED != 0) } else { color };
                        *dst = if a == 255 { color } else { blend(*dst, color, a) };
                    }
                }
            }
        }
    }
}
pub struct Laid { pub row: i64, pub x0: i64, pub x1: i64, pub adv: usize, pub h: usize,
    pub dy: i64 }
pub fn layout<'a>(mut runs: Vec<&'a Run>, size: (usize, usize), indexed: &[u8], (sx, sy): (f32, f32), shadow_dy: i64) -> Vec<(Vec<&'a Run>, Laid)> {
    runs.sort_by_key(|r| (r.x, r.row()));
    let mut lines: Vec<Vec<&Run>> = Vec::new();
    for r in runs {
        match lines.iter_mut().find(|l| (l[0].row() - r.row()).abs() <= 2 && r.x == l[l.len() - 1].x + l[l.len() - 1].width as i64) {
            Some(l) => l.push(r), None => lines.push(vec![r]),
        }
    }
    let geom: Vec<(i64, i64, i64)> = lines.iter().map(|l| (l.iter().map(|r| r.y).min().unwrap_or(0), l[0].x, l[l.len() - 1].x + l[l.len() - 1].width as i64)).collect();
    let boxes: Vec<(i64, i64)> = lines.iter().zip(&geom).map(|(l, g)| text_box(l, size, indexed, g.1, g.2)).collect();
    let mut pitches: Vec<f32> = Vec::with_capacity(lines.len());
    let mut sizes: Vec<f32> = (0..lines.len()).map(|i| {
        let (row, x0, x1) = geom[i];
        let below = geom.iter().filter(|g| g.0 > row + 2 && g.2 > x0 && g.1 < x1).map(|g| (g.0 - row) as f32 * sy).fold(f32::INFINITY, f32::min);
        let (top, bottom) = boxes[i]; let bottom = (bottom - top.max(row)) as f32 * sy;
        let width = ((x1 - x0) as f32 * sx / lines[i].len() as f32).floor() * 14.0 / 8.0;
        pitches.push(below - shadow_dy as f32);
        (14.0 * sy).min(below - shadow_dy as f32).min(bottom - shadow_dy as f32).min(width)
    }).collect();
    let linked = |a: &(i64, i64, i64), b: &(i64, i64, i64)| ((a.0 - b.0).abs() <= 2 && a.1 <= b.2 + 32 && b.1 <= a.2 + 32) || ((a.0 - b.0).abs() <= 16 && ((a.1 - b.1).abs() <= 2 || (a.2 > b.1 && b.2 > a.1)));
    let mut fits: Vec<f32> = lines.iter().map(|l| cell_height(l[0]) as f32 * sy).collect();
    let tall: Vec<bool> = lines.iter().map(|l| l.iter().any(|r| r.bytes.first().is_some_and(|c| (0xD0..=0xD9).contains(c)))).collect();
    let same = |i: usize, j: usize| tall[i] == tall[j] && linked(&geom[i], &geom[j]);
    for _ in 0..64 {
        let (prev, prev_fit, prev_pitch) = (sizes.clone(), fits.clone(), pitches.clone());
        for i in 0..geom.len() { for j in 0..geom.len() { if i != j && same(i, j) { sizes[i] = sizes[i].min(prev[j]); fits[i] = fits[i].max(prev_fit[j]); pitches[i] = pitches[i].min(prev_pitch[j]); } } }
        if sizes == prev && fits == prev_fit && pitches == prev_pitch { break; }
    }
    for (fit, pitch) in fits.iter_mut().zip(&pitches) { *fit = fit.min(pitch.max(6.0 * sy - shadow_dy as f32)); }
    for ((fit, &(top, bottom)), g) in fits.iter_mut().zip(&boxes).zip(&geom) { *fit = fit.min((bottom - top.max(g.0)) as f32 * sy - shadow_dy as f32); }
    for _ in 0..64 {
        let prev = fits.clone();
        for i in 0..geom.len() { for j in 0..geom.len() { if i != j && same(i, j) { fits[i] = fits[i].min(prev[j]); } } }
        if fits == prev { break; }
    }
    let mut laid: Vec<(Vec<&Run>, Laid, i64)> = lines.into_iter().zip(geom).zip(sizes.iter().zip(&fits)).zip(&boxes).map(|(((line, (row, x0, x1)), (&h, &fit)), &(top, bottom))| {
        let h = h.max(fit).round().max(1.0);
        let budget = if line.len() == 1 { f32::INFINITY } else { ((x1 - x0) as f32 * sx / line.len() as f32).floor() };
        let adv = (h * 8.0 / 14.0).round().min(budget).min(((size.0 as i64 - x0) as f32 * sx / line.len() as f32).floor()).max(1.0) as usize;
        let natural = match baseline(&line) { Some(base) => ((base - row) as f32 * sy - 0.75 * h).max(0.0), None => ((8.0 * sy - h) / 2.0).max(0.0) };
        let dy = match baseline(&line) {
            Some(_) => natural.min((bottom - row) as f32 * sy - shadow_dy as f32 - h).max((top - row) as f32 * sy),
            None => natural,
        }.round() as i64;
        (line, Laid { row, x0, x1, adv, h: h as usize, dy }, natural.round() as i64)
    }).collect();
    let mut order: Vec<usize> = (0..laid.len()).collect(); order.sort_by_key(|&i| laid[i].1.row);
    for &i in &order {
        let (row, x0, x1, dy, natural) = (laid[i].1.row, laid[i].1.x0, laid[i].1.x1, laid[i].1.dy, laid[i].2);
        if dy >= natural { continue }
        let floor = laid.iter().filter(|(_, g, _)| g.row < row - 2 && g.x1 > x0 && g.x0 < x1)
            .map(|(_, g, _)| (g.row as f32 * sy).round() as i64 + g.dy + g.h as i64 + shadow_dy - (row as f32 * sy).round() as i64).max();
        if let Some(floor) = floor { laid[i].1.dy = dy.max(floor.min(natural)); }
    }
    laid.into_iter().map(|(l, g, _)| (l, g)).collect()
}
pub(crate) fn ink(r: &Run) -> Option<(i64, i64)> {
    let w = r.erase_width(); let inked = |y: &usize| (0..w).any(|x| r.bitmap.get(y * w + x) != r.background.get(y * w + x));
    let (a, b) = ((0..r.erase_height()).find(inked)?, (0..r.erase_height()).rfind(inked)?); Some((r.y + a as i64, r.y + b as i64))
}
fn baseline(line: &[&Run]) -> Option<i64> {
    let alnum = |r: &&&Run| r.bytes.first().and_then(|&b| shown(b)).is_some_and(|c| c.is_ascii_alphanumeric());
    line.iter().filter(alnum).filter_map(|r| ink(r)).map(|i| i.1 + 1).min().or_else(|| line.iter().filter_map(|r| ink(r)).map(|i| i.1 + 1).max())
}
fn text_box(line: &[&Run], size: (usize, usize), indexed: &[u8], x0: i64, x1: i64) -> (i64, i64) {
    let band = line.iter().filter_map(|r| ink(r)).fold(None, |a: Option<(i64, i64)>, i| Some(a.map_or(i, |a| (a.0.min(i.0), a.1.max(i.1)))));
    let (rows, cells) = (line.iter().map(|r| r.y).min().unwrap_or(0), line.iter().map(|r| r.y + r.erase_height() as i64).max().unwrap_or(0));
    if indexed.len() != size.0 * size.1 { return (0, size.1 as i64); }
    let (top, bottom) = band.unwrap_or((rows, cells - 1));
    let mut fill = [false; 256];
    for r in line { let w = r.erase_width(); for (i, &c) in r.background.iter().enumerate() { if (top..=bottom).contains(&(r.y + (i / w.max(1)) as i64)) { fill[c as usize] = true; } } }
    for r in line.iter().filter(|r| r.covered) { let w = r.erase_width().max(1); for (i, &c) in r.background.iter().enumerate() {
        let (x, y) = (r.x + (i % w) as i64, r.y + (i / w) as i64);
        if (0..size.0 as i64).contains(&x) && (0..size.1 as i64).contains(&y) { let v = indexed[y as usize * size.0 + x as usize]; if v != c { fill[v as usize] = true; } }
    } }
    let cols = x0.max(0)..x1.min(size.0 as i64);
    let open = |y: i64| (0..size.1 as i64).contains(&y) && cols.clone().filter(|&x| !fill[indexed[y as usize * size.0 + x as usize] as usize]).count() * 2 < cols.clone().count();
    let (mut a, mut b) = (top, bottom + 1);
    while open(a - 1) { a -= 1; } while open(b) { b += 1; }
    (a, b)
}
fn overdraw_bitmap(out: &mut [u32], rect: PictureRect, size: (usize, usize), run: &Run) {
    for dy in 0..run.erase_height() { for dx in 0..run.erase_width() {
        let i = dy * run.erase_width() + dx;
        if run.bitmap[i] == run.background[i] { continue; }
        if run.shadow.is_some_and(|s| run.bitmap[i] == s && s != run.colour & 15) { continue; }
        let (x, y) = (run.x + dx as i64, run.y + dy as i64);
        let (x0, x1) = (edge(x, rect.width, size.0 as i128).max(0), edge(x + 1, rect.width, size.0 as i128).min(rect.width as i64));
        let (y0, y1) = (edge(y, rect.height, size.1 as i128).max(0), edge(y + 1, rect.height, size.1 as i128).min(rect.height as i64));
        for y in y0..y1 { for x in x0..x1 {
            let dst = &mut out[(rect.y + y as usize) * rect.out_width + rect.x + x as usize];
            *dst = readable_ink(*dst, run.colour & DIMMED != 0);
        } }
    } }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{look::{Filter, Palette, ScaleMode, Screen, Shape}, pipeline::{self, Frame}};
    use mt_engine::{data::ly::Rec, present::{fb::Fb, palette, prim::Raster, text::{self, Font}, textlog}};
    fn scene() -> Frame {
        textlog::set_enabled(true);
        let mut fb = Fb::new();
        for y in 0..200 { for x in 0..320 { fb.pixel(Raster::Clip, 0, y, x, if (x + y) % 2 == 0 { 1 } else { 2 }, 0); } }
        let mut entries = vec![Vec::new(); 256];
        for c in [b'A', b'g', b'W'] { entries[c as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 4, x2: 1 }]; }
        let mut widths = [4; 256]; widths[b'W' as usize] = 2;
        let mut flags = [0; 256]; flags[b'g' as usize] = 1;
        let font = Font::new(entries, widths, &flags, 1);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"AgW ", 20, 10, 15, Some(4));
        text::draw(&mut fb, Raster::Clip, 0, &font, b"W", 197, 319, 15, Some(4));
        text::draw(&mut fb, Raster::Clip, 0, &font, b"A", -2, -1, 15, Some(4));
        let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        textlog::set_enabled(false);
        assert_eq!(frame.text.visible.len(), 6);
        frame
    }
    fn relaid_band(r: &Run, rect: PictureRect, x: i64, y: i64) -> bool {
        x + 1 >= edge(r.x - 1, rect.width, 320) && x < edge(r.x + 8 * r.width as i64 + 2, rect.width, 320) + 1
            && y + 1 >= edge(r.row() - 2, rect.height, 200) && y < edge(r.row() + 15, rect.height, 200) + 1
    }
    fn run(x: i64, y: i64, w: u8, c: u8, bg: u8) -> Run {
        Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: w, descend: false,
              background_width: w as u16, background_height: 8, background: vec![bg; w as usize * 8], bitmap: vec![bg; w as usize * 8], covered: false }
    }
    #[test]
    fn a_key_name_in_prose_is_boxed_like_a_key_cap() {
        let look = Look::default();
        let mut runs = word("Esc", 10, 50, 6, 1); for r in &mut runs { r.colour = 15; r.bitmap[7] = 15; }
        let rect = PictureRect { x: 0, y: 0, width: 640, height: 400, out_width: 640, out_height: 400 };
        let indexed = vec![1u8; 64000]; let mut plain = vec![0x00202020u32; 640 * 400]; let mut cap = plain.clone();
        overdraw(&mut plain, rect, &indexed, &TextLog { visible: runs.clone(), ..TextLog::default() }, &look);
        for r in &mut runs { r.colour |= KEYCAP; }
        overdraw(&mut cap, rect, &indexed, &TextLog { visible: runs, ..TextLog::default() }, &look);
        let diff: Vec<(usize, usize)> = (0..640 * 400).filter(|&i| plain[i] != cap[i]).map(|i| (i % 640, i / 640)).collect();
        let above = diff.iter().filter(|&&(_, y)| y == 99).count(); let between = diff.iter().filter(|&&(x, y)| x == 32 && (101..115).contains(&y)).count();
        assert!(above >= 30, "a line above the word: {above}"); assert!(diff.iter().any(|&(x, _)| x == 20) && diff.iter().any(|&(x, _)| x == 55), "the ends");
        assert_eq!(between, 0, "no bar between the letters");
    }
    #[test]
    fn a_hotkey_letter_is_underlined_under_its_laid_glyph() {
        let look = Look::default();
        let mut runs = word("Game", 10, 50, 6, 1); for r in &mut runs { r.colour = 15; r.bitmap[7] = 15; }
        let rect = PictureRect { x: 0, y: 0, width: 640, height: 400, out_width: 640, out_height: 400 };
        let indexed = vec![1u8; 64000]; let mut plain = vec![0x00202020u32; 640 * 400]; let mut hot = plain.clone();
        overdraw(&mut plain, rect, &indexed, &TextLog { visible: runs.clone(), ..TextLog::default() }, &look);
        runs[0].colour |= UNDERLINE;
        overdraw(&mut hot, rect, &indexed, &TextLog { visible: runs, ..TextLog::default() }, &look);
        let diff: Vec<usize> = (0..640 * 400).filter(|&i| plain[i] != hot[i]).collect();
        assert!(diff.len() >= 8, "the G gets a line: {} px", diff.len());
        assert!(diff.iter().all(|&i| (20..32).contains(&(i % 640))), "only under the G's cell");
        let rows: std::collections::BTreeSet<_> = diff.iter().map(|&i| i / 640).collect();
        assert!(rows.iter().all(|&y| y >= 100 + 12), "below the glyph body: {rows:?}");
    }
    #[test]
    fn ink_equal_to_the_fill_under_it_is_never_laid() {
        let look = Look::default(); assert!(look.high_contrast_text());
        let mut hidden = word("efate", 10, 2, 6, 6); for r in &mut hidden { r.colour = 6; }
        let mut shown = word("dock", 10, 2, 6, 6); for r in &mut shown { r.colour = 0; r.bitmap[0] = 0; }
        assert!(hidden.iter().all(invisible) && !shown.iter().any(invisible));
        let mut log = TextLog::default(); log.visible = hidden.iter().chain(&shown).cloned().collect();
        let rect = PictureRect { x: 0, y: 0, width: 640, height: 400, out_width: 640, out_height: 400 };
        let mut out = vec![0x00aa5500u32; 640 * 400]; let indexed = vec![6u8; 64000];
        overdraw(&mut out, rect, &indexed, &log, &look);
        let touched = out.iter().filter(|&&p| p != 0x00aa5500).count();
        assert!(touched > 0, "the shown word is laid");
        assert!((0..400).all(|y| (10 * 2 + 4 * 6 * 2 + 8..640).all(|x| out[y * 640 + x] == 0x00aa5500)), "nothing past the shown word: the hidden one was not laid");
    }
    #[test]
    fn covered_run_restores_only_its_surviving_ink() {
        let look = Look::default();
        let mut r = run(40, 100, 6, b'A', 1);
        for i in [0usize, 1, 6, 7] { r.bitmap[i] = 15; }
        let mut out = vec![1u8; 320 * 200];
        let at = |dx: usize, dy: usize| (100 + dy) * 320 + 40 + dx;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1)] { out[at(dx, dy)] = 15; }
        out[at(1, 1)] = 9; out[at(5, 7)] = 9; out[at(5, 0)] = 9;
        let mut plain = out.clone();
        let log = TextLog { visible: vec![r.clone()], ..TextLog::default() };
        restore_background(&mut plain, (320, 200), &log, &look);
        assert_eq!((plain[at(0, 0)], plain[at(1, 1)], plain[at(5, 7)], plain[at(5, 0)]), (1, 1, 1, 1), "a plain run gives the whole cell back");
        r.covered = true;
        let log = TextLog { visible: vec![r], ..TextLog::default() };
        restore_background(&mut out, (320, 200), &log, &look);
        assert_eq!((out[at(0, 0)], out[at(1, 0)], out[at(0, 1)]), (1, 1, 1), "its surviving ink is restored");
        assert_eq!((out[at(1, 1)], out[at(5, 7)], out[at(5, 0)]), (9, 9, 9), "the cursor and the outline stay");
    }
    fn word(s: &str, x: i64, y: i64, w: u8, bg: u8) -> Vec<Run> { s.bytes().enumerate().map(|(i, c)| run(x + i as i64 * w as i64, y, w, c, bg)).collect() }
    fn lower(s: &str, x: i64, y: i64, w: u8) -> Vec<Run> {
        word(s, x, y, w, 0).into_iter().map(|mut r| { r.descend = true; r.background_height = 10; r.background = vec![0; w as usize * 10]; r.bitmap = r.background.clone(); r }).collect()
    }
    fn laid<'a>(runs: &'a [Vec<Run>], indexed: &[u8]) -> Vec<(Vec<&'a Run>, Laid)> {
        layout(runs.iter().flatten().collect(), (320, 200), indexed, (4.0, 4.8), 1)
    }
    fn at<'a>(l: &'a [(Vec<&Run>, Laid)], x: i64, row: i64) -> &'a Laid { &l.iter().find(|(_, g)| g.x0 == x && g.row == row).expect("a line starts there").1 }
    #[test]
    fn emblem_rank_numeral_stays_the_1990_bitmap_but_a_lone_small_digit_is_text() {
        let mut log = TextLog::default();
        let mut icon = run(10, 10, 12, 0x04, 1); icon.colour = 0; icon.bitmap = vec![0; 12 * 8];
        let mut digit = run(13, 11, 5, 0xB4, 0); digit.colour = 5; for (i, p) in digit.bitmap.iter_mut().enumerate() { if i % 5 == 2 { *p = 5 } }
        let mut lone = run(100, 100, 5, 0xB4, 1); lone.bitmap[2] = 5;
        log.visible = vec![icon.clone(), digit.clone(), lone.clone()];
        assert!(badge(&digit, &log) && !badge(&lone, &log) && !badge(&icon, &log));
        let mut indexed = [1u8; 64000];
        for r in [&icon, &digit, &lone] { for dy in 0..8 { for dx in 0..r.width as usize { indexed[(r.y as usize + dy) * 320 + r.x as usize + dx] = r.bitmap[dy * r.width as usize + dx] } } }
        let kept = background(&indexed, &log, &Look::default());
        assert_eq!(kept[11 * 320 + 15], 5, "the numeral's ink is kept"); assert_eq!(kept[100 * 320 + 100], 1, "the lone digit's cell is restored to its background for the re-lay");
        let rect = PictureRect { x: 0, y: 0, width: 320, height: 200, out_width: 320, out_height: 200 };
        let base: Vec<u32> = kept.iter().map(|&c| c as u32 * 0x111111).collect(); let mut out = base.clone();
        overdraw(&mut out, rect, &kept, &log, &Look::default());
        let changed = |o: &Vec<u32>, x: usize, y: usize, w: usize| (y..y + 8).any(|py| (x..x + w).any(|px| o[py * 320 + px] != base[py * 320 + px]));
        if font_menu::selected(Look::default().text_font()).is_some() { assert!(changed(&out, 100, 100, 8), "a lone small digit is re-laid"); }
        assert!(!changed(&out, 10, 9, 14), "the emblem and its numeral are untouched");
    }
    #[test]
    fn width_budget_gap_columns_one_size_per_box_and_the_floor() {
        let runs = [word("Do you wish to develop this character?", 90, 61, 6, 1), word("age", 190, 30, 10, 7)];
        let l = laid(&runs, &[]);
        assert_eq!(l.len(), 2);
        let q = at(&l, 90, 61); assert!(q.adv * 38 <= 38 * 6 * 4, "{} px over a {} px budget", q.adv * 38, 38 * 6 * 4); assert_eq!((q.h, q.adv), (42, 24));
        assert_eq!((at(&l, 190, 30).h, at(&l, 190, 30).adv), (67, 38), "10 px glyphs: legible 14 rows at 4.8x");
        let runs = [word("Strength", 130, 125, 6, 0), word("4", 200, 125, 6, 0), word("Yes", 250, 125, 6, 4), word("No", 275, 125, 6, 0)];
        let l = laid(&runs, &[]);
        assert_eq!(l.iter().map(|(_, g)| (g.x0, g.x1)).collect::<Vec<_>>(), [(130, 178), (200, 206), (250, 268), (275, 287)]);
        assert_eq!(at(&l, 250, 125).h, at(&l, 275, 125).h);
        let labels = ["Strength", "Dexterity", "Endurance", "Intelligence", "Education", "Social St."];
        let mut runs: Vec<Vec<Run>> = labels.iter().enumerate().map(|(i, s)| word(s, 130, 125 + 12 * i as i64, if i == 3 { 6 } else { 7 }, 0)).collect();
        runs.extend(["4", "10", "3", "5", "10", "12"].iter().enumerate().map(|(i, s)| word(s, 210, 125 + 12 * i as i64, 7, 0)));
        let l = laid(&runs, &[]);
        assert_eq!(l.len(), 12);
        assert!(l.iter().all(|(_, g)| g.h == 42 && g.adv == 24), "{:?}", l.iter().map(|(_, g)| (g.x0, g.row, g.h)).collect::<Vec<_>>());
        let runs = [word("Social St.", 10, 10, 2, 0)];
        let l = laid(&runs, &[]); assert_eq!((at(&l, 10, 10).h, at(&l, 10, 10).adv), (38, 8), "the floor keeps the height; the cell narrows to the 80 px budget");
    }
    #[test]
    fn a_line_straddling_the_right_edge_ends_inside_the_surface() {
        let runs = [word("ABCDEFGHI", 270, 10, 6, 1)];
        let l = laid(&runs, &[]);
        let g = at(&l, 270, 10);
        assert_eq!(g.x1, 324, "the 1990 run itself runs 4 px past the edge");
        assert!(edge(g.x0, 1280, 320) + (9 * g.adv) as i64 <= 1280, "{} glyphs x {} px start at {}: past the 1280 px picture", 9, g.adv, edge(g.x0, 1280, 320));
        assert_eq!(g.adv, 22);
    }
    #[test]
    fn a_station_title_fits_its_bar_beside_a_tall_numeral() {
        let mut indexed = vec![3u8; 64000];
        let mut band = |y0: i64, y1: i64, c: u8| for y in y0..y1 { for x in 0..320 { indexed[y as usize * 320 + x] = c; } };
        band(19, 20, 8); band(20, 28, 6); band(28, 29, 8); band(29, 46, 7); band(46, 47, 8);
        let cell = |c: u8, x: i64, y: i64, h: usize, ink: std::ops::RangeInclusive<usize>, descend: bool| {
            let background: Vec<u8> = (0..h * 6).map(|i| indexed[(y as usize + i / 6) * 320 + x as usize + i % 6]).collect();
            let mut bitmap = background.clone(); for r in ink { bitmap[r * 6 + 2] = 15; }
            Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend, background_width: 6, background_height: h as u8, background, bitmap, covered: false }
        };
        let runs = vec![b"thrust".iter().enumerate().map(|(i, &c)| cell(c, 10 + 6 * i as i64, 19, 10, 2..=6, true)).collect::<Vec<_>>(), vec![cell(0xD2, 16, 31, 12, 1..=11, false)]];
        let l = laid(&runs, &indexed);
        let (title, numeral) = (&l.iter().find(|(r, _)| r.len() == 6).unwrap().1, &l.iter().find(|(r, _)| r.len() == 1).unwrap().1);
        let (top, h) = (title.row as f32 * 4.8 + title.dy as f32, title.h as f32);
        assert!(top >= 20.0 * 4.8 - 0.5 && top + h <= 28.0 * 4.8 + 0.5, "the title {top}..{} inside its bar {}..{}", top + h, 20.0 * 4.8, 28.0 * 4.8);
        assert!((top + 0.75 * h - 26.0 * 4.8).abs() <= 4.8, "its baseline near the 1990 one (row 26): {}", top + 0.75 * h);
        assert!(numeral.h > title.h, "the numeral keeps its own size: {} / {}", numeral.h, title.h);
    }
    #[test]
    fn a_two_ink_word_cut_by_a_box_stays_one_word() {
        let mut runs = word("Leave", 125, 13, 6, 7);
        runs[0].colour = 0; for i in 0..6 { runs[0].bitmap[i * 6 + 1] = 0; }
        for r in &mut runs[1..] { r.descend = true; r.colour = 5; r.background_height = 10; r.background = vec![7; 60]; r.bitmap = r.background.clone(); for y in 2..7 { r.bitmap[y * 6 + 2] = 5; } }
        let l = layout(runs.iter().collect(), (320, 200), &[], (4.0, 4.8), 1);
        assert_eq!(l.len(), 1, "one line"); assert_eq!(l[0].0.len(), 5, "five letters");
        let float = crate::panels::Surface::new(320, 100);
        for (top, kept) in [(22, 5), (18, 0)] {
            let mut s = crate::panels::Surface::new(320, 200); s.text.visible = runs.clone();
            s.cover(&float, 0, top);
            assert_eq!(s.text.visible.len(), kept, "a float from row {top}");
        }
    }
    #[test]
    fn a_row_on_a_pop_up_edge_never_rises_into_the_row_above() {
        let mut indexed = vec![1u8; 64000];
        for y in 69..71 { for x in 0..320 { indexed[y * 320 + x] = 0; } }
        let price = |y: i64| { let mut w = word("$70000", 240, y, 6, 1); for r in &mut w { for i in 0..7 { r.bitmap[i * 6 + 2] = 15; } } w };
        let runs = [price(37), price(45), price(53), price(61)];
        let l = layout(runs.iter().flatten().collect(), (320, 200), &indexed, (3.68, 3.91), 1);
        let top = |row: i64| { let g = at(&l, 240, row); (row as f32 * 3.91).round() as i64 + g.dy };
        let bottom = |row: i64| top(row) + at(&l, 240, row).h as i64;
        assert!(top(61) >= bottom(53), "the fourth row ({}) starts under the third ({})", top(61), bottom(53));
    }
    #[test]
    fn packed_list_keeps_an_even_pitch_within_one_pixel() {
        let words = ["atv", "bribery", "cmbt rflmn.", "comm", "computer", "engineering", "laser wpn", "navigation", "pilot", "sword", "trader", "turret wpn", "vacc suit"];
        let runs: Vec<Vec<Run>> = words.iter().enumerate().map(|(i, w)| {
            let y = 60 + 6 * i as i64;
            if w.bytes().any(|c| b"bdfhklt".contains(&c)) { word(w, 10, y, 6, 0) } else { lower(w, 10, y, 6) }
        }).collect();
        let l = laid(&runs, &[]);
        assert_eq!(l.len(), 13);
        let mut ys: Vec<i64> = l.iter().map(|(_, g)| edge(g.row, 960, 200) + g.dy as i64).collect(); ys.sort();
        let pitches: Vec<i64> = ys.windows(2).map(|w| w[1] - w[0]).collect();
        assert!(pitches.iter().all(|p| (p - 29).abs() <= 1), "uneven pitch {pitches:?}");
        assert!(l.iter().all(|(_, g)| g.h == l[0].1.h && g.dy == l[0].1.dy), "one size and one offset per box");
    }
    #[test]
    fn narrative_box_lines_end_above_the_box_bottom() {
        let (top, bottom) = (18i64, 77i64);
        let mut indexed = vec![0u8; 64000];
        for y in top..bottom { for x in 0..320 { indexed[y as usize * 320 + x] = 1; } }
        let runs: Vec<Vec<Run>> = (0..5).map(|i| word("There is a Jump-1 Sh", 10, 20 + 12 * i, 8, 1)).collect();
        let l = laid(&runs, &indexed);
        assert_eq!(l.len(), 5);
        let (sy, shadow) = (4.8, 1.0);
        for (_, g) in &l {
            assert_eq!(g.h, 42, "one size per box, bound by the last line's room to the edge ({} rows)", bottom - 68);
            assert!(g.row as f32 * sy + g.h as f32 + shadow <= bottom as f32 * sy + 0.5, "row {} reaches past the box bottom", g.row);
        }
        let packed: Vec<Vec<Run>> = (0..4).map(|i| word("bribery", 10, 20 + 6 * i, 6, 0)).collect();
        for (_, g) in laid(&packed, &[]) { assert!(g.h as f32 + 1.0 <= 6.0 * 4.8 + 0.5, "row {} overprints the next ({} px)", g.row, g.h); }
        assert!(laid(&runs, &[]).iter().all(|(_, g)| g.h == 56));
        let frame = scene();
        let look = Look { font: FontChoice::Builtin(0), shape: Shape::Square, scale: ScaleMode::Fill, text_colour: crate::look::TextColour::Original, ..Look::default() };
        let (w, h) = (640, 400);
        let original = pipeline::render(&frame, &Look { font: FontChoice::Original, ..look.clone() }, w, h);
        assert_eq!(original, pipeline::render(&frame, &Look { font: FontChoice::Original, ..look.clone() }, w, h), "Original is unchanged");
        assert_eq!(pipeline::render(&frame, &look, w, h).len(), w * h, "clipped to the window");
    }
    #[test]
    fn cut_strip_takes_the_box_fill_in_a_modern_face_and_stays_in_the_original() {
        let mut strip = run(40, 100, 6, b'A', 1); strip.bytes.clear(); strip.background_height = 3; strip.background = vec![1; 18]; strip.bitmap = vec![1; 18];
        for i in [0, 1, 2, 7, 8, 13] { strip.bitmap[i] = 15; }
        let mut indexed = [1u8; 64000];
        for (i, &c) in strip.bitmap.iter().enumerate() { indexed[(100 + i / 6) * 320 + 40 + i % 6] = c; }
        let log = TextLog { visible: vec![strip], ..TextLog::default() };
        let look = Look { font: FontChoice::Builtin(0), ..Look::default() };
        assert!(background(&indexed, &log, &look).iter().all(|&c| c == 1), "the modern face paints the fill over the cut strip");
        assert_eq!(background(&indexed, &log, &Look { font: FontChoice::Original, ..Look::default() }), indexed, "the Original face keeps the 1990 pixels");
        let (rect, mut out) = (PictureRect { x: 0, y: 0, width: 640, height: 400, out_width: 640, out_height: 400 }, vec![7u32; 640 * 400]);
        overdraw(&mut out, rect, &indexed, &log, &look);
        assert!(out.iter().all(|&c| c == 7), "a cut strip is never re-laid");
    }
    #[test]
    fn a_j_is_spaced_like_its_neighbours() {
        let face = font_menu::builtin(0).unwrap();
        for c in "objecmn".chars() {
            let m = mask(&face, c, 16, 28, false, false); let stride = 16 + 2 * m.pad;
            let cols: Vec<usize> = (0..stride).filter(|&x| (9..15).any(|y| m.alpha[y * stride + x] > 127)).collect();
            let centre = (cols[0] + cols[cols.len() - 1] + 1) as f32 / 2.0 - m.pad as f32;
            assert!((centre - 8.0).abs() <= 1.0, "{c}: body centre {centre} in a 16 px cell");
        }
    }
    #[test]
    fn letters_centre_on_their_ink_and_a_lone_1_is_as_big_as_a_0() {
        for i in 0..font_menu::BUILTINS.len() as u8 { let face = font_menu::builtin(i).unwrap();
            for c in "CudgelBrawling".chars() {
                let m = mask(&face, c, 16, 28, false, false); let stride = 16 + 2 * m.pad;
                let cols: Vec<usize> = (0..stride).filter(|&x| (0..28).any(|y| m.alpha[y * stride + x] > 0)).collect();
                let centre = (cols[0] + cols[cols.len() - 1] + 1) as f32 / 2.0 - m.pad as f32;
                assert!((centre - 8.0).abs() <= 1.0, "face {i} {c}: ink centre {centre} in a 16 px cell");
            }
        }
        let runs = [word("0", 72, 66, 7, 0), word("1", 72, 74, 3, 0), word("0", 72, 82, 7, 0)];
        let l = laid(&runs, &[]);
        assert_eq!(at(&l, 72, 74).adv, at(&l, 72, 66).adv, "the 1 and the 0 one width");
        assert_eq!(at(&l, 72, 74).h, at(&l, 72, 66).h, "and one height");
    }
    #[test]
    fn all_fonts_fit_cells_at_integer_fractional_tiny_and_clipped_sizes() {
        let frame = scene();
        for i in 0..font_menu::BUILTINS.len() as u8 { for bold in [false, true] { for shape in [Shape::Classic, Shape::Square] {
            let look = Look { font: FontChoice::Builtin(i), bold, shape, scale: ScaleMode::Fill, ..Look::default() };
            for (w, h) in [(0, 0), (1, 1), (37, 19), (641, 479), (960, 720)] {
                let original = pipeline::render(&frame, &Look { font: FontChoice::Original, ..look.clone() }, w, h);
                let modern = pipeline::render(&frame, &look, w, h);
                let rect = pipeline::scale::picture_rect(w, h, &look);
                let mut changes = 0;
                for (at, (a, b)) in original.iter().zip(&modern).enumerate() {
                    if a == b { continue; } changes += 1;
                    let (x, y) = ((at % w) as i64 - rect.x as i64, (at / w) as i64 - rect.y as i64);
                    assert!(frame.text.visible.iter().any(|r| relaid_band(r, rect, x, y)), "face {i}, bold {bold}, pixel ({x},{y}) outside every re-laid line band");
                }
                if w >= 640 { assert!(changes > 50); }
            }
        } } }
    }
    #[test]
    fn backgrounds_colours_shadow_bold_and_invalid_font_fallback() {
        let frame = scene();
        let look = Look { font: FontChoice::Builtin(0), shape: Shape::Square, text_colour: crate::look::TextColour::Original, ..Look::default() };
        let cleaned = background(&frame.indexed, &frame.text, &look);
        for y in 20..28 { for x in 10..14 {
            assert_eq!(cleaned[y * 320 + x], if (x + y) % 2 == 0 { 1 } else { 2 });
        } }
        let regular = pipeline::render(&frame, &look, 1280, 800);
        let bold = pipeline::render(&frame, &Look { bold: true, ..look.clone() }, 1280, 800);
        assert_ne!(regular, bold);
        let mut without_shadow = Frame { text: frame.text.clone(), ..Frame::default() };
        without_shadow.indexed = frame.indexed;
        for r in &mut without_shadow.text.visible { r.shadow = None; }
        assert_ne!(regular, pipeline::render(&without_shadow, &look, 1280, 800));
        let original = pipeline::render(&frame, &Look { font: FontChoice::Original, ..look.clone() }, 640, 400);
        assert_eq!(original, pipeline::render(&frame, &Look { font: FontChoice::Builtin(255), ..look.clone() }, 640, 400));
        let colors = palette::colors(&frame.regs, &palette::ega_dac());
        assert!(regular.iter().any(|&c| c == colors[15]));
        overdraw(&mut [], PictureRect { width: usize::MAX, height: 1, ..PictureRect::default() }, &[], &frame.text, &look);
        let mut log = frame.text.clone();
        for r in &mut log.visible { r.x = i64::MAX; r.y = i64::MIN; }
        let mut out = vec![0; 64000]; overdraw(&mut out, pipeline::scale::picture_rect(320, 200, &look), &[], &log, &look);
        assert!(out.iter().all(|&c| c == 0));
        let face = font_menu::builtin(0).unwrap();
        let lower_l = mask(&face, 'l', 24, 32, false, true);
        let lower_g = mask(&face, 'g', 24, 32, false, true);
        assert!(lower_l.alpha[16 * 36..].iter().all(|&a| a == 0), "lowercase ascender ends at the original baseline");
        assert!(lower_g.alpha[16 * 36..].iter().any(|&a| a != 0), "descender remains below the original baseline");
    }
    #[test]
    fn font_codes_render_as_what_the_bitmap_shows() {
        assert_eq!([0xB1, 0xB4, 0xB0, 0xB9, 0xAF, 0xD0, 0xD7, b'A', b'z', b' '].map(shown),
                   ['1', '4', '0', '9', '/', '0', '7', 'A', 'z', ' '].map(Some));
        assert!([0x00, 0x03, 0x0A, 0x1F, b'[', b'^', b'|', b'~', 0x7F, 0xAE, 0xBA, 0xCF, 0xDA, 0xE9, 0xFF].iter().all(|&c| shown(c).is_none()));
        textlog::set_enabled(true);
        let mut fb = Fb::new();
        let mut entries = vec![Vec::new(); 256];
        for c in [0xB1usize, b'1' as usize, 0xE9] { entries[c] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 4, x2: 1 }]; }
        let font = Font::new(entries, [4; 256], &[0; 256], 1);
        text::draw(&mut fb, Raster::Clip, 0, &font, b"\xB1\xE9", 20, 10, 15, Some(4));
        let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        textlog::set_enabled(false);
        assert_eq!(frame.text.visible.len(), 2);
        let mut ascii = Frame { text: frame.text.clone(), ..Frame::default() }; ascii.indexed = frame.indexed;
        ascii.text.visible[0].bytes = vec![b'1'];
        for i in 0..font_menu::BUILTINS.len() as u8 {
            let look = Look { font: FontChoice::Builtin(i), ..Look::default() };
            assert_eq!(pipeline::render(&frame, &look, 640, 400), pipeline::render(&ascii, &look, 640, 400), "face {i}");
            let cleaned = background(&frame.indexed, &frame.text, &look);
            for y in 20..25 { for x in 15..17 { assert_eq!(cleaned[y * 320 + x], frame.indexed[y * 320 + x], "face {i}: bitmap at {x},{y} erased"); } }
            assert_eq!(frame.indexed[20 * 320 + 15], 15);
        }
    }
    #[test]
    fn erase_includes_last_bitmap_column_descender_and_shadow_row() {
        textlog::set_enabled(true);
        let mut fb = Fb::new();
        for y in 0..200 { for x in 0..320 { fb.pixel(Raster::Clip, 0, y, x, if (x + y) % 2 == 0 { 1 } else { 2 }, 0); } }
        let clean: [u8; 64000] = fb.indexed(0).try_into().unwrap();
        let mut entries = vec![Vec::new(); 256];
        entries[b'g' as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 7, x2: 3 }];
        let mut flags = [0; 256]; flags[b'g' as usize] = 1;
        let font = Font::new(entries, [4; 256], &flags, 1);
        for (y, x) in [(20, 10), (197, 318), (-2, -1)] {
            text::draw(&mut fb, Raster::Clip, 0, &font, b"g", y, x, 15, Some(4));
        }
        let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        textlog::set_enabled(false);
        assert_eq!(frame.text.visible.len(), 3);
        assert_eq!(frame.indexed[30 * 320 + 14], 4);
        for i in 0..font_menu::BUILTINS.len() as u8 {
            let look = Look { font: FontChoice::Builtin(i), ..Look::default() };
            assert_eq!(background(&frame.indexed, &frame.text, &look), clean, "face {i}: bitmap ink survived erasure");
        }
    }
    #[test]
    fn redrawing_overlapping_shadow_cells_preserves_neighbors_and_clean_underlay() {
        textlog::set_enabled(true);
        let mut fb = Fb::new(); fb.fill(Raster::Clip, 0, 0, 0, 200, 320, 3, 0);
        let clean: [u8; 64000] = fb.indexed(0).try_into().unwrap();
        let mut entries = vec![Vec::new(); 256];
        for c in b"ABCD" { entries[*c as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 7, x2: if *c == b'D' { 1 } else { 3 } }]; }
        let mut widths = [4; 256]; widths[b'D' as usize] = 2;
        let font = Font::new(entries, widths, &[0; 256], 1);
        let look = Look { font: FontChoice::Builtin(1), ..Look::default() };
        for colour in [15, 6, 2, 15] {
            text::draw(&mut fb, Raster::Clip, 0, &font, b"ABC", 20, 10, colour, Some(1));
            let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
            assert_eq!(frame.text.visible.len(), 3);
            assert_eq!(background(&frame.indexed, &frame.text, &look), clean);
        }
        text::draw(&mut fb, Raster::Clip, 0, &font, b"D", 20, 10, 15, None);
        let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        assert_eq!(frame.text.visible.len(), 3);
        assert_eq!(frame.text.visible.last().unwrap().width, 2);
        assert_eq!(background(&frame.indexed, &frame.text, &look), clean, "the previous wider glyph and its shadow are also erased");
        textlog::set_enabled(false);
    }
    #[test]
    fn overdraw_fits_cells_on_real_filters_and_effects() {
        let frame = scene();
        for i in 0..font_menu::BUILTINS.len() as u8 { for bold in [false, true] { for shape in [Shape::Classic, Shape::Square] {
            for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] { for screen in [Screen::None, Screen::Scanlines, Screen::Crt, Screen::Phosphor] {
                let look = Look { font: FontChoice::Builtin(i), bold, shape, filter, screen, scale: ScaleMode::Fill, ..Look::default() };
                let clean = Frame { indexed: background(&frame.indexed, &frame.text, &look), regs: frame.regs, ..Frame::default() };
                for (w, h) in [(1, 1), (37, 19), (641, 479)] {
                    let base = pipeline::render(&clean, &Look { font: FontChoice::Original, ..look.clone() }, w, h);
                    let modern = pipeline::render(&frame, &look, w, h);
                    let rect = pipeline::scale::picture_rect(w, h, &look);
                    let mut changed = false;
                    for (at, (a, b)) in base.iter().zip(&modern).enumerate() {
                        if a == b { continue; } changed = true;
                        let (x, y) = ((at % w) as i64 - rect.x as i64, (at / w) as i64 - rect.y as i64);
                        assert!(frame.text.visible.iter().any(|r| relaid_band(r, rect, x, y)), "pixel ({x},{y}) outside every re-laid line band");
                    }
                    if w > 640 { assert!(changed); }
                }
            } }
        } } }
    }
    #[test]
    fn overflow_uses_the_whole_bitmap_picture_for_that_frame() {
        let mut frame = scene();
        frame.text.overflowed = true;
        for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] {
            let look = Look { font: FontChoice::Builtin(0), bold: true, filter, screen: Screen::Crt, ..Look::default() };
            assert_eq!(pipeline::render(&frame, &look, 641, 479), pipeline::render(&frame, &Look { font: FontChoice::Original, ..look }, 641, 479));
        }
    }
    #[test]
    #[ignore = "manual release measurement of 1000 repeated glyphs and the integrated pipeline"]
    fn text_release_budget() {
        use std::{hint::black_box, time::Instant};
        assert!(!cfg!(debug_assertions), "run with --release");
        textlog::set_enabled(true);
        let mut entries = vec![Vec::new(); 256];
        for c in b'A'..=b'Z' { entries[c as usize] = vec![Rec { op: 2, colour: 16, y1: 0, x1: 0, y2: 4, x2: 3 }]; }
        let font = Font::new(entries, [6; 256], &[0; 256], 1);
        let mut fb = Fb::new();
        let draw = |fb: &mut Fb| { for i in 0..1000 {
            text::glyph(fb, Raster::Clip, 0, &font, b'A' + (i % 26) as u8, (i / 50) * 10, (i % 50) * 6, &mut mt_engine::present::ly::Pen::new(15), 15, Some(1));
        } };
        draw(&mut fb);
        let mut frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        let report = |label: &str, times: &mut Vec<f64>| {
            times.sort_by(f64::total_cmp);
            println!("measure: {label}: median {:.3} ms / p95 {:.3} ms / max {:.3} ms (40 frames)", times[20], times[37], times[39]);
        };
        let mut times = Vec::new();
        for _ in 0..40 {
            let start = Instant::now(); draw(&mut fb);
            frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
            times.push(start.elapsed().as_secs_f64() * 1000.0);
            assert_eq!(frame.text.visible.len(), 1000); assert!(!frame.text.overflowed);
        }
        report("1000 existing-cell redraws + RGB/log handoff", &mut times);
        for shape in [Shape::Classic, Shape::Square] {
            for (filter, screen) in [(Filter::Nearest, Screen::None), (Filter::Sharp, Screen::Crt), (Filter::Soft, Screen::Crt)] {
                let look = Look { font: FontChoice::Builtin(0), bold: true, shape, filter, screen, palette: Palette::Amber, scale: ScaleMode::Fill, ..Look::default() };
                for _ in 0..5 { black_box(pipeline::render(&frame, &look, 2560, 1440)); }
                times.clear();
                for _ in 0..40 {
                    let start = Instant::now(); black_box(pipeline::render(black_box(&frame), &look, 2560, 1440));
                    times.push(start.elapsed().as_secs_f64() * 1000.0);
                }
                report(&format!("2560x1440 1000 glyphs Px437 Bold {shape:?}/{filter:?}/{screen:?}/Amber"), &mut times);
            }
        }
        textlog::set_enabled(false);
    }
}
