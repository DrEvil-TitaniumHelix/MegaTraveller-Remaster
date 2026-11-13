use mt1::{ground::{fire, orders, round::{self, Boundary, Def, End, NpcSt, Order, Rect, Side, Start, Step, Surprise, Turn, KEY_SPACE, MOVE_WALK, ROUND_TICKS}, rules::{self, adapters, balance::Options, compendium::Compendium, weapons::{Profile, GRENADES_PM, WEAPONS_PM}, Band, Difficulty, Level}, squad, state::{Far, Ground, Mem, MAP_PTR, N_X, N_Y, W_ACTIVE, W_X, W_Y}, walk}, play, save::{Mode, Saved}, ui::{Intent, Target, Unit, Verb}};
use mt_engine::{io::{Input, Overlay, Scripted}, play::{Choice, Decision, Kind, Play}, sim::tick::Counter};

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
    g.squad = Some(squad::State::with_compendium(Compendium::parse(include_str!("../../../data/npcs.toml")).unwrap()));
    g
}
fn place(g: &mut Ground, k: u8, (row, col): (i16, i16), attitude: u8, weapon: u8) {
    let n = Mem::npc(k as i16);
    g.mem.write(n.add(2), 0, 1); g.mem.write(n.add(9), 41, 1); g.mem.write(n.add(10), 12, 1); g.mem.write(n.add(11), 3, 1); g.mem.write(n.add(13), attitude as u32, 1);
    g.mem.write(n.add(31), weapon as u32, 1); g.mem.write(n.add(0x1E), 50, 1);
    g.mem.write(n.add(N_X), (row * 2) as u16 as u32, 2); g.mem.write(n.add(N_Y), (col * 2) as u16 as u32, 2);
    g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 2) as u16, 0x8000), k as u32, 1);
}
fn arm(g: &mut Ground, who: u8, id: u8, rounds: u8) {
    let r = (who as u16) << 8; let index = who as u16 + 1;
    g.mem.set_party_u8(r + 0x1CE + 42, index as u8); g.mem.set_party_u8(r + 0x1CE + 43, 0);
    g.mem.set_party_u8(0xA82 + 5 * (index - 1), id); g.mem.set_party_u8(0xA82 + 5 * (index - 1) + 2, rounds);
    g.mem.write(Far::new(46 * id as u16 + 0x1D, 0x9100), 2, 2); g.mem.write(Far::new(46 * id as u16 + 0x27, 0x9100), 0x78, 1);
}
fn kill(g: &mut Ground, k: u8) { g.mem.write(Mem::npc(k as i16).add(2), 20, 1) }
fn wall(g: &mut Ground, (row, col): (i16, i16)) { g.mem.write(Far::new(((row as usize * COLS as usize + col as usize) * 3 + 1) as u16, 0x8000), 0x50, 1) }
fn fight_def(g: &mut Ground, surprise: Surprise) { g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise, ..Def::default_sight() }) }
fn npc_turn_lines(g: &Ground, k: u8) -> usize { p1(g).log.iter().filter(|l| l.contains(&format!("(NPC {k})")) && !l.starts_with("Trav")).count() }
fn p1(g: &Ground) -> &round::State { &g.squad.as_ref().unwrap().p1 }
fn encounter(g: &Ground) -> Option<round::Encounter> { p1(g).encounter.clone() }
fn log_has(g: &Ground, s: &str) -> bool { p1(g).log.iter().any(|l| l.contains(s)) }

fn walker_at(g: &mut Ground, i: u8, (row, col): (i16, i16)) {
    let w = Mem::walker(i as i16);
    g.mem.write(w.add(W_ACTIVE), 1, 1); g.mem.write(w.add(W_X), (row * 2) as u16 as u32, 2); g.mem.write(w.add(W_Y), (col * 2) as u16 as u32, 2);
}
fn starts(fix: &dyn Fn(&mut Ground), cell: (i16, i16)) -> bool { let mut g = ground(); fix(&mut g); place(&mut g, 14, cell, 0, 4); round::pass(&mut g, &[]); encounter(&g).is_some() }
#[test]
fn a_sight_start_needs_the_1990_attack_window_not_medium_range() {
    let g: &dyn Fn(&mut Ground) = &|_| {};
    assert_eq!(round::SCREEN_TILES, (7, 12));
    assert!(starts(g, (19, 0)) && starts(g, (25, 11)) && starts(g, (19, 11)) && starts(g, (25, 0)), "the screen's four corners engage");
    assert!(!starts(g, (18, 5)), "one row above the top: no fight"); assert!(!starts(g, (26, 5)), "one row below the bottom: no fight");
    assert!(!starts(g, (22, 12)), "one column past the right edge: no fight"); assert!(!starts(g, (2, 5)), "20 rows off: no fight");
    let g: &dyn Fn(&mut Ground) = &|g| walk::set_player_pos(g, 22, 20);
    { let mut f = ground(); g(&mut f); assert_eq!((f.mem.view_x(), f.mem.view_y()), (38, 28)); }
    assert!(starts(g, (19, 14)) && starts(g, (25, 25)), "corners");
    assert!(!starts(g, (22, 13)), "one column past the left edge: no fight"); assert!(!starts(g, (22, 26)), "one column past the right edge: no fight");
    assert!(!starts(g, (18, 20)) && !starts(g, (26, 20)), "one row past the top / bottom: no fight");
    let g: &dyn Fn(&mut Ground) = &|g| walk::set_player_pos(g, 2, 3);
    { let mut f = ground(); g(&mut f); assert_eq!((f.mem.view_x(), f.mem.view_y()), (0, 0), "clamped at 0"); }
    assert!(starts(g, (6, 11)) && starts(g, (0, 1)), "the clamped screen's corners engage ((0, 0) is an absent record, round::npc_present)");
    assert!(!starts(g, (7, 11)) && !starts(g, (6, 12)), "one past the clamped screen's bottom / right: no fight");
    let g: &dyn Fn(&mut Ground) = &|g| { g.mem.set_combat(1); walker_at(g, 4, (10, 40)) };
    { let mut f = ground(); g(&mut f); assert_eq!(round::traveller_cell(&f, 4), (10, 40)); }
    assert!(!starts(g, (10, 41)), "adjacent to the far Traveller, off the leader's screen: no fight");
    assert!(starts(g, (22, 6)), "beside the leader, on the screen: Encounter!");
}
#[test]
fn the_default_fight_keeps_an_npc_off_the_screen_out_of_it() {
    let mut g = ground();
    place(&mut g, 14, (22, 9), 0, 14); place(&mut g, 11, (12, 20), 0, 17); fight_def(&mut g, Surprise::None);
    round::pass(&mut g, &[]);
    assert!(encounter(&g).is_some(), "{:?}", p1(&g).log);
    assert_eq!(round::npc_side(&g), vec![14], "the far NPC is not on the side");
    for _ in 0..6 { if encounter(&g).map_or(true, |e| e.round > 1) { break } orders::intent(&mut g, Intent::EndTurn) }
    assert_eq!(npc_turn_lines(&g, 11), 0, "no turn for the NPC off the screen: {:?}", p1(&g).log);
    assert!(npc_turn_lines(&g, 14) >= 1, "the near NPC acted in round 1: {:?}", p1(&g).log);
    kill(&mut g, 14);
    for _ in 0..12 { if encounter(&g).is_none() { break } orders::intent(&mut g, Intent::EndTurn) }
    assert!(encounter(&g).is_none(), "the far NPC does not hold the fight open: {:?}", p1(&g).log);
    for _ in 0..3 { round::pass(&mut g, &[]) }
    assert_eq!(p1(&g).log.iter().filter(|l| l.starts_with("Encounter!")).count(), 1, "no new fight on the far NPC: {:?}", p1(&g).log);
}
#[test]
fn victory_is_not_followed_by_a_default_fight_with_npcs_off_screen() {
    let mut g = ground();
    place(&mut g, 14, (22, 9), 0, 14); for (k, c) in [(11, 25), (12, 26), (13, 27)] { place(&mut g, k, (2, c), 0, 17) }
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "ambush".into(), start: Start::Area(Rect { r0: 20, c0: 1, r1: 24, c1: 12 }), end: End::Locked { escape: None }, boundary: Boundary::Area(Rect { r0: 17, c0: 0, r1: 27, c1: 16 }), participants: Some(vec![14]), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    assert_eq!(encounter(&g).map(|e| e.def), Some("ambush".into()));
    assert_eq!(round::npc_side(&g), vec![14], "boundary = area: the pirates never join");
    kill(&mut g, 14); orders::intent(&mut g, Intent::EndTurn);
    assert!(encounter(&g).is_none() && p1(&g).strip == "Victory", "{:?}", p1(&g).log);
    for _ in 0..3 { round::pass(&mut g, &[]) }
    assert!(encounter(&g).is_none(), "no Encounter! after Victory: the pirates are 20 rows off, out of the 1990 window: {:?}", p1(&g).log);
    assert_eq!(p1(&g).log.iter().filter(|l| l.starts_with("Encounter!")).count(), 1, "one fight in the whole log");
}

fn alert(g: &mut Ground, k: u8) { g.squad.as_mut().unwrap().p1.npcs.insert(k, NpcSt { alerted: Some(10), alert_full: 10, last_seen: Some((22, 5)), ..Default::default() }); }
fn walks(g: &mut Ground, n: usize) -> usize { for i in 0..n { if encounter(g).is_some() { return i } round::walked(g, 1); round::pass(g, &[]) } n }
#[test]
fn an_alerted_hunter_off_the_screen_closes_until_the_fight_can_start() {
    let mut g = ground(); place(&mut g, 14, (22, 16), 0, 4); alert(&mut g, 14);
    round::pass(&mut g, &[]); assert!(encounter(&g).is_none(), "off the screen: no fight yet: {:?}", p1(&g).log);
    let n = walks(&mut g, 12);
    assert!(encounter(&g).is_some(), "the hunter never arrived in 12 walks: at {:?} {:?}", round::npc_cell(&g, 14), p1(&g).log);
    assert!(round::on_screen(&g, round::npc_cell(&g, 14)) && n <= 6, "it stepped onto the screen ({n} walks, at {:?})", round::npc_cell(&g, 14));
    assert!(log_has(&g, "No surprise: they were expecting you"), "{:?}", p1(&g).log);
    let mut g = ground(); place(&mut g, 14, (22, 16), 0, 4); alert(&mut g, 14);
    for (r, c) in [(21, 15), (21, 16), (21, 17), (22, 15), (22, 17), (23, 15), (23, 16), (23, 17)] { wall(&mut g, (r, c)) }
    assert_eq!(walks(&mut g, 12), 12); assert!(encounter(&g).is_none()); assert_eq!(round::npc_cell(&g, 14), (22, 16), "no path: stays put");
    assert_eq!(p1(&g).strip, "Alerted: 1", "still alerted (within the radius)");
    let mut g = ground(); place(&mut g, 14, (22, 16), 0, 4);
    assert_eq!(walks(&mut g, 12), 12); assert!(encounter(&g).is_none()); assert_eq!(round::npc_cell(&g, 14), (22, 16));
}

#[test]
fn every_unit_acts_once_a_round_and_a_round_opens_on_the_marching_head() {
    let mut g = ground();
    place(&mut g, 14, (22, 9), 0, 4); place(&mut g, 15, (23, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    assert_eq!(encounter(&g).map(|e| (e.round, e.side)), Some((1, Side::Party)));
    let mut prompts: Vec<(u16, u8)> = Vec::new();
    let npc_lines = |g: &Ground, round: u16| p1(g).log.iter().filter(|l| l.starts_with("Round ") || l.contains("(NPC ")).fold((0u16, 0usize), |(r, n), l| if l.starts_with("Round ") { (l[6..7].parse().unwrap(), n) } else if r == round && (l.contains(") fires") || l.contains(") closes in") || l.contains(") reloads") || l.contains(") has no ammunition")) { (r, n + 1) } else { (r, n) }).1;
    round::pass(&mut g, &[Intent::Select(4)]); assert_eq!(round::selected(&g), 4);
    for _ in 0..12 {
        let e = encounter(&g).unwrap(); if e.round > 2 { break }
        assert_eq!(e.side, Side::Party); prompts.push((e.round, round::selected(&g)));
        round::pass(&mut g, &[Intent::EndTurn]);
    }
    let round1: Vec<u8> = prompts.iter().filter(|p| p.0 == 1).map(|p| p.1).collect();
    let round2: Vec<u8> = prompts.iter().filter(|p| p.0 == 2).map(|p| p.1).collect();
    assert_eq!(round1.len(), 5, "five Travellers, five turns: {prompts:?}");
    assert_eq!({ let mut s = round1.clone(); s.sort(); s }, vec![0, 1, 2, 3, 4], "each exactly once: {prompts:?}");
    assert_eq!(round1[0], 4, "the player's out-of-order pick stands");
    assert_eq!(round2[0], 0, "round 2 opens on the marching order's head (D-R14), not the last-selected Traveller: {prompts:?}");
    assert_eq!({ let mut s = round2.clone(); s.sort(); s }, vec![0, 1, 2, 3, 4], "{prompts:?}");
    assert_eq!(npc_lines(&g, 1), 2, "two NPCs, two NPC turns in round 1: {:?}", p1(&g).log);
    let mut g = ground();
    place(&mut g, 14, (22, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]); round::pass(&mut g, &[Intent::Select(2)]);
    round::order(&mut g, Order::Attack);
    assert_eq!((round::selected(&g), p1(&g).turns[2].attacked, p1(&g).turns[2].done), (2, true, false));
    assert_eq!(p1(&g).strip, "Your move: Trav3", "the half-done turn stays with Trav3");
}

#[test]
fn a_decision_reports_the_frozen_clock_never_the_raw_poll_counter() {
    let mut g = ground();
    g.input = Box::new(Play::scripted([]));
    g.squad.as_mut().unwrap().p1.clock = Some(2902);
    g.clock = Box::new(Counter::at(3563));
    play::decide(&mut g, Decision::new(Kind::CombatTurn, "t", "test"));
    assert_eq!(g.clock.now(), 2902, "the decision is registered on the frozen clock");
    for _ in 0..3 { g.clock.wait_tick() }
    round::pass(&mut g, &[Intent::OpenMenu { target: Target::Unit(Unit::Traveller(2)), at: (0, 0) }, Intent::CloseMenu]);
    play::decide(&mut g, Decision::new(Kind::CombatTurn, "t", "test"));
    assert_eq!(g.clock.now(), 2902, "a menu costs 0 ticks");
    g.input = Box::new(Scripted::new([]));
    place(&mut g, 14, (22, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    for _ in 0..5 { orders::intent(&mut g, Intent::EndTurn); }
    assert_eq!((encounter(&g).unwrap().round, p1(&g).pending), (2, ROUND_TICKS));
    let mut last = 2902;
    for _ in 0..8 { let now = round::clock(&mut g).unwrap(); assert!(now >= last, "monotonic: {now} < {last}"); last = now; g.clock.wait_tick() }
    assert_eq!(last, 2902 + ROUND_TICKS);
    g.input = Box::new(Play::scripted([]));
    play::decide(&mut g, Decision::new(Kind::CombatTurn, "t", "test"));
    assert_eq!(g.clock.now(), 2902 + ROUND_TICKS);
}

#[test]
fn the_party_is_whole_after_the_fight() {
    let mut g = ground();
    place(&mut g, 14, (22, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    assert!(g.mem.combat() != 0 && !encounter(&g).unwrap().grouped, "outdoors: the party broke for the fight");
    g.mem.write(Mem::walker(0).add(mt1::ground::state::W_X), 40, 2);
    assert_ne!(round::traveller_cell(&g, 0), round::traveller_cell(&g, 1));
    kill(&mut g, 14);
    let mut point = round::traveller_cell(&g, round::selected(&g));
    for _ in 0..6 { point = round::traveller_cell(&g, round::selected(&g)); orders::intent(&mut g, Intent::EndTurn); if encounter(&g).is_none() { break } }
    assert_eq!(p1(&g).strip, "Encounter over", "{:?}", p1(&g).log);
    assert_eq!(g.mem.combat(), 0, "the party is whole again: no residual broken state");
    assert_eq!(g.mem.individual(), 0); assert!((0..5).all(|i| g.mem.read(Mem::walker(i).add(mt1::ground::state::W_TARGET), 1) == 0), "the 1990 Regroup cleared every walker's target (walker +5)");
    assert_eq!((g.mem.x() >> 1, g.mem.y() >> 1), point, "the figure stands where the regroup point (the selected Traveller) stood");
}
#[test]
fn a_refused_step_ends_the_move_grant() {
    let mut p = Play::scripted([Choice::new("d:1", "move:east:5")]);
    let d = play::commands::ground(&ground());
    assert!(p.register(&d));
    let mut clock = Counter::default();
    for _ in 0..3 { p.poll(&mut clock); }
    assert_eq!(p.grant().map(|g| (g.requested, g.used, g.remaining)), Some((5, 2, 3)));
    p.refuse_grant();
    assert!(p.grant().is_none(), "the sim refused the held key's step: no more ticks are waited for");
    let e = &p.journal().entries[0]; assert_eq!((e.grant.requested, e.grant.used, e.grant.remaining), (5, 2, 3), "the journal keeps what was used and what was cut");
    assert!(matches!(p.poll(&mut clock), mt_engine::io::Event::KeyUp(_)), "the held key is released");
}

#[test]
fn a_pick_stands_until_the_turn_ends_or_another_pick() {
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4); fight_def(&mut g, Surprise::None);
    g.mem.set_selected_char(2);
    round::pass(&mut g, &[]);
    assert_eq!((round::selected(&g), p1(&g).strip.as_str()), (2, "Your move: Trav3"), "round 1 opens on the pre-fight selection: {:?}", p1(&g).log);
    round::pass(&mut g, &[]); assert_eq!(round::selected(&g), 2, "and it stands across an empty pass");
    round::pass(&mut g, &[Intent::Select(4)]); assert_eq!(round::selected(&g), 4);
    for _ in 0..3 { round::pass(&mut g, &[]) }
    assert_eq!((round::selected(&g), p1(&g).strip.as_str()), (4, "Your move: Trav5"), "the pick stands across empty passes: {:?}", p1(&g).log);
    round::pass(&mut g, &[Intent::Select(1)]); round::pass(&mut g, &[]); assert_eq!(round::selected(&g), 1, "another pick replaces it");
    round::pass(&mut g, &[Intent::EndTurn]);
    assert_eq!(round::selected(&g), 0, "the turn over, the prompt goes to the marching order's next open turn: {}", p1(&g).strip);
}
#[test]
fn a_shot_from_exploration_starts_the_fight_on_the_target() {
    let mut g = ground(); place(&mut g, 14, (22, 20), 0, 4); arm(&mut g, 0, 14, 20);
    round::pass(&mut g, &[]); assert!(encounter(&g).is_none(), "15 columns off: no sight start");
    round::pass(&mut g, &[Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }]);
    assert!(encounter(&g).is_none() && !log_has(&g, "fires the"), "off-screen input cannot start a fight");
    assert!(g.squad.as_ref().unwrap().orders.hover.contains("outside the screen"));
    orders::fire(&mut g, 14);
    assert!(log_has(&g, "fires the"), "the shot resolved: {:?}", p1(&g).log);
    let e = encounter(&g).expect("the shot started the fight");
    assert!(round::npc_side(&g).contains(&14), "the target is on the NPC side: {:?}", round::npc_side(&g));
    assert_eq!(p1(&g).active_def().attacker, Side::Party, "the party made the first hostile move");
    assert_eq!((e.round, p1(&g).turns[0].attacked), (1, true), "round 1, Trav1's Attack spent");
}
#[test]
fn an_npc_shot_at_inside_a_fight_joins_it_and_takes_turns() {
    let mut g = ground(); place(&mut g, 15, (22, 9), 0, 4); place(&mut g, 14, (22, 20), 0, 4); arm(&mut g, 0, 14, 20); fight_def(&mut g, Surprise::None);
    round::pass(&mut g, &[]);
    assert_eq!(round::npc_side(&g), vec![15], "15 columns off: not on the side of a default (Screen) fight");
    round::pass(&mut g, &[Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }]);
    assert_eq!(round::npc_side(&g), vec![15], "refused input cannot admit the off-screen NPC");
    assert!(g.squad.as_ref().unwrap().orders.hover.contains("outside the screen"));
    orders::fire(&mut g, 14);
    assert!(log_has(&g, "(NPC 14)"), "the shot resolved: {:?}", p1(&g).log);
    assert!(round::npc_side(&g).contains(&14), "shot at: it joins: {:?}", round::npc_side(&g));
    for _ in 0..12 { if encounter(&g).is_none() || npc_turn_lines(&g, 14) > 0 { break } round::pass(&mut g, &[Intent::EndTurn]) }
    assert!(npc_turn_lines(&g, 14) > 0, "NPC 14 takes turns: {:?}", p1(&g).log);
}
#[test]
fn a_locked_data_fight_is_not_won_by_walking_off_and_reports_participants_out_of_reach() {
    assert!(matches!(Def::default_sight().boundary, Boundary::Screen), "the engine's data-less fight: the 1990 screen");
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4); place(&mut g, 15, (2, 55), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "l".into(), surprise: Surprise::None, end: End::Locked { escape: None }, boundary: Boundary::Radius(round::ENCOUNTER_RADIUS), participants: Some(vec![14, 15]), ..Def::default_sight() });
    round::pass(&mut g, &[]); assert!(round::in_encounter(&g));
    assert!(log_has(&g, "out of reach: NPC 15"), "a participant 50 columns off is reported: {:?}", p1(&g).log);
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "l".into(), surprise: Surprise::None, end: End::Locked { escape: None }, boundary: Boundary::Radius(round::ENCOUNTER_RADIUS), ..Def::default_sight() });
    round::pass(&mut g, &[]); assert!(round::in_encounter(&g));
    for k in 0..5 { let w = Mem::walker(k); g.mem.write(w.add(W_Y), 2, 2); g.mem.write(w.add(W_X), 44, 2) } g.mem.set_y(2);
    g.mem.write(Mem::npc(14).add(N_Y), 40, 2);
    for _ in 0..3 { round::pass(&mut g, &[Intent::EndTurn]) }
    assert!(round::in_encounter(&g) && round::npc_side(&g).contains(&14), "19 columns off, inside radius 29: the hostile stays on the side, no Victory: {}", p1(&g).strip);
}
#[test]
fn a_restore_leaves_no_stale_frozen_clock_or_fight_state() {
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4); fight_def(&mut g, Surprise::None); round::pass(&mut g, &[]);
    assert!(round::in_encounter(&g));
    g.clock = Box::new(Counter::at(3000)); { let s = &mut g.squad.as_mut().unwrap().p1; s.clock = Some(3000); s.pending = 50 }
    let saved = Saved::capture(&mut g, Mode::Ground, "t");
    g.clock = Box::new(Counter::at(5000)); g.squad.as_mut().unwrap().p1.clock = Some(5000);
    saved.install(&mut g); assert_eq!(p1(&g).clock, Some(3000), "install: the frozen clock follows the restored now (was 5000)");
    mt1::save::restored(&mut g);
    let s = p1(&g);
    assert_eq!((s.clock, s.encounter.is_none(), s.acting.is_none(), g.squad.as_ref().unwrap().encounter), (Some(3000), true, true, false), "restore: no stale frozen clock, no fight state");
    assert!(s.turns.iter().all(|t| !t.moved && !t.attacked && !t.done), "fresh turns");
    assert_eq!(round::clock(&mut g), Some(3000 + 50.min(mt_engine::sim::sched::SECOND)), "the next pass runs on from the restored now (the pending backlog drains as before)");
}
#[test]
fn a_square_is_walked_on_arriving_on_a_new_whole_tile_on_foot_and_in_a_vehicle() {
    assert_eq!(round::step((66, 8, 6), (66, 6, 6), -1), Step::Walked, "on foot: a whole tile west");
    assert_eq!(round::step((66, 8, 6), (66, 8, 6), -1), Step::Refused, "on foot: the bump out and back");
    assert_eq!(round::step((66, 8, 7), (66, 8, 6), -1), Step::Nothing, "a turn in place");
    assert_eq!(round::step((66, 8, 6), (66, 8, 6), 10), Step::Nothing, "a tile event fired");
    assert_eq!(round::step((66, 8, 6), (66, 7, 6), -1), Step::Nothing, "in a vehicle: the first half tile");
    assert_eq!(round::step((66, 7, 6), (66, 6, 6), -1), Step::Walked, "in a vehicle: arriving on the whole tile (before >> 1 == after >> 1 was the old rule's blind spot)");
    assert_eq!(round::step((64, 8, 0), (63, 8, 0), -1), Step::Nothing); assert_eq!(round::step((63, 8, 0), (62, 8, 0), -1), Step::Walked, "north in a vehicle");
}
#[test]
fn the_automatic_regroup_leaves_party_fired_and_willing() {
    let mut g = ground(); place(&mut g, 14, (22, 9), 2, 4); place(&mut g, 15, (22, 10), 0, 4); fight_def(&mut g, Surprise::None);
    round::pass(&mut g, &[]); assert!(round::in_encounter(&g));
    g.mem.set_party_fired(1); g.mem.write(Mem::npc(14).add(0x26), 1, 1);
    assert!(round::willing(&g, 14));
    kill(&mut g, 15); g.mem.write(Mem::npc(14).add(N_Y), 100, 2);
    for _ in 0..8 { if !round::in_encounter(&g) { break } orders::intent(&mut g, Intent::EndTurn); }
    assert!(!round::in_encounter(&g) && g.mem.combat() == 0, "{:?}", p1(&g).log);
    assert_eq!((g.mem.party_fired(), round::willing(&g, 14)), (1, true), "the fight's end changes neither");
}
#[test]
fn the_regroup_point_is_a_conscious_traveller() {
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "l".into(), surprise: Surprise::None, end: End::Locked { escape: None }, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    assert!(g.mem.combat() != 0 && round::selected(&g) == 0);
    g.mem.write(Mem::walker(0).add(W_X), 40, 2);
    g.mem.set_party_u8(0x127, 0);
    assert!(!round::conscious(&g, 0) && round::conscious(&g, 1));
    let trav2 = round::traveller_cell(&g, 1); assert_ne!(trav2, (20, 5));
    kill(&mut g, 14); round::pass(&mut g, &[]);
    assert_eq!((g.mem.combat(), p1(&g).strip.as_str()), (0, "Victory"), "{:?}", p1(&g).log);
    assert_eq!((g.mem.x() >> 1, g.mem.y() >> 1), trav2, "the figure stands where Trav2 (the marching order's first conscious) stood, not on Trav1's square (20, 5)");
}
#[test]
fn a_surprised_round_opens_on_the_marching_head_without_an_npc_attack() {
    let mut still_surprised = false;
    for seed in 1..=24u32 {
        let mut g = ground(); place(&mut g, 14, (22, 12), 0, 4); for c in 8..=10 { wall(&mut g, (22, c)) }
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "s".into(), start: Start::Area(Rect { r0: 20, c0: 1, r1: 24, c1: 12 }), boundary: Boundary::Level, surprise: Surprise::Attackers, attacker: Side::Npcs, ..Def::default_sight() });
        g.rand.lcg.seed = seed;
        round::pass(&mut g, &[]);
        assert_eq!(encounter(&g).map(|e| e.surprised), Some(Some(Side::Party)), "{:?}", p1(&g).log);
        round::pass(&mut g, &[Intent::Select(4)]);
        let mut prompts = Vec::new();
        for _ in 0..16 {
            let Some(e) = encounter(&g) else { break }; if e.round > 2 { break }
            if e.choosing { round::key(&mut g, KEY_SPACE); continue }
            if e.side != Side::Party { round::pass(&mut g, &[]); continue }
            prompts.push((e.round, round::selected(&g), e.surprised.is_some())); round::pass(&mut g, &[Intent::EndTurn]);
        }
        let r2 = prompts.iter().find(|p| p.0 == 2).copied().expect("round 2 reached");
        assert_eq!(r2.1, 0, "seed {seed}: round 2 opens on the marching head: {prompts:?}\n{:?}", p1(&g).log);
        still_surprised |= r2.2;
    }
    assert!(still_surprised, "some seed keeps the party surprised into round 2 (the alarm roll fails)");
}

#[test]
fn table_task_roll_pm_p9() {
    assert_eq!([Difficulty::Simple, Difficulty::Routine, Difficulty::Difficult, Difficulty::Formidable, Difficulty::Impossible].map(|d| d.target()), [3, 7, 11, 15, 19]);
    let t = rules::Task { difficulty: Difficulty::Routine, target: 7, dm: 2 };
    let o = rules::resolve(&t, (2, 3)); assert_eq!((o.total, o.hit, o.level), (7, true, Some(Level::Marginal)), "2D 5 + 2 = 7 vs 7: a marginal success (PM p.70)");
    let o = rules::resolve(&t, (1, 1)); assert!(o.fumble && !o.hit, "natural 2 fails regardless of DMs (PM p.9)");
    let o = rules::resolve(&t, (1, 2)); assert_eq!((o.hit, o.exceptional_failure), (false, true), "5 vs 7: short by 2 = exceptional failure (RM p.13)");
    assert_eq!(rules::char_dm(7), 1, "PM p.9: Dex 7-9 = +1");
    assert_eq!(rules::hit_task(&rules::Fire { skill: 9, dex: 15, tactics: 3, ..Default::default() }).unwrap().dm, 8, "PM p.67: DMs capped at +8");
}
#[test]
fn table_to_hit_pm_p72() {
    let t = |profile, band, braced| rules::difficulty(profile, band, braced).map(|d| d.target());
    assert_eq!((t(Profile::Handgun, Band::Close, false), t(Profile::Handgun, Band::Short, false), t(Profile::Handgun, Band::Medium, false), t(Profile::Handgun, Band::Long, false)), (Some(3), Some(7), Some(11), Some(15)), "Handgun: Simple / Routine / Difficult / Formidable");
    assert_eq!((t(Profile::Rifle, Band::Short, false), t(Profile::Rifle, Band::Medium, false), t(Profile::Rifle, Band::Long, false), t(Profile::Rifle, Band::Distant, false)), (Some(7), Some(11), Some(11), Some(19)), "Rifle: Routine / Difficult / Difficult / Impossible");
    assert_eq!(t(Profile::Rifle, Band::Medium, true), Some(7), "Gyrostable (the ACR, PM p.76 **): Routine at medium (Errata PM p.72)");
    assert_eq!((rules::band(2), rules::band(3), rules::band(29), rules::band(30)), (Band::Short, Band::Medium, Band::Medium, Band::Long), "PM p.74: short 1-2, medium 3-29, long 30-149 squares");
    let f = rules::Fire { profile: Profile::Handgun, band: Band::Medium, skill: 1, dex: 8, movement: rules::movement_dm(1, false, 0, false, Band::Medium), ..Default::default() };
    let task = rules::hit_task(&f).unwrap();
    assert_eq!((task.target, task.dm), (11, 2), "a 9mm auto pistol at a walking target 10 squares off: Difficult 11+, skill 1 + Dex +1, the target's walk scaled 50% drops to 0 (PM p.68 errata guide)");
    assert_eq!(rules::movement_dm(3, false, 4, false, Band::Short), -7, "PM p.68 worked example: attacker speed 4 vs target speed 3 at short = -7");
    assert_eq!(rules::hit_task(&rules::Fire { unskilled: true, ..f }).unwrap().target, 15, "no skill: one level harder (RM p.15, R-4)");
}
#[test]
fn table_damage_pm_p70_errata() {
    let (pistol, rifle) = (&WEAPONS_PM[4], &WEAPONS_PM[7]);
    assert_eq!((pistol.pen, pistol.dmg, rifle.pen, rifle.dmg, rifle.rds), (2, 3, 4, 3, 20), "PM p.76 + Errata PM p.76");
    assert_eq!(rules::damage(4, 0, true, Level::Normal, 3), 3, "High class (Pen >= 2 AV, AV 0): 100% at +1");
    assert_eq!(rules::damage(4, 0, true, Level::Marginal, 3), 1, "High class, marginal: 50%, fractions dropped");
    assert_eq!(rules::damage(4, 0, true, Level::X8, 3), 24, "beat by 8: x8 (R-5: a 24-point hit)");
    assert_eq!(rules::damage(2, 2, true, Level::Normal, 3), 1, "Low class (Pen = AV mesh 2): 50%");
    assert_eq!(rules::damage(2, 5, true, Level::X2, 3), 1, "Zero class (Pen 2 < cloth AV 5), exposed areas: 1 fixed point at +2");
    assert_eq!(rules::damage(2, 5, false, Level::X2, 3), 1, "enclosed and Pen >= AV / 10: the fixed point still lands");
    assert_eq!(rules::damage(0, 10, false, Level::X8, 3), 0, "enclosed, Pen below AV / 10: nothing");
    assert_eq!((rules::pen_at(4, Some(2), Band::Medium), rules::pen_at(4, Some(2), Band::Long)), (4, 2), "attenuation /2: full to medium, half at long (Errata PM p.70)");
    assert_eq!(rules::life_force(7, 7, 7), (3, 5), "PM p.66 Life Force 21 = 3/5");
}
#[test]
fn table_grenade_b35_on_and_off() {
    let on = Options { grenade_range: true, npc_selective_autofire: true }; let off = Options { grenade_range: false, ..on };
    let target = |o: Options, squares| rules::difficulty(Profile::Thrown, o.throw_band(squares), false).unwrap().target();
    assert_eq!((target(off, 1), target(off, 2), target(off, 3), target(off, 5), target(off, 6), target(off, 29)), (11, 11, 15, 15, 15, 15), "OFF = the printed rows: Short Difficult 11+, Medium Formidable 15+");
    assert_eq!((target(on, 1), target(on, 2), target(on, 3), target(on, 5), target(on, 6), target(on, 29)), (11, 11, 11, 11, 15, 15), "ON: 3-5 squares use the Short row; 1-2 and 6+ as printed");
    assert_eq!(rules::difficulty(Profile::Thrown, Band::Long, false), Some(Difficulty::Impossible), "PM p.72: Thrown at long = Impossible; PM p.78: a grenade's maximum range is medium");
    assert_eq!((GRENADES_PM[0].max, GRENADES_PM[0].danger_dm, rules::danger_radius(GRENADES_PM[0].danger_dm)), (Band::Medium, 30, 1), "HE grenade: 3 m danger space = the target and its 8 neighbours");
    assert_eq!(rules::danger_pen(9, 1), 4, "half Pen one square out (PM p.72-73)");
    let mut g = ground(); g.mem.set_party_u8(0x116, 8); g.mem.set_party_u8(0x117, 8); g.mem.set_party_u8(0x1CB, 1); g.mem.write(Mem::walker(0).add(mt1::ground::state::W_BUILTIN), 1, 1);
    for (opt, want) in [(off, 15), (on, 11)] { g.balance = opt; let s = mt1::ground::fire::throw_check(&g, 0, (22, 9)).unwrap(); assert_eq!((s.task.target, s.task.dm), (want, 2), "4 squares: {:?}", s.describe()) }
}
#[test]
fn table_autofire_b36_on_and_off() {
    let acr = &WEAPONS_PM[14]; assert_eq!((acr.auto, rules::rounds_per_burst(acr.auto)), (2, 4), "ACR: 2 targets, 4 rounds a burst (Errata PM p.76, PM p.92)");
    let smg = &WEAPONS_PM[12]; assert_eq!((smg.auto, rules::rounds_per_burst(smg.auto), smg.rapid), (3, 10, true), "SMG: 3 targets, 10 rounds, /R");
    assert_eq!(WEAPONS_PM[7].auto, 0, "the 9mm rifle is semi-automatic");
    let on = Options { grenade_range: true, npc_selective_autofire: true }; let off = Options { npc_selective_autofire: false, ..on };
    assert!(on.npc_bursts(2, 20, 5, false) && !on.npc_bursts(2, 20, 6, false) && on.npc_bursts(2, 20, 25, true), "ON: 5 squares yes, 6 no, boxed yes");
    assert!(off.npc_bursts(2, 20, 6, false) && off.npc_bursts(2, 20, 25, false), "OFF: a burst whenever two rounds remain (O-6)");
    assert!(!on.npc_bursts(0, 20, 3, true) && !off.npc_bursts(2, 1, 3, true), "no Autofire row, or one round left: never");
    for (opt, want) in [(on, false), (off, true)] {
        let mut g = ground(); g.balance = opt; g.mem.set_view_y(8);
        place(&mut g, 14, (22, 13), 0, 14);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "a".into(), surprise: Surprise::None, ..Def::default_sight() });
        round::pass(&mut g, &[]); round::pass(&mut g, &[Intent::EndTurn]);
        let burst = p1(&g).log.iter().any(|l| l.contains("(NPC 14) fires") && l.contains("(burst, 4 rounds)"));
        assert_eq!(burst, want, "selective {}: {:?}", opt.npc_selective_autofire, p1(&g).log);
    }
    let mut g = ground(); place(&mut g, 14, (22, 9), 0, 4); place(&mut g, 15, (23, 9), 0, 4);
    arm(&mut g, 0, 14, 20);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "b".into(), surprise: Surprise::None, ..Def::default_sight() });
    round::pass(&mut g, &[]);
    mt1::ground::fire::attack(&mut g, 14, mt1::ground::fire::Opts { burst: true, ..Default::default() });
    assert!(log_has(&g, "(burst, 4 rounds)") && log_has(&g, "burst on thug (NPC 15)"), "{:?}", p1(&g).log);
    assert_eq!(g.mem.party_u8(0xA82 + 2), 16, "4 rounds spent (PM p.92)");
}
#[test]
fn fixture_sanity() {
    let g = ground();
    assert_eq!(round::travellers(&g), vec![0, 1, 2, 3, 4]); assert_eq!(round::marching_order(&g), vec![0, 1, 2, 3, 4]);
    assert_eq!(round::traveller_cell(&g, 0), (22, 5)); assert_eq!(Turn::default().mp, 0); assert_eq!(MOVE_WALK, 10);
    let f = adapters::traveller(&g, 0, 0); assert_eq!(f.pool, 21);
    let _ = NpcSt::default();
}

#[test]
fn an_npc_shot_at_the_grouped_party_is_one_roll() {
    for seed in 1..40u32 {
        let mut g = ground(); place(&mut g, 14, (24, 7), 0, 4);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, auto_break: false, ..Def::default_sight() });
        round::pass(&mut g, &[]); assert!(encounter(&g).is_some_and(|e| e.grouped), "{:?}", p1(&g).log);
        g.rand.lcg.seed = seed;
        round::end_turn(&mut g);
        assert!(log_has(&g, "(NPC 14) fires the"), "seed {seed}: {:?}", p1(&g).log);
        assert!(!log_has(&g, "in the line:"), "seed {seed}: a second roll on the same figure: {:?}", p1(&g).log);
    }
}
#[test]
fn the_shot_that_starts_a_fight_spends_the_attack_when_the_party_wins_initiative() {
    let mut g = ground(); place(&mut g, 14, (22, 20), 0, 4); arm(&mut g, 0, 14, 20);
    g.mem.set_party_u8(0x12B + adapters::SKILL_TACTICS as u16, 1);
    g.squad.as_mut().unwrap().p1.defs.push(Def { id: "f".into(), surprise: Surprise::None, boundary: Boundary::Level, ..Def::default_sight() });
    orders::fire(&mut g, 14);
    let e = encounter(&g).expect("the shot started the fight");
    assert!(e.choosing, "the party won the initiative: {:?}", p1(&g).log);
    assert!(p1(&g).turns[0].attacked, "Trav1's Attack is spent: {:?}", p1(&g).log);
    round::key(&mut g, KEY_SPACE);
    let shots = p1(&g).log.iter().filter(|l| l.contains("Trav1 fires the")).count();
    orders::fire(&mut g, 14);
    assert_eq!(p1(&g).log.iter().filter(|l| l.contains("Trav1 fires the")).count(), shots, "no second attack in round 1: {:?}", p1(&g).log);
}
#[test]
fn a_non_fatal_hit_on_an_npc_queues_the_impact_sound() {
    let mut g = ground(); place(&mut g, 14, (24, 7), 0, 4);
    let pool = squad::npc_pool(&mut g, 14).unwrap(); assert!(pool > 1);
    assert_eq!(g.mem.extra_sound(), 0);
    assert!(!fire::apply(&mut g, Unit::Npc(14), 1, 2, 0)); assert_eq!((g.mem.extra_sound(), g.mem.impact_sound()), (6, 0));
    g.mem.set_extra_sound(0);
    assert!(fire::apply(&mut g, Unit::Npc(14), 99, 2, 0)); assert_eq!((g.mem.extra_sound(), g.mem.impact_sound()), (0, 1));
}
#[test]
fn an_npc_killed_by_its_own_blast_does_not_act() {
    let mut killed = 0;
    for seed in 1..40u32 {
        let mut g = ground(); place(&mut g, 14, (22, 20), 0, 4);
        g.squad.as_mut().unwrap().p1.defs.push(Def { id: "far".into(), start: Start::Area(Rect { r0: 22, c0: 5, r1: 22, c1: 5 }), boundary: Boundary::Level, surprise: Surprise::None, ..Def::default_sight() });
        round::pass(&mut g, &[]); assert!(encounter(&g).is_some());
        squad::npc_pool(&mut g, 14); g.squad.as_mut().unwrap().pools.insert(14, 1);
        let d = Difficulty::Simple; let task = rules::Task { difficulty: d, target: d.target(), dm: 8 };
        g.squad.as_mut().unwrap().fire.pending.push(fire::Pending { who: Unit::Npc(14), cell: (22, 20), weapon: 17, task, round: 0, launcher: true });
        g.rand.lcg.seed = seed;
        round::end_turn(&mut g);
        assert!(log_has(&g, "4cm RAM GL-10 lands at (22, 20)"), "seed {seed}: {:?}", p1(&g).log);
        if !log_has(&g, "(NPC 14) is dead") { continue }
        killed += 1;
        assert!(!log_has(&g, "(NPC 14) fires the") && !log_has(&g, "(NPC 14) closes in") && !log_has(&g, "(NPC 14) minds"), "seed {seed}: the dead NPC acted: {:?}", p1(&g).log);
        assert_eq!(g.mem.read(Mem::npc(14).add(0x1E), 1), 50, "no round spent");
    }
    assert!(killed > 0, "no seed killed the NPC with its own round");
}
#[test]
fn rapid_fire_with_every_target_refused_spends_nothing() {
    let mut g = ground(); place(&mut g, 14, (24, 7), 0, 4); place(&mut g, 15, (25, 9), 0, 4); arm(&mut g, 0, 12, 30); fight_def(&mut g, Surprise::None);
    round::pass(&mut g, &[]); assert!(encounter(&g).is_some());
    fire::shoot(&mut g, 14, fire::Opts { rapid: true, ..Default::default() });
    assert_eq!(g.squad.as_ref().unwrap().fire.rapid, Some(vec![14]), "{}", g.squad.as_ref().unwrap().orders.hover);
    kill(&mut g, 14);
    assert!(fire::rapid_enter(&mut g));
    let s = g.squad.as_ref().unwrap();
    assert_eq!((g.mem.party_u8(0xA82 + 2), s.fire.reloading[0], s.p1.turns[0].attacked, s.fire.rapid.clone()), (30, false, false, None), "{:?}", s.p1.log);
    assert!(!log_has(&g, "clip is empty") && s.orders.hover.starts_with("Rapid fire: "), "{}", s.orders.hover);
}
#[test]
fn a_selected_value_past_the_party_does_not_panic() {
    let mut g = ground(); place(&mut g, 14, (24, 7), 0, 4); fight_def(&mut g, Surprise::None);
    round::pass(&mut g, &[]); assert!(encounter(&g).is_some());
    g.mem.set_selected_char(7); g.mem.set_leader(7);
    assert_eq!(round::selected(&g), 4);
    round::key(&mut g, 0x48); round::walked(&mut g, 1); round::hold(&mut g); round::order(&mut g, Order::Move); round::end_turn(&mut g);
}
