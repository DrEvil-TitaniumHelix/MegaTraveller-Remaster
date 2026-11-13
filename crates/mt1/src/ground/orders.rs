use super::{commands, fire as p20, render, round::{self, Order, Side}, rules::{self, adapters, weapons::{Profile, Recoil}}, standing, state::{Ground, Mem, N_ATTITUDE, W_ACTIVE, W_AIM_X, W_AIM_Y, W_HAND, W_ROUTE_LENGTH, W_TARGET}, tactics, walk};
use crate::ui::{Cell, Intent, Target, Unit, Verb};
use std::collections::{BTreeMap, VecDeque};
#[cfg(test)]
mod regressions;

pub const ROUTE_MAX: usize = 62;
const SCAN_C: u8 = 0x2E; const SCAN_I: u8 = 0x17; const SCAN_P: u8 = 0x19; const SCAN_V: u8 = 0x2F; const SCAN_F1: u8 = 0x3B;
const SCAN_F: u8 = 0x21; const SCAN_ENTER: u8 = 0x1C; const SCAN_R: u8 = 0x13;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Speed { Auto, Run, Evade, Walk }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op { Move(Speed), HoldHere, Fire, SetTarget, Converse, Take, Select(u8), Reload, SwapHand, FireMode, Hold, Character(u8), Party, Vehicle, Later(&'static str),
    FullAuto, Rapid, Pinpoint, Melee, Disarm, Throw, Launch,
    Advance, HoldFire, HoldRun, Tactics,
    Look }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem { pub key: u8, pub label: String, pub op: Op, pub grey: Option<String> }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu { pub target: Target, pub items: Vec<MenuItem>, pub default: usize }
pub const PATH_WALK: u8 = 0; pub const PATH_RUN: u8 = 1; pub const PATH_LATER: u8 = 2; pub const PATH_BLOCKED: u8 = 3;
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State { pub full_auto: [bool; 5], pub menu: Option<Menu>, pub hover: String, pub path: Vec<(Cell, u8)>,
    pub target: Option<Target>,
    pub hovered: Option<Target> }

fn st(g: &mut Ground) -> Option<&mut State> { g.squad.as_mut().map(|s| &mut s.orders) }
pub(crate) fn log(g: &mut Ground, line: String) { if let Some(s) = g.squad.as_mut() { s.p1.log(line) } }
pub(crate) fn say(g: &mut Ground, text: &str) { log(g, text.to_string()); if let Some(s) = st(g) { s.hover = text.to_string() } g.mem.set_redraw_pending(1) }
fn walker(who: u8) -> super::state::Far { Mem::walker(who as i16) }
fn npc_byte(g: &Ground, k: u8, off: i32) -> u8 { g.mem.read(Mem::npc(k as i16).add(off), 1) as u8 }

pub fn intent(g: &mut Ground, i: Intent) {
    match i {
        Intent::Select(t) => select(g, t),
        Intent::EndTurn => standing::end_turn(g),
        Intent::Continue => { standing::run(g); }
        Intent::Hover(t) => hover(g, t),
        Intent::OpenMenu { target, .. } => { let m = menu_for(g, target); if let Some(s) = st(g) { s.menu = Some(m) } g.mem.set_redraw_pending(1) }
        Intent::CloseMenu => { if let Some(s) = st(g) { if s.menu.take().is_some() { g.mem.set_redraw_pending(1) } } }
        Intent::MenuPick(n) => { round::fx_begin(g, Unit::Traveller(round::selected(g))); pick(g, n) }
        Intent::Verb { verb, target } => { round::fx_begin(g, Unit::Traveller(round::selected(g))); default_verb(g, verb, target) }
        _ => {}
    }
}
pub fn key(g: &mut Ground, scan: u8) -> bool {
    if g.squad.is_none() { return false }
    if matches!(scan, SCAN_F | SCAN_ENTER) && g.mem.combat() != 0 { round::fx_begin(g, Unit::Traveller(round::selected(g))) }
    if scan == SCAN_ENTER && p20::rapid_enter(g) { return true }
    match scan {
        2..=6 if g.mem.combat() != 0 => { select(g, scan - 2); true }
        SCAN_F | SCAN_ENTER if g.mem.combat() != 0 => {
            let who = round::selected(g);
            match g.mem.read(walker(who).add(W_TARGET), 1) as u8 { 0 => say(g, "no target: press Tab"), k => fire_input(g, k) }
            true
        }
        SCAN_R if g.mem.combat() != 0 => { reload(g); true }
        _ => false,
    }
}

pub fn select(g: &mut Ground, t: u8) {
    if t > 4 || g.squad.is_none() { return }
    if !round::conscious(g, t) { let n = round::name(g, t); say(g, &format!("{n} is down")); return }
    if let Some((_, w)) = tactics::interrupting(g) { if w != t { let n = round::name(g, w); say(g, &format!("{n} interrupts: finish their turn first")); return } }
    let route = read_route(g, t);
    round::select(g, t);
    write_route(g, t, &route);
    if g.mem.combat() != 0 { render::select_panel(g, 10, g.mem.u8(0x111 + t as u16)); render::walker_panel(g, false, true) }
    if let Some(s) = st(g) { s.hover.clear(); s.path.clear() }
    g.mem.set_redraw_pending(1);
}
pub fn read_route(g: &Ground, who: u8) -> Vec<u8> {
    let w = walker(who); let n = (g.mem.read(w.add(W_ROUTE_LENGTH), 1) as usize).min(ROUTE_MAX);
    (0..n).map(|i| g.mem.read(w.add(W_ROUTE_LENGTH + 1 + i as i32), 1) as u8).collect()
}
pub fn write_route(g: &mut Ground, who: u8, route: &[u8]) {
    let w = walker(who); let route = &route[..route.len().min(ROUTE_MAX)];
    g.mem.write(w.add(W_ROUTE_LENGTH), route.len() as u32, 1);
    for (i, d) in route.iter().enumerate() { g.mem.write(w.add(W_ROUTE_LENGTH + 1 + i as i32), *d as u32, 1) }
}

fn attitude(g: &Ground, k: u8) -> &'static str { ["Hostile", "Police", "Neutral", "Neutral"][(npc_byte(g, k, N_ATTITUDE) & 3) as usize] }
pub fn hover(g: &mut Ground, t: Option<Target>) {
    let Some(sq) = g.squad.as_mut() else { return };
    sq.orders.hovered = t;
    if sq.orders.menu.is_some() { return }
    let who = round::selected(g);
    let (text, path) = match t {
        None => (String::new(), Vec::new()),
        Some(Target::Cell(c)) if p20::aims_at_squares(g, who) => (p20::preview_square(g, who, c), p20::ring(g, who, c)),
        Some(Target::Cell(c)) => { let (text, path) = preview_move(g, who, c); (named(g, c, text), path) }
        Some(Target::Unit(Unit::Traveller(i))) => { let n = round::name(g, i); let acted = round::in_encounter(g) && g.squad.as_ref().is_some_and(|s| s.p1.turns[(i as usize).min(4)].done); (if !round::conscious(g, i) { format!("{n}: down") } else if acted { format!("{n}: has acted") } else { n }, Vec::new()) }
        Some(Target::Unit(Unit::Npc(k))) => (preview_npc(g, who, k), p20::ring(g, who, round::npc_cell(g, k))),
        Some(Target::Object(_)) => ("Take".into(), Vec::new()),
        Some(Target::Portrait(i)) => (round::name(g, i), Vec::new()),
    };
    let s = st(g).unwrap(); s.target = t;
    if s.hover != text || s.path != path { s.hover = text; s.path = path; g.mem.set_redraw_pending(1) }
}
pub fn trigger_name(flags: u8, tile: u8) -> Option<&'static str> {
    if flags & 1 != 0 { return None }
    Some(match tile { 0..=4 => "Building entrance", 5 => "Starport", 6 => "Vehicle rental", 7 => "Casino", 8 => "Library", 9 => "Travellers' Aid Society",
        10 => "Hospital", 11 => "Weapons store", 12 => "Armor shop", 13 => "General store", 14 => "Imperial base", 38 | 39 => "Police post", _ => return None })
}
fn named(g: &Ground, (r, c): Cell, text: String) -> String {
    if r < 0 || c < 0 || r >= g.mem.map_blocks_high() as i16 * 12 || c >= g.mem.map_stride() { return text }
    let cell = commands::cell(g, r, c);
    match trigger_name(g.mem.read(cell, 1) as u8, g.mem.read(cell.add(1), 1) as u8) { Some(n) if text.is_empty() => n.into(), Some(n) => format!("{n}: {text}"), None => text }
}
fn preview_npc(g: &Ground, who: u8, k: u8) -> String {
    if !round::npc_present(g, k) { return String::new() }
    let head = round::described(g, k, attitude(g, k));
    match fire_check(g, who, k) {
        Ok(shot) => format!("{head}: {}{}", shot.describe(), p20::line_warning(g, who, k)),
        Err(reason) => format!("{head}: {reason}"),
    }
}
fn squares(n: i32) -> String { format!("{n} square{}", if n == 1 { "" } else { "s" }) }
fn preview_move(g: &Ground, who: u8, to: Cell) -> (String, Vec<(Cell, u8)>) {
    let from = round::traveller_cell(g, who);
    if from == to { return (String::new(), Vec::new()) }
    let Some(path) = find_path(g, who, from, to) else { return ("no path".into(), vec![(to, PATH_BLOCKED)]) };
    let n = path.len() as i32;
    if !round::in_encounter(g) { return (squares(n), path.into_iter().map(|c| (c, PATH_WALK)).collect()) }
    let t = g.squad.as_ref().unwrap().p1.turns[who as usize];
    if t.done { return (format!("{}: has acted", round::name(g, who)), path.into_iter().map(|c| (c, PATH_LATER)).collect()) }
    let (walk, run) = if t.moved { (t.mp, t.mp) } else { (round::MOVE_WALK, round::MOVE_RUN) };
    let cost = if t.evade { 2 } else { 1 };
    let (walk, run) = (walk / cost, run / cost);
    let classed = path.iter().enumerate().map(|(i, &c)| (c, if (i as i32) < walk { PATH_WALK } else if (i as i32) < run { PATH_RUN } else { PATH_LATER })).collect();
    let text = if n <= walk { format!("Walk {}", squares(n)) }
        else if n <= run { format!("Run {}: -2 to be hit, -2 to your shots", squares(n)) }
        else { format!("{} rounds", (n + run - 1) / run.max(1)) };
    (text, classed)
}

fn item(key: u8, label: &str, op: Op, grey: Option<String>) -> MenuItem { MenuItem { key, label: label.into(), op, grey } }
fn later(key: u8, label: &str, piece: &'static str) -> MenuItem { item(key, label, Op::Later(piece), Some("not yet".into())) }
pub fn menu_for(g: &Ground, target: Target) -> Menu {
    let who = round::selected(g);
    let broken = g.mem.combat() != 0;
    let encounter = round::in_encounter(g);
    let turn = turn_check(g);
    let grey_turn = |g: &Ground| turn.err().map(|r| r.to_string()).or_else(|| (encounter && g.squad.as_ref().unwrap().p1.turns[who as usize].attacked).then(|| "no Attack left".to_string()));
    let mut items = Vec::new();
    match target {
        Target::Cell(c) => {
            let path = find_path(g, who, round::traveller_cell(g, who), c);
            let grey_move = |g: &Ground| move_check(g, who).err().map(String::from).or_else(|| path.is_none().then(|| "no path".to_string()));
            items.push(item(b'M', "Move", Op::Move(Speed::Auto), grey_move(g)));
            items.push(item(b'N', "Run here", Op::Move(Speed::Run), grey_move(g)));
            items.push(item(b'E', "Evade to here", Op::Move(Speed::Evade), grey_move(g)));
            items.push(item(b'H', "Hold here", Op::HoldHere, None));
            items.push(item(b'T', "Throw grenade here", Op::Throw, p20::throw_check(g, who, c).err()));
            if adapters::traveller_weapon(g, who).is_some_and(|w| w.profile == Profile::Indirect) { items.push(item(b'L', "Fire launcher here", Op::Launch, p20::launch_check(g, who, c).err())) }
            items.push(item(b'A', "Advance", Op::Advance, grey_move(g)));
        }
        Target::Unit(Unit::Npc(k)) => {
            let neutral = !attackable(g, k);
            let fire_grey = offscreen_target(g, k).map(String::from).or_else(|| fire_check(g, who, k).err());
            let dry = chamber_empty(g, who);
            if neutral {
                let talk = if broken { Some("regroup to talk".to_string()) } else { None };
                items.push(item(b'C', "Converse", Op::Converse, talk.or_else(|| grey_turn(g))));
            }
            items.push(item(b'F', "Attack", Op::Fire, fire_grey.clone()));
            let near = round::squares(round::traveller_cell(g, who), round::npc_cell(g, k)) <= 1;
            items.push(item(b'W', "Walk to", Op::Move(Speed::Auto), if near { Some("adjacent already".into()) } else { move_check(g, who).err().map(String::from).or_else(|| beside(g, who, k).is_none().then(|| "no path".to_string())) }));
            items.push(item(b'L', "Look", Op::Look, None));
            let w = adapters::traveller_weapon(g, who);
            let gun = w.is_some_and(|w| matches!(w.profile, Profile::Handgun | Profile::Rifle));
            if dry && gun { items.push(item(b'R', "Reload", Op::Reload, reload_check(g, who).err())) }
            let full = g.squad.as_ref().is_some_and(|s| s.orders.full_auto[who as usize]);
            items.push(item(b'U', "Full auto", Op::FullAuto, match w { Some(w) if gun && w.auto > 0 => if full { fire_grey.clone().or_else(|| p20::burst_check(g, who).err()) } else { Some("switch to full auto first (Q)".into()) }, _ => Some("no autofire".into()) }));
            items.push(item(b'P', "Rapid fire", Op::Rapid, if gun && w.is_some_and(|w| w.rapid) { fire_check_opts(g, who, k, p20::Opts { rapid: true, ..Default::default() }).err() } else { Some("no rapid fire".into()) }));
            items.push(item(b'I', "Pinpoint", Op::Pinpoint, if gun { fire_check_opts(g, who, k, p20::Opts { pinpoint: true, ..Default::default() }).err() } else { Some("no pinpoint with this weapon".into()) }));
            items.push(item(b'K', "Melee", Op::Melee, p20::melee_check(g, who, k).err()));
            items.push(item(b'D', "Disarm", Op::Disarm, p20::melee_check(g, who, k).err()));
            items.push(item(b'T', "Throw", Op::Throw, p20::throw_check(g, who, round::npc_cell(g, k)).err()));
            items.push(item(b'A', "Target", Op::SetTarget, (!round::npc_present(g, k)).then(|| "gone".to_string())));
            let (next, cap, left) = (tactics::declared(g) + 1, tactics::cap(g), tactics::left(g));
            items.push(item(b'+', &format!("Tactics +{next}"), Op::Tactics, if !encounter { Some("not in a fight".into()) } else if next > cap { Some(format!("maximum {cap} (the highest single level)")) } else if left <= 0 { Some("pool empty".into()) } else { None }));
        }
        Target::Unit(Unit::Traveller(i)) => {
            if !broken {
                items.push(item(b'B', "Break", Op::Party, (g.mem.encounter_mode() != 0).then(|| "in a vehicle".to_string())));
                items.push(later(b'U', "Use item", "P15"));
                items.push(item(b'C', "Character", Op::Character(i), None));
                if g.mem.encounter_mode() != 0 { items.insert(0, item(b'L', "Leave / Status", Op::Vehicle, None)) }
            } else {
                items.push(item(b'S', "Select", Op::Select(i), select_check(g, i).err().map(String::from)));
                let standing = g.mem.read(walker(i).add(W_TARGET), 1) as u8;
                items.push(item(b'F', "Fire", Op::Fire, select_check(g, i).err().map(String::from).or_else(|| if standing == 0 { Some("no target".into()) } else { offscreen_target(g, standing).map(String::from).or_else(|| fire_check(g, i, standing).err()) })));
                items.push(item(b'R', "Reload", Op::Reload, reload_check(g, i).err()));
                items.push(item(b'W', "Swap hand", Op::SwapHand, select_check(g, i).err().map(String::from)));
                items.push(item(b'Q', "Fire mode", Op::FireMode, fire_mode_check(g, i).err()));
                items.push(later(b'D', "Hide", "P22"));
                items.push(item(b'H', "Hold", Op::Hold, tactics::hold_check(g, i).err()));
                items.push(item(b'N', "Hold (run)", Op::HoldRun, tactics::hold_check(g, i).err()));
                items.push(item(b'X', "Hold fire", Op::HoldFire, select_check(g, i).err().map(String::from).or_else(|| (g.mem.read(walker(i).add(W_TARGET), 1) == 0).then(|| "no target".to_string()))));
                items.push(later(b'U', "Use item", "P15"));
                items.push(later(b'G', "Give to", "P16"));
                items.push(item(b'C', "Character", Op::Character(i), None));
                items.push(item(b'P', "Regroup", Op::Party, encounter.then(|| "not in a fight".to_string())));
            }
        }
        Target::Object(_) => {
            let cell = object_cell(g, target);
            let path = cell.and_then(|c| if c == round::traveller_cell(g, who) { Some(Vec::new()) } else { find_path(g, who, round::traveller_cell(g, who), c) });
            let grey = grey_turn(g).or_else(|| path.is_none().then(|| "no path".to_string()));
            items.push(item(b'T', "Take", Op::Take, grey.clone()));
            items.push(item(b'U', "Use", Op::Take, grey));
            items.push(item(b'M', "Move here", Op::Move(Speed::Auto), cell.and_then(|c| if c == round::traveller_cell(g, who) { Some("here".to_string()) } else { move_check(g, who).err().map(String::from).or_else(|| path.is_none().then(|| "no path".to_string())) })));
        }
        Target::Portrait(i) => { items.push(item(b'S', "Select", Op::Select(i), (!round::conscious(g, i)).then(|| "down".to_string()))); items.push(item(b'C', "Character", Op::Character(i), None)) }
    }
    let default = match target {
        Target::Cell(_) if p20::aims_at_squares(g, who) => items.iter().position(|i| i.op == Op::Throw).unwrap_or(0),
        Target::Unit(Unit::Npc(k)) if attackable(g, k) && chamber_empty(g, who) && !p20::aims_at_squares(g, who) => items.iter().position(|i| i.op == Op::Reload).unwrap_or(0),
        Target::Unit(Unit::Npc(k)) if attackable(g, k) => items.iter().position(|i| i.op == Op::Fire).unwrap_or(0),
        Target::Unit(Unit::Npc(_)) if !broken => items.iter().position(|i| i.op == Op::Converse).unwrap_or(0),
        Target::Unit(Unit::Npc(_)) => items.iter().position(|i| i.op == Op::SetTarget).unwrap_or(0),
        _ => 0,
    };
    Menu { target, items, default }
}
pub fn pick(g: &mut Ground, n: u8) {
    let Some(menu) = st(g).and_then(|s| s.menu.take()) else { return };
    g.mem.set_redraw_pending(1);
    let Some(it) = menu.items.get(n as usize).cloned() else { return };
    if let Some(reason) = &it.grey { say(g, &format!("{}: {reason}", it.label)); return }
    perform(g, it.op, menu.target)
}
pub fn default_verb(g: &mut Ground, verb: Verb, target: Target) {
    match (verb, target) {
        (Verb::Move, Target::Cell(c)) => { if !p20::at_square(g, c) { move_to(g, c, Speed::Auto) } }
        (Verb::Fire, Target::Unit(Unit::Npc(k))) => {
            let who = round::selected(g);
            if p20::rapid_add(g, k) { return }
            if !attackable(g, k) { say(g, &format!("{}: right-click for the menu", attitude(g, k))); return }
            if let Some(reason) = offscreen_target(g, k) { say(g, reason); return }
            if chamber_empty(g, who) && !p20::aims_at_squares(g, who) { reload(g) } else { fire(g, k) }
        }
        (Verb::Converse, Target::Unit(Unit::Npc(_))) => perform(g, Op::Converse, target),
        (Verb::Take, _) => perform(g, Op::Take, target),
        (Verb::Hold, _) => perform(g, Op::Hold, target),
        (Verb::Give, _) => say(g, "Give: not yet"),
        _ => {}
    }
}
pub fn perform(g: &mut Ground, op: Op, target: Target) {
    if matches!(op, Op::Reload | Op::SwapHand | Op::FireMode | Op::Hold | Op::HoldRun | Op::HoldFire) {
        if let Target::Unit(Unit::Traveller(i)) | Target::Portrait(i) = target {
            if let Err(reason) = select_check(g, i) { say(g, reason); return }
            select(g, i);
            round::fx_begin(g, Unit::Traveller(i));
        }
    }
    match op {
        Op::Move(speed) => { if let Some(c) = match target { Target::Cell(c) => Some(c), Target::Unit(Unit::Npc(k)) => beside(g, round::selected(g), k), t => object_cell(g, t) } { move_to(g, c, speed) } }
        Op::Look => { if let Target::Unit(Unit::Npc(k)) = target { let t = preview_npc(g, round::selected(g), k); say(g, &t) } }
        Op::HoldHere => { let who = round::selected(g); write_route(g, who, &[]); standing::route_cleared(g, who); let n = round::name(g, who); say(g, &format!("{n} holds here")) }
        Op::Fire => match target {
            Target::Unit(Unit::Npc(k)) => fire_input(g, k),
            Target::Unit(Unit::Traveller(_)) | Target::Portrait(_) => match g.mem.read(walker(round::selected(g)).add(W_TARGET), 1) as u8 { 0 => say(g, "no target: press Tab"), k => fire_input(g, k) },
            _ => {}
        }
        Op::SetTarget => { if let Target::Unit(Unit::Npc(k)) = target { set_target(g, k) } }
        Op::Converse => {
            if g.mem.combat() != 0 { say(g, "regroup to talk"); return } if let Err(r) = turn_check(g) { say(g, r); return }
            if let Target::Unit(Unit::Npc(k)) = target {
                let (from, to) = (round::traveller_cell(g, round::selected(g)), round::npc_cell(g, k)); let d = (to.0 - from.0, to.1 - from.1);
                if d.0.abs().max(d.1.abs()) != 1 { let l = super::fire::label(g, Unit::Npc(k)); say(g, &format!("{l}: not adjacent")); return }
                if let Some(dir) = direction(g, d) { if dir as i16 != g.mem.heading() { walk::move_party(g, d.0, d.1, dir as i16); } }
            }
            post_key(g, SCAN_C); attack_substitute(g)
        }
        Op::Take => take(g, target),
        Op::Select(i) => select(g, i),
        Op::Reload => reload(g),
        Op::SwapHand => swap_hand(g),
        Op::FireMode => fire_mode(g),
        Op::Hold => tactics::hold(g, 1),
        Op::HoldRun => tactics::hold(g, 2),
        Op::HoldFire => standing::hold_fire(g),
        Op::Advance => { if let Target::Cell(c) = target { standing::advance(g, c) } }
        Op::Tactics => { tactics::declare(g, 1); }
        Op::Character(i) => post_key(g, SCAN_F1 + (i.min(4))),
        Op::Party => { if round::in_encounter(g) && g.mem.combat() != 0 { say(g, "not in a fight"); return } post_key(g, SCAN_P) }
        Op::Vehicle => post_key(g, SCAN_V),
        Op::Later(piece) => say(g, &format!("not yet ({piece})")),
        Op::FullAuto => { if let Target::Unit(Unit::Npc(k)) = target { p20::shoot(g, k, p20::Opts { burst: true, ..Default::default() }) } }
        Op::Rapid => { if let Target::Unit(Unit::Npc(k)) = target { p20::shoot(g, k, p20::Opts { rapid: true, ..Default::default() }) } }
        Op::Pinpoint => { if let Target::Unit(Unit::Npc(k)) = target { p20::shoot(g, k, p20::Opts { pinpoint: true, ..Default::default() }) } }
        Op::Melee => { if let Target::Unit(Unit::Npc(k)) = target { p20::melee(g, k, false) } }
        Op::Disarm => { if let Target::Unit(Unit::Npc(k)) = target { p20::melee(g, k, true) } }
        Op::Throw => { match target { Target::Cell(c) => p20::throw(g, c), Target::Unit(Unit::Npc(k)) => p20::throw(g, round::npc_cell(g, k)), _ => {} } }
        Op::Launch => { if let Target::Cell(c) = target { p20::launch(g, c) } }
    }
}
fn post_key(g: &mut Ground, scan: u8) { g.mem.set_key(scan) }
pub(crate) fn attack_used(g: &Ground) -> &'static str { if round::in_encounter(g) { " - Attack used" } else { "" } }
fn attack_substitute(g: &mut Ground) { if round::in_encounter(g) { round::order(g, Order::Attack) } }
fn beside(g: &Ground, who: u8, k: u8) -> Option<Cell> {
    let (n, from) = (round::npc_cell(g, k), round::traveller_cell(g, who));
    (-1i16..=1).flat_map(|dr| (-1i16..=1).map(move |dc| (n.0 + dr, n.1 + dc))).filter(|&c| c != n && passable(g, who, c))
        .filter_map(|c| find_path(g, who, from, c).map(|p| (p.len(), c))).min().map(|(_, c)| c)
}
fn object_cell(g: &Ground, t: Target) -> Option<Cell> {
    let Target::Object(id) = t else { return None };
    let (rows, cols) = (g.mem.map_blocks_high() as i16 * 12, g.mem.map_stride());
    (0..rows).flat_map(|r| (0..cols).map(move |c| (r, c))).find(|&(r, c)| g.mem.read(commands::cell(g, r, c), 1) as u8 >> 1 == id)
}
fn take(g: &mut Ground, target: Target) {
    let who = round::selected(g);
    let Some(c) = object_cell(g, target) else { say(g, "nothing there"); return };
    if let Err(r) = turn_check(g) { say(g, r); return }
    if c != round::traveller_cell(g, who) { move_to(g, c, Speed::Auto); if c != round::traveller_cell(g, who) { return } }
    post_key(g, SCAN_I); attack_substitute(g)
}

pub(crate) fn turn_check(g: &Ground) -> Result<u8, &'static str> {
    turn_check_for(g, round::selected(g))
}
fn select_check(g: &Ground, who: u8) -> Result<(), &'static str> {
    if who > 4 { return Err("no such Traveller") }
    if !round::conscious(g, who) { return Err("down") }
    if tactics::interrupting(g).is_some_and(|(_, actor)| actor != who) { return Err("finish the interrupting Traveller's turn first") }
    Ok(())
}
pub(crate) fn turn_check_for(g: &Ground, who: u8) -> Result<u8, &'static str> {
    select_check(g, who)?;
    let Some(s) = g.squad.as_ref() else { return Err("Classic 1990 combat") };
    let Some(e) = &s.p1.encounter else { return Ok(who) };
    if e.choosing { return Err("choose who goes first: Space = us, N = them") }
    if e.side != Side::Party { return Err("not your turn") }
    if s.p1.turns[who as usize].done { return Err("has acted") }
    Ok(who)
}
fn move_check(g: &Ground, who: u8) -> Result<u8, &'static str> {
    turn_check(g)?;
    if !round::in_encounter(g) { return Ok(who) }
    let t = g.squad.as_ref().unwrap().p1.turns[who as usize];
    if t.mp <= 0 { return Err("no Move left") }
    if t.attacked && adapters::traveller_weapon(g, who).is_some_and(|w| w.recoil == Recoil::Hi) { return Err("fired: high recoil") }
    Ok(who)
}

pub fn movement_range(g: &Ground, who: u8) -> usize {
    if move_check(g,who).is_err() { return 0 }
    if !round::in_encounter(g) { return ROUTE_MAX }
    let t=g.squad.as_ref().unwrap().p1.turns[who as usize];
    let mp=if t.moved || t.speed!=0 {t.mp} else {round::MOVE_WALK};
    (mp.max(0) / if t.evade {2} else {1}) as usize
}

pub fn reachable(g: &Ground, who: u8, limit: usize) -> Vec<Cell> {
    MovementMap::capture(g,who).reachable(limit)
}
pub struct MovementMap { origin:Cell, rows:i16, cols:i16, cells:Vec<bool>, allowed:bool }
impl MovementMap {
    pub fn capture(g:&Ground,who:u8)->Self {
        let (rows,cols)=(g.mem.map_blocks_high() as i16*12,g.mem.map_stride());
        Self { origin:round::traveller_cell(g,who), rows,cols,allowed:move_check(g,who).is_ok(),
            cells:(0..rows).flat_map(|r|(0..cols).map(move |c|passable(g,who,(r,c)))).collect() }
    }
    pub fn reachable(&self,limit:usize)->Vec<Cell> {
    if !self.allowed {return Vec::new()}
    if limit==0 { return Vec::new() }
    let from=self.origin;
    let mut seen=std::collections::BTreeSet::from([from]);
    let mut queue=VecDeque::from([(from,0usize)]);
    while let Some((c,d))=queue.pop_front() {
        if d>=limit.min(ROUTE_MAX) { continue }
        for dr in -1..=1 { for dc in -1..=1 {
            let n=(c.0+dr,c.1+dc);
            if !seen.contains(&n) && n.0>=0 && n.1>=0 && n.0<self.rows && n.1<self.cols && self.cells[n.0 as usize*self.cols as usize+n.1 as usize] {seen.insert(n);queue.push_back((n,d+1));}
        } }
    }
    seen.remove(&from); seen.into_iter().collect()
    }
}

pub fn occupant(g: &Ground, (r, c): Cell) -> u8 {
    if r < 0 || c < 0 || r >= g.mem.map_blocks_high() as i16 * 12 || c >= g.mem.map_stride() { return 0 }
    g.mem.read(commands::cell(g, r, c).add(2), 1) as u8 & 0x3F
}
pub(super) fn passable(g: &Ground, who: u8, (r, c): Cell) -> bool {
    if r < 0 || c < 0 || r >= g.mem.map_blocks_high() as i16 * 12 || c >= g.mem.map_stride() { return false }
    let cell = commands::cell(g, r, c);
    let (f, t, occ) = (g.mem.read(cell, 1) as u8, g.mem.read(cell.add(1), 1) as u8, g.mem.read(cell.add(2), 1) as u8);
    if (occ & 0x3F != 0 && !(!round::in_encounter(g) && round::neutral(g, occ & 0x3F))) || (g.mem.combat() != 0 && occ & 0x40 != 0) { return false }
    if f & 1 == 0 && (t <= 14 || (g.mem.combat() != 0 && (t == 38 || t == 39))) { return false }
    if f & 1 != 0 && t <= 4 { return false }
    if g.mem.combat() != 0 && (0..5).any(|i| i != who && g.mem.read(walker(i).add(W_ACTIVE), 1) != 0 && round::traveller_cell(g, i) == (r, c)) { return false }
    (f & 1 != 0 && t / 20 < 5) || t / 20 < 2
}
pub(super) fn entry(g: &Ground, (r, c): Cell) -> bool {
    if g.mem.combat() != 0 || r < 0 || c < 0 || r >= g.mem.map_blocks_high() as i16 * 12 || c >= g.mem.map_stride() { return false }
    let cell = commands::cell(g, r, c);
    let (f, t, occ) = (g.mem.read(cell, 1) as u8, g.mem.read(cell.add(1), 1) as u8, g.mem.read(cell.add(2), 1) as u8);
    occ & 0x3F == 0 && if f & 1 == 0 { t <= 14 } else { t <= 4 }
}
pub fn find_path(g: &Ground, who: u8, from: Cell, to: Cell) -> Option<Vec<Cell>> {
    if from == to || !(passable(g, who, to) || entry(g, to)) { return None }
    let mut prev: BTreeMap<Cell, Cell> = BTreeMap::new();
    let mut queue = VecDeque::from([(from, 0usize)]);
    let near = |c: Cell| { let (dr, dc) = ((c.0 - to.0) as i32, (c.1 - to.1) as i32); (dr.abs().max(dc.abs()), dr * dr + dc * dc) };
    while let Some((c, d)) = queue.pop_front() {
        if d >= ROUTE_MAX { continue }
        let mut next: Vec<Cell> = (-1i16..=1).flat_map(|dr| (-1i16..=1).map(move |dc| (c.0 + dr, c.1 + dc))).filter(|&n| n != c).collect();
        next.sort_by_key(|&n| near(n));
        for n in next {
            if n == from || prev.contains_key(&n) || !(passable(g, who, n) || n == to && entry(g, n)) { continue }
            prev.insert(n, c);
            if n == to {
                let mut path = vec![n]; let mut at = n;
                while let Some(&p) = prev.get(&at) { if p == from { break } path.push(p); at = p }
                path.reverse(); return Some(path)
            }
            queue.push_back((n, d + 1));
        }
    }
    None
}
pub fn wait_for(g: &mut Ground, k: u8) {
    let l = super::fire::label(g, Unit::Npc(k)); log(g, format!("Waiting for {l} to move"));
    if let Some(s) = st(g) { s.hover = "waiting".into(); s.path.clear() }
    g.mem.set_redraw_pending(1); walk::wait_turn(g)
}
pub fn direction(g: &Ground, (dr, dc): (i16, i16)) -> Option<u8> { (0..9u16).find(|&d| g.mem.i8(0x2F5 + d * 2) as i16 == dr && g.mem.i8(0x2F6 + d * 2) as i16 == dc).map(|d| d as u8) }
fn step(g: &mut Ground, (dr, dc): (i16, i16)) -> bool {
    let Some(d) = direction(g, (dr, dc)) else { return false };
    let before = (g.mem.x(), g.mem.y());
    recentre(g, ((before.0 >> 1) + dr, (before.1 >> 1) + dc));
    for _ in 0..2 {
        walk::move_party(g, dr, dc, (d & 7) as i16);
        if (g.mem.x(), g.mem.y()) != before { return (g.mem.x() | g.mem.y()) & 1 == 0 || g.mem.event() != -1 }
        if g.mem.event() != -1 { return false }
    }
    false
}
fn recentre(g: &mut Ground, (r, c): Cell) {
    if g.mem.combat() == 0 { return }
    let (x, y) = (r * 2, c * 2);
    let (vx, vy) = (g.mem.view_x(), g.mem.view_y());
    if x >= vx && y >= vy && x <= vx + 12 && y <= vy + 22 { return }
    let (rows, cols) = (g.mem.map_blocks_high() as i16 * 24, g.mem.map_stride() * 2);
    g.mem.set_view_x((x - 6).clamp(0, (rows - 14).max(0)) & !1); g.mem.set_view_y((y - 12).clamp(0, (cols - 24).max(0)) & !1);
    g.mem.set_event_id(-1);
}
fn in_sight(g: &Ground) -> bool { !round::in_encounter(g) && round::watcher(g).is_some() }
pub fn move_to(g: &mut Ground, to: Cell, speed: Speed) {
    let who = match move_check(g, round::selected(g)) { Ok(w) => w, Err(r) => { say(g, r); return } };
    let from = round::traveller_cell(g, who);
    let Some(path) = find_path(g, who, from, to) else { say(g, "no path"); return };
    let n = path.len() as i32;
    let encounter = round::in_encounter(g);
    let cost = if encounter {
        let s = g.squad.as_mut().unwrap(); let t = &mut s.p1.turns[who as usize];
        if !t.moved && t.speed == 0 {
            let evade = speed == Speed::Evade;
            let run = speed == Speed::Run || (speed == Speed::Auto && n > round::MOVE_WALK) || (evade && n * 2 > round::MOVE_WALK);
            t.speed = if run { 2 } else { 1 }; t.evade = evade; t.mp = if run { round::MOVE_RUN } else { round::MOVE_WALK };
        }
        if t.evade { 2 } else { 1 }
    } else { 1 };
    let name = round::name(g, who);
    let mut walked = 0usize; let mut cell = from; g.waiting = None; g.blocked_by = None;
    for &next in &path {
        if encounter && g.squad.as_ref().unwrap().p1.turns[who as usize].mp < cost { break }
        if !step(g, (next.0 - cell.0, next.1 - cell.1)) {
            if let Some(k) = g.blocked_by.take() { g.waiting = Some((who, to)); wait_for(g, k) }
            break
        }
        cell = next; walked += 1;
        round::walked(g, cost as u32);
        if g.mem.event() != -1 || in_sight(g) { break }
    }
    let rest: Vec<u8> = path[walked..].windows(2).filter_map(|w| direction(g, (w[1].0 - w[0].0, w[1].1 - w[0].1))).collect();
    let rest: Vec<u8> = if g.mem.event() != -1 { Vec::new() }
        else if walked < path.len() { let first = direction(g, (path[walked].0 - cell.0, path[walked].1 - cell.1)); first.into_iter().chain(rest).collect() } else { Vec::new() };
    write_route(g, who, &rest);
    if g.waiting.is_some() { return }
    let t = g.squad.as_ref().unwrap().p1.turns[who as usize];
    let speed_word = if !encounter { "walks" } else if t.evade { "evades" } else if t.speed == 2 { "runs" } else { "walks" };
    log(g, format!("{name} {speed_word} {walked} square{}{}", if walked == 1 { "" } else { "s" }, if rest.is_empty() { String::new() } else { format!(", {} to go", rest.len()) }));
    if encounter && walked > 0 { round::order(g, Order::Move) }
    if let Some(s) = st(g) { s.hover.clear(); s.path.clear() }
    g.mem.set_redraw_pending(1);
}

pub fn attackable(g: &Ground, k: u8) -> bool { round::npc_present(g, k) && round::willing(g, k) }
pub(crate) fn hand(g: &Ground, who: u8) -> (super::state::Far, super::state::Far) { let slot = g.mem.read(walker(who).add(W_HAND), 1) as i32 + 21; (walk::item_ptr(g, who as i32, slot), walk::item_def(g, who as i32, slot)) }
pub fn chamber_empty(g: &Ground, who: u8) -> bool {
    let (obj, def) = hand(g, who);
    obj != Default::default() && g.mem.read_i(def.add(0x1D), 2) == 2 && g.mem.read(def.add(0x27), 1) & 8 != 0 && g.mem.read(obj.add(2), 1) == 0
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Shot { pub weapon: &'static rules::Weapon, pub task: rules::Task, pub band: rules::Band, pub from: Cell, pub to: Cell, pub skill: adapters::Skill, pub dex: i32, pub movement: i32, pub av: i32, pub exposed: bool, pub label: String,
    pub terms: Vec<String>, pub note: String, pub cover: bool,
    pub tactics: i32 }
impl Shot {
    pub fn dm_terms(&self) -> Vec<String> {
        let mut terms = if self.terms.is_empty() { vec![format!("skill {}", self.skill.level), format!("Dex {:+}", rules::char_dm(self.dex))] } else { self.terms.clone() };
        if self.movement != 0 { terms.push(format!("moving {:+}", self.movement)) }
        if self.terms.is_empty() && self.skill.unskilled { terms.push("unskilled".into()) }
        if self.cover { terms.push("cover".into()) }
        if self.tactics > 0 { terms.push(format!("+{} tactics", self.tactics)) }
        terms
    }
    pub fn band_word(&self) -> String { let mut b = self.band.name().to_string(); if let Some(c) = b.get_mut(0..1) { c.make_ascii_uppercase() } b }
    pub fn effect(&self) -> &'static str {
        match rules::class(rules::pen_at(self.weapon.pen as i32, self.weapon.atten, self.band), self.av) {
            rules::Class::High => "High pen: full damage on a hit", rules::Class::Low => "Low pen: half damage", rules::Class::Zero => "Zero pen: only an exceptional hit grazes",
        }
    }
    pub fn describe(&self) -> String {
        format!("{}: {} {}+, DM {:+} ({}), needs {}+ ({}%){}", self.band_word(), self.task.difficulty.name(), self.task.target, self.task.dm, self.dm_terms().join(", "), self.task.needs(), self.task.chance_pct(), if self.note.is_empty() { String::new() } else { format!(", {}", self.note) })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sight { pub from: Cell, pub to: Cell, pub los: bool, pub cover: bool, pub band: rules::Band }
pub fn preview(g: &Ground, t: Option<Target>) -> (Vec<String>, Option<Sight>) {
    let Some(Target::Unit(Unit::Npc(k))) = t else { return (Vec::new(), None) };
    if g.squad.is_none() || !round::npc_present(g, k) { return (Vec::new(), None) }
    let who = round::selected(g);
    let (from, to) = (round::traveller_cell(g, who), round::npc_cell(g, k));
    let (los, band) = (round::line_of_sight(g, from, to), rules::band(round::squares(from, to)));
    let mut lines = vec![round::described(g, k, attitude(g, k))];
    let sight = match fire_check(g, who, k) {
        Ok(s) => {
            lines.push(format!("{}: {} {}+", s.band_word(), s.task.difficulty.name(), s.task.target));
            lines.push(format!("DM {:+}: {}", s.task.dm, s.dm_terms().join(", ")));
            let pct = s.task.chance_pct();
            lines.push(format!("needs {}+ on 2D ({pct}%){}", s.task.needs(), if pct == 0 { ": no chance" } else { "" }));
            lines.push(s.effect().into());
            if !s.note.is_empty() { lines.push(s.note.clone()) }
            let warn = p20::line_warning(g, who, k); if !warn.is_empty() { lines.push(warn.trim_start_matches("; ").into()) }
            Sight { from, to, los, cover: s.cover, band }
        }
        Err(r) => { lines.push(r); Sight { from, to, los, cover: false, band } }
    };
    (lines, Some(sight))
}
pub fn destination(g: &Ground, who: u8) -> Option<Cell> {
    if g.mem.combat() == 0 || g.mem.read(walker(who).add(W_ACTIVE), 1) == 0 { return None }
    let route = read_route(g, who); if route.is_empty() { return None }
    let mut c = round::traveller_cell(g, who);
    for d in route { let d = (d as u16).min(8); c = (c.0 + g.mem.i8(0x2F5 + d * 2) as i16, c.1 + g.mem.i8(0x2F6 + d * 2) as i16) }
    Some(c)
}
pub fn fire_check(g: &Ground, who: u8, k: u8) -> Result<Shot, String> { fire_check_opts(g, who, k, p20::Opts::default()) }
pub fn fire_check_opts(g: &Ground, who: u8, k: u8, o: p20::Opts) -> Result<Shot, String> {
    if !round::npc_present(g, k) { return Err("gone".into()) }
    turn_check(g).map_err(String::from)?;
    let w = adapters::traveller_weapon(g, who).ok_or("no weapon in hand")?;
    match w.profile { Profile::Melee => return p20::melee_check(g, who, k), Profile::Indirect => return p20::launch_check(g, who, round::npc_cell(g, k)), Profile::Thrown => return p20::throw_check(g, who, round::npc_cell(g, k)), _ => {} }
    if round::in_encounter(g) && g.squad.as_ref().unwrap().p1.turns[who as usize].attacked { return Err("no Attack left".into()) }
    if o.rapid && !w.rapid { return Err("no rapid fire".into()) }
    if o.rapid && o.pinpoint { return Err("rapid fire and pinpoint do not mix".into()) }
    if chamber_empty(g, who) { return Err("chamber empty".into()) }
    let (from, to) = (round::traveller_cell(g, who), round::npc_cell(g, k));
    let band = rules::band(round::squares(from, to));
    if !round::line_of_sight(g, from, to) { return Err("no line of sight".into()) }
    if band > w.max { return Err("out of range".into()) }
    let t = g.squad.as_ref().unwrap().p1.turns[who as usize];
    if w.recoil == Recoil::Hi && t.moved { return Err("moved: high recoil".into()) }
    let f = round::traveller_fighter(g, who);
    let skill = f.skill(g, &g.squad.as_ref().unwrap().compendium, w);
    let target = round::npc_fighter(g, k);
    let tspeed = g.squad.as_ref().unwrap().p1.npcs.get(&k).map_or(0, |n| n.speed as i32);
    let movement = rules::movement_dm(tspeed, false, t.speed as i32, t.evade, band);
    let cover = p20::in_cover(g, from, to);
    let fire = rules::Fire { profile: w.profile, band, braced: w.gyro, cover, rapid: o.rapid, pinpoint: o.pinpoint, skill: skill.level, unskilled: skill.unskilled, dex: f.dex, movement, ..Default::default() };
    let task = rules::hit_task(&fire).ok_or("out of range")?;
    let mut av = target.armour.vs(w.class);
    if (cover && target.armour.exposed) || o.pinpoint { av /= 2 }
    Ok(tactics::apply(g, Shot { weapon: w, task, band, from, to, skill, dex: f.dex, movement, av, exposed: target.armour.exposed, label: target.label, terms: Vec::new(), note: String::new(), cover, tactics: 0 }))
}
fn offscreen_target(g: &Ground, k: u8) -> Option<&'static str> {
    (g.squad.is_some() && !round::on_screen(g, round::npc_cell(g, k))).then_some("target outside the screen: cannot fire")
}
fn fire_input(g: &mut Ground, k: u8) {
    if let Some(reason) = offscreen_target(g, k) { say(g, reason); return }
    fire(g, k)
}
pub fn fire(g: &mut Ground, k: u8) { p20::attack(g, k, p20::Opts::default()) }
pub fn set_target(g: &mut Ground, k: u8) {
    let who = round::selected(g); let w = walker(who);
    g.mem.write(w.add(W_TARGET), k as u32, 1);
    let n = Mem::npc(k as i16);
    g.mem.write(w.add(W_AIM_X), g.mem.read(n.add(32), 2) & 0xFFFE, 2); g.mem.write(w.add(W_AIM_Y), g.mem.read(n.add(34), 2) & 0xFFFE, 2);
    walk::walker_set_pose(g, w);
    g.mem.set_redraw_pending(1);
}
pub(crate) fn reload_check(g: &Ground, who: u8) -> Result<(), String> {
    turn_check_for(g, who).map_err(String::from)?;
    if round::in_encounter(g) && g.squad.as_ref().unwrap().p1.turns[who as usize].attacked { return Err("no Attack left".into()) }
    let (obj, def) = hand(g, who);
    if obj == Default::default() || g.mem.read_i(def.add(0x1D), 2) != 2 || g.mem.read(def.add(0x27), 1) & 0x20 == 0 { return Err("nothing to reload".into()) }
    if g.mem.read(obj.add(1), 1) == 0 { return Err("no magazine".into()) }
    if adapters::item_id(g, def) == Some(25) && round::in_encounter(g) { return Err("PGMP: not in a fight".into()) }
    Ok(())
}
pub fn reload(g: &mut Ground) {
    let who = round::selected(g);
    if let Err(r) = reload_check(g, who) { say(g, &r); return }
    let (obj, def) = hand(g, who);
    let rds = adapters::item_id(g, def).and_then(rules::weapon).map_or_else(|| g.mem.read(def.add(0x28), 1), |w| w.rds as u32);
    let mags = g.mem.read(obj.add(1), 1);
    g.mem.write(obj.add(1), mags - 1, 1); g.mem.write(obj.add(2), rds, 1);
    render::walker_panel(g, false, true);
    let n = round::name(g, who);
    log(g, format!("{n} reloads ({rds} rounds, {} magazine{} left){}", mags - 1, if mags == 2 { "" } else { "s" }, attack_used(g)));
    if let Some(s) = st(g) { s.hover.clear() }
    round::order(g, Order::Attack);
    g.mem.set_redraw_pending(1);
}
pub fn swap_hand(g: &mut Ground) {
    if g.mem.combat() == 0 { return }
    let p = walker(round::selected(g)).add(W_HAND);
    g.mem.write(p, g.mem.read(p, 1) ^ 1, 1);
    render::walker_panel(g, false, true);
    g.mem.set_redraw_pending(1);
}
fn fire_mode_check(g: &Ground, who: u8) -> Result<(), String> {
    turn_check_for(g, who).map_err(String::from)?;
    if !adapters::traveller_weapon(g, who).is_some_and(|w| w.auto > 0) { return Err("no autofire".into()) }
    if round::in_encounter(g) && g.squad.as_ref().unwrap().p1.turns[who as usize].attacked { return Err("no Attack left".into()) }
    Ok(())
}
pub fn fire_mode(g: &mut Ground) {
    let who = round::selected(g);
    if let Err(r) = fire_mode_check(g, who) { say(g, &r); return }
    let s = st(g).unwrap(); s.full_auto[who as usize] ^= true; let full = s.full_auto[who as usize];
    let n = round::name(g, who);
    log(g, format!("{n} switches to {}{}", if full { "full auto" } else { "semi-automatic" }, attack_used(g)));
    round::order(g, Order::Attack);
    g.mem.set_redraw_pending(1);
}

#[cfg(test)]
pub(crate) mod t {
    use super::*;
    use crate::ground::{squad, rules::compendium::{Compendium, NPCS_TOML}, state::{Far, MAP_PTR, N_X, N_Y, W_X, W_Y}};
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    pub(crate) const ROWS: i16 = 36; pub(crate) const COLS: i16 = 60; pub(crate) const ITEMS: u16 = 0x9100;
    pub(crate) fn ground() -> Ground {
        let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.alloc(0x8000, ROWS as usize * COLS as usize * 3); g.mem.set_pointer(MAP_PTR, Far::new(0, 0x8000));
        for i in 0..ROWS as usize * COLS as usize { g.mem.write(Far::new((i * 3 + 1) as u16, 0x8000), 0x20, 1) }
        g.mem.set_map_stride(COLS); g.mem.set_map_blocks_high((ROWS / 12) as u8); g.mem.set_u8(0x5C3B, (COLS / 20) as u8);
        g.mem.alloc(ITEMS, 46 * 140); g.mem.set_pointer(0x6D6D, Far::new(0, ITEMS));
        for id in 0..=28u16 { g.mem.write(Far::new(46 * id + 0x1D, ITEMS), 2, 2); g.mem.write(Far::new(46 * id + 0x27, ITEMS), 0x78 | 0x20, 1); g.mem.write(Far::new(46 * id + 0x28, ITEMS), 20, 1) }
        for (d, (dx, dy)) in [(0i8, 0i8), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1)].into_iter().enumerate() { g.mem.set_u8(0x2F5 + 2 * d as u16, dx as u8); g.mem.set_u8(0x2F6 + 2 * d as u16, dy as u8) }
        g.mem.set_party_u8(0, 0x1F); for i in 0..5 { g.mem.set_party_u8(1 + i, i as u8) }
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        for i in 0..5u16 { let r = 0x102 + 256 * i; for (k, b) in format!("Trav{}", i + 1).bytes().enumerate() { g.mem.set_party_u8(r + k as u16, b) } g.mem.set_party_u8(r + 0x23, 99); g.mem.set_party_u8(r + 0x24, 0); g.mem.set_party_u8(r + 0x25, 99); g.mem.set_party_u8(r + 0x14 + 1, 8); }
        g.mem.set_party_u8(0x12B + 11, 2);
        g.mem.set_party_u16(0x1CE + 2 * 21, 1); g.mem.set_party_u8(0xA82, 14); g.mem.set_party_u8(0xA82 + 1, 3); g.mem.set_party_u8(0xA82 + 2, 20);
        g.mem.set_outdoors(1); g.mem.set_npc_count(32); g.mem.set_combat(1); g.mem.set_event(-1);
        for (i, (r, c)) in [(22i16, 5i16), (22, 6), (23, 5), (23, 6), (22, 7)].into_iter().enumerate() { let w = Mem::walker(i as i16); g.mem.write(w, 1, 1); g.mem.write(w.add(W_X), (r * 2) as u16 as u32, 2); g.mem.write(w.add(W_Y), (c * 2) as u16 as u32, 2); }
        g.mem.set_x(44); g.mem.set_y(10); g.mem.set_view_x(38); g.mem.set_view_y(0); g.mem.set_leader(0);
        g.scene = Some(crate::ui::SceneHook::default());
        g.squad = Some(squad::State::with_compendium(Compendium::parse(NPCS_TOML).unwrap()));
        g
    }
    pub(crate) fn place(g: &mut Ground, k: u8, (row, col): Cell, attitude: u8, weapon: u8, picture: u8) {
        let n = Mem::npc(k as i16);
        g.mem.write(n.add(2), 0, 1); g.mem.write(n.add(9), picture as u32, 1); g.mem.write(n.add(10), 12, 1); g.mem.write(n.add(11), 3, 1); g.mem.write(n.add(13), attitude as u32, 1);
        g.mem.write(n.add(31), weapon as u32, 1); g.mem.write(n.add(0x1E), 50, 1);
        g.mem.write(n.add(N_X), (row * 2) as u16 as u32, 2); g.mem.write(n.add(N_Y), (col * 2) as u16 as u32, 2);
        g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 2) as u16, 0x8000), k as u32, 1);
    }
    pub(crate) fn wall(g: &mut Ground, (row, col): Cell) { g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 1) as u16, 0x8000), 0, 1) }
    pub(crate) fn p1(g: &Ground) -> &round::State { &g.squad.as_ref().unwrap().p1 }
    pub(crate) fn fight(g: &mut Ground) { g.squad.as_mut().unwrap().p1.defs.push(round::Def { id: "f".into(), surprise: round::Surprise::None, ..round::Def::default_sight() }); round::pass(g, &[]); assert!(round::in_encounter(g)) }

    #[test]
    fn preview_sight_fx_and_destination() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        let (lines, sight) = preview(&g, Some(Target::Unit(Unit::Npc(14))));
        assert_eq!(&lines[..4], ["Hostile (thug, NPC 14)", "Short: Routine 7+", "DM +3: skill 2, Dex +1", "needs 4+ on 2D (91%)"], "{lines:?}"); assert!(lines[4].contains(" pen: ") && lines[5] == "Trav4 in the line!", "{lines:?}");
        let s = sight.unwrap(); assert_eq!((s.from, s.to, s.los, s.cover, s.band), ((22, 5), (24, 7), true, false, rules::Band::Short));
        wall(&mut g, (23, 6)); let (lines, sight) = preview(&g, Some(Target::Unit(Unit::Npc(14)))); assert_eq!(lines[1], "no line of sight"); assert!(!sight.unwrap().los);
        g.mem.write(Far::new(((23 * COLS as usize + 6) * 3 + 1) as u16, 0x8000), 0x20, 1);
        assert!(preview(&g, Some(Target::Cell((1, 1)))).0.is_empty() && preview(&g, None).1.is_none());
        hover(&mut g, Some(Target::Unit(Unit::Npc(14)))); assert_eq!(g.squad.as_ref().unwrap().orders.target, Some(Target::Unit(Unit::Npc(14))));
        let sq = crate::ui::scene::Squad::capture(&g); assert_eq!((sq.preview.len(), sq.sight.is_some(), sq.mp[0], sq.pools, sq.full_auto[0], sq.log_n > 0), (6, true, 10, (0, 0), false, true));
        let seq0 = p1(&g).fx.seq;
        intent(&mut g, Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) });
        let f = p1(&g).fx.clone(); assert_eq!((f.seq, f.unit, f.shots.len(), f.shots[0].0, f.shots[0].2), (seq0 + 1, Some(Unit::Traveller(0)), 1, (22, 5), 0x78));
        let hit = p1(&g).log.iter().any(|l| l.contains("hit by")); assert_eq!(f.hits.len(), usize::from(hit), "{:?}", p1(&g).log); if hit { assert_eq!((f.hits[0].0, f.hits[0].1), (Unit::Npc(14), (24, 7))) }
        assert_eq!(crate::ui::scene::Squad::capture(&g).preview.len(), 0, "the shot cleared the hover: no preview");
        round::end_turn(&mut g);
        let f = p1(&g).fx.clone(); assert_eq!((f.unit, f.walk[0]), (Some(Unit::Npc(14)), (24, 7))); assert!(f.seq > seq0 + 1 && !f.shots.is_empty(), "{f:?} {:?}", p1(&g).log);
        assert_eq!(p1(&g).strip, "Your move: Trav2");
        write_route(&mut g, 0, &[3, 3, 3, 3]); assert_eq!(destination(&g, 0), Some((22, 9))); write_route(&mut g, 0, &[]); assert_eq!(destination(&g, 0), None);
    }
    #[test]
    fn path_finder_and_the_route_encoding() {
        let mut g = ground();
        let p = find_path(&g, 0, (22, 5), (22, 8)).unwrap(); assert_eq!((p.len(), p[2]), (3, (22, 8)), "around Trav2 and Trav5: {p:?}");
        assert_eq!(find_path(&g, 0, (22, 5), (22, 6)), None, "Trav2 stands there: occupied");
        let p = find_path(&g, 0, (22, 5), (19, 8)).unwrap(); assert_eq!(p, vec![(21, 6), (20, 7), (19, 8)], "diagonals are one square");
        for c in 0..COLS { if c != 30 { wall(&mut g, (20, c)) } }
        let p = find_path(&g, 0, (22, 5), (18, 5)).unwrap(); assert!(p.contains(&(20, 30)) && p.len() >= 25, "{p:?}");
        wall(&mut g, (20, 30)); assert_eq!(find_path(&g, 0, (22, 5), (18, 5)), None, "no gap: no path");
        assert_eq!(find_path(&g, 0, (22, 5), (0, 59)).map(|p| p.len()), None, "beyond 62 squares");
        assert_eq!((direction(&g, (0, 1)), direction(&g, (-1, -1)), direction(&g, (2, 0))), (Some(3), Some(8), None));
        assert!(!passable(&g, 0, (22, 6)) && passable(&g, 0, (22, 8)) && !passable(&g, 0, (-1, 0)));
    }
    #[test]
    fn a_broken_party_waits_for_a_neutral_too_with_a_log_line() {
        let mut g = ground(); g.mem.set_event(-1); assert!(g.mem.combat() != 0 && !round::in_encounter(&g), "broken, exploring");
        place(&mut g, 20, (21, 5), 2, 0, 41);
        assert!(passable(&g, 0, (21, 5)) && find_path(&g, 0, (22, 5), (20, 5)).is_some(), "routed through the neutral");
        let (d, before) = (direction(&g, (-1, 0)).unwrap(), g.clock.now()); g.mem.set_heading(d as i16);
        walk::move_party(&mut g, -1, 0, d as i16); let k = g.blocked_by.take(); assert_eq!(k, Some(20));
        wait_for(&mut g, 20);
        assert!(p1(&g).log.last().is_some_and(|l| l.starts_with("Waiting for ") && l.ends_with("(NPC 20) to move")), "{:?}", p1(&g).log);
        assert!(g.clock.now() >= before + mt_engine::sim::sched::SECOND); assert_eq!(round::traveller_cell(&g, 0), (22, 5));
    }
    #[test]
    fn reload_outside_a_fight_says_what_happened() {
        let mut g = ground(); g.mem.set_combat(0); g.mem.set_event(-1);
        super::super::input::dispatch(&mut g, 0x13, 0, 0);
        assert_eq!(p1(&g).log.last().map(String::as_str), Some("Trav1: nothing to reload, the chamber is loaded"), "{:?}", p1(&g).log);
        g.mem.set_party_u8(0xA82 + 2, 0);
        super::super::input::dispatch(&mut g, 0x13, 0, 0);
        assert_eq!(p1(&g).log.last().map(String::as_str), Some("Trav1 reloads"), "{:?}", p1(&g).log); assert_eq!(g.mem.party_u8(0xA82 + 2), 20);
    }
    #[test]
    fn traveller_list_fire_row_uses_the_standing_target_and_reasons_are_plain() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        let find = |m: &Menu, l: &str| m.items.iter().find(|i| i.label == l).cloned().unwrap_or_else(|| panic!("{l}: {:?}", m.items.iter().map(|i| &i.label).collect::<Vec<_>>()));
        let m = menu_for(&g, Target::Unit(Unit::Traveller(0)));
        assert_eq!(find(&m, "Fire").grey.as_deref(), Some("no target")); assert_eq!(find(&m, "Use item").grey.as_deref(), Some("not yet")); assert!(!m.items.iter().any(|i| i.grey.as_deref().is_some_and(|r| r.contains("(P"))), "no internal codes");
        set_target(&mut g, 14); let m = menu_for(&g, Target::Unit(Unit::Traveller(0))); assert_eq!(find(&m, "Fire").grey, None);
        let logged = p1(&g).logged; g.squad.as_mut().unwrap().orders.menu = Some(m.clone()); pick(&mut g, m.items.iter().position(|i| i.label == "Fire").unwrap() as u8);
        assert!(p1(&g).logged > logged && p1(&g).turns[0].attacked, "the standing target was fired at: {:?}", p1(&g).log);
        let m = menu_for(&g, Target::Unit(Unit::Traveller(0))); assert_eq!(find(&m, "Fire").grey.as_deref(), Some("no Attack left"));
    }
    #[test]
    fn npc_menu_offers_converse_attack_walk_to_and_look() {
        let mut g = ground(); g.mem.set_combat(0); g.mem.set_event(-1);
        place(&mut g, 20, (21, 6), 2, 0, 41);
        let m = menu_for(&g, Target::Unit(Unit::Npc(20))); let find = |m: &Menu, l: &str| m.items.iter().find(|i| i.label == l).cloned().unwrap_or_else(|| panic!("{l}: {:?}", m.items.iter().map(|i| &i.label).collect::<Vec<_>>()));
        assert_eq!(find(&m, "Converse").grey, None); assert_eq!(find(&m, "Walk to").grey.as_deref(), Some("adjacent already")); assert_eq!(find(&m, "Look").grey, None);
        assert_eq!(find(&m, "Attack").op, Op::Fire); assert_eq!(m.items[m.default].label, "Converse");
        g.squad.as_mut().unwrap().orders.menu = Some(m.clone()); pick(&mut g, m.items.iter().position(|i| i.label == "Look").unwrap() as u8);
        assert!(p1(&g).log.last().is_some_and(|l| l.starts_with("Neutral (") && l.contains("NPC 20")), "{:?}", p1(&g).log);
        place(&mut g, 21, (18, 2), 2, 0, 41); let m = menu_for(&g, Target::Unit(Unit::Npc(21)));
        assert_eq!(find(&m, "Walk to").grey, None); assert_eq!(beside(&g, 0, 21), Some((19, 2)), "the nearest free square beside them");
        g.squad.as_mut().unwrap().orders.menu = Some(m.clone()); pick(&mut g, m.items.iter().position(|i| i.label == "Walk to").unwrap() as u8);
        assert_eq!(round::traveller_cell(&g, 0), (19, 2), "{:?}", p1(&g).log);
    }
    #[test]
    fn converse_faces_the_targeted_npc_and_a_far_one_is_refused_aloud() {
        let mut g = ground(); g.mem.set_combat(0); g.mem.set_event(-1); g.mem.set_heading(0); g.mem.set_key(0);
        place(&mut g, 20, (21, 6), 2, 0, 41);
        perform(&mut g, Op::Converse, Target::Unit(Unit::Npc(20)));
        assert_eq!((g.mem.heading(), g.mem.key()), (direction(&g, (-1, 1)).unwrap() as i16, SCAN_C), "turned to face, then the talk key");
        assert_eq!(round::traveller_cell(&g, 0), (22, 5), "a turn, not a step");
        g.mem.set_key(0); place(&mut g, 21, (18, 5), 2, 0, 41);
        perform(&mut g, Op::Converse, Target::Unit(Unit::Npc(21)));
        assert_eq!(g.mem.key(), 0); assert!(p1(&g).log.last().is_some_and(|l| l.ends_with("(NPC 21): not adjacent")), "{:?}", p1(&g).log);
        g.mem.set_heading(0);
        super::super::commands_talk::talk(&mut g);
        assert_eq!(p1(&g).log.last().map(String::as_str), Some("You aren't facing anybody."), "{:?}", p1(&g).log);
    }
    #[test]
    fn a_neutral_passer_by_is_waited_for_and_the_route_resumes_when_he_moves() {
        let mut g = ground(); g.mem.set_combat(0); g.mem.set_event(-1);
        place(&mut g, 20, (21, 5), 2, 0, 41);
        assert!(round::neutral(&g, 20) && passable(&g, 0, (21, 5)) && occupant(&g, (21, 5)) == 20);
        assert_eq!(find_path(&g, 0, (22, 5), (19, 5)), Some(vec![(21, 5), (20, 5), (19, 5)]), "planned through him");
        hover(&mut g, Some(Target::Cell((19, 5)))); let before = g.clock.now();
        move_to(&mut g, (19, 5), Speed::Auto);
        assert_eq!(round::traveller_cell(&g, 0), (22, 5), "{:?}", p1(&g).log); assert_eq!(g.waiting, Some((0, (19, 5))));
        assert!(g.clock.now() >= before + mt_engine::sim::sched::SECOND, "a turn passed: {} -> {}", before, g.clock.now()); assert_eq!(read_route(&g, 0).len(), 3, "the route stands");
        let s = crate::ui::Scene::capture(&g, crate::ui::ScreenKind::Ground, None); assert_eq!((s.squad.label.as_str(), s.squad.hover.as_str()), ("waiting", ""));
        round::pass(&mut g, &[]); assert_eq!(round::traveller_cell(&g, 0), (22, 5), "still blocked: still waiting"); assert!(g.waiting.is_some());
        g.mem.write(Far::new(((21usize * COLS as usize + 5) * 3 + 2) as u16, 0x8000), 0, 1);
        round::pass(&mut g, &[]); assert_eq!(round::traveller_cell(&g, 0), (19, 5), "resumed: {:?}", p1(&g).log); assert!(g.waiting.is_none() && read_route(&g, 0).is_empty());
        place(&mut g, 21, (18, 5), 2, 0, 41); let (d, before) = (direction(&g, (-1, 0)).unwrap(), g.clock.now());
        walk::move_party(&mut g, -1, 0, d as i16); assert_eq!(g.blocked_by.take(), Some(21)); let _ = before;
        place(&mut g, 22, (18, 5), 0, 14, 41); assert!(!round::neutral(&g, 22) && !passable(&g, 0, (18, 5)), "a hostile is not routed through");
        walk::move_party(&mut g, -1, 0, d as i16); assert_eq!(g.blocked_by, None);
    }
    #[test]
    fn a_door_ends_a_route_and_its_text_is_the_label_not_the_stripe() {
        let mut g = ground(); g.mem.set_combat(0); g.mem.set_event(-1);
        let door = (20, 5); let tile = |g: &mut Ground, (r, c): Cell, t: u8| g.mem.write(Far::new(((r as usize * COLS as usize + c as usize) * 3 + 1) as u16, 0x8000), t as u32, 1);
        for c in 0..COLS { tile(&mut g, (20, c), 40) }
        tile(&mut g, door, 3);
        assert!(!passable(&g, 0, door) && entry(&g, door) && !entry(&g, (20, 6)));
        assert_eq!(find_path(&g, 0, (22, 5), door), Some(vec![(21, 5), door]));
        assert_eq!(find_path(&g, 0, (22, 5), (18, 5)), None, "a door is never a step through");
        g.mem.set_combat(1); assert!(!entry(&g, door) && find_path(&g, 0, (22, 5), door).is_none(), "in combat try_move refuses the tile"); g.mem.set_combat(0);
        hover(&mut g, Some(Target::Cell(door)));
        let s = crate::ui::Scene::capture(&g, crate::ui::ScreenKind::Ground, None);
        assert_eq!((s.squad.label.as_str(), s.squad.hover.as_str()), ("Building entrance: 2 squares", ""), "the walk text is the label; a square that starts something is named");
        tile(&mut g, (21, 6), 38); hover(&mut g, Some(Target::Cell((21, 6))));
        assert_eq!(g.squad.as_ref().unwrap().orders.hover, "Police post: 1 square");
        assert_eq!((trigger_name(0, 16), trigger_name(0, 19), trigger_name(1, 38), trigger_name(0, 7)), (None, None, None, Some("Casino")), "ambush and plot squares stay secret; a lit 38 is ground");
        hover(&mut g, Some(Target::Cell(door)));
        move_to(&mut g, door, Speed::Auto);
        assert_eq!((round::traveller_cell(&g, 0), g.mem.x() & 1), (door, 1), "on the door's half-step, as a keyed step: {:?}", p1(&g).log); assert_eq!(g.mem.event(), 8, "the door's event fires");
        assert!(read_route(&g, 0).is_empty(), "no route back into the door"); assert!(p1(&g).log.iter().any(|l| l == "Trav1 walks 2 squares"), "{:?}", p1(&g).log);
        place(&mut g, 14, (24, 7), 0, 14, 41); hover(&mut g, Some(Target::Unit(Unit::Npc(14))));
        let s = crate::ui::Scene::capture(&g, crate::ui::ScreenKind::Ground, None);
        assert!(s.squad.label.is_empty() && s.squad.hover.starts_with("Hostile"), "an NPC's text stays the stripe: {:?}", s.squad.hover);
        g.mem.set_event(-1); hover(&mut g, Some(Target::Cell((20, 6))));
        let s = crate::ui::Scene::capture(&g, crate::ui::ScreenKind::Ground, None); assert_eq!((s.squad.label.as_str(), s.squad.hover.as_str()), ("no path", ""));
        move_to(&mut g, (20, 6), Speed::Auto);
        let s = crate::ui::Scene::capture(&g, crate::ui::ScreenKind::Ground, None); assert_eq!((s.squad.label.as_str(), s.squad.hover.as_str()), ("no path", ""), "a refused click is the label too");
    }
    #[test]
    fn move_order_walks_declares_speed_and_keeps_the_rest_as_the_route() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        assert_eq!(p1(&g).strip, "Your move: Trav1");
        hover(&mut g, Some(Target::Cell((22, 19)))); { let s = &g.squad.as_ref().unwrap().orders; assert_eq!(s.hover, "Run 14 squares: -2 to be hit, -2 to your shots"); assert_eq!(s.path.len(), 14); assert_eq!((s.path[9].1, s.path[10].1, s.path[13].1), (PATH_WALK, PATH_RUN, PATH_RUN)); }
        hover(&mut g, Some(Target::Cell((22, 30)))); assert!(g.squad.as_ref().unwrap().orders.hover.ends_with("rounds"), "{}", g.squad.as_ref().unwrap().orders.hover);
        move_to(&mut g, (22, 19), Speed::Auto);
        let t = p1(&g).turns[0]; assert_eq!((t.speed, t.moved, t.mp, t.evade), (2, true, 6, false), "{t:?}");
        assert_eq!(round::traveller_cell(&g, 0), (22, 19)); assert_eq!(read_route(&g, 0), Vec::<u8>::new());
        assert!(p1(&g).log.iter().any(|l| l == "Trav1 runs 14 squares"), "{:?}", p1(&g).log);
        move_to(&mut g, (22, 29), Speed::Auto);
        assert_eq!(round::traveller_cell(&g, 0), (22, 25)); assert_eq!(read_route(&g, 0), vec![3, 3, 3, 3], "four steps east kept as the 1990 route");
        assert_eq!(p1(&g).turns[0].mp, 0); assert!(round::key(&mut g, 0x4D), "no MP: the arrow is refused");
        select(&mut g, 1); select(&mut g, 0); assert_eq!(read_route(&g, 0), vec![3, 3, 3, 3]); assert_eq!(round::selected(&g), 0);
        perform(&mut g, Op::HoldHere, Target::Cell((0, 0))); assert!(read_route(&g, 0).is_empty());
        round::end_turn(&mut g); assert_eq!(p1(&g).strip, "Your move: Trav2");
        move_to(&mut g, (22, 12), Speed::Evade);
        let t = p1(&g).turns[1]; assert_eq!((t.speed, t.evade, t.mp), (2, true, 8), "6 squares evading at a run: 12 of 20 MP; {t:?}");
        assert_eq!(round::traveller_cell(&g, 1), (22, 12));
        round::end_turn(&mut g); wall(&mut g, (23, 8)); move_to(&mut g, (23, 8), Speed::Auto); assert_eq!(g.squad.as_ref().unwrap().orders.hover, "no path");
        assert!(!p1(&g).turns[2].moved);
    }
    #[test]
    fn fire_reload_target_and_the_menu() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); place(&mut g, 17, (28, 20), 2, 0, 42); fight(&mut g);
        wall(&mut g, (23, 6)); assert_eq!(fire_check(&g, 0, 14).err().as_deref(), Some("no line of sight"), "the wall at (23, 6) sits on the line (22, 5) -> (24, 7)");
        g.mem.write(Far::new(((23 * COLS as usize + 6) * 3 + 1) as u16, 0x8000), 0x20, 1);
        let shot = fire_check(&g, 0, 14).unwrap();
        assert_eq!((shot.band, shot.task.target, shot.task.dm, shot.task.needs()), (rules::Band::Short, 7, 3, 4));
        assert_eq!(shot.describe(), "Short: Routine 7+, DM +3 (skill 2, Dex +1), needs 4+ (91%)");
        hover(&mut g, Some(Target::Unit(Unit::Npc(14)))); assert!(g.squad.as_ref().unwrap().orders.hover.starts_with("Hostile (thug, NPC 14): Short: Routine 7+"), "{}", g.squad.as_ref().unwrap().orders.hover);
        let m = menu_for(&g, Target::Unit(Unit::Npc(14)));
        assert_eq!((m.items[m.default].label.as_str(), m.items[0].grey.as_deref()), ("Attack", None));
        assert_eq!(m.items.iter().find(|i| i.label == "Melee").unwrap().grey.as_deref(), Some("not adjacent"));
        assert!(m.items.iter().any(|i| i.label == "Target" && i.grey.is_none()));
        set_target(&mut g, 14); let w = Mem::walker(0); assert_eq!((g.mem.read(w.add(W_TARGET), 1), g.mem.read(w.add(W_AIM_X), 2)), (14, 48)); assert!(!p1(&g).turns[0].attacked);
        let pool = squad::npc_pool(&mut g, 14).unwrap();
        let before = (g.mem.party_u8(0x17B + 11), g.mem.party_u8(0x19F));
        fire(&mut g, 14);
        let line = p1(&g).log.iter().find(|l| l.starts_with("Trav1 fires the 9mm advanced combat rifle at thug (NPC 14): short: 2D")).expect("the log line").clone();
        let hit = line.contains("hit by");
        assert_eq!(g.mem.party_u8(0xA82 + 2), 19, "one round spent"); assert!(p1(&g).turns[0].attacked); assert_eq!(g.mem.party_fired(), 1);
        assert_eq!((g.mem.party_u8(0x17B + 11), g.mem.party_u8(0x19F)), if hit { (before.0 + 1, before.1 + 1) } else { (before.0 + 2, before.1 + 2) }, "a miss trains twice (f644b.c:128-131): {line}");
        if hit { assert!(g.squad.as_ref().unwrap().pools.get(&14).is_none_or(|&p| p < pool), "{line}") } else { assert_eq!(g.squad.as_ref().unwrap().pools[&14], pool) }
        assert_eq!(fire_check(&g, 0, 14).err().as_deref(), Some("no Attack left"));
        assert!(p1(&g).npcs.get(&14).is_none_or(|n| !n.acted) && !p1(&g).turns[0].done, "the Attack alone does not end the turn (RULES 1.3: the Move is still open)");
        round::end_turn(&mut g); for _ in 0..4 { round::end_turn(&mut g) } assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
        select(&mut g, 0); assert_eq!(round::selected(&g), 0);
        g.mem.set_party_u8(0xA82 + 2, 0);
        assert_eq!(fire_check(&g, 0, 14).err().as_deref(), Some("chamber empty"));
        let m = menu_for(&g, Target::Unit(Unit::Npc(14))); assert_eq!(m.items[m.default].label, "Reload"); assert_eq!(m.items[0].grey.as_deref(), Some("chamber empty"));
        default_verb(&mut g, Verb::Fire, Target::Unit(Unit::Npc(14)));
        assert_eq!((g.mem.party_u8(0xA82 + 1), g.mem.party_u8(0xA82 + 2)), (2, 20)); assert!(p1(&g).turns[0].attacked);
        assert_eq!(reload_check(&g, 0).err().as_deref(), Some("no Attack left"));
        let m = menu_for(&g, Target::Unit(Unit::Npc(17))); assert_eq!(m.items[m.default].label, "Target"); assert!(m.items.iter().any(|i| i.label == "Converse" && i.grey.as_deref() == Some("regroup to talk")));
        default_verb(&mut g, Verb::Fire, Target::Unit(Unit::Npc(17))); assert!(g.squad.as_ref().unwrap().orders.hover.starts_with("Neutral"));
        let m = menu_for(&g, Target::Unit(Unit::Traveller(0))); assert_eq!(m.items.iter().find(|i| i.label == "Regroup").unwrap().grey.as_deref(), Some("not in a fight"));
        swap_hand(&mut g); assert_eq!(g.mem.read(Mem::walker(0).add(W_HAND), 1), 1); swap_hand(&mut g);
        intent(&mut g, Intent::OpenMenu { target: Target::Unit(Unit::Npc(14)), at: (0, 0) }); assert!(g.squad.as_ref().unwrap().orders.menu.is_some());
        intent(&mut g, Intent::MenuPick(0)); assert!(g.squad.as_ref().unwrap().orders.menu.is_none()); assert_eq!(g.squad.as_ref().unwrap().orders.hover, "Attack: no Attack left");
        assert!(key(&mut g, 3)); assert_eq!(round::selected(&g), 1); assert!(key(&mut g, SCAN_F)); assert!(g.squad.as_ref().unwrap().orders.hover.starts_with("no target"));
        perform(&mut g, Op::Character(2), Target::Portrait(2)); assert_eq!(g.mem.key(), SCAN_F1 + 2); g.mem.set_key(0);
        g.squad = None; assert!(!key(&mut g, 3)); intent(&mut g, Intent::Select(2)); assert_eq!(g.mem.leader(), 1);
    }

    #[test]
    fn reload_writer_equals_try_reload_on_an_empty_chamber() {
        use crate::ground::state::Far;
        for w in rules::WEAPONS_PM.iter().filter(|w| w.rds > 0 && !matches!(w.profile, rules::Profile::Melee | rules::Profile::Thrown)) {
            let laser = (18..=20).contains(&w.id);
            let def_rds = if laser { 20 } else { w.rds as u32 };
            let arm = |g: &mut Ground| { g.mem.set_party_u8(0xA82, w.id); g.mem.set_party_u8(0xA82 + 1, 3); g.mem.set_party_u8(0xA82 + 2, 0); g.mem.write(Far::new(46 * w.id as u16 + 0x28, ITEMS), def_rds, 1); };
            let mut a = ground(); arm(&mut a); reload(&mut a);
            let mut b = ground(); arm(&mut b); let rc = walk::try_reload(&mut b, 0);
            let (na, nb) = ((a.mem.party_u8(0xA82 + 1), a.mem.party_u8(0xA82 + 2)), (b.mem.party_u8(0xA82 + 1), b.mem.party_u8(0xA82 + 2)));
            assert_eq!(rc, 1, "try_reload reloads the {}", w.name);
            if laser { assert_eq!((na, nb), ((2, w.rds as u8), (2, def_rds as u8)), "{}: the PM count pinned (O-15), the 1990 byte differs", w.name) }
            else { assert_eq!(na, nb, "{}: the new writer and try_reload write the same two bytes", w.name); assert_eq!(na, (2, w.rds as u8)) }
            assert_eq!(a.squad.as_ref().unwrap().p1.log.last().map(|l| l.starts_with("Trav1 reloads (")), Some(true), "{}: {:?}", w.name, a.squad.as_ref().unwrap().p1.log);
        }
    }
    #[test]
    fn a_p3_route_decodes_and_walks_with_move_party() {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g); assert_eq!(round::traveller_cell(&g, 0), (22, 5));
        write_route(&mut g, 0, &[1, 1, 1, 3, 3]); assert_eq!(destination(&g, 0), Some((19, 7)));
        let route = read_route(&g, 0); write_route(&mut g, 0, &[]);
        for d in route {
            let (dr, dc) = (g.mem.i8(0x2F5 + d as u16 * 2) as i16, g.mem.i8(0x2F6 + d as u16 * 2) as i16);
            let start = round::traveller_cell(&g, 0); let want = (start.0 + dr, start.1 + dc); recentre(&mut g, want);
            for _ in 0..4 { if round::traveller_cell(&g, 0) == want && (g.mem.x() | g.mem.y()) & 1 == 0 { break } walk::move_party(&mut g, dr, dc, d as i16); }
            assert_eq!(round::traveller_cell(&g, 0), want, "route byte {d}");
        }
        assert_eq!(round::traveller_cell(&g, 0), (19, 7), "three squares north, two east, as 32F0 would walk them");
    }
}
