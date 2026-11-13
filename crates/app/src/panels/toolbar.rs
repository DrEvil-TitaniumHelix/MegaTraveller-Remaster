use super::{assets::Assets, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use crate::mouse::decision;
use mt_engine::play::Kind;
use mt1::ui::{Intent, Scene, ScreenKind};
use std::{cell::{Cell, RefCell}, sync::Arc};

pub const HEIGHT: u32 = 36;
pub struct Toolbar { assets: Arc<Assets>, page: Cell<usize>, displayed: RefCell<Option<mt_engine::play::Decision>> }
impl Toolbar {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets, page: Cell::new(0), displayed: RefCell::default() } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Bottom, size: Size { preferred: (320, HEIGHT), min: (100, HEIGHT) }, order: 80,
            screens: &[ScreenKind::Ground], when: super::always, origin: Origin::New, optional: false, undockable: false, title: "Commands" }
    }
    fn indices(ui: &UiState) -> Vec<usize> {
        ui.decision.as_ref().filter(|d| d.kind == Kind::Ground).map(|d| d.options.iter().enumerate()
            .filter_map(|(i, o)| o.id.starts_with("cmd:").then_some(i)).collect()).unwrap_or_default()
    }
    fn grid(size: (u32, u32)) -> (usize, usize, u32) {
        let columns = (size.0 / 76).max(1) as usize;
        (columns, (size.1.saturating_sub(12) / 12).max(1) as usize, size.0 / columns as u32)
    }
}
impl Component for Toolbar {
    fn visible(&self, ui: &UiState) -> bool {
        let live = !Self::indices(ui).is_empty();
        if ui.decision.is_some() { *self.displayed.borrow_mut() = live.then(|| ui.decision.clone().unwrap()); }
        live || ui.preset == mt1::ui::Preset::Modern
    }
    fn draw(&self, _scene: &Scene, ui: &UiState, out: &mut Surface) {
        self.visible(ui);
        let mut displayed = ui.clone();
        if displayed.decision.is_none() && ui.preset == mt1::ui::Preset::Modern { displayed.decision = self.displayed.borrow().clone(); }
        let ui = &displayed;
        let Some(d) = &ui.decision else { return };
        let indices = Self::indices(ui); let (cols, rows, width) = Self::grid((out.w as u32, out.h as u32));
        let count = cols * rows; let page = self.page.get().min(indices.len().saturating_sub(1) / count); self.page.set(page);
        out.indexed.fill(0);
        for (slot, &i) in indices.iter().skip(page * count).take(count).enumerate() {
            let r = Rect::new((slot % cols) as i32 * width as i32, (slot / cols) as i32 * 12, width, 12);
            let (label, hot) = super::choices::display(d, i);
            let label = label.replace("Select Traveller ", "Select ");
            if label.chars().count() > (width.saturating_sub(6) / 6) as usize {
                super::choices::button(&self.assets, out, r, "", d.options[i].enabled, false, None);
                self.assets.text_fit_hot(out, &label, Rect::new(r.x + 3, r.y + 2, width.saturating_sub(6), 8), if d.options[i].enabled { 15 } else { 8 }, hot);
            } else { super::choices::button(&self.assets, out, r, &label, d.options[i].enabled, false, hot); }
        }
        if indices.len() > count {
            let y = out.h.saturating_sub(12) as i32;
            super::choices::button(&self.assets, out, Rect::new(0, y, 65, 12), "Previous", page > 0, false, None);
            self.assets.text(out, &format!("Commands {}/{}", page + 1, indices.len().div_ceil(count)), (70, y + 2), 7);
            super::choices::button(&self.assets, out, Rect::new(out.w as i32 - 100, y, 44, 12), "Next", (page + 1) * count < indices.len(), false, None);
        }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if *e != UiEvent::Up(Button::Left) { return vec![] }
        let size = ui.component_input.size;
        if !Rect::new(0, 0, size.0, size.1).contains(at) { return vec![] }
        let indices = Self::indices(ui); let (cols, rows, width) = Self::grid(size); let count = cols * rows;
        let page = self.page.get().min(indices.len().saturating_sub(1) / count);
        if at.1 >= size.1.saturating_sub(12) as i32 {
            if at.0 < 65 { self.page.set(page.saturating_sub(1)); }
            else if (size.0 as i32 - 100..size.0 as i32 - 56).contains(&at.0) && (page + 1) * count < indices.len() { self.page.set(page + 1); }
            return vec![]
        }
        let slot = (at.1 / 12) as usize * cols + (at.0 as u32 / width.max(1)) as usize;
        if let Some(&i) = indices.get(page * count + slot).filter(|_| slot < count) {
            return decision::option(ui.decision.as_ref().unwrap(), i)
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::Event, play::{Decision, Option as Opt}};
    #[test]
    fn efate_quiet_each_toolbar_button_is_its_native_command_including_second_page() {
        let mut d = Decision::new(Kind::Ground, "t", "6923");
        for (id, k) in [("back", 0x101), ("game", 0x122), ("talk", 0x12e), ("party", 0x119),
            ("inventory", 0x117), ("vehicle", 0x12f), ("orders", 0x118), ("reload", 0x113),
            ("char/0", 0x13b), ("char/1", 0x13c), ("char/2", 0x13d), ("char/3", 0x13e), ("char/4", 0x13f)] {
            d.options.push(Opt::new(format!("cmd:{id}"), id, vec![Event::Key(k)]));
        }
        let mut ui = UiState { decision: Some(d.clone()), ..UiState::default() }; ui.component_input.size = (320, HEIGHT);
        let c = Toolbar::new(Arc::default()); let s = Scene::program(ScreenKind::Ground);
        for (i, o) in d.options.iter().enumerate() {
            if i == 8 { assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (240, 30)).is_empty()); }
            let slot = i % 8; let at = ((slot % 4) as i32 * 80 + 2, (slot / 4) as i32 * 12 + 2);
            assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), at), decision::raw(&o.input), "{}", o.id);
        }
        ui.decision.as_mut().unwrap().options[8].enabled = false;
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (2, 2)).is_empty());
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (-1, 2)).is_empty());
        assert!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (2, 2)).is_empty());
        let mut out = Surface::new(320, HEIGHT); c.draw(&s, &ui, &mut out); assert!(!out.text.visible.is_empty());
        ui.decision = None; assert!(c.visible(&ui));
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (2, 2)).is_empty(), "cached labels never dispatch stale commands");
        ui.preset = mt1::ui::Preset::Classic; assert!(!c.visible(&ui));
    }
    #[test]
    fn efate_quiet_commands_are_clickable_in_both_product_presets() {
        for preset in [mt1::ui::Preset::Modern, mt1::ui::Preset::Classic] {
            let mut layout = super::super::Layout::default(); super::super::components::register(&mut layout, Arc::default());
            let mut d = Decision::new(Kind::Ground, "t", "6923"); d.options.push(Opt::new("cmd:game", "Game", vec![Event::Key(0x122)]));
            let ui = UiState { decision: Some(d), preset, ..UiState::default() }; let s = Scene::program(ScreenKind::Ground);
            layout.arrange_for(&s, &ui, (320, 200)); assert!(layout.visible("Commands", &s));
            let super::super::Host::Dock(r) = layout.host("Commands").unwrap() else { panic!() };
            assert_eq!(layout.hit(&s, None, (r.x + 2, r.y + 2)).unwrap().0, "Commands");
            let composed = layout.compose(&s, &ui, (320, 200));
            assert!(composed.text.visible.iter().any(|run| run.bytes.as_slice() == b"G" && run.y >= r.y as i64), "Classic retains toolbar font annotations");
        }
    }
}
