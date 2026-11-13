#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use std::sync::Arc;
use megatraveller::{look::{FontChoice, Look}, panels::{self, assets::Assets, Rect, Surface, UiState}, pipeline};
use mt1::ui::{Scene, ScreenKind};
use mt_engine::io::Overlay;

fn assets() -> Option<Arc<Assets>> {
    let dir = oracle_env::gamefiles()?;
    Some(Arc::new(Assets::load(&mut Overlay::new(Some(std::path::Path::new(&dir))))))
}
fn faces() -> Vec<FontChoice> {
    static FACES: std::sync::OnceLock<Vec<FontChoice>> = std::sync::OnceLock::new();
    FACES.get_or_init(|| {
    let dir = Look::path().with_file_name("fonts");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("g5-readability-file.ttf"), include_bytes!("../fonts/monospace_IBMPlexMono.ttf")).unwrap();
    let mut faces: Vec<_> = std::iter::once(FontChoice::Original).chain((0..megatraveller::font_menu::BUILTINS.len() as u8).map(FontChoice::Builtin)).collect();
    for entry in megatraveller::font_menu::Catalog::default().scan(&dir) {
        let font = FontChoice::File(entry.file); assert!(megatraveller::font_menu::selected(&font).is_some()); faces.push(font);
    }
        faces
    }).clone()
}
fn fight() -> Scene {
    let mut s = Scene::program(ScreenKind::Ground);
    s.squad.active = true; s.squad.encounter = true; s.combat = true; s.squad.mp = [10; 5];
    s.party.0 = vec![0; 0x1000];
    s.party.0[0x102..0x108].copy_from_slice(b"JURGEN");
    s.party.0[0x1CE + 42] = 1; s.party.0[0xA82] = 12; s.party.0[0xA83] = 4; s.party.0[0xA84] = 2;
    s.party.0[0x1CB..0x1CE].copy_from_slice(&[2, 0, 0]);
    s
}
fn luminance(rgb: u32) -> f64 {
    let linear = |s: u32| { let v = ((rgb >> s) & 255) as f64 / 255.; if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) } };
    0.2126 * linear(16) + 0.7152 * linear(8) + 0.0722 * linear(0)
}
fn ratio(a: u32, b: u32) -> f64 { let (a, b) = (luminance(a), luminance(b)); (a.max(b) + 0.05) / (a.min(b) + 0.05) }

#[test]
fn fight_side_panel_text_stays_inside_boxes_for_every_face_and_window_size() {
    let Some(assets) = assets() else { eprintln!("SKIP: MT1_GAMEFILES required"); return };
    let boxes = [Rect::new(5, 9, 121, 7), Rect::new(5, 17, 100, 7), Rect::new(108, 17, 18, 7),
        Rect::new(59, 27, 36, 9), Rect::new(98, 27, 27, 9), Rect::new(28, 41, 98, 7), Rect::new(5, 49, 121, 7),
        Rect::new(12, 57, 20, 7), Rect::new(34, 57, 20, 7), Rect::new(56, 57, 20, 7),
        Rect::new(12, 66, 20, 18), Rect::new(34, 66, 20, 18), Rect::new(56, 66, 20, 18),
        Rect::new(79, 57, 47, 9), Rect::new(79, 67, 47, 9), Rect::new(79, 77, 47, 9),
        Rect::new(78, 87, 49, 7), Rect::new(78, 94, 49, 6), Rect::new(78, 100, 49, 7), Rect::new(78, 107, 49, 7),
        Rect::new(67, 88, 8, 8), Rect::new(67, 102, 8, 8)];
    for size in [[1531, 1221], [640, 480]] { for font in faces() { for bold in [false, true] {
        let look = Look { font: font.clone(), bold, ..Look::default() }; let s = fight(); let ui = UiState::default();
        let metrics = panels::render::Metrics::main(size, &look, Some(&s)); assert!(!metrics.classic);
        let mut layout = panels::Layout::default(); panels::components::register(&mut layout, assets.clone());
        layout.arrange_for(&s, &ui, metrics.logical);
        let (_, host, out) = layout.draw(&s, &ui).into_iter().find(|(id, ..)| *id == "Side panel").unwrap();
        for r in &out.text.visible {
            let h = r.erase_height().min(8) as i64;
            assert!(boxes.iter().any(|b| r.x >= b.x as i64 && r.row() >= b.y as i64
                && r.x + r.width as i64 <= b.x as i64 + b.w as i64 && r.row() + h <= b.y as i64 + b.h as i64),
                "{:?} {size:?}: {:?} at {},{} + {}x{h} crosses its box", look.font, r.bytes, r.x, r.row(), r.width);
        }
        let mut rows: Vec<(i64, i64, char)> = out.text.visible.iter().map(|r| (r.y, r.x, r.bytes[0] as char)).collect(); rows.sort();
        let mut lines: Vec<(i64, String)> = Vec::new();
        for (y, _, c) in rows { match lines.last_mut() { Some((ly, t)) if (*ly - y).abs() <= 1 => t.push(c), _ => lines.push((y, c.to_string())) } }
        let texts: Vec<&str> = lines.iter().map(|(_, t)| t.as_str()).collect();
        for expected in ["Rounds 2", "expchmradfire", "MP 10", "Attack 0", "SEMI"] {
            assert!(texts.iter().any(|t| t.contains(expected)), "{:?} {size:?}: {expected:?} missing in {texts:?}", look.font);
        }
        let at = |needle: &str| lines.iter().position(|(_, t)| t.contains(needle)).unwrap();
        assert!(at("MP 10") < at("Attack 0") && at("Attack 0") < at("SEMI"), "readout order: {texts:?}");
        let panels::Host::Dock(rect) = host else { panic!("expected dock") };
        let mut clean = out.clone();
        pipeline::text::restore_background(&mut clean.indexed, (clean.w, clean.h), &clean.text, &look); clean.text = Default::default();
        let baseline = panels::render::compose([(rect, clean)], metrics, &look);
        let rendered = panels::render::compose([(rect, out.clone())], metrics, &look);
        assert_eq!(rendered.len(), size[0] as usize * size[1] as usize);
        let p = metrics.rect(rect);
        let edge = |v: i64, extent: usize, base: usize| (v as usize * extent).div_ceil(base);
        let pixels: Vec<_> = boxes.iter().map(|b| (p.x + edge(b.x as i64, p.width, out.w), p.y + edge(b.y as i64, p.height, out.h),
            p.x + edge(b.x as i64 + b.w as i64, p.width, out.w), p.y + edge(b.y as i64 + b.h as i64, p.height, out.h))).collect();
        let mut ink = 0;
        for (at, (a, b)) in rendered.iter().zip(&baseline).enumerate() { if a != b {
            ink += 1; let (x, y) = (at % size[0] as usize, at / size[0] as usize);
            assert!(pixels.iter().any(|&(l, t, r, b)| x >= l && x < r && y >= t && y < b), "{font:?} bold={bold} at {size:?}: ink outside boxes at {x},{y}");
        } }
        assert!(ink > 100);
    } } }
}

#[test]
fn high_contrast_text_has_wcag_aa_pairs_including_original_bitmap() {
    let Some(assets) = assets() else { eprintln!("SKIP: MT1_GAMEFILES required"); return };
    for font in faces() { for bold in [false, true] {
        for bg in [0, 1, 5, 7, 8, 15] {
            let mut surface = Surface::new(48, 16); surface.indexed.fill(bg);
            surface.regs[1] = 5; surface.regs[15] = 6; surface.text.regs = surface.regs;
            assets.text(&mut surface, "FIRE", (2, 2), 15);
            let look = Look { font: font.clone(), bold, shape: megatraveller::look::Shape::Square, ..Look::default() };
            let background = pipeline::palette::map(&[bg], &surface.regs, &look)[0];
            let metrics = panels::render::Metrics::component([384, 128], (48, 16), &look);
            let image = panels::render::compose([(Rect::new(0, 0, 48, 16), surface)], metrics, &look);
            let mut counts = std::collections::HashMap::new();
            for &p in &image { if p != background { *counts.entry(p).or_insert(0usize) += 1; } }
            let ink = counts.into_iter().max_by_key(|(_, n)| *n).unwrap().0;
            assert!(ratio(ink, background) >= 4.5, "{:?}, background {background:06x}, ink {ink:06x}, contrast {}", font, ratio(ink, background));
        }
    } }
}

#[test]
fn original_colour_and_classic_preserve_the_bitmap_palette() {
    let Some(assets) = assets() else { return };
    let mut source = Surface::new(64, 16); source.indexed.fill(1); source.regs[1] = 5; source.regs[15] = 6; source.text.regs = source.regs;
    assets.text(&mut source, "END TURN", (2, 2), 15);
    let original = Look { font: FontChoice::Original, shape: megatraveller::look::Shape::Square, text_colour: megatraveller::look::TextColour::Original, ..Look::default() };
    let metrics = panels::render::Metrics::component([64, 16], (64, 16), &original);
    let pixels = pipeline::palette::map(&source.indexed, &source.regs, &original);
    assert_eq!(panels::render::compose([(Rect::new(0, 0, 64, 16), source.clone())], metrics, &original), pixels);
    let mut classic = original.clone(); classic.text_colour = megatraveller::look::TextColour::HighContrast;
    classic.layout.preset = panels::settings::LayoutPreset::Classic;
    assert!(!classic.high_contrast_text());
    assert_eq!(panels::render::compose([(Rect::new(0, 0, 64, 16), source)], metrics, &classic), pixels);
}

#[test]
fn disabled_commands_are_dimmed_but_readable_for_every_palette_colour() {
    for r in (0..=255).step_by(17) { for g in (0..=255).step_by(17) { for b in (0..=255).step_by(17) {
        let bg = r << 16 | g << 8 | b;
        let enabled = pipeline::text::contrast_ink(bg); let disabled = pipeline::text::readable_ink(bg, true);
        assert!(ratio(enabled, bg) >= 4.5);
        assert!(ratio(disabled, bg) >= 3.0 && ratio(disabled, bg) < ratio(enabled, bg));
    } } }
    let Some(assets) = assets() else { return };
    for font in faces() { for bold in [false, true] {
        let mut surface = Surface::new(128, 16); surface.indexed.fill(1); surface.regs[1] = 5;
        surface.text.regs = surface.regs;
        assets.text(&mut surface, "Fire", (2, 2), 8); assets.text(&mut surface, "Regroup", (50, 2), 8);
        let look = Look { font: font.clone(), bold, shape: megatraveller::look::Shape::Square, ..Look::default() };
        let metrics = panels::render::Metrics::component([1024, 128], (128, 16), &look);
        let out = panels::render::compose([(Rect::new(0, 0, 128, 16), surface)], metrics, &look);
        let ink = pipeline::text::readable_ink(0xaa00aa, true);
        assert!(out.iter().filter(|&&c| c == ink).count() > 50, "{font:?} bold={bold}: disabled ink missing");
        assert!(ratio(ink, 0xaa00aa) >= 3.0);
    } }
}

#[test]
fn text_colour_defaults_and_look_toml_round_trip() {
    let saved = toml::Value::try_from(Look::default()).unwrap();
    assert_eq!(saved.get("text_colour").and_then(toml::Value::as_str), Some("high_contrast"));
    for choice in ["original", "high_contrast"] {
        let look: Look = toml::from_str(&format!("text_colour = '{choice}'")).unwrap();
        let value = toml::Value::try_from(&look).unwrap();
        assert_eq!(value["text_colour"].as_str(), Some(choice));
        assert_eq!(toml::from_str::<Look>(&toml::to_string(&look).unwrap()).unwrap(), look);
    }
}

#[test]
fn initiative_whole_names_fit_eight_slots_at_both_modern_sizes() {
    use megatraveller::panels::{Component, initiative::Initiative};
    use mt1::ui::Unit;
    let Some(assets) = assets() else { return };
    let component = Initiative::new(assets);
    for size in [[1531, 1221], [640, 480]] {
        let mut scene = fight();
        scene.squad.queue = vec![Unit::Traveller(0); 8];
        let metrics = panels::render::Metrics::main(size, &Look::default(), Some(&scene));
        for name in ["Magellan", "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdef"] {
            scene.party.0[0x102..0x122].fill(0); scene.party.0[0x102..0x102 + name.len()].copy_from_slice(name.as_bytes());
            let mut out = Surface::new(metrics.logical.0, 24); component.draw(&scene, &UiState::default(), &mut out);
            let chip = ((metrics.logical.0 - 26) / 8).clamp(24, 64) as i64;
            for i in 0..8 {
                let left = 26 + i * chip;
                let runs: Vec<_> = out.text.visible.iter().filter(|r| r.x >= left && r.x < left + chip).collect();
                assert_eq!(runs.iter().map(|r| r.bytes[0] as char).collect::<String>(), name, "{size:?}, slot {i}");
                assert!(runs.iter().all(|r| r.x + r.width as i64 <= left + chip - 2 && r.row() + r.erase_height() as i64 <= 23));
            }
        }
    }
}

#[test]
fn zz_skip_report() { oracle_env::report() }
