use super::{render, state::{Ground, Mem}};
use mt_engine::io::{mouse::{hit, Hotspot, MouseEvent}, scan};
pub mod dialogue;
pub mod menu;
pub mod ui;
mod dispatch;
pub use dispatch::{dispatch, m4b_note, Flow};

pub const CALLBACK: u16 = 0x7004;
pub const MOUSE_PENDING: u16 = 0x7014;
pub const ASCII: u16 = 0x504D;
pub const RECTANGLES: u16 = 0x707D;

pub fn read_mouse(g: &mut Ground) {
    let mut m = g.input.mouse();
    g.mem.set_u16(0x6FF8, m.buttons);
    if g.mem.i16(0x7012) != 0 && m.row > 46 && m.col <= 192 {
        if g.mem.i16(0x6FFE) <= 46 { m.row = 46 } else { m.col = 193 }
        g.input.set_mouse_position(m.col, m.row);
    }
    g.mem.set_i16(0x6FFE, m.row); g.mem.set_i16(0x6FFC, m.col);
}

pub fn mouse_callback(g: &mut Ground, m: MouseEvent) {
    g.mouse_event = Some(m);
    g.mem.set_u16(CALLBACK, m.event);
    g.mem.set_u16(CALLBACK + 2, m.buttons);
    g.mem.set_i16(CALLBACK + 6, m.col);
    g.mem.set_i16(CALLBACK + 4, m.row);
    g.mem.set_key(if m.event == 0x10 { 1 } else { 0x6E });
}

pub fn special_key(g: &mut Ground, table_off: u16, count: usize) {
    let (row, col) = (g.mem.i16(CALLBACK + 4), g.mem.i16(CALLBACK + 6));
    let spots = (0..count.min(65536 / 9)).map(|i| {
        let off = table_off.wrapping_add((i * 9) as u16);
        Hotspot { top: g.mem.i16(off), left: g.mem.i16(off.wrapping_add(2)),
            height: g.mem.i16(off.wrapping_add(4)), width: g.mem.i16(off.wrapping_add(6)), id: g.mem.u8(off.wrapping_add(8)) }
    }).take_while(|h| h.id != 0);
    let found = hit(spots, col, row);
    if let Some(id) = found {
        g.mem.set_u8(MOUSE_PENDING, 1); g.mem.set_key(id); g.mem.set_u16(CALLBACK, 0);
        g.mouse_event = None;
    }
}

pub fn keyboard_scan(g: &mut Ground, code: u8) {
    g.mem.set_u8(0x6FEA, code);
    g.speaker_refresh^=0x10;
    g.mem.set_u8(0x6FE5,g.speaker_refresh);
    if code == scan::SCROLL_LOCK || code == 0x1D { return }
    if code > 0x80 { if g.mem.key() & 0x7F > 0x46 { g.mem.set_key(0) } }
    else { g.mem.set_key(code) }
}

pub fn wait_scan(g: &mut Ground) -> u8 {
    crate::play_prompts::ack(g, "D2C3");
    if g.mem.u16(0x7000) != 0 { while g.input.mouse().buttons != 0 { g.poll() } }
    g.mem.set_key(0); render::cursor_on(g);
    while g.mem.key() == 0 { g.poll() }
    render::cursor_off(g);
    let code = g.mem.key(); g.mem.set_key(0); g.mem.set_u8(MOUSE_PENDING, 0); code
}

pub fn translated_key(mem: &Mem, code: u8) -> u16 {
    match code {
        0x1C => 13, 0x0E => 14, 1 => 27,
        0x48 => 1, 0x50 => 2, 0x4B => 3, 0x4D => 4,
        0x49 => 5, 0x51 => 6, 0x47 => 7, 0x4F => 8,
        0x4E => 43, 0x4A => 45, 0x46 | 0x4C => 0,
        _ => mem.u8(ASCII + code as u16) as u16,
    }
}

pub fn menu_getkey(g: &mut Ground) -> u16 {
    crate::play_prompts::ack(g, "161EE");
    g.mem.set_key(0); g.mem.set_i16(0x6E0C, -1);
    loop {
        render::cursor_on(g);
        while g.mem.key() == 0 { g.poll() }
        render::cursor_off(g);
        if g.mem.key() == 0x6E {
            if g.mem.u8(0x6E2D) != 0 {
                read_mouse(g);
                let row = g.mem.i16(0x6FFE);
                if row < 10 { return 5 }
                if row > 190 { return 6 }
                g.mem.set_i16(0x6E0C, row); return 5000;
            }
            let bridge = g.mem.u8(0x6E2C) != 0;
            special_key(g, if bridge { 0x1F59 } else { 0x1ED2 }, 65536 / 9);
            if g.mem.key() == 0x6E { g.mem.set_key(0x2C) }
            if bridge { let key = g.mem.key(); g.mem.set_key(0); return key as u16 }
        }
        let key = translated_key(&g.mem, g.mem.key());
        if key != 0 { g.mem.set_key(0); return key }
        g.poll();
    }
}

pub fn inventory_action(mut key: u8, selected: u8) -> (&'static str, u8) {
    if (2..=9).contains(&key) {
        key -= 2; if key != selected { return ("select_action", key) } key = 0x2F;
    }
    let action = match key {
        0x41..=0x44 => "equipment", 0x96..=0xA8 => "inventory_slot",
        1 => "leave", 0x10 => "unequip", 0x16 => "equip", 0x19 => "pick",
        0x1C | 0x2F => "activate", 0x32 => "move", 0x50 => "next_action",
        0x51 => "next_skill", 0x3B..=0x3F => "character", 0x48 => "previous_action",
        0x49 => "first_skill", 0x18 => "orders", _ => "ignore",
    }; (action,key)
}
pub fn character_scan(g: &mut Ground, who: i16) -> u8 {
    crate::play_prompts::character(g,who);
    let key = wait_scan(g);
    if key != 0x6E { return key }
    g.mem.set_key(key);
    special_key(g, super::state::HOTSPOTS_CHAR, super::state::HOTSPOTS_CHAR_N);
    let key = g.mem.key(); g.mem.set_key(0); key
}

#[cfg(test)]
mod pending_tests {
    use super::*;
    struct Pending;
    impl crate::ground::Tooling for Pending {
        fn poll(&mut self, _: &mut Ground) -> bool { false }
        fn pending(&self) -> bool { true }
        fn last_error(&self) -> Option<String> { Some("scenario x: boom".into()) }
    }
    #[test]
    fn a_pending_scenario_pick_unwinds_modal_waits_with_escape_and_reports_errors() {
        let mut g = crate::ground::orders::t::ground(); g.tooling = Some(Box::new(Pending));
        assert_eq!(menu_getkey(&mut g), 27, "Escape");
        let mut injected = 0; for _ in 0..260 { g.mem.set_key(0); g.poll(); if g.mem.key() == 1 { injected += 1 } }
        assert_eq!(injected, 199, "200 per pending pick (one was spent by the wait above)");
        g.main_wait = true; g.tooling = Some(Box::new(Pending)); g.escapes = 0; g.mem.set_key(0); g.poll(); assert_eq!(g.mem.key(), 0, "never inside the main loop's own wait");
        assert_eq!(crate::play::diagnostic::ground(&mut g).scenario_error.as_deref(), Some("scenario x: boom"));
    }
}
