#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use std::{cell::RefCell, path::PathBuf, rc::Rc};
use megatraveller::{look::{FontChoice, Look}, pipeline::{self, Frame, text::shown}};
use mt_engine::{data::{exepack, ly}, io::{Input, Key, Overlay, keys}, present::{fb::Fb, palette, prim::Raster, text::{self, Font}, textlog}, sim::tick::Counter};
use mt1::{screens::cg::Screens, ui::CgUi};

fn gamefiles() -> Option<PathBuf> { oracle_env::gamefiles() }
fn read(dir: &PathBuf, name: &str) -> Vec<u8> {
    let path = std::fs::read_dir(dir).unwrap().flatten().find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(name)).unwrap().path();
    std::fs::read(path).unwrap()
}
fn cells(font: &Font) -> Vec<Vec<u8>> {
    (0..256).map(|c| {
        let mut fb = Fb::new();
        text::draw(&mut fb, Raster::Clip, 0, font, &[c as u8], 1, 1, 15, None);
        let px = fb.indexed(0);
        (0..12).flat_map(|y| px[y * 320..y * 320 + 16].to_vec()).collect()
    }).collect()
}
fn ink_rows(cell: &[u8]) -> usize { cell.chunks(16).filter(|r| r.iter().any(|&p| p != 0)).count() }

#[test]
fn font_table_inventory_matches_shown() {
    let Some(dir) = gamefiles() else { return };
    let font = Font::new(ly::decode(&read(&dir, "FONT.LY")).unwrap(), [8; 256], &[0; 256], 0);
    let cells = cells(&font);
    let drawn: Vec<usize> = (0..256).filter(|&c| ink_rows(&cells[c]) > 0).collect();
    let expected: Vec<usize> = (0x00..=0x0A).chain(0x21..=0x5A).chain(0x61..=0x7A).chain(0xAF..=0xB9).chain(0xD0..=0xD9).collect();
    assert_eq!(drawn, expected, "FONT.LY inventory changed: re-audit pipeline::text::shown");
    for c in 0..256u16 {
        let c = c as u8;
        let text = (0x20..=0x5A).contains(&c) || (0x61..=0x7A).contains(&c) || (0xAF..=0xB9).contains(&c) || (0xD0..=0xD9).contains(&c);
        assert_eq!(shown(c).is_some(), text, "code {c:02X}: every text glyph and only text glyphs get a TTF character");
    }
    for d in 0..10u8 {
        assert_eq!((shown(0xB0 + d), shown(0xD0 + d)), (Some((b'0' + d) as char), Some((b'0' + d) as char)));
        assert_eq!((ink_rows(&cells[0xB0 + d as usize]), ink_rows(&cells[0xD0 + d as usize])), (5, 11), "digit {d}");
        assert!(font.descend[0xB0 + d as usize] && !font.descend[0xD0 + d as usize]);
    }
    assert_eq!((shown(0xAF), ink_rows(&cells[0xAF]), font.descend[0xAF]), (Some('/'), 5, true));
}

#[test]
fn tall_digits_restore_their_entire_bitmap_before_plus_overdraw() {
    let Some(dir) = gamefiles() else { return };
    textlog::set_enabled(true);
    let ds = mt1::ground::state::ds_image(&read(&dir, "T.DAT")).unwrap();
    let font = Font::from_tables(ly::decode(&read(&dir, "FONT.LY")).unwrap(), &ds, 0x5128, 0x5637, 8).unwrap();
    for c in 0xD0..=0xD9 { for shadow in [None, Some(4)] {
        let mut fb = Fb::new(); text::draw(&mut fb, Raster::Clip, 0, &font, &[c], 20, 10, 15, shadow);
        let frame = Frame::from_rgb(&fb.rgb(0, &palette::ega_dac()));
        let clean = pipeline::text::background(&frame.indexed, &frame.text, &Look::default());
        let leftover: Vec<_> = clean.iter().enumerate().filter(|(_, p)| **p != 0).map(|(i, _)| (i % 320, i / 320)).collect();
        assert!(leftover.is_empty(), "native tall digit {c:02x}, shadow {shadow:?}, leaves ink outside its annotation: {leftover:?}");
    } }
    textlog::set_enabled(false);
}

struct Capture { frames: Rc<RefCell<Vec<Frame>>> }
impl Input for Capture {
    fn next_key(&mut self) -> Key { keys::ESC }
    fn present(&mut self, rgb: &[u32]) { self.frames.borrow_mut().push(Frame::from_rgb(rgb)); }
}
#[test]
fn view_character_sheet_skill_levels_are_digits_in_ttf_faces() {
    let Some(dir) = gamefiles() else { return };
    textlog::set_enabled(true);
    let frames = Rc::new(RefCell::new(Vec::new()));
    let mut s = Screens::new(exepack::unpack(&read(&dir, "CG.DAT")).unwrap().exe, ly::decode(&read(&dir, "FONT.LY")).unwrap(),
        ly::decode(&read(&dir, "CG.LY")).unwrap(), Box::new(Capture { frames }), Box::new(Counter::default()), Box::new(Overlay::new(Some(&dir)))).unwrap();
    let pool = read(&dir, "DCHARS.DAT");
    let records: Vec<&[u8]> = pool[1..].chunks_exact(256).take(pool[0] as usize).collect();
    let rec = records.iter().copied().find(|r| r[0x2A..0x2A + 79].contains(&1) && r[0x2A..0x2A + 79].contains(&4)).unwrap_or(records[0]);
    CgUi::layout(&mut s, 19, 0, 0);
    for (row, col, label, color) in s.ds.text_list(0x2CEA) { CgUi::move_to(&mut s, row, col); CgUi::set_text_color(&mut s, color as i16); CgUi::text(&mut s, &label) }
    CgUi::character_sheet(&mut s, 0, rec);
    let frame = Frame::from_rgb(&s.fb.rgb(0, &palette::ega_dac()));
    textlog::set_enabled(false);
    let levels: Vec<u8> = frame.text.visible.iter().filter(|r| (0xB0..=0xB9).contains(&r.bytes[0])).map(|r| r.bytes[0]).collect();
    assert!(levels.contains(&0xB1) || levels.contains(&0xB4), "skill levels logged as FONT.LY small digits: {levels:02X?}");
    for r in &frame.text.visible { assert!(shown(r.bytes[0]).is_some() || r.bytes[0] <= 0x0A, "unmapped text code {:02X} at ({},{})", r.bytes[0], r.x, r.y); }
    let mut ascii = Frame { text: frame.text.clone(), regs: frame.regs, ..Frame::default() }; ascii.indexed = frame.indexed;
    for r in &mut ascii.text.visible { if (0xB0..=0xB9).contains(&r.bytes[0]) { r.bytes[0] -= 0x80 } }
    let proof = std::env::var_os("MT_GLYPH_PROOF").map(PathBuf::from);
    for (name, font) in [("px437", FontChoice::Builtin(0)), ("ibm-plex-mono", FontChoice::Builtin(1)), ("vt323", FontChoice::Builtin(2)), ("original", FontChoice::Original)] {
        let look = Look { font, ..Look::default() };
        let out = pipeline::render(&frame, &look, 1280, 960);
        assert!(out == pipeline::render(&ascii, &look, 1280, 960), "{name}: level cells differ from ASCII digit cells");
        if let Some(dir) = proof.as_ref().filter(|_| look.font == Look::default().font) {
            std::fs::create_dir_all(dir).unwrap();
            let mut ppm = b"P6\n1280 960\n255\n".to_vec();
            for c in out { ppm.extend_from_slice(&[(c >> 16) as u8, (c >> 8) as u8, c as u8]); }
            std::fs::write(dir.join(format!("sheet-{name}.ppm")), ppm).unwrap();
        }
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
