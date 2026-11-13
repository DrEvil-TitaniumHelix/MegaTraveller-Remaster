use super::{assets::Assets, Anchor, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use crate::mouse::decision;
use mt_engine::{io::{keys, scan, Event}, play::{Decision, Kind}};
use mt1::ui::{Intent, Scene};
use std::{cell::RefCell, sync::Arc};

pub const ROW: u32 = 12;
const TOP: u32 = 24;
pub const ENABLED: bool = false;
pub const BESIDE_SIZE: (u32, u32) = (480, 200);
fn side_column(scene: &Scene, ui: &UiState) -> bool {
    ui.preset == mt1::ui::Preset::Modern && scene.screen == mt1::ui::ScreenKind::Space
        && ui.decision.as_ref().is_some_and(|d| matches!(d.context.routine.as_str(), "9FDB" | "955C"))
}
pub fn beside_picture(scene: &Scene, ui: &UiState) -> bool {
    if !ENABLED { return false; }
    if side_column(scene, ui) { return true; }
    ui.preset == mt1::ui::Preset::Modern && !matches!(scene.screen, mt1::ui::ScreenKind::Ground | mt1::ui::ScreenKind::Space)
        && ((scene.screen == mt1::ui::ScreenKind::Picture && matches!(scene.context.event, 9 | 10))
            || ui.decision.as_ref().is_some_and(|d| matches!(d.kind, Kind::Menu | Kind::List | Kind::YesNo)
                || (d.kind == Kind::Ack && matches!(scene.screen, mt1::ui::ScreenKind::Record | mt1::ui::ScreenKind::Picture))))
}
pub fn logical(scene: &Scene, ui: &UiState) -> (u32, u32) {
    if side_column(scene, ui) { return BESIDE_SIZE }
    (super::FRAME.w, super::FRAME.h + ui.decision.as_ref().map_or(0, |d| size(d).1))
}
pub fn dock(scene: &Scene, ui: &UiState, main: (u32, u32)) -> Rect {
    if side_column(scene, ui) { return Rect::new(320, 0, main.0.saturating_sub(320).max(1), main.1) }
    let (w, h) = ui.decision.as_ref().map_or((main.0, 1), size);
    let w = w.min(main.0); let h = h.min(main.1.saturating_sub(super::FRAME.h)).max(1);
    Rect::new(((main.0 - w) / 2) as i32, super::FRAME.h as i32, w, h)
}
pub fn height(d: &Decision) -> u32 { size(d).1 }
fn footer_rows(d: &Decision) -> u32 { (d.list.is_some() || Choices::count(d) > 12) as u32 }
pub fn size(d: &Decision) -> (u32, u32) {
    let labels = d.list.as_ref().map_or_else(|| (0..d.options.len()).map(|i| display(d, i).0.len()).max().unwrap_or(0),
        |l| l.rows.iter().map(|r| r.label.len() + if l.increase.is_empty() { 0 } else { 20 }).max().unwrap_or(0));
    let footer: usize = Choices::footer_labels(d).iter().map(|l| l.len() + 2).sum();
    let longest = labels.max(d.prompt.as_deref().map_or(16, str::len)).max(if footer_rows(d) == 1 { footer } else { 0 });
    ((longest as u32 * 6 + 16).max(120), TOP + Choices::count(d).clamp(1, 12) as u32 * ROW + footer_rows(d) * ROW)
}
fn gunner(d: &Decision) -> bool { d.context.program == "s" && matches!(d.context.routine.as_str(), "9FDB" | "955C") }
fn gun_key(code: Option<u8>) -> bool {
    matches!(code, Some(scan::UP | scan::LEFT | scan::DOWN | scan::RIGHT | scan::ENTER | 0x14 | 0x21))
}
pub fn handles_key(d: &Decision, k: u16) -> bool {
    let code = scan::make_code(k);
    if gunner(d) && (gun_key(code) || matches!(code, Some(scan::PGUP | scan::PGDN | scan::SPACE))) { return true; }
    matches!(code, Some(scan::UP | scan::LEFT | scan::DOWN | scan::RIGHT | scan::ENTER))
        || (d.list.is_some() && matches!(k, 43 | 45 | 0x14e | 0x14a))
        || d.options.iter().any(|o| o.input.iter().filter(|e| matches!(e, Event::Key(_))).count() == 1
            && o.input.iter().any(|e| matches!(e, Event::Key(native) if scan::make_code(*native) == code && code.is_some())))
        || by_initial(d, k).is_some()
}
#[derive(Default)]
struct View { identity: String, page: usize, selected: usize }
pub struct Choices { assets: Arc<Assets>, view: RefCell<View> }
impl Choices {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets, view: RefCell::default() } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Float(Anchor::Centre), size: Size { preferred: (304, 180), min: (120, 60) }, order: 95,
            screens: &[], when: super::always, origin: Origin::New, optional: false, undockable: false, title: "Choices" }
    }
    fn sync(&self, d: &Decision) {
        let identity = format!("{:?}:{:?}:{:?}:{:?}", d.context, d.default, d.list, d.options);
        let mut v = self.view.borrow_mut();
        if v.identity != identity { *v = View { identity, page: 0, selected: d.list.as_ref().map_or_else(|| d.options.iter().position(|o| Some(&o.id) == d.default.as_ref()).unwrap_or(0), |l| l.current) }; }
    }
    fn rows(height: u32, d: &Decision) -> usize { (height.saturating_sub(TOP + footer_rows(d) * ROW) / ROW).max(1) as usize }
    fn footer_labels(d: &Decision) -> Vec<String> { Self::footer(d, 600, 600).into_iter().map(|(_, l, _)| l).collect() }
    fn count(d: &Decision) -> usize { d.list.as_ref().map_or(d.options.len(), |l| l.rows.len()) }
    fn footer(d: &Decision, w: u32, h: u32) -> Vec<(Rect, String, bool)> {
        if footer_rows(d) == 0 { return Vec::new() }
        let mut items = vec![("Previous".into(), true), ("Next".into(), true)];
        if let Some(l) = &d.list {
            if !l.decrease.is_empty() { items.push(("-".into(), true)); }
            if !l.increase.is_empty() { items.push(("+".into(), true)); }
            items.push(("OK".into(), !l.submit.is_empty()));
            items.extend(d.options.iter().map(|o| (o.label.clone(), o.enabled)));
        }
        let width = w / items.len().max(1) as u32;
        items.into_iter().enumerate().map(|(i, (label, enabled))| (Rect::new(i as i32 * width as i32, h.saturating_sub(ROW) as i32, width, ROW), label, enabled)).collect()
    }
    fn activate(&self, d: &Decision, label: &str) -> Vec<Intent> {
        let Some(l) = &d.list else { return vec![] };
        let v = self.view.borrow();
        let Some(row) = l.rows.get(v.selected).filter(|r| r.enabled) else { return vec![] };
        let tail = match label {
            "+" if row.qty < row.limit => &l.increase,
            "-" if row.qty > row.min_qty => &l.decrease,
            "OK" => &l.submit,
            _ => return vec![],
        };
        if tail.is_empty() { return vec![] }
        decision::raw(&row.input.iter().chain(tail).copied().collect::<Vec<_>>())
    }
}

pub fn hotkeys(d: &Decision) -> Vec<Option<usize>> {
    let own: Vec<Option<(usize, u8)>> = d.options.iter().map(|o| { let (label, hot) = shortcut(&o.label, &o.input); hot.filter(|_| label == o.label).and_then(|i| o.label.as_bytes().get(i).map(|b| (i, b.to_ascii_lowercase()))) }).collect();
    let mut taken: Vec<u8> = own.iter().flatten().map(|&(_, b)| b).collect();
    let menu = matches!(d.kind, Kind::Menu | Kind::YesNo);
    own.iter().zip(&d.options).map(|(own, o)| {
        if let Some(&(i, _)) = own.as_ref() { return Some(i) }
        let keys: Vec<u16> = o.input.iter().filter_map(|e| if let Event::Key(k) = e { Some(*k) } else { None }).collect();
        if !menu || keys.len() != 1 || !((0x13b..=0x144).contains(&keys[0]) || keys[0] == keys::ESC) { return None }
        let (i, b) = o.label.bytes().enumerate().find(|(_, b)| b.is_ascii_alphabetic())?;
        let b = b.to_ascii_lowercase(); if taken.contains(&b) { return None } taken.push(b); Some(i)
    }).collect()
}
pub fn by_initial(d: &Decision, k: u16) -> Option<usize> {
    let b = u8::try_from(k).ok().filter(u8::is_ascii_alphabetic)?.to_ascii_lowercase();
    hotkeys(d).into_iter().enumerate().find(|(i, h)| h.is_some_and(|h| d.options[*i].enabled && d.options[*i].label.as_bytes().get(h).is_some_and(|c| c.to_ascii_lowercase() == b))).map(|(i, _)| i)
}
pub fn display(d: &Decision, i: usize) -> (String, Option<usize>) {
    let o = &d.options[i];
    if let Some(h) = hotkeys(d).get(i).copied().flatten() { return (o.label.clone(), Some(h)) }
    match shortcut(&o.label, &o.input) { (s, Some(0)) if s != o.label => (s.split_once(": ").map_or(s.clone(), |(k, _)| format!("{} [{k}]", o.label)), None), (s, _) => (s, None) }
}
pub fn shortcut(label: &str, input: &[Event]) -> (String, Option<usize>) {
    if input.iter().filter(|e| matches!(e, Event::Key(_))).count() != 1 { return (label.into(), None) }
    let Some(k) = input.iter().find_map(|e| if let Event::Key(k) = e { Some(*k) } else { None }) else { return (label.into(), None) };
    let code = scan::make_code(k);
    if let Some(i) = label.bytes().position(|b| b.is_ascii_alphanumeric() && scan::make_code(b as u16) == code && code.is_some()) {
        return (label.into(), Some(i));
    }
    let key = match k {
        keys::ENTER | 0x11c => "Enter".into(), keys::ESC | 0x101 => "Esc".into(), keys::SPACE | 0x139 => "Space".into(),
        keys::UP => "Up".into(), keys::DOWN => "Down".into(), keys::LEFT => "Left".into(), keys::RIGHT => "Right".into(),
        0x13b..=0x144 => format!("F{}", k - 0x13a),
        33..=126 => char::from(k as u8).to_string(), _ => return (label.into(), None),
    };
    (format!("{key}: {label}"), Some(0))
}
pub fn button(assets: &Assets, out: &mut Surface, r: Rect, label: &str, enabled: bool, selected: bool, underline: Option<usize>) {
    let bg = if selected { 1 } else { 0 }; let ink = if enabled { 15 } else { 8 };
    for y in r.y..r.y + r.h as i32 { for x in r.x..r.x + r.w as i32 { out.put(x, y, bg); } }
    for x in r.x..r.x + r.w as i32 { out.put(x, r.y + r.h as i32 - 1, 8); }
    let label: String = label.chars().take(r.w.saturating_sub(6) as usize / 6).collect();
    assets.text_hot(out, &label, (r.x + 3, r.y + 2), ink, underline.filter(|&i| i < label.len()));
}
impl Component for Choices {
    fn visible(&self, ui: &UiState) -> bool {
        ENABLED && ui.decision.as_ref().is_some_and(|d| matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::Ack | Kind::List))
    }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        let Some(d) = &ui.decision else { return }; self.sync(d);
        let v = self.view.borrow(); let rows = Self::rows(out.h as u32, d); let count = Self::count(d);
        out.indexed.fill(0);
        let fallback = if scene.screen == mt1::ui::ScreenKind::Ground && d.kind == Kind::Ack && d.context.routine == "restore" {
            "Select drive to restore from:"
        } else { "Choose an action" };
        self.assets.text(out, d.prompt.as_deref().unwrap_or(fallback), (3, 3), 14);
        let hint = if gunner(d) { "PgUp/PgDn Space: use".into() } else if footer_rows(d) == 0 { String::new() } else {
            format!("Page {}/{}{}", v.page + 1, count.div_ceil(rows).max(1),
                d.list.as_ref().map_or(String::new(), |l| format!("   Credits: {}", l.money)))
        };
        self.assets.text(out, &hint, (3, 14), 7);
        for index in (v.page * rows..count).take(rows) {
            let r = Rect::new(0, (TOP + (index % rows) as u32 * ROW) as i32, out.w as u32, ROW);
            if let Some(l) = &d.list {
                let row = &l.rows[index]; let label = if l.increase.is_empty() { row.label.clone() } else { format!("{}  {} Cr  Qty {}", row.label, row.price, row.qty) };
                button(&self.assets, out, r, &label, row.enabled, index == v.selected, None);
            } else {
                let o = &d.options[index]; let (label, underline) = display(d, index);
                button(&self.assets, out, r, &label, o.enabled, index == v.selected, underline);
            }
        }
        for (r, label, mut enabled) in Self::footer(d, out.w as u32, out.h as u32) {
            if label == "Previous" { enabled = v.page > 0; }
            if label == "Next" { enabled = (v.page + 1) * rows < count; }
            if matches!(label.as_str(), "+" | "-") { enabled = d.list.as_ref().and_then(|l| l.rows.get(v.selected)).is_some_and(|r| r.enabled && if label == "+" { r.qty < r.limit } else { r.qty > r.min_qty }); }
            button(&self.assets, out, r, &label, enabled, false, None);
        }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let Some(d) = &ui.decision else { return vec![] }; self.sync(d);
        let size = ui.component_input.size; let rows = Self::rows(size.1, d); let count = Self::count(d);
        if let UiEvent::Key(k) = *e {
            let mut code = scan::make_code(k);
            if gunner(d) {
                if gun_key(code) { return decision::raw(&[Event::Key(k)]); }
                code = match code { Some(scan::PGUP) => Some(scan::UP), Some(scan::PGDN) => Some(scan::DOWN),
                    Some(scan::SPACE) => Some(scan::ENTER), _ => code };
            }
            if matches!(code, Some(scan::UP | scan::LEFT | scan::DOWN | scan::RIGHT)) && count > 0 {
                let mut v = self.view.borrow_mut();
                let forward = matches!(code, Some(scan::DOWN | scan::RIGHT));
                for _ in 0..count {
                    v.selected = if forward { (v.selected + 1) % count } else { (v.selected + count - 1) % count };
                    if d.list.as_ref().map_or_else(|| d.options[v.selected].enabled, |l| l.rows[v.selected].enabled) { break }
                }
                v.page = v.selected / rows; return vec![]
            }
            if code == Some(scan::ENTER) {
                return if d.list.is_some() { self.activate(d, "OK") } else { decision::option(d, self.view.borrow().selected) }
            }
            if d.list.is_some() && matches!(k, 43 | 45 | 0x14e | 0x14a) { return self.activate(d, if matches!(k, 43 | 0x14e) { "+" } else { "-" }) }
            if let Some(i) = d.options.iter().position(|o| o.input.iter().filter(|e| matches!(e, Event::Key(_))).count() == 1 && o.input.iter().any(|e| matches!(e, Event::Key(native) if scan::make_code(*native) == code && code.is_some()))) {
                return decision::option(d, i)
            }
            if let Some(i) = by_initial(d, k) {
                return decision::option(d, i)
            }
            return vec![];
        }
        if let UiEvent::Wheel(delta) = *e {
            let mut v = self.view.borrow_mut();
            if delta < 0 && (v.page + 1) * rows < count { v.page += 1; } else if delta > 0 { v.page = v.page.saturating_sub(1); }
            return vec![]
        }
        if *e != UiEvent::Up(Button::Left) || !Rect::new(0, 0, size.0, size.1).contains(at) { return vec![] }
        let footer = Self::footer(d, size.0, size.1);
        if let Some((index, (_, label, enabled))) = footer.iter().enumerate().find(|(_, (r, _, _))| r.contains(at)) {
            if !enabled { return vec![] }
            if index < 2 {
                let mut v = self.view.borrow_mut();
                if index == 0 { v.page = v.page.saturating_sub(1); } else if (v.page + 1) * rows < count { v.page += 1; }
                return vec![]
            }
            let option_start = footer.len() - d.options.len();
            if index >= option_start { return decision::option(d, index - option_start) }
            return self.activate(d, label)
        }
        if at.1 < TOP as i32 { return vec![] }
        let index = self.view.borrow().page * rows + (at.1 as u32 - TOP) as usize / ROW as usize;
        if let Some(l) = &d.list {
            let Some(row) = l.rows.get(index).filter(|r| r.enabled) else { return vec![] };
            self.view.borrow_mut().selected = index;
            if l.increase.is_empty() && l.decrease.is_empty() { return decision::raw(&row.input.iter().chain(&l.submit).copied().collect::<Vec<_>>()) }
            vec![]
        } else { decision::option(d, index) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::play::{List, Option as Opt, Row as ListRow};
    fn ui(d: Decision) -> UiState { let mut ui = UiState { decision: Some(d), ..UiState::default() }; ui.component_input.size = (304, 180); ui }
    #[test]
    fn g7_gunner_keys_reach_the_game_and_menu_has_separate_keys() {
        for routine in ["9FDB", "955C"] {
            let mut d = Decision::new(Kind::Menu, "s", routine);
            d.options = vec![Opt::new("back", "Back", vec![Event::Key(keys::ESC)]),
                Opt::new("cmd:target", "Target", vec![Event::Key(b'T' as u16)]),
                Opt::new("cmd:fire", "Fire", vec![Event::Key(keys::ENTER)])];
            let ui = ui(d); let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Space);
            for k in [keys::LEFT, keys::RIGHT, keys::UP, keys::DOWN, keys::ENTER, 0x11c, b'T' as u16, b't' as u16, 0x114] {
                assert!(handles_key(ui.decision.as_ref().unwrap(), k));
                assert_eq!(c.input(&s, &ui, &UiEvent::Key(k), (0, 0)), decision::raw(&[Event::Key(k)]),
                    "{routine}: native key {k:#x} must reach the gun");
            }
            for k in [keys::PGUP, keys::PGDN, keys::SPACE] { assert!(handles_key(ui.decision.as_ref().unwrap(), k)); }
            assert!(c.input(&s, &ui, &UiEvent::Key(keys::PGDN), (0, 0)).is_empty());
            assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::SPACE), (0, 0)), decision::raw(&[Event::Key(b'T' as u16)]));
            c.input(&s, &ui, &UiEvent::Key(keys::PGUP), (0, 0));
            assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::SPACE), (0, 0)), decision::raw(&[Event::Key(keys::ESC)]));
        }
        let d = Decision::new(Kind::Menu, "s", "choose_station");
        for k in [keys::UP, keys::DOWN, keys::LEFT, keys::RIGHT, keys::ENTER] { assert!(handles_key(&d, k)); }
    }
    #[ignore = "the action list is disabled; set choices::ENABLED to run"]
    #[test]
    fn action_list_is_sized_to_content_in_a_band_under_the_whole_1990_screen() {
        use super::super::{components, render::Metrics, Host, Layout};
        let mut d = Decision::new(Kind::YesNo, "c", "develop"); d.prompt = Some("DEVELOP THIS CHARACTER?".into());
        d.options = vec![Opt::new("yes", "Yes", vec![Event::Key(keys::ENTER)]), Opt::new("no", "No", vec![Event::Key(b'N' as u16)]), Opt::new("back", "Cancel", vec![Event::Key(keys::ESC)])];
        let (w, h) = size(&d);
        assert_eq!(h, TOP + 3 * ROW, "rows only: no paging row"); assert_eq!(w, 23 * 6 + 16, "as wide as the prompt");
        let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Cg);
        let mut ui = UiState { decision: Some(d.clone()), ..UiState::default() }; ui.component_input.size = (w, h);
        let mut out = Surface::new(w, h); c.draw(&s, &ui, &mut out);
        let words: Vec<String> = out.text.visible.iter().map(|r| String::from_utf8_lossy(&r.bytes).to_string()).collect();
        assert!(words.iter().all(|t| !t.contains("Page") && !t.contains("Previous") && !t.contains("Next")), "{words:?}");
        let mut long = Decision::new(Kind::Menu, "t", "AF79"); long.options = (0..20).map(|i| Opt::new("o", format!("Option {i}"), vec![])).collect();
        assert_eq!(size(&long).1, TOP + 12 * ROW + ROW, "a long menu keeps its paging row");
        let look = crate::look::Look::default();
        for window in [crate::look::default_window_size(), [1280, 800]] {
            let m = Metrics::for_ui(window, &look, Some(&s), &ui);
            assert_eq!(m.logical, (320, 200 + h));
            let mut l = Layout::default(); components::register(&mut l, Arc::default()); l.arrange_for(&s, &ui, m.logical);
            let Host::Dock(pic) = l.host("Picture").unwrap() else { panic!() }; assert_eq!(pic, super::super::FRAME);
            let Host::Dock(host) = l.host("Choices").unwrap() else { panic!() };
            assert!(host.y >= 200 && host.y + host.h as i32 <= m.logical.1 as i32 && host.x >= 0 && host.x + host.w as i32 <= 320, "{host:?} outside the 1990 screen");
            assert_eq!((host.w, host.h), (w, h));
            let at = (host.x + 10, host.y + TOP as i32 + ROW as i32 + 3);
            assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (at.0 - host.x, at.1 - host.y)), decision::raw(&[Event::Key(b'N' as u16)]));
        }
        let plain = UiState::default();
        assert!(!beside_picture(&s, &plain), "no band without a list");
        assert!(!beside_picture(&Scene::program(mt1::ui::ScreenKind::Ground), &ui), "the ground keeps its float");
    }
    #[test]
    fn efate_quiet_menu_row_is_native_option_and_disabled_does_nothing() {
        let mut d = Decision::new(Kind::Menu, "t", "AF79");
        d.options = vec![Opt::new("save", "Save", vec![Event::Key(0x13b)]), Opt::new("back", "Back", vec![Event::Key(keys::ESC)])];
        d.default = Some("save".into()); d.options[1].enabled = false;
        let ui = ui(d); let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Ground);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 27)), decision::raw(&[Event::Key(0x13b)]));
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 40)).is_empty());
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (-1, 27)).is_empty());
        let mut out = Surface::new(304, 180); c.draw(&s, &ui, &mut out);
        assert!(out.text.runs.iter().any(|r| r.bytes[0] == b'S' && r.colour & crate::pipeline::text::UNDERLINE != 0), "S: the free initial of the F1 row is its hotkey");
        assert_eq!(hotkeys(&ui.decision.clone().unwrap()), [Some(0), Some(0)], "Save = S, Back = B (Esc row)");
        assert_eq!(by_initial(ui.decision.as_ref().unwrap(), b's' as u16), Some(0)); assert_eq!(by_initial(ui.decision.as_ref().unwrap(), b'b' as u16), None, "disabled rows take no letter");
        assert!(handles_key(ui.decision.as_ref().unwrap(), b'S' as u16));
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(b's' as u16), (0, 0)), decision::raw(&[Event::Key(0x13b)]), "typing S presses the row's F1");
        let mut clash = Decision::new(Kind::Menu, "t", "AF79");
        clash.options = vec![Opt::new("s1", "Sound off", vec![Event::Key(0x13d)]), Opt::new("s2", "Save", vec![Event::Key(0x13b)]), Opt::new("l", "Load", vec![Event::Key(keys::ENTER)])];
        assert_eq!(hotkeys(&clash), [Some(0), None, None], "the second S row is F-key only; an Enter row takes no letter");
        assert_eq!(shortcut("Game", &[Event::Key(0x122)]), ("Game".into(), Some(0)));
        assert_eq!(shortcut("Back", &[Event::Key(keys::ESC)]), ("Esc: Back".into(), Some(0)));
    }
    #[test]
    fn efate_shop_keys_and_clicks_share_selection_without_stale_native_navigation() {
        let ui = ui(list(true)); let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Picture);
        assert!(c.input(&s, &ui, &UiEvent::Key(keys::DOWN), (0, 0)).is_empty());
        let mut expected = vec![Event::Key(keys::DOWN); 2]; expected.push(Event::Key(b'+' as u16));
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(b'+' as u16), (0, 0)), decision::raw(&expected));
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::ESC), (0, 0)), decision::raw(&[Event::Key(keys::ESC)]));
        assert!(c.input(&s, &ui, &UiEvent::Key(b'-' as u16), (0, 0)).is_empty());
    }
    fn list(quantity: bool) -> Decision {
        let mut d = Decision::new(Kind::List, "t", "11571");
        d.options.push(Opt::new("exit", "Exit", vec![Event::Key(keys::ESC)]));
        d.list = Some(List { rows: (0..22).map(|i| ListRow { id: i.to_string(), label: format!("Item {i}"), enabled: i != 1, price: 50,
            limit: 3, qty: 1, min_qty: 0, input: vec![Event::Key(keys::DOWN); i] }).collect(), current: 0, more: true, money: 1000,
            increase: if quantity { vec![Event::Key(b'+' as u16)] } else { vec![] }, decrease: vec![], submit: vec![Event::Key(keys::ENTER)] }); d
    }
    #[test]
    fn efate_shop_long_list_pages_and_quantity_use_the_selected_rows_route() {
        let ui = ui(list(true)); let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Picture);
        c.input(&s, &ui, &UiEvent::Up(Button::Left), (70, 174));
        c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 27));
        let mut expected = vec![Event::Key(keys::DOWN); 12]; expected.push(Event::Key(b'+' as u16));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (130, 174)), decision::raw(&expected));
        let mut out = Surface::new(304, 180); c.draw(&s, &ui, &mut out); assert!(out.text.visible.len() > 20);
        c.input(&s, &ui, &UiEvent::Wheel(1), (0, 0));
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 40)).is_empty(), "disabled heading");
    }
    #[ignore = "the action list is disabled; set choices::ENABLED to run"]
    #[test]
    fn starport_desk_payer_row_commits_and_ack_back_has_a_button() {
        let c = Choices::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Picture);
        let ui = ui(list(false));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 51)), decision::raw(&[Event::Key(keys::DOWN), Event::Key(keys::DOWN), Event::Key(keys::ENTER)]));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (270, 174)), decision::raw(&[Event::Key(keys::ESC)]));
        for kind in [Kind::Ack, Kind::YesNo] { let mut d = Decision::new(kind, "t", "D2C3"); d.options.push(Opt::new("back", "Back", vec![Event::Key(keys::ESC)]));
            let u = super::tests::ui(d); assert!(c.visible(&u)); assert_eq!(c.input(&s, &u, &UiEvent::Up(Button::Left), (10, 27)), decision::raw(&[Event::Key(keys::ESC)])); }
    }
    #[test]
    fn efate_shop_ack_buttons_leave_the_message_visible_above_them() {
        let mut d = Decision::new(Kind::Ack, "t", "161EE");
        d.options.push(Opt::new("continue", "Continue", vec![Event::Key(13)]));
        d.options.push(Opt::new("back", "Back", vec![Event::Key(27)]));
        let mut ui = ui(d); ui.preset = mt1::ui::Preset::Classic; ui.frame.indexed.fill(3);
        let mut layout = super::super::Layout::default(); super::super::components::register(&mut layout, Arc::default());
        let s = Scene::program(mt1::ui::ScreenKind::Picture); layout.arrange_for(&s, &ui, (320, 200));
        let super::super::Host::Dock(r) = layout.host("Choices").unwrap() else { panic!() };
        let h = height(ui.decision.as_ref().unwrap()); assert_eq!((r.y, r.h), (200 - 10 - h as i32, h), "Classic Ack: sized to its two rows, above the bottom");
        assert_eq!(layout.compose(&s, &ui, (320, 200)).get(160, 90), Some(3));
    }
}
