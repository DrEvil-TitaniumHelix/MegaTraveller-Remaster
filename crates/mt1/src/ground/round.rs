use super::{fire, combat, commands, commands_party, orders, rules::adapters::{self, Fighter}, squad, state::{Ground, Mem, N_ARMED, N_ATTITUDE, N_LEVEL, N_X, N_Y, W_ACTIVE, W_FACING, W_X, W_Y}, walk, render};
use crate::ui::{Intent, Unit};
use mt_engine::sim::sched::SECOND;
use std::collections::BTreeMap;

pub const ROUND_TICKS: u32 = 108; pub const SQUARE_TICKS: u32 = 11; pub const ENCOUNTER_RADIUS: i32 = 29;
pub const SCREEN_TILES: (i32, i32) = (7, 12);
pub const ENCOUNTER_END_ROUNDS: u16 = 1; pub const ALERT_ROUNDS: u16 = 10;
pub const PHASE_AUTOFIRE: u8 = 1; pub const PHASE_ATTACKS: u8 = 2; pub const PHASE_ROUTES: u8 = 4; pub const PHASE_WANDER: u8 = 8; pub const PHASES_ALL: u8 = 15;
pub const MOVE_WALK: i32 = 10; pub const MOVE_RUN: i32 = 20;
const LOG_LIMIT: usize = 64;
pub const KEY_O: u8 = 0x18; pub const KEY_SPACE: u8 = 0x39; pub const KEY_N: u8 = 0x31;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect { pub r0: i16, pub c0: i16, pub r1: i16, pub c1: i16 }
impl Rect { pub fn contains(&self, (r, c): (i16, i16)) -> bool { (self.r0..=self.r1).contains(&r) && (self.c0..=self.c1).contains(&c) } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side { Party, Npcs }
impl Side { pub fn other(self) -> Side { match self { Side::Party => Side::Npcs, Side::Npcs => Side::Party } } pub fn name(self) -> &'static str { match self { Side::Party => "party", Side::Npcs => "NPCs" } } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Start { Sight, Area(Rect), Unsupported(String) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum End { Open { rounds: u16 }, Locked { escape: Option<Rect> } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Boundary { Radius(i32), Area(Rect), Level, Screen }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surprise { Roll, Attackers, Defenders, None }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Def { pub id: String, pub start: Start, pub end: End, pub boundary: Boundary, pub participants: Option<Vec<u8>>, pub auto_break: bool, pub allow_save: bool, pub surprise: Surprise, pub attacker: Side, pub alerted_rounds: u16 }
impl Def {
    pub fn default_sight() -> Def { Def { id: "default".into(), start: Start::Sight, end: End::Open { rounds: ENCOUNTER_END_ROUNDS }, boundary: Boundary::Screen, participants: None, auto_break: true, allow_save: false, surprise: Surprise::Roll, attacker: Side::Npcs, alerted_rounds: ALERT_ROUNDS } }
    pub fn locked(&self) -> bool { matches!(self.end, End::Locked { .. }) }
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Turn { pub moved: bool, pub attacked: bool, pub done: bool, pub hold: bool, pub speed: u8, pub evade: bool, pub mp: i32 }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NpcSt { pub role: Option<String>, pub skills: BTreeMap<String, i32>, pub engaged: bool, pub alerted: Option<u16>, pub alert_full: u16, pub last_seen: Option<(i16, i16)>, pub speed: u8, pub acted: bool,
    pub clip: u16, pub reload: bool, pub routed: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Encounter { pub def: String, pub round: u16, pub side: Side, pub first: Side, pub choosing: bool, pub surprised: Option<Side>, pub quiet: u16, pub grouped: bool, pub group_moved: bool, pub pools: (i32, i32),
    pub npc_start: Vec<u8>, pub withdrawing: bool, pub morale_round: u16,
    pub interrupt: Option<(u8, u8)> }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fx { pub seq: u32, pub unit: Option<Unit>, pub walk: Vec<(i16, i16)>, pub shots: Vec<((i16, i16), (i16, i16), u8)>, pub hits: Vec<(Unit, (i16, i16), i32)>, pub blasts: Vec<(i16, i16)> }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State {
    pub defs: Vec<Def>, pub done: Vec<String>, pub encounter: Option<Encounter>, pub turns: [Turn; 5], pub npcs: BTreeMap<u8, NpcSt>,
    pub strip: String, pub log: Vec<String>, pub acting: Option<Unit>,
    pub fx: Fx, pub logged: u32,
    pub clock: Option<u32>, pub pending: u32, pub explore_ticks: u32, pub walked: u32,
}
impl State {
    pub fn reset_level(&mut self) { self.encounter = None; self.npcs.clear(); self.done.clear(); self.acting = None; self.strip.clear(); self.explore_ticks = 0; self.walked = 0; }
    pub fn def(&self, id: &str) -> Option<&Def> { self.defs.iter().find(|d| d.id == id) }
    pub fn active_def(&self) -> Def { self.encounter.as_ref().and_then(|e| self.def(&e.def)).cloned().unwrap_or_else(Def::default_sight) }
    pub fn log(&mut self, line: String) { if self.log.len() == LOG_LIMIT { self.log.remove(0); } self.log.push(line); self.logged = self.logged.wrapping_add(1) }
}
fn st(g: &mut Ground) -> Option<&mut State> { g.squad.as_mut().map(|s| &mut s.p1) }
fn rd(g: &Ground) -> Option<&State> { g.squad.as_ref().map(|s| &s.p1) }
pub fn fx_begin(g: &mut Ground, unit: Unit) { if let Some(s) = st(g) { s.fx = Fx { seq: s.fx.seq.wrapping_add(1), unit: Some(unit), ..Fx::default() } } }
pub fn fx(g: &mut Ground) -> Option<&mut Fx> { st(g).map(|s| &mut s.fx) }

fn party(g: &Ground, off: u16) -> u8 { g.mem.party_u8(off) }
pub fn alive(g: &Ground, i: u8) -> bool { i < 5 && party(g, 0) & (1 << i) != 0 && party(g, 0x127 + 256 * i as u16) != 0 }
pub fn conscious(g: &Ground, i: u8) -> bool { alive(g, i) && party(g, 0x127 + 256 * i as u16) > party(g, 0x126 + 256 * i as u16) }
pub fn name(g: &Ground, i: u8) -> String { (0..32).map(|k| party(g, 0x102 + 256 * i as u16 + k)).take_while(|&b| b != 0).map(|b| b as char).collect() }
pub fn traveller_cell(g: &Ground, i: u8) -> (i16, i16) {
    let w = Mem::walker(i as i16);
    if g.mem.combat() != 0 && g.mem.read(w.add(W_ACTIVE), 1) != 0 { return (g.mem.read_i(w.add(W_X), 2) as i16 >> 1, g.mem.read_i(w.add(W_Y), 2) as i16 >> 1) }
    (g.mem.x() >> 1, g.mem.y() >> 1)
}
pub fn travellers(g: &Ground) -> Vec<u8> { (0..5).filter(|&i| conscious(g, i)).collect() }
pub fn marching_order(g: &Ground) -> Vec<u8> { (0..5).map(|k| party(g, 1 + k)).filter(|&i| i < 5).collect() }
pub fn npc_cell(g: &Ground, k: u8) -> (i16, i16) { let n = Mem::npc(k as i16); (g.mem.read_i(n.add(N_X), 2) as i16 >> 1, g.mem.read_i(n.add(N_Y), 2) as i16 >> 1) }
fn npc_byte(g: &Ground, k: u8, off: i32) -> u8 { g.mem.read(Mem::npc(k as i16).add(off), 1) as u8 }
pub fn npc_present(g: &Ground, k: u8) -> bool { k >= 1 && k <= g.mem.npc_count() && npc_byte(g, k, N_LEVEL) as i8 == g.mem.level() && npc_cell(g, k) != (0, 0) }
pub fn npc_armed(g: &Ground, k: u8) -> bool { npc_byte(g, k, N_ARMED) != 0 }
pub fn willing(g: &Ground, k: u8) -> bool {
    let attitude = npc_byte(g, k, N_ATTITUDE) & 3;
    (npc_byte(g, k, 0x26) != 0 && g.mem.party_fired() != 0) || attitude == 0 || (attitude == 1 && g.mem.provoked() != 0)
}
pub fn hostile(g: &Ground, k: u8) -> bool { npc_byte(g, k, N_ATTITUDE) & 3 == 0 }
pub fn routed(g: &Ground, k: u8) -> bool { rd(g).is_some_and(|s| s.npcs.get(&k).is_some_and(|n| n.routed)) }
pub fn squares(a: (i16, i16), b: (i16, i16)) -> i32 { (a.0 as i32 - b.0 as i32).abs().max((a.1 as i32 - b.1 as i32).abs()) }
pub fn nearest_traveller(g: &Ground, from: (i16, i16)) -> Option<(u8, i32)> { travellers(g).into_iter().map(|i| (i, squares(from, traveller_cell(g, i)))).min_by_key(|&(_, d)| d) }
pub fn line_of_sight(g: &Ground, a: (i16, i16), b: (i16, i16)) -> bool {
    let (dr, dc) = (b.0 as i32 - a.0 as i32, b.1 as i32 - a.1 as i32);
    let n = dr.abs().max(dc.abs());
    for i in 1..n {
        let (r, c) = (a.0 as i32 + (dr * i * 2 + n).div_euclid(2 * n), a.1 as i32 + (dc * i * 2 + n).div_euclid(2 * n));
        if r < 0 || c < 0 || r >= g.mem.map_blocks_high() as i32 * 12 || c >= g.mem.map_stride() as i32 { return false }
        let cell = commands::cell(g, r as i16, c as i16);
        if combat::cell_enterable(g, cell, g.mem.read(cell.add(1), 1) as i32 / 20, 0) == 0 { return false }
    }
    true
}
pub fn on_screen(g: &Ground, (r, c): (i16, i16)) -> bool {
    let (x, y, vx, vy) = (r as i32 * 2, c as i32 * 2, g.mem.view_x() as i32, g.mem.view_y() as i32);
    x >= vx && y >= vy && x <= vx + 2 * (SCREEN_TILES.0 - 1) && y <= vy + 2 * (SCREEN_TILES.1 - 1)
}
pub fn spots(g: &Ground, k: u8) -> Option<u8> {
    let from = npc_cell(g, k);
    if !on_screen(g, from) { return None }
    let mut seen: Vec<(i32, u8)> = travellers(g).into_iter().map(|i| (squares(from, traveller_cell(g, i)), i))
        .filter(|&(_, i)| line_of_sight(g, from, traveller_cell(g, i))).collect();
    seen.sort(); seen.first().map(|&(_, i)| i)
}
pub fn neutral(g: &Ground, k: u8) -> bool { npc_present(g, k) && !willing(g, k) && npc_byte(g, k, N_ATTITUDE) & 3 != 1 }
pub fn watcher(g: &Ground) -> Option<u8> {
    (1..=g.mem.npc_count()).find(|&k| npc_present(g, k) && npc_armed(g, k) && willing(g, k) && !routed(g, k) && spots(g, k).is_some())
}
pub fn sees(g: &Ground, k: u8) -> Option<u8> {
    let from = npc_cell(g, k);
    let mut seen: Vec<(i32, u8)> = travellers(g).into_iter().map(|i| (squares(from, traveller_cell(g, i)), i)).filter(|&(d, i)| d <= ENCOUNTER_RADIUS && line_of_sight(g, from, traveller_cell(g, i))).collect();
    seen.sort(); seen.first().map(|&(_, i)| i)
}
fn in_boundary(g: &Ground, def: &Def, cell: (i16, i16)) -> bool {
    match &def.boundary {
        Boundary::Level => true, Boundary::Area(r) => r.contains(cell),
        Boundary::Radius(r) => travellers(g).iter().any(|&i| squares(cell, traveller_cell(g, i)) <= *r),
        Boundary::Screen => on_screen(g, cell),
    }
}
pub fn npc_side(g: &Ground) -> Vec<u8> {
    let Some(s) = rd(g) else { return Vec::new() };
    if s.encounter.is_none() { return Vec::new() }
    let def = s.active_def();
    let engaged = |k: u8| s.npcs.get(&k).is_some_and(|n| n.engaged);
    let mut side: Vec<(i32, u8)> = (1..=g.mem.npc_count()).filter(|&k| npc_present(g, k) && npc_armed(g, k) && (engaged(k) || in_boundary(g, &def, npc_cell(g, k))))
        .map(|k| (nearest_traveller(g, npc_cell(g, k)).map_or(i32::MAX, |(_, d)| d), k)).collect();
    side.sort(); side.into_iter().map(|(_, k)| k).collect()
}
fn hostiles_left(g: &Ground, def: &Def) -> bool {
    match &def.participants {
        Some(list) => list.iter().any(|&k| k >= 1 && k <= 32 && npc_byte(g, k, N_LEVEL) != 20 && hostile(g, k) && !routed(g, k)),
        None => npc_side(g).into_iter().any(|k| hostile(g, k) && !routed(g, k)),
    }
}
pub fn npc_fighter(g: &Ground, k: u8) -> Fighter { let mut f = role_fighter(g, k); if let Some(n) = story_name(g, k) { f.label = n.into() } f }
pub const STORY_NAMES: &[(u32, &str)] = &[(221, "Viktor"), (321, "Konrad Kiefer")];
pub fn story_name(g: &Ground, k: u8) -> Option<&'static str> { let id = g.mem.read(Mem::npc(k as i16), 2); STORY_NAMES.iter().find(|n| n.0 == id).map(|n| n.1) }
pub fn tagged(label: &str, k: u8) -> String { if STORY_NAMES.iter().any(|n| n.1 == label) || label == format!("NPC {k}") { label.into() } else { format!("{label} (NPC {k})") } }
pub fn described(g: &Ground, k: u8, attitude: &str) -> String { let l = npc_fighter(g, k).label; if story_name(g, k).is_some() { format!("{attitude} ({l})") } else { format!("{attitude} ({l}, NPC {k})") } }
fn role_fighter(g: &Ground, k: u8) -> Fighter {
    let Some(sq) = g.squad.as_ref() else { return adapters::npc(g, k, &Default::default()) };
    let over = sq.p1.npcs.get(&k);
    let level = npc_byte(g, k, 11) as i32;
    let mut f = match over.and_then(|o| o.role.as_deref()) {
        Some("none") => adapters::npc(g, k, &Default::default()),
        Some(role) => sq.compendium.roles.get(role).map_or_else(|| adapters::npc(g, k, &sq.compendium), |e| adapters::fighter_of(e, level)),
        None => adapters::npc(g, k, &sq.compendium),
    };
    if let Some(o) = over {
        for (skill, &level) in &o.skills {
            match skill.as_str() { "Tactics" => f.tactics = level, "Leader" => f.leader = level, "Recon" => f.recon = level, _ => {} }
            if let adapters::Skills::Entry(map, _) = &mut f.skills { map.insert(skill.clone(), level); }
        }
    }
    f
}
pub fn traveller_fighter(g: &Ground, i: u8) -> Fighter { adapters::traveller(g, i, g.squad.as_ref().map_or(0, |s| s.ablat_worn[(i as usize).min(4)] as i32)) }
pub fn pools(g: &Ground) -> (i32, i32) {
    (travellers(g).iter().map(|&i| traveller_fighter(g, i).tactics).sum(), npc_side(g).iter().filter(|&&k| !routed(g, k)).map(|&k| npc_fighter(g, k).tactics).sum())
}
fn leader_recon(g: &Ground, side: Side) -> (i32, i32) {
    let fs: Vec<Fighter> = match side { Side::Party => travellers(g).iter().map(|&i| traveller_fighter(g, i)).collect(), Side::Npcs => npc_side(g).iter().map(|&k| npc_fighter(g, k)).collect() };
    (fs.iter().map(|f| f.leader).max().unwrap_or(0), fs.iter().map(|f| f.recon).max().unwrap_or(0))
}
pub fn in_encounter(g: &Ground) -> bool { rd(g).is_some_and(|s| s.encounter.is_some()) }
pub fn locked(g: &Ground) -> bool { rd(g).is_some_and(|s| s.encounter.is_some() && s.active_def().locked()) }
pub fn alerted_count(g: &Ground) -> usize { rd(g).map_or(0, |s| s.npcs.values().filter(|n| n.alerted.is_some()).count()) }
pub fn selected(g: &Ground) -> u8 { ((if g.mem.combat() != 0 { g.mem.leader() as u8 } else { g.mem.selected_char() }) & 7).min(4) }
pub fn queue(g: &Ground) -> Vec<Unit> {
    let Some(s) = rd(g) else { return Vec::new() };
    let Some(e) = &s.encounter else { return Vec::new() };
    let party: Vec<Unit> = marching_order(g).into_iter().filter(|&i| conscious(g, i) && !s.turns[i as usize].done).map(Unit::Traveller).collect();
    let npcs: Vec<Unit> = npc_side(g).into_iter().filter(|k| !s.npcs.get(k).is_some_and(|n| n.acted)).map(Unit::Npc).collect();
    let (mut a, mut b) = if e.side == Side::Party { (party.into_iter(), npcs.into_iter()) } else { (npcs.into_iter(), party.into_iter()) };
    let mut out = Vec::new();
    loop { match (a.next(), b.next()) { (None, None) => break out, (x, y) => { out.extend(x); out.extend(y) } } }
}
pub fn phases(g: &Ground) -> u8 { match rd(g) { None => PHASES_ALL, Some(s) if s.encounter.is_some() => 0, Some(_) => PHASE_ROUTES | PHASE_WANDER } }

pub fn clock(g: &mut Ground) -> Option<u32> {
    let now = g.clock.now();
    let s = st(g)?;
    let base = *s.clock.get_or_insert(now);
    let step = s.pending.min(SECOND); s.pending -= step;
    s.clock = Some(base.wrapping_add(step)); s.clock
}
pub fn frozen(g: &Ground) -> Option<u32> { rd(g)?.clock }
pub fn refreeze(g: &mut Ground) { if let Some(f) = frozen(g) { freeze(g, f) } }
pub fn freeze(g: &mut Ground, f: u32) {
    g.set_clock(f);
    while g.clock.ticks() != f && (f.wrapping_sub(g.clock.ticks()) as i32) > 0 { g.clock.wait_tick() }
    g.set_clock(f)
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step { Walked, Refused, Nothing }
pub fn step(before: (i16, i16, i16), after: (i16, i16, i16), event: i8) -> Step {
    let (x, y) = (after.0, after.1);
    if (x | y) & 1 != 0 { return Step::Nothing }
    if (x, y) != (before.0, before.1) { Step::Walked } else if after.2 == before.2 && event == -1 { Step::Refused } else { Step::Nothing }
}
pub fn walked(g: &mut Ground, squares: u32) {
    let who = selected(g) as usize;
    if g.mem.combat() != 0 {
        let w = Mem::walker(who as i16);
        g.mem.write(w.add(W_X), g.mem.x() as u16 as u32, 2); g.mem.write(w.add(W_Y), g.mem.y() as u16 as u32, 2); g.mem.write(w.add(W_FACING), g.mem.heading() as u8 as u32, 1);
    }
    let Some(s) = st(g) else { return };
    if s.encounter.is_some() { let t = &mut s.turns[who]; t.moved = true; t.mp = (t.mp - squares as i32).max(0); return }
    s.pending += SQUARE_TICKS * squares; s.explore_ticks += SQUARE_TICKS * squares; s.walked += SQUARE_TICKS * squares;
}

pub fn pass(g: &mut Ground, intents: &[Intent]) {
    if g.squad.is_none() { return }
    for &i in intents { intent(g, i) }
    if in_encounter(g) { run(g) } else { explore(g) }
}
pub fn key(g: &mut Ground, scan: u8) -> bool {
    if super::keymap::key(g, scan) { return true }
    let Some(s) = rd(g) else { return false };
    let Some(e) = &s.encounter else { return scan == KEY_O && g.mem.combat() != 0 };
    let who = selected(g);
    let (choosing, side, t) = (e.choosing, e.side, s.turns[who as usize]);
    match scan {
        KEY_O => { if choosing { choose(g, Side::Party) } else { super::standing::run(g); } true }
        KEY_SPACE => { if choosing { choose(g, Side::Party); true } else { false } }
        KEY_N => { if choosing { choose(g, Side::Npcs) } true }
        0x47..=0x51 if scan != 0x4E => choosing || side != Side::Party || t.done || t.mp <= 0 || !conscious(g, who),
        _ => false,
    }
}
fn intent(g: &mut Ground, i: Intent) { orders::intent(g, i) }
pub fn hold_at(g: &mut Ground, speed: u8) {
    if !in_encounter(g) { return }
    let who = selected(g) as usize; let n = name(g, who as u8);
    if let Some(s) = st(g) { let t = &mut s.turns[who]; if !t.done { t.hold = true; t.speed = speed; t.mp = if speed == 2 { MOVE_RUN } else { MOVE_WALK }; s.log(format!("{n} holds")) } }
}
pub fn hold(g: &mut Ground) { hold_at(g, 1) }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Order { Move, Attack }
pub fn order(g: &mut Ground, o: Order) {
    let who = selected(g);
    let Some(s) = st(g) else { return };
    let Some(e) = s.encounter.as_mut() else { return };
    if e.choosing || e.side != Side::Party { return }
    let grouped = e.grouped;
    let t = &mut s.turns[who as usize];
    if t.done { return }
    t.hold = false;
    match o {
        Order::Move => { if grouped { e.group_moved = true } t.moved = true }
        Order::Attack => { t.attacked = true; if t.moved { t.mp = 0 } }
    }
    let finished = t.attacked && (t.moved || (grouped && e.group_moved));
    if finished { t.done = true; t.hold = false; e.side = Side::Npcs; }
    if finished { fire::turn_over(g, Unit::Traveller(who)) }
    run(g)
}
pub fn select(g: &mut Ground, t: u8) {
    if g.mem.combat() != 0 {
        let (route, f3) = (orders::read_route(g, t), Mem::walker(t as i16).add(super::state::W_BUILTIN)); let grenade = g.mem.read(f3, 1);
        walk::set_leader(g, t as i16); orders::write_route(g, t, &route); if grenade != 0 { g.mem.write(f3, grenade, 1) }
    }
    g.mem.set_selected_char(t);
    if in_encounter(g) && rd(g).is_some_and(|s| s.encounter.as_ref().is_some_and(|e| e.side == Side::Party && !e.choosing) && !s.turns[(t as usize).min(4)].done) { fire::turn_start(g, Unit::Traveller(t)); }
    g.mem.set_redraw_pending(1);
}
pub fn end_turn(g: &mut Ground) {
    let who = selected(g);
    let Some(s) = st(g) else { return };
    let Some(e) = s.encounter.as_mut() else { return };
    if e.choosing || e.side != Side::Party || s.turns[who as usize].done { return }
    s.turns[who as usize] = Turn { done: true, ..s.turns[who as usize] }; s.turns[who as usize].hold = false;
    e.side = Side::Npcs;
    let n = name(g, who); st(g).unwrap().log(format!("{n}: turn over"));
    fire::turn_over(g, Unit::Traveller(who));
    run(g)
}
fn choose(g: &mut Ground, first: Side) {
    if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { if e.choosing { e.choosing = false; e.first = first; e.side = first } }
    if let Some(s) = st(g) { s.log(format!("Initiative: {} first", first.name())) }
    run(g)
}

fn explore(g: &mut Ground) {
    resume_wait(g);
    let dark = matches!(g.mem.level(), 1 | 2) && g.mem.party_u16(0xF82) == 0;
    if dark && !g.dark_told { if let Some(s) = st(g) { let line = "It is dark: press L for a light"; if s.log.last().is_none_or(|l| l != line) { s.log(line.into()) } } g.mem.set_redraw_pending(1) }
    g.dark_told = dark;
    let s = st(g).unwrap();
    let (mut rounds, mut steps) = (0, 0);
    while s.explore_ticks >= ROUND_TICKS { s.explore_ticks -= ROUND_TICKS; rounds += 1 }
    while s.walked >= SQUARE_TICKS { s.walked -= SQUARE_TICKS; steps += 1 }
    let alerted: Vec<u8> = s.npcs.iter().filter(|(_, n)| n.alerted.is_some()).map(|(&k, _)| k).collect();
    for k in alerted {
        if !npc_present(g, k) || npc_byte(g, k, N_LEVEL) == 20 { st(g).unwrap().npcs.get_mut(&k).unwrap().alerted = None; continue }
        let near = nearest_traveller(g, npc_cell(g, k)).is_some_and(|(_, d)| d <= ENCOUNTER_RADIUS); let who = story_name(g, k).map_or(format!("NPC {k}"), Into::into);
        for _ in 0..steps { approach(g, k, 1); }
        let s = st(g).unwrap(); let n = s.npcs.get_mut(&k).unwrap();
        for _ in 0..rounds { if near { n.alerted = Some(n.alert_full) } else if let Some(left) = n.alerted { n.alerted = (left > 1).then(|| left - 1) } }
        let gone = n.alerted.is_none();
        if gone { s.log(format!("{who} loses interest")) }
    }
    let count = alerted_count(g);
    let strip = if count > 0 { format!("Alerted: {count}") } else { String::new() };
    let s = st(g).unwrap(); let changed = s.strip != strip; if changed { s.strip = strip }
    if changed { g.mem.set_redraw_pending(1) }
    let cells: Vec<(i16, i16)> = travellers(g).into_iter().map(|i| traveller_cell(g, i)).collect();
    let s = rd(g).unwrap();
    let triggered = s.defs.iter().find(|d| !s.done.contains(&d.id) && matches!(&d.start, Start::Area(r) if cells.iter().any(|c| r.contains(*c)))).cloned();
    if let Some(def) = triggered { start(g, def, false); return }
    if let Some(k) = watcher(g) {
        let alerted = rd(g).unwrap().npcs.get(&k).is_some_and(|n| n.alerted.is_some());
        let def = pick_def(g, k);
        start(g, def, alerted);
    }
}
fn resume_wait(g: &mut Ground) {
    let Some((who, dest)) = g.waiting else { return };
    let route = orders::read_route(g, who);
    if route.is_empty() || who != selected(g) || in_encounter(g) { g.waiting = None; return }
    let (r, c) = traveller_cell(g, who); let d = super::standing::delta(g, route[0]);
    if orders::occupant(g, (r + d.0, c + d.1)) == 0 { g.waiting = None; orders::move_to(g, dest, orders::Speed::Walk) }
}
fn pick_def(g: &Ground, k: u8) -> Def {
    let s = rd(g).unwrap();
    s.defs.iter().find(|d| d.start == Start::Sight && match &d.participants { Some(list) => list.contains(&k), None => in_boundary(g, d, npc_cell(g, k)) }).cloned().unwrap_or_else(Def::default_sight)
}
pub fn start(g: &mut Ground, def: Def, alerted_restart: bool) {
    let locked = def.locked();
    let s = st(g).unwrap();
    if !s.defs.iter().any(|d| d.id == def.id) { s.defs.push(def.clone()) }
    if matches!(def.start, Start::Area(_)) { s.done.push(def.id.clone()) }
    s.log(if locked { "Encounter! No way out but through.".into() } else { "Encounter!".into() });
    let mut grouped = g.mem.combat() == 0;
    if grouped && def.auto_break && g.mem.outdoors() != 0 && g.mem.encounter_mode() == 0 && commands_party::split(g) { grouped = false }
    if grouped { st(g).unwrap().log("The party fights as one (no room to break)".into()) }
    let e = Encounter { def: def.id.clone(), round: 1, side: Side::Party, first: Side::Party, choosing: false, surprised: None, quiet: 0, grouped, group_moved: false, pools: (0, 0), npc_start: Vec::new(), withdrawing: false, morale_round: 0, interrupt: None };
    set_encounter(g, Some(e));
    let roster = npc_side(g); if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { e.npc_start = roster }
    let any_alerted = npc_side(g).iter().any(|k| rd(g).unwrap().npcs.get(k).is_some_and(|n| n.alerted.is_some()));
    let surprised = if alerted_restart || any_alerted { st(g).unwrap().log("No surprise: they were expecting you".into()); None } else { surprise(g, &def) };
    if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { e.surprised = surprised }
    let strip = if locked { "Encounter! No way out but through." } else { "Encounter!" };
    st(g).unwrap().strip = strip.into();
    round_start(g);
    g.mem.set_redraw_pending(1);
    run(g)
}
pub fn engage(g: &mut Ground, k: u8) {
    if let Some(s) = st(g) { s.npcs.entry(k).or_default().engaged = true }
    if in_encounter(g) || !npc_present(g, k) { return }
    let who = (selected(g) as usize).min(4);
    let mut def = pick_def(g, k); def.attacker = Side::Party;
    start(g, def, false);
    if let Some(s) = st(g).filter(|s| s.encounter.is_some()) { s.turns[who].attacked = true; g.mem.set_redraw_pending(1) }
}
fn surprise(g: &mut Ground, def: &Def) -> Option<Side> {
    let (att, dfd) = (def.attacker, def.attacker.other());
    let result = match def.surprise {
        Surprise::None => None, Surprise::Attackers => Some(dfd), Surprise::Defenders => Some(att),
        Surprise::Roll => {
            let ((al, ar), (dl, dr)) = (leader_recon(g, att), leader_recon(g, dfd));
            let (a, b) = squad::two_d(g);
            let total = a + b + al + ar - dl - dr;
            let line = format!("Surprise ({} rolls): 2D {a}+{b} + Leader {al} + Recon {ar} - {dl} - {dr} = {total} vs 11", att.name());
            st(g).unwrap().log(line);
            if total >= 11 { Some(dfd) } else if total <= 9 { Some(att) } else { None }
        }
    };
    let text = match result { Some(Side::Party) => "Surprised!", Some(Side::Npcs) => "Surprise: the party", None => "No surprise" };
    st(g).unwrap().log(text.into());
    result
}
fn set_encounter(g: &mut Ground, e: Option<Encounter>) {
    if let Some(sq) = g.squad.as_mut() { sq.encounter = e.is_some(); sq.round = e.as_ref().map_or(0, |e| e.round); sq.p1.encounter = e; if sq.p1.encounter.is_none() { sq.p1.acting = None } }
}
fn round_start(g: &mut Ground) {
    let (p, n) = pools(g);
    let surprised = rd(g).unwrap().encounter.as_ref().and_then(|e| e.surprised);
    let s = st(g).unwrap();
    let Some(e) = s.encounter.as_mut() else { return };
    e.pools = (p, n); e.group_moved = false;
    let (first, choosing) = if p > n { (Side::Party, true) } else if n > p { (Side::Npcs, false) } else { (Side::Party, false) };
    e.first = first; e.side = first; e.choosing = choosing;
    let round = e.round;
    for t in &mut s.turns { *t = Turn { attacked: surprised == Some(Side::Party), mp: MOVE_WALK, ..Turn::default() } }
    for np in s.npcs.values_mut() { np.acted = false }
    let line = format!("Round {round} - initiative: {} (Tactics {p} vs {n})", if n > p { "NPCs" } else { "party" });
    s.strip = if choosing { format!("{line}. We go first? Space = yes, N = they go first") } else { line.clone() };
    s.log(line);
    s.acting = None;
    if round > 1 { if let Some(head) = marching_order(g).into_iter().find(|&i| conscious(g, i)) { if selected(g) != head { select(g, head) } } }
    let def = rd(g).unwrap().active_def();
    if let Some(list) = &def.participants {
        let side = npc_side(g);
        let out: Vec<String> = list.iter().copied().filter(|&k| k >= 1 && k <= 32 && npc_present(g, k) && !routed(g, k) && !side.contains(&k)).map(|k| k.to_string()).collect();
        if !out.is_empty() { st(g).unwrap().log(format!("Round {round}: out of reach: NPC {} (beyond the fight's extent, no turn)", out.join(", "))) }
    }
    fire::round_start(g);
}
fn run(g: &mut Ground) {
    for _ in 0..256 {
        let Some(e) = rd(g).and_then(|s| s.encounter.clone()) else { return };
        if e.choosing { return }
        if check_end(g, false) { return }
        if !(0..5u16).any(|i| party(g, 0x127 + 256 * i) > party(g, 0x126 + 256 * i)) { return }
        if let Some((k, who)) = e.interrupt {
            if !rd(g).unwrap().turns[who as usize].done { let n = name(g, who); let s = st(g).unwrap(); let strip = format!("{n} interrupts: your move"); if s.acting != Some(Unit::Traveller(who)) || s.strip != strip { s.acting = Some(Unit::Traveller(who)); s.strip = strip; g.mem.set_redraw_pending(1) } return }
            if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { e.interrupt = None; e.side = Side::Npcs }
            npc_resume(g, k);
            if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { if e.interrupt.is_none() { e.side = Side::Party } }
            continue
        }
        let s = rd(g).unwrap();
        let party_left: Vec<u8> = marching_order(g).into_iter().filter(|&i| conscious(g, i) && !s.turns[i as usize].done).collect();
        let holds: Vec<u8> = party_left.iter().copied().filter(|&i| s.turns[i as usize].hold).collect();
        let npcs_left: Vec<u8> = npc_side(g).into_iter().filter(|k| !s.npcs.get(k).is_some_and(|n| n.acted)).collect();
        match e.side {
            Side::Party if !party_left.is_empty() => {
                if holds.len() == party_left.len() && npcs_left.is_empty() {
                    for &i in &holds { st(g).unwrap().turns[i as usize].hold = false; let n = name(g, i); st(g).unwrap().log(format!("{n}: hold released")) }
                }
                let ready: Vec<u8> = party_left.iter().copied().filter(|&i| !rd(g).unwrap().turns[i as usize].hold).collect();
                if ready.is_empty() { if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { e.side = Side::Npcs } continue }
                let sel = selected(g);
                let who = if ready.contains(&sel) { sel } else { ready[0] };
                if who != sel { select(g, who) }
                if fire::turn_start(g, Unit::Traveller(who)) { continue }
                let n = name(g, who); let offer = super::standing::offer(g, who);
                let s = st(g).unwrap(); let strip = match offer { Some(o) => format!("Your move: {n}. Continue (O): {o}"), None => format!("Your move: {n}") };
                if s.acting != Some(Unit::Traveller(who)) || s.strip != strip { s.acting = Some(Unit::Traveller(who)); s.strip = strip; g.mem.set_redraw_pending(1) }
                return
            }
            Side::Party if !npcs_left.is_empty() => { st(g).unwrap().encounter.as_mut().unwrap().side = Side::Npcs }
            Side::Npcs if !npcs_left.is_empty() => { npc_turn(g, npcs_left[0]); if let Some(e) = st(g).and_then(|s| s.encounter.as_mut()) { e.side = Side::Party } }
            Side::Npcs if !party_left.is_empty() => { st(g).unwrap().encounter.as_mut().unwrap().side = Side::Party }
            _ => round_end(g),
        }
        g.mem.set_redraw_pending(1);
    }
}
pub fn npc_turn(g: &mut Ground, k: u8) {
    fx_begin(g, Unit::Npc(k)); let from = npc_cell(g, k); if let Some(f) = fx(g) { f.walk.push(from) }
    let who = fire::label(g, Unit::Npc(k)); if let Some(s) = st(g) { s.strip = format!("NPC turn: {who}") }
    super::npc_turn::turn(g, k);
    if g.scene.is_some() { g.mem.set_redraw_pending(0); render::redraw(g) }
}
fn npc_resume(g: &mut Ground, k: u8) {
    fx_begin(g, Unit::Npc(k)); let from = npc_cell(g, k); if let Some(f) = fx(g) { f.walk.push(from) }
    let who = fire::label(g, Unit::Npc(k)); if let Some(s) = st(g) { s.strip = format!("NPC turn: {who} (resumed)") }
    super::npc_turn::resume(g, k);
    if g.scene.is_some() { g.mem.set_redraw_pending(0); render::redraw(g) }
}
fn approach(g: &mut Ground, k: u8, max: usize) -> usize { super::npc_turn::approach(g, k, max, None) }
fn round_end(g: &mut Ground) {
    let surprised = rd(g).unwrap().encounter.as_ref().and_then(|e| e.surprised);
    { let s = st(g).unwrap(); s.pending += ROUND_TICKS; let e = s.encounter.as_mut().unwrap(); e.round += 1; }
    if let Some(sq) = g.squad.as_mut() { sq.round = sq.p1.encounter.as_ref().map_or(0, |e| e.round) }
    if let Some(side) = surprised {
        let (l, r) = leader_recon(g, side);
        let (a, b) = squad::two_d(g);
        let lost = a + b + l + r >= 11;
        let s = st(g).unwrap();
        s.log(format!("Alarm ({}): 2D {a}+{b} + Leader {l} + Recon {r} vs 11: {}", side.name(), if lost { "surprise lost" } else { "still surprised" }));
        if lost { s.encounter.as_mut().unwrap().surprised = None }
    }
    if check_end(g, true) { return }
    round_start(g)
}
fn check_end(g: &mut Ground, round_end: bool) -> bool {
    let Some(s) = rd(g) else { return false };
    if s.encounter.is_none() { return false }
    let def = s.active_def();
    match &def.end {
        End::Locked { escape } => {
            if !hostiles_left(g, &def) { end(g, "Victory", false); return true }
            if escape.is_some_and(|z| travellers(g).iter().any(|&i| z.contains(traveller_cell(g, i)))) { end(g, "You got away", true); return true }
            false
        }
        End::Open { rounds } => {
            if !round_end { return false }
            let quiet = !npc_side(g).into_iter().any(|k| willing(g, k) && !routed(g, k) && sees(g, k).is_some());
            let s = st(g).unwrap(); let e = s.encounter.as_mut().unwrap();
            e.quiet = if quiet { e.quiet + 1 } else { 0 };
            if e.quiet >= *rounds { end(g, "Encounter over", true); return true }
            false
        }
    }
}
fn end(g: &mut Ground, text: &str, alert: bool) {
    fire::settle_all(g);
    let def = rd(g).unwrap().active_def();
    let here = (g.mem.x() >> 1, g.mem.y() >> 1);
    let took_part: Vec<u8> = { let s = rd(g).unwrap(); npc_side(g).into_iter().chain(s.npcs.iter().filter(|(_, n)| n.engaged).map(|(&k, _)| k)).collect() };
    for k in took_part {
        let engaged = rd(g).unwrap().npcs.get(&k).is_some_and(|n| n.engaged);
        if !alert || !npc_present(g, k) || routed(g, k) || !(hostile(g, k) || engaged) { continue }
        let n = st(g).unwrap().npcs.entry(k).or_default(); n.alerted = Some(def.alerted_rounds); n.alert_full = def.alerted_rounds; n.last_seen = Some(here);
    }
    let gone: Vec<u8> = rd(g).unwrap().npcs.keys().copied().filter(|&k| !npc_present(g, k)).collect();
    for n in st(g).unwrap().npcs.values_mut() { n.engaged = false; n.acted = false }
    for k in gone { st(g).unwrap().npcs.get_mut(&k).unwrap().alerted = None }
    set_encounter(g, None);
    let count = alerted_count(g);
    let s = st(g).unwrap();
    s.log(text.into());
    s.strip = if count > 0 { format!("{text}. Alerted: {count}") } else { text.into() };
    s.explore_ticks = 0; s.walked = 0;
    if g.mem.combat() != 0 {
        let sel = selected(g);
        if !conscious(g, sel) { if let Some(i) = marching_order(g).into_iter().find(|&i| conscious(g, i)) { select(g, i) } }
        let fired = g.mem.party_fired();
        g.mem.set_individual(0); commands_party::regroup(g);
        g.mem.set_party_fired(fired);
    }
    g.mem.set_redraw_pending(1);
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::{rules::compendium::{Compendium, NPCS_TOML}, state::{Far, MAP_PTR}};
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    const ROWS: i16 = 36; const COLS: i16 = 60;
    fn ground() -> Ground {
        let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.alloc(0x8000, ROWS as usize * COLS as usize * 3); g.mem.set_pointer(MAP_PTR, Far::new(0, 0x8000));
        for i in 0..ROWS as usize * COLS as usize { g.mem.write(Far::new((i * 3 + 1) as u16, 0x8000), 0x20, 1) }
        g.mem.set_map_stride(COLS); g.mem.set_map_blocks_high((ROWS / 12) as u8); g.mem.set_u8(0x5C3B, (COLS / 20) as u8);
        g.mem.alloc(0x9100, 46 * 140); g.mem.set_pointer(0x6D6D, Far::new(0, 0x9100));
        g.mem.set_party_u8(0, 0x1F); for i in 0..5 { g.mem.set_party_u8(1 + i, i as u8) }
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        for i in 0..5u16 { let r = 0x102 + 256 * i; for (k, b) in format!("Trav{}", i + 1).bytes().enumerate() { g.mem.set_party_u8(r + k as u16, b) } g.mem.set_party_u8(r + 0x23, 21); g.mem.set_party_u8(r + 0x24, 0); g.mem.set_party_u8(r + 0x25, 21); }
        g.mem.set_x(44); g.mem.set_y(10); g.mem.set_view_x(38); g.mem.set_view_y(0); g.mem.set_outdoors(1); g.mem.set_npc_count(32);
        g.squad = Some(squad::State::with_compendium(Compendium::parse(NPCS_TOML).unwrap()));
        g
    }
    fn place(g: &mut Ground, k: u8, (row, col): (i16, i16), attitude: u8, weapon: u8, picture: u8) {
        let n = Mem::npc(k as i16);
        g.mem.write(n.add(2), 0, 1); g.mem.write(n.add(9), picture as u32, 1); g.mem.write(n.add(10), 12, 1); g.mem.write(n.add(11), 3, 1); g.mem.write(n.add(13), attitude as u32, 1);
        g.mem.write(n.add(31), weapon as u32, 1); g.mem.write(n.add(0x1E), 50, 1);
        g.mem.write(n.add(N_X), (row * 2) as u16 as u32, 2); g.mem.write(n.add(N_Y), (col * 2) as u16 as u32, 2);
        g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 2) as u16, 0x8000), k as u32, 1);
    }
    fn wall(g: &mut Ground, (row, col): (i16, i16)) { g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 1) as u16, 0x8000), 0, 1) }
    fn p1(g: &Ground) -> &State { &g.squad.as_ref().unwrap().p1 }
    fn set_tactics(g: &mut Ground, i: u16, level: u8) { g.mem.set_party_u8(256 * i + 0x12B + adapters::SKILL_TACTICS as u16, level) }

    #[test]
    fn exploration_clock_and_sight_start() {
        let mut g = ground();
        assert_eq!(phases(&g), PHASE_ROUTES | PHASE_WANDER);
        g.squad = None; assert_eq!(phases(&g), PHASES_ALL); assert!(clock(&mut g).is_none(), "no squad state: the clock runs free");
        let mut g = ground();
        pass(&mut g, &[]); assert!(p1(&g).encounter.is_none() && p1(&g).strip.is_empty(), "no NPC: exploration");
        assert_eq!(clock(&mut g), Some(0)); assert_eq!(clock(&mut g), Some(0));
        walked(&mut g, 5); assert_eq!(p1(&g).pending, 55);
        assert_eq!(clock(&mut g), Some(18)); assert_eq!(clock(&mut g), Some(36)); assert_eq!(clock(&mut g), Some(54)); assert_eq!(clock(&mut g), Some(55)); assert_eq!(clock(&mut g), Some(55));
        place(&mut g, 17, (22, 10), 2, 14, 41); pass(&mut g, &[]); assert!(p1(&g).encounter.is_none());
        place(&mut g, 14, (22, 11), 0, 14, 41); wall(&mut g, (22, 8)); pass(&mut g, &[]); assert!(p1(&g).encounter.is_none(), "a wall blocks the line of sight");
        place(&mut g, 15, (22, 12), 0, 4, 41); pass(&mut g, &[]); assert!(p1(&g).encounter.is_none(), "column 12: one past the 1990 screen's right edge, no sight start");
        place(&mut g, 16, (18, 5), 0, 4, 41); pass(&mut g, &[]); assert!(p1(&g).encounter.is_none(), "row 18: one above the screen's top");
        g.mem.write(Far::new(((22 * COLS as usize + 8) * 3 + 1) as u16, 0x8000), 0x20, 1);
        pass(&mut g, &[]);
        let e = p1(&g).encounter.clone().expect("sight start");
        assert_eq!((e.def.as_str(), e.round), ("default", 1));
        assert!(p1(&g).log.iter().any(|l| l.starts_with("Encounter!")) && p1(&g).log.iter().any(|l| l.starts_with("Surprise (NPCs rolls)")), "{:?}", p1(&g).log);
        assert!(p1(&g).strip.starts_with("Your move: Trav1") || p1(&g).strip.contains("initiative: NPCs"), "{}", p1(&g).strip);
        assert_eq!(phases(&g), 0); assert_eq!(g.squad.as_ref().unwrap().encounter, true);
        assert_eq!(squad::save_refusal(&g), Some(squad::SAVE_REFUSED));
    }
    #[test]
    fn alternation_pips_round_end_and_open_end() {
        let mut g = ground();
        place(&mut g, 14, (22, 9), 0, 14, 41); place(&mut g, 15, (23, 9), 0, 4, 41);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "fight".into(), surprise: Surprise::None, ..Def::default_sight() });
        pass(&mut g, &[]);
        let e = p1(&g).encounter.clone().unwrap(); assert_eq!(e.def, "fight"); assert_eq!(e.surprised, None); assert_eq!(e.pools, (0, 0)); assert_eq!(e.side, Side::Party);
        assert_eq!(queue(&g).len(), 7); assert_eq!(queue(&g)[0], Unit::Traveller(0)); assert_eq!(queue(&g)[1], Unit::Npc(14), "nearest NPC first");
        assert!(!key(&mut g, 0x48), "a step is allowed while the Move has points");
        order(&mut g, Order::Move); assert!(p1(&g).turns[0].moved && !p1(&g).turns[0].done);
        order(&mut g, Order::Attack); assert!(p1(&g).turns[0].done && p1(&g).turns[0].mp == 0, "attacking after moving forfeits the rest of the move");
        assert!(p1(&g).npcs[&14].acted && p1(&g).log.iter().any(|l| l.contains("NPC 14) fires the") || l.contains("NPC 14) closes in")), "the NPC shot or closed in: {:?}", p1(&g).log);
        assert_eq!(p1(&g).strip, "Your move: Trav2"); assert_eq!(selected(&g), 1);
        select(&mut g, 0); assert!(key(&mut g, 0x48), "a Traveller who acted cannot step"); select(&mut g, 1);
        order(&mut g, Order::Attack); assert!(p1(&g).turns[1].attacked && !p1(&g).turns[1].done && p1(&g).turns[1].mp == MOVE_WALK);
        end_turn(&mut g); assert!(p1(&g).turns[1].done); assert!(p1(&g).npcs[&15].acted); assert_eq!(p1(&g).strip, "Your move: Trav3");
        for _ in 0..3 { end_turn(&mut g) }
        let e = p1(&g).encounter.clone().unwrap(); assert_eq!(e.round, 2); assert_eq!(p1(&g).pending, 108);
        assert!(p1(&g).log.iter().any(|l| l == "Round 2 - initiative: party (Tactics 0 vs 0)"), "{:?}", p1(&g).log);
        assert_eq!(g.squad.as_ref().unwrap().round, 2);
        assert!(!p1(&g).turns.iter().any(|t| t.done || t.moved || t.attacked)); assert!(!p1(&g).npcs[&14].acted);
        for k in [14u8, 15] { g.mem.write(Mem::npc(k as i16).add(2), 20, 1) }
        for _ in 0..5 { end_turn(&mut g); }
        assert!(p1(&g).encounter.is_none(), "{:?}", p1(&g).log); assert_eq!(p1(&g).strip, "Encounter over"); assert_eq!(alerted_count(&g), 0);
        assert_eq!(phases(&g), PHASE_ROUTES | PHASE_WANDER); assert!(squad::save_refusal(&g).is_none());
    }
    #[test]
    fn a_party_with_nobody_conscious_stops_the_rounds() {
        let mut g = ground();
        place(&mut g, 14, (22, 9), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "fight".into(), surprise: Surprise::None, ..Def::default_sight() });
        pass(&mut g, &[]); assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 1);
        for i in 0..5u16 { g.mem.set_party_u8(0x126 + 256 * i, 6); g.mem.set_party_u8(0x127 + 256 * i, 1) }
        let log = p1(&g).log.len();
        for _ in 0..3 { pass(&mut g, &[]) }
        assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 1, "{:?}", p1(&g).log); assert!(!p1(&g).npcs.get(&14).is_some_and(|n| n.acted)); assert_eq!(p1(&g).log.len(), log);
    }
    #[test]
    fn initiative_choice_surprise_and_alert_expiry() {
        let mut g = ground();
        set_tactics(&mut g, 0, 2); set_tactics(&mut g, 2, 1);
        place(&mut g, 14, (22, 9), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.npcs.insert(14, NpcSt { role: Some("none".into()), ..Default::default() });
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "amb".into(), surprise: Surprise::Attackers, attacker: Side::Npcs, ..Def::default_sight() });
        pass(&mut g, &[]);
        let e = p1(&g).encounter.clone().unwrap();
        assert_eq!((e.pools, e.choosing, e.surprised), ((3, 0), true, Some(Side::Party)));
        assert!(p1(&g).strip.starts_with("Round 1 - initiative: party (Tactics 3 vs 0). We go first?"), "{}", p1(&g).strip);
        assert!(p1(&g).turns.iter().all(|t| t.attacked && !t.moved), "surprised: the Attack pips are dark, Move only");
        assert!(key(&mut g, 0x48), "no step while the initiative choice is open");
        assert!(key(&mut g, KEY_N));
        let e = p1(&g).encounter.clone().unwrap(); assert!(!e.choosing); assert_eq!(e.first, Side::Npcs); assert!(p1(&g).npcs[&14].acted);
        assert_eq!(p1(&g).strip, "Your move: Trav1");
        order(&mut g, Order::Attack); assert!(!p1(&g).turns[0].done, "a surprised Traveller's Attack order books nothing new");
        for _ in 0..5 { end_turn(&mut g); }
        assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
        assert!(p1(&g).log.iter().any(|l| l.starts_with("Alarm (party)") || l == "The alarm is raised: surprise lost"), "the round-end roll, or the NPC's unsilenced shot earlier: {:?}", p1(&g).log);
        let mut g = ground(); g.mem.set_outdoors(0);
        place(&mut g, 14, (22, 9), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.npcs.insert(14, NpcSt { skills: [("Tactics".to_string(), 2)].into_iter().collect(), ..Default::default() });
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "n".into(), surprise: Surprise::None, ..Def::default_sight() });
        pass(&mut g, &[]);
        let e = p1(&g).encounter.clone().unwrap(); assert_eq!((e.pools, e.first, e.choosing), ((0, 2), Side::Npcs, false));
        assert!(p1(&g).log.iter().any(|l| l == "Round 1 - initiative: NPCs (Tactics 0 vs 2)"), "{:?}", p1(&g).log);
        assert!(p1(&g).npcs[&14].acted, "the NPC acted first, no prompt");
        walk::set_player_pos(&mut g, 35, 59);
        for _ in 0..6 { end_turn(&mut g); }
        assert!(p1(&g).encounter.is_none(), "{:?}", p1(&g).log); assert_eq!(p1(&g).npcs[&14].alerted, Some(ALERT_ROUNDS)); assert_eq!(p1(&g).strip, "Encounter over. Alerted: 1");
        let before = squares(npc_cell(&g, 14), (35, 59));
        walked(&mut g, 1); pass(&mut g, &[]);
        assert_eq!(squares(npc_cell(&g, 14), (35, 59)), before - 1, "one step toward the party"); assert_eq!(p1(&g).strip, "Alerted: 1");
        g.mem.write(Mem::npc(14).add(7), 4, 1);
        for _ in 0..10 { walked(&mut g, 10); pass(&mut g, &[]); }
        assert_eq!(p1(&g).npcs[&14].alerted, None, "{:?}", p1(&g).log); assert_eq!(p1(&g).strip, "");
    }
    #[test]
    fn locked_area_trigger_victory_and_escape() {
        let mut g = ground();
        g.mem.set_outdoors(0);
        place(&mut g, 14, (8, 52), 0, 14, 41); place(&mut g, 16, (9, 52), 0, 14, 41);
        let def = Def { id: "road".into(), start: Start::Area(Rect { r0: 7, c0: 21, r1: 9, c1: 21 }), end: End::Locked { escape: Some(Rect { r0: 7, c0: 35, r1: 9, c1: 37 }) }, boundary: Boundary::Level, participants: Some(vec![14, 16]), surprise: Surprise::None, ..Def::default_sight() };
        g.squad.as_mut().unwrap().p1.defs.push(def);
        walk::set_player_pos(&mut g, 8, 20); pass(&mut g, &[]); assert!(p1(&g).encounter.is_none(), "beside the trigger: nothing");
        g.mem.set_y(42); pass(&mut g, &[]);
        let e = p1(&g).encounter.clone().unwrap(); assert_eq!(e.def, "road"); assert!(e.grouped); assert!(locked(&g)); assert_eq!(p1(&g).strip, "Your move: Trav1");
        assert!(p1(&g).log[0].starts_with("Encounter! No way out but through."));
        assert!(p1(&g).done.contains(&"road".to_string()), "an area trigger fires once");
        assert_eq!(npc_side(&g), vec![14, 16], "level boundary: both take turns though 31 squares off");
        for _ in 0..10 { end_turn(&mut g); }
        assert!(p1(&g).encounter.is_some()); assert!(squares(npc_cell(&g, 14), (8, 21)) < 31, "NPC 14 walked toward the party: {:?}", npc_cell(&g, 14));
        g.mem.set_y(72); end_turn(&mut g);
        assert!(p1(&g).encounter.is_none()); assert_eq!(p1(&g).strip, "You got away. Alerted: 2"); assert_eq!(alerted_count(&g), 2);
        assert!(!locked(&g));
        let (nr, nc) = npc_cell(&g, 14); walk::set_player_pos(&mut g, nr, nc - 3);
        pass(&mut g, &[]);
        assert!(p1(&g).encounter.is_some(), "{:?}", p1(&g).log); assert!(p1(&g).log.iter().any(|l| l.starts_with("No surprise: they were expecting you")));
        assert_eq!(p1(&g).encounter.as_ref().unwrap().def, "default");
        for k in [14u8, 16] { g.mem.write(Mem::npc(k as i16).add(2), 20, 1) }
        for _ in 0..5 { end_turn(&mut g); }
        assert!(p1(&g).encounter.is_none(), "{:?}", p1(&g).log); assert_eq!(p1(&g).strip, "Encounter over"); assert_eq!(alerted_count(&g), 0, "dead NPCs are not alerted");
        let mut g = ground(); g.mem.set_outdoors(0);
        place(&mut g, 14, (8, 30), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "v".into(), start: Start::Area(Rect { r0: 7, c0: 21, r1: 9, c1: 21 }), end: End::Locked { escape: None }, boundary: Boundary::Level, participants: Some(vec![14]), surprise: Surprise::None, ..Def::default_sight() });
        g.mem.set_x(16); g.mem.set_y(42); pass(&mut g, &[]); assert!(locked(&g));
        g.mem.write(Mem::npc(14).add(2), 20, 1);
        end_turn(&mut g);
        assert!(p1(&g).encounter.is_none()); assert_eq!(p1(&g).strip, "Victory"); assert_eq!(alerted_count(&g), 0);
    }
    #[test]
    fn keys_outside_an_encounter_and_hold_release() {
        let mut g = ground();
        assert!(!key(&mut g, KEY_O), "grouped O reaches the 1990 marching-order command");
        g.mem.set_combat(1); assert!(key(&mut g, KEY_O), "split O never enters orders mode"); g.mem.set_combat(0);
        assert!(!key(&mut g, KEY_SPACE)); assert!(!key(&mut g, 0x48));
        place(&mut g, 14, (22, 9), 0, 14, 41);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "h".into(), surprise: Surprise::None, ..Def::default_sight() });
        pass(&mut g, &[Intent::Select(3)]);
        assert!(p1(&g).encounter.is_some()); assert_eq!(selected(&g), 3);
        pass(&mut g, &[Intent::Verb { verb: crate::ui::Verb::Hold, target: crate::ui::Target::Cell((0, 0)) }]);
        assert!(p1(&g).turns[3].hold, "Trav4 holds");
        for s in 1..20000u32 { g.rand.lcg.seed = s; let (a, b) = squad::two_d(&mut g); if a + b <= 5 { g.rand.lcg.seed = s; break } }
        pass(&mut g, &[Intent::Select(1), Intent::EndTurn]);
        assert!(p1(&g).turns[1].done && p1(&g).npcs[&14].acted);
        for _ in 0..3 { end_turn(&mut g); }
        assert!(p1(&g).log.iter().any(|l| l == "Trav4: hold released"), "{:?}", p1(&g).log);
        assert_eq!(p1(&g).strip, "Your move: Trav4"); assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 1);
        end_turn(&mut g); assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
        assert!(line_of_sight(&g, (0, 0), (35, 59))); wall(&mut g, (10, 17)); assert!(!line_of_sight(&g, (0, 0), (35, 59)));
    }

    #[test]
    fn surprise_thresholds_11_10_and_9() {
        let def = Def { surprise: Surprise::Roll, attacker: Side::Npcs, ..Def::default_sight() };
        for (sum, want) in [(11, Some(Side::Party)), (12, Some(Side::Party)), (10, None), (9, Some(Side::Npcs)), (2, Some(Side::Npcs))] {
            let mut g = ground();
            let seed = (1..20000u32).find(|&s| { g.rand.lcg.seed = s; let (a, b) = squad::two_d(&mut g); a + b == sum }).expect("a seed for the sum");
            g.rand.lcg.seed = seed;
            assert_eq!(surprise(&mut g, &def), want, "2D = {sum}: {:?}", p1(&g).log);
            assert!(p1(&g).log.iter().any(|l| l.starts_with("Surprise (NPCs rolls): 2D ") && l.ends_with(&format!("= {sum} vs 11"))), "{:?}", p1(&g).log);
        }
    }
}
