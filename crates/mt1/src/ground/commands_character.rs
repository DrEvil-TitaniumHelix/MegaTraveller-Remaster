use super::{
    commands, commands_items as items,
    input::{self, menu, ui},
    render::{self, character as draw},
    state::{Ground, Mem},
    walk,
};

fn build_menu(g: &mut Ground, who: i16, placing: bool) {
    let held = g.mem.i16(0x6EFC).wrapping_sub(1);
    let big = items::icon(g, who, held) >= 100;
    g.mem.set_i16(0x6ECC, 0);
    g.mem.set_i16(0x6EF8, -1);
    let mut i = 0;
    while i < 19 {
        let value = g.mem.party_u16(items::inv(who, i)) as i16;
        if !((placing && value == 0) || value > 0) {
            i += 1;
            continue;
        }
        if placing && !big && items::icon(g, who, i) >= 100 {
            i += 2;
            continue;
        }
        if placing && big && !items::fits(g, who, i, true) {
            i += 1;
            continue;
        }
        let n = g.mem.u16(0x6ECC);
        if held == i {
            g.mem.set_u16(0x6EF8, n)
        }
        let icon = if value == 0 {
            0
        } else {
            items::icon(g, who, i)
        };
        g.mem.set_i16(0x6ED2 + n * 2, i);
        g.mem.set_i16(
            0x6E2E + n * 8,
            g.mem.i16(0x3BE4 + i as u16 * 4).wrapping_sub(1),
        );
        g.mem
            .set_i16(0x6E30 + n * 8, g.mem.i16(0x3BE6 + i as u16 * 4));
        g.mem.set_i16(0x6E32 + n * 8, 17);
        g.mem
            .set_i16(0x6E34 + n * 8, if icon >= 100 { 48 } else { 24 });
        if icon >= 100 {
            i += 1
        }
        g.mem.set_u16(0x6ECC, n + 1);
        i += 1;
    }
    if g.mem.i16(0x6EF8) == -1 {
        g.mem.set_i16(0x6EF8, 0)
    }
}
fn select_inventory(g: &mut Ground) -> i16 {
    menu::weapon_menu(
        g,
        Mem::ds(0x6E2E),
        g.mem.u16(0x6ECC) as usize,
        g.mem.i16(0x6EF8),
        6,
        8,
        None,
    )
}
pub fn swap(g: &mut Ground, who: i16, dst: i16) {
    let src = g.mem.i16(0x6EFC).wrapping_sub(1);
    let big = items::icon(g, who, src) >= 100;
    render::front(g);
    draw::erase_slot(g, who, src);
    let a = g.mem.party_u16(items::inv(who, dst));
    let b = g.mem.party_u16(items::inv(who, dst.wrapping_add(1)));
    g.mem
        .set_party_u16(items::inv(who, dst), g.mem.party_u16(items::inv(who, src)));
    if big {
        g.mem
            .set_party_u16(items::inv(who, dst.wrapping_add(1)), 65535)
    }
    g.mem.set_party_u16(items::inv(who, src), a);
    if big {
        g.mem.set_party_u16(items::inv(who, src.wrapping_add(1)), b)
    }
    draw::slot(g, who, dst);
    draw::slot(g, who, src);
    g.mem.set_i16(0x6EFC, 0);
    draw::hint(g, 1);
}
pub fn transfer(g: &mut Ground, who: i16, to: i16) {
    if g.mem.combat() == 0 {
        let src = g.mem.i16(0x6EFC).wrapping_sub(1);
        let icon = items::icon(g, who, src);
        let dst = items::free_slot(g, to, icon, Some(src));
        if dst != -1 {
            g.mem
                .set_party_u16(items::inv(to, dst), g.mem.party_u16(items::inv(who, src)));
            if icon >= 100 {
                g.mem
                    .set_party_u16(items::inv(to, dst.wrapping_add(1)), 65535);
                g.mem.set_party_u16(items::inv(who, src.wrapping_add(1)), 0)
            }
            g.mem.set_party_u16(items::inv(who, src), 0);
        }
    }
    g.mem.set_i16(0x6EFC, 0);
}
pub fn drop(g: &mut Ground, who: i16, slot: i16) {
    let (x, y) = if g.mem.combat() == 0 || (g.mem.leader() == who && g.mem.individual() == 0) {
        (g.mem.x(), g.mem.y())
    } else {
        let p = Mem::walker(who);
        (
            g.mem.read_i(p.add(8), 2) as i16,
            g.mem.read_i(p.add(10), 2) as i16,
        )
    };
    if g.mem.outdoors() != 0 {
        let value = g.mem.party_u16(items::inv(who, slot)) as i16;
        if (walk::place_object(g, (x >> 1) as i32, (y >> 1) as i32, value as i32) as i16) >= 0 {
            if items::icon(g, who, slot) >= 100 {
                g.mem
                    .set_party_u16(items::inv(who, slot.wrapping_add(1)), 0)
            }
            g.mem.set_party_u16(items::inv(who, slot), 0);
        } else {
            ui::message(g, b"You've littered on this planet quite enough already.")
        }
    } else {
        ui::message(g, b"Littering in the Starport area is severly prohibited!")
    }
    g.mem.set_i16(0x6EFC, 0);
}
fn pick(g: &mut Ground, who: i16, place: bool) {
    let held = g.mem.i16(0x6EFC);
    if held == 0 || place {
        build_menu(g, who, false);
        if g.mem.i16(0x6ECC) > 0 {
            let choice = select_inventory(g);
            if choice != -1 {
                let slot = g
                    .mem
                    .i16(0x6ED2u16.wrapping_add((choice as u16).wrapping_mul(2)));
                render::front(g);
                g.mem.set_i16(0x6EFC, slot.wrapping_add(1));
                draw::highlight(g, who, slot, 10);
                render::front(g);
                draw::hint(g, 0);
            } else {
                g.mem.set_i16(0x6EFC, 0)
            }
        }
    } else {
        build_menu(g, who, true);
        let choice = select_inventory(g);
        if choice != -1 {
            let slot = g
                .mem
                .i16(0x6ED2u16.wrapping_add((choice as u16).wrapping_mul(2)));
            if held.wrapping_sub(1) == slot {
                render::front(g);
                draw::highlight(g, who, slot, 0);
                draw::slot(g, who, slot);
                g.mem.set_i16(0x6EFC, 0);
                draw::hint(g, 1)
            } else {
                swap(g, who, slot)
            }
        }
    }
}
fn take_equipment(g: &mut Ground, who: i16, slot: i16) -> bool {
    let item = g.mem.party_u16(items::inv(who, slot)) as i16;
    let dst = items::add(g, who, item);
    if dst == -1 {
        return false;
    }
    render::front(g);
    draw::slot(g, who, dst);
    draw::erase_slot(g, who, slot);
    g.mem.set_party_u16(items::inv(who, slot), 0);
    true
}
pub fn equip(g: &mut Ground, who: i16, mut slot: i16, mut dst: i16) {
    build_menu(g, who, false);
    if g.mem.i16(0x6ECC) <= 0 {
        return;
    }
    if slot == -1 {
        slot = select_inventory(g)
    }
    if slot == -1 {
        return;
    }
    if dst == -1 {
        slot = g
            .mem
            .i16(0x6ED2u16.wrapping_add((slot as u16).wrapping_mul(2)))
    }
    match items::kind(g, who, slot) {
        3 => dst = 20,
        4 => dst = 19,
        _ => {
            if dst == -1 {
                dst = menu::weapon_menu(g, Mem::ds(0x3BD4), 2, 0, 6, 8, None)
            }
            if dst != -1 {
                dst = dst.wrapping_add(21)
            }
        }
    }
    if dst < 19 {
        return;
    }
    let ok = g.mem.party_u16(items::inv(who, dst)) == 0 || take_equipment(g, who, dst);
    render::front(g);
    if !ok {
        return;
    }
    draw::erase_slot(g, who, slot);
    if items::icon(g, who, slot) >= 100 {
        g.mem
            .set_party_u16(items::inv(who, slot.wrapping_add(1)), 0)
    }
    g.mem
        .set_party_u16(items::inv(who, dst), g.mem.party_u16(items::inv(who, slot)));
    g.mem.set_party_u16(items::inv(who, slot), 0);
    draw::slot(g, who, dst);
    if dst == 21 {
        draw::hint(g, 2 + u16::from(items::kind(g, who, 21) == 2))
    }
    if dst == 22 {
        draw::hint(g, 4 + u16::from(items::kind(g, who, 22) == 2))
    }
    if dst == 19 || dst == 20 {
        render::back(g);
        if dst == 19 {
            render::air_panel(g, false)
        }
        draw::hands(g, who);
        render::status_panel(g);
        render::front(g);
        render::cursor_off(g);
    }
    if g.mem.i16(0x6EFC).wrapping_sub(1) == slot {
        draw::hint(g, 1);
        g.mem.set_i16(0x6EFC, 0)
    }
}
pub fn unequip(g: &mut Ground, who: i16, mut slot: i16) {
    if slot == -1 {
        slot = menu::weapon_menu(g, Mem::ds(0x3BC4), 4, 0, 6, 8, None)
    }
    if slot == -1 {
        return;
    }
    slot = slot.wrapping_add(19);
    if !take_equipment(g, who, slot) {
        return;
    }
    if slot == 19 {
        render::back(g);
        render::air_panel(g, false);
        draw::hands(g, who);
        render::status_panel(g)
    } else if slot == 20 {
        render::back(g);
        draw::hands(g, who);
        render::status_panel(g)
    }
    render::cursor_off(g);
    render::front(g);
    if slot == 21 {
        draw::hint(g, 2)
    }
    if slot == 22 {
        draw::hint(g, 4)
    }
}
pub fn page(g: &mut Ground, who: i16, home: bool) {
    if home {
        if g.mem.i16(0x6ECE) == 0 {
            return;
        }
        render::back(g);
        g.mem.set_i16(0x6ECE, 0);
        draw::sheet(g, who, true);
        g.mem.set_i16(0x6ED0, 0);
        render::front(g);
    } else if g.mem.i16(0x6ED0) == 0 {
        let mut i = g.mem.i16(0x6ECE);
        let mut n = 0;
        while i < 80 && n < 21 {
            if g.mem.party_u8(
                (who as u16)
                    .wrapping_mul(256)
                    .wrapping_add(i as u16)
                    .wrapping_add(0x12B),
            ) != 0
            {
                n += 1
            }
            i = i.wrapping_add(1)
        }
        if i <= 80 {
            render::back(g);
            g.mem.set_i16(0x6ECE, i);
            draw::sheet(g, who, true);
            render::front(g)
        }
    }
}
pub fn run(g: &mut Ground, mut who: i16) {
    if g.mem
        .party_u8((who as u16).wrapping_mul(256).wrapping_add(0x127))
        == 0
    {
        return;
    }
    g.screen_kind(crate::ui::ScreenKind::Record);
    render::cursor_off(g);
    g.mem.set_u32(0x6ECE, 0);
    g.mem.set_u16(0x7012, 0);
    g.mem.set_u8(0x3E49, 25);
    commands::window(g, 53, 0, 147, 40);
    draw::sheet(g, who, true);
    render::front(g);
    draw::selection(g, g.mem.i16(0x3FF7), false);
    loop {
        render::front(g);
        let mut key = input::character_scan(g, who);
        if key == 0x6E {
            continue;
        }
        if (2..=9).contains(&key) {
            let selected = key as i16 - 2;
            if selected == g.mem.i16(0x3FF7) {
                key = 0x2F
            } else {
                draw::selection(g, g.mem.i16(0x3FF7), true);
                g.mem.set_i16(0x3FF7, selected);
                draw::selection(g, selected, false);
                continue;
            }
        }
        if (0x41..=0x44).contains(&key) {
            if g.mem.i16(0x6EFC) != 0 {
                equip(
                    g,
                    who,
                    g.mem.i16(0x6EFC).wrapping_sub(1),
                    (key as i16 - 0x41) % 2,
                )
            } else {
                unequip(g, who, key as i16 - 0x41)
            }
            continue;
        }
        if (0x96..=0xA8).contains(&key) {
            let mut slot = key as i16 - 0x96;
            if g.mem.party_u16(items::inv(who, slot)) == 65535 {
                slot -= 1
            }
            let held = g.mem.i16(0x6EFC);
            if held == 0 {
                if g.mem.party_u16(items::inv(who, slot)) != 0 {
                    g.mem.set_i16(0x6EFC, slot + 1);
                    draw::highlight(g, who, slot, 10);
                    draw::hint(g, 0)
                }
            } else if held.wrapping_sub(1) == slot {
                draw::highlight(g, who, held.wrapping_sub(1), 0);
                draw::slot(g, who, held.wrapping_sub(1));
                draw::hint(g, 1);
                g.mem.set_i16(0x6EFC, 0)
            } else if items::fits(
                g,
                who,
                slot,
                items::icon(g, who, held.wrapping_sub(1)) >= 100,
            ) {
                swap(g, who, slot)
            }
            continue;
        }
        match key {
            1 => break,
            0x3B..=0x3F => {
                let other = key as i16 - 0x3B;
                if g.mem.party_u8(other as u16 * 256 + 0x127) == 0 {
                    continue;
                }
                if other == who {
                    break;
                }
                if g.mem.i16(0x6EFC) != 0 {
                    transfer(g, who, other)
                }
                who = other;
                g.mem.set_u32(0x6ECE, 0);
                render::back(g);
                draw::sheet(g, who, true);
                render::front(g);
                draw::selection(g, g.mem.i16(0x3FF7), false);
            }
            0x48 | 0x50 => {
                let old = g.mem.i16(0x3FF7);
                draw::selection(g, old, true);
                let next = if key == 0x48 {
                    let n = old.wrapping_sub(1);
                    if n < 0 {
                        7
                    } else {
                        n
                    }
                } else {
                    let n = old.wrapping_add(1);
                    if n > 7 {
                        0
                    } else {
                        n
                    }
                };
                g.mem.set_i16(0x3FF7, next);
                draw::selection(g, next, false);
            }
            0x10 => unequip(g, who, -1),
            0x19 => {
                let held = g.mem.i16(0x6EFC);
                if held != 0 {
                    draw::highlight(g, who, held.wrapping_sub(1), 0);
                    draw::slot(g, who, held.wrapping_sub(1));
                    draw::hint(g, 1)
                }
                pick(g, who, true)
            }
            0x32 => {
                if g.mem.i16(0x6EFC) != 0 {
                    pick(g, who, false)
                }
            }
            0x16 => equip(g, who, -1, -1),
            0x18 => {
                walk::party_order(g, 0);
                render::cursor_off(g)
            }
            0x49 | 0x51 => {
                page(g, who, key == 0x49);
                draw::selection(g, g.mem.i16(0x3FF7), false)
            }
            0x1C | 0x2F => panel(g, who, g.mem.i16(0x3FF7)),
            _ => {}
        }
    }
    render::back(g);
    commands::clear_view(g);
    render::main_screen(g, 0);
    if g.mem.i16(0x6EFC) != 0 {
        drop(g, who, g.mem.i16(0x6EFC).wrapping_sub(1))
    }
    render::redraw(g);
    render::redraw_party(g);
    commands::window(g, 54, 0, 112, 24);
    g.mem.set_u16(0x7012, 1);
}
fn panel(g: &mut Ground, who: i16, row: i16) {
    if !(0..8).contains(&row) {
        return;
    }
    let saved = g.mem.i16(0x6EFC);
    let mut layout = [14, 15, 16, 16, 18, 19, 20, 17][row as usize];
    if row == 7 && saved == 0 {
        let start = g.mem.i16(0x6EFA);
        let mut found = None;
        for n in 0..19 {
            let i = start.wrapping_add(n).rem_euclid(19);
            let value = g.mem.party_u16(items::inv(who, i));
            if value != 0 && value != 65535 {
                found = Some(i);
                break;
            }
        }
        let Some(i) = found else { return };
        g.mem.set_i16(0x6EFA, (i + 1) % 19);
        g.mem.set_i16(0x6EFC, i + 1);
        g.mem.set_i16(0x6ECA, 1);
    }
    if (row == 2 || row == 3) && items::kind(g, who, row + 19) != 2 {
        if g.mem.party_u16(items::inv(who, row + 19)) == 0 {
            return;
        }
        layout = 17;
    }
    render::back(g);
    draw::sheet(g, who, false);
    draw::panel(g, who, row, layout);
    render::flip(g);
    g.mem.set_key(0);
    g.get_scan();
    g.mem.set_i16(0x6ECA, 0);
    g.mem.set_i16(0x6EFC, saved);
    if row == 7 && g.mem.i16(0x6EFC) != 0 {
        g.mem.set_i16(0x6EFC, 0)
    }
    draw::sheet(g, who, false);
    draw::selection(g, row, false);
    render::flip(g);
}
