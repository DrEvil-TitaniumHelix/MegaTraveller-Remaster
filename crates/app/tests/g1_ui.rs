use megatraveller::{look::Look, panels::{*, render::Metrics}};
use mt1::{ground::orders::{Menu, MenuItem, Op}, ui::{Intent, Scene, ScreenKind, Target, Unit}};
use mt_engine::play::{Decision, Kind};
use std::sync::Arc;

fn layout() -> Layout { let mut l = Layout::default(); components::register(&mut l, Arc::default()); l }
fn menu(items: Vec<MenuItem>) -> Scene {
    let mut s = Scene::program(ScreenKind::Ground);
    s.squad.menu = Some(Menu { target: Target::Portrait(0), items, default: 0 }); s
}
fn text(out: &Surface) -> String { out.text.visible.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect() }

#[test]
fn save_ack_keeps_starport_geometry_and_shipyard_uncovered() {
    let mut l = layout(); let mut ui = UiState::default(); let mut s = Scene::program(ScreenKind::Picture);
    s.context.event = 9;
    let mut expected = None;
    for kind in [Some(Kind::Menu), None, Some(Kind::Ack), None, Some(Kind::Ack), Some(Kind::Menu)] {
        ui.decision = kind.map(|k| Decision::new(k, "t", "starport"));
        let m = Metrics::for_ui([1531, 1221], &Look::default(), Some(&s), &ui);
        l.arrange_for(&s, &ui, m.logical);
        let picture = l.host("Picture").unwrap();
        let geometry = (m.pixels, picture);
        if let Some(expected) = expected { assert_eq!(geometry, expected, "Select drive / Game saved must keep the starport in place"); }
        else { expected = Some(geometry); }
        assert!(!l.visible("Choices", &s), "the retired list must not cover Shipyard");
        let drawn = l.draw_host(&s, &ui, None);
        assert!(drawn.iter().all(|(id, ..)| *id != "Choices"));
        assert_eq!(l.visible("Continue", &s), kind == Some(Kind::Ack));
        if kind == Some(Kind::Ack) {
            assert_eq!(l.host("Continue").unwrap(), picture, "Ack must coincide with the starport picture");
        }
    }
}

#[test]
fn ground_restore_ack_draws_one_ground_screen() {
    let mut l = layout(); let mut ui = UiState::default(); let s = Scene::program(ScreenKind::Ground);
    let m = Metrics::for_ui([1531, 1221], &Look::default(), Some(&s), &ui);
    l.arrange_for(&s, &ui, m.logical);
    let viewport = l.host("Viewport").unwrap();
    let mut d = Decision::new(Kind::Ack, "t", "restore");
    d.options.push(mt_engine::play::Option::new("continue", "Continue", vec![mt_engine::io::Event::Key(32)]));
    d.options.push(mt_engine::play::Option::new("back", "Back", vec![mt_engine::io::Event::Key(27)]));
    ui.decision = Some(d);
    l.arrange_for(&s, &ui, m.logical);
    assert_eq!(l.host("Viewport").unwrap(), viewport);
    assert!(!l.visible("Continue", &s), "the Ack picture must not duplicate the ground docks");
    let drawn = l.draw_host(&s, &ui, None);
    assert!(!l.visible("Choices", &s));
    assert!(drawn.iter().all(|(id, ..)| !["Choices", "Continue", "Picture"].contains(id)));
    for id in ["Viewport", "Party strip", "Side panel", "Combat log"] {
        assert_eq!(drawn.iter().filter(|(drawn_id, ..)| *drawn_id == id).count(), 1, "one {id} during restore");
    }
}

#[test]
fn portrait_menu_has_only_two_rows_of_background() {
    let s = menu(vec![MenuItem { key: b'S', label: "Select".into(), op: Op::Select(0), grey: None },
        MenuItem { key: b'C', label: "Character".into(), op: Op::Character(0), grey: None }]);
    let mut l = layout(); l.arrange_for(&s, &UiState::default(), (510, 339));
    assert_eq!(l.host("Menu").unwrap().size().1, 22, "two 9-pixel rows plus borders, no empty black box");
}

#[test]
fn target_menu_draws_whole_labels_and_reasons() {
    let labels = [("Full auto", "switch to full auto first (Q)", b'U'), ("Tactics +1", "maximum 0 (the highest single level)", b'+')];
    let s = menu(labels.iter().map(|&(label, reason, key)| MenuItem { key, label: label.into(), op: Op::Fire, grey: Some(reason.into()) }).collect());
    let mut l = layout(); let ui = UiState::default(); l.arrange_for(&s, &ui, (510, 339));
    let (_, _, out) = l.draw_host(&s, &ui, None).into_iter().find(|(id, ..)| *id == "Menu").unwrap();
    let drawn = text(&out);
    for (label, reason, _) in labels { assert!(drawn.contains(&format!("{label} ({reason})")), "{drawn}"); }
}

#[test]
fn help_starts_with_keyboard_table_on_every_screen() {
    for screen in [ScreenKind::Picture, ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Ground, ScreenKind::Space] {
        let t = megatraveller::plus::help::text(&Scene::program(screen));
        assert!(t.starts_with(megatraveller::plus::help::GROUND_KEYS), "{screen:?}: {t}");
    }
}

#[test]
fn early_round_log_reaches_oldest_and_reverses_on_first_press() {
    let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.squad.encounter = true;
    s.squad.acting = Some(Unit::Traveller(0)); s.squad.strip = "Your move: Magellan".into();
    let mut l = layout(); let mut ui = UiState::default(); ui.encounters = 1;
    for i in 0..8 { ui.components.log.messages.push_back(combat_log::Line::Text(format!("History {i}"))); }
    for round in 1..=3 {
        s.squad.round = round; l.arrange_for(&s, &ui, (510, 339));
        controls::prepare(&l, &mut ui);
        let draw = |ui: &UiState| l.draw_host(&s, ui, None).into_iter().find(|(id, ..)| *id == "Combat log").unwrap().2;
        ui.apply(Intent::LogScroll(100)); assert_eq!(ui.log_scroll, 6, "8 lines minus 2 history rows under the hint");
        let oldest = text(&draw(&ui)); assert!(oldest.contains("History 0") && oldest.contains("History 1"), "{oldest}");
        ui.apply(Intent::LogScroll(1)); assert_eq!(text(&draw(&ui)), oldest);
        ui.apply(Intent::LogScroll(-1)); assert!(text(&draw(&ui)).contains("History 2"));
        ui.apply(Intent::LogScroll(-100)); let newest = text(&draw(&ui)); assert!(newest.contains("History 7"));
        ui.apply(Intent::LogScroll(-1)); assert_eq!(text(&draw(&ui)), newest);
        ui.apply(Intent::LogScroll(1)); assert!(text(&draw(&ui)).contains("History 5"));
    }
}

#[test]
fn atoms_distinguish_end_turn_from_o_and_name_visible_evidence() {
    let atoms = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/atoms");
    let read = |prefix: &str| std::fs::read_to_string(std::fs::read_dir(&atoms).unwrap().map(Result::unwrap)
        .map(|e| e.path()).find(|p| p.extension().is_some_and(|x| x == "md") && p.file_name().unwrap().to_string_lossy().starts_with(prefix)).unwrap()).unwrap();
    let end_turn = read("014_"); let orders = read("015_");
    assert!(end_turn.contains("Click End turn (or press E)"));
    assert!(end_turn.contains("next Traveller after Magellan"));
    assert!(orders.contains("Click Orders (or press O), without End turn"));
    assert!(orders.contains("still reads Your move: Magellan"));
    assert!(!read("010_").contains("still reads Your move"));
    assert!(read("009_").contains("DOWN label"));
}
