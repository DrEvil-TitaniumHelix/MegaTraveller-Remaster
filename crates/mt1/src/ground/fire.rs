use super::{combat, commands, orders::{self, Shot}, render, round::{self, Order, ENCOUNTER_RADIUS}, rules::{self, adapters, weapons::{Profile, Recoil, Weapon}}, squad, state::{Far, Ground, Mem, PARTY_PTR, W_BUILTIN}, tactics, walk};
use crate::ui::{Cell, Unit};

pub const PATH_DANGER: u8 = 4;
pub const RAPID_TARGETS: usize = 3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Opts { pub pinpoint: bool, pub rapid: bool, pub burst: bool, pub disarm: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pending { pub who: Unit, pub cell: Cell, pub weapon: u8, pub task: rules::Task, pub round: u16, pub launcher: bool }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State { pub rapid: Option<Vec<u8>>, pub reloading: [bool; 5], pub disarmed: Vec<u8>, pub pending: Vec<Pending> }

fn st(g: &mut Ground) -> Option<&mut State> { g.squad.as_mut().map(|s| &mut s.fire) }
fn rd(g: &Ground) -> Option<&State> { g.squad.as_ref().map(|s| &s.fire) }
fn log(g: &mut Ground, line: String) { orders::log(g, line) }
fn say(g: &mut Ground, t: &str) { orders::say(g, t) }
fn record(g: &Ground, who: u8) -> Far { g.mem.pointer(PARTY_PTR).add((who as i32) << 8) }
fn bump(g: &mut Ground, rec: Far, skill_off: i32) { for off in [skill_off, 0x19F] { let v = g.mem.read(rec.add(off), 1); g.mem.write(rec.add(off), v + 1, 1) } }
fn in_map(g: &Ground, (r, c): Cell) -> bool { r >= 0 && c >= 0 && r < g.mem.map_blocks_high() as i16 * 12 && c < g.mem.map_stride() }
fn current_round(g: &Ground) -> u16 { g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref()).map_or(0, |e| e.round) }
fn turn(g: &Ground, who: u8) -> round::Turn { g.squad.as_ref().map_or(round::Turn::default(), |s| s.p1.turns[(who as usize).min(4)]) }
fn doubled(c: Cell) -> (i32, i32) { (c.0 as i32 * 2, c.1 as i32 * 2) }
fn present(g: &Ground, u: Unit) -> bool { match u { Unit::Npc(k) => round::npc_present(g, k), Unit::Traveller(i) => round::alive(g, i) } }
pub fn hostile(g: &Ground, attacker: Unit, u: Unit) -> bool { match (attacker, u) { (Unit::Traveller(_), Unit::Npc(k)) => orders::attackable(g, k), (Unit::Npc(_), Unit::Traveller(i)) => round::alive(g, i), _ => false } }
pub fn verdict(o: &rules::Outcome) -> String { match o.level { Some(l) if o.hit => format!("hit by {} ({})", o.margin, l.name()), _ if o.fumble => "fumble".into(), _ => "miss".into() } }

pub fn blocked(g: &Ground, c: Cell) -> bool {
    if !in_map(g, c) { return true }
    let cell = commands::cell(g, c.0, c.1);
    combat::cell_enterable(g, cell, g.mem.read(cell.add(1), 1) as i32 / 20, 0) == 0
}
fn neighbours((r, c): Cell) -> [Cell; 8] { [(r - 1, c - 1), (r - 1, c), (r - 1, c + 1), (r, c - 1), (r, c + 1), (r + 1, c - 1), (r + 1, c), (r + 1, c + 1)] }
pub fn occupants(g: &Ground, c: Cell) -> Vec<Unit> {
    let mut out = Vec::new();
    if in_map(g, c) { let k = g.mem.read(commands::cell(g, c.0, c.1).add(2), 1) as u8 & 0x3F; if k != 0 && round::npc_present(g, k) { out.push(Unit::Npc(k)) } }
    out.extend((0..5).filter(|&i| round::alive(g, i) && round::traveller_cell(g, i) == c).map(Unit::Traveller));
    out
}
pub fn label(g: &Ground, u: Unit) -> String { match u { Unit::Traveller(i) => round::name(g, i), Unit::Npc(k) => round::tagged(&round::npc_fighter(g, k).label, k) } }
pub fn in_cover(g: &Ground, from: Cell, to: Cell) -> bool {
    let d = round::squares(from, to);
    d > 1 && neighbours(to).into_iter().any(|n| round::squares(from, n) < d && blocked(g, n))
}
pub fn line_cells(g: &Ground, from: Cell, to: Cell) -> Vec<Cell> {
    let (dr, dc) = (to.0 as i32 - from.0 as i32, to.1 as i32 - from.1 as i32);
    let n = dr.abs().max(dc.abs());
    let mut out = Vec::new();
    if n == 0 { return out }
    for i in 1..=ENCOUNTER_RADIUS {
        let c = ((from.0 as i32 + (dr * i * 2 + n).div_euclid(2 * n)) as i16, (from.1 as i32 + (dc * i * 2 + n).div_euclid(2 * n)) as i16);
        if c != to && blocked(g, c) { break }
        out.push(c);
    }
    out
}
pub fn line_candidates(g: &Ground, attacker: Unit, from: Cell, to: Cell, friendlies: bool) -> Vec<(Unit, Cell)> {
    let band = rules::band(round::squares(from, to));
    line_cells(g, from, to).into_iter().filter(|&c| rules::band(round::squares(from, c)) == band)
        .flat_map(|c| occupants(g, c).into_iter().map(move |u| (u, c)))
        .filter(|&(u, _)| u != attacker && (friendlies || hostile(g, attacker, u)))
        .collect()
}
pub fn line_warning(g: &Ground, who: u8, k: u8) -> String {
    if !adapters::traveller_weapon(g, who).is_some_and(|w| matches!(w.profile, Profile::Handgun | Profile::Rifle)) { return String::new() }
    let (from, to) = (round::traveller_cell(g, who), round::npc_cell(g, k));
    let names: Vec<String> = line_candidates(g, Unit::Traveller(who), from, to, true).into_iter()
        .filter(|&(u, _)| !matches!(u, Unit::Npc(n) if orders::attackable(g, n))).map(|(u, _)| label(g, u)).collect();
    if names.is_empty() { String::new() } else { format!("; {} in the line!", names.join(", ")) }
}
pub fn ring(g: &Ground, who: u8, centre: Cell) -> Vec<(Cell, u8)> {
    let Some(w) = adapters::traveller_weapon(g, who) else { return Vec::new() };
    let r = rules::danger_radius(w.danger_dm) as i16;
    if r == 0 { return Vec::new() }
    (-r..=r).flat_map(|dr| (-r..=r).map(move |dc| (dr, dc))).filter(|&(dr, dc)| dr.abs() == r || dc.abs() == r)
        .map(|(dr, dc)| ((centre.0 + dr, centre.1 + dc), PATH_DANGER)).filter(|(c, _)| in_map(g, *c)).collect()
}

fn armour_of(g: &Ground, u: Unit, w: &Weapon, halve: bool) -> (i32, bool) {
    let a = match u { Unit::Traveller(i) => round::traveller_fighter(g, i).armour, Unit::Npc(k) => round::npc_fighter(g, k).armour };
    let av = if w.ignores_armour { 0 } else { a.vs(w.class) };
    (if halve { av / 2 } else { av }, a.exposed)
}
pub fn apply(g: &mut Ground, u: Unit, dmg: i32, pen: i32, av: i32) -> bool {
    let text = if dmg == 255 { "knock-out".to_string() } else { format!("{dmg} damage") };
    match u {
        Unit::Npc(k) => {
            let lbl = label(g, u); let (r, c) = round::npc_cell(g, k);
            let died = squad::npc_take(g, k, dmg);
            if let Some(f) = round::fx(g) { f.hits.push((u, (r, c), dmg)) }
            if died { walk::set_cell_flags(g, r * 2, c * 2, 0x80); g.mem.set_event_id(-1); g.mem.set_impact_sound(1); g.mem.set_party_u8(0x81, 1); log(g, format!("{lbl} is dead")) }
            else { if g.mem.extra_sound() == 0 { g.mem.set_extra_sound(6) } log(g, format!("Pen {pen} vs AV {av}: {text} to {lbl}")) }
            died
        }
        Unit::Traveller(i) => {
            let n = round::name(g, i); let cell = round::traveller_cell(g, i);
            let r = squad::traveller_take(g, i, dmg);
            if let Some(f) = round::fx(g) { f.hits.push((u, cell, dmg)) }
            render::status_panel(g);
            log(g, format!("Pen {pen} vs AV {av}: {text} to {n}{}", match r { 2 => ": dead", 3 => ": down", _ => "" }));
            r == 2
        }
    }
}
pub fn roll_on(g: &mut Ground, u: Unit, w: &Weapon, task: &rules::Task, pen: i32, pinpoint: bool, what: &str) -> rules::Outcome {
    let dice = squad::two_d(g);
    let out = rules::resolve(task, dice);
    let lbl = label(g, u);
    log(g, format!("  {what} {lbl}: 2D {}+{} {:+} = {} vs {} {}", dice.0, dice.1, task.dm, out.total, task.target, verdict(&out)));
    if let (true, Some(level)) = (out.hit, out.level) {
        let (av, exposed) = armour_of(g, u, w, pinpoint);
        let dmg = if w.dmg == 255 { 255 } else { rules::damage(pen, av, exposed, level, w.dmg as i32) };
        apply(g, u, dmg, pen, av);
    }
    out
}
pub fn blast(g: &mut Ground, centre: Cell, w: &Weapon, task: &rules::Task, pen: i32, skip: &[Unit]) -> Vec<Unit> {
    let radius = rules::danger_radius(w.danger_dm);
    let mut cells: Vec<(i32, Cell)> = (-radius..=radius).flat_map(|dr| (-radius..=radius).map(move |dc| (dr.abs().max(dc.abs()), (centre.0 + dr as i16, centre.1 + dc as i16)))).collect();
    cells.sort();
    let mut hit = Vec::new();
    for (d, c) in cells {
        let p = rules::danger_pen(pen, d);
        if p <= 0 && d > 0 { continue }
        for u in occupants(g, c) {
            if skip.contains(&u) || hit.contains(&u) { continue }
            if roll_on(g, u, w, task, p, false, &format!("blast ({d} sq, Pen {p}) on")).hit { hit.push(u) }
        }
    }
    hit
}
fn explode(g: &mut Ground, p: &Pending) {
    let Some(w) = rules::weapon(p.weapon) else { return };
    let (r, c) = p.cell;
    let n = label(g, p.who);
    round::fx_begin(g, p.who); if let Some(f) = round::fx(g) { f.blasts.push(p.cell) }
    log(g, format!("{n}'s {} {} at ({r}, {c})", w.name, if p.launcher { "lands" } else { "goes off" }));
    if in_map(g, p.cell) {
        let cell = commands::cell(g, r, c);
        let terrain = (g.mem.read(cell.add(1), 1) / 20) as i16;
        let interior = g.mem.interior_kind() == b'I';
        if (!interior && walk::vehicle_terrain(g, cell, terrain) != 0) || (interior && terrain < 5) {
            let f = g.mem.read(cell, 1); g.mem.write(cell, f & !1, 1);
            g.mem.write(cell.add(1), 0x23 + if p.launcher { 0 } else { (p.weapon - 0x81) as u32 }, 1);
            g.mem.set_event_id(-1);
        }
    }
    g.mem.set_extra_sound(7);
    if let Some(e) = g.squad.as_mut().and_then(|s| s.p1.encounter.as_mut()) { if e.surprised.take().is_some() { log(g, "The alarm is raised: surprise lost".into()) } }
    if blast(g, p.cell, w, &p.task, w.pen as i32, &[]).is_empty() { render::front(g); combat::flash_marker(g, -1, r as i32 * 2, c as i32 * 2); render::back(g) }
    if g.mem.npc_attacking() == 0 { walk::play_pending_sounds(g) }
    g.mem.set_redraw_pending(1);
}
fn drain(g: &mut Ground, pred: impl Fn(&Pending) -> bool) -> Vec<Pending> {
    let Some(s) = st(g) else { return Vec::new() };
    let (take, keep): (Vec<Pending>, Vec<Pending>) = std::mem::take(&mut s.pending).into_iter().partition(|p| pred(p));
    s.pending = keep; take
}

pub fn turn_over(g: &mut Ground, who: Unit) { for p in drain(g, |p| !p.launcher && p.who == who) { explode(g, &p) } }
pub fn turn_start(g: &mut Ground, who: Unit) -> bool {
    let round = current_round(g);
    let due = drain(g, |p| p.launcher && p.who == who && round > p.round);
    let any = !due.is_empty();
    for p in due { explode(g, &p) }
    any
}
pub fn round_start(g: &mut Ground) {
    for i in 0..5u8 {
        if !rd(g).is_some_and(|s| s.reloading[i as usize]) { continue }
        st(g).unwrap().reloading[i as usize] = false;
        if let Some(s) = g.squad.as_mut() { s.p1.turns[i as usize].attacked = true }
        let n = round::name(g, i);
        let (obj, def) = orders::hand(g, i);
        let mags = if obj == Far::default() { 0 } else { g.mem.read(obj.add(1), 1) };
        if mags > 0 {
            let rds = adapters::item_id(g, def).and_then(rules::weapon).map_or_else(|| g.mem.read(def.add(0x28), 1), |w| w.rds as u32);
            g.mem.write(obj.add(1), mags - 1, 1); g.mem.write(obj.add(2), rds, 1);
            log(g, format!("{n} is reloading after rapid fire ({rds} rounds, {} magazine{} left)", mags - 1, if mags == 2 { "" } else { "s" }));
        } else { log(g, format!("{n} spends the round reloading: no magazine")) }
    }
    if let Some(s) = st(g) { s.disarmed.clear() }
    let round = current_round(g);
    let mut keep = Vec::new();
    for p in drain(g, |p| p.launcher && round > p.round) {
        let gone = match p.who { Unit::Traveller(i) => !round::conscious(g, i), Unit::Npc(k) => !round::npc_present(g, k) };
        if gone { explode(g, &p) } else { keep.push(p) }
    }
    if let Some(s) = st(g) { s.pending.extend(keep) }
}
pub fn settle_all(g: &mut Ground) { for p in drain(g, |_| true) { explode(g, &p) } }

pub fn attack(g: &mut Ground, k: u8, o: Opts) {
    let who = round::selected(g);
    if rapid_add(g, k) { return }
    let Some(w) = adapters::traveller_weapon(g, who) else { say(g, "no weapon in hand"); return };
    match w.profile {
        Profile::Melee => melee(g, k, o.disarm),
        Profile::Thrown => throw(g, round::npc_cell(g, k)),
        Profile::Indirect => launch(g, round::npc_cell(g, k)),
        _ => shoot(g, k, o),
    }
    round::engage(g, k);
}
fn finish(g: &mut Ground, silent: bool, hit_standing: bool) {
    tactics::spend(g, round::selected(g));
    if !silent || hit_standing { if let Some(e) = g.squad.as_mut().and_then(|s| s.p1.encounter.as_mut()) { if e.surprised.take().is_some() { log(g, "The alarm is raised: surprise lost".into()) } } }
    if g.mem.npc_attacking() == 0 { walk::play_pending_sounds(g) }
    render::walker_panel(g, false, true);
    g.mem.set_party_fired(1);
    if let Some(s) = g.squad.as_mut() { s.orders.hover.clear(); s.orders.path.clear() }
    if round::in_encounter(g) { round::order(g, Order::Attack) } else { settle_all(g) }
    g.mem.set_redraw_pending(1);
}
pub fn burst_check(g: &Ground, who: u8) -> Result<(), String> {
    let (obj, _) = orders::hand(g, who);
    if obj != Far::default() && g.mem.read(obj.add(2), 1) >= 2 { Ok(()) } else { Err("a burst needs two rounds or more".into()) }
}
pub fn line_fire(g: &mut Ground, attacker: Unit, primary: Unit, from: Cell, to: Cell, w: &Weapon, task: &rules::Task, out: rules::Outcome, pen: i32, pinpoint: bool, av: i32, exposed: bool) -> (Vec<Unit>, Vec<Unit>, Cell) {
    let (mut attacked, mut hits, mut landed) = (Vec::new(), Vec::new(), to);
    for (u, c) in line_candidates(g, attacker, from, to, out.exceptional_failure) {
        if attacked.contains(&u) || (u != primary && c == to) { continue }
        let hit = if u == primary {
            if let (true, Some(level)) = (out.hit, out.level) {
                let dmg = if w.dmg == 255 { 255 } else { rules::damage(pen, av, exposed, level, w.dmg as i32) };
                apply(g, primary, dmg, pen, av);
            }
            out.hit
        } else { roll_on(g, u, w, task, pen, pinpoint, "in the line:").hit };
        attacked.push(u);
        if hit { hits.push(u); landed = c; break }
    }
    (attacked, hits, landed)
}
pub fn burst_targets(g: &Ground, attacker: Unit, primary: Unit, from: Cell, to: Cell, n: usize, attacked: &[Unit]) -> Vec<Unit> {
    let line = line_cells(g, from, to);
    let mut adj: Vec<(bool, i32, Unit)> = neighbours(to).into_iter().flat_map(|c| occupants(g, c).into_iter().map(move |u| (c, u)))
        .filter(|&(_, u)| hostile(g, attacker, u) && !attacked.contains(&u))
        .map(|(c, u)| (!line.contains(&c), round::squares(from, c), u)).collect();
    adj.sort_by_key(|x| (x.0, x.1)); adj.dedup_by_key(|x| x.2);
    let mut out: Vec<Unit> = adj.into_iter().map(|x| x.2).take(n).collect();
    let others_in_line = line_candidates(g, attacker, from, to, false).into_iter().any(|(u, _)| u != primary);
    if !others_in_line && present(g, primary) { while out.len() < n { out.push(primary) } }
    out
}
pub fn shoot(g: &mut Ground, k: u8, o: Opts) {
    let who = round::selected(g);
    let shot = match orders::fire_check_opts(g, who, k, o) { Ok(s) => s, Err(r) => { say(g, &r); return } };
    let full = o.burst || (!o.rapid && !o.pinpoint && shot.weapon.auto > 0 && g.squad.as_ref().is_some_and(|s| s.orders.full_auto[who as usize]));
    if full && shot.weapon.auto == 0 { say(g, "no autofire"); return }
    if full { if let Err(r) = burst_check(g, who) { say(g, &r); return } }
    if o.rapid { if let Some(s) = st(g) { s.rapid = Some(vec![k]) } say(g, &format!("Rapid fire at {}: click up to {} more hostiles in the arc, Enter fires", shot.label, RAPID_TARGETS - 1)); return }
    let name = round::name(g, who);
    let rec = record(g, who); let skill_off = 0x17B + shot.weapon.skill as i32;
    bump(g, rec, skill_off);
    orders::set_target(g, k);
    let (obj, def) = orders::hand(g, who);
    let kind = g.mem.read(def.add(0x27), 1) as i32;
    let ((row, col), (tr, tc)) = (doubled(shot.from), doubled(shot.to));
    combat::draw_shot(g, kind, row, col, tr, tc, who as i32 + 1);
    let clip = g.mem.read(obj.add(2), 1);
    let rounds = if full { (rules::rounds_per_burst(shot.weapon.auto) as u32).min(clip) } else { 1 };
    if kind & 8 != 0 { g.mem.write(obj.add(2), clip.saturating_sub(rounds), 1) }
    let dice = squad::two_d(g);
    let out = rules::resolve(&shot.task, dice);
    let mode = if full { format!(" (burst, {rounds} rounds)") } else if o.pinpoint { " (pinpoint)".into() } else { String::new() };
    log(g, format!("{name} fires the {} at {}: {}: 2D {}+{} {:+} = {} vs {} {}{mode}", shot.weapon.name, round::tagged(&shot.label, k), shot.band.name(), dice.0, dice.1, shot.task.dm, out.total, shot.task.target, verdict(&out)));
    let pen = rules::pen_at(shot.weapon.pen as i32, shot.weapon.atten, shot.band);
    let (mut attacked, mut hits, landed) = line_fire(g, Unit::Traveller(who), Unit::Npc(k), shot.from, shot.to, shot.weapon, &shot.task, out, pen, o.pinpoint, shot.av, shot.exposed);
    if full {
        for u in burst_targets(g, Unit::Traveller(who), Unit::Npc(k), shot.from, shot.to, shot.weapon.auto as usize, &attacked) {
            if !present(g, u) { continue }
            if roll_on(g, u, shot.weapon, &shot.task, pen, o.pinpoint, "burst on").hit { hits.push(u) }
            attacked.push(u);
        }
    }
    if rules::danger_radius(shot.weapon.danger_dm) > 0 { attacked.push(Unit::Traveller(who)); hits.extend(blast(g, landed, shot.weapon, &shot.task, pen, &attacked)) }
    if hits.is_empty() { render::front(g); combat::flash_marker(g, -1, tr, tc); render::back(g); bump(g, rec, skill_off) }
    let standing = hits.iter().any(|&u| present(g, u));
    finish(g, matches!(kind, 0xAA | 0xAB), standing);
}

pub fn in_arc(from: Cell, first: Cell, other: Cell) -> bool {
    let (a, b) = (((first.0 - from.0) as i64, (first.1 - from.1) as i64), ((other.0 - from.0) as i64, (other.1 - from.1) as i64));
    let dot = a.0 * b.0 + a.1 * b.1;
    dot > 0 && dot * dot * 10000 >= 8536 * (a.0 * a.0 + a.1 * a.1) * (b.0 * b.0 + b.1 * b.1)
}
pub fn rapid_add(g: &mut Ground, k: u8) -> bool {
    let Some(mut list) = rd(g).and_then(|s| s.rapid.clone()) else { return false };
    let who = round::selected(g);
    if list.contains(&k) { say(g, "already a target"); return true }
    if !orders::attackable(g, k) { say(g, "not a hostile"); return true }
    let from = round::traveller_cell(g, who);
    if !in_arc(from, round::npc_cell(g, list[0]), round::npc_cell(g, k)) { say(g, "outside the 45 degree arc"); return true }
    if let Err(r) = orders::fire_check_opts(g, who, k, Opts { rapid: true, ..Opts::default() }) { say(g, &r); return true }
    list.push(k);
    let n = list.len();
    if let Some(s) = st(g) { s.rapid = Some(list) }
    if n >= RAPID_TARGETS { rapid_fire(g) } else { say(g, &format!("Rapid fire: {n} of {RAPID_TARGETS} targets; Enter fires")) }
    true
}
pub fn rapid_enter(g: &mut Ground) -> bool { if rd(g).is_some_and(|s| s.rapid.is_some()) { rapid_fire(g); true } else { false } }
fn rapid_fire(g: &mut Ground) {
    let Some(list) = st(g).and_then(|s| s.rapid.take()) else { return };
    let who = round::selected(g);
    let name = round::name(g, who);
    let (obj, def) = orders::hand(g, who);
    let kind = g.mem.read(def.add(0x27), 1) as i32;
    let rec = record(g, who);
    let mut standing = false;
    if let Some(r) = list.iter().map(|&k| orders::fire_check_opts(g, who, k, Opts { rapid: true, ..Opts::default() }).err()).collect::<Option<Vec<_>>>() { say(g, &format!("Rapid fire: {}", r.first().map_or("no target", |s| s.as_str()))); return }
    for &k in &list {
        let shot = match orders::fire_check_opts(g, who, k, Opts { rapid: true, ..Opts::default() }) { Ok(s) => s, Err(r) => { log(g, format!("  NPC {k}: {r}")); continue } };
        let skill_off = 0x17B + shot.weapon.skill as i32;
        bump(g, rec, skill_off);
        orders::set_target(g, k);
        let ((row, col), (tr, tc)) = (doubled(shot.from), doubled(shot.to));
        combat::draw_shot(g, kind, row, col, tr, tc, who as i32 + 1);
        let dice = squad::two_d(g);
        let out = rules::resolve(&shot.task, dice);
        log(g, format!("{name} rapid-fires the {} at {}: {}: 2D {}+{} {:+} = {} vs {} {}", shot.weapon.name, round::tagged(&shot.label, k), shot.band.name(), dice.0, dice.1, shot.task.dm, out.total, shot.task.target, verdict(&out)));
        let pen = rules::pen_at(shot.weapon.pen as i32, shot.weapon.atten, shot.band);
        let (_, hits, _) = line_fire(g, Unit::Traveller(who), Unit::Npc(k), shot.from, shot.to, shot.weapon, &shot.task, out, pen, false, shot.av, shot.exposed);
        if hits.is_empty() { render::front(g); combat::flash_marker(g, -1, tr, tc); render::back(g); bump(g, rec, skill_off) }
        standing |= hits.iter().any(|&u| present(g, u));
    }
    if obj != Far::default() && kind & 8 != 0 { g.mem.write(obj.add(2), 0, 1) }
    if let Some(s) = st(g) { s.reloading[(who as usize).min(4)] = true }
    log(g, format!("{name}'s clip is empty: next round reloading"));
    finish(g, matches!(kind, 0xAA | 0xAB), standing);
}

pub fn melee_check(g: &Ground, who: u8, k: u8) -> Result<Shot, String> {
    if !round::npc_present(g, k) { return Err("gone".into()) }
    orders::turn_check(g).map_err(String::from)?;
    let (from, to) = (round::traveller_cell(g, who), round::npc_cell(g, k));
    if round::squares(from, to) > 1 { return Err("not adjacent".into()) }
    let w = adapters::traveller_weapon(g, who).ok_or("no weapon in hand")?;
    if w.profile != Profile::Melee { return Err("no melee weapon in hand".into()) }
    let t = turn(g, who);
    if round::in_encounter(g) && t.attacked { return Err("no Attack left".into()) }
    if round::in_encounter(g) && t.speed == 2 { return Err("ran: no melee this turn".into()) }
    let f = round::traveller_fighter(g, who);
    let skill = f.skill(g, &g.squad.as_ref().unwrap().compendium, w);
    let d = round::npc_fighter(g, k);
    let (dskill, block) = defence(g, Unit::Npc(k));
    let str_dm = rules::char_dm(f.strength);
    let task = melee_task(skill, str_dm, dskill, block);
    let mut terms = vec![format!("skill {}", skill.level), format!("Str {str_dm:+}"), format!("vs skill {} block {block}", dskill.level)];
    if skill.unskilled { terms.push("unskilled".into()) }
    if dskill.unskilled { terms.push("defender unskilled".into()) }
    Ok(tactics::apply(g, Shot { weapon: w, task, band: rules::Band::Close, from, to, skill, dex: f.dex, movement: 0, av: d.armour.vs(0x00), exposed: d.armour.exposed, label: d.label, terms, note: String::new(), cover: false, tactics: 0 }))
}
pub fn defence(g: &Ground, u: Unit) -> (adapters::Skill, i32) {
    let (f, w, disarmed) = match u {
        Unit::Npc(k) => (round::npc_fighter(g, k), adapters::npc_weapon(g, k), rd(g).is_some_and(|s| s.disarmed.contains(&k))),
        Unit::Traveller(i) => (round::traveller_fighter(g, i), adapters::traveller_weapon(g, i), false),
    };
    match w { Some(dw) if dw.profile == Profile::Melee && !disarmed => (f.skill(g, &g.squad.as_ref().unwrap().compendium, dw), dw.block as i32), _ => (adapters::Skill { level: 0, unskilled: true }, 1) }
}
pub fn melee_task(skill: adapters::Skill, str_dm: i32, dskill: adapters::Skill, block: i32) -> rules::Task {
    let difficulty = rules::Difficulty::Routine.harder(skill.unskilled as i32 - dskill.unskilled as i32);
    rules::Task { difficulty, target: difficulty.target(), dm: (skill.level + str_dm - dskill.level - block).clamp(-rules::DM_CAP, rules::DM_CAP) }
}
pub fn melee(g: &mut Ground, k: u8, disarm: bool) {
    let who = round::selected(g);
    let shot = match melee_check(g, who, k) { Ok(s) => s, Err(r) => { say(g, &r); return } };
    let name = round::name(g, who);
    let rec = record(g, who); let skill_off = 0x17B + shot.weapon.skill as i32;
    bump(g, rec, skill_off);
    orders::set_target(g, k);
    let ((row, col), (tr, tc)) = (doubled(shot.from), doubled(shot.to));
    combat::draw_shot(g, 0, row, col, tr, tc, who as i32 + 1);
    let dice = squad::two_d(g);
    let out = rules::resolve(&shot.task, dice);
    log(g, format!("{name} attacks {} with the {}: close: 2D {}+{} {:+} = {} vs {} {}{}", round::tagged(&shot.label, k), shot.weapon.name, dice.0, dice.1, shot.task.dm, out.total, shot.task.target, verdict(&out), if disarm { " (disarm)" } else { "" }));
    let mut standing = false;
    if let (true, Some(level)) = (out.hit, out.level) {
        if disarm { if let Some(s) = st(g) { s.disarmed.push(k) } let lbl = label(g, Unit::Npc(k)); log(g, format!("{lbl} is disarmed for the round")); standing = true }
        else { let pen = shot.weapon.pen as i32; let dmg = rules::damage(pen, shot.av, shot.exposed, level, shot.weapon.dmg as i32); standing = !apply(g, Unit::Npc(k), dmg, pen, shot.av) }
    } else { render::front(g); combat::flash_marker(g, -1, tr, tc); render::back(g); bump(g, rec, skill_off) }
    finish(g, true, standing);
    round::end_turn(g);
}

fn grenade_count(g: &Ground, who: u8, u: u8) -> u32 { g.mem.read(record(g, who).add(0x1CA + u as i32), 1) }
pub fn aims_at_squares(g: &Ground, who: u8) -> bool { g.mem.read(Mem::walker(who as i16).add(W_BUILTIN), 1) != 0 }
pub fn throw_check(g: &Ground, who: u8, to: Cell) -> Result<Shot, String> {
    orders::turn_check(g).map_err(String::from)?;
    let u = g.mem.read(Mem::walker(who as i16).add(W_BUILTIN), 1) as u8;
    let w = rules::grenade(u).ok_or("choose a grenade: T")?;
    if grenade_count(g, who, u) == 0 { return Err(format!("no {} left", w.name)) }
    if round::in_encounter(g) && turn(g, who).attacked { return Err("no Attack left".into()) }
    let from = round::traveller_cell(g, who);
    if from == to { return Err("not at your own square".into()) }
    if !in_map(g, to) { return Err("off the map".into()) }
    let band = g.balance.throw_band(round::squares(from, to));
    let base = rules::difficulty(Profile::Thrown, band, false).filter(|_| band <= w.max).ok_or("out of range")?;
    let f = round::traveller_fighter(g, who);
    let (s, d) = (rules::char_dm(f.strength), rules::char_dm(f.dex));
    let dm = (s + d).clamp(-rules::DM_CAP, rules::DM_CAP);
    Ok(tactics::apply(g, Shot { weapon: w, task: rules::Task { difficulty: base, target: base.target(), dm }, band, from, to, skill: adapters::Skill { level: 0, unskilled: false }, dex: f.dex, movement: 0,
        av: 0, exposed: true, label: format!("({}, {})", to.0, to.1), terms: vec![format!("Str {s:+}"), format!("Dex {d:+}")], note: "goes off at the end of the turn".into(), cover: false, tactics: 0 }))
}
pub fn scatter(g: &mut Ground, from: Cell) -> Cell {
    const DIRS: [(i16, i16); 6] = [(-1, 0), (-1, 1), (1, 1), (1, 0), (1, -1), (-1, -1)];
    let d = DIRS[(walk::random(g, 6) as usize).min(5)];
    let c = (from.0 + d.0, from.1 + d.1);
    if blocked(g, c) { from } else { c }
}
pub fn throw(g: &mut Ground, to: Cell) {
    let who = round::selected(g);
    let shot = match throw_check(g, who, to) { Ok(s) => s, Err(r) => { say(g, &r); return } };
    let name = round::name(g, who);
    let u = (shot.weapon.id - 0x80) as i32;
    let rec = record(g, who);
    let n = g.mem.read(rec.add(0x1CA + u), 1); g.mem.write(rec.add(0x1CA + u), n - 1, 1);
    g.mem.write(Mem::walker(who as i16).add(W_BUILTIN), 0, 1);
    let ((row, col), (tr, tc)) = (doubled(shot.from), doubled(shot.to));
    combat::draw_shot(g, 0x49, row, col, tr, tc, who as i32 + 1);
    let dice = squad::two_d(g);
    let out = rules::resolve(&shot.task, dice);
    let cell = if out.hit { to } else { scatter(g, to) };
    log(g, format!("{name} throws an {} at {}: {}: 2D {}+{} {:+} = {} vs {} {}{}{}", shot.weapon.name, shot.label, shot.band.name(), dice.0, dice.1, shot.task.dm, out.total, shot.task.target, verdict(&out), if cell != to { format!(", scatter to ({}, {})", cell.0, cell.1) } else { String::new() }, orders::attack_used(g)));
    let round = current_round(g);
    if let Some(s) = st(g) { s.pending.push(Pending { who: Unit::Traveller(who), cell, weapon: shot.weapon.id, task: shot.task, round, launcher: false }) }
    finish(g, true, false);
}

pub fn launch_check(g: &Ground, who: u8, to: Cell) -> Result<Shot, String> {
    orders::turn_check(g).map_err(String::from)?;
    let w = adapters::traveller_weapon(g, who).ok_or("no weapon in hand")?;
    if w.profile != Profile::Indirect { return Err("no launcher in hand".into()) }
    if g.mem.interior_kind() == b'I' { return Err("not indoors".into()) }
    let t = turn(g, who);
    if round::in_encounter(g) && t.attacked { return Err("no Attack left".into()) }
    if orders::chamber_empty(g, who) { return Err("chamber empty".into()) }
    if round::in_encounter(g) && w.recoil == Recoil::Hi && t.moved { return Err("moved: high recoil".into()) }
    let from = round::traveller_cell(g, who);
    if from == to { return Err("not at your own square".into()) }
    if !in_map(g, to) { return Err("off the map".into()) }
    let band = rules::band(round::squares(from, to));
    if band > w.max { return Err("out of range".into()) }
    let f = round::traveller_fighter(g, who);
    let skill = f.skill(g, &g.squad.as_ref().unwrap().compendium, w);
    let d = rules::Difficulty::Difficult;
    Ok(tactics::apply(g, Shot { weapon: w, task: rules::Task { difficulty: d, target: d.target(), dm: skill.level.clamp(-rules::DM_CAP, rules::DM_CAP) }, band, from, to, skill: adapters::Skill { unskilled: false, ..skill }, dex: f.dex, movement: 0,
        av: 0, exposed: true, label: format!("({}, {})", to.0, to.1), terms: vec![format!("Forward Observer {}", skill.level)], note: "lands at the start of your next turn".into(), cover: false, tactics: 0 }))
}
pub fn launch(g: &mut Ground, to: Cell) {
    let who = round::selected(g);
    let shot = match launch_check(g, who, to) { Ok(s) => s, Err(r) => { say(g, &r); return } };
    let name = round::name(g, who);
    let rec = record(g, who); let skill_off = 0x17B + shot.weapon.skill as i32;
    bump(g, rec, skill_off);
    let (obj, def) = orders::hand(g, who);
    let kind = g.mem.read(def.add(0x27), 1) as i32;
    let ((row, col), (tr, tc)) = (doubled(shot.from), doubled(shot.to));
    combat::draw_shot(g, kind, row, col, tr, tc, who as i32 + 1);
    if kind & 8 != 0 { let v = g.mem.read(obj.add(2), 1); g.mem.write(obj.add(2), v.saturating_sub(1), 1) }
    let dice = squad::two_d(g);
    let out = rules::resolve(&shot.task, dice);
    let cell = if out.hit { to } else { scatter(g, to) };
    log(g, format!("{name} fires the {} at {}: 2D {}+{} {:+} = {} vs {} {}; a marker on ({}, {})", shot.weapon.name, shot.label, dice.0, dice.1, shot.task.dm, out.total, shot.task.target, verdict(&out), cell.0, cell.1));
    if !out.hit { bump(g, rec, skill_off) }
    let round = current_round(g);
    if let Some(s) = st(g) { s.pending.push(Pending { who: Unit::Traveller(who), cell, weapon: shot.weapon.id, task: shot.task, round, launcher: true }) }
    finish(g, false, false);
}
pub fn at_square(g: &mut Ground, c: Cell) -> bool {
    if !aims_at_squares(g, round::selected(g)) { return false }
    throw(g, c); true
}
pub fn preview_square(g: &Ground, who: u8, c: Cell) -> String {
    match throw_check(g, who, c) { Ok(s) => format!("{}: {}", s.weapon.name, s.describe()), Err(e) => e }
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::rules::Difficulty;
    use crate::ground::orders::t::{fight, ground, p1, place, wall, ITEMS};
    use crate::ground::state::{Far, W_BUILTIN};
    use crate::ui::Target;
    fn pool(g: &mut Ground, k: u8) -> i32 { squad::npc_pool(g, k).unwrap() }
    fn logged(g: &Ground, s: &str) -> usize { p1(g).log.iter().filter(|l| l.contains(s)).count() }
    fn aim(g: &mut Ground, task: &rules::Task, hit: bool) {
        for s in 1..20000u32 { g.rand.lcg.seed = s; let d = squad::two_d(g); if rules::resolve(task, d).hit == hit { g.rand.lcg.seed = s; return } }
        panic!("no seed resolves {task:?} as hit = {hit}")
    }
    fn arm(g: &mut Ground, id: u8, chamber: u8, kind: u8) { g.mem.set_party_u8(0xA82, id); g.mem.set_party_u8(0xA82 + 2, chamber); g.mem.write(Far::new(46 * id as u16 + 0x27, ITEMS), kind as u32, 1) }
    fn rounds_to(g: &mut Ground, round: u16) { for _ in 0..20 { if p1(g).encounter.as_ref().is_some_and(|e| e.round >= round) { return } round::end_turn(g) } panic!("round {round} never came: {:?}", p1(g).log) }

    #[test]
    fn l_turns_the_first_carried_light_on_and_off() {
        let mut g = ground(); let before = g.mem.party_u16(0xF82);
        let none: Vec<i16> = (0..5).flat_map(|w| (0..=22).map(move |s| (w, s))).filter(|&(w, s)| super::super::commands_items::kind(&g, w, s) == 11).map(|(w, _)| w).collect();
        assert!(none.is_empty(), "the fixture carries no light"); assert_eq!(super::super::commands_items::toggle_light(&mut g), "Nobody carries a light");
        arm(&mut g, 52, 0, 0); g.mem.write(Far::new(46 * 52 + 0x1D, ITEMS), 11, 2);
        let line = super::super::commands_items::toggle_light(&mut g);
        assert!(line.starts_with("Light turned on (") && line.ends_with(')'), "{line}"); assert_eq!(g.mem.party_u16(0xF82), before.wrapping_add(1), "the 1990 light count");
        let line = super::super::commands_items::toggle_light(&mut g);
        assert!(line.starts_with("Light turned off ("), "{line}"); assert_eq!(g.mem.party_u16(0xF82), before);
    }
    #[test]
    fn line_of_fire_cover_arc_ring_and_the_occupant_scan() {
        let mut g = ground(); place(&mut g, 14, (16, 5), 0, 14, 41); place(&mut g, 15, (18, 5), 0, 14, 41); place(&mut g, 16, (24, 5), 0, 14, 41);
        let cells = line_cells(&g, (22, 5), (16, 5));
        assert_eq!((&cells[..6], cells.len()), (&[(21, 5), (20, 5), (19, 5), (18, 5), (17, 5), (16, 5)][..], 22), "beyond the target to the map edge");
        assert_eq!(line_candidates(&g, Unit::Traveller(0), (22, 5), (16, 5), false), vec![(Unit::Npc(15), (18, 5)), (Unit::Npc(14), (16, 5))], "closest first, both at medium");
        assert_eq!((occupants(&g, (22, 6)), occupants(&g, (16, 5)), occupants(&g, (30, 30))), (vec![Unit::Traveller(1)], vec![Unit::Npc(14)], vec![]));
        assert_eq!(line_candidates(&g, Unit::Traveller(0), (22, 5), (24, 5), false), vec![(Unit::Npc(16), (24, 5))]);
        assert_eq!(line_candidates(&g, Unit::Traveller(0), (22, 5), (24, 5), true), vec![(Unit::Traveller(2), (23, 5)), (Unit::Npc(16), (24, 5))]);
        assert_eq!(line_warning(&g, 0, 16), "; Trav3 in the line!"); assert_eq!(line_warning(&g, 0, 14), "");
        wall(&mut g, (20, 5)); assert_eq!(line_cells(&g, (22, 5), (16, 5)), vec![(21, 5)], "an obstruction ends the line");
        place(&mut g, 17, (23, 12), 0, 14, 41); wall(&mut g, (22, 11));
        assert!(in_cover(&g, (22, 5), (23, 12)) && !in_cover(&g, (22, 5), (16, 5)) && !in_cover(&g, (22, 5), (21, 11)), "the corner covers (23, 12), not a square beside it");
        let shot = orders::fire_check(&g, 0, 17).unwrap();
        assert!(shot.cover && shot.task.target == 11 && shot.describe().starts_with("Medium: Difficult 11+") && shot.describe().ends_with("cover), needs 8+ (41%)"), "the gyro-stabilised ACR: Routine at medium, +1 cover; {}", shot.describe());
        assert!(in_arc((22, 5), (22, 15), (23, 15)) && in_arc((22, 5), (22, 15), (24, 15)) && in_arc((22, 5), (24, 15), (22, 15)));
        assert!(!in_arc((22, 5), (22, 15), (27, 10)) && !in_arc((22, 5), (22, 15), (22, 0)) && !in_arc((22, 5), (22, 15), (30, 5)));
        assert!(ring(&g, 0, (24, 7)).is_empty());
        arm(&mut g, 25, 40, 0xAC); let r = ring(&g, 0, (24, 7)); assert_eq!(r.len(), 16); assert!(r.iter().all(|&(c, cl)| round::squares(c, (24, 7)) == 2 && cl == PATH_DANGER));
        assert_eq!((rules::danger_radius(45), scatter_is_adjacent(&mut g)), (2, true));
        for n in [(9i16, 9i16), (9, 10), (9, 11), (10, 9), (10, 11), (11, 9), (11, 10), (11, 11)] { wall(&mut g, n) }
        assert_eq!(scatter(&mut g, (10, 10)), (10, 10), "walled in: the grenade stops on its square");
    }
    fn scatter_is_adjacent(g: &mut Ground) -> bool { (0..12).all(|_| { let c = scatter(g, (10, 10)); c != (10, 10) && round::squares(c, (10, 10)) == 1 }) }

    #[test]
    fn a_story_person_is_called_by_name() {
        let mut g = ground(); g.mem.set_view_x(34);
        place(&mut g, 14, (17, 5), 0, 14, 41); place(&mut g, 15, (19, 5), 0, 14, 41); g.mem.write(Mem::npc(14), 221, 2); fight(&mut g);
        assert_eq!((label(&g, Unit::Npc(14)), label(&g, Unit::Npc(15))), ("Viktor".into(), "thug (NPC 15)".into()));
        assert!(orders::preview(&g, Some(Target::Unit(Unit::Npc(14)))).0[0].ends_with("(Viktor)"));
        let shot = orders::fire_check(&g, 0, 14).unwrap(); aim(&mut g, &shot.task, true); orders::fire(&mut g, 14);
        assert_eq!(logged(&g, "Trav1 fires the 9mm advanced combat rifle at Viktor: medium: 2D"), 1, "{:?}", p1(&g).log);
        g.mem.write(Mem::npc(14), 321, 2); assert_eq!(label(&g, Unit::Npc(14)), "Konrad Kiefer");
    }
    #[test]
    fn a_single_shot_rolls_on_the_closer_unit_in_the_line_first() {
        let mut g = ground(); g.mem.set_view_x(34);
        place(&mut g, 14, (17, 5), 0, 14, 41); place(&mut g, 15, (19, 5), 0, 14, 41); fight(&mut g);
        let shot = orders::fire_check(&g, 0, 14).unwrap(); aim(&mut g, &shot.task, true);
        let (p14, p15) = (pool(&mut g, 14), pool(&mut g, 15));
        orders::fire(&mut g, 14);
        assert_eq!(logged(&g, "Trav1 fires the 9mm advanced combat rifle at thug (NPC 14): medium: 2D"), 1, "{:?}", p1(&g).log);
        assert_eq!(logged(&g, "in the line: thug (NPC 15)"), 1, "record 15 stands in the line at the same band: its own roll first; {:?}", p1(&g).log);
        let hit15 = p1(&g).log.iter().any(|l| l.contains("in the line: thug (NPC 15)") && l.contains("hit by"));
        let left15 = g.squad.as_ref().unwrap().pools.get(&15).copied();
        if hit15 { assert!(left15.is_none_or(|p| p < p15)); assert_eq!(pool(&mut g, 14), p14, "one projectile: the first hit stops it") }
        else { assert_eq!(left15, Some(p15)); assert!(g.squad.as_ref().unwrap().pools.get(&14).is_none_or(|&p| p < p14), "the primary's seeded hit lands") }
        assert_eq!(g.mem.party_u8(0xA82 + 2), 19); assert!(p1(&g).turns[0].attacked);
    }

    #[test]
    fn full_auto_spends_the_burst_and_rolls_bonus_attacks() {
        let mut g = ground(); arm(&mut g, 12, 30, 0x78 | 0x20); g.mem.set_party_u8(0x12B + 62, 2);
        place(&mut g, 14, (24, 7), 0, 14, 41); place(&mut g, 15, (24, 8), 0, 14, 41); fight(&mut g);
        let m = orders::menu_for(&g, Target::Unit(Unit::Npc(14)));
        assert_eq!(m.items.iter().find(|i| i.label == "Full auto").unwrap().grey.as_deref(), Some("switch to full auto first (Q)"));
        g.squad.as_mut().unwrap().orders.full_auto[0] = true;
        assert_eq!(orders::menu_for(&g, Target::Unit(Unit::Npc(14))).items.iter().find(|i| i.label == "Full auto").unwrap().grey, None);
        let shot = orders::fire_check(&g, 0, 14).unwrap(); aim(&mut g, &shot.task, false);
        orders::fire(&mut g, 14);
        assert_eq!(logged(&g, "(burst, 10 rounds)"), 1, "{:?}", p1(&g).log);
        assert_eq!(g.mem.party_u8(0xA82 + 2), 20, "10 rounds off the clip (N = 3)");
        assert_eq!(logged(&g, "burst on thug (NPC 15)"), 1, "the adjacent hostile takes a bonus attack; {:?}", p1(&g).log);
        assert!(logged(&g, "burst on thug (NPC 14)") <= 2 && logged(&g, "burst on") == 3 || !round::npc_present(&g, 14), "3 bonus attacks: 15, then the primary takes the rest (nobody else in the line); {:?}", p1(&g).log);
        assert!(p1(&g).turns[0].attacked);
        g.mem.set_party_u8(0xA82 + 2, 1); assert_eq!(burst_check(&g, 0).err().as_deref(), Some("a burst needs two rounds or more"));
    }

    #[test]
    fn rapid_fire_picks_three_targets_in_the_arc_empties_the_clip_and_reloads_next_round() {
        let mut g = ground(); arm(&mut g, 12, 30, 0x78 | 0x20); g.mem.set_view_y(8);
        place(&mut g, 14, (24, 15), 0, 14, 41); place(&mut g, 15, (25, 15), 0, 14, 41); place(&mut g, 16, (23, 15), 0, 14, 41); place(&mut g, 17, (30, 5), 0, 14, 41); fight(&mut g);
        assert_eq!(orders::menu_for(&g, Target::Unit(Unit::Npc(14))).items.iter().find(|i| i.label == "Rapid fire").unwrap().grey, None);
        shoot(&mut g, 14, Opts { rapid: true, ..Opts::default() });
        assert_eq!(g.squad.as_ref().unwrap().fire.rapid, Some(vec![14])); assert!(g.squad.as_ref().unwrap().orders.hover.starts_with("Rapid fire at thug"));
        assert!(rapid_add(&mut g, 17)); assert_eq!(g.squad.as_ref().unwrap().orders.hover, "outside the 45 degree arc");
        assert!(rapid_add(&mut g, 15)); assert_eq!(g.squad.as_ref().unwrap().fire.rapid.as_ref().map(|l| l.len()), Some(2));
        orders::intent(&mut g, crate::ui::Intent::Verb { verb: crate::ui::Verb::Fire, target: Target::Unit(Unit::Npc(16)) });
        assert!(g.squad.as_ref().unwrap().fire.rapid.is_none());
        assert_eq!(logged(&g, "rapid-fires the 9mm submachinegun at thug"), 3, "{:?}", p1(&g).log);
        assert!(p1(&g).log.iter().filter(|l| l.contains("rapid-fires")).all(|l| l.contains("vs 15 ") || l.contains("vs 19 ")), "+1 difficulty: Handgun medium 11 -> 15 (unskilled 19); {:?}", p1(&g).log);
        assert_eq!((g.mem.party_u8(0xA82 + 2), g.squad.as_ref().unwrap().fire.reloading[0]), (0, true), "the clip emptied, next round reloading");
        assert!(p1(&g).turns[0].attacked);
        rounds_to(&mut g, 2);
        assert!(p1(&g).turns[0].attacked && !p1(&g).turns[0].done, "round 2: the Attack is the reload");
        assert_eq!((g.mem.party_u8(0xA82 + 1), g.mem.party_u8(0xA82 + 2), g.squad.as_ref().unwrap().fire.reloading[0]), (2, 30, false));
        assert_eq!(logged(&g, "Trav1 is reloading after rapid fire (30 rounds, 2 magazines left)"), 1, "{:?}", p1(&g).log);
        rounds_to(&mut g, 3); orders::select(&mut g, 0); shoot(&mut g, 17, Opts { rapid: true, ..Opts::default() }); assert!(orders::key(&mut g, 0x1C)); assert_eq!(logged(&g, "rapid-fires"), 4, "{:?}", p1(&g).log);
    }

    #[test]
    fn melee_is_the_errata_confrontation_ends_the_turn_and_may_disarm() {
        let mut g = ground(); arm(&mut g, 26, 0, 0); place(&mut g, 14, (22, 4), 0, 14, 41); place(&mut g, 15, (26, 4), 0, 14, 41); fight(&mut g);
        assert_eq!(melee_check(&g, 0, 15).err().as_deref(), Some("not adjacent"));
        let shot = melee_check(&g, 0, 14).unwrap();
        assert_eq!(shot.describe(), "Close: Routine 7+, DM -1 (skill 0, Str +0, vs skill 0 block 1, unskilled, defender unskilled), needs 8+ (41%)", "an unskilled dagger against a gunman blocking with his hands");
        let m = orders::menu_for(&g, Target::Unit(Unit::Npc(14)));
        assert_eq!((m.items.iter().find(|i| i.label == "Melee").unwrap().grey.as_deref(), m.items.iter().find(|i| i.label == "Disarm").unwrap().grey.as_deref(), m.items.iter().find(|i| i.label == "Pinpoint").unwrap().grey.as_deref()), (None, None, Some("no pinpoint with this weapon")));
        g.squad.as_mut().unwrap().p1.turns[0].speed = 2; assert_eq!(melee_check(&g, 0, 14).err().as_deref(), Some("ran: no melee this turn")); g.squad.as_mut().unwrap().p1.turns[0].speed = 0;
        aim(&mut g, &shot.task, true); let p = pool(&mut g, 14);
        melee(&mut g, 14, true);
        assert_eq!(logged(&g, "Trav1 attacks thug (NPC 14) with the dagger: close: 2D"), 1, "{:?}", p1(&g).log);
        let q = pool(&mut g, 14);
        assert_eq!((g.squad.as_ref().unwrap().fire.disarmed.as_slice(), q, p1(&g).turns[0].done), (&[14][..], p, true), "{:?}", p1(&g).log);
        assert_eq!(logged(&g, "thug (NPC 14) is disarmed for the round"), 1);
        rounds_to(&mut g, 2); assert!(g.squad.as_ref().unwrap().fire.disarmed.is_empty());
        round::select(&mut g, 0); let shot = melee_check(&g, 0, 14).unwrap(); aim(&mut g, &shot.task, true);
        orders::fire(&mut g, 14);
        assert!(g.squad.as_ref().unwrap().pools.get(&14).is_none_or(|&q| q < p), "the dagger's Pen 2 / Dmg 2 against the melee armour column; {:?}", p1(&g).log);
        assert!(p1(&g).turns[0].done);
    }

    #[test]
    fn grenade_range_toggle_shifts_only_three_through_five_squares() {
        let mut g = ground(); g.mem.write(Mem::walker(0).add(W_BUILTIN), 1, 1); g.mem.set_party_u8(0x1CB, 2);
        g.mem.set_party_u8(0x116, 8);
        for enabled in [true, false] {
            g.balance.grenade_range = enabled;
            for distance in [1, 2, 3, 5, 6, 29] {
                let shot = throw_check(&g, 0, (22, 5 + distance)).unwrap();
                let expected = if distance <= 2 || (enabled && distance <= 5) { Difficulty::Difficult } else { Difficulty::Formidable };
                assert_eq!((shot.task.difficulty, shot.task.target, shot.task.dm), (expected, expected.target(), 2), "toggle={enabled}, distance={distance}");
                assert_eq!(shot.task.chance_pct(), if expected == Difficulty::Difficult { 27 } else { 0 });
            }
            assert_eq!(throw_check(&g, 0, (22, 35)).err().as_deref(), Some("out of range"), "the maximum stays Medium");
        }
    }

    #[test]
    fn a_grenade_is_thrown_at_a_square_and_goes_off_at_the_end_of_the_turn() {
        let mut g = ground(); g.mem.write(Mem::walker(0).add(W_BUILTIN), 1, 1); g.mem.set_party_u8(0x1CB, 2);
        place(&mut g, 14, (24, 7), 0, 14, 41); place(&mut g, 15, (24, 8), 0, 14, 41); fight(&mut g);
        assert!(aims_at_squares(&g, 0));
        assert_eq!(throw_check(&g, 0, (24, 8)).unwrap().describe(), "Short: Difficult 11+, DM +1 (Str +0, Dex +1), needs 10+ (16%), goes off at the end of the turn", "the 3-square throw uses the Short row");
        let shot = throw_check(&g, 0, (23, 7)).unwrap();
        assert_eq!(shot.describe(), "Short: Difficult 11+, DM +1 (Str +0, Dex +1), needs 10+ (16%), goes off at the end of the turn");
        assert_eq!(throw_check(&g, 0, (22, 5)).err().as_deref(), Some("not at your own square")); assert_eq!(throw_check(&g, 0, (0, 59)).err().as_deref(), Some("out of range"));
        let m = orders::menu_for(&g, Target::Cell((23, 7))); assert_eq!(m.items[m.default].label, "Throw grenade here"); assert_eq!(m.items[m.default].grey, None);
        orders::hover(&mut g, Some(Target::Cell((23, 7)))); assert_eq!(g.squad.as_ref().unwrap().orders.path.len(), 8, "the danger ring: the 8 neighbours");
        aim(&mut g, &shot.task, true);
        orders::intent(&mut g, crate::ui::Intent::Verb { verb: crate::ui::Verb::Move, target: Target::Cell((23, 7)) });
        assert_eq!((g.mem.party_u8(0x1CB), g.mem.read(Mem::walker(0).add(W_BUILTIN), 1), g.squad.as_ref().unwrap().fire.pending.len()), (1, 0, 1));
        assert_eq!(g.squad.as_ref().unwrap().fire.pending[0].cell, (23, 7));
        assert_eq!((logged(&g, "Trav1 throws an explosive grenade at (23, 7): short: 2D"), logged(&g, "goes off")), (1, 0), "{:?}", p1(&g).log);
        assert!(p1(&g).turns[0].attacked && !p1(&g).turns[0].done);
        round::end_turn(&mut g);
        assert_eq!(logged(&g, "Trav1's explosive grenade goes off at (23, 7)"), 1, "{:?}", p1(&g).log);
        assert_eq!((logged(&g, "blast (1 sq, Pen 4) on thug (NPC 14)"), logged(&g, "blast (1 sq, Pen 4) on thug (NPC 15)"), logged(&g, "blast (1 sq, Pen 4) on Trav2"), logged(&g, "blast (1 sq, Pen 4) on Trav4"), logged(&g, "blast (1 sq, Pen 4) on Trav5")), (1, 1, 1, 1, 1), "radius 1 at half Pen, the party in its own blast; {:?}", p1(&g).log);
        assert!(g.squad.as_ref().unwrap().fire.pending.is_empty());
        let mut g = ground(); g.mem.write(Mem::walker(0).add(W_BUILTIN), 2, 1); g.mem.set_party_u8(0x1CC, 1); place(&mut g, 14, (28, 20), 0, 14, 41);
        assert!(at_square(&mut g, (26, 12))); assert_eq!(logged(&g, "Trav1's chemical grenade goes off"), 1, "{:?}", p1(&g).log);
        assert!(!at_square(&mut g, (26, 12)));
    }

    #[test]
    fn selecting_the_traveller_keeps_the_chosen_grenade() {
        let mut g = ground(); g.mem.write(Mem::walker(0).add(W_BUILTIN), 1, 1); g.mem.set_party_u8(0x1CB, 2);
        place(&mut g, 14, (24, 7), 0, 14, 41); fight(&mut g);
        orders::select(&mut g, 0); assert!(aims_at_squares(&g, 0), "re-selected");
        orders::select(&mut g, 1); orders::select(&mut g, 0); assert!(aims_at_squares(&g, 0), "away and back");
        let m = orders::menu_for(&g, Target::Cell((22, 7))); assert_eq!(m.items[m.default].label, "Throw grenade here");
    }
    #[test]
    fn a_launcher_fires_outdoors_leaves_a_marker_and_lands_at_the_next_turn() {
        let mut g = ground(); arm(&mut g, 17, 3, 0x69); g.mem.set_party_u8(0x12B + 22, 1);
        g.mem.set_view_x(40);
        place(&mut g, 14, (26, 10), 0, 14, 41); fight(&mut g);
        let shot = launch_check(&g, 0, (26, 10)).unwrap();
        assert!(shot.describe().ends_with("Difficult 11+, DM +1 (Forward Observer 1), needs 10+ (16%), lands at the start of your next turn"), "{}", shot.describe());
        assert_eq!(orders::fire_check(&g, 0, 14).map(|s| s.task), Ok(shot.task), "Fire on the NPC = the launcher at its square");
        g.mem.set_interior_kind(b'I'); assert_eq!(launch_check(&g, 0, (26, 10)).err().as_deref(), Some("not indoors")); g.mem.set_interior_kind(b'G');
        let m = orders::menu_for(&g, Target::Cell((26, 10))); assert!(m.items.iter().any(|i| i.label == "Fire launcher here" && i.grey.is_none()));
        assert_eq!(ring(&g, 0, (26, 10)).len(), 72, "the radius-9 ring");
        aim(&mut g, &shot.task, true);
        orders::fire(&mut g, 14);
        assert_eq!(g.mem.party_u8(0xA82 + 2), 2, "one round off the launcher");
        assert_eq!(logged(&g, "Trav1 fires the 4cm RAM GL-10 at (26, 10): 2D"), 1, "{:?}", p1(&g).log);
        assert_eq!(crate::ui::scene::Squad::capture(&g).markers, vec![(26, 10)], "the marker rides the Scene");
        assert!(p1(&g).turns[0].attacked && logged(&g, "lands") == 0);
        rounds_to(&mut g, 2); orders::select(&mut g, 0);
        assert_eq!(logged(&g, "Trav1's 4cm RAM GL-10 lands at (26, 10)"), 1, "{:?}", p1(&g).log);
        assert_eq!(logged(&g, "blast (0 sq, Pen 9) on thug (NPC 14)"), 1, "{:?}", p1(&g).log);
        assert!(g.squad.as_ref().unwrap().fire.pending.is_empty() && crate::ui::scene::Squad::capture(&g).markers.is_empty());
    }
}
