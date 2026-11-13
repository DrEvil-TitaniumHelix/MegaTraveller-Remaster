#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use megatraveller::scenario;
use mt1::{ground::{self, commands, main_loop, orders::{self, Speed}, round::{self, Side, KEY_SPACE}, Ground}, ui::{Intent, Target, Unit, Verb}};
use mt_engine::{io::Scripted, sim::tick::Counter};
use std::path::{Path, PathBuf};

fn files() -> Option<PathBuf> { oracle_env::gamefiles() }
fn quickstart(dir: &Path) -> Ground {
    let g = scenario_ground(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m4c_efate_ambush.toml"), dir);
    assert!(g.squad.as_ref().unwrap().p1.defs.iter().any(|d| d.id == "efate_quickstart_ambush"), "the level's encounter table is installed");
    assert_eq!(party_cell(&g), (22, 5));
    g
}
fn scenario_ground(path: &Path, dir: &Path) -> Ground {
    let prepared = scenario::load(path, dir).unwrap();
    let mut g = prepared.game;
    scenario::attach_scene(&mut g); g.clock = Box::new(Counter::at(1234)); g.input = Box::new(Scripted::new([]));
    let compendium = mt1::ground::rules::compendium::Compendium::parse(&std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/npcs.toml")).unwrap()).unwrap();
    g.squad = Some(mt1::ground::squad::State::with_compendium(compendium));
    g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), dir.into(), prepared.extensions)));
    g.tool_poll();
    g
}
#[allow(dead_code)]
fn cell_bytes(g: &Ground, r: i16, c: i16) -> (u8, u8, u8) { let p = commands::cell(g, r, c); (g.mem.read(p, 1) as u8, g.mem.read(p.add(1), 1) as u8, g.mem.read(p.add(2), 1) as u8) }
#[allow(dead_code)]
fn glyph(g: &Ground, r: i16, c: i16) -> char {
    let (f, t, occ) = cell_bytes(g, r, c);
    if occ & 0x3F != 0 { return char::from_digit((occ & 0x3F) as u32 % 10, 10).unwrap() }
    if occ & 0x40 != 0 { return 'E' }
    let blocked = (f & 1 == 0 && (t <= 14 || t == 38 || t == 39)) || (f & 1 != 0 && t <= 4);
    if blocked { '#' } else if (f & 1 != 0 && t / 20 < 5) || t / 20 < 2 { '.' } else { '#' }
}
fn party_cell(g: &Ground) -> (i16, i16) { (g.mem.x() >> 1, g.mem.y() >> 1) }
fn encounter_id(g: &Ground) -> Option<String> { g.squad.as_ref().unwrap().p1.encounter.as_ref().map(|e| e.def.clone()) }
fn npcs(g: &Ground) -> String {
    (1..=g.mem.npc_count()).filter(|&k| round::npc_present(g, k)).map(|k| { let n = ground::state::Mem::npc(k as i16); format!("{k}@{:?} att{} hp{} lvl{} w{}", round::npc_cell(g, k), g.mem.read(n.add(13), 1) & 3, g.mem.read(n.add(10), 1), g.mem.read(n.add(11), 1), g.mem.read(n.add(31), 1)) }).collect::<Vec<_>>().join("  ")
}
fn fight(g: &mut Ground, max: usize) -> Vec<(u16, u8)> {
    let mut turns = Vec::new();
    for _ in 0..max {
        let Some(e) = g.squad.as_ref().unwrap().p1.encounter.clone() else { break };
        if e.choosing { round::key(g, KEY_SPACE); continue }
        assert_eq!(e.side, Side::Party, "the engine returns on the party's turn");
        let who = round::selected(g);
        turns.push((e.round, who));
        let target = (1..=g.mem.npc_count()).find(|&k| orders::attackable(g, k) && orders::fire_check(g, who, k).is_ok());
        let intent = match target { Some(k) => Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(k)) }, None => Intent::EndTurn };
        round::pass(g, &[intent]);
        if g.squad.as_ref().unwrap().p1.turns[who as usize].done == false && g.squad.as_ref().unwrap().p1.encounter.as_ref().is_some_and(|e| e.side == Side::Party && !e.choosing) { round::pass(g, &[Intent::EndTurn]) }
    }
    turns
}
fn tail(g: &Ground, n: usize) -> String { g.squad.as_ref().unwrap().p1.log.iter().rev().take(n).rev().cloned().collect::<Vec<_>>().join("\n") }

fn walk(g: &mut Ground, wp: (i16, i16)) {
    orders::move_to(g, wp, Speed::Auto);
    for _ in 0..3 { round::pass(g, &[]) }
    assert_eq!(party_cell(g), wp, "the leg completes: {}", tail(g, 3));
    assert!(encounter_id(g).is_none(), "nothing engages on the way: {}", tail(g, 4));
}
const ROAD: [(i16, i16); 3] = [(23, 19), (29, 19), (33, 19)];
fn drive(g: &mut Ground, picks: &'static [&'static str]) -> Vec<(mt_engine::play::Kind, Option<mt_engine::play::Grant>)> {
    use mt_engine::{io::Input, play::{Choice, Kind, Play}};
    use std::{cell::RefCell, panic::{catch_unwind, AssertUnwindSafe}, rc::Rc};
    #[derive(Debug)] struct Stop;
    let out = Rc::new(RefCell::new(Vec::new())); let seen = out.clone(); let mut n = 0;
    let mut play = Play::with_service(move |p| {
        let d = p.pending_public().unwrap();
        seen.borrow_mut().push((d.kind, p.journal().entries.last().map(|e| e.grant.clone())));
        if d.kind != Kind::Ground || n >= picks.len() { std::panic::resume_unwind(Box::new(Stop)) }
        p.choose(Choice::new(d.id, picks[n])).unwrap(); n += 1;
    });
    play.flush(); g.input = Box::new(play);
    let e = catch_unwind(AssertUnwindSafe(|| main_loop::run(g))).expect_err("the session stops"); if !e.is::<Stop>() { std::panic::resume_unwind(e) }
    g.input = Box::new(Scripted::new([]));
    let r = out.borrow().clone(); r
}
const GATE: ((i16, i16), (i16, i16)) = ((34, 17), (34, 14));
#[test]
fn quickstart_ambush_then_the_walk_to_the_starport_door() {
    let Some(dir) = files() else { return };
    let mut g = quickstart(&dir);
    assert_eq!(round::npc_cell(&g, 32), (18, 49), "Szen Sanai's W.DAT home"); assert_eq!(round::npc_cell(&g, 12), (3, 26), "pirate 12's W.DAT home");
    round::pass(&mut g, &[]);
    assert_eq!(encounter_id(&g).as_deref(), Some("efate_quickstart_ambush"), "the area trigger starts the ambush");
    let mut side = round::npc_side(&g); side.sort(); assert_eq!(side, vec![14, 15, 16], "boundary = area: the three of 1990 only");
    let turns = fight(&mut g, 400);
    assert!(encounter_id(&g).is_none(), "the ambush ended: {}", tail(&g, 6));
    assert!(g.squad.as_ref().unwrap().p1.log.iter().any(|l| l == "Victory"), "{}", tail(&g, 6));
    for r in 1..=turns.last().map_or(0, |t| t.0) { let mut v: Vec<u8> = turns.iter().filter(|t| t.0 == r).map(|t| t.1).collect(); let n = v.len(); v.sort(); v.dedup(); assert_eq!(n, v.len(), "F8: a Traveller acted twice in round {r}: {turns:?}") }
    assert_eq!(g.mem.combat(), 0, "F6: the party is whole after the fight");
    for wp in ROAD.into_iter().chain([GATE.0]) { walk(&mut g, wp) }
    assert!(orders::find_path(&g, 0, GATE.0, GATE.1).is_some_and(|p| p.contains(&(34, 16)) && p.contains(&(34, 15))), "the click-walk routes through the gate tiles");
    walk(&mut g, GATE.1);
    for wp in [(34, 5), (33, 4)] { walk(&mut g, wp) }
    let door = drive(&mut g, &["move:west:10"]);
    eprintln!("=== door: party {:?} event {} decisions {door:?}", party_cell(&g), g.mem.event());
    assert_eq!(g.mem.event(), 10, "the starport door event (event 10 at [33, 3]) fired: party at {:?}", party_cell(&g));
    assert!(door.last().is_some_and(|(k, _)| !k.is_command()), "the door's blocking decision cut the grant: {door:?}");
    assert!(encounter_id(&g).is_none() && round::alerted_count(&g) == 0, "nobody engaged on the way: {}", tail(&g, 4));
}
#[test]
fn quickstart_ambush_then_the_walk_to_the_launch_pad() {
    let Some(dir) = files() else { return };
    let mut g = quickstart(&dir);
    round::pass(&mut g, &[]); fight(&mut g, 400); assert!(encounter_id(&g).is_none() && g.mem.combat() == 0);
    for wp in ROAD.into_iter().chain([GATE.0, GATE.1, (34, 8), (32, 9)]) { walk(&mut g, wp) }
    let pad = drive(&mut g, &["move:north:10"]);
    eprintln!("=== launch: party {:?} event {} launch_pending {} decisions {pad:?}", party_cell(&g), g.mem.event(), g.launch_pending);
    assert_eq!(pad.last().map(|(k, _)| *k), Some(mt_engine::play::Kind::Space), "the launch pad (event 8 at [31, 9]) took the party into space: event {} {pad:?}", g.mem.event());
    assert!(encounter_id(&g).is_none(), "nobody engaged on the way: {}", tail(&g, 4));
}
#[test]
fn louzy_warehouse_every_participant_takes_a_turn_or_is_reported_out_of_reach() {
    let Some(dir) = files() else { return };
    let toml = "[meta]\nformat = 1\nname = \"louzy_warehouse_probe\"\nversion = \"1\"\nauthor = \"DrEvil\"\npurpose = \"test\"\nrow = \"warehouse probe\"\n\n[place]\nworld = 3\narea = 0\nlevel = 1\ncell = [18, 10]\noutdoors = false\n";
    let toml = format!("{toml}\n[party]\nitems = [{{ slot = 0, replace = false, pack = [{{ id = 52, name = \"electric torch\", qty = 1, charge = 1 }}] }}]\n");
    let path = std::env::temp_dir().join(format!("louzy_warehouse_{}.toml", std::process::id())); std::fs::write(&path, toml).unwrap();
    let mut g = scenario_ground(&path, &dir); let _ = std::fs::remove_file(&path);
    let def = g.squad.as_ref().unwrap().p1.def("louzy_warehouse").cloned().expect("the level's encounter table is installed");
    assert!(matches!(def.boundary, round::Boundary::Radius(29)), "no boundary line: the data default, radius 29: {:?}", def.boundary);
    assert_eq!(party_cell(&g), (18, 10)); eprintln!("=== louzy: {}", npcs(&g));
    round::pass(&mut g, &[]);
    assert_eq!(encounter_id(&g).as_deref(), Some("louzy_warehouse"), "a guard in the room sees the party: {}", tail(&g, 4));
    for _ in 0..80 {
        let Some(e) = g.squad.as_ref().unwrap().p1.encounter.clone() else { break }; if e.round > 3 { break }
        if e.choosing { round::key(&mut g, KEY_SPACE); continue }
        if e.side == Side::Party { round::pass(&mut g, &[Intent::EndTurn]) } else { round::pass(&mut g, &[]) }
    }
    let log = g.squad.as_ref().unwrap().p1.log.clone();
    for k in 15..=28u8 {
        let acted = log.iter().any(|l| l.find(&format!("(NPC {k})")).is_some_and(|i| !l[..i].contains(" at ")));
        let reported = log.iter().any(|l| l.contains("out of reach:") && l.split(|c: char| !c.is_ascii_digit()).any(|w| w == k.to_string()));
        assert!(acted || reported, "NPC {k} neither took a turn nor was reported out of reach:\n{}", log.join("\n"));
    }
    eprintln!("=== louzy rounds 1-3:\n{}", log.join("\n"));
}
#[test]
fn main_loop_reports_the_frozen_clock_at_every_decision() {
    let Some(dir) = files() else { return };
    let mut g = quickstart(&dir); round::pass(&mut g, &[]); fight(&mut g, 400); assert!(encounter_id(&g).is_none());
    let mut last = 0;
    const PICKS: [&[&str]; 5] = [&["move:east:5"], &["wait:5"], &["move:east:5"], &["wait:20"], &["move:west:9"]];
    for picks in PICKS { let pick = picks[0];
        drive(&mut g, picks);
        let (now, frozen) = (g.clock.now(), round::frozen(&g).unwrap());
        assert_eq!(now, frozen, "after {pick}: the decision stands on the frozen clock");
        assert!(now >= last, "after {pick}: the clock ran backwards: {now} < {last}"); last = now;
    }
}
#[test]
fn a_held_key_grant_after_a_click_walk_still_walks() {
    let Some(dir) = files() else { return };
    let mut g = quickstart(&dir); round::pass(&mut g, &[]); fight(&mut g, 400); assert!(encounter_id(&g).is_none() && g.mem.combat() == 0);
    walk(&mut g, (23, 19)); let pending = g.squad.as_ref().unwrap().p1.pending; assert!(pending >= 100, "the click-walk's backlog: {pending}");
    let before = party_cell(&g);
    let d = drive(&mut g, &["move:south:40"]);
    let grant = d.last().and_then(|(_, g)| g.clone()).unwrap();
    assert_eq!(grant.used, 40, "{grant:?}");
    assert!(party_cell(&g).0 >= before.0 + 3, "40 polls of a held key walk 3+ squares south: {before:?} -> {:?} ({grant:?})", party_cell(&g));
}
#[test]
fn a_refused_step_ends_the_ground_move_grant() {
    use mt_engine::{io::Input, play::{Choice, Kind, Play}};
    use std::{cell::RefCell, panic::{catch_unwind, AssertUnwindSafe}, rc::Rc};
    #[derive(Debug)] struct Stop;
    let Some(dir) = files() else { return };
    let mut g = quickstart(&dir);
    round::pass(&mut g, &[]); fight(&mut g, 400); assert!(encounter_id(&g).is_none() && g.mem.combat() == 0);
    assert_eq!(party_cell(&g), (22, 5));
    let out = Rc::new(RefCell::new(None)); let seen = out.clone();
    let mut play = Play::with_service(move |p| {
        let d = p.pending_public().unwrap();
        if d.id == "d:1" { assert_eq!(d.kind, Kind::Ground); p.choose(Choice::new(d.id, "move:east:400")).unwrap() }
        else { *seen.borrow_mut() = Some((p.journal().entries[0].grant.clone(), d.kind)); std::panic::resume_unwind(Box::new(Stop)) }
    });
    play.flush(); g.input = Box::new(play);
    let e = catch_unwind(AssertUnwindSafe(|| main_loop::run(&mut g))).expect_err("stops at d:2"); if !e.is::<Stop>() { std::panic::resume_unwind(e) }
    let (grant, kind) = out.borrow().clone().unwrap();
    eprintln!("=== grant {grant:?} kind {kind:?} party {:?}", party_cell(&g));
    assert_eq!(grant.requested, 400); assert!(grant.used < 100 && grant.remaining > 300, "the refused step cut the grant: {grant:?}");
    assert_eq!(party_cell(&g), (22, 10), "walked east to the building at column 11 and stopped there");
}

#[test]
fn zz_skip_report() { oracle_env::report() }
