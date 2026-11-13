use crate::ground::Ground;
use mt_engine::{io::{keys, Event, Key}, play::{Charset, Decision, Kind, Option, TextInput}};

pub fn keys_decision(program: &str, routine: &str, kind: Kind, choices: &[(&str, &str, Key)]) -> Decision {
    let mut d = Decision::new(kind, program, routine);
    d.options = choices.iter().map(|&(id, label, key)| Option::new(id, label, vec![Event::Key(key)])).collect();
    d.default = d.options.first().map(|o| o.id.clone());
    d
}
pub fn prefer_no(d: &mut Decision) {
    if let Some(no) = d.options.iter().find(|o| o.id == "no" || o.label.to_ascii_lowercase().starts_with("no") || o.label.starts_with("Continue")) { d.default = Some(no.id.clone()); }
}
pub fn table_rects(g: &Ground, d: &mut Decision, table: u16, n: usize) {
    hotspot_rects(d, (0..n as u16).map(|e| { let at = table.wrapping_add(9 * e);
        mt_engine::io::mouse::Hotspot { top: g.mem.i16(at), left: g.mem.i16(at + 2), height: g.mem.i16(at + 4), width: g.mem.i16(at + 6), id: g.mem.u8(at + 8) } }));
}
pub fn hotspot_rects(d: &mut Decision, hits: impl Iterator<Item = mt_engine::io::mouse::Hotspot>) {
    for h in hits {
        let id = h.id; if id == 0 { continue }
        let r = [h.left as i32, h.top as i32, h.width as i32, h.height as i32];
        for (i, o) in d.options.iter().enumerate() {
            let hit = match o.input.as_slice() {
                [Event::Key(k)] => mt_engine::io::scan::make_code(*k) == Some(id),
                [Event::Mouse(m)] => (r[0]..r[0] + r[2]).contains(&(m.col as i32)) && (r[1]..r[1] + r[3]).contains(&(m.row as i32)),
                _ => false };
            if hit { d.rects.push((i, r)); }
        }
    }
}
pub fn named(g: &mut Ground, routine: &str, choices: &[(&str, &str, Key)], words: &[[i32; 4]]) {
    if g.lockstep.is_none() && g.input.wants_decision() {
        let mut d = keys_decision("t", routine, Kind::Menu, choices); d.rects = words.iter().copied().enumerate().collect();
        crate::play::decide(g, d);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_destructive_yes_no_prefers_no() {
        let mut d = keys_decision("s", "4ED0", Kind::YesNo, &[("yes", "Quit", b'Y' as u16), ("no", "Continue flight", b'N' as u16)]);
        assert_eq!(d.default.as_deref(), Some("yes"), "the 1990 order");
        prefer_no(&mut d); assert_eq!(d.default.as_deref(), Some("no"));
        let mut cg = keys_decision("cg", "1AA2", Kind::YesNo, &[("yes", "YES", b'Y' as u16), ("no", "NO", b'N' as u16)]);
        prefer_no(&mut cg); assert_eq!(cg.default.as_deref(), Some("no"));
        let mut other = keys_decision("t", "x", Kind::Menu, &[("a", "Alpha", b'A' as u16)]); prefer_no(&mut other); assert_eq!(other.default.as_deref(), Some("a"), "nothing to prefer");
    }
}
pub fn named_state(g: &mut Ground, routine: &str, choices: &[(&str, &str, Key)], live: &[u8], words: &[[i32; 4]]) {
    if g.lockstep.is_none() && g.input.wants_decision() {
        let mut d=keys_decision("t",routine,Kind::Menu,choices);
        let mut h=mt_engine::play::Fnv::default(); h.field(live); d.context.table=Some(h.state());
        d.rects=words.iter().copied().enumerate().collect();
        crate::play::decide(g,d);
    }
}
pub fn ack(g: &mut Ground, routine: &str) {
    if g.lockstep.is_none() && g.input.wants_decision() {
        crate::play::decide(g, keys_decision("t", routine, Kind::Ack,
            &[("continue", "Continue", 32), ("back", "Back", keys::ESC)]));
    }
}
pub fn text(g: &mut Ground, max_len: usize, alpha: bool) {
    if g.lockstep.is_none() && g.input.wants_decision() {
        let mut d = Decision::new(if alpha { Kind::Text } else { Kind::Number }, "t", "15B69");
        d.input = Some(TextInput { max_len, width: None, charset: if alpha { Charset::Upper } else { Charset::Digits }, submit: vec![Event::Key(keys::ENTER)] });
        d.options.push(Option::new("back", "Cancel", vec![Event::Key(keys::ESC)]));
        crate::play::decide(g, d);
    }
}
pub fn character(g: &mut Ground, who: i16) {
    if g.lockstep.is_some() || !g.input.wants_decision() { return }
    let mut d = keys_decision("t", "A8F7", Kind::Menu, &[
        ("back", "Close", keys::ESC), ("cmd:unequip", "Quit using", b'Q' as Key),
        ("cmd:equip", "Use object", b'U' as Key), ("cmd:pick", "Pick object", b'P' as Key),
        ("cmd:activate", "View", keys::ENTER), ("cmd:move", "Move item", b'M' as Key),
        ("cmd:next-action", "Next action", keys::DOWN), ("cmd:previous-action", "Previous action", keys::UP),
        ("cmd:next-skill", "Next skill", keys::PGDN), ("cmd:first-skill", "First skill", keys::PGUP),
        ("cmd:orders", "Orders", b'O' as Key),
    ]);
    for i in 0..8 { d.options.push(Option::new(format!("action:{i}"), format!("Action {}", i+1), vec![Event::Key(b'1' as Key+i)])); }
    for i in 0..5 { d.options.push(Option::new(format!("cmd:char/{i}"), format!("Traveller {}", i+1), vec![Event::Key(keys::F1+i)])); }
    for i in 0..4 { d.options.push(Option::new(format!("equipment:{i}"), format!("Equipment {}", i+1), vec![Event::Key(0x141+i)])); }
    for i in 0..crate::ground::state::HOTSPOTS_CHAR_N {
        let at = crate::ground::state::HOTSPOTS_CHAR.wrapping_add(9*i as u16);
        let id = g.mem.u8(at+8);
        if (0x96..=0xA8).contains(&id) {
            d.options.push(Option::new(format!("inventory:{}", id-0x96), format!("Inventory cell {}", id-0x96),
                vec![Event::Mouse(mt_engine::io::MouseEvent { event: 4, buttons: 0, col: g.mem.i16(at+2)+1, row: g.mem.i16(at)+1 })]));
        }
    }
    d.context.who = Some(who as usize);
    table_rects(g, &mut d, crate::ground::state::HOTSPOTS_CHAR, crate::ground::state::HOTSPOTS_CHAR_N);
    for (i, o) in d.options.iter().enumerate() {
        match o.id.as_str() {
            "cmd:activate" => {
                let at = crate::ground::state::HOTSPOTS_CHAR + 11 * 9;
                d.rects.push((i, [g.mem.i16(at + 2) as i32, g.mem.i16(at) as i32, g.mem.i16(at + 6) as i32, g.mem.i16(at + 4) as i32]));
            }
            "back" => {
                let at = 0x3CD0 + 13 * 9;
                let width = g.mem.cstr(g.mem.pointer(at + 4)).bytes().map(|c| g.mem.u8(0x5128 + c as u16) as i32).sum();
                d.rects.push((i, [g.mem.i16(at + 2) as i32, g.mem.i16(at) as i32, width, 8]));
            }
            _ => {}
        }
    }
    crate::play::decide(g, d);
}
pub fn bridge(g: &mut Ground, mask: u8, selected: i16) {
    if g.lockstep.is_some() || !g.input.wants_decision() { return }
    let mut d = keys_decision("t", "13FF2", Kind::Menu, &[("back", "Leave starport", keys::ESC)]);
    let mut cells = String::new();
    for (slot, id, label) in [(1,"info","Information"),(2,"save","Save"),(4,"bank","Bank"),
        (5,"pensions","Pensions"),(7,"recruit","Recruit"),(8,"training","Training"),(10,"buy-cargo","Buy cargo"),
        (11,"sell-cargo","Sell cargo"),(13,"fuel","Refuel"),(14,"repair","Repairs"),(15,"sell-weapon","Sell weapons"),
        (16,"buy-weapon","Buy weapons"),(17,"supplies","Ship supplies")] {
        let row = if slot >= 15 { 4 } else { slot / 3 };
        let mut option = Option::new(format!("cmd:{id}"), label, vec![]);
        option.enabled = mask & (1 << row) != 0;
        if let Some(i) = (0..64).find(|&i| g.mem.u8(0x1F59 + 9*i+8) == 200+slot as u8) {
            let at = 0x1F59+9*i;
            let event = Event::Mouse(mt_engine::io::MouseEvent { event: 4, buttons: 0, row: g.mem.i16(at)+g.mem.i16(at+4)/2, col: g.mem.i16(at+2)+g.mem.i16(at+6)/2 });
            option.input = if selected == slot { vec![event] } else { vec![event, Event::Key(keys::ENTER)] };
            cells += &format!("{}{}={},{},{},{}", if cells.is_empty() { "" } else { ";" }, d.options.len(), g.mem.i16(at+2), g.mem.i16(at), g.mem.i16(at+6), g.mem.i16(at+4));
        } else { option.enabled = false; }
        d.options.push(option);
    }
    if !cells.is_empty() { d.context.table = Some(format!("cells:{cells}")); }
    crate::play::decide(g, d);
}

#[cfg(test)]
mod starport_tests {
    use super::*;
    use crate::ground::state::{ds_image, Mem};
    use mt_engine::{io::{mouse, Input, Overlay}, sim::tick::Counter};
    use std::{cell::RefCell, rc::Rc};
    mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }

    #[test]
    fn every_starport_action_hits_its_own_original_cell() {
        struct Capture(Rc<RefCell<std::option::Option<Decision>>>);
        impl Input for Capture {
            fn next_key(&mut self) -> u16 { 27 }
            fn wants_decision(&self) -> bool { true }
            fn decide(&mut self, d: &Decision) { *self.0.borrow_mut() = Some(d.clone()); }
        }
        let Some(dir) = oracle_env::gamefiles() else { return };
        let ds = ds_image(&std::fs::read(dir.join("T.DAT")).unwrap()).unwrap();
        let captured = Rc::new(RefCell::new(None));
        let mut g = Ground::from_ds(ds, vec![0; Mem::PARTY_SIZE], Box::new(Capture(captured.clone())),
            Box::new(Counter::at(123)), Box::new(Overlay::new(None)));
        assert_eq!(mouse::hit(mouse::table(&g.mem.ds[0x1F59..], 13), 155, 177), Some(214));
        for selected in [15, 16] {
            bridge(&mut g, 31, selected);
            let d = captured.borrow(); let d = d.as_ref().unwrap();
            for (option, slot) in d.options[1..].iter().zip([1,2,4,5,7,8,10,11,13,14,15,16,17]) {
                assert!(option.enabled);
                let Event::Mouse(m) = option.input[0] else { panic!("missing native click") };
                assert_eq!(mouse::hit(mouse::table(&g.mem.ds[0x1F59..], 13), m.col, m.row), Some(200+slot), "{}", option.id);
                assert_eq!(&option.input[1..], if selected == slot as i16 { &[][..] } else { &[Event::Key(keys::ENTER)][..] });
            }
        }
    }
}
