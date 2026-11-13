use super::{Host, Result};
use mt_engine::{
    data::ly::{Entry, Rec},
    present::{
        fb::Fb,
        ly::{self, Pen},
        prim::Raster,
        text::{self, Font},
    },
};

fn records(h: &dyn Host, off: i32, seg: i32) -> Result<Entry> {
    let mut out = Vec::new();
    for i in 0..4096 {
        let at = (off + i * 10) & 65535;
        let op = h.read(at, seg, 1) as u8;
        if op == 0 {
            return Ok(out);
        }
        let w = |n| h.read((at + n) & 65535, seg, 2) as u16 as i16;
        out.push(Rec {
            op,
            colour: h.read((at + 1) & 65535, seg, 1) as u8,
            y1: w(2),
            x1: w(4),
            y2: w(6),
            x2: w(8),
        });
    }
    Err("S raster: unterminated layout entry".into())
}
fn layout(h: &dyn Host, p: [i32; 2]) -> Result<Vec<Entry>> {
    let n = h.read(p[0], p[1], 1) as usize;
    let table = h.read((p[0] + 1) & 65535, p[1], 2) as i32;
    let seg = h.read((p[0] + 3) & 65535, p[1], 2) as i32;
    (0..n)
        .map(|i| {
            let off = h.read((table + i as i32 * 4) & 65535, seg, 2) as i32;
            let s = h.read((table + i as i32 * 4 + 2) & 65535, seg, 2) as i32;
            if off == 0 && s == 0 {
                Ok(Vec::new())
            } else {
                records(h, off, s)
            }
        })
        .collect()
}
fn combine(old: u8, value: u8, op: i32) -> u8 {
    match op {
        8 => old & value,
        16 => old | value,
        24 => old ^ value,
        _ => value,
    }
}
fn offset(off: i32, seg: i32) -> usize {
    ((seg as u16 as i64 * 16 + off as u16 as i64 - 0xA0000) & 65535) as usize
}

pub fn draw(fb: &mut Fb, h: &dyn Host, address: u32, args: &[i32]) -> Result<()> {
    let needed = match address {
        0xBD20 => 14,
        0xE43F => 5,
        0xE389 => 4,
        0xE3B5 => 3,
        0xEB83 | 0xEFDE | 0xE411 => 5,
        0xE42B => 3,
        0xEF1E => 10,
        0xD1BE => 1,
        _ => return Err(format!("S raster: unsupported frontier {address:X}")),
    };
    if args.len() != needed {
        return Err(format!("S raster {address:X}: expected {needed} arguments"));
    }
    let a = |i: usize| args[i] as i16 as i32;
    let u = |i: usize| args[i] as u16 as i32;
    let start = offset(0, h.w(0x2A15));
    if start % 8192 != 0 {
        return Err("S raster: non-page drawing segment".into());
    }
    let page = start / 8192;
    let r = Raster::Wrap;
    let mut pen = Pen::new(h.b(0x2A2A) as u8);
    pen.op = h.b(0x2A14) as u8;
    match address {
        0xE3B5 => {
            let entries = layout(h, h.ptr(0x4996))?;
            ly::layout(fb, r, page, &entries, u(0) as usize, a(1), a(2), &mut pen)
        }
        0xE389 => {
            let mut entries = layout(h, h.ptr(0x4996))?;
            let index = entries.len();
            entries.push(records(h, u(0), u(1))?);
            ly::entry(fb, r, page, &entries, index, a(2), a(3), &mut pen)
        }
        0xEB83 | 0xEFDE => fb.fill(r, page, a(0), a(1), a(2), a(3), a(4) as u8, pen.op),
        0xE411 => fb.line(r, page, a(0), a(1), a(2), a(3), a(4) as u8, pen.op),
        0xE42B => fb.pixel(r, page, a(0), a(1), a(2) as u8, pen.op),
        0xEF1E => {
            let (src, dst) = (offset(u(0), u(1)) as i64, offset(u(6), u(7)) as i64);
            for row in 0..a(4).max(0) {
                for col in 0..a(5).max(0) {
                    let s = ((src + (a(2) + row) as i64 * 40 + (a(3) + col) as i64) & 65535) as usize;
                    let d = ((dst + (a(8) + row) as i64 * 40 + (a(9) + col) as i64) & 65535) as usize;
                    for plane in 0..4 {
                        fb.planes[plane * 65536 + d] = fb.planes[plane * 65536 + s];
                    }
                }
            }
        }
        0xD1BE => {
            let ch = u(0) as u8;
            if ch == h.b(0x4993) as u8 {
                return Ok(());
            }
            let glyphs = layout(h, h.ptr(0x4987))?;
            let widths = std::array::from_fn(|i| h.b(0x288A + i as i32) as u8);
            let flags = std::array::from_fn(|i| h.b(0x2E51 + i as i32) as u8);
            let font = Font::new(glyphs, widths, &flags, 8);
            let shadow = h.b(0x4995) & 1 != 0;
            let color = if shadow { h.b(0x4994) as u8 } else { pen.colour };
            let b = if ch == 0xE1 { vec![b's', b's'] } else { vec![ch] };
            text::draw(
                fb,
                r,
                page,
                &font,
                &b,
                h.w(0x498F),
                h.w(0x4991),
                color,
                shadow.then_some(h.b(0x48D0) as u8),
            );
        }
        0xE43F => {
            let (off, seg) = (u(0), u(1));
            let w = |delta| h.read((off + delta) & 65535, seg, 2) as i32;
            let (height, width, planes, po, ps) = (w(0), w(2), w(4), w(8), w(10));
            if planes > 4 {
                return Err("S bitmap: plane count > 4".into());
            }
            let dst = offset(h.w(0x2792), h.w(0x2794)) as i64;
            let (sy, sx, rows, cols) = (h.w(0x49F2) & 65535, h.w(0x49EC) & 65535, h.w(0x49F4) & 65535, h.w(0x49F6) & 65535);
            if rows == 0 || cols == 0 {
                return Err("S bitmap: zero size".into());
            }
            for plane in 0..planes {
                for row in 0..rows {
                    for col in 0..cols {
                        let src = (po as i64
                            + plane as i64 * height as i64 * width as i64
                            + (sy + row) as i64 * width as i64
                            + (sx + col) as i64)
                            & 65535;
                        let value = h.read(src as i32, ps, 1) as u8;
                        let pos = ((dst + (a(2) + row) as i64 * (h.w(0x279C) & 65535) as i64 + (a(3) + col) as i64) & 65535) as usize
                            + plane as usize * 65536;
                        fb.planes[pos] = combine(fb.planes[pos], value, u(4));
                    }
                }
            }
        }
        0xBD20 => {
            let op = u(12);
            if !matches!(op, 0 | 8 | 16 | 24) {
                return Err("BD20: invalid operation".into());
            }
            let rows = if u(10) == 0 { 65536 } else { u(10) };
            let cols = if u(11) == 0 && op != 0 { 65536 } else { u(11) };
            let src = (u(0) as i64 + u(7) as i64 * u(9) as i64 + u(8) as i64) & 65535;
            let dst = (u(2) as i64 + u(4) as i64 * u(6) as i64 + u(5) as i64) & 65535;
            for row in 0..rows {
                for col in 0..cols {
                    let so = ((src + row as i64 * u(9) as i64 + col as i64) & 65535) as i32;
                    let d = u(3) as i64 * 16 + ((dst + row as i64 * u(6) as i64 + col as i64) & 65535) - 0xA0000;
                    if !(0..65536).contains(&d) {
                        return Err("BD20: destination outside EGA aperture".into());
                    }
                    let value = h.read(so, u(1), 1) as u8;
                    for plane in 0..4 {
                        if u(13) & (1 << plane) != 0 {
                            let at = plane * 65536 + d as usize;
                            fb.planes[at] = combine(fb.planes[at], value, op)
                        }
                    }
                }
            }
        }
        _ => return Err("S raster: unsupported frontier".into()),
    }
    Ok(())
}
