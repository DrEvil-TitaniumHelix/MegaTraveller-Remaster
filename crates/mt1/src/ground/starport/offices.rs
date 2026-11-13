use super::shop_menu as shop;
use crate::ground::{
    input::{self, dialogue, menu, ui},
    render,
    state::{Ground, Mem},
    walk,
};

pub fn vehicle_refund(row_distance: i16, col_distance: i16, condition: u8) -> u32 {
    if row_distance.unsigned_abs() > 3 || col_distance.unsigned_abs() > 3 {
        0
    } else if condition == 100 {
        2000
    } else {
        ((condition as i16 / 10).wrapping_mul(2000) / 10) as i32 as u32
    }
}

pub fn vehicle_return(g: &mut Ground) {
    ui::help(g, b"\"Welcome to Zenaca Vehicles. Can I help you?\"");
    loop {
        match menu::run_menu_with(g, Mem::ds(0x47C0), 1, 0, &mut |g, callback, choice| {
            if callback == 0x3A05 { rent_vehicle(g, choice); 0 } else { choice }
        }) {
            -1 => break,
            1 => {
                if g.mem.u8(0x6D31) == 0 {
                    ui::help(g, b"\"You have not rented a vehicle.\"");
                    continue;
                }
                let row = g.mem.u8(0x6419) as i16;
                let col = g.mem.u8(0x63F4) as i16;
                walk::release_cell(g, row * 2, col * 2);
                g.mem.set_u8(0x6D31, 0);
                g.mem.set_event_id(-1);
                let dr = (g.mem.previous_x() >> 1) - row;
                let dc = (g.mem.previous_y() >> 1) - col;
                if dr.unsigned_abs() > 3 || dc.unsigned_abs() > 3 {
                    ui::help(g, b"\"You parked the vehicle too far away. You lose your deposit.\"");
                } else {
                    ui::help(g, if g.mem.vehicle_condition() == 100 {
                        b"\"Thank you for the return. Your account has been credited with your deposit.\""
                    } else {
                        b"\"You damaged the vehicle. Your account will recieve part of your deposit.\""
                    });
                    let credit = vehicle_refund(dr, dc, g.mem.vehicle_condition());
                    shop::set_cash(g, -2, shop::cash(g, -2).wrapping_add(credit));
                }
                acknowledge_reply(g);
            }
            _ => {}
        }
    }
}

fn pay_from(g: &mut Ground, amount: u32) -> i16 {
    for i in 0..6 { g.mem.set_u16(0x42B9 + i * 2, 0xADAA); }
    let choice = menu::run_menu(g, Mem::ds(0x4310), 1, 0);
    let payer = match choice { 0 => 6, 1 => g.mem.i16(0x6EFE), _ => -1 };
    for i in 0..6 { g.mem.set_u16(0x42B9 + i * 2, 0); }
    if payer == -1 { return -1; }
    let account = if payer == 6 { -2 } else { payer };
    let money = shop::cash(g, account);
    if money < amount { -2 } else {
        shop::set_cash(g, account, money.wrapping_sub(amount));
        payer
    }
}

fn rent_vehicle(g: &mut Ground, selected: i16) {
    if g.mem.u8(0x6D31) != 0 {
        ui::help(g, b"\"You must return your last vehicle first.\"");
        return;
    }
    let vehicle = selected.wrapping_add(1);
    if !(1..=3).contains(&vehicle) || g.mem.u8(0x5E1C) & (1 << (vehicle - 1)) == 0 {
        ui::help(g, b"\"Sorry, we are all out of those today.\"");
        return;
    }
    let amount = g.mem.i16(0x10A + 2 * (vehicle as u16 - 1)).wrapping_add(2000) as i32 as u32;
    match pay_from(g, amount) {
        -2 => { ui::help(g, b"\"You don't have enough money.\""); return; }
        -1 => { ui::help(g, b"\"May I help you?\""); return; }
        _ => {}
    }
    if g.mem.u8(0x6D30) != vehicle as u8 {
        g.mem.set_u8(0x6D30, vehicle as u8);
        g.mem.set_u8(0x2E8, vehicle as u8);
        render::front(g); ui::layout(g, 1, 54, 0);
        render::back(g); ui::layout(g, 1, 54, 0);
        if !render::rental_picture(g, vehicle as u8) {
            eprintln!("ground: unable to load rental picture from c.dat");
            std::process::exit(3);
        }
    }
    g.mem.set_event_id(-1);
    let row = (g.mem.x().wrapping_add(4) >> 1) as u8;
    let col = (g.mem.y() >> 1) as u8;
    g.mem.set_u8(0x6419, row);
    g.mem.set_u8(0x63F4, col);
    let cell = crate::ground::commands::cell(g, row as i16, col as i16).add(2);
    let flags = g.mem.read(cell, 1) as u8;
    if flags & 0x3F == 0 {
        g.mem.write(cell, (flags | 0x40) as u32, 1);
        ui::help(g, b"\"Your vehicle is now parked outside. Remember to return it to this location undamaged or you will lose your security deposit.\"");
        g.mem.set_u8(0x6D31, 1);
        g.mem.set_vehicle_condition(100);
        acknowledge_reply(g);
    } else {
        ui::help(g, b"\"You can't rent that vehicle at this time. Please try back later.\"");
    }
}

pub fn imperial_refusal(g: &mut Ground) {
    ui::help(g, b"\"You are not permitted to enter this official imperial base. Please leave.\"");
    input::wait_scan(g);
}

fn message(g: &mut Ground, index: i16, reset: bool, mode: i16) -> Option<Vec<u8>> {
    match dialogue::get_message(g, index, reset, mode) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("ground: {error}");
            std::process::exit(3)
        }
    }
}

pub fn library(g: &mut Ground, first: i16, last: i16) {
    g.mem.set_u16(0x7012, 0);
    shop::set_box(g, 2);
    shop::limits(g, g.mem.i16(0x6E08) + 10, g.mem.i16(0x6E08) + 100);
    let count = last.wrapping_sub(first).clamp(0, 5) as usize;
    let mut names = Vec::with_capacity(count * 2);
    let mut prices = Vec::with_capacity(count * 2);
    for i in 0..count {
        let text = message(g, first.wrapping_add(i as i16), false, 1).unwrap_or_default();
        names.push(format!(" {} ", super::money(g, 2000)));
        names.push(ui::string(&text));
        prices.extend([2000, 0]);
    }
    shop::clear(g);
    let mut chosen = vec![0; count * 2];
    let (payer, mut money) = loop {
        let payer = shop::choose_payer(g, "Who will the pay to rent material?", 0, 0);
        if payer == -1 {
            shop::clear(g);
            g.mem.set_u16(0x7012, 1);
            return;
        }
        let money = shop::cash(g, payer);
        if shop::run(g, shop::Menu {
            title: "The following information is available:", names: &names,
            prices: &prices, quantities: &mut chosen, limits: &vec![1; count * 2],
            rows: count * 2, select_only: 0, money, max_load: 0,
            weights: None, kind: 0x62, sign: 1, colors: None,
        }) != 0 { break (payer, money); }
    };
    for i in 0..count {
        if chosen[i * 2] != 0 {
            shop::clear(g);
            ui::ink(g, 6);
            message(g, first.wrapping_add(i as i16), true, 0);
            shop::say(g, b"^<hit a key>", false, 6);
            input::menu_getkey(g);
            money = money.wrapping_sub(prices[i * 2]);
        }
    }
    shop::set_cash(g, payer, money);
    shop::clear(g);
    g.mem.set_u16(0x7012, 1);
}

pub fn tas(g: &mut Ground) {
    let name = g.mem.cstr(g.mem.pointer(0x15E1 + 4 * g.mem.u8(0x5E20) as u16));
    let uwp: [u8; 8] = std::array::from_fn(|i| g.mem.u8(0x5E14 + i as u16));
    let msg = g.mem.u8(0x6D12) as i16;
    shop::set_box(g, 2);
    shop::limits(g, g.mem.i16(0x6E08) + 10, g.mem.i16(0x6E08) + 80);
    shop::clear(g);
    let member_flags = (g.mem.party_u8(1) as u16 * 256).wrapping_add(0x12A);
    if g.mem.party_u8(member_flags) & 1 == 0 {
        shop::say(g, b"We're so sorry... but the Traveller's Aide Society is an EXCLUSIVE club for members only.", true, 10);
        input::menu_getkey(g);
        return;
    }
    g.mem.set_u16(0x7012, 0);
    shop::say(g, format!("Welcome to {name} TAS").as_bytes(), true, 6);
    g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(2));
    let size = match uwp[1] { 0 => "very small", 1..=4 => "small", 5..=7 => "medium", _ => "large" };
    let atmos = match uwp[2] { 0..=3 => "vacuum", 4..=5 => "thin", 6..=7 => "standard", 8..=9 => "dense", _ => "exotic" };
    let hydro = match uwp[3] { 0 => "desert", 1..=2 => "dry", 3..=9 => "wet", _ => "water" };
    let popul = match uwp[4] { 0..=3 => "low", 4..=8 => "mod", _ => "high" };
    let gov = g.mem.cstr(g.mem.pointer(0x2056 + 4 * uwp[5] as u16));
    let law = match uwp[6] { 0 => "no law", 1..=3 => "low law", 4..=7 => "moderate law", 8..=9 => "high law", _ => "extreme law" };
    let tech = match uwp[7] { 0..=3 => "pre-industrial", 4..=5 => "industrial", 6..=8 => "pre-stellar", 9..=10 => "early stellar", 11..=13 => "average stellar", _ => "high stellar" };
    for (label, word) in [("size", size), ("atmos.", atmos), ("hydro.", hydro), ("popul.", popul), ("gov.", gov.as_str()), ("law", law), ("tech", tech)] {
        shop::say(g, format!("^{label}:^{word}").as_bytes(), false, 10);
    }
    shop::say(g, if uwp[0] != 0 { b"^Starport:^excellent" } else { b"^Starport:^good" }, false, 10);
    g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(3));
    shop::say(g, b"Press a key to continue...", false, 6);
    input::menu_getkey(g);
    shop::clear(g);
    g.mem.set_u8(0x6E2A, 0);
    message(g, msg, false, 0);
    input::menu_getkey(g);
    shop::clear(g);
    g.mem.set_u16(0x7012, 1);
}

fn acknowledge_reply(g: &mut Ground) -> bool {
    if g.scene.is_some() && g.lockstep.is_none() && !g.harness {
        g.screen_kind(crate::ui::ScreenKind::Picture);
        ui::frame(g, "office_reply");
        input::wait_scan(g);
        true
    } else { false }
}

fn wait_reply(g: &mut Ground, ticks: i16) {
    if !acknowledge_reply(g) { walk::wait_ticks(g, ticks); }
}

pub fn hospital(g: &mut Ground) {
    let mut visited = false;
    g.mem.set_u16(0x7012, 0);
    let pct = 100i16.wrapping_sub(shop::skill_bonus(g));
    let mut payer = -1;
    shop::set_box(g, 2);
    loop {
        let mut names = Vec::new();
        let mut who = Vec::new();
        let mut prices = Vec::new();
        for i in 0..5u16 {
            let hp = g.mem.party_u8(i * 256 + 0x127);
            let max = g.mem.party_u8(i * 256 + 0x125);
            if hp > 0 && hp < max {
                let price = shop::price((max - hp) as u32 * 200, pct);
                names.push(format!("{}^{}", shop::name(g, i as i16), super::money(g, price as i32)));
                who.push(i as u8 + 1);
                prices.push(price);
            }
        }
        if who.is_empty() {
            if !visited {
                shop::clear(g);
                shop::say(g, b"Please don't disturb us! We have patients to take care of!", true, 6);
                wait_reply(g, 0x24);
            }
            g.mem.set_u16(0x7012, 1);
            return;
        }
        visited = true;
        if payer == -1 {
            payer = shop::choose_payer(g, "Who will pay the doctor's bills?", 0, 0);
            if payer == -1 { shop::clear(g); return; }
        }
        let n = who.len();
        let selected = shop::run(g, shop::Menu {
            title: "Who would like to check in?", names: &names, prices: &prices,
            quantities: &mut vec![0; n], limits: &vec![0; n], rows: n,
            select_only: 1, money: 0, max_load: 0, weights: None, kind: 0,
            sign: 1, colors: Some(&who),
        });
        if selected == 0 { payer = -1; continue; }
        let index = selected - 1;
        render::cursor_off(g);
        shop::clear(g);
        if prices[index] > shop::cash(g, payer) {
            shop::say(g, if payer == -2 {
                b"I'm sure you people can scrape more than that!"
            } else { b"We don't have a payment system!" }, true, if payer == -2 {6} else {10});
            wait_reply(g, if payer == -2 {0x24} else {0x14});
        } else {
            shop::set_cash(g, payer, shop::cash(g, payer).wrapping_sub(prices[index]));
            let member = who[index] as u16 - 1;
            g.mem.set_party_u8(member * 256 + 0x127, g.mem.party_u8(member * 256 + 0x125));
            if payer != -2 { shop::clear(g); }
            render::back(g);
            render::hp_bar(g, member);
            if payer == -2 { render::status_panel(g); }
            render::front(g);
            shop::say(g, b"You're cured.", true, 10);
            if payer != -2 { render::status_panel(g); }
            wait_reply(g, 8);
        }
        render::cursor_on(g);
    }
}
