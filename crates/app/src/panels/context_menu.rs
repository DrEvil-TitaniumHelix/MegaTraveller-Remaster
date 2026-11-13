use super::{assets::Assets, Button, Component, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene};
use mt_engine::io::keys;
use std::sync::Arc;

pub const ROW: i32 = 9; pub const WIDTH: u32 = 176; pub const ROWS: u32 = 10; pub const HEIGHT: u32 = 4 + ROWS * ROW as u32;
fn item_text(it: &mt1::ground::orders::MenuItem) -> (String, Option<usize>) {
    let (label, underline) = super::choices::shortcut(&it.label, &[mt_engine::io::Event::Key(it.key as u16)]);
    (match &it.grey { Some(r) => format!("{label} ({r})"), None => label }, underline)
}
fn item_rows(it: &mt1::ground::orders::MenuItem, width: u32) -> Vec<String> {
    super::side_panel::wrap(&item_text(it).0, (width.saturating_sub(6) / 6).max(1) as usize)
}
pub fn size(scene: &Scene, available: (u32, u32)) -> (u32, u32) {
    let Some(m) = &scene.squad.menu else { return (WIDTH.min(available.0), HEIGHT.min(available.1)) };
    let width = m.items.iter().map(|it| item_text(it).0.chars().count() as u32 * 6 + 6).max().unwrap_or(6).min(available.0);
    let rows: usize = m.items.iter().map(|it| item_rows(it, width).len()).sum();
    (width, (4 + rows as u32 * ROW as u32).min(available.1))
}
pub struct ContextMenu { assets: Arc<Assets> }
impl ContextMenu { pub fn new(assets: Arc<Assets>) -> Self { Self { assets } } }
impl Component for ContextMenu {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        let Some(m) = &scene.squad.menu else { return };
        let row = super::keyboard::menu_row(ui, m);
        out.indexed.fill(0);
        let h = (4 + m.items.iter().map(|it| item_rows(it, out.w as u32).len() as i32 * ROW).sum::<i32>()).min(out.h as i32);
        for x in 0..out.w as i32 { out.put(x, 0, 15); out.put(x, h - 1, 15); }
        for y in 0..h { out.put(0, y, 15); out.put(out.w as i32 - 1, y, 15); }
        let mut y = 2;
        for (i, it) in m.items.iter().enumerate() {
            let rows = item_rows(it, out.w as u32); let height = rows.len() as i32 * ROW;
            if i == row { for yy in y..y + height { for x in 1..out.w as i32 - 1 { out.put(x, yy, 1); } } }
            let hot = item_text(it).1.filter(|&i| rows.first().is_some_and(|r| i < r.len()));
            for (row, text) in rows.iter().enumerate() {
                self.assets.text_hot(out, text, (3, y + row as i32 * ROW + 1), if it.grey.is_some() { 8 } else { 15 }, if row == 0 { hot } else { None });
            }
            y += height;
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !crate::mouse::decision::pointer_allowed(scene, "Menu", e, at) { return vec![]; }
        let Some(m) = &scene.squad.menu else { return vec![Intent::CloseMenu] };
        let pick = |i: usize| vec![Intent::MenuPick(i as u8), Intent::CloseMenu];
        match *e {
            UiEvent::Up(Button::Left) => {
                let mut y = 2;
                for (i, it) in m.items.iter().enumerate() {
                    let end = y + item_rows(it, ui.component_input.size.0).len() as i32 * ROW;
                    if (y..end).contains(&at.1) { return pick(i) } y = end;
                }
                vec![Intent::CloseMenu]
            }
            UiEvent::Key(keys::ESC) => vec![Intent::CloseMenu],
            UiEvent::Key(keys::ENTER) => pick(super::keyboard::menu_row(ui, m)),
            UiEvent::Key(k) => { let k = (k as u8).to_ascii_uppercase(); m.items.iter().position(|it| it.key == k).map_or(Vec::new(), pick) }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt1::ground::orders::{Menu, MenuItem, Op};
    use mt1::ui::{Target, Unit};
    fn scene() -> Scene {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground);
        s.squad.menu = Some(Menu { target: Target::Unit(Unit::Npc(14)), default: 0, items: vec![
            MenuItem { key: b'F', label: "Fire".into(), op: Op::Fire, grey: None },
            MenuItem { key: b'A', label: "Target".into(), op: Op::SetTarget, grey: Some("gone".into()) }] });
        s
    }
    #[test]
    fn efate_broken_menu_contains_only_the_sim_offered_target_actions() {
        let mut s = scene(); let ui = UiState::default(); let c = ContextMenu::new(Arc::default());
        s.squad.menu.as_mut().unwrap().items.truncate(1);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let labels: Vec<_> = out.text.visible.iter().flat_map(|r| r.bytes.clone()).collect();
        assert_eq!(String::from_utf8(labels).unwrap(), "Fire");
        assert!(out.text.runs.iter().any(|r| r.bytes[0] == b'F' && r.colour & crate::pipeline::text::UNDERLINE != 0), "F is the hotkey");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (8, 4)), [Intent::MenuPick(0), Intent::CloseMenu]);
        assert!(c.input(&s, &ui, &UiEvent::Key(b'a' as u16), (0, 0)).is_empty());
    }
    #[test]
    fn picks_by_row_letter_and_enter_and_closes_on_escape() {
        let c = ContextMenu::new(Arc::default()); let s = scene(); let ui = UiState::default();
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 12)), [Intent::MenuPick(1), Intent::CloseMenu]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (10, 40)), [Intent::CloseMenu], "below the items: closes");
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(b'f' as u16), (0, 0)), [Intent::MenuPick(0), Intent::CloseMenu]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::ENTER), (0, 0)), [Intent::MenuPick(0), Intent::CloseMenu]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::ESC), (0, 0)), [Intent::CloseMenu]);
        assert!(c.input(&s, &ui, &UiEvent::Key(b'z' as u16), (0, 0)).is_empty());
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert_eq!((out.get(0, 0), out.get(5, 3), out.get(5, 12)), (Some(15), Some(1), Some(0)), "border, the default row highlighted, the greyed row plain");
        let none = Scene::program(mt1::ui::ScreenKind::Ground); assert_eq!(c.input(&none, &ui, &UiEvent::Up(Button::Left), (1, 1)), [Intent::CloseMenu]);
    }
}
