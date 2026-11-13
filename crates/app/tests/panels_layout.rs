use megatraveller::{look::{Look, Shape, Screen, Filter, ScaleMode}, panels::{*, settings::*, render::{self, Metrics}}, pipeline::{self, Frame}};
use mt1::ui::{Intent, Preset, Scene, ScreenKind, Unit};
use mt_engine::io::Event;

fn scene() -> Scene { Scene::program(ScreenKind::Ground) }
fn picture_layout() -> Layout { Layout::pictures() }
fn product_layout() -> Layout { let mut layout = Layout::default(); components::register(&mut layout, std::sync::Arc::default()); layout }
fn pattern() -> Frame { Frame { indexed: std::array::from_fn(|i| ((i * 7 + i / 320) & 15) as u8), ..Default::default() } }

#[test]
fn product_catalogue_modern_docks_and_all_picture_screens() {
    let mut layout = product_layout(); let mut scene = scene(); scene.squad.encounter = true;
    let ui = UiState::default(); layout.arrange_for(&scene, &ui, (500, 400));
    for (id, r) in [("Party strip", Rect::new(0, 0, 500, 64)), ("Initiative order", Rect::new(0, 64, 500, 24)),
        ("Minimap", Rect::new(372, 88, 128, 64)), ("Side panel", Rect::new(372, 152, 128, 212)),
        ("Combat log", Rect::new(0, 328, 372, 36)), ("Viewport", Rect::new(0, 88, 372, 240)), ("Commands", Rect::new(0, 364, 500, 36))] {
        assert!(layout.visible(id, &scene), "{id}"); assert_eq!(layout.host(id), Some(Host::Dock(r)), "{id}");
    }
    for screen in [ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Record, ScreenKind::Space] {
        scene.screen = screen;
        let ui = UiState { frame: Box::new(pattern()), ..Default::default() };
        let look = Look::default();
        assert_eq!(render::main(&mut layout, &scene, &ui, &look, [960, 720]), pipeline::render(&ui.frame, &look, 960, 720));
        assert_eq!(layout.draw_host(&scene, &ui, None).iter().map(|(id, ..)| *id).collect::<Vec<_>>(), ["Picture"]);
    }
}

#[test]
fn classic_tiles_every_pixel_and_reconstructs_every_pattern() {
    let mut coverage = [0u8; 64000];
    for r in CLASSIC { for y in r.y..r.y + r.h as i32 { for x in r.x..r.x + r.w as i32 { coverage[y as usize * 320 + x as usize] += 1; } } }
    assert!(coverage.iter().all(|n| *n == 1));
    let mut layout = product_layout();
    for seed in 0..16 {
        let mut ui = UiState { frame: Box::new(pattern()), preset: Preset::Classic, ..Default::default() };
        for p in &mut ui.frame.indexed { *p ^= seed; }
        ui.frame.regs = std::array::from_fn(|i| (i as u8 * 7 + seed) & 63);
        layout.arrange_for(&scene(), &ui, (320, 200));
        let out = layout.compose(&scene(), &ui, (320, 200)).into_frame();
        assert_eq!(out.indexed, ui.frame.indexed); assert_eq!(out.regs, ui.frame.regs);
    }
}

#[test]
fn classic_boundary_text_keeps_one_annotation_in_original_order() {
    use mt_engine::present::textlog::Run;
    let mut ui = UiState { frame: Box::new(pattern()), preset: Preset::Classic, ..Default::default() };
    for (x, y) in [(190, 52), (189, 163), (7, 51)] {
        ui.frame.text.visible.push(Run { page: 0, x, y, colour: 15, shadow: Some(0), bytes: vec![b'A'], font_id: 1,
            width: 8, descend: false, covered: false, background_width: 9, background_height: 9, background: vec![1; 81], bitmap: vec![15; 81] });
    }
    ui.frame.text.runs = ui.frame.text.visible.clone();
    let mut layout = product_layout(); layout.arrange_for(&scene(), &ui, (320, 200));
    let frame = layout.compose(&scene(), &ui, (320, 200)).into_frame();
    assert_eq!(frame.text, ui.frame.text);
    let look = Look { font: megatraveller::look::FontChoice::Builtin(0), ..Default::default() };
    assert_eq!(pipeline::render(&frame, &look, 960, 720), pipeline::render(&ui.frame, &look, 960, 720));
}

#[test]
#[ignore = "set MT_PANEL_FRAMES to frames captured by tools/panel_frames.mjs"]
fn classic_reconstructs_lockstep_reference_frames() {
    let path = std::path::PathBuf::from(std::env::var_os("MT_PANEL_FRAMES").expect("MT_PANEL_FRAMES"));
    let mut layout = product_layout(); let mut count = 0;
    for entry in std::fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path(); if path.extension().is_none_or(|e| e != "ega") { continue; }
        let bytes = std::fs::read(&path).unwrap(); assert_eq!(bytes.len(), 64016);
        let mut ui = UiState { preset: Preset::Classic, ..Default::default() };
        ui.frame.indexed.copy_from_slice(&bytes[..64000]); ui.frame.regs.copy_from_slice(&bytes[64000..]);
        layout.arrange_for(&scene(), &ui, (320, 200));
        let out = layout.compose(&scene(), &ui, (320, 200)).into_frame();
        assert_eq!(out.indexed, ui.frame.indexed, "{}", path.display()); assert_eq!(out.regs, ui.frame.regs); count += 1;
    }
    assert!(count > 0); println!("Classic Picture reconstruction: {count} lockstep reference frames PASS");
}

fn new_panel(layout: &mut Layout, id: &'static str, dock: Dock, order: u8, size: (u32, u32)) {
    layout.register(Box::new(Picture::full()), Placement { dock, order, size: Size { preferred: size, min: (1, 1) }, origin: Origin::New, optional: true, ..Picture::placement(id, FRAME) });
}
#[test]
fn modern_edges_resize_optional_priority_and_classic_fallback() {
    let mut layout = picture_layout(); let mut ui = UiState::default();
    new_panel(&mut layout, "Initiative", Dock::Top, 5, (320, 24));
    new_panel(&mut layout, "Minimap", Dock::Right, 9, (128, 96));
    layout.arrange_for(&scene(), &ui, (500, 400));
    assert_eq!(layout.host("Party strip"), Some(Host::Dock(Rect::new(0, 0, 500, 54))));
    assert_eq!(layout.host("Initiative"), Some(Host::Dock(Rect::new(0, 54, 500, 24))));
    assert_eq!(layout.host("Minimap"), Some(Host::Dock(Rect::new(372, 78, 128, 96))));
    assert_eq!(layout.host("Side panel"), Some(Host::Dock(Rect::new(372, 174, 128, 226))));
    assert_eq!(layout.host("Combat log"), Some(Host::Dock(Rect::new(0, 364, 372, 36))));
    assert_eq!(layout.host("Viewport"), Some(Host::Dock(Rect::new(0, 78, 372, 286))));
    layout.arrange_for(&scene(), &ui, (500, 240));
    assert!(layout.visible("Initiative", &scene()));
    assert!(layout.visible("Minimap", &scene()));
    assert_eq!(layout.host("Minimap"), Some(Host::Dock(Rect::new(372, 78, 128, 16))));
    assert_eq!(layout.host("Side panel").unwrap().size().1, 146);
    layout.arrange_for(&scene(), &ui, (320, 202));
    assert!(!layout.visible("Initiative", &scene()));
    assert!(!layout.visible("Minimap", &scene()));
    assert_eq!(layout.host("Viewport").unwrap().size(), (192, 112));
    assert!(!layout.set_kind(&mut ui, "Viewport", HostKind::Window));
    assert!(!layout.set_kind(&mut ui, "Side panel", HostKind::Hidden));
    assert!(layout.set_kind(&mut ui, "Minimap", HostKind::Hidden));
    assert!(layout.set_kind(&mut ui, "Side panel", HostKind::Window));
    layout.set_host("Side panel", Host::Window { id: 123, size: (128, 146) });
    layout.arrange_for(&scene(), &ui, (500, 400));
    assert_eq!(layout.host("Viewport").unwrap().size().0, 372);
    let mut look = Look::default();
    for size in [[639, 480], [640, 479], [1, 1]] { assert!(Metrics::main(size, &look, Some(&scene())).classic); }
    assert!(!Metrics::main([640, 480], &look, Some(&scene())).classic);
    look.layout.preset = LayoutPreset::Classic; assert!(Metrics::main([1600, 1000], &look, Some(&scene())).classic);
}

#[test]
fn settings_roundtrip_bad_unknown_clamped_and_missing_monitor() {
    let path = std::env::temp_dir().join(format!("mt-panel-layout-{}.toml", std::process::id()));
    let mut look = Look::default();
    look.layout.preset = LayoutPreset::Classic; look.layout.log_lines = 17;
    for id in ["Party strip", "Side panel", "Combat log", "Viewport", "Minimap", "Initiative", "Picture"] {
        look.layout.components.insert(id.into(), ComponentSettings { host: HostKind::Window, window_pos: Some([-1000, 27]), window_size: Some([700, 200]), size: Some([192, 90]) });
    }
    look.save(&path).unwrap(); assert_eq!(Look::load(&path), look);
    std::fs::write(&path, "bold=true\n[layout]\nlog_lines=255\nunknown=17\n[layout.components.Viewport]\nhost='window'\nwindow_size=[4294967295,1]\n[layout.components.Unknown]\nhost='hidden'\n").unwrap();
    let mut loaded = Look::load(&path); assert!(loaded.bold); assert_eq!(loaded.layout.log_lines, 20);
    assert_eq!(loaded.layout.components["Viewport"].window_size, Some([8192, 1]));
    let mut ui = UiState::default(); loaded.layout.restore(&picture_layout(), &mut ui);
    assert!(!loaded.layout.components.contains_key("Unknown")); assert_eq!(ui.host_kind("Viewport"), HostKind::Docked);
    std::fs::write(&path, "bold=true\n[layout.components.Unknown]\nhost=[1,2,3]\n").unwrap();
    assert!(Look::load_for_layout(&path, &picture_layout()).bold);
    assert!(Look::load_for_layout(&path, &picture_layout()).layout.components.is_empty());
    for bad in ["garbage", "[layout]\nlog_lines=-1", "[layout]\npreset='garbage'", "[layout.components.P]\nhost='bad'", "[layout.components.P]\nwindow_size=[-1,2]"] {
        std::fs::write(&path, bad).unwrap(); assert_eq!(Look::load(&path), Look::default());
    }
    for bad in [format!("bold=true\nunknown={}0{}", "[".repeat(128), "]".repeat(128)), format!("bold=true\nunknown={}0{}", "{ x = ".repeat(128), "}".repeat(128))] {
        std::fs::write(&path, bad).unwrap(); assert_eq!(Look::load(&path), Look::default());
    }
    assert_eq!(visible_position(Some([i32::MAX, i32::MIN]), [([0, 0], [1920, 1080])]), None);
    assert_eq!(visible_position(Some([-1000, 27]), [([-1920, 0], [1920, 1080])]), Some([-1000, 27]));
    assert_eq!(visible_position(Some([20, 27]), []), None);
    assert_eq!(visible_position(Some([1920, 20]), [([0, 0], [1920, 1080])]), None);
    std::fs::remove_file(path).unwrap();
}

struct Echo(u16);
impl Component for Echo {
    fn draw(&self, _: &Scene, _: &UiState, s: &mut Surface) { s.indexed.fill(self.0 as u8); }
    fn input(&self, _: &Scene, _: &UiState, _: &UiEvent, _: (i32, i32)) -> Vec<Intent> { vec![Intent::Raw(Event::Key(self.0))] }
}
#[test]
fn float_z_order_capture_and_host_blind_input() {
    let mut layout = Layout::default(); let mut ui = UiState::default();
    layout.register(Box::new(Echo(1)), Placement { dock: Dock::Float(Anchor::Pointer), origin: Origin::New, ..Picture::placement("float", Rect::new(0, 0, 24, 24)) });
    layout.register(Box::new(Echo(2)), Picture::placement("fill", FRAME));
    ui.menu = Some((mt1::ui::Target::Cell((0, 0)), (9999, -10)));
    layout.arrange_for(&scene(), &ui, (320, 200));
    assert_eq!(layout.host("float"), Some(Host::Dock(Rect::new(296, 0, 24, 24))));
    assert_eq!(layout.hit(&scene(), None, (300, 10)).unwrap().0, "float");
    assert_eq!(layout.route(&scene(), &mut ui, None, UiEvent::Down(Button::Left), (300, 10)), [Intent::Raw(Event::Key(1))]);
    assert_eq!(layout.route(&scene(), &mut ui, None, UiEvent::Move, (20, 100)), [Intent::Raw(Event::Key(1))]);
    layout.route(&scene(), &mut ui, None, UiEvent::Up(Button::Left), (20, 100)); assert!(ui.capture.is_none());
    let dock = layout.input("fill", &scene(), &ui, &UiEvent::Key(32), (7, 9));
    layout.set_host("fill", Host::Window { id: 9, size: (320, 200) });
    assert_eq!(layout.input("fill", &scene(), &ui, &UiEvent::Key(32), (7, 9)), dock);
    assert_eq!(layout.hit(&scene(), Some(9), (7, 9)).unwrap().0, "fill");
    assert!(layout.hit(&scene(), Some(10), (7, 9)).is_none());
    ui.menu_host = Some("fill");
    layout.arrange_for(&scene(), &ui, (500, 400));
    assert_eq!(layout.host_window("float"), Some(9));
    assert!(layout.draw_host(&scene(), &ui, None).is_empty());
    assert_eq!(layout.draw_host(&scene(), &ui, Some(9)).len(), 2);
    assert_eq!(layout.hit(&scene(), Some(9), (300, 10)).unwrap().0, "float");
    ui.menu_host = None; ui.preset = Preset::Classic;
    layout.arrange_for(&scene(), &ui, (320, 200));
    assert_eq!(layout.input("fill", &scene(), &ui, &UiEvent::Key(32), (0, 0)), [Intent::Raw(Event::Key(32))]);
}

#[test]
fn fractional_boundaries_hit_the_pixel_actually_painted() {
    let mut layout = picture_layout(); let ui = UiState::default(); let scene = scene();
    let look = Look { shape: Shape::Square, ..Default::default() };
    let m = Metrics::main([997, 631], &look, Some(&scene));
    layout.arrange_for(&scene, &ui, m.logical);
    for id in layout.ids().into_iter().filter(|id| layout.visible(id, &scene)) {
        let Host::Dock(r) = layout.host(id).unwrap() else { continue; };
        let p = m.rect(r);
        for point in [(p.x, p.y), (p.x + p.width - 1, p.y + p.height - 1)] {
            let hit = layout.hit_mapped(&scene, None, (point.0 as i32, point.1 as i32), |r| { let p = m.rect(r); Rect::new(p.x as i32, p.y as i32, p.width as u32, p.height as u32) });
            assert_eq!(hit.unwrap().0, id);
        }
        for at in [(2, 2), (r.w as i32 - 3, r.h as i32 - 3)] {
            for screen in [Screen::None, Screen::Crt] {
                let look = Look { screen, ..look.clone() };
                let projected = m.project(r, at, &look).unwrap();
                let pixel = m.unwarp(projected, &look, false).unwrap();
                assert_eq!(m.local(pixel, r, &look), at);
            }
        }
    }
}

#[test]
fn component_effects_once_and_pointer_samples_match_render() {
    for filter in [Filter::Nearest, Filter::Sharp, Filter::Soft] {
        let look = Look { shape: Shape::Square, filter, ..Default::default() };
        let metrics = Metrics::component([517, 313], (320, 200), &look);
        let r = Rect::new(7, 9, 123, 71); let p = metrics.rect(r);
        let colors: Vec<u32> = (0..r.w * r.h).collect();
        let nearest = pipeline::scale::scale_surface(&colors, r.w as usize, r.h as usize, p.width, p.height, &Look { filter: Filter::Nearest, ..look.clone() });
        if filter == Filter::Nearest {
            for y in (0..p.height).step_by(7) { for x in (0..p.width).step_by(7) {
                let at = metrics.local(((p.x + x) as i32, (p.y + y) as i32), r, &look);
                assert_eq!(nearest[y * p.width + x], (at.1 as u32 * r.w + at.0 as u32));
            } }
        }
    }
    let look = Look { shape: Shape::Square, ..Default::default() };
    let metrics = Metrics::component([320, 200], (320, 200), &look);
    let mut left = Surface::new(160, 200); left.indexed.fill(4);
    let mut right = Surface::new(160, 200); right.indexed.fill(2);
    let surfaces = vec![(Rect::new(0, 0, 160, 200), left), (Rect::new(160, 0, 160, 200), right)];
    let flat = render::compose(surfaces.clone(), metrics, &look);
    let crt = Look { screen: Screen::Crt, ..look };
    assert_eq!(render::compose(surfaces, metrics, &crt), pipeline::screen::apply(&flat, 320, 200, &crt));
    let tube = pipeline::screen::apply(&vec![0xffffff; 64000], 320, 200, &crt);
    for y in (0..200).step_by(5) { for x in (0..320).step_by(5) { assert_eq!(metrics.unwarp((x as f64, y as f64), &crt, false).is_some(), tube[y * 320 + x] != 0); } }
    assert!(metrics.unwarp((f64::NAN, 0.0), &crt, false).is_none());
}

#[test]
fn one_scale_across_exploration_squad_turn_and_npc_turn() {
    let look = Look { shape: Shape::Classic, scale: ScaleMode::Integer, ..Default::default() };
    let explore = scene();
    let mut turn = scene(); turn.squad.active = true; turn.squad.encounter = true; turn.squad.round = 1; turn.squad.acting = Some(Unit::Traveller(0));
    let mut npc = turn.clone(); npc.squad.acting = Some(Unit::Npc(14));
    for size in [[1531, 1221], [640, 480]] {
        let m = [&explore, &turn, &npc].map(|s| Metrics::main(size, &look, Some(s)));
        for x in &m {
            assert!(!x.classic, "{size:?}");
            assert_eq!((x.logical, x.pixels.width, x.pixels.height), (m[0].logical, m[0].pixels.width, m[0].pixels.height), "{size:?}: one scale across ground frames");
            assert!(x.logical.1 >= 54 + 22 + 146 + 36, "{size:?}: party strip, combat bar, side panel with its Vehicle row and the commands fit");
        }
    }
}

#[test]
fn ground_rects_equal_in_exploration_fight_npc_turn_and_after_victory() {
    use mt_engine::play::{Decision, Kind, Option as Choice};
    let look = Look { shape: Shape::Classic, scale: ScaleMode::Integer, ..Default::default() };
    let mut ground = Decision::new(Kind::Ground, "t", "6923"); ground.options.push(Choice::new("cmd:game", "Game", vec![Event::Key(0x122)]));
    let combat = Decision::new(Kind::CombatTurn, "t", "combat.turn");
    let mut explore = scene(); explore.squad.active = true;
    let mut turn = explore.clone(); turn.squad.encounter = true; turn.squad.round = 2; turn.squad.acting = Some(Unit::Traveller(0)); turn.squad.queue = vec![Unit::Traveller(0), Unit::Npc(14)];
    let mut npc = turn.clone(); npc.squad.acting = Some(Unit::Npc(14));
    let mut victory = explore.clone(); victory.squad.strip = "Victory".into(); victory.squad.round = 3;
    let ids = ["Party strip", "Combat controls", "Initiative order", "Viewport", "Side panel", "Combat log", "Commands"];
    for size in [[1531, 1221], [640, 480]] {
        let rects: Vec<Vec<_>> = [(&explore, &ground), (&turn, &combat), (&npc, &combat), (&victory, &ground)].into_iter().map(|(s, d)| {
            let ui = UiState { decision: Some(d.clone()), ..Default::default() };
            let m = Metrics::for_ui(size, &look, Some(s), &ui); assert!(!m.classic);
            let mut layout = product_layout(); layout.arrange_for(s, &ui, m.logical);
            ids.into_iter().map(|id| {
                assert!(layout.visible(id, s), "{id} missing at {size:?} with {:?}", s.squad.strip);
                let Some(Host::Dock(r)) = layout.host(id) else { panic!("{id}") };
                if id == "Viewport" { assert!(r.w >= 192 && r.h >= 112, "the 7 x 12 view fits: {r:?}"); }
                if id == "Side panel" { assert!(r.h >= 146, "Vehicle row whole: {r:?}"); }
                (id, r, m.rect(r))
            }).collect()
        }).collect();
        for r in &rects[1..] { assert_eq!(r, &rects[0], "{size:?}"); }
    }
}
