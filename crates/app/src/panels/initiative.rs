use super::{assets::Assets, controls, minimap::attitude, Button, Component, Rect, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene, Target, Unit, Verb};
use std::sync::Arc;
const CHIP: i32 = 64;
const HEAD: i32 = 26;
pub const TURNS: &str = "Turn order: the sides alternate, one unit each. On your turn pick any Traveller who has not acted; the slot order is only the default. Clicking a name selects, it skips no one.";
pub struct Initiative { assets: Arc<Assets> }
impl Initiative { pub fn new(assets: Arc<Assets>) -> Self { Self { assets } } }
fn queue(scene: &Scene, ui: &UiState) -> Vec<Unit> {
    if scene.squad.encounter && !scene.squad.queue.is_empty() { return scene.squad.queue.iter().copied().take(37).collect() }
    if !scene.squad.encounter && !ui.debug_initiative { return Vec::new() }
    let mut seen = 0u8;
    scene.party.order().into_iter().filter(|&i| { if i >= 5 || seen & (1 << i) != 0 || scene.party.mask() & (1 << i) == 0 { return false } seen |= 1 << i; true }).map(Unit::Traveller).collect()
}
fn chip(width: usize, n: usize) -> i32 { if n == 0 || width <= HEAD as usize { CHIP } else { (((width - HEAD as usize) / n) as i32).clamp(24, CHIP) } }
impl Component for Initiative {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        if !scene.squad.encounter && !ui.debug_initiative { return }
        out.indexed.fill(8);
        self.assets.text(out, &format!("R{}", scene.squad.round), (2, 3), 15); self.assets.text(out, &format!("{}-{}", scene.squad.pools.0, scene.squad.pools.1), (2, 13), 7);
        let units = queue(scene, ui); let chip = chip(out.w, units.len());
        for (i, unit) in units.into_iter().enumerate().take(out.w.div_ceil(chip as usize)) {
            let x = i as i32 * chip + HEAD;
            let (name, colour, slot, facing) = match unit {
                Unit::Traveller(n) if n < 5 => (scene.party.name(n), n + 1, 2, scene.walkers[n as usize].facing()),
                Unit::Npc(n) => { let Some(npc) = n.checked_sub(1).and_then(|i| scene.npcs.get(i as usize)) else { continue }; (format!("NPC {n}"), attitude(npc.attitude()), npc.sprite(), npc.facing()) }
                _ => continue,
            };
            for dx in 0..chip - 1 { out.put(x + dx, 0, colour); out.put(x + dx, 23, colour); }
            let beside = chip - 22;
            if name.len() * 6 <= beside as usize {
                if let Some(b) = self.assets.cpic(scene, slot) { b.draw(out, Rect::new((facing & 7) as i32 * 16, 0, 16, 16), (x + 1, 4), Some(1)); }
                self.assets.text_fit(out, &name, Rect::new(x + 18, 5, beside as u32, 8), 15);
            } else {
                let chars: Vec<_> = name.chars().collect(); let mid = chars.len().div_ceil(2);
                for (line, chars) in chars.chunks(mid.max(1)).enumerate() {
                    self.assets.text_fit(out, &chars.iter().collect::<String>(), Rect::new(x + 2, 3 + line as i32 * 10, (chip - 4) as u32, 8), 15);
                }
            }
            if i == 0 { for y in 0..24 { out.put(x, y, 15); out.put(x + chip - 2, y, 15); } }
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !(0..24).contains(&at.1) { return Vec::new() }
        let units = queue(scene, ui); let chip = chip(ui.component_input.size.0 as usize, units.len());
        let unit = if at.0 < HEAD { None } else { units.get(((at.0 - HEAD) / chip) as usize).copied() };
        let Some(unit) = unit else {
            return if *e == UiEvent::Move && (scene.squad.encounter || ui.debug_initiative) { vec![Intent::Hover(None), Intent::Hint(Some(TURNS))] } else { Vec::new() } };
        match *e {
            UiEvent::Move => vec![Intent::Hover(Some(Target::Unit(unit)))],
            UiEvent::Up(Button::Left) => {
                if scene.squad.active {
                    return match unit { Unit::Traveller(i) => vec![Intent::Select(i)], Unit::Npc(_) => vec![Intent::Verb { verb: Verb::Fire, target: Target::Unit(unit) }] }
                }
                let mut out = vec![Intent::Focus(unit)]; if let Some(p) = controls::position(scene, unit) { out.push(Intent::PanTo(controls::cell(p, scene))); } out
            }
            UiEvent::Key(k) => vec![Intent::Raw(mt_engine::io::Event::Key(k))],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_bar_head_explains_the_turn_order_on_hover() {
        let c = Initiative::new(Arc::default()); let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.encounter = true; s.squad.active = true;
        let ui = UiState::default();
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (5, 10)), [Intent::Hover(None), Intent::Hint(Some(TURNS))], "the round counter");
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (300, 10)), [Intent::Hover(None), Intent::Hint(Some(TURNS))], "the empty bar past the chips");
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (5, 10)).is_empty(), "a click there does nothing");
        assert!(c.input(&s, &ui, &UiEvent::Move, (5, 30)).is_empty(), "below the bar");
    }
    #[test]
    fn squad_chips_select_travellers_and_offer_npc_targeting() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground);
        s.squad.active = true; s.squad.encounter = true; s.squad.queue = vec![Unit::Traveller(2), Unit::Npc(14)];
        let c = Initiative::new(Arc::default()); let ui = UiState::default();
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD + 3, 8)), [Intent::Select(2)]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD + CHIP + 3, 8)), [Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }]);
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD + 2 * CHIP, 8)).is_empty());
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD, 24)).is_empty());
    }
    #[test]
    fn encounter_gate_and_debug_party_order_never_select() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.party.0 = vec![7, 2, 0, 1, 255, 2];
        let mut ui = UiState::default(); assert!(queue(&s, &ui).is_empty()); ui.debug_initiative = true;
        assert_eq!(queue(&s, &ui), [Unit::Traveller(2), Unit::Traveller(0), Unit::Traveller(1)]);
        let c = Initiative::new(Arc::default()); assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (1, 1)).is_empty(), "the round head is no chip");
        let intents = c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD + 1, 1));
        assert!(matches!(intents[0], Intent::Focus(Unit::Traveller(2)))); assert!(!intents.iter().any(|i| matches!(i, Intent::Select(_) | Intent::Verb { .. })));
        ui.debug_initiative = false; s.squad.encounter = true; s.squad.queue = vec![Unit::Npc(2)]; assert_eq!(queue(&s, &ui), [Unit::Npc(2)]);
    }
    #[test]
    fn eight_entries_fit_the_row_with_whole_names() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.active = true; s.squad.encounter = true;
        s.party.0 = vec![0; 0x1000]; s.npc_count = 3;
        for i in 0..5 { s.party.0[0x102 + i * 256..0x10A + i * 256].copy_from_slice(b"Magellan"); }
        s.squad.queue = vec![Unit::Traveller(0), Unit::Traveller(1), Unit::Traveller(2), Unit::Traveller(3), Unit::Traveller(4), Unit::Traveller(0), Unit::Traveller(1), Unit::Traveller(2)];
        let c = Initiative::new(Arc::default()); let mut ui = UiState::default(); ui.component_input.size = (510, 24);
        let mut out = Surface::new(510, 24); c.draw(&s, &ui, &mut out);
        let w = chip(510, 8); assert_eq!(w, 60); assert!(HEAD + 8 * w <= 510);
        for i in 0..8 {
            let left = HEAD + i * w;
            let name: String = out.text.visible.iter().filter(|r| r.x >= left as i64 && r.x < (left + w) as i64).map(|r| r.bytes[0] as char).collect();
            assert_eq!(name, "Magellan");
        }
        assert!(out.text.visible.iter().all(|r| r.x + i64::from(r.width) <= 510), "no glyph past the row");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (HEAD + 7 * w + 3, 8)), [Intent::Select(2)], "the eighth chip answers");
        assert_eq!(chip(640, 8), CHIP, "at the minimum Modern window the chips keep their full width");
    }
}
