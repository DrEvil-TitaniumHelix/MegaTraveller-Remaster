mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
use super::*;
use mt_engine::{io::{keys, Dir, Event, Input}, play::{Decision, Kind, Option as Choice}};
use std::sync::Arc;

fn text_at(out: &Surface, y: i64) -> String {
    let mut runs: Vec<_> = out.text.visible.iter().filter(|r| r.y == y).collect();
    runs.sort_by_key(|r| r.x);
    runs.into_iter().flat_map(|r| r.bytes.iter().copied()).map(char::from).collect()
}
fn game() -> Option<(mt1::ground::Ground, Arc<assets::Assets>)> {
    let Some(dir) = oracle_env::gamefiles() else { return None };
    let dir = std::path::PathBuf::from(dir);
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml");
    let mut g = crate::scenario::load(&path, &dir).unwrap().game;
    struct Idle;
    impl Input for Idle {
        fn next_key(&mut self) -> u16 { panic!("unexpected native wait") }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { clock.wait_tick(); Event::Idle }
    }
    crate::scenario::attach_scene(&mut g); g.input = Box::new(Idle);
    mt1::ground::round::pass(&mut g, &[]);
    Some((g, Arc::new(assets::Assets::load(&mut Dir(dir)))))
}

#[test]
fn scaled_map_replay_text_keeps_the_plus_face() {
    let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.combat = true;
    s.squad.fx = mt1::ground::round::Fx { seq: 7, hits: vec![(Unit::Npc(1), (3, 4), 9)], ..Default::default() };
    let ui = UiState { fixed_screen: true, replay: Some((7, 0.1)), ..Default::default() };
    let Some((_, assets)) = game() else { return };
    let mut layout = Layout::default(); components::register(&mut layout, assets);
    let look = crate::look::Look::default();
    for size in [[1531, 1221], [640, 480]] {
        let hit = render::main(&mut layout, &s, &ui, &look, size);
        let mut clear = s.clone(); clear.squad.fx.hits.clear();
        let no_hit = render::main(&mut layout, &clear, &ui, &look, size);
        let Host::Dock(host) = layout.host("Viewport").unwrap() else { panic!() };
        let r = viewport::screen_rect((host.w, host.h));
        let m = render::Metrics::for_ui(size, &look, Some(&s), &ui);
        let r = m.rect(Rect::new(host.x + r.x, host.y + r.y, r.w, r.h));
        let changed: Vec<_> = hit.iter().zip(&no_hit).enumerate().filter(|(_, (a, b))| a != b).map(|(i, _)| (i % size[0] as usize, i / size[0] as usize)).collect();
        assert!(!changed.is_empty(), "scaled hit digit must retain its Plus font at {size:?}");
        assert!(changed.iter().all(|&(x, y)| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height));
    }
}

#[test]
fn map_fills_its_host_without_moving_f10_panels_between_turns() {
    use crate::look::Look;
    for preset in [Preset::Modern, Preset::Classic] {
        let mut look = Look::default();
        if preset == Preset::Classic { look.layout.preset = settings::LayoutPreset::Classic; }
        for size in [[1531, 1221], [640, 480]] {
            let mut ui = UiState { fixed_screen: true, preset, ..Default::default() };
            let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
            let mut before = None;
            for phase in 0..6 {
                let mut s = Scene::program(ScreenKind::Ground); s.squad.active = phase != 5;
                s.combat = matches!(phase, 1 | 2); s.squad.encounter = s.combat;
                s.squad.acting = (phase == 2).then_some(Unit::Npc(1));
                s.squad.strip = ["Party split", "Your move", "NPC turn", "Victory", "Quit", ""][phase].into();
                ui.decision = (phase == 4).then(|| Decision::new(Kind::Menu, "t", "AF79"));
                let m = render::Metrics::for_ui(size, &look, Some(&s), &ui);
                layout.arrange_for(&s, &ui, m.logical);
                let rects: Vec<_> = ["Party strip", "Side panel", "Combat controls", "Initiative order", "Combat log", "Commands", "Viewport"]
                    .into_iter().map(|id| {
                        let Host::Dock(r) = layout.host(id).unwrap() else { panic!() }; (id, m.rect(r))
                    }).collect();
                if let Some(ref original) = before { assert_eq!(&rects, original, "{preset:?} {size:?} phase {phase}"); }
                else { before = Some(rects); }
                let Host::Dock(host) = layout.host("Viewport").unwrap() else { panic!() };
                let r = viewport::screen_rect((host.w, host.h));
                assert!(r.w == host.w || r.h == host.h, "{preset:?} {size:?}: map must reach a host edge: {r:?} in {host:?}");
                assert!((i64::from(r.w) * 112 - i64::from(r.h) * 192).abs() < 192, "one proportional scale, rounded to a pixel");
                assert_eq!((r.x, r.y), (((host.w - r.w) / 2) as i32, ((host.h - r.h) / 2) as i32));
                let Host::Dock(log) = layout.host("Combat log").unwrap() else { panic!() };
                assert_eq!(host.y + host.h as i32, log.y);
            }
        }
    }
}

#[ignore = "the action list is disabled; set choices::ENABLED to run"]

#[test]
fn g6_gunner_commands_leave_the_native_target_view_visible() {
    let scene=Scene::program(mt1::ui::ScreenKind::Space);
    let ui=UiState {decision:Some(mt_engine::play::Decision::new(mt_engine::play::Kind::Menu,"s","955C")),..Default::default()};
    let metrics=render::Metrics::for_ui([1531,1221],&crate::look::Look::default(),Some(&scene),&ui);
    assert_eq!(metrics.logical,choices::BESIDE_SIZE);
    let mut layout=Layout::default();components::register(&mut layout,std::sync::Arc::default());
    layout.arrange_for(&scene,&ui,metrics.logical);
    assert_eq!(layout.host("Choices"),Some(Host::Dock(Rect::new(320,0,160,200))));
    assert_eq!(layout.host("Picture"),Some(Host::Dock(Rect::new(0,0,320,200))));
}

#[test]
fn f5_traveller_panel_tracks_end_turn_with_a_stale_native_picture() {
    let Some((mut g, assets)) = game() else { return };
    let panel = side_panel::SidePanel::new(assets);
    let mut ui = UiState::default();
    let before = Scene::capture(&g, ScreenKind::Ground, None);
    let mut stale = Surface::new(128, 146); panel.draw(&before, &ui, &mut stale);
    let mut frame = Surface::new(320, 200); frame.draw(&stale, 192, 54); *ui.frame = frame.into_frame();
    let original = ui.frame.indexed;
    for _ in 0..4 {
        mt1::ground::round::pass(&mut g, &[Intent::EndTurn]);
        let scene = Scene::capture(&g, ScreenKind::Ground, None);
        let mut out = Surface::new(128, 146); panel.draw(&scene, &ui, &mut out);
        assert_ne!(scene.squad.selected, before.squad.selected);
        assert_eq!(text_at(&out, 9), scene.party.name(scene.squad.selected));
        assert!(out.text.visible.iter().filter(|r| r.y == 9).any(|r| r.bitmap.iter().zip(&r.background).any(|(&ink, &bg)| ink == 0 && bg != 0)), "the name is painted in black, not merely annotated");
        assert_ne!(text_at(&out, 9), text_at(&stale, 9));
        if let Some((_, mags, rounds)) = scene.party.hand(scene.squad.selected, scene.walkers[scene.squad.selected as usize].hand()) {
            assert_eq!(text_at(&out, 28), format!("ammo{mags}"));
            assert!(text_at(&out, 49).starts_with(&format!("Rounds {rounds}")));
        }
        assert_eq!(ui.frame.indexed, original, "Modern rendering never changes the native frame");
    }
    let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
    ui.preset = Preset::Classic; layout.arrange_for(&before, &ui, (320, 200));
    assert_eq!(layout.compose(&before, &ui, (320, 200)).indexed, original);
}

#[test]
fn f5_party_box_survives_native_clear_and_tracks_party_order() {
    let Some((g, assets)) = game() else { return };
    let mut scene = Scene::capture(&g, ScreenKind::Ground, None);
    let panel = party_strip::PartyStrip::new(assets);
    let mut ui = UiState::default();
    let box_pixels = |s: &Surface| (0..54).flat_map(|y| (256..320).map(move |x| s.get(x, y).unwrap())).collect::<Vec<_>>();
    let mut initial = Surface::new(320, 54); panel.draw(&scene, &ui, &mut initial);
    for y in 0..54 { ui.frame.indexed[y * 320 + 256..y * 320 + 320].fill(6); }
    let mut cleared = Surface::new(320, 54); panel.draw(&scene, &ui, &mut cleared);
    assert_eq!(box_pixels(&cleared), box_pixels(&initial));
    assert!(box_pixels(&cleared).iter().any(|&c| c == 0));
    assert!(box_pixels(&cleared).iter().any(|&c| c == 1));
    scene.party.0.swap(1, 2);
    let mut swapped = Surface::new(320, 54); panel.draw(&scene, &ui, &mut swapped);
    assert_ne!(box_pixels(&swapped), box_pixels(&cleared), "the box is drawn from today's order, not a cached opening");
    let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
    ui.preset = Preset::Classic; layout.arrange_for(&scene, &ui, (320, 200));
    assert_eq!(layout.compose(&scene, &ui, (320, 200)).indexed, ui.frame.indexed);
}

#[ignore = "the action list is disabled; set choices::ENABLED to run"]

#[test]
fn f5_action_lists_leave_every_sheet_pixel_and_name_visible() {
    let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
    for screen in [ScreenKind::Record, ScreenKind::Picture] {
        let scene = Scene::program(screen);
        let mut d = Decision::new(Kind::Menu, "t", "sheet");
        d.options = (0..40).map(|i| Choice::new(format!("row{i}"), format!("Action {i}"), vec![Event::Key(keys::ENTER)])).collect();
        let mut ui = UiState { decision: Some(d), ..UiState::default() };
        ui.frame.indexed = std::array::from_fn(|i| (i % 16) as u8);
        let mut name = Surface::new(320, 200); assets::Assets::default().text(&mut name, "MAGELLAN", (6, 64), 15);
        ui.frame.text = name.text;
        let look = crate::look::Look::default();
        for size in [[640, 480], [1536, 1220], [320, 200], [0, 0], [1, 999]] {
            let m = render::Metrics::for_ui(size, &look, Some(&scene), &ui);
            assert!(!m.classic); assert_eq!(m.logical, choices::logical(&scene, &ui)); let w = m.logical.0 as usize;
            layout.arrange_for(&scene, &ui, m.logical);
            let Host::Dock(r) = layout.host("Choices").unwrap() else { panic!() };
            assert!(r.y >= 200 && r.x + r.w as i32 <= 320, "the band is under the whole sheet: {r:?}");
            let composed = layout.compose(&scene, &ui, m.logical);
            for y in 0..200 { assert_eq!(&composed.indexed[y * w..y * w + 320], &ui.frame.indexed[y * 320..(y + 1) * 320]); }
            assert_eq!(text_at(&composed, 64), "MAGELLAN");
        }
        ui.preset = Preset::Classic;
        let m = render::Metrics::for_ui([960, 720], &look, Some(&scene), &ui);
        assert!(m.classic); assert_eq!(m.logical, (320, 200));
        layout.arrange_for(&scene, &ui, m.logical);
        assert_eq!(layout.host("Choices"), Some(Host::Dock(Rect::new(8, 10, 304, 180))));
    }
}

#[test]
fn f5_held_key_keeps_commands_and_all_dock_rectangles_until_next_decision() {
    let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
    let scene = Scene::program(ScreenKind::Ground);
    let mut ui = UiState::default(); let mut store = crate::mouse::decision::Store::default();
    let mut d = Decision::new(Kind::Ground, "t", "6923");
    d.options.push(Choice::new("cmd:game", "Game", vec![Event::Key(b'G' as u16)]));
    store.record(&mut ui, &d); layout.arrange_for(&scene, &ui, (384, 254));
    let geometry = |l: &Layout| l.ids().into_iter().filter(|id| l.visible(id, &scene)).map(|id| (id, l.host(id))).collect::<Vec<_>>();
    let before = geometry(&layout); let picture = layout.compose(&scene, &ui, (384, 254));
    for event in [Event::Key(keys::RIGHT), Event::Key(keys::RIGHT), Event::KeyUp(keys::RIGHT)] {
        store.hardware(&mut ui, event); assert!(ui.decision.is_none());
        layout.arrange_for(&scene, &ui, (384, 254));
        assert!(layout.visible("Commands", &scene)); assert_eq!(geometry(&layout), before);
        assert_eq!(layout.compose(&scene, &ui, (384, 254)), picture);
        assert!(layout.input("Commands", &scene, &ui, &UiEvent::Up(Button::Left), (2, 2)).is_empty());
    }
    store.record(&mut ui, &d); layout.arrange_for(&scene, &ui, (384, 254)); assert_eq!(geometry(&layout), before);
    store.record(&mut ui, &Decision::new(Kind::Menu, "t", "game"));
    layout.arrange_for(&scene, &ui, (384, 254)); assert!(layout.visible("Commands", &scene), "F10 r2: the row keeps its place (blank) under a menu");
    for (id, host) in &before { assert_eq!(layout.host(id), *host, "{id} keeps its dock under the menu"); }
}

#[test]
fn f7_log_scroll_bounds_match_the_rendered_width_and_height() {
    for (width, height) in [(256, 36), (640, 45), (96, 27)] {
        let mut ui = UiState::default();
        for i in 0..16 { ui.components.log.messages.push_back(combat_log::Line::Text(format!("line {i}: a long combat message that wraps differently at each panel width"))); }
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        layout.set_host("Combat log", Host::Window { id: 1, size: (width, height) });
        controls::prepare(&layout, &mut ui);
        let visible = ((height as usize - 9) / 9).min(3);
        let oldest = ui.components.log.rows(width as usize - 4).len() as i32 - visible as i32;
        ui.apply(Intent::LogScroll(10000));
        assert_eq!(ui.log_scroll, oldest, "bound must match actual {width} x {height} log");
        ui.apply(Intent::LogScroll(3)); assert_eq!(ui.log_scroll, oldest);
        ui.apply(Intent::LogScroll(-1)); assert_eq!(ui.log_scroll, oldest - 1, "first reverse key moves immediately");
        ui.apply(Intent::LogScroll(-10000)); ui.apply(Intent::LogScroll(-3)); assert_eq!(ui.log_scroll, 0);
        ui.apply(Intent::LogScroll(1)); assert_eq!(ui.log_scroll, 1);
    }
}

#[test]
fn f7_modern_exploration_keeps_the_vehicle_row_above_commands_at_both_sizes() {
    let mut scene = Scene::program(ScreenKind::Ground); scene.squad.active = true;
    let mut decision = Decision::new(Kind::Ground, "t", "6923");
    decision.options.push(Choice::new("cmd:vehicle", "Vehicle", vec![Event::Key(b'V' as u16)]));
    let mut ui = UiState { decision: Some(decision), ..UiState::default() };
    for x in 192..320 { ui.frame.indexed[199 * 320 + x] = 5; }
    for size in [[640, 480], [1531, 1221]] {
        let metrics = render::Metrics::for_ui(size, &crate::look::Look::default(), Some(&scene), &ui);
        assert!(!metrics.classic);
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        layout.arrange_for(&scene, &ui, metrics.logical);
        let Host::Dock(side) = layout.host("Side panel").unwrap() else { panic!() };
        let Host::Dock(commands) = layout.host("Commands").unwrap() else { panic!() };
        assert!(side.h >= 146 && side.y + 146 <= commands.y, "Vehicle row clipped at {size:?}: {side:?}, {commands:?}");
        let frame = layout.compose(&scene, &ui, metrics.logical);
        assert_eq!(frame.get(side.x, side.y + 145), Some(5), "bottom native row remains visible");
    }
}
