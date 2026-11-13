#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use std::{cell::RefCell, collections::VecDeque, path::PathBuf, rc::Rc};
use megatraveller::{look::{FontChoice, Look}, pipeline::{self, Frame}};
use mt_engine::{data::{exepack, ly}, io::{Input, Key, Overlay, keys}, present::textlog, sim::tick::Counter};
use mt1::{screens::cg::Screens, ui::{CgUi, Menu}};

struct Capture { frames: Rc<RefCell<Vec<Frame>>>, keys: VecDeque<Key> }
impl Input for Capture {
    fn next_key(&mut self) -> Key { self.keys.pop_front().unwrap_or(keys::ESC) }
    fn present(&mut self, rgb: &[u32]) { self.frames.borrow_mut().push(Frame::from_rgb(rgb)); }
}

#[test]
fn successive_welcome_menus_restore_clean_backgrounds() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let read = |name: &str| {
        let path = std::fs::read_dir(&dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).unwrap().path();
        std::fs::read(path).unwrap()
    };
    textlog::set_enabled(true);
    let frames = Rc::new(RefCell::new(Vec::new()));
    let input = Capture { frames: frames.clone(), keys: [keys::ENTER, keys::ESC, keys::DOWN, keys::DOWN, keys::DOWN, keys::ENTER].into() };
    let mut screens = Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe,
        ly::decode(&read("FONT.LY")).unwrap(), ly::decode(&read("CG.LY")).unwrap(),
        Box::new(input), Box::new(Counter::default()), Box::new(Overlay::new(Some(&dir)))).unwrap();
    assert_eq!(mt1::screens::cg::run(&mut screens).0, mt1::chargen::pool::Exit::Dos);
    textlog::set_enabled(false);
    let frames = frames.borrow();
    let expected = "Use My Party DiskCreate Party DiskUse Default Characters";
    let frame = frames.iter().find(|f| f.text.runs.iter().map(|r| r.bytes[0] as char).collect::<String>().contains("Use My Party Disk")).unwrap();
    let visible: String = frame.text.visible.iter().map(|r| r.bytes[0] as char).collect();
    assert_eq!(visible, expected);
    let look = Look { font: FontChoice::Builtin(1), ..Look::default() };
    let main = "WELCOME TO MEGATRAVELLERStart New GameContinue Old GameGet Started QuickReturn to DOS";
    for f in frames.iter() {
        let text: String = f.text.visible.iter().map(|r| r.bytes[0] as char).collect();
        assert!(text == main || text == expected, "selection lost or resurrected text: {text}");
        let clean = pipeline::text::background(&f.indexed, &f.text, &look);
        for y in 20..30 { for x in 0..320 { assert_eq!(clean[y * 320 + x], 2, "banner bitmap at {x},{y}"); } }
    }
    let clean = pipeline::text::background(&frame.indexed, &frame.text, &look);
    for y in 20..30 { for x in 0..320 { assert_eq!(clean[y * 320 + x], 2, "banner bitmap at {x},{y}"); } }
    for y in 82..106 { for x in 33..166 {
        assert_eq!(clean[y * 320 + x], if y < 90 { 4 } else { 0 }, "menu bitmap at {x},{y}");
    } }
    if let Some(dir) = std::env::var_os("MT_TEXT_PROOF") {
        let dir = PathBuf::from(dir); std::fs::create_dir_all(&dir).unwrap();
        for (step, name, captured) in [(0, "welcome", &frames[0]), (1, "party", frame)] {
            let planes = std::fs::read(dir.join(format!("dump/step_{step:04}/planes.bin"))).unwrap();
            let regs = std::fs::read(dir.join(format!("dump/step_{step:04}/regs.bin"))).unwrap();
            let mut fb = mt_engine::present::fb::Fb::new(); fb.planes = planes; fb.regs.copy_from_slice(&regs);
            assert_eq!(captured.indexed.as_slice(), fb.indexed(0));
            assert_eq!(captured.regs, fb.regs);
            let proof = Frame { indexed: fb.indexed(0).try_into().unwrap(), regs: fb.regs, text: captured.text.clone() };
            let mut ppm = b"P6\n1280 960\n255\n".to_vec();
            for c in pipeline::render(&proof, &Look::default(), 1280, 960) { ppm.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]); }
            std::fs::write(dir.join(format!("{name}-px437.ppm")), ppm).unwrap();
        }
    }
}
#[test]
fn actual_menu_keeps_text_when_highlight_moves_and_font_changes() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let read = |name: &str| {
        let path = std::fs::read_dir(&dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).unwrap().path();
        std::fs::read(path).unwrap()
    };
    textlog::set_enabled(true);
    let frames = Rc::new(RefCell::new(Vec::new()));
    let input = Capture { frames: frames.clone(), keys: [keys::DOWN, keys::ESC].into() };
    let mut screens = Screens::new(exepack::unpack(&read("CG.DAT")).unwrap().exe,
        ly::decode(&read("FONT.LY")).unwrap(), ly::decode(&read("CG.LY")).unwrap(),
        Box::new(input), Box::new(Counter::default()), Box::new(Overlay::new(Some(&dir)))).unwrap();
    screens.set_text_color(15);
    screens.draw_text_at(0, 10, 10, 8, 0, "MegaTraveller - gypqj WMi");
    screens.set_line_height(8); screens.set_text_color(15);
    let items = ["Generate character", "View characters", "Return to DOS"];
    assert_eq!(screens.menu(&Menu { items: &items, highlight: 4, row: 40, col: 30, allow_unique: 1,
        border: 2, color: 0, extra: &[1, 0, 3], initial: -1 }), -1);
    textlog::set_enabled(false);
    let frames = frames.borrow(); assert!(frames.len() >= 2);
    let expected = items.concat().len() + "MegaTraveller - gypqj WMi".len();
    for frame in frames.iter().take(2) {
        let missing: Vec<_> = frame.text.runs.iter().filter(|r| !frame.text.visible.iter().any(|v| v.x == r.x && v.y == r.y)).map(|r| (r.x, r.y, r.row(), r.width, r.bytes.clone())).collect();
        assert_eq!(frame.text.visible.len(), expected, "every menu/header glyph survives selection: missing {missing:?}");
        let original = pipeline::render(frame, &Look { font: FontChoice::Original, ..Look::default() }, 960, 720);
        for font in std::iter::once(FontChoice::Original).chain((0..megatraveller::font_menu::BUILTINS.len() as u8).map(FontChoice::Builtin)) {
            let look = Look { font, ..Look::default() };
            let rendered = pipeline::render(frame, &look, 960, 720);
            if look.font != FontChoice::Original { assert!(rendered != original, "{:?} must replace bitmap glyphs", look.font); }
            if let Some(dir) = std::env::var_os("MT_TEXT_PREVIEW").filter(|_| look.font == Look::default().font) {
                let dir = PathBuf::from(dir); std::fs::create_dir_all(&dir).unwrap();
                let mut ppm = b"P6\n960 720\n255\n".to_vec();
                for c in rendered { ppm.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]); }
                std::fs::write(dir.join("font-px437.ppm"), ppm).unwrap();
            }
        }
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
