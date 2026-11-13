use megatraveller::{look::{Filter, Look, Screen, Shape}, mouse::pointer::{self, Element, Hit, MapCell},
    panels::{self, assets::Assets, components, render::Metrics, Host, Layout, Origin, Rect, UiState}};
use mt1::ui::{Preset, Scene, ScreenKind, Target, Unit};
use std::sync::Arc;

fn scene() -> Scene {
    let mut s = Scene::program(ScreenKind::Ground); s.view = (30, 40); s.pos = (40, 50);
    let m = Arc::make_mut(&mut s.map); m.rows = 100; m.cols = 100; m.stride = 100; m.cells = vec![0; 30000];
    s
}
fn layout(scene: &Scene, ui: &UiState, size: (u32, u32)) -> Layout {
    let mut l = Layout::default(); components::register(&mut l, Arc::new(Assets::default())); l.arrange_for(scene, ui, size); l
}

#[test]
fn pointer_freedom_every_pixel_band_in_modern_and_1990_positions() {
    let s = scene();
    for (preset, size) in [(Preset::Modern, [801, 601]), (Preset::Modern, [1377, 711]),
        (Preset::Classic, [320, 200]), (Preset::Classic, [997, 719])] {
        let mut look = Look::default(); look.layout.preset = preset.into(); look.screen = Screen::None;
        let m = Metrics::main(size, &look, Some(&s));
        let ui = UiState { preset: if m.classic { Preset::Classic } else { preset }, camera: (30.0, 40.0), ..UiState::default() };
        let l = layout(&s, &ui, m.logical);
        let rects: Vec<_> = l.ids().into_iter().filter(|id| l.visible(id, &s)).filter_map(|id| {
            let Host::Dock(r) = l.host(id)? else { return None }; Some((id, m.rect(r)))
        }).collect();
        let mut map_hits = 0;
        for y in 0..size[1] { for x in 0..size[0] {
            let expected = rects.iter().find(|(_, r)| x as usize >= r.x && y as usize >= r.y
                && (x as usize) < r.x + r.width && (y as usize) < r.y + r.height).map(|r| r.0);
            let hit = pointer::window(&l, &s, &ui, m, &look, None, (x as f64 + 0.25, y as f64 + 0.25));
            match (expected, hit) {
                (None, Hit::Background) => {},
                (Some(id), Hit::Component(h)) => {
                    assert_eq!(h.id, id, "{preset:?} {size:?} ({x},{y})");
                    let host = l.host(id).unwrap(); assert!(Rect::new(0, 0, host.size().0, host.size().1).contains(h.local));
                    let legacy = pointer::live_mouse(Some((id, l.placement(id).unwrap().origin, h.local)));
                    assert_eq!(legacy.buttons, 0);
                    if id == "Viewport" { assert_eq!((legacy.col, legacy.row), (-1, -1)); assert!(matches!(h.element, Element::Map(Some(_)))); map_hits += 1; }
                }
                other => panic!("{preset:?} {size:?} ({x},{y}): {other:?}"),
            }
        } }
        assert!(map_hits > 0);
        for at in [(-0.1, 0.0), (0.0, -0.1), (size[0] as f64, 0.0), (0.0, size[1] as f64), (f64::NAN, 0.0), (0.0, f64::INFINITY)] {
            assert_eq!(pointer::window(&l, &s, &ui, m, &look, None, at), Hit::Outside);
        }
    }
}

#[test]
fn each_element_and_all_zoom_levels_are_in_map_coordinates() {
    let mut s = scene(); s.squad.encounter = true; s.squad.queue = vec![Unit::Traveller(3), Unit::Npc(2)];
    let mut ui = UiState { camera: (30.0, 40.0), ..UiState::default() };
    let l = layout(&s, &ui, (640, 400));
    for i in 0..5u8 {
        assert_eq!(pointer::element("Party strip", &s, &ui, (640, 54), (6 + i as i32 * 40, 20), false), Element::Traveller(i));
    }
    assert_eq!(pointer::element("Party strip", &s, &ui, (640, 54), (206, 20), false), Element::Pixel(206, 20));
    assert_eq!(pointer::element("Initiative order", &s, &ui, (640, 24), (27, 10), false), Element::Initiative(Unit::Traveller(3)));
    assert_eq!(pointer::element("Initiative order", &s, &ui, (640, 24), (91, 10), false), Element::Initiative(Unit::Npc(2)));
    for id in ["Side panel", "Controls", "Picture", "Combat log", "Menu"] {
        assert_eq!(pointer::element(id, &s, &ui, (128, 146), (10, 100), false), Element::Pixel(10, 100));
    }
    s.screen = ScreenKind::Record;
    assert_eq!(pointer::element("Picture", &s, &ui, (320, 200), (50, 10), false), Element::Traveller(1));
    s.screen = ScreenKind::Ground;
    let size = l.host("Viewport").unwrap().size();
    ui.camera = (30.5, 40.5);
    for zoom in [0.25, 0.5, 1.0, 2.0] {
        ui.zoom = zoom;
        assert_eq!(pointer::map_cell(&s, &ui, size, (size.0 as i32 / 2, size.1 as i32 / 2), false), Some(MapCell { row: 30, col: 40 }));
    }
}

#[test]
fn filters_shapes_crt_background_and_detached_hosts_follow_displayed_pixels() {
    let s = scene(); let ui = UiState::default();
    for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] { for shape in [Shape::Classic, Shape::Square] {
        for screen in [Screen::None, Screen::Crt] {
            let look = Look { filter, shape, screen, ..Look::default() };
            let m = Metrics::component([733, 317], (320, 54), &look);
            let mut l = layout(&s, &ui, (640, 400)); l.set_host("Party strip", Host::Window { id: 7, size: m.logical });
            for y in (0..317).step_by(3) { for x in (0..733).step_by(3) {
                let at = (x as f64, y as f64); let hit = pointer::window(&l, &s, &ui, m, &look, Some(7), at);
                if m.unwarp(at, &look, false).is_none() { assert_eq!(hit, Hit::Background); }
                else { assert!(matches!(hit, Hit::Component(h) if h.id == "Party strip")); }
            } }
            assert_eq!(pointer::window(&l, &s, &ui, m, &look, Some(8), (100.0, 100.0)), Hit::Background);
        }
    } }
}

#[test]
fn tooltip_never_swallows_any_pixel_in_either_host_or_layout() {
    let mut s = scene(); s.squad.hover = "Vlaris: ready".into();
    for preset in [Preset::Modern, Preset::Classic] { for detached in [false, true] {
        let mut ui = UiState { preset, hover: Some(Target::Unit(Unit::Traveller(0))), ..UiState::default() };
        let mut l = layout(&s, &ui, (640, 400));
        if detached { l.set_host("Party strip", Host::Window { id: 7, size: (320, 54) }); }
        ui.components.pointer = Some(("Party strip", (40, 20))); l.arrange_for(&s, &ui, (640, 400));
        assert!(l.visible(panels::tooltip::ID, &s));
        let Host::Dock(rect) = l.host(panels::tooltip::ID).unwrap() else { panic!() };
        assert_eq!(l.host_window(panels::tooltip::ID), detached.then_some(7));
        assert!(rect.y > 0, "the anchor is the actual pointer, not ui.menu's default origin");
        for y in rect.y..rect.y + rect.h as i32 { for x in rect.x..rect.x + rect.w as i32 {
            assert_ne!(l.hit(&s, detached.then_some(7), (x, y)).map(|h| h.0), Some(panels::tooltip::ID));
        } }
    } }
}

#[test]
fn a_held_pad_never_enters_the_ground_live_words() {
    use mt_engine::{io::{Input, Overlay, MouseState}, sim::tick::Counter};
    use mt1::ground::{input::read_mouse, state::{Ground, Mem}};
    struct HeldPad;
    impl Input for HeldPad {
        fn next_key(&mut self) -> u16 { 27 }
        fn mouse(&mut self) -> MouseState { pointer::live_mouse(Some(("Side panel", Origin::Classic1990(components::SIDE_RECT), (40, 100)))) }
        fn set_mouse_position(&mut self, _: i16, _: i16) { panic!("pad never asks to warp") }
    }
    let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(HeldPad), Box::new(Counter::at(0)), Box::new(Overlay::new(None)));
    g.mem.set_i16(0x7012, 1);
    for _ in 0..1000 { read_mouse(&mut g); assert_eq!(g.mem.u16(0x6FF8), 0); }
    assert_eq!((g.mem.i16(0x6FFC), g.mem.i16(0x6FFE)), (232, 154));
}
