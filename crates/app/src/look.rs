use serde::{Deserialize, Serialize};
use std::{fs, io::{self, Read}, path::{Path, PathBuf}};

pub const MAX_WINDOW_SIDE: u32 = 8192;
pub fn clamp_window_size(size: [u32; 2]) -> [u32; 2] { size.map(|n| n.min(MAX_WINDOW_SIDE)) }
pub const DEFAULT_CLIENT: [u32; 2] = [1080, 1120];
pub fn menu_height() -> u32 {
    #[cfg(windows)] { (unsafe { windows_sys::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows_sys::Win32::UI::WindowsAndMessaging::SM_CYMENU) }).max(0) as u32 }
    #[cfg(not(windows))] { 0 }
}
pub fn default_window_size() -> [u32; 2] { [DEFAULT_CLIENT[0], DEFAULT_CLIENT[1].saturating_sub(menu_height()).max(1)] }
pub fn lock_aspect(size: [u32; 2], previous: Option<[u32; 2]>, aspect: [u32; 2]) -> [u32; 2] {
    let [w, h] = size.map(|n| u64::from(n.max(1))); let [aw, ah] = aspect.map(|n| u64::from(n.max(1)));
    let by_w = [w, (w * ah + aw / 2) / aw]; let by_h = [(h * aw + ah / 2) / ah, h];
    let pick = match previous { Some([pw, ph]) if h.abs_diff(u64::from(ph)) > w.abs_diff(u64::from(pw)) => by_h, _ => by_w };
    clamp_window_size(pick.map(|n| n.min(u64::from(MAX_WINDOW_SIDE)) as u32))
}
const MAX_SETTINGS_BYTES: usize = 64 * 1024;
const MAX_SETTINGS_STRUCTURE: usize = 16;
const MAX_SETTINGS_TOKENS: usize = 512;
fn bounded_syntax(s: &str) -> bool {
    let bytes = s.as_bytes();
    let (mut i, mut structure, mut dots, mut span, mut tokens) = (0, 0usize, 0usize, 0usize, 0);
    while i < bytes.len() {
        match bytes[i] {
            b'#' => { while i < bytes.len() && bytes[i] != b'\n' { i += 1; } }
            quote @ (b'\'' | b'"') => {
                let multiline = bytes.get(i..i + 3) == Some(&[quote; 3]);
                i += if multiline { 3 } else { 1 };
                loop {
                    let Some(&c) = bytes.get(i) else { return false; };
                    if c == b'\\' && quote == b'"' { i += 2; continue; }
                    if !multiline && (c == b'\r' || c == b'\n') { return false; }
                    if c == quote {
                        if !multiline { i += 1; break; }
                        let start = i;
                        while bytes.get(i) == Some(&quote) { i += 1; }
                        if i - start > 5 { return false; }
                        if i - start >= 3 { break; }
                    } else { i += 1; }
                }
            }
            b'[' | b'{' => {
                structure += 1; span += 1; tokens += 1;
                if tokens > MAX_SETTINGS_TOKENS || structure > MAX_SETTINGS_STRUCTURE || span > MAX_SETTINGS_STRUCTURE { return false; }
                i += 1;
            }
            b']' | b'}' => { structure = structure.saturating_sub(1); i += 1; }
            b'.' => { dots += 1; span += 1; tokens += 1; if dots > MAX_SETTINGS_STRUCTURE || span > MAX_SETTINGS_STRUCTURE || tokens > MAX_SETTINGS_TOKENS { return false; } i += 1; }
            b'\n' => { dots = 0; if structure == 0 { span = 0; } i += 1; }
            b'=' => { dots = 0; i += 1; }
            _ => i += 1,
        }
    }
    true
}

macro_rules! choices {
    ($name:ident { $($v:ident),+ }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum $name { $($v),+ }
    };
}
choices!(ScaleMode { Integer, Fill });
choices!(Shape { Classic, Square });
choices!(Filter { Nearest, Sharp, Soft });
choices!(Screen { None, Scanlines, Crt, Phosphor });
choices!(Palette { Ega, Warm, Amber, Green, Custom });
choices!(Background { Black, DarkGrey, Custom });
choices!(TextColour { HighContrast, Original });
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MenuSound { #[serde(alias = "a", alias = "b")] On, Off }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FontChoice { Original, Builtin(u8), File(String) }

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum SavedFont { Original, Builtin(BuiltinName), File(String) }
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
enum BuiltinName { Name(String), Legacy(u8) }
impl Serialize for FontChoice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let value = match self {
            Self::Original => SavedFont::Original,
            Self::Builtin(i) => SavedFont::Builtin(BuiltinName::Name(crate::font_menu::BUILTIN_IDS.get(*i as usize).unwrap_or(&crate::font_menu::BUILTIN_IDS[crate::font_menu::DEFAULT_FACE as usize]).to_string())),
            Self::File(file) => SavedFont::File(file.clone()),
        };
        value.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for FontChoice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(match SavedFont::deserialize(deserializer)? {
            SavedFont::Original => Self::Original,
            SavedFont::File(file) => Self::File(file),
            SavedFont::Builtin(saved) => {
                let name = match saved {
                    BuiltinName::Name(name) => name,
                    BuiltinName::Legacy(i) => ["noto_sans", "noto_serif", "ibm_plex_mono", "courier_prime", "press_start_2p", "orbitron", "px437_ibm_ega_8x14", "vt323"].get(i as usize).unwrap_or(&"").to_string(),
                };
                Self::Builtin(crate::font_menu::BUILTIN_IDS.iter().position(|&id| id == name).map_or(crate::font_menu::DEFAULT_FACE, |i| i as u8))
            }
        })
    }
}

fn plus_mouse<'de, D: serde::Deserializer<'de>>(d: D) -> Result<bool, D::Error> {
    bool::deserialize(d).map(|_| true)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Look {
    #[serde(deserialize_with = "plus_mouse")]
    pub mouse: bool,
    pub scale: ScaleMode,
    pub shape: Shape,
    pub filter: Filter,
    pub screen: Screen,
    pub palette: Palette,
    pub font: FontChoice,
    pub bold: bool,
    pub text_colour: TextColour,
    pub background: Background,
    pub custom_tint: [u8; 3],
    pub custom_background: [u8; 3],
    pub window_size: [u32; 2],
    pub window_pos: Option<[i32; 2]>,
    pub fullscreen: bool,
    pub layout: crate::panels::settings::LayoutSettings,
    pub classic_combat: bool,
    pub grenade_range: bool, pub npc_selective_autofire: bool,
    pub npc_playback_ms: u32,
    pub menu_sound: MenuSound,
    pub autosave: MenuSound,
}
impl Default for Look {
    fn default() -> Self {
        Self { mouse: true, scale: ScaleMode::Integer, shape: Shape::Classic, filter: Filter::Nearest,
            screen: Screen::None, palette: Palette::Ega, font: FontChoice::Builtin(crate::font_menu::DEFAULT_FACE), bold: false, text_colour: TextColour::HighContrast,
            background: Background::Black, custom_tint: [255, 220, 170], custom_background: [24, 24, 32],
            window_size: default_window_size(), window_pos: None, fullscreen: false, layout: Default::default(), classic_combat: false, grenade_range: true, npc_selective_autofire: true, npc_playback_ms: 250, menu_sound: MenuSound::On, autosave: MenuSound::On }
    }
}
impl Look {
    pub fn text_font(&self) -> &FontChoice {
        &self.font
    }
    pub fn faithful() -> Self {
        Self { mouse: false, font: FontChoice::Original, text_colour: TextColour::Original, shape: Shape::Square, window_size: [320, 200], ..Self::default() }
    }
    pub fn high_contrast_text(&self) -> bool {
        self.text_colour == TextColour::HighContrast && self.layout.preset != crate::panels::settings::LayoutPreset::Classic
    }
    pub fn balance_options(&self) -> mt1::ground::rules::balance::Options {
        mt1::ground::rules::balance::Options { grenade_range: self.grenade_range, npc_selective_autofire: self.npc_selective_autofire }
    }
    pub fn path() -> PathBuf {
        std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default().join("look.toml")
    }
    pub fn load(path: impl AsRef<Path>) -> Self { Self::load_registered(path, None) }
    pub fn load_for_layout(path: impl AsRef<Path>, layout: &crate::panels::Layout) -> Self {
        Self::load_registered(path, Some(&layout.ids()))
    }
    fn load_registered(path: impl AsRef<Path>, ids: Option<&[&str]>) -> Self {
        let mut look: Self = (|| {
            let mut s = String::new();
            fs::File::open(path).ok()?.take((MAX_SETTINGS_BYTES + 1) as u64).read_to_string(&mut s).ok()?;
            if s.len() > MAX_SETTINGS_BYTES || !bounded_syntax(&s) { return None; }
            let mut value: toml::Value = toml::from_str(&s).ok()?;
            if let Some(ids) = ids {
                if let Some(components) = value.get_mut("layout").and_then(|v| v.get_mut("components")).and_then(toml::Value::as_table_mut) {
                    components.retain(|id, _| ids.contains(&id));
                }
            }
            value.try_into().ok()
        })().unwrap_or_default();
        look.window_size = if look.window_size.contains(&0) { Self::default().window_size } else { lock_aspect(clamp_window_size(look.window_size), None, default_window_size()) };
        if matches!(look.font, FontChoice::Builtin(i) if i as usize >= crate::font_menu::BUILTINS.len()) { look.font = FontChoice::Original; }
        look.layout.clamp();
        look
    }
    pub fn save(&self, path: impl AsRef<Path>) -> io::Result<()> {
        let data = toml::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, data)
    }
}
#[cfg(test)]
mod tests {
    #[test]
    fn h3_default_window_is_the_screenshot_shape_minus_the_menu_bar() {
        let d = default_window_size();
        assert_eq!(d[0], 1080); assert_eq!(d[1] + menu_height(), 1120);
        assert_eq!(Look::default().window_size, d);
        assert_eq!(lock_aspect(d, None, d), d, "the default is on its own ratio");
    }
    #[test]
    fn h3_aspect_lock_follows_the_dragged_axis_and_never_drifts() {
        let a = [1080, 1100]; let p = Some(a);
        assert_eq!(lock_aspect([1620, 1100], p, a), [1620, 1650], "wider: height follows");
        assert_eq!(lock_aspect([1080, 550], p, a), [540, 550], "shorter: width follows");
        assert_eq!(lock_aspect([540, 1100], p, a), [540, 550], "narrower: height follows");
        assert_eq!(lock_aspect([1080, 1100], Some([1620, 1650]), a), a, "back to the default");
        for size in [[1531, 1221], [1200, 800], [100, 100], [7000, 300], [7000, 9000]] {
            let once = lock_aspect(size, None, a); assert_eq!(lock_aspect(once, Some(once), a), once, "{size:?} re-corrects");
            assert!(once[0] <= MAX_WINDOW_SIDE && once[1] <= MAX_WINDOW_SIDE && once[0] >= 1 && once[1] >= 1);
            let r = (once[0] as f64 / once[1] as f64 - 1080.0 / 1100.0).abs(); assert!(r < 0.01, "{size:?} -> {once:?} off ratio by {r}");
        }
    }
    use super::*;
    #[test]
    fn px437_product_default_faithful_and_saved_choices() {
        let look = Look::default();
        assert_eq!(look.font, FontChoice::Builtin(crate::font_menu::DEFAULT_FACE));
        assert!(!look.bold); assert!(look.high_contrast_text());
        let faithful = Look::faithful();
        assert_eq!(faithful.text_font(), &FontChoice::Original); assert!(!faithful.bold && !faithful.high_contrast_text());
        let mut classic = look; classic.layout.preset = crate::panels::settings::LayoutPreset::Classic;
        assert_eq!(classic.text_font(), &FontChoice::Builtin(crate::font_menu::DEFAULT_FACE)); assert!(!classic.high_contrast_text());
        let path = std::env::temp_dir().join(format!("mt-look-fonts-{}.toml", std::process::id()));
        for source in ["", "window_size = [1531, 1221]\ntext_colour = 'original'\n"] {
            fs::write(&path, source).unwrap();
            let loaded = Look::load(&path);
            assert_eq!(loaded.font, FontChoice::Builtin(crate::font_menu::DEFAULT_FACE));
            assert!(!loaded.bold);
            if !source.is_empty() { assert_eq!(loaded.window_size, lock_aspect([1531, 1221], None, default_window_size())); assert_eq!(loaded.text_colour, TextColour::Original); }
        }
        for font in std::iter::once(FontChoice::Original).chain((0..crate::font_menu::BUILTINS.len() as u8).map(FontChoice::Builtin)) {
            let look = Look { font: font.clone(), bold: true, ..Look::default() };
            look.save(&path).unwrap(); assert_eq!(Look::load(&path), look, "saved choice {font:?}");
        }
        let invalid = Look { font: FontChoice::Builtin(crate::font_menu::BUILTINS.len() as u8), ..Look::default() };
        invalid.save(&path).unwrap(); assert_eq!(Look::load(&path).font, Look::default().font);
        fs::remove_file(&path).unwrap(); assert_eq!(Look::load(&path), Look::default());
    }
    #[test]
    fn saved_builtin_indices_migrate_by_identity_and_new_files_use_names() {
        for (old, name) in [(0, "px437_ibm_ega_8x14"), (1, "px437_ibm_ega_8x14"), (2, "ibm_plex_mono"),
            (3, "px437_ibm_ega_8x14"), (4, "px437_ibm_ega_8x14"), (5, "px437_ibm_ega_8x14"),
            (6, "px437_ibm_ega_8x14"), (7, "vt323"), (255, "px437_ibm_ega_8x14")] {
            let look: Look = toml::from_str(&format!("bold = true\n[font]\nbuiltin = {old}\n")).unwrap();
            let saved = toml::Value::try_from(&look).unwrap();
            assert_eq!(saved["font"]["builtin"].as_str(), Some(name), "old index {old}");
            assert!(look.bold); assert_eq!(toml::from_str::<Look>(&toml::to_string(&look).unwrap()).unwrap(), look);
        }
        for name in ["noto_sans", "noto_serif", "courier_prime", "press_start_2p", "orbitron"] {
            let look: Look = toml::from_str(&format!("[font]\nbuiltin = '{name}'")).unwrap();
            assert_eq!(look.font, Look::default().font);
        }
        assert_eq!(toml::from_str::<Look>("font = 'original'").unwrap().font, FontChoice::Original);
        assert_eq!(toml::from_str::<Look>("[font]\nfile = 'my-font.ttf'").unwrap().font, FontChoice::File("my-font.ttf".into()));
    }
    #[test]
    fn h2_mouse_defaults_on_and_old_settings_migrate_without_changing_faithful() {
        assert!(Look::default().mouse);
        assert!(toml::from_str::<Look>("bold = true").unwrap().mouse);
        assert!(toml::from_str::<Look>("mouse = true").unwrap().mouse);
        let path = std::env::temp_dir().join(format!("mt-h2-mouse-{}.toml", std::process::id()));
        fs::write(&path, "mouse = false\nbold = true\nwindow_size = [1200, 800]\n").unwrap();
        let loaded = Look::load(&path);
        assert!(loaded.mouse && loaded.bold);
        assert_eq!(loaded.window_size, lock_aspect([1200, 800], None, default_window_size()));
        fs::remove_file(path).unwrap();
        assert!(!Look::faithful().mouse);
    }
    #[test]
    fn settings_roundtrip_and_bad_input() {
        let path = std::env::temp_dir().join(format!("mt-look-{}.toml", std::process::id()));
        let mut look = Look { scale: ScaleMode::Fill, shape: Shape::Square, filter: Filter::Sharp,
            screen: Screen::Crt, palette: Palette::Custom, background: Background::Custom,
            bold: true, window_pos: Some([-100, 72]), fullscreen: true, ..Look::default() };
        for text_colour in [TextColour::Original, TextColour::HighContrast] {
            for font in [FontChoice::Original, FontChoice::Builtin(2), FontChoice::File("fonts/Some Face.otf".into())] {
                look.font = font; look.text_colour = text_colour; look.save(&path).unwrap(); assert_eq!(Look::load(&path), look);
            }
        }
        for s in ["[", "scale = 'unknown'", "bold = 2", "window_size = [-1, 2]", "custom_tint = [999, 1, 2]"] {
            fs::write(&path, s).unwrap(); assert_eq!(Look::load(&path), Look::default());
        }
        fs::write(&path, "window_size = [0, 4294967295]").unwrap();
        assert_eq!(Look::load(&path), Look::default());
        fs::remove_file(&path).unwrap(); assert_eq!(Look::load(&path), Look::default());
    }
    #[test]
    fn settings_bounds_and_quoted_syntax() {
        let path = std::env::temp_dir().join(format!("mt-look-bounds-{}.toml", std::process::id()));
        let syntax = "[.{".repeat(128);
        for quoted in [format!("'{syntax}'"), format!("\"{syntax}\""), format!("'''{syntax}'''"),
            format!("\"\"\"{syntax}\"\"\""), format!("'''{syntax}''''"), format!("\"\"\"{syntax}\"\"\"\"\""),
            format!("\"escaped \\\" {syntax}\"")] {
            let s = format!("bold = true\nunknown = {quoted}\n# {syntax}\n");
            assert!(bounded_syntax(&s), "{s}"); fs::write(&path, s).unwrap();
            assert!(Look::load(&path).bold);
        }
        for s in [format!("bold = true\n#{}", "x".repeat(MAX_SETTINGS_BYTES)),
            format!("bold = true\n{}z = 1", "x.".repeat(128)),
            format!("bold = true\n[{}z]\ny = 1", "x.".repeat(128)),
            format!("bold = true\nunknown = {}0{}", "{ x = ".repeat(128), "}".repeat(128)),
            format!("bold = true\nunknown = [\"\"\"{syntax}\"\"\"\"\", {}0{}]", "[".repeat(128), "]".repeat(128))] {
            fs::write(&path, s).unwrap(); assert_eq!(Look::load(&path), Look::default());
        }
        fs::write(&path, [0xff, 0xfe]).unwrap(); assert_eq!(Look::load(&path), Look::default());
        let deepest = format!("bold = true\nunknown = {}0{}", "[".repeat(MAX_SETTINGS_STRUCTURE), "]".repeat(MAX_SETTINGS_STRUCTURE));
        assert!(bounded_syntax(&deepest)); fs::write(&path, deepest).unwrap(); assert!(Look::load(&path).bold);
        fs::write(&path, "bold = true\nwindow_size = [16384, 16384]").unwrap();
        let look = Look::load(&path); assert!(look.bold); assert_eq!(look.window_size, lock_aspect([MAX_WINDOW_SIDE; 2], None, default_window_size()));
        fs::write(&path, "window_size = [4294967295, 1]").unwrap();
        assert_eq!(Look::load(&path).window_size, lock_aspect([MAX_WINDOW_SIDE, 1], None, default_window_size()));
        fs::remove_file(path).unwrap();
    }
}
