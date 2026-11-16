use megatraveller::{font_menu::{self, Catalog, Entry}, look::*, panels::{Layout, HostKind, settings::LayoutPreset}};
use mt1::ui::{ComponentId, Intent, Scene, ScreenKind};
use muda::{AboutMetadata, CheckMenuItem, IconMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use std::{collections::BTreeMap, path::PathBuf};

fn scenario_entry(text: &str) -> Option<(String, String)> {
    let v: toml::Value = toml::from_str(text).ok()?;
    let (group, label) = v.get("meta")?.get("menu")?.as_str()?.split_once(" / ")?;
    Some((group.trim().to_string(), label.trim().to_string()))
}
fn scenario_groups(files: impl IntoIterator<Item = (PathBuf, String)>) -> BTreeMap<String, Vec<(String, PathBuf)>> {
    let mut groups: BTreeMap<String, Vec<(String, PathBuf)>> = BTreeMap::new();
    for (path, text) in files {
        if let Some((group, label)) = scenario_entry(&text) { groups.entry(group).or_default().push((label, path)); }
    }
    for items in groups.values_mut() { items.sort(); }
    groups
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Scale(ScaleMode), Shape(Shape), Filter(Filter), Screen(Screen), Palette(Palette), Font(FontChoice),
    Bold, TextColour(TextColour), Background(Background), Fullscreen, Windowed, Quit, Scenario(PathBuf),
    Layout(LayoutPreset), Host(ComponentId, HostKind), LogLines(u8), ResetLayout, DebugInitiative, Follow,
    ClassicCombat, UnusedSkills,
    GrenadeRange, NpcSelectiveAutofire,
    Minimap,
    GameKey(u8),
    Plus(Option<u8>),
}
impl Action {
    pub fn intent(&self, scene: &Scene) -> Option<Intent> {
        (*self == Self::Follow && scene.screen == ScreenKind::Ground)
            .then(|| Intent::Follow(Some(megatraveller::panels::controls::selected(scene))))
    }
    pub fn apply(&self, look: &mut Look) {
        match self {
            Self::Scale(v) => look.scale = *v, Self::Shape(v) => look.shape = *v, Self::Filter(v) => look.filter = *v,
            Self::Screen(v) => look.screen = *v, Self::Palette(v) => look.palette = *v, Self::Font(v) => look.font = v.clone(),
            Self::Bold => look.bold = !look.bold, Self::Background(v) => look.background = *v,
            Self::TextColour(v) => look.text_colour = *v,
            Self::ClassicCombat => look.classic_combat = !look.classic_combat,
            Self::UnusedSkills => look.disable_unused_skills = !look.disable_unused_skills,
            Self::GrenadeRange => look.grenade_range = !look.grenade_range,
            Self::NpcSelectiveAutofire => look.npc_selective_autofire = !look.npc_selective_autofire,
            Self::Layout(p) => look.layout.preset = *p,
            Self::Host(id, host) => look.layout.components.entry((*id).into()).or_default().host = *host,
            Self::Minimap => { let e = look.layout.components.entry(megatraveller::panels::components::MINIMAP_ID.into()).or_default(); e.host = if e.host == HostKind::Hidden { HostKind::Docked } else { HostKind::Hidden } }
            Self::LogLines(n) => look.layout.log_lines = (*n).clamp(2, 20),
            Self::ResetLayout => look.layout = Default::default(), Self::DebugInitiative | Self::Follow | Self::Plus(_) | Self::GameKey(_) => {},
            Self::Fullscreen => look.fullscreen = true, Self::Windowed => look.fullscreen = false, Self::Quit | Self::Scenario(_) => {}
        }
    }
    fn checked(&self, look: &Look) -> bool {
        match self {
            Self::Bold => look.bold,
            Self::Font(v) => v == look.text_font(),
            Self::TextColour(v) => *v == if look.high_contrast_text() { TextColour::HighContrast } else { TextColour::Original },
            Self::ClassicCombat => look.classic_combat,
            Self::UnusedSkills => look.disable_unused_skills,
            Self::GrenadeRange => look.grenade_range,
            Self::NpcSelectiveAutofire => look.npc_selective_autofire,
            Self::Host(id, host) => look.layout.components.get(*id).map_or(HostKind::Docked, |s| s.host) == *host,
            Self::Minimap => look.layout.components.get(megatraveller::panels::components::MINIMAP_ID).is_none_or(|s| s.host != HostKind::Hidden),
            Self::Scenario(_) | Self::Quit | Self::ResetLayout | Self::DebugInitiative | Self::Follow | Self::Plus(_) | Self::GameKey(_) => false,
            _ => { let mut candidate = look.clone(); self.apply(&mut candidate); &candidate == look }
        }
    }
}
pub struct Menus {
    pub root: Menu,
    pub text: Submenu,
    view: Submenu,
    pub scenario_menu: Option<Submenu>,
    choices: Vec<(CheckMenuItem, Action)>,
    scenarios: Vec<(MenuItem, PathBuf)>,
    fonts: Vec<(CheckMenuItem, Action)>,
    entries: Vec<Entry>,
    catalog: Catalog,
    fonts_dir: PathBuf,
    look: Look,
}
const GAME_KEYS: [(&str, &str); 4] = [("game-save", "Save..."), ("game-load", "Load..."), ("game-quick-save", "Quick-save\tF6"), ("game-quick-load", "Quick-load\tF9")];
fn game_key(id: &str) -> Option<u8> {
    use mt1::save::*;
    Some(match id { "game-save" => KEY_SAVE, "game-load" => KEY_LOAD, "game-quick-save" => KEY_QUICK_SAVE, "game-quick-load" => KEY_QUICK_LOAD, _ => return None })
}
impl Menus {
    pub fn new(look: &Look) -> Result<Self, muda::Error> {
        let root = Menu::new();
        let file = Submenu::new("File", true);
        let look_menu = Submenu::new("Look", true);
        let text = Submenu::new("Text", true);
        let view = Submenu::new("View", true);
        let help = Submenu::new("Help", true);
        let plus = Submenu::new(PLUS_TITLE, true);
        root.append_items(&[&file, &look_menu, &view, &help, &MenuItem::with_id("plus-gap", "      ", false, None), &plus])?;
        let emblem = |tab: Option<u8>| plus_emblem(tab).ok();
        plus.append_items(&[&IconMenuItem::with_id("plus-open", "Open Plus\tF12", true, emblem(None), None), &PredefinedMenuItem::separator()])?;
        for (id, label, tab) in PLUS_TABS { plus.append(&IconMenuItem::with_id(id, label, true, emblem(Some(tab)), None))?; }
        let mut menus = Self { root, text, view, scenario_menu: None, scenarios: Vec::new(), choices: Vec::new(), fonts: Vec::new(), entries: Vec::new(),
            catalog: Catalog::default(), fonts_dir: Look::path().with_file_name("fonts"), look: look.clone() };
        plus.append(&PredefinedMenuItem::separator())?;
        menus.add(&plus, "Disable unused skills", Action::UnusedSkills)?;
        for (id, label) in GAME_KEYS { file.append(&MenuItem::with_id(id, label, true, None))?; }
        file.append(&PredefinedMenuItem::separator())?;
        menus.add(&file, "Fullscreen (F11)", Action::Fullscreen)?;
        menus.add(&file, "Windowed", Action::Windowed)?;
        file.append(&PredefinedMenuItem::separator())?;
        menus.group(&file, "Options", &[("Classic 1990 combat", Action::ClassicCombat), ("Easier grenade range", Action::GrenadeRange), ("Selective NPC autofire", Action::NpcSelectiveAutofire)])?;
        file.append(&PredefinedMenuItem::separator())?;
        file.append(&MenuItem::with_id("quit", "Quit", true, None))?;
        menus.group(&look_menu, "Scale", &[("Integer-only", Action::Scale(ScaleMode::Integer)), ("Fill the window", Action::Scale(ScaleMode::Fill))])?;
        menus.group(&look_menu, "Shape", &[("1990 4:3 (Ctrl+S)", Action::Shape(Shape::Classic)), ("Square pixels (Ctrl+S)", Action::Shape(Shape::Square))])?;
        menus.group(&look_menu, "Filter", &[("Raw blocks (nearest)", Action::Filter(Filter::Nearest)), ("Sharp", Action::Filter(Filter::Sharp)), ("Soft", Action::Filter(Filter::Soft))])?;
        menus.group(&look_menu, "Screen", &[("None", Action::Screen(Screen::None)), ("Scanlines", Action::Screen(Screen::Scanlines)), ("CRT", Action::Screen(Screen::Crt)), ("Phosphor bleed", Action::Screen(Screen::Phosphor))])?;
        menus.group(&look_menu, "Palette", &[("EGA", Action::Palette(Palette::Ega)), ("Warm monitor", Action::Palette(Palette::Warm)), ("Amber mono", Action::Palette(Palette::Amber)), ("Green mono", Action::Palette(Palette::Green)), ("Custom tint", Action::Palette(Palette::Custom))])?;
        look_menu.append(&menus.text)?;
        let text = menus.text.clone();
        menus.add(&text, font_menu::BUILTINS[font_menu::DEFAULT_FACE as usize], Action::Font(FontChoice::Builtin(font_menu::DEFAULT_FACE)))?;
        menus.add(&text, "Original 1990", Action::Font(FontChoice::Original))?;
        for (i, name) in font_menu::BUILTINS.iter().enumerate().filter(|(i, _)| *i != font_menu::DEFAULT_FACE as usize) { menus.add(&text, name, Action::Font(FontChoice::Builtin(i as u8)))?; }
        text.append(&PredefinedMenuItem::separator())?;
        menus.add(&text, "Bold", Action::Bold)?;
        menus.group(&text, "Colour", &[("High contrast", Action::TextColour(TextColour::HighContrast)), ("Original", Action::TextColour(TextColour::Original))])?;
        text.append(&PredefinedMenuItem::separator())?;
        menus.group(&look_menu, "Background", &[("Black", Action::Background(Background::Black)), ("Dark grey", Action::Background(Background::DarkGrey)), ("Custom colour", Action::Background(Background::Custom))])?;
        let cosmetic = "Look settings are cosmetic: they never change the game";
        help.append(&PredefinedMenuItem::about(Some("About"), Some(AboutMetadata {
            name: Some("MegaTraveller 1 Plus".into()),
            version: Some(format!("{} (build {})", env!("CARGO_PKG_VERSION"), env!("MT_BUILD_HASH"))),
            comments: Some(format!("{cosmetic}\n\n{}", font_menu::LICENSES)),
            credits: Some(format!("{cosmetic}\n\n{}", font_menu::LICENSES)),
            license: Some(format!("Code: MIT. {}", font_menu::LICENSES)), ..AboutMetadata::default()
        })))?;
        menus.refresh_fonts()?;
        menus.sync(look);
        Ok(menus)
    }
    pub fn components(&mut self, layout: &Layout) -> Result<(), muda::Error> {
        let view = self.view.clone();
        view.append(&MenuItem::with_id("follow", "Follow", true, None))?;
        self.group(&view, "Layout", &[("Modern", Action::Layout(LayoutPreset::Modern)), ("1990 positions", Action::Layout(LayoutPreset::Classic))])?;
        self.add(&view, "Minimap (N)", Action::Minimap)?;
        let components = Submenu::new("Components", true);
        for id in layout.ids() {
            if id == megatraveller::panels::plus::ID { continue; }
            let Some(p) = layout.placement(id) else { continue; };
            let group = Submenu::new(id, true);
            self.add(&group, "Docked", Action::Host(id, HostKind::Docked))?;
            if p.undockable { self.add(&group, "Window", Action::Host(id, HostKind::Window))?; }
            if p.optional { self.add(&group, "Hidden", Action::Host(id, HostKind::Hidden))?; }
            components.append(&group)?;
        }
        view.append(&components)?;
        let lines = Submenu::new("Log lines", true);
        for n in 2..=20 { self.add(&lines, &n.to_string(), Action::LogLines(n))?; }
        view.append(&lines)?;
        view.append(&MenuItem::with_id("reset-layout", "Reset layout", true, None))?;
        self.group(&view, "Debug", &[("Show initiative panel", Action::DebugInitiative)])?;
        Ok(())
    }
    fn add(&mut self, group: &Submenu, name: &str, action: Action) -> Result<(), muda::Error> {
        let item = CheckMenuItem::new(name, true, action.checked(&self.look), None);
        group.append(&item)?; self.choices.push((item, action)); Ok(())
    }
    fn group(&mut self, parent: &Submenu, name: &str, actions: &[(&str, Action)]) -> Result<(), muda::Error> {
        let group = Submenu::new(name, true);
        for (name, action) in actions { self.add(&group, name, action.clone())?; }
        parent.append(&group)
    }
    pub fn scenarios(&mut self, dir: &std::path::Path) -> Result<(), muda::Error> {
        let menu = Submenu::new("Testing scenarios", true);
        menu.append(&MenuItem::new("Testing scenarios (not part of the game)", false, None))?;
        menu.append(&PredefinedMenuItem::separator())?;
        let files = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("toml")))
            .filter_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t)));
        let groups = scenario_groups(files);
        if groups.is_empty() { menu.append(&MenuItem::new("No scenarios in folder", false, None))?; }
        for (name, files) in groups {
            let group = Submenu::new(name.replace('&', "&&"), true);
            for (label, path) in files {
                let item = MenuItem::new(label.replace('&', "&&"), true, None);
                group.append(&item)?;
                self.scenarios.push((item,path));
            }
            menu.append(&group)?;
        }
        self.scenario_menu = Some(menu.clone());
        self.root.insert(&menu, 3)
    }
    pub fn action(&self, id: &muda::MenuId) -> Option<Action> {
        if id == "follow" { return Some(Action::Follow); }
        if id == "plus-open" { return Some(Action::Plus(None)); }
        if let Some((_, _, tab)) = PLUS_TABS.iter().find(|(i, ..)| id == i) { return Some(Action::Plus(Some(*tab))); }
        if id == "reset-layout" { return Some(Action::ResetLayout); }
        if id == "quit" { return Some(Action::Quit); }
        if let Some(scan) = game_key(id.as_ref()) { return Some(Action::GameKey(scan)); }
        if let Some((_,path)) = self.scenarios.iter().find(|(item,_)|item.id()==id) { return Some(Action::Scenario(path.clone())); }
        self.choices.iter().chain(&self.fonts).find(|(item, _)| item.id() == id).map(|(_, action)| action.clone())
    }
    pub fn sync(&mut self, look: &Look) {
        self.look = look.clone();
        for (item, action) in self.choices.iter().chain(&self.fonts) { item.set_checked(action.checked(look)); }
        for (item, action) in &self.choices { if *action == Action::TextColour(TextColour::HighContrast) { item.set_enabled(look.layout.preset != LayoutPreset::Classic); } }
    }
    pub fn debug_checked(&self, value: bool) {
        for (item, action) in &self.choices { if *action == Action::DebugInitiative { item.set_checked(value); } }
    }
    pub fn refresh_fonts(&mut self) -> Result<(), muda::Error> {
        let entries = self.catalog.scan(&self.fonts_dir);
        if self.entries == entries { return Ok(()); }
        for (item, _) in self.fonts.drain(..) { self.text.remove(&item)?; }
        for entry in &entries {
            let action = Action::Font(FontChoice::File(entry.file.clone()));
            let item = CheckMenuItem::new(entry.name.replace('&', "&&"), true, action.checked(&self.look), None);
            self.text.append(&item)?; self.fonts.push((item, action));
        }
        self.entries = entries; Ok(())
    }
}
pub const PLUS_TITLE: &str = "\u{2605} Plus page \u{2605}";
pub const PLUS_TABS: [(&str, &str, u8); 7] = [("plus-journal", "Journal", 2), ("plus-log", "Game log", 9), ("plus-characters", "Characters", 4),
    ("plus-map", "Map", 3), ("plus-test", "Test sheet", 16), ("plus-help", "Help\t?", 1), ("plus-bug", "Report a bug or request", 0)];
pub fn plus_emblem(tab: Option<u8>) -> Result<muda::Icon, muda::BadIcon> {
    let (w, h) = (16u32, 16u32); let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h { for x in 0..w {
        let corner = (x == 1 || x == 2) && (y == 1 || y == 2) && tab.is_some();
        let edge = x == 0 || y == 0 || x == w - 1 || y == h - 1;
        let plus = (5..=10).contains(&x) && (7..=8).contains(&y) || (7..=8).contains(&x) && (5..=10).contains(&y);
        let px: [u8; 4] = if plus || corner { [255, 255, 255, 255] } else if edge { [85, 0, 85, 255] } else { [170, 0, 170, 255] };
        rgba.extend_from_slice(&px);
    } }
    muda::Icon::from_rgba(rgba, w, h)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn text_colour_menu_defaults_high_contrast_and_classic_uses_original() {
        let mut look = Look::default();
        assert!(Action::TextColour(TextColour::HighContrast).checked(&look));
        Action::TextColour(TextColour::Original).apply(&mut look);
        assert!(!look.high_contrast_text());
        Action::TextColour(TextColour::HighContrast).apply(&mut look);
        assert!(look.high_contrast_text());
        Action::Layout(LayoutPreset::Classic).apply(&mut look);
        assert!(!look.high_contrast_text());
        assert!(Action::TextColour(TextColour::Original).checked(&look));
        assert!(!Action::TextColour(TextColour::HighContrast).checked(&look));
        assert!(!Action::Font(FontChoice::Original).checked(&look));
        assert!(Action::Font(FontChoice::Builtin(font_menu::DEFAULT_FACE)).checked(&look));
        Action::Layout(LayoutPreset::Modern).apply(&mut look);
        assert!(Action::Font(FontChoice::Builtin(font_menu::DEFAULT_FACE)).checked(&look));
    }
    #[test]
    fn scenario_menu_lines_group_by_place_and_skip_unlabelled_files() {
        let groups = scenario_groups([
            ("m8c_save_ground.toml", "[meta]\nmenu = \"Save and load / Saved game on the ground\"\nname = \"m8c_save_ground\"\n"),
            ("efate_wall.toml", "[meta]\nmenu = \"Efate ground / Edge of the area\"\n"),
            ("efate_door.toml", "[meta]\nmenu = \" Efate ground / At a building door \"\n"),
            ("efate_old.toml", "[meta]\nname = \"efate_old\"\n"),
            ("nolabel.toml", "[meta]\nmenu = \"no slash\"\n"),
            ("broken.toml", "not toml ["),
        ].map(|(p, t)| (PathBuf::from(p), t.to_string())));
        let labels: Vec<_> = groups.iter().map(|(group, items)| (group.as_str(), items.iter().map(|(label, _)| label.as_str()).collect::<Vec<_>>())).collect();
        assert_eq!(labels, vec![
            ("Efate ground", vec!["At a building door", "Edge of the area"]), ("Save and load", vec!["Saved game on the ground"]),
        ]);
        assert_eq!(groups["Efate ground"][0].1, PathBuf::from("efate_door.toml"));
    }
    #[cfg(target_os = "windows")]
    #[test]
    fn scenario_menu_registers_every_labelled_file_and_resolves_nested_items() {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../scenarios");
        let files: Vec<_> = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("toml")))
            .filter_map(|p| std::fs::read_to_string(&p).ok().map(|t| (p, t))).collect();
        let listed = files.iter().filter(|(_, t)| scenario_entry(t).is_some()).count();
        assert!(listed >= 20 && listed < files.len(), "the library keeps unlisted test scenarios; {listed} of {} listed", files.len());
        let groups = scenario_groups(files.clone());
        assert_eq!(groups.values().map(Vec::len).sum::<usize>(), listed);
        assert_eq!(groups.keys().collect::<Vec<_>>(), ["Efate ground", "Efate starport", "Save and load", "Space"]);
        let mut menus = Menus::new(&Look::default()).unwrap();
        menus.scenarios(&dir).unwrap();
        assert_eq!(menus.scenarios.len(), listed);
        let menu = menus.scenario_menu.as_ref().unwrap();
        assert_eq!(menu.text(), "Testing scenarios");
        let rows = menu.items();
        let note = rows[0].as_menuitem().unwrap();
        assert_eq!(note.text(), "Testing scenarios (not part of the game)");
        assert!(!note.is_enabled());
        assert!(rows[1].as_predefined_menuitem().is_some());
        assert_eq!(rows.len(), groups.len() + 2);
        for (row, (name, items)) in rows[2..].iter().zip(groups) {
            let group = row.as_submenu().unwrap();
            assert_eq!(group.text(), name);
            let children = group.items();
            assert_eq!(children.len(), items.len());
            for (child, (label, path)) in children.iter().zip(items) {
                assert_eq!(child.as_menuitem().unwrap().text(), label.replace('&', "&&"));
                assert_eq!(menus.action(child.id()), Some(Action::Scenario(path)));
            }
        }
        let mut empty = Menus::new(&Look::default()).unwrap();
        empty.scenarios(&dir.join("saves")).unwrap();
        assert!(empty.scenarios.is_empty());
        let rows = empty.scenario_menu.as_ref().unwrap().items();
        assert_eq!(rows.len(), 3);
        let placeholder = rows[2].as_menuitem().unwrap();
        assert_eq!(placeholder.text(), "No scenarios in folder");
        assert!(!placeholder.is_enabled());
    }
    #[test]
    fn balance_toggles_are_independent_and_persist_with_look_settings() {
        let mut look: Look = toml::from_str("classic_combat = false").unwrap();
        assert_eq!(look.balance_options(), Default::default());
        for action in [Action::GrenadeRange, Action::NpcSelectiveAutofire] {
            assert!(action.checked(&look)); action.apply(&mut look); assert!(!action.checked(&look));
            let restored: Look = toml::from_str(&toml::to_string(&look).unwrap()).unwrap();
            assert_eq!(restored.balance_options(), look.balance_options());
            assert!(!look.classic_combat);
        }
        Action::GrenadeRange.apply(&mut look);
        assert!(look.grenade_range && !look.npc_selective_autofire);
    }
    #[test]
    fn unused_skills_toggle_is_off_by_default_and_persists() {
        let mut look = Look::default();
        assert!(!look.disable_unused_skills && !Action::UnusedSkills.checked(&look));
        Action::UnusedSkills.apply(&mut look);
        let restored: Look = toml::from_str(&toml::to_string(&look).unwrap()).unwrap();
        assert!(restored.disable_unused_skills && Action::UnusedSkills.checked(&restored));
    }
    #[test]
    fn independent_knobs_and_radio_selection() {
        let mut look = Look::default();
        for action in [Action::Scale(ScaleMode::Fill), Action::Shape(Shape::Square), Action::Filter(Filter::Soft),
            Action::Screen(Screen::Crt), Action::Palette(Palette::Amber), Action::Font(FontChoice::Builtin(2)),
            Action::Bold, Action::Background(Background::DarkGrey), Action::Fullscreen] {
            action.apply(&mut look); assert!(action.checked(&look));
        }
        assert!(look.bold && look.fullscreen); assert_eq!(look.filter, Filter::Soft);
        assert!(!Action::Palette(Palette::Green).checked(&look));
        Action::Windowed.apply(&mut look); assert!(!look.fullscreen);
        Action::Bold.apply(&mut look); assert!(!look.bold);
    }
}
