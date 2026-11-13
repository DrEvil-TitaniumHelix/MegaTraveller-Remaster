use crate::ground::{render, state::{Far, Ground, Mem}};
use mt_engine::{data::ly as data_ly, present::{ly::{self, Pen}, prim::Raster, text::{Cursor, Font}}};

pub fn bytes(g: &Ground, p: Far) -> Vec<u8> {
    (0..65536).map(|i| g.mem.read(p.add(i), 1) as u8).take_while(|&b| b != 0).collect()
}
pub fn string(b: &[u8]) -> String { b.iter().map(|&c| c as char).collect() }
pub fn frame(g: &mut Ground, name: &str) {
    g.frame(name);
    if g.mem.u16(0x52F4) & 1 != 0 { g.input.present(&g.fb.rgb(1, &g.dac)) }
}
pub fn page(g: &Ground) -> usize {
    let p = g.mem.pointer(0x503A);
    (((p.seg as usize * 16 + p.off as usize).wrapping_sub(0xA0000)) & 65535) / 8192
}
pub fn target(g: &mut Ground, front: bool) {
    let n = g.mem.u16(0x52F4) & 1;
    g.mem.set_u16(0x7198, front as u16);
    g.mem.set_u16(0x52B3, 0xA000 + ((n ^ (!front as u16)) << 9));
    let p = g.mem.pointer(0x7168 + 4 * (n ^ front as u16));
    g.mem.set_pointer(0x503A, p);
}
pub fn present(g: &mut Ground, y: i32, x: i32, height: i32, width: i32) {
    let n = g.mem.u16(0x52F4) & 1;
    let source = g.mem.pointer(0x7168 + 4 * n);
    let dest = g.mem.pointer(0x7168 + 4 * (n ^ 1));
    let base = |p: Far| ((p.seg as usize * 16 + p.off as usize).wrapping_sub(0xA0000) & 65535) / 8192;
    g.fb.copy(base(source), y, x, height, width, base(dest), y, x);
}
pub fn ink(g: &mut Ground, color: u8) { g.mem.set_u8(0x52C8, color); g.mem.set_u8(0x713E, color) }
pub fn layout(g: &mut Ground, index: usize, row: i32, col: i32) {
    let Some(data) = g.files().read("GROUND.LY") else { return };
    let Ok(entries) = data_ly::decode(&data) else { return };
    let mut pen = Pen::new(g.mem.u8(0x52C8));
    let p = page(g); ly::layout(&mut g.fb, Raster::Wrap, p, &entries, index, row, col, &mut pen);
    g.mem.set_u8(0x52C8, pen.colour); g.mem.set_u8(0x52B2, pen.op);
}
pub fn font(g: &mut Ground) -> Option<Font> {
    let entries = data_ly::decode(&g.files().read("FONT.LY")?).ok()?;
    Font::from_tables(entries, &g.mem.ds, 0x5128, 0x5637, 8)
}
pub fn cursor(g: &Ground) -> Cursor {
    Cursor { r: Raster::Wrap, page: page(g), y: g.mem.i16(0x7139) as i64, x: g.mem.i16(0x713B) as i64,
        pen: g.mem.u8(0x52C8), colour: g.mem.u8(0x713E), shadow: g.mem.u8(0x713F) & 1 != 0,
        bg: g.mem.u8(0x707A), tab: g.mem.u8(0x713D), tabs: [g.mem.i16(0x5228) as i64, g.mem.i16(0x522A) as i64],
        window_x: g.mem.i16(0x6E08) as i64, ops: std::array::from_fn(|i| g.mem.u8(0x52C4 + i as u16)) }
}
pub fn finish(g: &mut Ground, c: &Cursor) {
    g.mem.set_i16(0x7139, c.y as i16); g.mem.set_i16(0x713B, c.x as i16);
    g.mem.set_u8(0x52C8, c.pen); g.mem.set_u8(0x713E, c.colour); g.mem.set_u8(0x52B2, 0);
}
pub fn text(g: &mut Ground, s: &[u8], y: i32, x: i32) {
    if let Some(font) = font(g) {
        let mut c = cursor(g); c.at(y as i64, x as i64); c.text(&mut g.fb, &font, s); finish(g, &c);
    }
}
pub fn wrapped(g: &mut Ground, s: &[u8], upper: bool) {
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) | 1);
    if let Some(font) = font(g) {
        let mut c = cursor(g);
        let (y,x,w,h,lh) = if upper { (58,3,187,142,8) } else { (169,1,189,40,9) };
        c.wrapped(&mut g.fb, &font, s, y,x,w,h,lh,6,9); finish(g, &c);
    }
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) & !1);
}
pub fn help(g: &mut Ground, s: &[u8]) {
    if g.mem.u8(0x6C3C) != 0 {
        target(g, true); ink(g, 6); layout(g, 1, 54, 0); wrapped(g, s, true); target(g, false);
    } else { message(g, s) }
}
pub fn message(g: &mut Ground, s: &[u8]) {
    let n = s.len().min(65534); let p = Far::new(0, 0x8F00);
    g.mem.alloc(p.seg, n + 1); g.mem.write_bytes(p, &s[..n]); g.mem.write(p.add(n as i32), 0, 1);
    render::show_message(g, p);
}
pub fn default_menu() -> Far { Mem::ds(0x435B) }
