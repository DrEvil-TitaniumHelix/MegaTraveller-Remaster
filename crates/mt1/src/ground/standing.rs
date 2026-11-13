use super::{orders::{self, Speed}, round, state::{Ground, Mem, W_TARGET}};
use crate::ui::Cell;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State { pub advance: [bool; 5] }

fn st(g: &mut Ground) -> Option<&mut State> { g.squad.as_mut().map(|s| &mut s.p6) }
fn advancing(g: &Ground, who: u8) -> bool { g.squad.as_ref().is_some_and(|s| s.p6.advance[(who as usize).min(4)]) }
fn turn_done(g: &Ground, who: u8) -> bool { g.squad.as_ref().is_some_and(|s| s.p1.turns[(who as usize).min(4)].done) }

pub fn target(g: &Ground, who: u8) -> Option<u8> {
    let k = g.mem.read(Mem::walker(who as i16).add(W_TARGET), 1) as u8;
    (k != 0 && round::npc_present(g, k)).then_some(k)
}
pub fn delta(g: &Ground, d: u8) -> (i16, i16) { (g.mem.i8(0x2F5 + 2 * d as u16) as i16, g.mem.i8(0x2F6 + 2 * d as u16) as i16) }
pub fn destination(g: &Ground, who: u8) -> Option<Cell> {
    let route = orders::read_route(g, who);
    if route.is_empty() { return None }
    let mut c = round::traveller_cell(g, who);
    for d in route { let (dr, dc) = delta(g, d); c = (c.0 + dr, c.1 + dc) }
    Some(c)
}
pub fn stance(g: &Ground, who: u8) -> &'static str {
    if target(g, who).is_some() { "crosshair" } else if destination(g, who).is_some() { "boot" } else { "hand" }
}
pub fn offer(g: &Ground, who: u8) -> Option<String> {
    let mut parts = Vec::new();
    let n = orders::read_route(g, who).len();
    if let Some(k) = target(g, who) {
        let action = match orders::fire_check(g, who, k) {
            Ok(_) => Some("fire at"),
            Err(r) if r == "chamber empty" => Some("reload or change weapon for"),
            Err(_) if n > 0 => Some("move on, then try to fire at"),
            Err(_) => None,
        };
        if let Some(action) = action { parts.push(format!("{action} {}", super::fire::label(g, crate::ui::Unit::Npc(k)))) }
    }
    if n > 0 { parts.push(format!("walk {}", n.min(round::MOVE_WALK as usize))) }
    if parts.is_empty() { None } else { Some(parts.join(", ")) }
}
pub fn hold_fire(g: &mut Ground) {
    if g.squad.is_none() || g.mem.combat() == 0 { return }
    let who = round::selected(g);
    g.mem.write(Mem::walker(who as i16).add(W_TARGET), 0, 1);
    let n = round::name(g, who);
    orders::say(g, &format!("{n} holds fire"));
}
pub fn route_cleared(g: &mut Ground, who: u8) { if let Some(s) = st(g) { s.advance[(who as usize).min(4)] = false } }
fn fire_standing(g: &mut Ground, who: u8, k: u8) -> bool {
    let attacked = |g: &Ground| g.squad.as_ref().unwrap().p1.turns[(who as usize).min(4)].attacked;
    if attacked(g) { return false }
    match orders::fire_check(g, who, k) {
        Ok(_) => { orders::fire(g, k); return true }
        Err(r) if r == "chamber empty" => {}
        Err(_) => return false,
    }
    orders::reload(g);
    if attacked(g) { return true }
    orders::swap_hand(g);
    if orders::fire_check(g, who, k).is_ok() { orders::fire(g, k); return true }
    orders::swap_hand(g);
    g.mem.write(Mem::walker(who as i16).add(W_TARGET), 0, 1);
    let n = round::name(g, who);
    orders::log(g, format!("{n} has nothing to fire: target dropped"));
    false
}
fn fire_nearest(g: &mut Ground, who: u8) -> bool {
    let from = round::traveller_cell(g, who);
    let mut hostiles: Vec<(i32, u8)> = (1..=g.mem.npc_count()).filter(|&k| orders::attackable(g, k) && orders::fire_check(g, who, k).is_ok()).map(|k| (round::squares(from, round::npc_cell(g, k)), k)).collect();
    hostiles.sort();
    let Some(&(_, k)) = hostiles.first() else { return false };
    orders::fire(g, k);
    true
}
pub fn run(g: &mut Ground) -> bool {
    let who = round::selected(g);
    if orders::turn_check(g).is_err() { return false }
    let (k, dest) = (target(g, who), destination(g, who));
    if k.is_none() && dest.is_none() && !advancing(g, who) { return false }
    let n = round::name(g, who);
    orders::log(g, format!("{n} continues"));
    let mut fired = false;
    if let Some(k) = k { fired = fire_standing(g, who, k) }
    if !turn_done(g, who) { if let Some(d) = dest { orders::move_to(g, d, Speed::Walk) } }
    if !turn_done(g, who) && !fired { if let Some(k) = target(g, who) { fired = fire_standing(g, who, k) } }
    if !turn_done(g, who) && !fired && advancing(g, who) && orders::read_route(g, who).is_empty() { route_cleared(g, who); fire_nearest(g, who); }
    true
}
pub fn end_turn(g: &mut Ground) {
    let who = round::selected(g);
    let before = g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref().map(|e| e.round));
    run(g);
    let after = g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref().map(|e| e.round));
    if before == after && round::selected(g) == who && !turn_done(g, who) { round::end_turn(g) }
}
pub fn advance(g: &mut Ground, c: Cell) {
    let who = round::selected(g);
    let before = round::traveller_cell(g, who);
    orders::move_to(g, c, Speed::Auto);
    if round::traveller_cell(g, who) == before && orders::read_route(g, who).is_empty() { return }
    if orders::read_route(g, who).is_empty() {
        let n = round::name(g, who);
        if !fire_nearest(g, who) { orders::log(g, format!("{n} advances: nobody in sight")) }
    } else if let Some(s) = st(g) { s.advance[(who as usize).min(4)] = true }
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::{orders::t::{fight, ground, p1, place, wall}, round::KEY_SPACE, rules, squad, state::{W_HAND, W_X}};
    use crate::ui::{Intent, Target};
    fn logged(g: &Ground, s: &str) -> usize { p1(g).log.iter().filter(|l| l.contains(s)).count() }
    fn route_east(g: &mut Ground, who: u8, n: usize) { let d = orders::direction(g, (0, 1)).unwrap(); orders::write_route(g, who, &vec![d; n]) }
    fn acr(g: &mut Ground, slot: u16, obj: u16) { let o = 0xA82 + 5 * (obj - 1); g.mem.set_party_u16(256 * slot + 0x1CE + 2 * 21, obj); g.mem.set_party_u8(o, 14); g.mem.set_party_u8(o + 1, 3); g.mem.set_party_u8(o + 2, 20); g.mem.set_party_u8(256 * slot + 0x12B + 11, 2) }
    fn miss(g: &mut Ground, who: u8, k: u8) {
        let task = orders::fire_check(g, who, k).unwrap().task;
        for s in 1..20000u32 { g.rand.lcg.seed = s; let d = squad::two_d(g); if !rules::resolve(&task, d).hit { g.rand.lcg.seed = s; return } }
        panic!("no seed")
    }

    fn data_pick(g: &mut Ground, pick: &str) {
        use mt_engine::{io::Input, play::{Play, Choice, Pick}};
        let mut play = Play::new(); play.register(&crate::play::turns::decision(g).unwrap());
        play.choose(Choice::new("d:1", Pick::Id(pick.into()))).unwrap();
        let i = *play.intent().unwrap().downcast::<Intent>().unwrap();
        orders::intent(g, i);
    }
    #[test]
    fn m8v4_end_turn_fires_then_ends() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        orders::set_target(&mut g, 14); miss(&mut g, 0, 14);
        let ammo = g.mem.party_u8(0xA84);
        data_pick(&mut g, "end_turn");
        assert_eq!(g.mem.party_u8(0xA84), ammo - 1);
        assert!(p1(&g).turns[0].done); assert_eq!(logged(&g, "Trav1 continues"), 1);
    }
    #[test]
    fn m8v4_end_turn_standing_shot_engages_a_target_beyond_the_fight_extent() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        place(&mut g, 17, (2, 9), 0, 14, 41);
        assert!(!round::npc_side(&g).contains(&17));
        orders::set_target(&mut g, 17); miss(&mut g, 0, 17);
        let ammo = g.mem.party_u8(0xA84);
        data_pick(&mut g, "end_turn");
        assert_eq!(g.mem.party_u8(0xA84), ammo - 1);
        assert!(p1(&g).npcs[&17].engaged);
        assert!(round::npc_side(&g).contains(&17));
        assert!(p1(&g).turns[0].done);
    }
    #[test]
    fn m8v4_continue_fires_and_keeps_move() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        orders::set_target(&mut g, 14); miss(&mut g, 0, 14);
        let ammo = g.mem.party_u8(0xA84); data_pick(&mut g, "continue");
        assert_eq!(g.mem.party_u8(0xA84), ammo - 1); assert!(!p1(&g).turns[0].done);
    }
    #[test]
    fn an_opened_turn_is_published_at_once_and_space_stays_round_start_only() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        g.mem.set_redraw_pending(0); let open_before = p1(&g).strip.starts_with("Your move");
        if p1(&g).encounter.as_ref().unwrap().choosing { assert!(round::key(&mut g, round::KEY_SPACE)); assert!(!p1(&g).encounter.as_ref().unwrap().choosing, "Space = we go first"); }
        round::pass(&mut g, &[]);
        if p1(&g).strip.starts_with("Your move") && !open_before {
            assert_eq!(g.mem.redraw_pending(), 1, "the opened turn asks for a redraw (the window showed the stale round-start strip)");
            g.mem.set_redraw_pending(0); round::pass(&mut g, &[]); round::pass(&mut g, &[]);
            assert_eq!(g.mem.redraw_pending(), 0, "an already open turn is not re-published every idle pass (T056: the driver starved)");
            let before = p1(&g).log.len(); assert!(!round::key(&mut g, round::KEY_SPACE), "Space does nothing during a turn"); assert_eq!(p1(&g).log.len(), before);
        }
    }
    #[test]
    fn m8v4_o_fires_and_keeps_move_space_no_longer_does() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        orders::set_target(&mut g, 14); miss(&mut g, 0, 14);
        let ammo = g.mem.party_u8(0xA84);
        assert!(!round::key(&mut g, round::KEY_SPACE)); assert_eq!(g.mem.party_u8(0xA84), ammo, "Space fires nothing during a turn");
        assert!(round::key(&mut g, round::KEY_O));
        assert_eq!(g.mem.party_u8(0xA84), ammo - 1); assert!(!p1(&g).turns[0].done);
    }
    #[test]
    fn m8v4_end_turn_with_no_orders_ends() {
        let mut g=ground();place(&mut g,14,(24,9),0,14,41);fight(&mut g);
        data_pick(&mut g,"end_turn");assert!(p1(&g).turns[0].done);
    }
    #[test]
    fn blocked_target_without_route_never_promises_movement() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        acr(&mut g, 1, 3); orders::select(&mut g, 1); orders::set_target(&mut g, 14);
        wall(&mut g, (23, 7)); wall(&mut g, (23, 8));
        assert!(orders::read_route(&g, 1).is_empty());
        assert!(offer(&g, 1).is_none(), "no route and no shot: {:?}", offer(&g, 1));
    }

    #[test]
    fn continue_fires_then_walks_hold_fire_walks_only() {
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        g.mem.write(Mem::walker(4).add(W_X), 26 * 2, 2);
        acr(&mut g, 1, 3);
        orders::select(&mut g, 1); route_east(&mut g, 1, 14); orders::set_target(&mut g, 14);
        assert_eq!((target(&g, 1), destination(&g, 1), stance(&g, 1)), (Some(14), Some((22, 20)), "crosshair"));
        assert_eq!(offer(&g, 1).as_deref(), Some("fire at thug (NPC 14), walk 10"));
        assert_eq!(offer(&g, 0), None); assert_eq!(stance(&g, 0), "hand");
        orders::select(&mut g, 0); round::end_turn(&mut g); assert!(p1(&g).turns[0].done && p1(&g).npcs[&14].acted);
        assert_eq!(p1(&g).strip, "Your move: Trav2. Continue (O): fire at thug (NPC 14), walk 10");
        miss(&mut g, 1, 14);
        assert!(!round::key(&mut g, KEY_SPACE)); assert!(round::key(&mut g, round::KEY_O));
        assert_eq!(logged(&g, "Trav2 continues"), 1); assert_eq!(logged(&g, "Trav2 fires the 9mm advanced combat rifle at thug (NPC 14)"), 1, "{:?}", p1(&g).log);
        assert_eq!(round::traveller_cell(&g, 1), (22, 16)); assert_eq!(orders::read_route(&g, 1).len(), 4, "10 walked, 4 to go");
        assert!(logged(&g, "Trav2 walks 10 squares, 4 to go") == 1 && p1(&g).turns[1].done, "{:?}", p1(&g).log);
        for _ in 0..3 { round::end_turn(&mut g) } assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
        orders::select(&mut g, 0); round::end_turn(&mut g); assert_eq!(p1(&g).strip, "Your move: Trav2. Continue (O): fire at thug (NPC 14), walk 4", "{:?}", p1(&g).log);
        hold_fire(&mut g); assert_eq!(target(&g, 1), None); assert_eq!(offer(&g, 1).as_deref(), Some("walk 4")); assert_eq!(stance(&g, 1), "boot");
        orders::intent(&mut g, Intent::Continue);
        assert_eq!(logged(&g, "Trav2 fires"), 1, "no second shot: {:?}", p1(&g).log); assert_eq!(round::traveller_cell(&g, 1), (22, 20)); assert!(orders::read_route(&g, 1).is_empty() && !p1(&g).turns[1].done);
        let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
        wall(&mut g, (23, 7)); wall(&mut g, (23, 8));
        g.mem.write(Mem::walker(4).add(W_X), 26 * 2, 2);
        acr(&mut g, 1, 3);
        orders::select(&mut g, 1); orders::set_target(&mut g, 14); route_east(&mut g, 1, 2);
        assert_eq!(orders::fire_check(&g, 1, 14).err().as_deref(), Some("no line of sight")); assert_eq!(offer(&g, 1).as_deref(), Some("move on, then try to fire at thug (NPC 14), walk 2"));
        assert!(run(&mut g));
        assert_eq!(round::traveller_cell(&g, 1), (22, 8), "{} {:?}", g.squad.as_ref().unwrap().orders.hover, p1(&g).log); assert_eq!(logged(&g, "Trav2 fires the 9mm advanced combat rifle at thug (NPC 14)"), 1, "{:?}", p1(&g).log);
        assert!(p1(&g).turns[1].done);
    }
    #[test]
    fn continue_reloads_swaps_or_drops_the_target() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        orders::set_target(&mut g, 14); g.mem.set_party_u8(0xA82 + 2, 0);
        assert!(run(&mut g)); assert_eq!(logged(&g, "Trav1 reloads (20 rounds, 2 magazines left)"), 1, "{:?}", p1(&g).log); assert!(!p1(&g).turns[0].done && logged(&g, "Trav1 fires") == 0);
        for _ in 0..5 { round::end_turn(&mut g) } assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2); orders::select(&mut g, 0);
        g.mem.set_party_u8(0xA82 + 1, 0); g.mem.set_party_u8(0xA82 + 2, 0);
        g.mem.set_party_u16(0x1CE + 2 * 22, 2); g.mem.set_party_u8(0xA82 + 5, 4); g.mem.set_party_u8(0xA82 + 5 + 1, 0); g.mem.set_party_u8(0xA82 + 5 + 2, 15);
        orders::swap_hand(&mut g); miss(&mut g, 0, 14); orders::swap_hand(&mut g);
        assert!(run(&mut g));
        assert_eq!((g.mem.read(Mem::walker(0).add(W_HAND), 1), logged(&g, "Trav1 fires the 9mm auto pistol at thug (NPC 14)")), (1, 1), "{:?}", p1(&g).log);
        for _ in 0..5 { round::end_turn(&mut g) } assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 3); orders::select(&mut g, 0);
        g.mem.set_party_u8(0xA82 + 5 + 2, 0);
        assert!(run(&mut g));
        assert_eq!(logged(&g, "Trav1 has nothing to fire: target dropped"), 1, "{:?}", p1(&g).log); assert_eq!(target(&g, 0), None); assert!(!p1(&g).turns[0].done);
        assert!(!run(&mut g), "nothing standing: Continue leaves the turn available");
    }
    #[test]
    fn advance_walks_then_fires_at_the_nearest() {
        let mut g = ground(); g.mem.set_view_x(40); g.mem.set_view_y(8);
        place(&mut g, 14, (24, 13), 0, 14, 41); place(&mut g, 15, (26, 13), 0, 14, 41); fight(&mut g);
        let m = orders::menu_for(&g, Target::Cell((22, 11))); let a = m.items.iter().find(|i| i.label == "Advance").unwrap(); assert_eq!((a.key, a.grey.as_deref()), (b'A', None));
        orders::perform(&mut g, orders::Op::Advance, Target::Cell((22, 11)));
        assert_eq!(round::traveller_cell(&g, 0), (22, 11)); assert_eq!(logged(&g, "Trav1 fires the 9mm advanced combat rifle at thug (NPC 14)"), 1, "{:?}", p1(&g).log);
        assert!(p1(&g).turns[0].moved && p1(&g).turns[0].attacked);
        let mut g = ground(); fight_quiet(&mut g);
        advance(&mut g, (22, 10)); assert_eq!(logged(&g, "Trav1 advances: nobody in sight"), 1, "{:?}", p1(&g).log);
        let mut g = ground(); place(&mut g, 14, (20, 30), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.defs.push(round::Def { id: "far".into(), start: round::Start::Area(round::Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), boundary: round::Boundary::Level, surprise: round::Surprise::None, ..round::Def::default_sight() }); round::pass(&mut g, &[]); assert!(round::in_encounter(&g));
        advance(&mut g, (22, 30)); assert_eq!(orders::read_route(&g, 0).len(), 5); assert!(g.squad.as_ref().unwrap().p6.advance[0]);
        for _ in 0..5 { round::end_turn(&mut g) }
        assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2); orders::select(&mut g, 0);
        assert!(run(&mut g)); assert_eq!(round::traveller_cell(&g, 0), (22, 30));
        assert_eq!(logged(&g, "Trav1 fires the 9mm advanced combat rifle at thug (NPC 14)"), 1, "{:?}", p1(&g).log); assert!(!g.squad.as_ref().unwrap().p6.advance[0]);
    }
    fn fight_quiet(g: &mut Ground) { place(g, 14, (34, 58), 2, 14, 41);  g.squad.as_mut().unwrap().p1.defs.push(round::Def { id: "q".into(), start: round::Start::Area(round::Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), surprise: round::Surprise::None, boundary: round::Boundary::Level, ..round::Def::default_sight() }); round::pass(g, &[]); assert!(round::in_encounter(g)) }
}
