use super::{combat, fire, render, round::{self, Side, MOVE_RUN, MOVE_WALK}, rules::{self, adapters::{self, Fighter, Source}, compendium::Morale, weapons::{self, Profile}, Band, Difficulty, Task}, squad, state::{Ground, Mem, N_X, N_Y}, tactics, walk, commands};
use crate::ui::Unit;
use std::collections::{BTreeMap, VecDeque};

pub const WANDER_HEARTBEATS: usize = 6;
const N_AMMO: i32 = 0x1E; const ITEM_KIND: i32 = 0x27;
#[cfg(test)] const N_TOKEN: i32 = 0x16;
const NPC_SOUND: i32 = 11; const NPC_LASER: i32 = 9;

fn p1(g: &mut Ground) -> &mut round::State { &mut g.squad.as_mut().expect("squad mode").p1 }
fn log(g: &mut Ground, line: String) { p1(g).log(line) }
fn npc_byte(g: &Ground, k: u8, off: i32) -> u8 { g.mem.read(Mem::npc(k as i16).add(off), 1) as u8 }
fn npc_st(g: &mut Ground, k: u8) -> &mut round::NpcSt { p1(g).npcs.entry(k).or_default() }
fn alerted(g: &Ground, k: u8) -> bool { g.squad.as_ref().is_some_and(|s| s.p1.npcs.get(&k).is_some_and(|n| n.alerted.is_some())) }
fn label(f: &Fighter, k: u8) -> String { round::tagged(&f.label, k) }

pub fn turn(g: &mut Ground, k: u8) {
    let f = round::npc_fighter(g, k);
    let who = label(&f, k);
    { let s = p1(g); s.acting = Some(Unit::Npc(k)); let n = s.npcs.entry(k).or_default(); n.acted = true; n.speed = 0; }
    fire::turn_start(g, Unit::Npc(k));
    if !round::npc_present(g, k) { return }
    let Some(e) = p1(g).encounter.clone() else { return };
    if !round::willing(g, k) || e.surprised == Some(Side::Npcs) {
        let moved = wander(g, k, WANDER_HEARTBEATS);
        log(g, format!("{who} {}{}", if e.surprised == Some(Side::Npcs) { "is surprised" } else { "minds its own business" }, if moved > 0 { format!(" (wanders {moved})") } else { String::new() }));
        return
    }
    npc_st(g, k).engaged = true;
    morale(g);
    let (withdrawing, routed) = { let s = p1(g); (s.encounter.as_ref().is_some_and(|e| e.withdrawing), s.npcs.get(&k).is_some_and(|n| n.routed)) };
    if routed { continue_turn(g, k, Plan::Flee { left: MOVE_RUN as usize, attack: false }); return }
    if npc_st(g, k).reload { let n = npc_st(g, k); n.reload = false; n.clip = 0; log(g, format!("{who} reloads")); return }
    let Some(w) = weapon(g, k) else { log(g, format!("{who} is unarmed")); return };
    if withdrawing { continue_turn(g, k, Plan::Flee { left: MOVE_WALK as usize, attack: true }); return }
    if in_reach(g, k, w).is_none() { continue_turn(g, k, Plan::Approach { left: MOVE_WALK as usize }) } else { continue_turn(g, k, Plan::Attack { checked: false }) }
}
fn weapon(g: &Ground, k: u8) -> Option<&'static rules::Weapon> {
    if g.squad.as_ref().is_some_and(|s| s.fire.disarmed.contains(&k)) { Some(&weapons::HANDS) } else { adapters::npc_weapon(g, k) }
}
fn in_reach(g: &Ground, k: u8, w: &rules::Weapon) -> Option<u8> { round::sees(g, k).filter(|&t| reach(g, k, t, w)) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan { Flee { left: usize, attack: bool }, Approach { left: usize }, Attack { checked: bool } }
fn pause(g: &mut Ground, k: u8, plan: Plan) { if let Some(s) = g.squad.as_mut() { s.p22.paused = Some((k, plan)) } }
pub fn resume(g: &mut Ground, k: u8) {
    let Some((pk, plan)) = g.squad.as_mut().and_then(|s| s.p22.paused.take()) else { return };
    if pk != k || !round::in_encounter(g) || !round::npc_present(g, k) { return }
    p1(g).acting = Some(Unit::Npc(k));
    continue_turn(g, k, plan)
}
fn continue_turn(g: &mut Ground, k: u8, plan: Plan) {
    let f = round::npc_fighter(g, k); let who = label(&f, k);
    let Some(w) = weapon(g, k) else { return };
    let plan = match plan {
        Plan::Flee { left, attack } => {
            let (moved, cut) = flee(g, k, left);
            if moved > 0 && attack { npc_st(g, k).speed = 1 }
            log(g, format!("{who} {}{}", if attack { "falls back" } else { "runs" }, if moved > 0 { format!(" ({moved} squares)") } else if attack { String::new() } else { ": cornered".into() }));
            if cut { pause(g, k, Plan::Flee { left: left - moved, attack }); return }
            if !attack || in_reach(g, k, w).is_none() { return }
            Plan::Attack { checked: false }
        }
        Plan::Approach { left } => {
            let (moved, cut) = approach_n(g, k, left, Some(w.max));
            if cut { pause(g, k, Plan::Approach { left: left - moved }); return }
            Plan::Attack { checked: false }
        }
        p => p,
    };
    let Plan::Attack { checked } = plan else { return };
    match in_reach(g, k, w) {
        Some(t) => { if !checked && tactics::interrupt(g, k) { pause(g, k, Plan::Attack { checked: true }); return } attack(g, k, t, w, &f); }
        None if w.profile == Profile::Melee => log(g, format!("{who} closes in to strike")),
        None => log(g, format!("{who} closes in")),
    }
}
fn reach(g: &Ground, k: u8, t: u8, w: &rules::Weapon) -> bool {
    let d = round::squares(round::npc_cell(g, k), round::traveller_cell(g, t));
    if w.profile == Profile::Melee { return d <= 1 }
    rules::band(d) <= w.max
}
fn attack(g: &mut Ground, k: u8, t: u8, w: &'static rules::Weapon, f: &Fighter) -> bool {
    let who = label(f, k);
    let (from, to) = (round::npc_cell(g, k), round::traveller_cell(g, t));
    let distance = round::squares(from, to);
    let band = rules::band(distance);
    let hands = w.id == weapons::HANDS.id;
    let skill = if hands { adapters::Skill { level: 0, unskilled: true } } else { f.skill(g, &g.squad.as_ref().unwrap().compendium, w) };
    let n = Mem::npc(k as i16);
    let kind = if w.profile == Profile::Melee { 0 } else { g.mem.read(g.mem.pointer(0x6D6D).add(w.id as i32 * 0x2E).add(ITEM_KIND), 1) as i32 };
    if kind != 0 && npc_byte(g, k, N_AMMO) == 0 { log(g, format!("{who} has no ammunition left")); return false }
    if w.profile == Profile::Indirect && g.mem.interior_kind() == b'I' { log(g, format!("{who} holds fire: no room for the {} indoors", w.name)); return false }
    let tt = g.squad.as_ref().unwrap().p1.turns[(t as usize).min(4)];
    let own = g.squad.as_ref().unwrap().p1.npcs.get(&k).map_or(0, |n| n.speed as i32);
    let movement = rules::movement_dm(tt.speed as i32, tt.evade, own, false, band);
    let (task, full) = match w.profile {
        Profile::Melee => { let (dskill, block) = fire::defence(g, Unit::Traveller(t)); (fire::melee_task(skill, rules::char_dm(f.strength), dskill, block), false) }
        Profile::Indirect => { let d = Difficulty::Difficult; (Task { difficulty: d, target: d.target(), dm: skill.level.clamp(-rules::DM_CAP, rules::DM_CAP) }, false) }
        _ => {
            let fr = rules::Fire { profile: w.profile, band, braced: w.gyro, skill: skill.level, unskilled: skill.unskilled, dex: f.dex, movement, ..Default::default() };
            let Some(task) = rules::hit_task(&fr) else { log(g, format!("{who} holds fire: the {} cannot reach", w.name)); return false };
            (task, g.balance.npc_bursts(w.auto, npc_byte(g, k, N_AMMO), distance, boxed(g, t)))
        }
    };
    let rounds = if full { (rules::rounds_per_burst(w.auto) as u32).min(npc_byte(g, k, N_AMMO) as u32) } else { 1 };
    if full && g.balance.npc_selective_autofire && distance > 5 { log(g, format!("{who} chooses a burst: {} is boxed (no free orthogonal neighbour)", round::name(g, t))); }
    let dice = squad::two_d(g);
    let out = rules::resolve(&task, dice);
    let (row, col, tr, tc) = (from.0 as i32 * 2, from.1 as i32 * 2, to.0 as i32 * 2, to.1 as i32 * 2);
    combat::draw_shot_with(g, kind, row, col, tr, tc, NPC_SOUND, NPC_LASER);
    if kind != 0 { let a = npc_byte(g, k, N_AMMO) as u32; g.mem.write(n.add(N_AMMO), a.saturating_sub(rounds), 1); npc_st(g, k).clip += rounds as u16 }
    let skill_text = match f.source { Source::Derived | Source::Invulnerable => format!("skill {} (derived)", skill.level), _ => format!("skill {}{}", skill.level, if skill.unskilled { " unskilled" } else { "" }) };
    let name = round::name(g, t);
    let roll = format!("{skill_text}, 2D {}+{} {:+} = {} vs {} {}", dice.0, dice.1, task.dm, out.total, task.target, fire::verdict(&out));
    let mut hits: Vec<Unit> = Vec::new();
    match w.profile {
        Profile::Melee => {
            log(g, format!("{who} attacks {name} with the {}: close: {roll}", w.name));
            if let (true, Some(level)) = (out.hit, out.level) {
                let tf = round::traveller_fighter(g, t); let av = tf.armour.vs(0x00);
                let (pen, d) = adapters::npc_weapon_values(f, w);
                let dmg = rules::damage(pen, av, tf.armour.exposed, level, d);
                fire::apply(g, Unit::Traveller(t), dmg, pen, av); hits.push(Unit::Traveller(t));
            }
        }
        Profile::Indirect => {
            let cell = if out.hit { to } else { fire::scatter(g, to) };
            log(g, format!("{who} fires the {} at ({}, {}): {roll}; a marker on ({}, {})", w.name, to.0, to.1, cell.0, cell.1));
            let round = p1(g).encounter.as_ref().map_or(0, |e| e.round);
            g.squad.as_mut().unwrap().fire.pending.push(fire::Pending { who: Unit::Npc(k), cell, weapon: w.id, task, round, launcher: true });
        }
        _ => {
            log(g, format!("{who} fires the {} at {name}: {}: {roll}{}", w.name, band.name(), if full { format!(" (burst, {rounds} rounds)") } else { String::new() }));
            let grouped = p1(g).encounter.as_ref().is_some_and(|e| e.grouped);
            let victim = if grouped { random_member(g).unwrap_or(t) } else { t };
            let tf = round::traveller_fighter(g, victim);
            let (av, pen, primary) = (tf.armour.vs(w.class), rules::pen_at(w.pen as i32, w.atten, band), Unit::Traveller(victim));
            let (mut attacked, mut got, landed) = fire::line_fire(g, Unit::Npc(k), primary, from, to, w, &task, out, pen, false, av, tf.armour.exposed);
            if full { for u in fire::burst_targets(g, Unit::Npc(k), primary, from, to, w.auto as usize, &attacked) { if fire::roll_on(g, u, w, &task, pen, false, "burst on").hit { got.push(u) } attacked.push(u) } }
            if rules::danger_radius(w.danger_dm) > 0 { attacked.push(Unit::Npc(k)); got.extend(fire::blast(g, landed, w, &task, pen, &attacked)) }
            hits = got;
        }
    }
    let standing = hits.iter().any(|&u| matches!(u, Unit::Traveller(i) if round::conscious(g, i)));
    if hits.iter().any(|&u| matches!(u, Unit::Traveller(i) if round::alive(g, i))) { g.mem.set_extra_sound(6) }
    else if hits.is_empty() && w.profile != Profile::Indirect { render::front(g); combat::flash_marker(g, -1, tr, tc); render::back(g) }
    if !matches!(kind, 0 | 0xAA | 0xAB) || standing { if let Some(e) = p1(g).encounter.as_mut() { if e.surprised.take().is_some() { log(g, "The alarm is raised: surprise lost".into()) } } }
    if g.mem.npc_attacking() == 0 { walk::play_pending_sounds(g) }
    combat::npc_reaction(g, n, (npc_byte(g, k, 13) & 3) as i32);
    if kind != 0 && w.rds > 0 && npc_st(g, k).clip >= w.rds { npc_st(g, k).reload = true }
    g.mem.set_redraw_pending(1);
    true
}
fn random_member(g: &mut Ground) -> Option<u8> {
    for _ in 0..64 { let t = walk::random(g, 5) as u8; if round::alive(g, t) { return Some(t) } }
    (0..5).find(|&t| round::alive(g, t))
}
fn boxed(g: &Ground, t: u8) -> bool {
    let (r, c) = round::traveller_cell(g, t);
    [(r - 1, c), (r, c + 1), (r + 1, c), (r, c - 1)].into_iter().all(|cell| !super::orders::passable(g, t, cell))
}

fn morale_rank(m: Morale) -> u8 { match m { Morale::VeryLow => 0, Morale::Low => 1, Morale::Average => 2, Morale::High => 3 } }
fn entry_morale(g: &Ground, k: u8) -> Option<Morale> {
    match round::npc_fighter(g, k).source { Source::Entry(id) => g.squad.as_ref()?.compendium.roles.get(&id).map(|e| e.morale), _ => None }
}
fn roster(g: &Ground) -> (Vec<u8>, Vec<u8>) {
    let Some(s) = g.squad.as_ref() else { return (Vec::new(), Vec::new()) };
    let Some(e) = &s.p1.encounter else { return (Vec::new(), Vec::new()) };
    let roster = s.p1.active_def().participants.unwrap_or_else(|| e.npc_start.clone());
    let lost = roster.iter().copied().filter(|&k| !round::npc_present(g, k) || npc_byte(g, k, 2) == 20).collect();
    (roster, lost)
}
fn morale(g: &mut Ground) {
    let Some(e) = p1(g).encounter.clone() else { return };
    if e.withdrawing || e.morale_round == e.round { return }
    let (roster, lost) = roster(g);
    if roster.is_empty() { return }
    let pct = lost.len() as i32 * 100 / roster.len() as i32;
    if pct < 25 { return }
    let standing: Vec<u8> = roster.iter().copied().filter(|k| !lost.contains(k)).collect();
    let leader = standing.iter().map(|&k| round::npc_fighter(g, k).leader).max().unwrap_or(0);
    let level = standing.iter().filter_map(|&k| entry_morale(g, k)).min_by_key(|&m| morale_rank(m)).unwrap_or_default();
    let base = match level { Morale::VeryLow => Difficulty::Formidable, Morale::Low => Difficulty::Difficult, Morale::Average => Difficulty::Routine, Morale::High => Difficulty::Simple };
    let d = base.harder((pct - 25) / 25);
    let task = Task { difficulty: d, target: d.target(), dm: leader.clamp(-rules::DM_CAP, rules::DM_CAP) };
    let dice = squad::two_d(g);
    let out = rules::resolve(&task, dice);
    let word = if out.hit { "they hold" } else if out.exceptional_failure { "ROUT" } else { "they fall back" };
    let line = format!("Morale ({} of {} down, {:?}): {} {}+, Leader {leader}: 2D {}+{} = {} vs {}: {word}", lost.len(), roster.len(), level, d.name(), d.target(), dice.0, dice.1, out.total, task.target);
    let side = round::npc_side(g);
    let s = p1(g);
    s.log(line);
    if let Some(e) = s.encounter.as_mut() {
        e.morale_round = e.round;
        if !out.hit { e.withdrawing = true }
    }
    if !out.hit && out.exceptional_failure { for k in side { s.npcs.entry(k).or_default().routed = true } }
}

fn in_map(g: &Ground, (r, c): (i16, i16)) -> bool { r >= 0 && c >= 0 && r < g.mem.map_blocks_high() as i16 * 12 && c < g.mem.map_stride() }
pub fn can_step(g: &Ground, k: u8, (r, c): (i16, i16), lift_leash: bool) -> bool {
    if !in_map(g, (r, c)) { return false }
    let n = Mem::npc(k as i16);
    let t = npc_byte(g, k, 7) as i32; let (mode, leash) = (t & 7, t >> 3);
    if mode == 4 { return false }
    if leash != 0 && !lift_leash {
        let home = ((g.mem.read_i(n.add(3), 2) >> 1) as i16, (g.mem.read_i(n.add(5), 2) >> 1) as i16);
        if (r - home.0).abs() as i32 >= leash || (c - home.1).abs() as i32 >= leash { return false }
    }
    if (g.mem.x() >> 1, g.mem.y() >> 1) == (r, c) && g.mem.encounter_mode() != 2 && g.mem.individual() == 0 { return false }
    let cell = commands::cell(g, r, c);
    let (flags, terrain, occ) = (g.mem.read(cell, 1) as i32, g.mem.read(cell.add(1), 1) as i32, g.mem.read(cell.add(2), 1) as i32);
    if occ & 0x7F != 0 { return false }
    if g.mem.combat() != 0 && (0..5).any(|i| { let w = Mem::walker(i); g.mem.read(w, 1) != 0 && (g.mem.read_i(w.add(8), 2) as i16, g.mem.read_i(w.add(10), 2) as i16) == (r * 2, c * 2) }) { return false }
    if flags & 1 == 0 && matches!(terrain, 38 | 39) { return false }
    if mode & 3 != 0 {
        let water = flags & 1 != 0 && (0x41..=0x5E).contains(&terrain);
        let land = flags & 1 == 0 || (0x28..=0x40).contains(&terrain);
        if (mode == 1 && !water) || (mode == 2 && !land) || !(water || land) { return false }
    }
    if flags & 1 != 0 { return terrain >= 5 && terrain / 20 < 5 }
    if matches!(terrain, 15 | 16) { return false }
    terrain / 20 == 1
}
fn place(g: &mut Ground, k: u8, to: (i16, i16)) {
    let n = Mem::npc(k as i16);
    let from = round::npc_cell(g, k);
    walk::set_cell_flags(g, from.0 * 2, from.1 * 2, 0);
    g.mem.write(n.add(N_X), (to.0 * 2) as u16 as u32, 2); g.mem.write(n.add(N_Y), (to.1 * 2) as u16 as u32, 2);
    walk::set_cell_flags(g, to.0 * 2, to.1 * 2, k as i16);
    if let Some(f) = round::fx(g) { if f.unit == Some(Unit::Npc(k)) && f.walk.len() < 64 { f.walk.push(to) } }
    g.mem.set_event_id(-1); g.mem.set_redraw_pending(1);
}
fn path_to(g: &Ground, k: u8, from: (i16, i16), goal: (i16, i16), lift_leash: bool) -> Option<Vec<(i16, i16)>> {
    if round::squares(from, goal) <= 1 { return Some(Vec::new()) }
    let mut prev: BTreeMap<(i16, i16), (i16, i16)> = BTreeMap::new();
    let mut queue = VecDeque::from([(from, 0usize)]);
    let near = |c: (i16, i16)| { let (dr, dc) = ((c.0 - goal.0) as i32, (c.1 - goal.1) as i32); (dr.abs().max(dc.abs()), dr * dr + dc * dc) };
    while let Some((c, d)) = queue.pop_front() {
        if d >= 62 { continue }
        let mut next: Vec<(i16, i16)> = (-1i16..=1).flat_map(|dr| (-1i16..=1).map(move |dc| (c.0 + dr, c.1 + dc))).filter(|&n| n != c).collect();
        next.sort_by_key(|&n| near(n));
        for n in next {
            if n == from || prev.contains_key(&n) || !can_step(g, k, n, lift_leash) { continue }
            prev.insert(n, c);
            if round::squares(n, goal) <= 1 {
                let mut path = vec![n]; let mut at = n;
                while let Some(&p) = prev.get(&at) { if p == from { break } path.push(p); at = p }
                path.reverse(); return Some(path)
            }
            queue.push_back((n, d + 1));
        }
    }
    None
}
fn greedy(g: &Ground, k: u8, goal: (i16, i16), toward: bool, lift_leash: bool) -> Option<(i16, i16)> {
    let from = round::npc_cell(g, k);
    let here = round::squares(from, goal);
    let mut best: Option<(i32, (i16, i16))> = None;
    for dr in -1i16..=1 { for dc in -1i16..=1 {
        if (dr, dc) == (0, 0) { continue }
        let c = (from.0 + dr, from.1 + dc);
        let d = round::squares(c, goal);
        let better = if toward { d < here } else { d > here };
        if better && best.is_none_or(|(bd, _)| if toward { d < bd } else { d > bd }) && can_step(g, k, c, lift_leash) { best = Some((d, c)) }
    } }
    best.map(|(_, c)| c)
}
pub fn approach(g: &mut Ground, k: u8, max: usize, stop_band: Option<Band>) -> usize { approach_n(g, k, max, stop_band).0 }
fn approach_n(g: &mut Ground, k: u8, max: usize, stop_band: Option<Band>) -> (usize, bool) {
    let lift = alerted(g, k) || round::routed(g, k);
    let encounter = round::in_encounter(g);
    let mut moved = 0;
    for _ in 0..max {
        let from = round::npc_cell(g, k);
        let last_seen = g.squad.as_ref().and_then(|s| s.p1.npcs.get(&k)).and_then(|n| n.last_seen);
        let nearest = round::nearest_traveller(g, from).map(|(t, _)| round::traveller_cell(g, t));
        let goal = match (encounter, last_seen) { (false, Some(ls)) if ls != from => ls, _ => match nearest { Some(c) => c, None => break } };
        if round::squares(from, goal) <= 1 { if !encounter && Some(goal) == last_seen && Some(goal) != nearest { npc_st(g, k).last_seen = None; continue } break }
        if moved > 0 || !encounter { if let Some(t) = if encounter { round::sees(g, k) } else { round::spots(g, k) } { if stop_band.is_none_or(|b| rules::band(round::squares(from, round::traveller_cell(g, t))) <= b) { break } } }
        let next = match path_to(g, k, from, goal, lift) { Some(p) if !p.is_empty() => p[0], _ => match greedy(g, k, goal, true, lift) { Some(c) => c, None => break } };
        place(g, k, next);
        moved += 1;
        if encounter { npc_st(g, k).speed = 1; if let Some(c) = nearest { npc_st(g, k).last_seen = Some(c) } if tactics::interrupt(g, k) { return (moved, true) } }
    }
    (moved, false)
}
fn flee(g: &mut Ground, k: u8, max: usize) -> (usize, bool) {
    let mut moved = 0;
    for _ in 0..max {
        let Some((t, _)) = round::nearest_traveller(g, round::npc_cell(g, k)) else { break };
        let goal = round::traveller_cell(g, t);
        let Some(c) = greedy(g, k, goal, false, true) else { break };
        place(g, k, c); moved += 1;
        if tactics::interrupt(g, k) { return (moved, true) }
    }
    (moved, false)
}
pub fn wander(g: &mut Ground, k: u8, beats: usize) -> usize {
    let p = Mem::npc(k as i16);
    let b = |g: &Ground, o: i32| g.mem.read(p.add(o), 1) as i32;
    let sb = |g: &Ground, o: i32| g.mem.read_i(p.add(o), 1);
    let w = |g: &Ground, o: i32| g.mem.read_i(p.add(o), 2);
    fn wb(g: &mut Ground, p: super::state::Far, o: i32, v: i32) { g.mem.write(p.add(o), v as u32 & 0xFF, 1) }
    fn ww(g: &mut Ground, p: super::state::Far, o: i32, v: i32) { g.mem.write(p.add(o), v as u16 as u32, 2) }
    let start = round::npc_cell(g, k);
    let mut beat = 0;
    while beat < beats || (w(g, 32) & 1 != 0 || w(g, 34) & 1 != 0) && beat < beats + 2 {
        beat += 1;
        if b(g, 2) != g.mem.level() as u8 as i32 { break }
        if b(g, 7) == 4 {
            if b(g, 37) == 0 {
                let f = (b(g, 36) + sb(g, 39)) & 7; wb(g, p, 36, f);
                let n = walk::random(g, 20) as i32; wb(g, p, 37, n + 20); wb(g, p, 39, 1);
                if walk::random(g, 10) < 5 { wb(g, p, 39, -1) }
            } else { let c = b(g, 37) - 1; wb(g, p, 37, c) }
        } else {
            let d = if sb(g, 40) >= 0 { g.mem.u8((0xD8 + b(g, 36)) as u16) as u16 } else { 5 };
            let (dx, dy, _) = g.mem.direction(d);
            let (x, y) = (((w(g, 32) >> 1) + dx as i32) as i16, ((w(g, 34) >> 1) + dy as i32) as i16);
            let stepping = w(g, 32) & 1 != 0 || w(g, 34) & 1 != 0 || (b(g, 37) > 0 && walk::npc_can_step(g, x, y, p) != 0);
            if stepping {
                if b(g, 2) != g.mem.level() as u8 as i32 { continue }
                let (ox, oy) = (w(g, 32), w(g, 34));
                walk::set_cell_flags(g, ox as i16, oy as i16, 0);
                ww(g, p, 32, ox + dx as i32); ww(g, p, 34, oy + dy as i32);
                walk::set_cell_flags(g, (ox + dx as i32) as i16, (oy + dy as i32) as i16, k as i16);
                let c = b(g, 37) - 1; wb(g, p, 37, c);
                if b(g, 7) & 3 == 0 || walk::random(g, 10) < 3 { wb(g, p, 39, 0) }
            } else {
                if sb(g, 39) == 0 {
                    if walk::random(g, 10) < 5 { wb(g, p, 39, 0); let n = walk::random(g, 10) as i32; wb(g, p, 37, n + 5); wb(g, p, 40, -1) }
                    else { let n = walk::random(g, 30) as i32; wb(g, p, 39, n / 10 - 1); let n = walk::random(g, 10) as i32; wb(g, p, 37, n + 2); wb(g, p, 40, 1) }
                }
                let f = b(g, 36) + sb(g, 39); wb(g, p, 36, f);
                if b(g, 36) == 8 { wb(g, p, 36, 0) } else if b(g, 36) == 255 { wb(g, p, 36, 7) }
                if b(g, 37) == 0 { wb(g, p, 37, 1) }
            }
        }
        if sb(g, 40) != -1 { let v = sb(g, 40) ^ 1; wb(g, p, 40, v) }
    }
    let moved = round::squares(start, round::npc_cell(g, k)) as usize;
    if moved > 0 || w(g, 32) != start.0 as i32 * 2 { g.mem.set_event_id(-1); g.mem.set_redraw_pending(1) }
    moved
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::{round::{Def, NpcSt, Surprise}, rules::compendium::{Compendium, NPCS_TOML}, state::{Far, MAP_PTR, W_X, W_Y}};
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    const ROWS: i16 = 36; const COLS: i16 = 60; const ITEMS: u16 = 0x9100;
    fn ground() -> Ground {
        let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.alloc(0x8000, ROWS as usize * COLS as usize * 3); g.mem.set_pointer(MAP_PTR, Far::new(0, 0x8000));
        for i in 0..ROWS as usize * COLS as usize { g.mem.write(Far::new((i * 3 + 1) as u16, 0x8000), 0x20, 1) }
        g.mem.set_map_stride(COLS); g.mem.set_map_blocks_high((ROWS / 12) as u8); g.mem.set_u8(0x5C3B, (COLS / 20) as u8);
        g.mem.alloc(ITEMS, 46 * 140); g.mem.set_pointer(0x6D6D, Far::new(0, ITEMS));
        for id in 0..=28u16 { g.mem.write(Far::new(46 * id + 0x1D, ITEMS), 2, 2); g.mem.write(Far::new(46 * id + 0x27, ITEMS), if id >= 26 { 0 } else { 0x78 }, 1); g.mem.write(Far::new(46 * id + 0x28, ITEMS), 20, 1) }
        for (d, (dx, dy)) in [(0i8, 0i8), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1)].into_iter().enumerate() { g.mem.set_u8(0x2F5 + 2 * d as u16, dx as u8); g.mem.set_u8(0x2F6 + 2 * d as u16, dy as u8) }
        for (d, (dx, dy)) in [(0i8, 0i8), (-1, 0), (-1, 1), (0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1)].into_iter().enumerate() { g.mem.set_u8(crate::ground::state::DIRECTIONS + 3 * d as u16, dx as u8); g.mem.set_u8(crate::ground::state::DIRECTIONS + 3 * d as u16 + 1, dy as u8) }
        for f in 0..8u16 { g.mem.set_u8(0xD8 + f, (f + 1) as u8) }
        g.mem.set_party_u8(0, 0x1F); for i in 0..5 { g.mem.set_party_u8(1 + i, i as u8) }
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        for i in 0..5u16 { let r = 0x102 + 256 * i; for (k, b) in format!("Trav{}", i + 1).bytes().enumerate() { g.mem.set_party_u8(r + k as u16, b) } g.mem.set_party_u8(r + 0x23, 21); g.mem.set_party_u8(r + 0x24, 9); g.mem.set_party_u8(r + 0x25, 21); g.mem.set_party_u8(r + 0x14 + 1, 8); }
        g.mem.set_outdoors(1); g.mem.set_npc_count(32); g.mem.set_combat(1); g.mem.set_event(-1);
        for (i, (r, c)) in [(22i16, 5i16), (22, 6), (23, 5), (23, 6), (22, 7)].into_iter().enumerate() { let w = Mem::walker(i as i16); g.mem.write(w, 1, 1); g.mem.write(w.add(W_X), (r * 2) as u16 as u32, 2); g.mem.write(w.add(W_Y), (c * 2) as u16 as u32, 2); }
        g.mem.set_x(44); g.mem.set_y(10); g.mem.set_view_x(38); g.mem.set_view_y(0); g.mem.set_leader(0);
        g.scene = Some(crate::ui::SceneHook::default());
        g.squad = Some(squad::State::with_compendium(Compendium::parse(NPCS_TOML).unwrap()));
        g
    }
    fn place_npc(g: &mut Ground, k: u8, (row, col): (i16, i16), attitude: u8, weapon: u8, picture: u8) {
        let n = Mem::npc(k as i16);
        g.mem.write(n.add(2), 0, 1); g.mem.write(n.add(7), 2, 1); g.mem.write(n.add(9), picture as u32, 1); g.mem.write(n.add(10), 12, 1); g.mem.write(n.add(11), 3, 1); g.mem.write(n.add(13), attitude as u32, 1);
        g.mem.write(n.add(31), weapon as u32, 1); g.mem.write(n.add(0x1E), 50, 1);
        g.mem.write(n.add(N_X), (row * 2) as u16 as u32, 2); g.mem.write(n.add(N_Y), (col * 2) as u16 as u32, 2);
        g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 2) as u16, 0x8000), k as u32, 1);
    }
    fn wall(g: &mut Ground, (row, col): (i16, i16)) { g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 1) as u16, 0x8000), 0, 1) }
    fn p1(g: &Ground) -> &round::State { &g.squad.as_ref().unwrap().p1 }
    fn fight(g: &mut Ground, participants: Option<Vec<u8>>) { g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, participants, ..Def::default_sight() }); round::pass(g, &[]); assert!(round::in_encounter(g)) }
    fn fight_far(g: &mut Ground) { g.squad.as_mut().unwrap().p1.defs.push(Def { id: "far".into(), start: round::Start::Area(round::Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), boundary: round::Boundary::Level, surprise: Surprise::None, ..Def::default_sight() }); round::pass(g, &[]); assert!(round::in_encounter(g)) }
    fn hits(g: &Ground, i: u16) -> u8 { g.mem.party_u8(0x127 + 256 * i) }
    fn shots(g: &Ground, k: u8) -> Vec<String> { p1(g).log.iter().filter(|l| l.contains(&format!("(NPC {k}) fires the")) || l.starts_with(&format!("NPC {k} fires the"))).cloned().collect() }

    #[test]
    fn the_shot_compendium_skill_derived_fallback_ammo_and_clip() {
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41); fight(&mut g, None);
        assert_eq!(p1(&g).strip, "Your move: Trav1");
        let before = (0..5).map(|i| hits(&g, i)).collect::<Vec<_>>();
        round::end_turn(&mut g);
        let s = shots(&g, 14); assert_eq!(s.len(), 1, "{:?}", p1(&g).log);
        assert!(s[0].starts_with("thug (NPC 14) fires the 9mm auto pistol at Trav") && s[0].contains(": skill 0, 2D "), "{}", s[0]);
        assert_eq!(npc_byte(&g, 14, N_AMMO), 49, "one round of the 1990 supply spent"); assert_eq!(p1(&g).npcs[&14].clip, 1);
        let after = (0..5).map(|i| hits(&g, i)).collect::<Vec<_>>();
        if s[0].contains("hit by") { assert!(after.iter().zip(&before).any(|(a, b)| a < b), "a hit lands on a Traveller: {s:?} {after:?}") } else { assert_eq!(after, before) }
        assert_eq!(g.mem.read_i(Mem::npc(14).add(N_TOKEN), 2), -1, "the reaction token is spent on the first attack");
        assert!(p1(&g).npcs[&14].engaged); assert_eq!(p1(&g).strip, "Your move: Trav2");
        g.squad.as_mut().unwrap().p1.npcs.get_mut(&14).unwrap().clip = 14;
        let until = |g: &mut Ground, f: &dyn Fn(&Ground) -> bool| { for _ in 0..12 { if f(g) { return true } round::end_turn(g) } f(g) };
        assert!(until(&mut g, &|g| p1(g).npcs[&14].reload), "{:?}", p1(&g).log); assert_eq!(shots(&g, 14).len(), 2);
        assert!(until(&mut g, &|g| p1(g).log.iter().any(|l| l == "thug (NPC 14) reloads")), "{:?}", p1(&g).log);
        assert!(!p1(&g).npcs[&14].reload && p1(&g).npcs[&14].clip == 0); assert_eq!(shots(&g, 14).len(), 2, "the reload round fires nothing");
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41);
        g.squad.as_mut().unwrap().p1.npcs.insert(14, NpcSt { role: Some("none".into()), ..Default::default() });
        fight(&mut g, None); round::end_turn(&mut g);
        let s = shots(&g, 14); assert!(s.len() == 1 && s[0].starts_with("NPC 14 fires the 9mm auto pistol at Trav") && s[0].contains(": skill 1 (derived), 2D "), "{s:?}");
        g.mem.write(Mem::npc(14).add(N_AMMO), 0, 1);
        let until = |g: &mut Ground, f: &dyn Fn(&Ground) -> bool| { for _ in 0..12 { if f(g) { return true } round::end_turn(g) } f(g) };
        assert!(until(&mut g, &|g| p1(g).log.iter().any(|l| l == "NPC 14 has no ammunition left")), "{:?}", p1(&g).log); assert_eq!(shots(&g, 14).len(), 1);
    }
    #[test]
    fn unwilling_npc_wanders_and_turns_when_hit() {
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41); place_npc(&mut g, 17, (25, 9), 2, 4, 42); fight(&mut g, None);
        g.mem.write(Mem::npc(17).add(37), 6, 1);
        for _ in 0..2 { round::end_turn(&mut g) }
        assert!(p1(&g).log.iter().any(|l| l.starts_with("civilian (NPC 17) minds its own business")), "{:?}", p1(&g).log);
        assert!(shots(&g, 17).is_empty()); assert!(round::squares(round::npc_cell(&g, 17), (25, 9)) <= 3, "wandered at most three tiles: {:?}", round::npc_cell(&g, 17));
        assert_eq!(g.mem.read(Mem::npc(17).add(N_X), 2) & 1, 0, "ends on a whole tile");
        g.mem.write(Mem::npc(17).add(0x26), 1, 1); g.mem.set_party_fired(1);
        for _ in 0..5 { if round::in_encounter(&g) { round::end_turn(&mut g) } }
        assert!(!shots(&g, 17).is_empty() || p1(&g).log.iter().any(|l| l.starts_with("civilian (NPC 17) closes in")), "{:?}", p1(&g).log);
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "s".into(), surprise: Surprise::Attackers, attacker: Side::Party, ..Def::default_sight() });
        round::pass(&mut g, &[]); assert_eq!(p1(&g).encounter.as_ref().unwrap().surprised, Some(Side::Npcs));
        round::end_turn(&mut g); assert!(p1(&g).log.iter().any(|l| l.starts_with("thug (NPC 14) is surprised")), "{:?}", p1(&g).log); assert!(shots(&g, 14).is_empty());
    }
    #[test]
    fn approach_round_a_wall_then_fire() {
        let mut g = ground(); place_npc(&mut g, 14, (22, 16), 0, 4, 41);
        for r in 18..=26 { wall(&mut g, (r, 11)) }
        g.mem.set_view_x(38); g.mem.set_view_y(0);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "w".into(), start: round::Start::Area(round::Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), boundary: round::Boundary::Level, surprise: Surprise::None, ..Def::default_sight() });
        round::pass(&mut g, &[]); assert!(round::in_encounter(&g) && round::sees(&g, 14).is_none(), "{:?}", p1(&g).log);
        round::end_turn(&mut g);
        let at = round::npc_cell(&g, 14);
        assert!(at != (22, 16) && round::squares(at, (22, 5)) < 11, "walked toward the party round the wall: {at:?} {:?}", p1(&g).log);
        assert_eq!(p1(&g).npcs[&14].speed, 1);
        assert!(round::sees(&g, 14).is_some() && shots(&g, 14).len() == 1, "the move brought a Traveller into sight, the shot followed: {at:?} {:?}", p1(&g).log);
        assert!(!can_step(&g, 14, (20, 11), false) && !can_step(&g, 14, (22, 6), false) && can_step(&g, 14, (20, 12), false));
        g.mem.write(Mem::npc(14).add(7), 2 | (2 << 3), 1); g.mem.write(Mem::npc(14).add(3), (at.0 * 2) as u32, 2); g.mem.write(Mem::npc(14).add(5), (at.1 * 2) as u32, 2);
        assert!(!can_step(&g, 14, (at.0 + 1, at.1 + 2), false) && can_step(&g, 14, (at.0 + 1, at.1 + 2), true) || !in_map(&g, (at.0 + 1, at.1 + 2)));
        g.mem.write(Mem::npc(14).add(7), 4, 1); assert!(!can_step(&g, 14, (at.0, at.1 + 1), true));
    }
    #[test]
    fn morale_roll_withdrawal_and_rout() {
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41); place_npc(&mut g, 15, (30, 30), 0, 4, 41); place_npc(&mut g, 16, (31, 30), 0, 4, 41);
        for k in [15u8, 16] { g.mem.write(Mem::npc(k as i16).add(2), 20, 1) }
        fight(&mut g, Some(vec![14, 15, 16]));
        round::end_turn(&mut g);
        let m: Vec<&String> = p1(&g).log.iter().filter(|l| l.starts_with("Morale (2 of 3 down, Low): Formidable 15+, Leader 0: 2D ")).collect();
        assert_eq!(m.len(), 1, "{:?}", p1(&g).log);
        let e = p1(&g).encounter.clone().unwrap(); assert_eq!(e.morale_round, 1);
        let fell = m[0].ends_with("they fall back") || m[0].ends_with("ROUT");
        assert_eq!(e.withdrawing, fell, "{}", m[0]);
        if m[0].ends_with("ROUT") { assert!(p1(&g).npcs[&14].routed && p1(&g).log.iter().any(|l| l.starts_with("thug (NPC 14) runs")), "{:?}", p1(&g).log) }
        else if fell { assert!(p1(&g).log.iter().any(|l| l.starts_with("thug (NPC 14) falls back")), "{:?}", p1(&g).log) }
        for _ in 0..4 { round::end_turn(&mut g) }
        if p1(&g).encounter.is_some() {
            assert_eq!(p1(&g).encounter.as_ref().unwrap().round, 2);
            round::end_turn(&mut g);
            let rolls = p1(&g).log.iter().filter(|l| l.starts_with("Morale (")).count();
            assert_eq!(rolls, if fell { 1 } else { 2 }, "{:?}", p1(&g).log);
        } else { assert!(fell, "{:?}", p1(&g).log) }
        let mut g = ground(); place_npc(&mut g, 14, (24, 7), 0, 4, 41); fight(&mut g, Some(vec![14]));
        g.squad.as_mut().unwrap().p1.npcs.entry(14).or_default().routed = true;
        let d0 = round::squares(round::npc_cell(&g, 14), (22, 5));
        round::end_turn(&mut g);
        assert!(round::squares(round::npc_cell(&g, 14), (22, 5)) > d0, "{:?}", p1(&g).log); assert!(shots(&g, 14).is_empty());
        assert!(p1(&g).log.iter().any(|l| l.starts_with("thug (NPC 14) runs (")), "{:?}", p1(&g).log);
        assert_eq!(round::pools(&g).1, 0, "a routed NPC adds no Tactics");
    }
    #[test]
    fn alerted_approach_lifts_the_leash() {
        let mut g = ground(); place_npc(&mut g, 14, (7, 50), 0, 4, 41);
        g.mem.write(Mem::npc(14).add(7), 2 | (2 << 3), 1); g.mem.write(Mem::npc(14).add(3), 14, 2); g.mem.write(Mem::npc(14).add(5), 100, 2);
        for r in 0..ROWS { if r != 7 { wall(&mut g, (r, 30)) } }
        g.squad.as_mut().unwrap().p1.npcs.insert(14, NpcSt { alerted: Some(10), alert_full: 10, last_seen: Some((7, 20)), ..Default::default() });
        round::pass(&mut g, &[]); assert!(!round::in_encounter(&g), "{:?}", p1(&g).log);
        for _ in 0..10 { round::walked(&mut g, 1); round::pass(&mut g, &[]); }
        let at = round::npc_cell(&g, 14);
        assert!(round::squares(at, (7, 50)) >= 9 && at.1 < 42, "ten squares toward the last seen square, the leash ignored: {at:?}");
        assert!(!round::in_encounter(&g));
        g.squad.as_mut().unwrap().p1.npcs.get_mut(&14).unwrap().alerted = None;
        let before = round::npc_cell(&g, 14);
        assert_eq!(approach(&mut g, 14, 5, None), 0, "unalerted and outside its leash: no step is allowed"); assert_eq!(round::npc_cell(&g, 14), before);
    }
    #[test]
    fn selective_autofire_uses_distance_box_and_toggle_in_the_attack() {
        for enabled in [true, false] { for distance in [4, 5, 6, 8] { for is_boxed in [false, true] {
            let mut g = ground(); g.balance.npc_selective_autofire = enabled;
            place_npc(&mut g, 14, (22, 5 + distance), 0, 12, 41);
            if is_boxed {
                for (i, (r, c)) in [(21, 5), (22, 6), (23, 5), (22, 4)].into_iter().enumerate() {
                    let walker = Mem::walker(i as i16 + 1);
                    g.mem.write(walker.add(W_X), (r * 2) as u32, 2); g.mem.write(walker.add(W_Y), (c * 2) as u32, 2);
                }
            }
            assert_eq!(boxed(&g, 0), is_boxed);
            assert!(round::line_of_sight(&g, round::npc_cell(&g, 14), round::traveller_cell(&g, 0)));
            let f = round::npc_fighter(&g, 14);
            assert!(attack(&mut g, 14, 0, rules::weapon(12).unwrap(), &f));
            let full = !enabled || distance <= 5 || is_boxed;
            assert_eq!((npc_byte(&g, 14, N_AMMO), p1(&g).npcs[&14].clip), if full { (40, 10) } else { (49, 1) });
            assert_eq!(shots(&g, 14)[0].contains("(burst, 10 rounds)"), full);
            assert_eq!(p1(&g).log.iter().any(|l| l.contains("is boxed (no free orthogonal neighbour)")), enabled && distance > 5 && is_boxed);
        } } }
    }
    #[test]
    fn boxed_uses_movement_terrain_occupancy_and_map_edges() {
        let mut g = ground();
        wall(&mut g, (21, 5));
        assert!(fire::in_cover(&g, (18, 5), (22, 5)) && !boxed(&g, 0));
        wall(&mut g, (22, 4)); assert!(boxed(&g, 0));
        let walker = Mem::walker(0); g.mem.write(walker.add(W_X), 0, 2); g.mem.write(walker.add(W_Y), 0, 2);
        wall(&mut g, (0, 1)); assert!(!boxed(&g, 0));
        place_npc(&mut g, 15, (1, 0), 1, 4, 41); assert!(boxed(&g, 0));
    }
    #[test]
    fn npc_burst_bonus_attacks_danger_space_and_the_clip() {
        let mut g = ground(); place_npc(&mut g, 14, (24, 5), 0, 12, 41); fight(&mut g, None);
        round::end_turn(&mut g);
        let s = shots(&g, 14); assert_eq!(s.len(), 1, "{:?}", p1(&g).log);
        assert!(s[0].starts_with("thug (NPC 14) fires the 9mm submachinegun at Trav3: short: skill ") && s[0].ends_with(" (burst, 10 rounds)"), "{}", s[0]);
        assert_eq!((npc_byte(&g, 14, N_AMMO), p1(&g).npcs[&14].clip), (40, 10), "10 rounds off the 1990 supply, counted toward the PM clip");
        let bonus: Vec<&String> = p1(&g).log.iter().filter(|l| l.contains("burst on Trav")).collect();
        assert_eq!(bonus.len(), 3, "Trav1 (22, 5), Trav2 (22, 6), Trav4 (23, 6) stand next to the primary: three bonus attacks; {:?}", p1(&g).log);
        assert!(bonus.iter().all(|l| !l.contains("Trav3")), "the primary takes no bonus attack while others stand beside it; {bonus:?}");
        g.squad.as_mut().unwrap().p1.npcs.get_mut(&14).unwrap().clip = 25; g.mem.write(Mem::npc(14).add(N_AMMO), 3, 1);
        let until = |g: &mut Ground, f: &dyn Fn(&Ground) -> bool| { for _ in 0..12 { if f(g) { return true } round::end_turn(g) } f(g) };
        assert!(until(&mut g, &|g| shots(g, 14).len() == 2), "{:?}", p1(&g).log);
        assert!(shots(&g, 14)[1].ends_with(" (burst, 3 rounds)") && npc_byte(&g, 14, N_AMMO) == 0 && p1(&g).npcs[&14].clip == 28 && !p1(&g).npcs[&14].reload, "the burst is capped by the supply (3 >= 2); 28 < 30: the clip owes no reload yet; {:?}", p1(&g).log);
        let mut g = ground(); place_npc(&mut g, 14, (24, 5), 0, 12, 41); g.mem.write(Mem::npc(14).add(N_AMMO), 1, 1); fight(&mut g, None); round::end_turn(&mut g);
        assert!(!shots(&g, 14)[0].contains("burst"), "{:?}", shots(&g, 14)); assert_eq!(npc_byte(&g, 14, N_AMMO), 0);
        let mut g = ground(); place_npc(&mut g, 14, (24, 5), 0, 25, 41); fight(&mut g, None); round::end_turn(&mut g);
        assert!(shots(&g, 14)[0].contains("fires the TL12 PGMP at Trav3"), "{:?}", p1(&g).log);
        assert_eq!(p1(&g).log.iter().filter(|l| l.contains("blast (") && l.contains("sq, Pen")).count(), 4, "Trav1, 2, 4, 5 inside the danger space; {:?}", p1(&g).log);
    }
    #[test]
    fn npc_melee_confrontation_and_the_disarmed_hands() {
        let mut g = ground(); place_npc(&mut g, 14, (24, 5), 0, 26, 41); fight(&mut g, None);
        round::end_turn(&mut g);
        let l = p1(&g).log.iter().find(|l| l.starts_with("thug (NPC 14) attacks Trav3 with the dagger: close: skill ")).cloned();
        assert!(l.is_some(), "{:?}", p1(&g).log); assert_eq!(npc_byte(&g, 14, N_AMMO), 50, "no ammunition for a knife");
        assert!(shots(&g, 14).is_empty(), "no shot line for the melee attack");
        let (dskill, block) = fire::defence(&g, Unit::Traveller(2)); assert_eq!((dskill.level, dskill.unskilled, block), (0, true, 1), "Trav3 holds a gun: hands 1 and unskilled as a defender");
        let task = fire::melee_task(adapters::Skill { level: 0, unskilled: true }, 0, dskill, block);
        assert_eq!((task.difficulty, task.dm), (rules::Difficulty::Routine, -1), "unskilled vs unskilled: Routine, DM -1 (the Block)");
        let mut g = ground(); place_npc(&mut g, 14, (24, 20), 0, 26, 41); fight_far(&mut g); round::end_turn(&mut g);
        assert!(p1(&g).log.iter().any(|l| l == "thug (NPC 14) closes in to strike"), "{:?}", p1(&g).log);
        assert!(!p1(&g).log.iter().any(|l| l.contains("[P20")), "no placeholder lines remain: {:?}", p1(&g).log);
        let mut g = ground(); place_npc(&mut g, 14, (24, 5), 0, 4, 41); fight(&mut g, None);
        g.squad.as_mut().unwrap().fire.disarmed.push(14);
        round::end_turn(&mut g);
        assert!(shots(&g, 14).is_empty() && p1(&g).log.iter().any(|l| l.starts_with("thug (NPC 14) attacks Trav3 with the hands: close: skill 0 unskilled, 2D ")), "{:?}", p1(&g).log);
        assert_eq!(npc_byte(&g, 14, N_AMMO), 50, "the pistol was not fired");
    }
    #[test]
    fn npc_launcher_round_lands_at_its_next_turn() {
        let mut g = ground(); place_npc(&mut g, 14, (30, 5), 0, 17, 41); fight_far(&mut g);
        round::end_turn(&mut g);
        let s = shots(&g, 14); assert_eq!(s.len(), 1, "{:?}", p1(&g).log);
        assert!(s[0].starts_with("thug (NPC 14) fires the 4cm RAM GL-10 at (23, 5): skill ") && s[0].contains("; a marker on ("), "{}", s[0]);
        let pend = &g.squad.as_ref().unwrap().fire.pending; assert_eq!((pend.len(), pend[0].who, pend[0].launcher), (1, Unit::Npc(14), true));
        assert_eq!((npc_byte(&g, 14, N_AMMO), p1(&g).npcs[&14].clip), (49, 1));
        assert!(!p1(&g).log.iter().any(|l| l.contains("lands at")), "nothing lands in the round it was fired");
        let until = |g: &mut Ground, f: &dyn Fn(&Ground) -> bool| { for _ in 0..12 { if f(g) { return true } round::end_turn(g) } f(g) };
        assert!(until(&mut g, &|g| p1(g).log.iter().any(|l| l.starts_with("thug (NPC 14)'s 4cm RAM GL-10 lands at ("))), "{:?}", p1(&g).log);
        assert_eq!((shots(&g, 14).len(), g.squad.as_ref().unwrap().fire.pending.len()), (2, 1), "the old round landed, the turn's new one is in the air; {:?}", p1(&g).log);
        assert!(p1(&g).log.iter().any(|l| l.contains("blast (") && l.contains(" sq, Pen ") && l.contains(") on Trav")), "the round lands on the party's square (or scatters next to it): the danger-space rolls; {:?}", p1(&g).log);
        let mut g = ground(); place_npc(&mut g, 14, (30, 5), 0, 17, 41); g.mem.set_interior_kind(b'I'); fight_far(&mut g); round::end_turn(&mut g);
        assert!(p1(&g).log.iter().any(|l| l == "thug (NPC 14) holds fire: no room for the 4cm RAM GL-10 indoors"), "{:?}", p1(&g).log);
        assert!(g.squad.as_ref().unwrap().fire.pending.is_empty() && npc_byte(&g, 14, N_AMMO) == 50);
    }
}
