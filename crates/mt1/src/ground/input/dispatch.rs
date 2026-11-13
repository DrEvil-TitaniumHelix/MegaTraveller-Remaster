use super::menu;
use crate::ground::{commands, commands_character, commands_party, commands_talk, render, state::{Ground, Mem}, walk};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow { Continue, Quit }
pub fn m4b_note(key: u8, debug: bool) -> Option<&'static str> {
    Some(match key {
        0x10 if debug => "not yet (M4B): debug status 6923",
        _ => return None,
    })
}
pub fn dispatch(g: &mut Ground, key: u8, now: u32, _changed: i16) -> Flow {
    if let Some(note) = m4b_note(key, g.mem.debug() != 0) {
        eprintln!("{note}"); g.mem.set_key(0); return Flow::Continue;
    }
    match key {
        1 => {
            render::cursor_off(g);
            if g.mem.individual() != 0 { walk::sync_leader_pos(g) }
            else if g.mem.orders() != 0 {
                let w = Mem::walker(g.mem.leader());
                if g.mem.read(w.add(16), 2) != 0 { for off in [16,18,20] { g.mem.write(w.add(off), 0, 2) } }
                else { walk::party_order(g, 0) }
            } else if menu::run_menu(g, Mem::ds(0x45B3), 0, 0) == 0 { return Flow::Quit }
            render::cursor_on(g);
        }
        0x22 => {
            render::cursor_off(g);
            match menu::run_menu(g, Mem::ds(0x45FE), 0, 0) {
                1 => return Flow::Quit,
                2 => {
                    g.mem.set_sound_enabled(g.mem.sound_enabled() ^ 1);
                    let i = g.mem.sound_enabled() as u16 * 4;
                    g.mem.set_pointer(0x4606, g.mem.pointer(0xEA + i));
                    render::show_message(g, g.mem.pointer(0xF2 + i));
                }
                0 => {
                    g.mem.set_event(20); walk::world_event(g); render::redraw(g); g.mem.set_event(-1);
                }
                _ => {}
            }
            render::cursor_on(g);
        }
        0x2E => {
            render::cursor_off(g);
            if g.mem.combat()==0 {commands_talk::talk(g);render::redraw(g)}
        }
        0x19 if g.mem.orders()==0 => {render::cursor_off(g);commands_party::run(g);render::redraw(g)}
        0x17 if g.mem.orders()==0 => {
            render::cursor_off(g);menu::run_menu(g,Mem::ds(0x4568),0,0);g.mem.set_event_id(-1);render::redraw(g);
        }
        0x2F if g.mem.orders()==0 => {render::cursor_off(g);commands::vehicle_menu(g);render::redraw(g)}
        0x11 if g.mem.debug()!=0 => {commands::world_uwp(g);return Flow::Continue}
        0x44 if g.mem.debug()!=0 => {commands::debug_teleport(g);return Flow::Continue}
        0x43 if g.mem.debug()!=0 => commands::debug_cheat(g),
        0x3B..=0x3F => {
            render::cursor_off(g);commands_character::run(g,key as i16-0x3B);
            g.mem.set_key(0);g.mem.set_panel_flag(0);render::cursor_on(g);
        }
        0x18 => { render::cursor_off(g); walk::party_order(g, 1) }
        0x26 if g.scene.is_some() => {
            render::cursor_off(g); let line = super::super::commands_items::toggle_light(g);
            super::super::orders::log(g, line); g.mem.set_redraw_pending(1);
        }
        0x13 => {
            render::cursor_off(g); let who = g.mem.leader();
            let done = walk::try_reload(g, who);
            if g.scene.is_some() {
                let n = super::super::round::name(g, who as u8);
                let line = if done != 0 { format!("{n} reloads") } else { match super::super::orders::reload_check(g, who as u8) { Err(r) => format!("{n}: {r}"), Ok(()) => format!("{n}: nothing to reload, the chamber is loaded") } };
                super::super::orders::log(g, line); g.mem.set_redraw_pending(1);
            }
        }
        0x21 | 0x1C | 0x14 if g.mem.combat() != 0 => target(g, key, now),
        0x0F if g.mem.combat() != 0 => next_target(g, now),
        0x0F => return Flow::Continue,
        0x0D | 0x4E if g.mem.combat() != 0 => {
            render::cursor_off(g); let p = Mem::walker(g.mem.leader()).add(2); g.mem.write(p, g.mem.read(p, 1) ^ 1, 1);
            render::walker_panel(g, false, true);
        }
        2..=6 if g.mem.combat() != 0 => {
            render::cursor_off(g); let who = key as u16 - 2;
            if g.mem.party_u8((who << 8) + 0x127) > g.mem.party_u8((who << 8) + 0x126) {
                walk::set_leader(g, who as i16);
                render::select_panel(g, 10, g.mem.u8(0x111 + who));
                render::walker_panel(g, false, true);
            }
            render::redraw(g);
        }
        _ => {}
    }
    g.mem.set_key(0); Flow::Continue
}

fn target(g: &mut Ground, key: u8, now: u32) {
    let w = Mem::walker(g.mem.leader()); let mut follow = false;
    if key != 0x14 {
        render::cursor_off(g);
        let builtin = g.mem.read(w.add(3), 1) as u16;
        let ammo = g.mem.party_u8(((g.mem.leader() as u16) << 8).wrapping_add(builtin).wrapping_add(0x1CA));
        if key == 0x21 || (builtin != 0 && ammo == 0) { g.mem.write(w.add(3), 0, 1) }
        follow = g.mem.individual() == 0;
    }
    if g.mem.individual() != 0 {
        walk::sync_leader_pos(g); walk::party_fire(g, g.mem.leader() as i32);
        if g.mem.orders() == 0 { g.mem.write(w.add(3), 0, 1) }
    } else if key == 0x14 {
        let choice = menu::weapon_menu(g, Mem::ds(0x1DD), 3, g.mem.read_i(w.add(4), 1) as i16, 6, 7, Some(Mem::ds(0x1276)));
        if choice != -1 { g.mem.write(w.add(4), choice as u32, 1); g.mem.write(w.add(3), (choice+1) as u32, 1); follow = true }
    }
    if follow {
        g.mem.set_individual(1); g.mem.set_target_deadline(now.wrapping_add(9));
        if g.mem.orders() == 0 {
            g.mem.write(w.add(8), g.mem.x() as u32, 2); g.mem.write(w.add(10), g.mem.y() as u32, 2); g.mem.write(w.add(1), g.mem.heading() as u32, 1);
        } else { g.mem.write(w.add(16), g.mem.x() as u32, 2); g.mem.write(w.add(18), g.mem.y() as u32, 2) }
        walk::walker_set_pose(g, w);
        let ident = g.mem.read(w.add(5), 1) as i16;
        if ident != 0 {
            let p = Mem::npc(ident);
            if g.mem.read(p.add(2), 1) as u8 == g.mem.level() as u8 {
                g.mem.write(w.add(12), g.mem.read(p.add(32), 2) & 0xFFFE, 2);
                g.mem.write(w.add(14), g.mem.read(p.add(34), 2) & 0xFFFE, 2);
            }
        }
        g.mem.set_x(g.mem.read_i(w.add(12), 2) as i16); g.mem.set_y(g.mem.read_i(w.add(14), 2) as i16); render::redraw(g);
    }
}
fn next_target(g: &mut Ground, now: u32) {
    let w = Mem::walker(g.mem.leader());
    if g.mem.individual() == 0 {
        if g.mem.read_i(w.add(8), 2) as i16 != g.mem.x() || g.mem.read_i(w.add(10), 2) as i16 != g.mem.y() {
            g.mem.write(w.add(16), g.mem.x() as u32, 2); g.mem.write(w.add(18), g.mem.y() as u32, 2);
        }
        walk::walker_set_pose(g, w);
        g.mem.set_x(g.mem.read_i(w.add(12), 2) as i16); g.mem.set_y(g.mem.read_i(w.add(14), 2) as i16);
        g.mem.set_target_deadline(now.wrapping_add(9));
    }
    g.mem.set_individual(1);
    let mut a = (g.mem.x().wrapping_sub(g.mem.view_x()) >> 1) as i32;
    let mut b = (g.mem.y().wrapping_sub(g.mem.view_y()) >> 1) as i32;
    let stride = g.mem.map_stride() as i32;
    let origin = g.mem.pointer(0xBC).add((((g.mem.view_x() as i32 * stride + g.mem.view_y() as i32) as i16 >> 1) as i32) * 3);
    let mut cell = origin.add(a * stride * 3 + b * 3);
    for _ in 0..84 {
        cell = cell.add(3); b += 1;
        if b == 12 { b = 0; cell = cell.add((stride-12)*3); a += 1; if a == 7 { a = b; cell = cell.add(-stride*21) } }
        let occupant = g.mem.read(cell.add(2), 1) & 63;
        if occupant != 0 {
            g.mem.set_x(((a << 1) + g.mem.view_x() as i32) as i16); g.mem.set_y(((b << 1) + g.mem.view_y() as i32) as i16);
            g.mem.write(w.add(5), occupant, 1); render::redraw(g); break;
        }
    }
}
