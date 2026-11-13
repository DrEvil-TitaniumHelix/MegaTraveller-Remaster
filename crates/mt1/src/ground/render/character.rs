use super::*;
use crate::ground::commands_items as items;

fn literal(g: &Ground, text: &[u8]) -> Far {
    g.mem
        .ds
        .windows(text.len() + 1)
        .position(|w| w[..text.len()] == *text && w[text.len()] == 0)
        .map_or(Far::default(), |i| Mem::ds(i as u16))
}
pub fn slot(g: &mut Ground, who: i16, slot: i16) -> i16 {
    let id = items::icon(g, who, slot);
    if id != -1 {
        let off = 0x3BE4u16.wrapping_add((slot as u16).wrapping_mul(4));
        let (y, x) = (g.mem.i16(off), g.mem.i16(off.wrapping_add(2)));
        let mut p = Paint::new(g, Raster::Planar);
        p.item_icon(id as u8, y as i32, (x >> 3) as i32 * 8);
        p.finish();
    }
    id
}
pub fn erase_slot(g: &mut Ground, who: i16, slot: i16) {
    let big = items::icon(g, who, slot) >= 100;
    let off = 0x3BE4u16.wrapping_add((slot as u16).wrapping_mul(4));
    let (y, x) = (g.mem.i16(off) as i32, g.mem.i16(off.wrapping_add(2)) as i32);
    let mut p = Paint::new(g, Raster::Planar);
    p.fill(y, x + 1, 15, 23, 0);
    if big {
        p.fill(y, x + 24, 15, 1, if slot >= 19 { 0 } else { 8 });
        p.fill(y, x + 25, 15, 23, 0)
    }
    p.finish();
}
pub fn highlight(g: &mut Ground, who: i16, slot: i16, color: u8) {
    let width = if items::icon(g, who, slot) >= 100 {
        47
    } else {
        23
    };
    let off = 0x3BE4u16.wrapping_add((slot as u16).wrapping_mul(4));
    let (y, x) = (g.mem.i16(off) as i32, g.mem.i16(off.wrapping_add(2)) as i32);
    let mut p = Paint::new(g, Raster::Planar);
    p.rect(y, x + 1, y + 14, x + width, color);
    p.finish();
}
pub fn hint(g: &mut Ground, index: u16) {
    if index >= 6 {
        return;
    }
    let text = g.mem.pointer(0x3FCD + index * 4);
    let mut p = Paint::new(g, Raster::Planar);
    let (y, x, h, w, c, ty) = match index {
        0 | 1 => (80, 98, 7, 66, 6, 80),
        2 | 3 => (94, 186, 5, 111, 10, 92),
        _ => (102, 186, 5, 111, 10, 100),
    };
    p.fill(y, x, h, w, c);
    p.at(ty, x);
    p.ink(0);
    let t = p.string(text);
    p.text(&t);
    p.finish();
    if index < 2 {
        g.mem.set_u8(0x3E49, g.mem.u8(0x3FE5 + index))
    }
}
pub fn selection(g: &mut Ground, row: i16, erase: bool) {
    let mut p = Paint::new(g, Raster::Planar);
    if erase {
        p.fill(row as i32 * 8 + 76, 176, 9, 6, 0x87)
    } else {
        p.ly("FONT", 5, row as i32 * 8 + 76, 176)
    }
    p.finish();
}
pub fn sheet(g: &mut Ground, who: i16, flip_page: bool) {
    cursor_off(g);
    for (row, text) in [
        (
            8,
            if g.mem.i16(0x6EFC) != 0 && g.mem.i16(0x6ECA) == 0 {
                b"Move object".as_slice()
            } else {
                b"Pick object"
            },
        ),
        (
            2,
            if items::kind(g, who, 21) == 2 {
                b"primary weapon".as_slice()
            } else {
                b"primary object"
            },
        ),
        (
            3,
            if items::kind(g, who, 22) == 2 {
                b"secondary weapon".as_slice()
            } else {
                b"secondary object"
            },
        ),
    ] {
        let p = literal(g, text);
        g.mem.set_pointer(0x3CD0 + row * 9 + 4, p)
    }
    let mut p = Paint::new(g, Raster::Planar);
    p.ly("CH", 0, 54, 0);
    for i in 0..128u16 {
        let a = 0x3CD0 + i * 9;
        let color = p.g.mem.u8(a + 8);
        if color == 255 {
            break;
        }
        let ptr = p.g.mem.pointer(a + 4);
        if ptr != Far::default() {
            let t = p.string(ptr);
            p.ink(color);
            p.at(p.g.mem.i16(a) as i32, p.g.mem.i16(a + 2) as i32);
            p.text(&t)
        }
    }
    let base = (who as u16).wrapping_mul(256);
    let career = p.g.mem.party_u8(base.wrapping_add(0x122)) as u16;
    let rank = p.g.mem.party_u8(base.wrapping_add(0x123)) as u16;
    p.counted(who as u8, career as u8, rank as u8, 58, 98);
    p.at(57, 126);
    p.text(b"ex-");
    p.pointed_text(0x1465 + 4 * career);
    if career != 3 {
        p.text(b" ");
        let title = p.g.mem.u8(0x13F6 + rank * 5 + career) as u16;
        p.pointed_text(0x1419 + title * 4)
    }
    p.at(65, 126);
    let name = p.string(p.g.mem.pointer(0x6D65).add(who as i32 * 256 + 0x102));
    p.text(&name);
    p.finish();
    let mut i = 0;
    while i < 19 {
        if g.mem.party_u16(items::inv(who, i)) != 0 && slot(g, who, i) >= 100 {
            i += 1
        }
        i += 1;
    }
    for i in 19..23 {
        if g.mem.party_u16(items::inv(who, i)) != 0 {
            slot(g, who, i);
        }
    }
    if g.mem.i16(0x6EFC) != 0 {
        highlight(g, who, g.mem.i16(0x6EFC).wrapping_sub(1), 10)
    }
    skills(g, who);
    select_panel(g, 10, g.mem.u8(0x111u16.wrapping_add(who as u16)));
    if flip_page {
        flip(g)
    }
}
fn skills(g: &mut Ground, who: i16) {
    let mut i = g.mem.i16(0x6ECE);
    let mut n = 0;
    let mut p = Paint::new(g, Raster::Planar);
    p.ink(10);
    while i < 80 && n < 21 {
        let level = p.g.mem.party_u8(
            (who as u16)
                .wrapping_mul(256)
                .wrapping_add(i as u16)
                .wrapping_add(0x12B),
        );
        if level != 0 {
            p.at(n * 6 + 66, 5);
            p.pointed_text(0x1479u16.wrapping_add((i as u16).wrapping_mul(4)));
            p.at(n * 6 + 66, 76);
            p.shifted(level as i32, 0x80);
            n += 1;
        }
        i = i.wrapping_add(1);
    }
    if i >= 80 {
        p.g.mem.set_i16(0x6ED0, 1)
    }
    p.finish();
}
pub fn hands(g: &mut Ground, who: i16) {
    let mut p = Paint::new(g, Raster::Planar);
    p.hands(who as u16);
    p.finish();
}
pub fn health_bar(g: &mut Ground, who: i16) {
    let base = (who as u16).wrapping_mul(256).wrapping_add(0x102);
    let cur = g.mem.party_u8(base.wrapping_add(0x25)) as i16;
    let max = g.mem.party_u8(base.wrapping_add(0x23)) as i16;
    let split = g.mem.party_u8(base.wrapping_add(0x24)) as i16;
    g.enter("draw_gauge");
    let mut p = Paint::new(g, Raster::Planar);
    let x = who as i32 * 40 + 39;
    if cur != 0 {
        let n = scale(cur, max, 45) as i32;
        p.fill(3, x, 45 - n, 4, 0);
        p.fill(48 - n, x, n, 4, who.wrapping_add(1) as u8);
    }
    p.finish();
    p.g.frame("draw_gauge");
    if cur != 0 {
        p.fill(48 - scale(split, max, 45) as i32, x, 2, 4, 6);
    }
    p.finish();
}
impl Paint<'_> {
    fn labels(&mut self, table: u16, y: i32, x: i32) {
        for i in 0..128u16 {
            let a = table.wrapping_add(i * 9);
            let color = self.g.mem.u8(a.wrapping_add(8));
            if color == 255 {
                break;
            }
            let ptr = self.g.mem.pointer(a.wrapping_add(4));
            if ptr == Far::default() {
                continue;
            }
            let t = self.string(ptr);
            self.at(
                self.g.mem.i16(a) as i32 + y,
                self.g.mem.i16(a.wrapping_add(2)) as i32 + x,
            );
            self.ink(color);
            self.text(&t);
        }
    }
    fn number(&mut self, n: i32, y: i32, x: i32) {
        self.at(y, x);
        self.shifted(n, 0x80)
    }
    fn hex6(&mut self, who: i16, offset: u16, vertical: bool) {
        for i in 0..6 {
            let value = self
                .g
                .mem
                .party_u8((who as u16).wrapping_mul(256).wrapping_add(offset + i));
            let (y, x) = (self.text.y, self.text.x);
            self.glyph(value.wrapping_add(if value < 10 { b'0' } else { b'7' }));
            if vertical {
                self.text.at(y + 8, x)
            }
        }
    }
    fn career(&mut self, who: i16, y: i32, x: i32) {
        let base = (who as u16).wrapping_mul(256);
        let service = self.g.mem.party_u8(base.wrapping_add(0x122)) as u16;
        self.at(y, x);
        self.text(b"ex-");
        self.pointed_text(0x1465 + service * 4);
        if service != 3 {
            self.text(b" ");
            let rank = self.g.mem.party_u8(base.wrapping_add(0x123)) as u16;
            let title = self.g.mem.u8(0x13F6 + rank * 5 + service) as u16;
            self.pointed_text(0x1419 + title * 4)
        }
    }
    fn meter(
        &mut self,
        y: i32,
        x: i32,
        h: i32,
        color: u8,
        value: i16,
        max: i16,
        width: i16,
    ) -> i16 {
        if value == 0 {
            return 0;
        }
        let n = scale(value, max, width);
        self.fill(y, x, h, n as i32, color);
        n
    }
    fn ammo(&mut self, who: i16, slot: i16, y: i32, x: i32) -> i16 {
        let Some((it, d)) = items::item(self.g, who, slot) else {
            return -1;
        };
        if self.g.mem.read(d.add(29), 2) != 2 {
            return -1;
        }
        let kind = (self.g.mem.read(d.add(39), 1) & 0xC0) as i16;
        if kind != 0 {
            let glyph = if kind == 0x40 { 7 } else { 8 };
            self.at(y, x);
            self.ink(10);
            let n = self.g.mem.read(it.add(2), 1);
            let max = self.g.mem.read(d.add(40), 1);
            for _ in 0..n {
                self.glyph(glyph)
            }
            self.ink(if self.g.mem.u16(0x503E) == 2 { 0 } else { 8 });
            for _ in n..max {
                self.glyph(glyph)
            }
        }
        kind
    }
    fn weapon_line(&mut self, who: i16, slot: i16, y: i32) -> i32 {
        let Some((it, d)) = items::item(self.g, who, slot) else {
            return 0;
        };
        if items::kind(self.g, who, slot) != 2 {
            return 0;
        }
        self.at(y, 186);
        let name = self.string(d);
        self.text(&name);
        if self.g.mem.read(d.add(39), 1) & 32 != 0 {
            self.at(y, 294);
            self.shifted(self.g.mem.read(it.add(1), 1) as i32, 0x80)
        }
        1
    }
}
fn scale(v: i16, max: i16, n: i16) -> i16 {
    if v > max {
        n
    } else if max == 0 || v <= 0 {
        0
    } else {
        (v.wrapping_mul(n) as i32 / max as i32) as i16
    }
}
fn scratch(g: &mut Ground, off: u16, text: &[u8]) -> Far {
    g.mem.write_bytes(Mem::ds(off), text);
    g.mem.set_u8(off.wrapping_add(text.len() as u16), 0);
    Mem::ds(off)
}
pub fn panel(g: &mut Ground, who: i16, row: i16, layout: usize) {
    let y = (row as i32 + 1) * 8 + 76;
    let base = (who as u16).wrapping_mul(256);
    let rec = |g: &Ground, off: u16| g.mem.party_u8(base.wrapping_add(off)) as i16;
    let mut p = Paint::new(g, Raster::Planar);
    p.ly("CH", layout, y, 184);
    match layout {
        14 => {
            p.g.mem.set_pointer(
                0x3C44,
                p.g.mem.pointer(0x6D65).add(who as i32 * 256 + 0x102),
            );
            let social = rec(p.g, 0x11B);
            let title = if social >= 11 {
                p.g.mem.pointer(0x3BB0 + (social as u16 - 11) * 4)
            } else {
                Far::default()
            };
            p.g.mem.set_pointer(0x3CC2, title);
            p.labels(0x3C40, y, 184);
            p.ink(0);
            p.at(y + 20, 229);
            p.hex6(who, 0x11C, true);
            p.ink(10);
            p.at(y + 11, 211);
            p.hex6(who, 0x116, false);
            p.at(y + 20, 213);
            p.hex6(who, 0x116, true);
            p.number(((rec(p.g, 0x124) - 18) / 4) as i32, y + 95, 251);
            p.number(
                crate::ground::commands_talk::cash(p.g, who) as i32,
                y + 70,
                254,
            );
            p.number(
                p.g.mem.read(p.g.mem.pointer(0x6D65).add(0x88), 4) as i32,
                y + 77,
                254,
            );
            let pension = rec(p.g, 0x129);
            p.at(y + 102, 261);
            if pension != 0 {
                p.shifted(pension as i32 * 20, 0x80);
                p.glyph(0xAF);
                p.text(b"mo.")
            } else {
                p.text(b"none")
            }
            p.number((rec(p.g, 0x125) - rec(p.g, 0x126)) as i32, y + 36, 277);
            p.glyph(0xAF);
            p.shifted(rec(p.g, 0x126) as i32, 0x80);
            p.career(who, y + 87, 187);
            p.counted(
                who as u8,
                rec(p.g, 0x122) as u8,
                rec(p.g, 0x123) as u8,
                y + 97,
                189,
            );
            let capacity = (rec(p.g, 0x116) + rec(p.g, 0x118)) * 30;
            let mut weight = 0i16;
            for s in 0..23 {
                if let Some((_, d)) = items::item(p.g, who, s) {
                    weight = weight.wrapping_add(p.g.mem.read(d.add(31), 2) as i16)
                }
            }
            p.ink(if weight > capacity { 6 } else { 10 });
            p.number((weight / 10) as i32, y + 20, 258);
            p.glyph(b'.');
            p.shifted((weight % 10) as i32, 0x80);
            p.text(b" kg.");
            p.ink(0);
            p.number((capacity / 10) as i32, y + 26, 258);
            p.text(b".\xB0 kg.");
            if rec(p.g, 0x12A) & 1 != 0 {
                p.ly("CH", 22, 93, 268)
            }
        }
        15 => {
            let d = items::item(p.g, who, 19).map(|(_, d)| d);
            let armor = d.unwrap_or_else(|| literal(p.g, b"none"));
            p.g.mem.set_pointer(0x3DAC, armor);
            if d.is_some() {
                p.item_icon(items::icon(p.g, who, 19) as u8, y + 26, 184);
                let n = p.meter(y + 57, 211, 3, 10, rec(p.g, 0x128), 45, 99);
                p.ink(10);
                p.number(n as i32, y + 53, 187)
            }
            let (hits, split, max) = (rec(p.g, 0x127), rec(p.g, 0x126), rec(p.g, 0x125));
            let health: &[u8] = if hits == 0 {
                b"dead"
            } else if hits <= split {
                b"uncon"
            } else if hits == max {
                b"perfect"
            } else if max - (max - split) / 2 >= hits {
                b"fair"
            } else {
                b"good"
            };
            let text = literal(p.g, health);
            p.g.mem.set_pointer(0x3D9A, text);
            p.meter(y + 21, 185, 4, 10, hits, max, 125);
            let marker = scale(split, max, 125);
            p.fill(y + 21, 185 + marker as i32, 4, 2, 6);
            p.labels(0x3D84, y, 184);
            p.ink(0);
            p.number((max - split) as i32, y + 1, 222);
            p.glyph(0xAF);
            p.shifted(split as i32, 0x80);
            p.ink(10);
            p.number((hits - split).max(0) as i32, y + 10, 222);
            p.ink(0);
            p.glyph(0xAF);
            p.shifted(if hits <= split { hits } else { split } as i32, 0x80);
        }
        16 | 17 => {
            let slot = if row == 7 {
                p.g.mem.i16(0x6EFC).wrapping_sub(1)
            } else {
                row + 19
            };
            let Some((it, d)) = items::item(p.g, who, slot) else {
                p.finish();
                return;
            };
            p.item_icon(items::icon(p.g, who, slot) as u8, y + 1, 184);
            if layout == 16 {
                let kind = p.ammo(who, slot, y + 26, 186);
                let space = literal(p.g, b" ");
                if kind == 0 {
                    p.g.mem.set_pointer(0x3DEB, Far::default());
                    p.g.mem.set_pointer(0x3DF4, space);
                    p.g.mem.set_pointer(0x3E0F, space)
                } else {
                    let t = literal(p.g, if kind == 64 { b"clip" } else { b"pwr" });
                    p.g.mem.set_pointer(0x3DF4, t)
                }
                p.g.mem.set_pointer(0x3DFD, d);
                let n = if kind != 0 {
                    let text = p.g.mem.read(it.add(1), 1).to_string();
                    scratch(p.g, 0x7016, text.as_bytes())
                } else {
                    space
                };
                p.g.mem.set_pointer(0x3E06, n);
                p.labels(0x3DE7, y, 184);
                let n = literal(p.g, b"number of");
                let r = literal(p.g, b"reloads");
                p.g.mem.set_pointer(0x3DEB, n);
                p.g.mem.set_pointer(0x3E0F, r);
            } else {
                p.ink(6);
                p.at(y + 17, 187);
                let name = p.string(d);
                p.text(&name);
                let weight = p.g.mem.read(d.add(31), 2) as i16;
                p.at(y + 33, 187);
                p.text(format!("{}.{} kilograms", weight / 10, weight % 10).as_bytes());
                p.at(y + 43, 187);
                p.text(format!("{} credits", p.g.mem.read(d.add(33), 4) as i32).as_bytes());
                match items::kind(p.g, who, slot) {
                    2 => {
                        if p.ammo(who, slot, y + 25, 189) != 0 {
                            p.ink(6);
                            p.at(y + 3, 236);
                            p.text(b"reloads: ");
                            p.shifted(p.g.mem.read(it.add(1), 1) as i32, 0x80)
                        }
                    }
                    9 => {
                        p.fill(y + 27, 191, 3, 100, 8);
                        p.meter(
                            y + 27,
                            191,
                            3,
                            6,
                            p.g.mem.read(it.add(1), 1) as i16,
                            p.g.mem.read(d.add(39), 1) as i16,
                            100,
                        );
                    }
                    _ => {}
                }
            }
        }
        18 => {
            p.ink(6);
            let mut n = 0;
            n += p.weapon_line(who, 21, y + n * 6);
            n += p.weapon_line(who, 22, y + n * 6);
            for i in 0..19 {
                n += p.weapon_line(who, i, y + n * 6);
                if n > 11 {
                    break;
                }
            }
        }
        19 => {
            if let Some((it, d)) = items::item(p.g, who, 20) {
                p.g.mem.set_pointer(0x3DC7, d);
                let weight = p.g.mem.read(d.add(31), 2) as i16;
                let text = format!("{}.{} kilograms", weight / 10, weight % 10);
                let q = scratch(p.g, 0x7027, text.as_bytes());
                p.g.mem.set_pointer(0x3DD0, q);
                p.item_icon(items::icon(p.g, who, 20) as u8, y + 1, 184);
                let max = ((p.g.mem.read(d.add(39), 1) & 15) * 20 + 20) as i16;
                let n = scale(
                    p.g.mem.read(it.add(2), 1) as i16,
                    p.g.mem.read(d.add(40), 1) as i16,
                    20,
                ) + (p.g.mem.read(it.add(1), 1) * 20) as i16;
                p.meter(y + 37, 185, 4, 10, n, max, 120);
            } else {
                let name = literal(p.g, b"none");
                let weight = literal(p.g, b"0.0 kilograms");
                p.g.mem.set_pointer(0x3DC7, name);
                p.g.mem.set_pointer(0x3DD0, weight)
            }
            p.labels(0x3DC3, y, 184);
        }
        20 => {
            p.ink(10);
            for i in 0..3 {
                p.at(y + 12, 198 + i * 22);
                p.shifted(rec(p.g, 0x1CB + i as u16) as i32, 0xA0)
            }
            p.labels(0x3E1D, y, 184)
        }
        _ => {}
    }
    p.finish();
}
