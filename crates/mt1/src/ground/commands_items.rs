use super::{
    commands,
    input::{menu, ui},
    render,
    state::{Far, Ground, Mem},
    walk,
};
pub fn inv(who: i16, slot: i16) -> u16 {
    (who as u16)
        .wrapping_mul(256)
        .wrapping_add(0x1CE)
        .wrapping_add((slot as u16).wrapping_mul(2))
}
pub fn object(g: &Ground, index: i16) -> Far {
    g.mem.pointer(0x6D65).add(0xA82 + 5 * (index as i32 - 1))
}
pub fn definition(g: &Ground, id: i16) -> Far {
    g.mem.pointer(0x6D6D).add(id as i32 * 46)
}
pub fn item(g: &Ground, who: i16, slot: i16) -> Option<(Far, Far)> {
    let id = g.mem.party_u16(inv(who, slot)) as i16;
    if id <= 0 {
        None
    } else {
        let p = object(g, id);
        Some((p, definition(g, g.mem.read(p, 1) as i16)))
    }
}
pub fn kind(g: &Ground, who: i16, slot: i16) -> i16 {
    item(g, who, slot).map_or(-1, |(_, d)| g.mem.read(d.add(0x1D), 2) as i16)
}
pub fn icon(g: &Ground, who: i16, slot: i16) -> i16 {
    item(g, who, slot).map_or(-1, |(_, d)| g.mem.read(d.add(0x26), 1) as i16)
}
pub fn fits(g: &Ground, who: i16, slot: i16, big: bool) -> bool {
    let empty = |s| g.mem.party_u16(inv(who, s)) == 0;
    if big {
        g.mem.u32(0x4D16) & 1u32.wrapping_shl(slot as u32) != 0
            && ((!empty(slot) && icon(g, who, slot) >= 100)
                || (empty(slot) && empty(slot.wrapping_add(1))))
    } else {
        empty(slot) || icon(g, who, slot) < 100
    }
}
pub fn free_slot(g: &Ground, who: i16, id: i16, preferred: Option<i16>) -> i16 {
    let ok = |s| fits(g, who, s, id >= 100) && g.mem.party_u16(inv(who, s)) == 0;
    if let Some(s) = preferred {
        if ok(s) {
            return s;
        }
    }
    (0..19).find(|&s| ok(s)).unwrap_or(-1)
}
pub fn add(g: &mut Ground, who: i16, index: i16) -> i16 {
    let p = object(g, index);
    let d = definition(g, g.mem.read(p, 1) as i16);
    let slot = free_slot(g, who, g.mem.read(d.add(0x26), 1) as i16, None);
    if slot != -1 {
        g.mem.set_party_u16(inv(who, slot), index as u16);
        if icon(g, who, slot) >= 100 {
            g.mem.set_party_u16(inv(who, slot + 1), 65535)
        }
    }
    slot
}
pub fn remove(g: &mut Ground, who: i16, slot: i16) {
    if (g.mem.party_u16(inv(who, slot)) as i16) > 0 {
        if icon(g, who, slot) >= 100 && slot < 19 {
            g.mem.set_party_u16(inv(who, slot.wrapping_add(1)), 0)
        }
        g.mem.set_party_u16(inv(who, slot), 0);
    }
}
pub fn consume(g: &mut Ground, who: i16, slot: i16) {
    let p = object(g, g.mem.party_u16(inv(who, slot)) as i16);
    g.mem.write(p, 255, 1);
    g.mem.set_party_u16(inv(who, slot), 0);
}
pub fn prompt(g: &mut Ground, text: &[u8], field: u16) {
    let p = Far::new(0, 0x8801);
    g.mem.alloc(p.seg, text.len().min(65534) + 1);
    g.mem.write_bytes(p, &text[..text.len().min(65534)]);
    g.mem.set_pointer(field, p);
}
fn choose_person(g: &mut Ground, text: &[u8]) -> i16 {
    prompt(g, text, 0x4296);
    let who = if g.mem.combat() != 0 {
        g.mem.leader()
    } else {
        menu::run_menu(g, Mem::ds(0x427A), 1, 0)
    };
    g.mem.set_pointer(0x4296, Far::default());
    who
}
pub fn take(g: &mut Ground) -> i16 {
    let p = commands::cell(g, g.mem.x() >> 1, g.mem.y() >> 1);
    let index = g.mem.read(p, 1) >> 1;
    if index == 0 {
        ui::message(g, b"There is nothing to take nearby.");
        return 0;
    }
    let obj = g.mem.pointer(0x6B6E).add(index as i32 * 9);
    let instance = object(g, g.mem.read(obj, 2) as i16);
    let name = ui::bytes(g, definition(g, g.mem.read(instance, 1) as i16));
    let mut text = b"Who will take the ".to_vec();
    text.extend_from_slice(&name);
    text.push(b'?');
    let who = choose_person(g, &text);
    if who == -1 {
        return -1;
    }
    if g.mem
        .party_u8((who as u16).wrapping_mul(256).wrapping_add(0x127))
        == 0
    {
        ui::message(g, b"That person is dead.")
    } else if add(g, who, g.mem.read(obj, 2) as i16) != -1 {
        let flag = g.mem.read(obj.add(8), 1) as u8;
        if flag != 0 {
            let bit = flag - 1;
            let off = 0x6C + (bit >> 3) as u16;
            g.mem
                .set_party_u8(off, g.mem.party_u8(off) | (1 << (bit & 7)));
            g.mem.write(obj.add(8), 0, 1);
        }
        g.mem.write(obj, 0, 2);
        g.mem.write(
            p,
            (g.mem.read(obj.add(2), 1) << 1) | (g.mem.read(p, 1) & 1),
            1,
        );
        let mut text = name;
        text.extend_from_slice(b" taken.");
        ui::message(g, &text);
    } else {
        ui::message(g, b"There is no room for that item.")
    }
    0
}
pub fn use_item(g: &mut Ground, user: i16, slot: i16, target: i16, kind: i16) {
    let Some((it, def)) = item(g, user, slot.wrapping_add(21)) else {
        return;
    };
    let rec = (target as u16).wrapping_mul(256).wrapping_add(0x102);
    match kind {
        12 => {
            let (p, y) = commands::facing_cell(g);
            if g.mem.read(p.add(1), 1) > 235 && g.mem.interior_kind() == b'I' {
                walk::play_sound(g, 6);
                for _ in 0..10 {
                    g.mem.set_view_y(g.mem.view_y().wrapping_add(1));
                    render::redraw(g);
                    g.mem.set_view_y(g.mem.view_y().wrapping_sub(1));
                    render::redraw(g);
                }
                if y & 3 == 3 {
                    g.mem.write(p.add(1), 37, 1);
                    g.mem.write(p, g.mem.read(p, 1) & 254, 1);
                    g.mem.set_event_id(-1);
                    ui::message(
                        g,
                        if g.mem.u8(0x5E1D) as u32 & 1u32.wrapping_shl(g.mem.level() as u32) != 0 {
                            b"Breached."
                        } else {
                            b"Good job. You have just blasted open an unlocked door."
                        },
                    );
                } else {
                    ui::message(g, b"That door refused to yield.")
                }
                consume(g, user, slot.wrapping_add(21));
            } else {
                ui::message(
                    g,
                    b"These charges are specially designed to breach closed and locked doors.",
                )
            }
        }
        13 => {
            if g.mem.party_u8(rec.wrapping_add(0x26)) == 45 {
                ui::message(g, b"No oxygen required.")
            } else {
                g.mem.set_party_u8(rec.wrapping_add(0x26), 45);
                consume(g, user, slot.wrapping_add(21));
                render::air_panel(g, true);
                ui::message(g, b"Oxygen supply replentished.");
            }
        }
        9 => {
            let need = g.mem.party_u8(rec.wrapping_add(0x23)) as i16
                - g.mem.party_u8(rec.wrapping_add(0x25)) as i16;
            if need == 0 {
                ui::message(g, b"No healing required.");
                return;
            }
            if g.mem.read(it.add(1), 1) == 0 {
                ui::message(g, b"Medkit all used up.");
                return;
            }
            if g.mem.party_u8(rec.wrapping_add(0x25)) == 0 {
                ui::message(g, b"It is too late for that traveller.");
                return;
            }
            g.mem.set_party_u8(
                rec.wrapping_add(0x25),
                g.mem
                    .party_u8(rec.wrapping_add(0x25))
                    .wrapping_add(need as u8),
            );
            let spend = (need
                - g.mem
                    .party_u8((user as u16).wrapping_mul(256).wrapping_add(0x156))
                    as i16)
                .max(1)
                .min(g.mem.read(it.add(1), 1) as i16);
            g.mem.write(
                it.add(1),
                g.mem.read(it.add(1), 1).wrapping_sub(spend as u32),
                1,
            );
            render::character::health_bar(g, target);
            render::status_panel(g);
            ui::message(g, b"Healed.");
            if g.mem.read(it.add(1), 1) == 0 {
                consume(g, user, slot.wrapping_add(21))
            }
        }
        11 => {
            let on = g.mem.read(it.add(2), 1) != 0;
            ui::message(
                g,
                if on {
                    b"Light turned off."
                } else {
                    b"Light turned on."
                },
            );
            g.mem.write(it.add(2), u32::from(!on), 1);
            g.mem.set_party_u16(
                0xF82,
                g.mem
                    .party_u16(0xF82)
                    .wrapping_add(if on { u16::MAX } else { 1 }),
            );
        }
        7 => {
            let status = rec.wrapping_add(0x28);
            match g.mem.read(def.add(0x27), 1) {
                3 => {
                    let need = g.mem.party_u8(rec.wrapping_add(0x23)) as i16
                        - g.mem.party_u8(rec.wrapping_add(0x25)) as i16;
                    if need == 0 {
                        ui::message(g, b"No healing required.");
                        return;
                    }
                    if g.mem.party_u8(rec.wrapping_add(0x25)) == 0 {
                        ui::message(g, b"It is too late for that traveller.");
                        return;
                    }
                    g.mem.set_party_u8(
                        rec.wrapping_add(0x25),
                        g.mem
                            .party_u8(rec.wrapping_add(0x25))
                            .wrapping_add(need as u8),
                    );
                    render::character::health_bar(g, target);
                    render::status_panel(g);
                    ui::message(g, b"Speed Healed.");
                }
                2 => {
                    g.mem.set_party_u8(status, g.mem.party_u8(status) | 240);
                    ui::message(g, b"Combat drug administered.")
                }
                0 => {
                    g.mem.set_party_u8(status, g.mem.party_u8(status) | 4);
                    ui::message(g, b"Fast drug administered.")
                }
                1 => {
                    if g.mem.party_u8(status) & 4 != 0 {
                        g.mem.set_party_u8(status, g.mem.party_u8(status) & 251);
                        ui::message(g, b"Fast drug effects eliminated.")
                    } else {
                        ui::message(g, b"No effect.")
                    }
                }
                _ => return,
            }
            consume(g, user, slot.wrapping_add(21));
        }
        _ => {}
    }
}
pub fn toggle_light(g: &mut Ground) -> String {
    for who in 0..5i16 {
        if g.mem.party_u8(0x127 + 256 * who as u16) == 0 { continue }
        for slot in 0..=22i16 {
            if kind(g, who, slot) != 11 { continue }
            let Some((it, def)) = item(g, who, slot) else { continue };
            let (on, name) = (g.mem.read(it.add(2), 1) != 0, g.mem.cstr(def));
            use_item(g, who, slot.wrapping_sub(21), who, 11);
            let n = super::round::name(g, who as u8);
            return format!("Light turned {} ({n}'s {name})", if on { "off" } else { "on" });
        }
    }
    "Nobody carries a light".into()
}
pub fn use_command(g: &mut Ground) -> i16 {
    let who = if g.mem.combat() != 0 {
        g.mem.leader()
    } else {
        choose_person(g, b"Who will use an item?")
    };
    g.mem.set_pointer(0x4296, Far::default());
    let mut message: Option<&[u8]> = None;
    if who != -1 {
        let first = (g.mem.party_u16(inv(who, 21)) as i16) > 0;
        let second = (g.mem.party_u16(inv(who, 22)) as i16) > 0;
        let two = first && second;
        let which = if two {
            let left = item(g, who, 21)
                .map(|(_, d)| g.mem.cstr(d))
                .unwrap_or_default();
            let right = item(g, who, 22)
                .map(|(_, d)| g.mem.cstr(d))
                .unwrap_or_default();
            prompt(
                g,
                format!("Primary: {left}; Second: {right}. Use which?").as_bytes(),
                0x440D,
            );
            menu::run_menu(g, Mem::ds(0x43F1), 1, 0)
        } else if first {
            0
        } else if second {
            1
        } else {
            message = Some(b"Nothing to use.");
            -1
        };
        if which != -1 {
            let kind = kind(g, who, which.wrapping_add(21));
            if matches!(kind, 11 | 12 | 9 | 7 | 13) {
                let target = if matches!(kind, 9 | 7 | 13) {
                    let name = item(g, who, which.wrapping_add(21))
                        .map(|(_, d)| g.mem.cstr(d))
                        .unwrap_or_default();
                    choose_person(g, format!("Who will the {name} be used on?").as_bytes())
                } else {
                    g.mem.leader()
                };
                if target != -1 {
                    use_item(g, who, which, target, kind);
                    return 0;
                }
            } else {
                message = Some(if two {
                    b"You can't use that."
                } else {
                    b"Nothing to use."
                })
            }
        }
    }
    if let Some(text) = message {
        ui::message(g, text)
    } else {
        render::dismiss_message(g, 0)
    }
    -1
}
