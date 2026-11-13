use super::{orders::{self, Shot}, round::{self, Side, ENCOUNTER_RADIUS}, rules, squad, state::Ground};
use crate::ui::Unit;

pub const INTERRUPT_TARGET: i32 = 7;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State { pub declared: i32, pub spent: i32, pub round: u16, pub paused: Option<(u8, super::npc_turn::Plan)> }

fn current_round(g: &Ground) -> u16 { g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref()).map_or(0, |e| e.round) }
fn st(g: &mut Ground) -> Option<&mut State> {
    let round = current_round(g);
    let s = &mut g.squad.as_mut()?.p22;
    if s.round != round { s.round = round; s.declared = 0; s.spent = 0 }
    Some(s)
}
fn rd(g: &Ground) -> State {
    let round = current_round(g);
    match g.squad.as_ref() { Some(s) if s.p22.round == round => s.p22.clone(), Some(s) => State { round, paused: s.p22.paused.clone(), ..State::default() }, None => State::default() }
}

pub fn pool(g: &Ground) -> i32 { g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref()).map_or(0, |e| e.pools.0) }
pub fn cap(g: &Ground) -> i32 { round::travellers(g).iter().map(|&i| round::traveller_fighter(g, i).tactics).max().unwrap_or(0) }
pub fn declared(g: &Ground) -> i32 { rd(g).declared }
pub fn left(g: &Ground) -> i32 { pool(g) - rd(g).spent - rd(g).declared }
pub fn hud(g: &Ground) -> String { if round::in_encounter(g) { format!("Tactics {}", left(g)) } else { String::new() } }
pub fn declare(g: &mut Ground, delta: i32) -> bool {
    if !round::in_encounter(g) { orders::say(g, "Tactics: not in a fight"); return false }
    let (cap, left, declared) = (cap(g), left(g), declared(g));
    let next = declared + delta;
    if next < 0 { orders::say(g, "Tactics: nothing declared"); return false }
    if delta > 0 && next > cap { orders::say(g, &format!("Tactics: maximum {cap} (the highest single level)")); return false }
    if delta > 0 && left <= 0 { orders::say(g, "Tactics: pool empty"); return false }
    if let Some(s) = st(g) { s.declared = next }
    let left = left - delta;
    orders::say(g, &format!("Tactics +{next} declared for the next roll ({left} left)"));
    true
}
pub fn apply(g: &Ground, mut shot: Shot) -> Shot {
    let n = if round::in_encounter(g) { declared(g) } else { 0 };
    if n > 0 { shot.task.dm = (shot.task.dm + n).clamp(-rules::DM_CAP, rules::DM_CAP); shot.tactics = n }
    shot
}
pub fn spend(g: &mut Ground, who: u8) {
    let n = declared(g);
    if n <= 0 { return }
    if let Some(s) = st(g) { s.spent += n; s.declared = 0 }
    let left = left(g); let name = round::name(g, who);
    orders::log(g, format!("{name} spends {n} tactical point{} ({left} left this round)", if n == 1 { "" } else { "s" }));
}

pub fn hold_check(g: &Ground, who: u8) -> Result<(), String> {
    orders::turn_check_for(g, who).map_err(String::from)?;
    if !round::in_encounter(g) { return Err("not in a fight".into()) }
    let t = g.squad.as_ref().unwrap().p1.turns[(who as usize).min(4)];
    if t.moved { return Err("has moved".into()) }
    if t.attacked { return Err("has attacked".into()) }
    Ok(())
}
pub fn hold(g: &mut Ground, speed: u8) {
    let who = round::selected(g);
    if let Err(r) = hold_check(g, who) { orders::say(g, &r); return }
    let speed = speed.clamp(1, 2);
    round::hold_at(g, speed);
    let n = round::name(g, who);
    orders::say(g, &format!("{n} holds ({}): interrupts at 2D + {speed} >= {INTERRUPT_TARGET}", if speed == 2 { "running" } else { "walking" }));
}
pub fn interrupt(g: &mut Ground, k: u8) -> bool {
    let Some(e) = g.squad.as_ref().and_then(|s| s.p1.encounter.clone()) else { return false };
    if e.interrupt.is_some() || e.side != Side::Npcs || !round::npc_present(g, k) { return false }
    let at = round::npc_cell(g, k);
    for i in round::marching_order(g) {
        let t = g.squad.as_ref().unwrap().p1.turns[i as usize];
        if !t.hold || t.done || !round::conscious(g, i) { continue }
        let from = round::traveller_cell(g, i);
        if round::squares(from, at) > ENCOUNTER_RADIUS || !round::line_of_sight(g, from, at) { continue }
        let speed = t.speed.clamp(1, 2) as i32;
        let (a, b) = squad::two_d(g);
        let total = a + b + speed;
        let ok = total >= INTERRUPT_TARGET;
        let n = round::name(g, i); let lbl = super::fire::label(g, Unit::Npc(k));
        orders::log(g, format!("Interrupt ({n} on {lbl}, speed {speed}): 2D {a}+{b} + {speed} = {total} vs {INTERRUPT_TARGET}: {}", if ok { "success" } else { "fails, the hold stays" }));
        if !ok { continue }
        let s = &mut g.squad.as_mut().unwrap().p1;
        s.turns[i as usize].hold = false;
        if let Some(e) = s.encounter.as_mut() { e.interrupt = Some((k, i)); e.side = Side::Party }
        s.log(format!("{n} interrupts!"));
        s.strip = format!("{n} interrupts: your move");
        s.acting = Some(Unit::Traveller(i));
        round::select(g, i);
        return true
    }
    false
}
pub fn interrupting(g: &Ground) -> Option<(u8, u8)> { g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref()).and_then(|e| e.interrupt) }

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::{orders::t::{fight, ground, p1, place}, round::{Order, MOVE_RUN, MOVE_WALK}, rules::adapters};
    use crate::ui::{Intent, Target, Verb};
    fn set_tactics(g: &mut Ground, i: u16, level: u8) { g.mem.set_party_u8(256 * i + 0x12B + adapters::SKILL_TACTICS as u16, level) }
    fn seed_2d(g: &mut Ground, at_least: bool, need: i32) {
        for s in 1..20000u32 { g.rand.lcg.seed = s; let (a, b) = squad::two_d(g); if (a + b >= need) == at_least { g.rand.lcg.seed = s; return } }
        panic!("no seed")
    }
    fn logged(g: &Ground, s: &str) -> usize { p1(g).log.iter().filter(|l| l.contains(s)).count() }

    #[test]
    fn tactical_points_declare_cap_preview_spend_and_renew() {
        let mut g = ground(); set_tactics(&mut g, 0, 2); set_tactics(&mut g, 2, 1);
        place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        assert!(round::key(&mut g, round::KEY_SPACE), "the larger pool chooses: we go first");
        assert_eq!((pool(&g), cap(&g), left(&g), hud(&g).as_str()), (3, 2, 3, "Tactics 3"));
        let base = orders::fire_check(&g, 0, 14).unwrap();
        assert!(declare(&mut g, 1) && declare(&mut g, 1)); assert_eq!((declared(&g), left(&g)), (2, 1));
        assert!(!declare(&mut g, 1)); assert_eq!(g.squad.as_ref().unwrap().orders.hover, "Tactics: maximum 2 (the highest single level)");
        let shot = orders::fire_check(&g, 0, 14).unwrap();
        assert_eq!((shot.task.dm, shot.tactics), (base.task.dm + 2, 2)); assert!(shot.describe().contains("+2 tactics"), "{}", shot.describe());
        orders::hover(&mut g, Some(Target::Unit(Unit::Npc(14)))); assert!(g.squad.as_ref().unwrap().orders.hover.contains("+2 tactics"));
        assert!(declare(&mut g, -1)); assert_eq!((declared(&g), left(&g)), (1, 2)); assert!(declare(&mut g, 1));
        let m = orders::menu_for(&g, Target::Unit(Unit::Npc(14)));
        let it = m.items.iter().find(|i| i.label.starts_with("Tactics")).unwrap(); assert_eq!((it.label.as_str(), it.grey.as_deref()), ("Tactics +3", Some("maximum 2 (the highest single level)")));
        orders::fire(&mut g, 14);
        assert_eq!((declared(&g), left(&g), logged(&g, "Trav1 spends 2 tactical points (1 left this round)")), (0, 1, 1), "{:?}", p1(&g).log);
        assert!(p1(&g).log.iter().any(|l| l.contains("fires the") && l.contains(&format!(" {:+} = ", base.task.dm + 2))), "the roll carried the DM: {:?}", p1(&g).log);
        assert!(declare(&mut g, 1)); assert!(!declare(&mut g, 1)); assert_eq!(g.squad.as_ref().unwrap().orders.hover, "Tactics: pool empty");
        for _ in 0..5 { round::end_turn(&mut g) }
        assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2); assert_eq!((declared(&g), left(&g)), (0, 3), "renewed, the undeclared point lost");
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        assert!(!declare(&mut g, 1)); assert_eq!(cap(&g), 0);
        let mut g = ground(); assert!(!declare(&mut g, 1)); assert_eq!(hud(&g), "");
    }
    #[test]
    fn hold_interrupts_an_npc_move_and_the_npc_resumes() {
        let mut g = ground(); place(&mut g, 14, (24, 5), 0, 26, 41); fight(&mut g);
        orders::select(&mut g, 2); assert_eq!(hold_check(&g, 2), Ok(()));
        hold(&mut g, 2);
        let t = p1(&g).turns[2]; assert!(t.hold && t.speed == 2 && t.mp == MOVE_RUN && !t.done, "{t:?}");
        orders::select(&mut g, 0); assert!(hold_check(&g, 0).is_ok());
        orders::move_to(&mut g, (21, 5), orders::Speed::Auto); assert_eq!(hold_check(&g, 0).err().as_deref(), Some("has moved"));
        seed_2d(&mut g, false, 5);
        round::end_turn(&mut g);
        assert_eq!(logged(&g, "Interrupt (Trav3 on thug (NPC 14), speed 2): 2D "), 1, "{:?}", p1(&g).log);
        assert!(logged(&g, "fails, the hold stays") == 1 && p1(&g).turns[2].hold && p1(&g).npcs[&14].acted, "{:?}", p1(&g).log);
        assert_eq!(logged(&g, "attacks Trav3 with the dagger"), 1, "the strike landed after the failed roll: {:?}", p1(&g).log);
        assert!(p1(&g).encounter.as_ref().unwrap().interrupt.is_none()); assert_eq!(p1(&g).strip, "Your move: Trav2");
        for _ in 0..3 { round::end_turn(&mut g) }
        assert_eq!(logged(&g, "Trav3: hold released"), 1, "{:?}", p1(&g).log); assert_eq!(p1(&g).strip, "Your move: Trav3");
        round::end_turn(&mut g); assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
        let mut g = ground(); place(&mut g, 14, (24, 5), 0, 26, 41); fight(&mut g);
        orders::select(&mut g, 2); hold(&mut g, 1); orders::select(&mut g, 0);
        seed_2d(&mut g, true, 6);
        round::end_turn(&mut g);
        assert_eq!(logged(&g, "Trav3 interrupts!"), 1, "{:?}", p1(&g).log); assert_eq!(logged(&g, "attacks Trav3 with the dagger"), 0);
        let e = p1(&g).encounter.clone().unwrap();
        assert_eq!((e.interrupt, e.side), (Some((14, 2)), Side::Party)); assert_eq!(round::selected(&g), 2); assert_eq!(p1(&g).strip, "Trav3 interrupts: your move");
        assert!(g.squad.as_ref().unwrap().p22.paused.as_ref().is_some_and(|(k, _)| *k == 14));
        assert!(!p1(&g).turns[2].hold && p1(&g).turns[2].mp == MOVE_WALK);
        orders::select(&mut g, 1); assert_eq!(round::selected(&g), 2, "only the interrupter acts during the interrupt");
        round::order(&mut g, Order::Attack); round::order(&mut g, Order::Move);
        assert!(p1(&g).turns[2].done); assert_eq!(logged(&g, "attacks Trav3 with the dagger"), 1, "the strike followed the interrupt turn: {:?}", p1(&g).log);
        assert!(p1(&g).encounter.as_ref().unwrap().interrupt.is_none() && g.squad.as_ref().unwrap().p22.paused.is_none(), "{:?}", p1(&g).log);
        assert_eq!(p1(&g).strip, "Your move: Trav2", "{:?}", p1(&g).log);
        let mut g = ground(); g.mem.set_view_y(8); place(&mut g, 14, (23, 15), 0, 26, 41); fight(&mut g);
        orders::select(&mut g, 2); hold(&mut g, 1); orders::select(&mut g, 0);
        let before = round::npc_cell(&g, 14);
        seed_2d(&mut g, true, 6);
        round::end_turn(&mut g);
        assert_eq!(logged(&g, "Trav3 interrupts!"), 1, "{:?}", p1(&g).log);
        assert_eq!(round::squares(round::npc_cell(&g, 14), before), 1, "the NPC stopped after its first square");
        assert_eq!(round::selected(&g), 2);
        round::end_turn(&mut g);
        assert!(round::squares(round::npc_cell(&g, 14), before) >= 5, "the NPC completed its approach: {:?} {:?}", round::npc_cell(&g, 14), p1(&g).log);
        assert_eq!((logged(&g, "Interrupt ("), logged(&g, "closes in to strike") + logged(&g, "with the dagger")), (1, 1), "no second roll; the approach ended in reach or not: {:?}", p1(&g).log);
        assert!(p1(&g).turns[2].done); assert_eq!(p1(&g).strip, "Your move: Trav2", "{:?}", p1(&g).log);
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        round::pass(&mut g, &[Intent::Verb { verb: Verb::Hold, target: Target::Cell((0, 0)) }]); assert!(p1(&g).turns[0].hold && p1(&g).turns[0].speed == 1);
        assert_eq!(round::selected(&g), 1, "the hold hands the turn on"); orders::select(&mut g, 0);
        orders::fire(&mut g, 14); assert_eq!(hold_check(&g, 0).err().as_deref(), Some("has attacked")); assert!(!p1(&g).turns[0].hold, "an order ends the hold");
        let mut g = ground(); hold(&mut g, 1); assert!(!p1(&g).turns[0].hold);
    }
}
