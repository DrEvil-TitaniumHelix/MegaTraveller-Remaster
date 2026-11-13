use super::{orders::{self, Op}, round, standing, tactics, state::{Ground, Mem, W_BUILTIN, W_TARGET}};
use crate::ui::{Intent, Target, Unit};

pub const SCAN_ESC: u8 = 0x01; pub const SCAN_MINUS: u8 = 0x0C; pub const SCAN_EQUALS: u8 = 0x0D; pub const SCAN_TAB: u8 = 0x0F; pub const SCAN_Q: u8 = 0x10;
pub const SCAN_ENTER: u8 = 0x1C; pub const SCAN_BACKSPACE: u8 = 0x0E; pub const SCAN_D: u8 = 0x20; pub const SCAN_H: u8 = 0x23; pub const SCAN_M: u8 = 0x32; pub const SCAN_KP_PLUS: u8 = 0x4E;
pub const BIOS_SHIFT_TAB: u16 = 0x10F; pub const BIOS_SHIFT_H: u16 = b'H' as u16; pub const BIOS_CTRL_H: u16 = 8;

pub fn key(g: &mut Ground, scan: u8) -> bool {
    if g.squad.is_none() { return false }
    let broken = g.mem.combat() != 0;
    let picking = g.squad.as_ref().is_some_and(|s| s.fire.rapid.is_some());
    let hovered = g.squad.as_ref().and_then(|s| s.orders.hovered);
    match scan {
        SCAN_ENTER if !picking && hovered.is_some() => { default_at(g, hovered.unwrap()); true }
        _ if orders::key(g, scan) => true,
        SCAN_TAB => { tab(g, g.bios_key == BIOS_SHIFT_TAB); true }
        SCAN_M => { menu_here(g); true }
        SCAN_Q => { orders::fire_mode(g); true }
        SCAN_EQUALS if broken => { orders::swap_hand(g); true }
        SCAN_KP_PLUS => { tactics::declare(g, 1); true }
        SCAN_MINUS => { tactics::declare(g, -1); true }
        SCAN_H if broken => { if g.bios_key == BIOS_SHIFT_H { tactics::hold(g, 1) } else { orders::perform(g, Op::HoldHere, Target::Cell((0, 0))) } true }
        SCAN_BACKSPACE if broken => { standing::hold_fire(g); true }
        SCAN_D if broken => { orders::say(g, "Hide: not yet"); true }
        SCAN_ESC => escape(g),
        _ => false,
    }
}
fn default_at(g: &mut Ground, t: Target) {
    let m = orders::menu_for(g, t);
    let Some(it) = m.items.get(m.default).cloned() else { return };
    if let Some(reason) = &it.grey { orders::say(g, &format!("{}: {reason}", it.label)); return }
    orders::perform(g, it.op, t)
}
pub fn tab(g: &mut Ground, back: bool) {
    let who = round::selected(g);
    if g.mem.combat() != 0 {
        let from = round::traveller_cell(g, who);
        let mut seen: Vec<(i32, u8)> = (1..=g.mem.npc_count()).filter(|&k| orders::attackable(g, k)).map(|k| (round::squares(from, round::npc_cell(g, k)), k))
            .filter(|&(d, k)| round::on_screen(g, round::npc_cell(g, k)) && d <= round::ENCOUNTER_RADIUS && round::line_of_sight(g, from, round::npc_cell(g, k))).collect();
        seen.sort();
        if !seen.is_empty() {
            let cur = g.mem.read(Mem::walker(who as i16).add(W_TARGET), 1) as u8;
            let n = seen.len();
            let next = match seen.iter().position(|&(_, k)| k == cur) { Some(i) => seen[if back { (i + n - 1) % n } else { (i + 1) % n }].1, None => seen[0].1 };
            orders::set_target(g, next);
            let name = round::name(g, who); let lbl = super::fire::label(g, Unit::Npc(next));
            orders::say(g, &format!("{name} targets {lbl}"));
            return
        }
    }
    let encounter = round::in_encounter(g);
    let order: Vec<u8> = round::marching_order(g).into_iter().filter(|&i| round::conscious(g, i) && (!encounter || !g.squad.as_ref().unwrap().p1.turns[i as usize].done)).collect();
    if order.is_empty() { return }
    let n = order.len();
    let next = match order.iter().position(|&i| i == who) { Some(i) => order[if back { (i + n - 1) % n } else { (i + 1) % n }], None => order[0] };
    orders::select(g, next);
}
fn menu_here(g: &mut Ground) {
    let who = round::selected(g);
    let standing = g.mem.read(Mem::walker(who as i16).add(W_TARGET), 1) as u8;
    let target = g.squad.as_ref().and_then(|s| s.orders.hovered)
        .or_else(|| (g.mem.combat() != 0 && standing != 0).then_some(Target::Unit(Unit::Npc(standing))))
        .unwrap_or(Target::Portrait(who));
    orders::intent(g, Intent::OpenMenu { target, at: (0, 0) });
}
fn escape(g: &mut Ground) -> bool {
    if g.squad.as_ref().is_some_and(|s| s.orders.menu.is_some()) { orders::intent(g, Intent::CloseMenu); return true }
    if let Some(s) = g.squad.as_mut() { if s.fire.rapid.take().is_some() { orders::say(g, "rapid fire cancelled"); return true } }
    let who = round::selected(g);
    if g.mem.combat() != 0 && g.mem.read(Mem::walker(who as i16).add(W_BUILTIN), 1) != 0 { g.mem.write(Mem::walker(who as i16).add(W_BUILTIN), 0, 1); orders::say(g, "grenade put away"); return true }
    if g.mem.combat() != 0 && !orders::read_route(g, who).is_empty() { orders::perform(g, Op::HoldHere, Target::Cell((0, 0))); return true }
    false
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::{orders::t::{fight, ground, p1, place}, state::W_HAND};
    fn hover(g: &Ground) -> String { g.squad.as_ref().unwrap().orders.hover.clone() }
    fn target(g: &Ground, who: u8) -> u8 { g.mem.read(Mem::walker(who as i16).add(W_TARGET), 1) as u8 }

    #[test]
    fn tab_scans_targets_in_a_fight_else_travellers() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); place(&mut g, 15, (25, 9), 0, 4, 41); fight(&mut g);
        g.mem.set_party_u16(0x100 + 0x1CE + 2 * 21, 1);
        assert!(round::key(&mut g, 3)); assert_eq!(round::selected(&g), 1);
        assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(target(&g, 1), 14); assert_eq!(hover(&g), "Trav2 targets thug (NPC 14)");
        assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(target(&g, 1), 15);
        g.bios_key = BIOS_SHIFT_TAB; assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(target(&g, 1), 14, "Shift+Tab cycles back"); g.bios_key = 0;
        assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(target(&g, 1), 15);
        assert!(round::key(&mut g, 0x21));
        assert!(p1(&g).log.iter().any(|l| l.starts_with("Trav2 fires the 9mm advanced combat rifle at thug (NPC 15)")), "{:?}", p1(&g).log);
        let mut g = ground(); fight_far(&mut g);
        assert_eq!(round::selected(&g), 0);
        assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(round::selected(&g), 1);
        round::end_turn(&mut g);
        assert_eq!(round::selected(&g), 0);
        assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(round::selected(&g), 2, "Trav2 has acted: skipped");
        g.bios_key = BIOS_SHIFT_TAB; assert!(round::key(&mut g, SCAN_TAB)); assert_eq!(round::selected(&g), 0); g.bios_key = 0;
        g.squad = None; assert!(!key(&mut g, SCAN_TAB)); assert!(!key(&mut g, SCAN_H));
    }
    #[test]
    fn hold_keys_tactics_keys_menu_enter_and_escape() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        orders::write_route(&mut g, 0, &[3, 3]); orders::set_target(&mut g, 14);
        assert!(round::key(&mut g, SCAN_H)); assert!(orders::read_route(&g, 0).is_empty(), "H = Hold here"); assert_eq!(hover(&g), "Trav1 holds here");
        assert!(round::key(&mut g, SCAN_BACKSPACE)); assert_eq!(target(&g, 0), 0, "Ctrl+H = Hold fire"); assert_eq!(hover(&g), "Trav1 holds fire");
        g.bios_key = BIOS_SHIFT_H; assert!(round::key(&mut g, SCAN_H)); g.bios_key = 0;
        assert!(p1(&g).turns[0].hold, "Shift+H = Hold (interrupt)");
        assert!(round::key(&mut g, SCAN_KP_PLUS)); assert_eq!(hover(&g), "Tactics: maximum 0 (the highest single level)");
        assert!(round::key(&mut g, SCAN_MINUS)); assert_eq!(hover(&g), "Tactics: nothing declared");
        assert!(round::key(&mut g, SCAN_EQUALS)); assert_eq!(g.mem.read(Mem::walker(0).add(W_HAND), 1), 1); assert!(round::key(&mut g, SCAN_EQUALS));
        assert!(round::key(&mut g, SCAN_D)); assert_eq!(hover(&g), "Hide: not yet");
        assert!(round::key(&mut g, SCAN_M)); assert_eq!(g.squad.as_ref().unwrap().orders.menu.as_ref().map(|m| m.target), Some(Target::Portrait(0)));
        assert!(round::key(&mut g, SCAN_ESC)); assert!(g.squad.as_ref().unwrap().orders.menu.is_none());
        orders::hover(&mut g, Some(Target::Unit(Unit::Npc(14))));
        assert!(round::key(&mut g, SCAN_M)); assert_eq!(g.squad.as_ref().unwrap().orders.menu.as_ref().map(|m| m.target), Some(Target::Unit(Unit::Npc(14))));
        orders::intent(&mut g, Intent::CloseMenu);
        p1_hold_off(&mut g);
        assert!(round::key(&mut g, SCAN_ENTER)); assert!(p1(&g).log.iter().any(|l| l.starts_with("Trav1 fires the 9mm advanced combat rifle at thug (NPC 14)")), "{:?}", p1(&g).log);
        assert!(round::key(&mut g, SCAN_ENTER)); assert_eq!(hover(&g), "Attack: no Attack left");
        assert!(!round::key(&mut g, SCAN_ESC), "nothing to close or cancel: the 1990 Escape");
        g.squad.as_mut().unwrap().fire.rapid = Some(vec![14]); assert!(round::key(&mut g, SCAN_ESC)); assert!(g.squad.as_ref().unwrap().fire.rapid.is_none()); assert_eq!(hover(&g), "rapid fire cancelled");
        g.mem.write(Mem::walker(0).add(W_BUILTIN), 1, 1); assert!(round::key(&mut g, SCAN_ESC)); assert_eq!(g.mem.read(Mem::walker(0).add(W_BUILTIN), 1), 0);
        orders::write_route(&mut g, 0, &[3]); assert!(round::key(&mut g, SCAN_ESC)); assert!(orders::read_route(&g, 0).is_empty());
        orders::hover(&mut g, Some(Target::Cell((21, 5)))); assert!(round::key(&mut g, SCAN_ENTER)); assert_eq!(round::traveller_cell(&g, 0), (21, 5));
        assert_eq!(round::selected(&g), 1); assert!(round::key(&mut g, SCAN_Q)); assert_eq!(hover(&g), "no autofire");
    }
    fn p1_hold_off(g: &mut Ground) { g.squad.as_mut().unwrap().p1.turns[0].hold = false }
    fn fight_far(g: &mut Ground) { place(g, 14, (34, 58), 0, 14, 41); g.squad.as_mut().unwrap().p1.defs.push(round::Def { id: "q".into(), start: round::Start::Area(round::Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), surprise: round::Surprise::None, boundary: round::Boundary::Level, ..round::Def::default_sight() }); round::pass(g, &[]); assert!(round::in_encounter(g)) }
}
