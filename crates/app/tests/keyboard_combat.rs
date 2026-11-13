use megatraveller::{panels::{keyboard, UiState}, scenario};
use mt1::{ground::{orders, round, Ground, state::{Mem, W_TARGET}}, ui::{Intent, Scene, ScreenKind, Target, Unit}};
use mt_engine::{io::{keys, scan, Input, Scripted}, play::{Choice, Play}, sim::tick::Counter};
use std::path::{Path, PathBuf};

fn game() -> Ground {
    let dir = PathBuf::from(std::env::var_os("MT1_GAMEFILES").expect("MT1_GAMEFILES is required"));
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let prepared = scenario::load(&root.join("scenarios/efate_broken.toml"), &dir).unwrap();
    let mut g = prepared.game;
    scenario::attach_scene(&mut g); g.clock = Box::new(Counter::at(1234)); g.input = Box::new(Scripted::new([]));
    let compendium = mt1::ground::rules::compendium::Compendium::parse(&std::fs::read_to_string(root.join("data/npcs.toml")).unwrap()).unwrap();
    g.squad = Some(mt1::ground::squad::State::with_compendium(compendium));
    g.tooling = Some(Box::new(scenario::Loader::new(Default::default(), dir, prepared.extensions)));
    g.tool_poll(); round::pass(&mut g, &[]); g
}

#[test]
fn tab_and_fire_refuse_offscreen_targets_without_starting_a_fight() {
    let mut g = game();
    g.squad.as_mut().unwrap().p1.encounter = None;
    let offscreen = 14;
    g.mem.set_view_y(40);
    assert!(!round::on_screen(&g, round::npc_cell(&g, offscreen)));
    for _ in 0..10 {
        assert!(round::key(&mut g, scan::TAB));
        let k = target(&g);
        assert!(k == 0 || round::on_screen(&g, round::npc_cell(&g, k)), "Tab named off-screen NPC {k}");
    }
    orders::set_target(&mut g, offscreen);
    let party = g.mem.party().to_vec();
    assert!(round::key(&mut g, 0x21));
    assert!(!round::in_encounter(&g), "refused F must not start a fight");
    assert_eq!(g.mem.party(), party);
    assert!(g.squad.as_ref().unwrap().orders.hover.contains("outside the screen"));
    orders::intent(&mut g, Intent::OpenMenu { target: Target::Unit(Unit::Npc(offscreen)), at: (0, 0) });
    let fire = g.squad.as_ref().unwrap().orders.menu.as_ref().unwrap().items.iter().position(|i| i.key == b'F').unwrap();
    orders::intent(&mut g, Intent::MenuPick(fire as u8));
    assert!(!round::in_encounter(&g)); assert_eq!(g.mem.party(), party);
    assert!(g.squad.as_ref().unwrap().orders.hover.contains("outside the screen"));
    g.squad = None; assert!(!round::key(&mut g, scan::TAB)); assert!(!round::key(&mut g, 0x21));
}
fn target(g: &Ground) -> u8 { g.mem.read(Mem::walker(round::selected(g) as i16).add(W_TARGET), 1) as u8 }
#[test]
fn select_after_reload_keeps_ammunition_and_turn_costs() {
    let (mut g, mut ui) = (game(), UiState::default());
    let hand = g.mem.read(Mem::walker(0).add(mt1::ground::state::W_HAND), 1) as i32 + 21;
    let weapon = mt1::ground::walk::item_ptr(&g, 0, hand);
    let magellan = (g.mem.read(weapon.add(1), 1), g.mem.read(weapon.add(2), 1));
    key(&mut g, &mut ui, b'3' as u16);
    assert_eq!(round::selected(&g), 2);
    key(&mut g, &mut ui, b'r' as u16);
    assert!(g.squad.as_ref().unwrap().p1.turns[2].attacked);
    let turns = g.squad.as_ref().unwrap().p1.turns;
    let party = g.mem.party().to_vec();
    key(&mut g, &mut ui, b'1' as u16);
    assert_eq!(round::selected(&g), 0);
    assert_eq!(g.squad.as_ref().unwrap().p1.turns, turns);
    assert_eq!(g.mem.party(), party);
    assert_eq!((g.mem.read(weapon.add(1), 1), g.mem.read(weapon.add(2), 1)), magellan);
}
fn key(g: &mut Ground, ui: &mut UiState, k: u16) {
    let scene = Scene::capture(g, ScreenKind::Ground, None);
    ui.decision = mt1::play::turns::decision(g);
    ui.menu = scene.squad.menu.as_ref().map(|m| (m.target, (0, 0)));
    if let Some(intents) = keyboard::route(&scene, ui, k) {
        for &intent in &intents { ui.apply(intent); }
        round::pass(g, &intents);
    } else {
        assert!(round::key(g, scan::make_code(k).unwrap()), "key {k:#x} must reach the squad table");
        round::pass(g, &[]);
    }
}
fn pick(g: &mut Ground, id: &str) {
    let d = mt1::play::turns::decision(g).expect("combat decision");
    let mut play = Play::new(); play.register(&d);
    let decision = play.pending().unwrap().id.clone();
    play.choose(Choice::new(decision, id)).unwrap();
    let intent = *play.intent().expect("pick has an intent").downcast::<Intent>().unwrap();
    round::pass(g, &[intent]);
}
fn equal(a: &mut Ground, b: &mut Ground, action: &str) {
    assert_eq!(mt1::play::hash(a), mt1::play::hash(b), "{action} differs from its play pick");
}

#[test]
fn keyboard_efate_target_fire_end_round_and_walk_match_picks() {
    let (mut a, mut b, mut ui) = (game(), game(), UiState::default());
    equal(&mut a, &mut b, "boot");
    key(&mut a, &mut ui, keys::TAB); key(&mut b, &mut UiState::default(), keys::TAB);
    let npc = target(&a); assert!(npc > 0);
    key(&mut a, &mut ui, b'f' as u16); pick(&mut b, &format!("fire:npc/{npc}"));
    equal(&mut a, &mut b, "fire");
    assert!(a.squad.as_ref().unwrap().p1.turns[0].attacked);
    let before = round::selected(&a);
    key(&mut a, &mut ui, b'e' as u16); pick(&mut b, "end_turn");
    equal(&mut a, &mut b, "End turn");
    assert_ne!(round::selected(&a), before);
    assert!(a.squad.as_ref().unwrap().p1.strip.starts_with("Your move:"));
    for _ in 0..10 {
        if a.squad.as_ref().unwrap().p1.encounter.as_ref().unwrap().round >= 2 { break }
        key(&mut a, &mut ui, b'e' as u16); pick(&mut b, "end_turn");
        equal(&mut a, &mut b, "round progression");
    }
    assert_eq!(a.squad.as_ref().unwrap().p1.encounter.as_ref().unwrap().round, 2);
    let (mut a, mut b, mut ui) = (game(), game(), UiState::default());
    let from = round::traveller_cell(&a, 0);
    key(&mut a, &mut ui, b'x' as u16); key(&mut a, &mut ui, keys::LEFT); key(&mut a, &mut ui, keys::DOWN);
    assert_eq!(round::traveller_cell(&a, 0), from, "cursor movement costs no movement");
    let to = ui.keyboard.cursor.unwrap();
    key(&mut a, &mut ui, keys::ENTER); pick(&mut b, &format!("move:cell/{},{}", to.0, to.1));
    assert_eq!(round::traveller_cell(&a, 0), to); assert_ne!(to, from);
    equal(&mut a, &mut b, "cursor walk");
    assert_eq!(a.squad.as_ref().unwrap().p1.turns[0].mp, round::MOVE_WALK - 1);
}

#[test]
fn keyboard_target_menu_letters_and_refused_fire_match_picks() {
    let (mut a, mut b, mut ui) = (game(), game(), UiState::default());
    key(&mut a, &mut ui, keys::TAB); key(&mut b, &mut UiState::default(), keys::TAB);
    let npc = target(&a);
    key(&mut a, &mut ui, b'm' as u16); pick(&mut b, &format!("menu:npc/{npc}"));
    equal(&mut a, &mut b, "target menu");
    assert_eq!(a.squad.as_ref().unwrap().orders.menu.as_ref().unwrap().target, Target::Unit(Unit::Npc(npc)));
    let aim = a.squad.as_ref().unwrap().orders.menu.as_ref().unwrap().items.iter().position(|i| i.key == b'A').unwrap();
    key(&mut a, &mut ui, b'a' as u16); pick(&mut b, &format!("menu:{aim}"));
    equal(&mut a, &mut b, "menu Target");
    key(&mut a, &mut ui, b'm' as u16); pick(&mut b, &format!("menu:npc/{npc}"));
    let menu = a.squad.as_ref().unwrap().orders.menu.as_ref().unwrap();
    let fire = menu.items.iter().position(|i| i.key == b'F').unwrap();
    key(&mut a, &mut ui, b'f' as u16); pick(&mut b, &format!("menu:{fire}"));
    equal(&mut a, &mut b, "menu Fire");
    let ammo = a.mem.party().to_vec();
    key(&mut a, &mut ui, b'm' as u16);
    key(&mut a, &mut ui, b'f' as u16);
    assert_eq!(a.mem.party(), ammo, "a refused second shot cannot spend ammunition");
    assert!(a.squad.as_ref().unwrap().orders.hover.contains("no Attack left"));
}

#[test]
fn keyboard_square_cursor_bounds_cancel_and_menu_precedence() {
    let (mut g, mut ui) = (game(), UiState::default());
    let pos = round::traveller_cell(&g, 0);
    key(&mut g, &mut ui, b'x' as u16);
    for _ in 0..30 { key(&mut g, &mut ui, keys::UP); key(&mut g, &mut ui, keys::LEFT); }
    assert_eq!(ui.keyboard.cursor, Some((g.mem.view_x() / 2, g.mem.view_y() / 2)));
    key(&mut g, &mut ui, keys::ESC);
    assert!(ui.keyboard.cursor.is_none() && ui.hover.is_none());
    assert_eq!(round::traveller_cell(&g, 0), pos);
    key(&mut g, &mut ui, b'x' as u16); key(&mut g, &mut ui, keys::LEFT); key(&mut g, &mut ui, keys::DOWN); key(&mut g, &mut ui, b'm' as u16);
    let menu = g.squad.as_ref().unwrap().orders.menu.as_ref().unwrap();
    assert!(matches!(menu.target, Target::Cell(_)));
    assert!(menu.items.iter().any(|i| i.key == b'E' && i.op == orders::Op::Move(orders::Speed::Evade)));
    key(&mut g, &mut ui, b'e' as u16);
    assert!(g.squad.as_ref().unwrap().p1.turns[0].evade, "menu E means Evade, never End turn: {}", g.squad.as_ref().unwrap().orders.hover);
    assert_eq!(round::selected(&g), 0);
}

#[test]
fn keyboard_split_targeting_and_grouped_orders_preserve_1990_commands() {
    let mut g = game();
    g.squad.as_mut().unwrap().p1.encounter = None;
    assert!(round::key(&mut g, scan::TAB)); assert!(target(&g) > 0, "split exploration can choose a thug");
    assert!(round::key(&mut g, round::KEY_O), "split orders remain squad-owned");
    g.mem.set_combat(0);
    assert!(!round::key(&mut g, round::KEY_O), "grouped O reaches 1990 marching order");
    g.squad = None;
    assert!(!round::key(&mut g, scan::TAB));
    assert!(!round::key(&mut g, round::KEY_O));
    let scene = Scene::capture(&g, ScreenKind::Ground, None);
    for k in [b'e', b'x', b'm'] { assert!(keyboard::route(&scene, &mut UiState::default(), k as u16).is_none()); }
}

#[test]
fn keyboard_end_turn_fires_standing_orders_and_matches_button_at_initiative() {
    let (mut a, mut b, mut ui) = (game(), game(), UiState::default());
    key(&mut a, &mut ui, keys::TAB); key(&mut b, &mut UiState::default(), keys::TAB);
    key(&mut a, &mut ui, b'e' as u16); pick(&mut b, "end_turn");
    equal(&mut a, &mut b, "standing orders plus End turn");
    assert!(a.squad.as_ref().unwrap().p1.turns[0].attacked && a.squad.as_ref().unwrap().p1.turns[0].done);
    assert_ne!(round::selected(&a), 0);
    let mut g = game();
    key(&mut g, &mut ui, keys::TAB); key(&mut g, &mut ui, b'o' as u16);
    assert!(g.squad.as_ref().unwrap().p1.turns[0].attacked && !g.squad.as_ref().unwrap().p1.turns[0].done);
    assert_eq!(round::selected(&g), 0, "Orders keeps the current Traveller's turn");
    let mut g = game();
    g.squad.as_mut().unwrap().p1.encounter.as_mut().unwrap().choosing = true;
    let before = mt1::play::hash(&mut g);
    let scene = Scene::capture(&g, ScreenKind::Ground, None);
    ui.decision = mt1::play::turns::decision(&g);
    let intents = keyboard::route(&scene, &mut ui, b'e' as u16).unwrap();
    assert_eq!(intents, [Intent::EndTurn], "E uses the enabled End turn button's intent at initiative");
    assert_eq!(mt1::play::hash(&mut g), before);
    let (mut a, mut b, mut ui) = (game(), game(), UiState::default());
    for _ in 0..5 {
        key(&mut a, &mut ui, b'e' as u16);
        round::pass(&mut b, &[Intent::EndTurn]);
        equal(&mut a, &mut b, "E versus End turn button from Round 1 - party to act");
    }
    assert_eq!(a.squad.as_ref().unwrap().p1.encounter.as_ref().unwrap().round, 2);
}

#[test]
fn restored_panel_receives_all_retained_squad_lines_without_live_duplicates() {
    use megatraveller::panels::combat_log::{History, Line};
    let mut g = game();
    let expected: Vec<_> = (36..100).map(|i| format!("retained line {i}")).collect();
    g.squad.as_mut().unwrap().p1.log = expected.clone();
    g.squad.as_mut().unwrap().p1.logged = 100;
    let before = mt1::play::hash(&mut g);
    let saved = mt1::play::anchors::Squad::capture(&g.squad).unwrap().encode();
    g.squad = mt1::play::anchors::Squad::decode(&saved).unwrap().state().clone();
    assert_eq!(mt1::play::hash(&mut g), before, "the existing squad save codec retains the full history");
    let scene = Scene::capture(&g, ScreenKind::Ground, None);
    let mut panel = History::default();
    panel.observe_squad(&scene.squad);
    let lines = |h: &History| h.messages.iter().filter_map(|l| if let Line::Text(t) = l { Some(t.clone()) } else { None }).collect::<Vec<_>>();
    assert_eq!(lines(&panel), expected, "a fresh/restored panel must recover all 64 engine lines");
    panel.observe_squad(&scene.squad);
    assert_eq!(lines(&panel), expected, "repeated Scene adds no duplicates");
    assert_eq!(mt1::play::hash(&mut g), before, "Scene capture never mutates the simulation");
    let state = &mut g.squad.as_mut().unwrap().p1;
    state.log.remove(0); state.log.push("one live line".into()); state.logged += 1;
    panel.observe_squad(&Scene::capture(&g, ScreenKind::Ground, None).squad);
    assert_eq!(panel.messages.len(), 65);
    assert_eq!(lines(&panel).last().unwrap(), "one live line");
}
