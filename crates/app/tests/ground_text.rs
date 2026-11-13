#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use std::{path::{Path, PathBuf}, cell::RefCell, rc::Rc};
use megatraveller::{pipeline::Frame, scenario};
use mt_engine::{io::{Input, Event, Key}, sim::tick::Clock, present::textlog};
use mt1::ground::{self, render};

struct Boundary;
struct Capture(Rc<RefCell<Frame>>);
impl Input for Capture {
    fn next_key(&mut self) -> Key { std::panic::resume_unwind(Box::new(Boundary)) }
    fn poll(&mut self, _: &mut dyn Clock) -> Event { std::panic::resume_unwind(Box::new(Boundary)) }
    fn present(&mut self, rgb: &[u32]) { *self.0.borrow_mut() = Frame::from_rgb(rgb); }
}
fn game() -> Option<ground::Ground> {
    let dir = oracle_env::gamefiles()?;
    textlog::set_enabled(true);
    Some(scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_quiet.toml"), &dir).unwrap().game)
}
fn rows(f: &Frame) -> String {
    let mut runs = f.text.visible.clone(); runs.sort_by_key(|r| (r.y, r.x));
    let mut lines = std::collections::BTreeMap::<i64,String>::new();
    for r in runs { lines.entry(r.y).or_default().push_str(&String::from_utf8_lossy(&r.bytes)); }
    format!("overflow={} {lines:?}", f.text.overflowed)
}
fn assert_face_applies(frame: &Frame, boxes: &[(usize, usize, usize, usize)]) {
    use megatraveller::{look::{Look, FontChoice, Shape}, pipeline};
    let look = Look { shape: Shape::Square, ..Look::default() };
    let plus = pipeline::render(frame, &look, 1280, 800);
    let bitmap = pipeline::render(frame, &Look { font: FontChoice::Original, ..look }, 1280, 800);
    for &(x, y, w, h) in boxes {
        assert!((y * 4..(y + h) * 4).any(|py| (x * 4..(x + w) * 4).any(|px| plus[py * 1280 + px] != bitmap[py * 1280 + px])),
            "selected face did not replace native glyphs in {x},{y} {w}x{h}");
    }
}
#[test]
fn first_ground_frame_retains_side_panel_text() {
    let Some(mut g) = game() else { return };
    let frame = Rc::new(RefCell::new(Frame::default())); g.input = Box::new(Capture(frame.clone()));
    let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ground::main_loop::run(&mut g)));
    assert!(stop.is_err_and(|e| e.is::<Boundary>()));
    let frame = frame.borrow(); let text = rows(&frame);
    assert!(text.contains("system:") && text.contains("Converse"), "{text}");
    assert_face_applies(&frame, &[(261, 141, 56, 8), (261, 155, 56, 8), (266, 147, 48, 8), (266, 161, 48, 8),
        (200, 171, 58, 8), (262, 171, 53, 8), (200, 181, 58, 8), (262, 181, 53, 8), (200, 191, 58, 8)]);
    textlog::set_enabled(false);
}
#[test]
fn character_sheet_retains_skills_and_record_text() {
    let Some(mut g) = game() else { return };
    render::character::sheet(&mut g, 0, true);
    let frame = Frame::from_rgb(&g.fb.rgb((g.mem.u16(0x52F4) & 1) as usize, &g.dac));
    let text = rows(&frame);
    assert!(frame.text.visible.iter().any(|r| r.x == 5 && r.y >= 66), "skill annotations missing: {text}");
    let objects: String = frame.text.visible.iter().filter(|r| r.y == 108 && (98..165).contains(&r.x)).flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
    assert_eq!(objects.to_ascii_lowercase(), "objects");
    assert_face_applies(&frame, &[(5, 66, 70, 8), (5, 90, 70, 8), (186, 76, 110, 8), (98, 108, 65, 8)]);
    let look = megatraveller::look::Look { shape: megatraveller::look::Shape::Square, ..Default::default() };
    let plus = megatraveller::pipeline::render(&frame, &look, 1280, 800);
    let bitmap = megatraveller::pipeline::render(&frame, &megatraveller::look::Look { font: megatraveller::look::FontChoice::Original, ..look }, 1280, 800);
    for y in 117 * 4..132 * 4 { for x in 98 * 4..120 * 4 {
        assert_eq!(plus[y * 1280 + x], bitmap[y * 1280 + x], "Objects must not draw over inventory art");
    } }
    textlog::set_enabled(false);
}

#[test]
fn snapshot_first_frame_retains_side_panel_text() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let target = std::env::var_os("CARGO_TARGET_DIR").map(PathBuf::from).unwrap_or_else(|| root.join("target"));
    let out = target.join(format!("ground-text-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    std::fs::create_dir_all(&out).unwrap();
    let route = out.join("route.json");
    std::fs::write(&route, serde_json::json!({"start":root.join("scenarios/efate_quiet.toml"),"start_tick":1234,"steps":[{"snapshot":"002","frame":true}]}).to_string()).unwrap();
    assert_eq!(megatraveller::mcp::executor::execute(&route, false, &out.join("prepared"), dir.clone(), out.clone()), 0);
    let frame = Rc::new(RefCell::new(Frame::default())); let captured = frame.clone();
    let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| megatraveller::mcp::launch::file_desktop(&out.join("prepared/snapshots/named-002.json"), &dir, &out, Default::default(), move |s| {
        Ok((Box::new(Capture(captured)), Box::new(mt_engine::sim::tick::Counter::at(s.ticks))))
    })));
    assert!(stop.is_err_and(|e| e.is::<Boundary>()));
    let frame = frame.borrow(); let text = rows(&frame);
    assert!(text.contains("system:") && text.contains("Converse"), "{text}");
    assert_face_applies(&frame, &[(261, 141, 56, 8), (200, 171, 58, 8)]);
    textlog::set_enabled(false);
}

#[test]
fn emblem_rank_numerals_stay_inside_the_icons_in_the_box_and_on_the_strip() {
    use megatraveller::{look::{FontChoice, Look, Shape}, panels::render::{compose, Metrics}, pipeline::{self, text::badge}};
    let Some(dir) = oracle_env::gamefiles() else { return };
    textlog::set_enabled(true);
    let assets = megatraveller::panels::assets::Assets::load(&mut mt_engine::io::Dir(dir));
    let mut scene = mt1::ui::Scene::program(mt1::ui::ScreenKind::Ground);
    scene.party.0 = vec![0; 0x1000]; scene.party.0[0] = 1; scene.party.0[0x127] = 5; scene.party.0[0x122] = 4; scene.party.0[0x123] = 4;
    let out = assets.party_box(&scene);
    let digit = out.text.visible.iter().find(|r| r.bytes == [0xB4]).expect("the rank numeral is drawn").clone();
    assert!(badge(&digit, &out.text), "the numeral sits on its emblem: {:?}", out.text.visible.iter().map(|r| (r.x, r.y, r.bytes[0])).collect::<Vec<_>>());
    let look = Look { shape: Shape::Square, ..Look::default() };
    let metrics = Metrics::component([256, 216], (64, 54), &look);
    let plus = compose([(megatraveller::panels::Rect::new(0, 0, 64, 54), out.clone())], metrics, &look);
    let bitmap = compose([(megatraveller::panels::Rect::new(0, 0, 64, 54), out.clone())], metrics, &Look { font: FontChoice::Original, ..look.clone() });
    let (w, s) = (256usize, 4usize);
    let same = |x: i64, y: i64, cw: i64, ch: i64| (y * s as i64..(y + ch) * s as i64).all(|py| (x * s as i64..(x + cw) * s as i64).all(|px| plus[py as usize * w + px as usize] == bitmap[py as usize * w + px as usize]));
    assert!(same(digit.x - 1, digit.y - 1, digit.width as i64 + 2, 10), "the box numeral is the 1990 bitmap in the selected face");
    let Some(mut g) = game() else { return };
    let frame = Rc::new(RefCell::new(Frame::default())); g.input = Box::new(Capture(frame.clone()));
    let stop = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ground::main_loop::run(&mut g)));
    assert!(stop.is_err_and(|e| e.is::<Boundary>()));
    let frame = frame.borrow();
    let numerals: Vec<_> = frame.text.visible.iter().filter(|r| r.y < 54 && r.bytes.first().is_some_and(|c| (0xB0..=0xB9).contains(c))).cloned().collect();
    assert!(!numerals.is_empty(), "the strip shows rank numerals: {}", rows(&frame));
    assert!(numerals.iter().all(|r| badge(r, &frame.text)), "every strip numeral sits on an emblem");
    let plus = pipeline::render(&frame, &look, 1280, 800); let bitmap = pipeline::render(&frame, &Look { font: FontChoice::Original, ..look.clone() }, 1280, 800);
    for r in &numerals { for py in (r.y - 1).max(0) * 4..(r.y + 9) * 4 { for px in (r.x - 1).max(0) * 4..(r.x + r.width as i64 + 1) * 4 {
        assert_eq!(plus[py as usize * 1280 + px as usize], bitmap[py as usize * 1280 + px as usize], "strip numeral at {},{} re-laid", r.x, r.y);
    } } }
    assert_face_applies(&frame, &[(261, 141, 56, 8)]);
    textlog::set_enabled(false);
}

#[test]
fn generated_labels_follow_the_picture_page() {
    let mut out = megatraveller::panels::Surface::new(64, 24); out.text.page = 1;
    megatraveller::panels::assets::Assets::default().text(&mut out, "END TURN", (2, 3), 15);
    assert_eq!(out.text.visible.len(), 8);
    assert!(out.text.visible.iter().all(|r| r.page == out.text.page), "labels on the back page bypass the face");
}

#[test]
fn item_menu_esc_row_reads_back_in_product_mode_only() {
    use mt1::{ground::{input::menu::Menu, state::Mem}, play::menus::af79};
    let Some(mut g) = game() else { return };
    let frame = Rc::new(RefCell::new(Frame::default())); g.input = Box::new(Capture(frame.clone()));
    let table = Mem::ds(0x4568);
    let m = Menu::read(&g.mem, table).expect("the Item menu table");
    scenario::attach_scene(&mut g);
    render::draw_menu(&mut g, table); render::present_menu(&mut g);
    let text = rows(&frame.borrow()); assert!(text.contains("Back") && !text.contains("ESC"), "product rows: {text}");
    let d = af79(&g, table, &m, 0, &mt1::ground::input::menu::rectangles(&g, Mem::ds(0x480B), m.labels.len())); let labels: Vec<&str> = d.options.iter().map(|o| o.label.as_str()).collect();
    assert_eq!(labels.iter().filter(|l| **l == "Back").count(), 1, "one Back: {labels:?}"); assert!(!labels.contains(&"ESC"));
    g.scene = None;
    render::draw_menu(&mut g, table); render::present_menu(&mut g);
    let text = rows(&frame.borrow()); assert!(text.contains("ESC") && !text.contains("Back"), "faithful rows: {text}");
    let d = af79(&g, table, &m, 0, &mt1::ground::input::menu::rectangles(&g, Mem::ds(0x480B), m.labels.len())); let labels: Vec<&str> = d.options.iter().map(|o| o.label.as_str()).collect();
    assert!(labels.contains(&"ESC") && labels.contains(&"Back"), "faithful keeps both: {labels:?}");
    textlog::set_enabled(false);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
