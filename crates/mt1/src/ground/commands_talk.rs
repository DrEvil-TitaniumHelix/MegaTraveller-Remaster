use super::{
    commands, commands_items as items,
    input::{dialogue, menu, ui},
    render,
    state::{Far, Ground, Mem},
    walk,
};
pub fn cash(g: &Ground, who: i16) -> u32 {
    g.mem
        .read(g.mem.pointer(0x6D65).add(who as i32 * 256 + 0x1FE), 4)
}
fn set_cash(g: &mut Ground, who: i16, n: u32) {
    g.mem
        .write(g.mem.pointer(0x6D65).add(who as i32 * 256 + 0x1FE), n, 4)
}
pub fn say(g: &mut Ground, text: &[u8]) {
    ui::ink(g, 6);
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) | 1);
    render::front(g);
    ui::layout(g, 1, 54, 0);
    ui::wrapped(g, text, true);
    render::back(g);
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) & !1);
}
fn say_token(g: &mut Ground, index: i16, timed: bool) {
    if let Err(error) = dialogue::say_message(g, index, timed) {
        eprintln!("{error}")
    }
}
fn box_text(g: &mut Ground, text: &[u8], reset: bool) {
    if reset {
        g.mem.set_u8(0x6E2A, 0)
    }
    let mut line = g.mem.u8(0x6E2A) as i32;
    let width = (g.mem.i16(0x6E00) / 6) as u8 as usize;
    if width == 0 {
        return;
    }
    let mut text = text;
    for _ in 0..65536 {
        let n = text.len().min(width);
        let end = if n == width {
            (1..width).rev().find(|&i| text[i] == b' ').unwrap_or(0)
        } else {
            n
        };
        ui::ink(g, 6);
        g.mem.set_u8(0x713F, g.mem.u8(0x713F) | 1);
        ui::text(
            g,
            &text[..end],
            line * 8 + g.mem.i16(0x6E0E) as i32,
            g.mem.i16(0x6E08) as i32,
        );
        g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
        if n != width {
            break;
        }
        text = &text[(end + 1).min(text.len())..];
        line += 1;
    }
    g.mem.set_u8(0x713F, g.mem.u8(0x713F) & !1);
}
pub fn bribe_value(g: &Ground, p: Far) -> u32 {
    let mut n = g.mem.read(p, 1);
    for _ in 0..g.mem.read(p.add(1), 1) {
        n = n.wrapping_mul(10)
    }
    n
}
pub fn skill(g: &mut Ground, who: i16, target: i16, terms: &[(i16, i16, i16)]) -> bool {
    let mut total = walk::random(g, 6).wrapping_add(1);
    let base = (who as u16).wrapping_mul(256);
    for &(kind, index, divisor) in terms {
        let div = if divisor == 0 { 1 } else { divisor };
        let off = base.wrapping_add(index as u16);
        let n = match kind {
            0 => {
                let n = (div
                    .wrapping_abs()
                    .wrapping_add(g.mem.party_u8(off.wrapping_add(0x12B)) as i16)
                    .wrapping_sub(1) as i32
                    / div as i32) as i16;
                g.mem.set_party_u8(
                    off.wrapping_add(0x17B),
                    g.mem.party_u8(off.wrapping_add(0x17B)).wrapping_add(1),
                );
                n
            }
            1 => (g.mem.party_u8(off.wrapping_add(0x116)) as i32 / div as i32) as i16,
            2 => index,
            _ => 0,
        };
        total = total.wrapping_add(n);
    }
    total = total.wrapping_add((g.mem.party_u8(base.wrapping_add(0x14F)) / 2) as i16);
    g.mem.set_party_u8(
        base.wrapping_add(0x19F),
        g.mem.party_u8(base.wrapping_add(0x19F)).wrapping_add(1),
    );
    total >= target
}
fn can_take(g: &Ground, who: i16, required: i16) -> bool {
    if required == 0 {
        true
    } else if required > 0 {
        g.mem.read(
            items::object(g, g.mem.party_u16(items::inv(who, 21)) as i16),
            1,
        ) == required as u32
    } else {
        items::kind(g, who, 21) == required.wrapping_neg()
    }
}
pub fn reward(g: &mut Ground, who: i16, npc: Far, quiet: bool) {
    let required = g.mem.read(npc.add(0x18), 2) as i16;
    if !can_take(g, who, required) {
        say_token(g, g.mem.read(npc.add(0x14), 2) as i16, !quiet);
        return;
    }
    let roll = walk::random(g, 100);
    let flags = g.mem.read(npc.add(13), 1);
    if flags & 32 != 0 && roll < 30 {
        say(g, b"Sorry, you lost the race. Better luck next time!");
        items::consume(g, who, 21);
        return;
    }
    let reward = g.mem.read(npc.add(0x1A), 2) as i16;
    let mut full = false;
    let mut obj = 0;
    if reward < 0 {
        set_cash(
            g,
            who,
            cash(g, who).wrapping_add((reward.wrapping_neg() as i32 * 100) as u32),
        );
        if required != 0 {
            items::consume(g, who, 21)
        }
    } else {
        obj = walk::new_object(g, reward as i32) as i16;
        if required != 0 {
            items::consume(g, who, 21);
            g.mem.set_party_u16(items::inv(who, 21), obj as u16)
        } else if items::add(g, who, obj) == -1 {
            full = true
        }
    }
    if full {
        let text =
            b"\"You can't seem to carry anything else, or else I might give you something.\"";
        if quiet {
            say(g, text)
        } else {
            ui::message(g, text)
        }
        g.mem.write(items::object(g, obj), 255, 1);
        return;
    }
    let name = g.mem.cstr(
        g.mem
            .pointer(0x6D65)
            .add(g.mem.selected_char() as i32 * 256 + 0x102),
    );
    if quiet {
        render::front(g);
        ui::layout(g, 1, 54, 0);
        if required != 0 {
            let item = g.mem.cstr(items::definition(g, required));
            box_text(g, format!("{name} hands over the {item}.").as_bytes(), true);
            g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
        } else {
            g.mem.set_u8(0x6E2A, 0)
        }
        ui::ink(g, 6);
        if let Err(error) = dialogue::get_message(g, g.mem.read(npc.add(0x12), 2) as i16, false, 0)
        {
            eprintln!("{error}")
        }
    } else {
        say_token(g, g.mem.read(npc.add(0x12), 2) as i16, true)
    }
    if quiet {
        let text = if reward < 0 {
            format!(
                "{name} is handed {}$.",
                (reward.wrapping_neg() as i32 * 100) as u32
            )
        } else {
            format!(
                "{name} is handed the {}.",
                g.mem.cstr(items::definition(g, reward))
            )
        };
        g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
        box_text(g, text.as_bytes(), false);
    }
    render::back(g);
    if flags & 4 != 0 {
        let id = g.mem.read(npc, 2) as i16;
        let off = ((id >> 3) as u16).wrapping_add(0x3A);
        g.mem
            .set_party_u8(off, g.mem.party_u8(off) | (1 << (id & 7)));
        g.mem.write(npc.add(0x1A), 466, 2);
    } else if flags & 16 != 0 {
        g.mem.write(npc.add(0x1A), 466, 2)
    }
}
fn ask_bribe(g: &mut Ground) {
    say(
        g,
        format!(
            "You have {} credits. Bribe with {} credits?",
            g.mem.u32(0x6D26) as i32,
            g.mem.u32(0x6C2A) as i32
        )
        .as_bytes(),
    );
}
pub fn bribe_callback(g: &mut Ground, callback: u16) -> i16 {
    if callback == 0x5427 {
        if g.mem.u32(0x6D26) < 1000 {
            say(g, b"\"You don't have enough money to bribe me.\"");
            return -1;
        }
        g.mem.set_u32(0x6C2A, 1000);
        ask_bribe(g);
        return if menu::run_menu(g, Mem::ds(0x4694), 1, 0) != -1 {
            1
        } else {
            -1
        };
    }
    let mut choice = 0;
    loop {
        choice = menu::run_menu(g, Mem::ds(0x4649), 1, choice);
        if choice == -1 {
            break;
        }
        let delta = g
            .mem
            .u16(0x312u16.wrapping_add((choice as u16).wrapping_mul(2))) as u32;
        let amount = g.mem.u32(0x6C2A);
        if callback == 0x5367 {
            let next = amount.wrapping_add(delta);
            if next <= g.mem.u32(0x6D26) {
                g.mem.set_u32(0x6C2A, next)
            }
        } else if delta < amount {
            g.mem.set_u32(0x6C2A, amount - delta)
        }
        ask_bribe(g);
    }
    -1
}
fn tell(g: &mut Ground, text: &[u8]) {
    if let Some(s) = g.squad.as_mut() { s.p1.log(text.iter().map(|&b| b as char).collect()) }
    ui::message(g, text)
}
pub fn talk(g: &mut Ground) {
    let (cell, _) = commands::facing_cell(g);
    let id = g.mem.read(cell.add(2), 1) & 63;
    let who = if g.mem.combat() != 0 {
        g.mem.leader()
    } else {
        g.mem.selected_char() as i16
    };
    g.mem.set_u32(0x6D26, cash(g, who));
    dialogue::set_text_box(g, 2);
    if id == 0 {
        tell(g, b"You aren't facing anybody.");
        return;
    }
    let npc = Mem::npc(id as i16);
    if g.mem.read(npc.add(0x12), 2) == 0 {
        g.mem.write(npc.add(0x12), 86, 2)
    }
    let task = g.mem.read(npc.add(0x1A), 2) as i16;
    if (25..54).contains(&g.mem.read(npc.add(9), 1)) {
        dialogue::set_text_box(g, 3);
        if task != 466 && task > 0 {
            reward(g, who, npc, false)
        } else if task == 466 {
            ui::message(g, b"\"I have completed my task.\"")
        } else {
            say_token(g, g.mem.read(npc.add(0x12), 2) as i16, true)
        }
        return;
    }
    let mut bribe = bribe_value(g, npc.add(0x1C));
    let level = g.mem.level() as i16;
    let off = 0xFAu16.wrapping_add((level as u16).wrapping_mul(2));
    g.mem.set_u8(off, (g.mem.x() >> 1) as u8);
    g.mem.set_u8(off.wrapping_add(1), (g.mem.y() >> 1) as u8);
    render::dismiss_message(g, 1);
    ui::layout(g, 1, 54, 0);
    g.screen_kind(crate::ui::ScreenKind::Picture);
    let room = if level < 5 {
        g.mem.u8(0x5E0Fu16.wrapping_add(level as u16)) as i16
    } else {
        g.mem
            .i16(0xD2u16.wrapping_add((level as u16 - 5).wrapping_mul(2)))
    };
    render::shop_portrait(g, g.mem.read(npc.add(9), 1) as i16, room);
    g.mem.set_u8(g.mem.u16(0x52F4).wrapping_add(0xE5), 100);
    commands::window(g, 54, 0, 146, 40);
    render::flip(g);
    let n = walk::random(g, 5);
    let greeting = ui::bytes(g, g.mem.pointer(0x31C + n as u16 * 4));
    if g.mem.debug() != 0 {
        let mut text = format!("[{}] ", g.mem.read(npc, 2)).into_bytes();
        text.extend_from_slice(&greeting);
        say(g, &text)
    } else {
        say(g, &greeting)
    }
    loop {
        let choice = menu::run_menu(g, Mem::ds(0x46DF), 1, 0);
        let task = g.mem.read(npc.add(0x1A), 2) as i16;
        match choice {
            0 => {
                if task == 466 {
                    say(g, b"\"It was nice talking to you.\"")
                } else if task != 0 {
                    reward(g, who, npc, true)
                } else if bribe <= 1 {
                    say_token(g, g.mem.read(npc.add(0x12), 2) as i16, false)
                } else {
                    say(
                        g,
                        b"\"It is claimed that credits can loosen anyone's tongue.\"",
                    )
                }
            }
            1 => {
                if bribe == 0 || task != 0 {
                    say(g, b"\"No thanks, I really don't need the money.\"")
                } else {
                    let offer = g.mem.u32(0x6C2A);
                    let result = if offer < bribe {
                        -1
                    } else {
                        i16::from(skill(
                            g,
                            who,
                            g.mem.read(npc.add(12), 1) as i16,
                            &[(0, 8, 1), (2, (offer / bribe) as i16, 1)],
                        ))
                    };
                    if result > 0 {
                        say_token(g, g.mem.read(npc.add(0x12), 2) as i16, false);
                        g.mem.write(npc.add(0x1C), 0, 2);
                        bribe = 0
                    } else {
                        say(g, b"\"I laugh at your money!\"");
                        if result == -1 {
                            bribe = bribe.wrapping_sub(offer >> 1)
                        }
                        bribe = bribe.max(1)
                    }
                    let balance = cash(g, who).wrapping_sub(g.mem.u32(0x6C2A));
                    set_cash(g, who, balance);
                    g.mem.set_u32(0x6D26, balance);
                }
            }
            -1 => break,
            _ => {}
        }
    }
    g.screen_kind(crate::ui::ScreenKind::Ground);
    render::draw_menu(g, Mem::ds(0x435B));
    render::present_menu(g);
    render::dismiss_message(g, 1);
    render::walker_panel(g, true, true);
    commands::window(g, 54, 0, 112, 24);
}
