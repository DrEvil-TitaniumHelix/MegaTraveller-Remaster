use super::UiState;
use mt1::ui::{Cell, Intent, Scene, ScreenKind, Target, Unit, Verb};
use mt_engine::io::keys;

#[derive(Clone, Debug, Default)]
pub struct State { pub cursor: Option<Cell>,
    pub menu_row: Option<usize> }
pub fn menu_row(ui: &UiState, menu: &mt1::ground::orders::Menu) -> usize { ui.keyboard.menu_row.filter(|&r| r < menu.items.len()).unwrap_or(menu.default) }

fn bounded(scene: &Scene, cell: Cell) -> Cell {
    let row = (scene.view.0 / 2).max(0).min(scene.map.rows.saturating_sub(1) as i16);
    let col = (scene.view.1 / 2).max(0).min(scene.map.cols.saturating_sub(1) as i16);
    (cell.0.clamp(row, (row + 6).min(scene.map.rows.saturating_sub(1) as i16)),
        cell.1.clamp(col, (col + 11).min(scene.map.cols.saturating_sub(1) as i16)))
}

pub fn route(scene: &Scene, ui: &mut UiState, key: u16) -> Option<Vec<Intent>> {
    if scene.screen != ScreenKind::Ground || !scene.squad.active || crate::mouse::decision::modal(ui) { return None }
    if let Some(menu) = &scene.squad.menu {
        if key == keys::ESC { ui.keyboard.menu_row = None; return Some(vec![Intent::CloseMenu]) }
        let n = menu.items.len().max(1);
        if matches!(key, keys::UP | keys::DOWN) { ui.keyboard.menu_row = Some((menu_row(ui, menu) + if key == keys::UP { n - 1 } else { 1 }) % n); return Some(Vec::new()) }
        let index = if key == keys::ENTER { Some(menu_row(ui, menu)) }
            else { u8::try_from(key).ok().and_then(|k| menu.items.iter().position(|it| it.key == k.to_ascii_uppercase())) };
        return Some(index.map_or_else(Vec::new, |i| {
            let mut intents = Vec::new();
            if ui.keyboard.cursor.take().is_some() { intents.push(Intent::Hover(None)); }
            intents.extend([Intent::MenuPick(i as u8), Intent::CloseMenu]); intents
        }));
    }
    if ui.menu.is_some() { return Some(Vec::new()) }
    let letter = u8::try_from(key).ok().map(|k| k.to_ascii_lowercase());
    if letter == Some(b'x') {
        if ui.keyboard.cursor.take().is_some() { return Some(vec![Intent::Hover(None)]) }
        let pos = super::controls::position(scene, Unit::Traveller(scene.squad.selected))
            .map(|p| ((p.0 - 0.5) as i16, (p.1 - 0.5) as i16)).unwrap_or((scene.pos.0 / 2, scene.pos.1 / 2));
        let cell = bounded(scene, pos); ui.keyboard.cursor = Some(cell);
        return Some(vec![Intent::Hover(Some(Target::Cell(cell)))]);
    }
    if let Some(cell) = ui.keyboard.cursor {
        let step = match key {
            keys::UP => Some((-1, 0)), keys::DOWN => Some((1, 0)), keys::LEFT => Some((0, -1)), keys::RIGHT => Some((0, 1)),
            keys::HOME => Some((-1, -1)), keys::PGUP => Some((-1, 1)), keys::END => Some((1, -1)), keys::PGDN => Some((1, 1)), _ => None,
        };
        if let Some((r, c)) = step {
            let cell = bounded(scene, (cell.0 + r, cell.1 + c)); ui.keyboard.cursor = Some(cell);
            return Some(vec![Intent::Hover(Some(Target::Cell(cell)))]);
        }
        if key == keys::ESC { ui.keyboard.cursor = None; return Some(vec![Intent::Hover(None)]) }
        if key == keys::ENTER {
            ui.keyboard.cursor = None;
            return Some(vec![Intent::Hover(None), Intent::Verb { verb: Verb::Move, target: Target::Cell(cell) }]);
        }
        if letter != Some(b'm') { return Some(Vec::new()) }
    }
    if letter == Some(b'm') {
        let standing = scene.walkers[scene.squad.selected.min(4) as usize].target();
        let target = ui.keyboard.cursor.map(Target::Cell).or(ui.hover)
            .or_else(|| (scene.combat && standing > 0).then_some(Target::Unit(Unit::Npc(standing as u8))))
            .unwrap_or(Target::Portrait(scene.squad.selected));
        return Some(vec![Intent::OpenMenu { target, at: (0, 0) }]);
    }
    if letter == Some(b'e') && scene.squad.encounter {
        return Some(if super::combat_bar::Action::EndTurn.enabled(scene) { vec![Intent::EndTurn] } else { Vec::new() });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt1::ground::orders::{Menu, MenuItem, Op};
    #[test]
    fn menu_keys_move_the_row_pick_it_and_never_escape_to_ground_commands() {
        let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true;
        s.squad.menu = Some(Menu { target: Target::Portrait(0), default: 1, items: vec![
            MenuItem { key: b'S', label: "Select".into(), op: Op::Select(0), grey: None },
            MenuItem { key: b'R', label: "Reload".into(), op: Op::Reload, grey: None },
            MenuItem { key: b'W', label: "Swap hand".into(), op: Op::SwapHand, grey: Some("down".into()) }] });
        let mut ui = UiState::default();
        assert_eq!(menu_row(&ui, s.squad.menu.as_ref().unwrap()), 1);
        assert_eq!(route(&s, &mut ui, keys::DOWN), Some(vec![])); assert_eq!(ui.keyboard.menu_row, Some(2));
        assert_eq!(route(&s, &mut ui, keys::DOWN), Some(vec![])); assert_eq!(ui.keyboard.menu_row, Some(0), "wraps");
        assert_eq!(route(&s, &mut ui, keys::UP), Some(vec![])); assert_eq!(ui.keyboard.menu_row, Some(2));
        assert_eq!(route(&s, &mut ui, keys::ENTER), Some(vec![Intent::MenuPick(2), Intent::CloseMenu]), "Enter picks the highlighted row");
        assert_eq!(route(&s, &mut ui, b'r' as u16), Some(vec![Intent::MenuPick(1), Intent::CloseMenu]));
        assert_eq!(route(&s, &mut ui, b'q' as u16), Some(vec![]), "an unknown letter never reaches the ground commands");
        assert_eq!(route(&s, &mut ui, keys::ESC), Some(vec![Intent::CloseMenu])); assert_eq!(ui.keyboard.menu_row, None);
        ui.keyboard.menu_row = Some(9); assert_eq!(menu_row(&ui, s.squad.menu.as_ref().unwrap()), 1, "a stale row falls back to the default");
    }
}
