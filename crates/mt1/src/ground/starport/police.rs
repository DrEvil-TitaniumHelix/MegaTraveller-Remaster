use crate::ground::{
    commands, commands_items as items, commands_talk as talk,
    input::{menu, ui}, render, state::{Ground, Mem}, walk,
};

fn restore_field(g: &mut Ground, dismiss: bool) {
    render::draw_menu(g, Mem::ds(0x435B));
    render::present_menu(g);
    render::walker_panel(g, true, true);
    if dismiss { render::dismiss_message(g, 1) }
    commands::window(g, 54, 0, 112, 24);
    g.screen_kind(crate::ui::ScreenKind::Ground);
}

pub fn confiscate_weapons(g: &mut Ground) {
    let mut any = false;
    for who in 0..5 {
        for slot in 0..23 {
            let Some((_, def)) = items::item(g, who, slot) else { continue };
            let law = g.mem.u8(0x5E1A);
            if g.mem.read(def.add(0x1D), 2) != 2
                || g.mem.read(def.add(0x2B), 1) > law as u32
                || g.mem.ship_count() >= 50 { continue }
            if slot == 22 {
                let hands = i16::from(items::icon(g, who, slot) >= 100);
                if talk::skill(g, who, 5, &[(0,59,1), (0,35,1), (1,3,5),
                    (2,-i16::from(law >= 8),1), (2,hands * -3,1)]) { continue }
            }
            let off = 0x6C43 + g.mem.ship_count() as u16 * 4;
            g.mem.set_ship_count(g.mem.ship_count().wrapping_add(1));
            g.mem.set_u16(off, g.mem.party_u16(items::inv(who, slot)));
            g.mem.set_i16(off + 2, who);
            items::remove(g, who, slot);
            any = true;
        }
    }
    ui::message(g, if any {
        b"\"Illegal weapons confiscated. You may claim them upon return to the starport.\""
    } else { b"\"No illegal weapons found. Thank you for your time.\"" });
}

pub fn weapons_search(g: &mut Ground) -> i16 {
    let mut ok = 1;
    if g.mem.outdoors() == 0 {
        render::cursor_off(g);
        render::dismiss_message(g, 1);
        g.mem.set_outdoors(1);
        if g.mem.u8(0x5E1A) != 0 {
            g.screen_kind(crate::ui::ScreenKind::Picture);
            render::shop_portrait(g, 14, g.mem.u8(0x5E0F) as i16);
            commands::window(g, 54, 0, 146, 40);
            render::present(g, 68, 25, 69, 14);
            ui::message(g, b"\"Will you submit to a search for illegal weapons?\"");
            let answer = menu::run_menu(g, Mem::ds(0x443C), 1, 0);
            if answer == 0 { confiscate_weapons(g) }
            else if g.mem.u8(0x5E1A) < 8 && answer != -1 {
                g.mem.set_provoked(1);
                ui::message(g, b"\"Then you are in violation. Police may detain you at any time.\"");
            } else {
                ok = 0;
                ui::message(g, if answer == -1 { b"\"You remain in the Starport area.\"" }
                    else { b"\"Then you must remain in the Starport area.\"" });
                let off = 0xFAu16.wrapping_add((g.mem.level() as u16).wrapping_mul(2));
                let x = (g.mem.x() >> 1) as u8;
                let y = (g.mem.y() >> 1) as u8;
                g.mem.set_u8(off, x); g.mem.set_u8(off.wrapping_add(1), y);
                walk::set_player_pos(g, x as i16, y as i16);
                let heading = g.mem.heading().wrapping_add(4);
                g.mem.set_heading(if heading > 7 { heading.wrapping_sub(8) } else { heading });
                g.mem.set_outdoors(0);
            }
            restore_field(g, false);
        }
        render::cursor_on(g);
    }
    g.mem.set_key(0);
    ok
}

pub fn encounter(g: &mut Ground) {
    let mut bribed = false;
    if g.mem.outdoors() != 0 && g.mem.provoked() >= 2
        && g.mem.read(g.mem.pointer(0x5E02).add(6), 1) >= 8 {
        bribed = talk::skill(g, g.mem.selected_char() as i16, 7,
            &[(0,59,1), (0,61,1), (0,64,1)]);
        if !bribed {
            let mut total = 0u32;
            let mut release = None;
            for who in 0..5i16 {
                if g.mem.party_u8(who as u16 * 256 + 0x127) == 0 { continue }
                total = total.wrapping_add(talk::cash(g, who));
                if release.is_none() {
                    for slot in 0..23 {
                        let index = g.mem.party_u16(items::inv(who, slot)) as i16;
                        if index > 0 && g.mem.read(items::object(g, index), 1) == 57 {
                            release = Some((who, slot));
                        }
                    }
                }
            }
            let ship = g.mem.pointer(0x6D65).add(0x88);
            total = total.wrapping_add(g.mem.read(ship, 4));
            let mut bail = (g.mem.u8(0x5E1A) as u16).wrapping_sub(8).wrapping_mul(4000).wrapping_add(10000);
            if bail as u32 > total { bail = total as u16 }
            render::cursor_off(g); render::dismiss_message(g, 1);
            ui::layout(g, 1, 54, 0);
            g.screen_kind(crate::ui::ScreenKind::Picture);
            render::shop_portrait(g, 14, g.mem.u8(0x5E0F) as i16);
            commands::window(g, 54, 0, 146, 40); render::flip(g);
            let answer = if release.is_some() {
                talk::say(g, format!("\"Due to crimes committed on this world, you have been arrested. You may use an imperial release, or pay your bail. Bail is set at {}.\"", super::money(g, bail)).as_bytes());
                menu::run_menu(g, Mem::ds(0x4487), 1, 0)
            } else {
                talk::say(g, format!("\"Due to crimes committed on this world, you have been arrested. Manditory bail is set at {}.\"", super::money(g, bail)).as_bytes());
                walk::wait_ticks(g, 50); -1
            };
            if answer == -1 {
                let funds = g.mem.read(ship, 4);
                if funds < bail as u32 { bail = bail.wrapping_sub(funds as u16); g.mem.write(ship, 0, 4) }
                else { g.mem.write(ship, funds - bail as u32, 4); bail = 0 }
                if bail != 0 {
                    for _ in 0..5 {
                        if g.mem.party_u8(0x627) == 0 { continue }
                        let p = g.mem.pointer(0x6D65).add(0x6FE);
                        let funds = g.mem.read(p, 4);
                        if funds < bail as u32 { bail = bail.wrapping_sub(funds as u16); g.mem.write(p, 0, 4) }
                        else { g.mem.write(p, funds - bail as u32, 4); break }
                    }
                }
                if g.mem.ship_count() != 0 {
                    g.mem.set_ship_count(0);
                    talk::say(g, b"\"Bail paid. Your illegal weapons will not be returned to you.\"");
                } else { talk::say(g, b"\"Bail paid. Please try to be better in the future.\"") }
                walk::wait_ticks(g, 40);
            } else if let Some((who, slot)) = release { items::consume(g, who, slot) }
            g.mem.set_provoked(0);
        }
        ui::layout(g, 1, 54, 0); restore_field(g, true);
    }
    if g.mem.outdoors() != 0 && g.mem.ship_count() != 0 {
        render::cursor_off(g); render::dismiss_message(g, 1);
        g.screen_kind(crate::ui::ScreenKind::Picture);
        render::shop_portrait(g, 14, g.mem.u8(0x5E0F) as i16);
        commands::window(g, 54, 0, 146, 40); render::present(g, 68, 25, 69, 14);
        ui::message(g, b"Do you wish to claim your weapons?");
        if menu::run_menu(g, Mem::ds(0x43A6), 1, 0) == 0 {
            let mut all_ok = true;
            for i in 0..g.mem.ship_count() as u16 {
                let off = 0x6C43 + i * 4;
                let obj = g.mem.i16(off); let who = g.mem.i16(off + 2);
                if obj <= 0 { continue }
                if g.mem.party_u8((who as u16).wrapping_mul(256).wrapping_add(0x127)) != 0
                    && items::add(g, who, obj) == -1 { all_ok = false }
                else { g.mem.set_u16(off, 0) }
            }
            if all_ok {
                g.mem.set_ship_count(0); ui::message(g, b"\"All of your weapons have been returned.\"");
            } else { ui::message(g, b"\"Some weapons remain confiscated as your hands are full.\"") }
        } else { ui::message(g, b"\"We will continue to hold them for you.\"") }
        restore_field(g, false); render::cursor_on(g);
    }
    g.mem.set_outdoors(0);
    if bribed {
        walk::wait_ticks(g, 18);
        let name = g.mem.cstr(g.mem.pointer(0x6D65).add(g.mem.selected_char() as i32 * 256 + 0x102));
        ui::message(g, format!("{name} says, \"I told you I could get us past that guard.\"").as_bytes());
    }
}
