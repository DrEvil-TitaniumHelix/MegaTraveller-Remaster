use super::{assets::Assets, Anchor, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use crate::mouse::decision;
use mt_engine::{io::Event, play::{Decision, Kind, List}};
use mt1::ui::{Intent, Scene, ScreenKind};
use std::sync::Arc;

pub const ID: &str = "Cargo";
pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 200;
const TOP: i32 = 22; const ROW: i32 = 10; const MAX_ROWS: usize = 15;
const MINUS_X: i32 = 4; const PLUS_X: i32 = 16; const BUTTON_W: u32 = 10;
const NAME_X: i32 = 30; const PRICE_RIGHT: i32 = 230; const QTY_RIGHT: i32 = 256; const TOTAL_RIGHT: i32 = 316;
const DONE: Rect = Rect::new(WIDTH as i32 - 112, HEIGHT as i32 - 15, 52, 12); const BACK: Rect = Rect::new(WIDTH as i32 - 56, HEIGHT as i32 - 15, 52, 12);
pub(super) struct Ink { pub panel: u8, pub ink: u8, pub field: u8, pub selected: u8, pub dim: u8 }
impl Ink {
    pub(super) fn of(regs: &[u8; 16]) -> Self {
        let pick = |wants: &[u8], fallback: u8| wants.iter().find_map(|w| regs.iter().position(|r| r == w)).map_or(fallback, |i| i as u8);
        Self { panel: pick(&[7], 7), ink: pick(&[0], 0), field: pick(&[23, 63, 15], 15), selected: pick(&[1, 17], 1), dim: pick(&[16, 56, 8], 8) }
    }
}

pub struct Cargo { assets: Arc<Assets> }
impl Cargo {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Float(Anchor::Centre), size: Size { preferred: (WIDTH, HEIGHT), min: (WIDTH, HEIGHT) }, order: 95,
            screens: &[ScreenKind::Ground, ScreenKind::Picture], when: super::always, origin: Origin::New, optional: false, undockable: false, title: ID }
    }
    fn fill(out: &mut Surface, r: Rect, c: u8) { fill(out, r, c) }
    fn frame(out: &mut Surface, r: Rect, c: u8) { frame(out, r, c) }
    fn right(&self, out: &mut Surface, text: &str, right: i32, y: i32, colour: u8) { self.assets.text(out, text, (right - text.len() as i32 * 6, y), colour); }
}
pub(super) fn fill(out: &mut Surface, r: Rect, c: u8) { for y in r.y..r.y + r.h as i32 { for x in r.x..r.x + r.w as i32 { out.put(x, y, c); } } }
pub(super) fn frame(out: &mut Surface, r: Rect, c: u8) {
    for x in r.x..r.x + r.w as i32 { out.put(x, r.y, c); out.put(x, r.y + r.h as i32 - 1, c); }
    for y in r.y..r.y + r.h as i32 { out.put(r.x, y, c); out.put(r.x + r.w as i32 - 1, y, c); }
}
pub fn cargo_list(d: &Decision) -> Option<&List> {
    let l = d.list.as_ref()?;
    (d.kind == Kind::List && d.context.program == "t" && d.context.routine == "11571" && !l.increase.is_empty()
        && d.prompt.as_deref().is_some_and(|p| p.to_ascii_lowercase().contains("cargo"))).then_some(l)
}
fn page(d: &Decision, n: usize) -> (usize, usize) {
    let parsed = d.context.table.as_deref().and_then(|t| t.strip_prefix("page:")).and_then(|t| {
        let (first, rest) = t.split_once('+')?; let (rows, _) = rest.split_once('/')?;
        Some((first.parse::<usize>().ok()?, rows.parse::<usize>().ok()?))
    });
    let (first, rows) = parsed.unwrap_or((0, n)); (first.min(n), rows.clamp(1, MAX_ROWS))
}
pub fn columns(label: &str) -> (String, String) {
    let parts: Vec<&str> = label.split('^').map(str::trim).filter(|p| !p.is_empty()).collect();
    match parts.as_slice() { [name, price, ..] => ((*name).into(), dollars(price)), [name] => ((*name).into(), String::new()), [] => (label.trim().into(), String::new()) }
}
fn dollars(price: &str) -> String {
    let digits: String = price.chars().filter(char::is_ascii_digit).collect();
    if digits.is_empty() { price.into() } else { format!("${digits}") }
}
fn row_at(at: (i32, i32), shown: usize) -> Option<usize> {
    if at.1 < TOP || at.0 < 0 || at.0 >= WIDTH as i32 { return None; }
    let i = ((at.1 - TOP) / ROW) as usize; (i < shown).then_some(i)
}
impl Component for Cargo {
    fn visible(&self, ui: &UiState) -> bool { ui.decision.as_ref().and_then(cargo_list).is_some() }
    fn draw(&self, _scene: &Scene, ui: &UiState, out: &mut Surface) {
        let Some(d) = ui.decision.as_ref() else { return }; let Some(l) = cargo_list(d) else { return };
        let c = Ink::of(&ui.frame.regs);
        out.regs = ui.frame.regs; out.indexed.fill(c.panel); Self::frame(out, Rect::new(0, 0, WIDTH, HEIGHT), c.ink);
        let title = d.prompt.as_deref().unwrap_or("Cargo").trim_end_matches(':').trim();
        self.assets.text(out, title, (4, 3), c.ink);
        self.right(out, &format!("credits ${}", l.money), WIDTH as i32 - 4, 3, c.ink);
        self.assets.text(out, "item", (NAME_X, 13), c.dim); self.right(out, "price", PRICE_RIGHT, 13, c.dim); self.right(out, "qty", QTY_RIGHT, 13, c.dim); self.right(out, "total", TOTAL_RIGHT, 13, c.dim);
        let (first, rows) = page(d, l.rows.len());
        for (k, (i, row)) in l.rows.iter().enumerate().skip(first).take(rows).enumerate() {
            let y = TOP + k as i32 * ROW; let selected = i == l.current;
            if selected { Self::fill(out, Rect::new(NAME_X - 2, y - 1, WIDTH - NAME_X as u32, ROW as u32), c.selected); }
            let ink = if !row.enabled { c.dim } else if selected { c.field } else { c.ink };
            for (x, sign) in [(MINUS_X, "-"), (PLUS_X, "+")] {
                let r = Rect::new(x, y - 1, BUTTON_W, ROW as u32); Self::fill(out, r, if row.enabled { c.field } else { c.dim }); Self::frame(out, r, c.ink);
                self.assets.text(out, sign, (x + 2, y), c.ink);
            }
            let (name, price_text) = columns(&row.label);
            let price = if row.price > 0 { format!("${}", row.price) } else { price_text };
            self.assets.text(out, &name.chars().take(22).collect::<String>(), (NAME_X, y), ink);
            self.right(out, &price, PRICE_RIGHT, y, ink);
            if row.qty > 0 {
                self.right(out, &row.qty.to_string(), QTY_RIGHT, y, ink);
                if row.price > 0 { self.right(out, &format!("${}", row.price * u64::from(row.qty)), TOTAL_RIGHT, y, ink); }
            }
        }
        if l.more { self.assets.text(out, "more below: Down", (NAME_X, HEIGHT as i32 - 12), c.dim); }
        for (r, label) in [(DONE, "Done"), (BACK, "Back")] {
            Self::fill(out, r, c.field); Self::frame(out, r, c.ink);
            self.assets.text(out, label, (r.x + (r.w as i32 - label.len() as i32 * 6) / 2, r.y + 2), c.ink);
        }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let Some(d) = ui.decision.as_ref() else { return Vec::new() }; let Some(l) = cargo_list(d) else { return Vec::new() };
        match *e {
            UiEvent::Key(k) => vec![Intent::Raw(Event::Key(k))],
            UiEvent::Up(Button::Left) if DONE.contains(at) => decision::raw(&l.submit),
            UiEvent::Up(Button::Left) if BACK.contains(at) => d.options.iter().find(|o| o.enabled && o.input.first() == Some(&Event::Key(mt_engine::io::keys::ESC))).map_or_else(Vec::new, |o| decision::raw(&o.input)),
            UiEvent::Down(Button::Left) | UiEvent::Up(Button::Left) => {
                let press = matches!(*e, UiEvent::Down(_));
                let (first, rows) = page(d, l.rows.len());
                let Some(k) = row_at(at, l.rows.len().saturating_sub(first).min(rows)) else { return Vec::new() };
                let i = first + k; let Some(row) = l.rows.get(i).filter(|r| r.enabled) else { return Vec::new() };
                let minus = Rect::new(MINUS_X, 0, BUTTON_W, HEIGHT).contains((at.0, 1)); let plus = Rect::new(PLUS_X, 0, BUTTON_W, HEIGHT).contains((at.0, 1));
                if press != (minus || plus) { return Vec::new() }
                let mut events: Vec<Event> = if i == l.current { Vec::new() } else { row.input.clone() };
                if minus { events.extend(l.decrease.iter().copied()); } else if plus { events.extend(l.increase.iter().copied()); }
                if events.is_empty() { return Vec::new() }
                decision::raw(&events)
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::keys, play::Row};
    fn cargo_decision() -> Decision {
        let mut d = Decision::new(Kind::List, "t", "11571");
        d.prompt = Some("Cargo Available:".into()); d.context.table = Some("page:0+15/3".into());
        let rows = [("^streechen wine^3750$", 3750u64, 0u32, 7u32), ("^water^100$", 100, 2, 60), ("^textiles^500$", 0, 0, 0)].iter().enumerate().map(|(i, (label, price, qty, limit))| Row {
            id: format!("list:11571/{i}"), label: (*label).into(), enabled: *price > 0 || i == 2, price: *price, limit: *limit, qty: *qty, min_qty: 0,
            input: vec![Event::Key(if i == 0 { keys::UP } else { keys::DOWN }); i.max(1)],
        }).collect();
        d.list = Some(List { rows, current: 1, more: false, money: 29800, increase: vec![Event::Key(b'+' as u16)], decrease: vec![Event::Key(b'-' as u16)], submit: vec![Event::Key(keys::ENTER)] });
        d.options.push(mt_engine::play::Option::new("exit", "Exit", vec![Event::Key(keys::ESC)]));
        d
    }
    #[test]
    fn cargo_page_shows_the_list_and_sends_the_1990_keys_for_a_click() {
        let c = Cargo::new(Arc::new(Assets::default())); let s = Scene::program(ScreenKind::Ground);
        let mut ui = UiState::default();
        assert!(!c.visible(&ui));
        ui.decision = Some(cargo_decision()); assert!(c.visible(&ui));
        let mut weapons = cargo_decision(); weapons.prompt = Some("Weapons for sale:".into()); assert!(cargo_list(&weapons).is_none(), "only the cargo pages");
        assert_eq!(columns("^water^100$"), ("water".into(), "$100".into())); assert_eq!(columns("plain"), ("plain".into(), String::new()));
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        for want in ["Cargo Available", "credits $29800", "streechen wine", "$3750", "water", "$100", "2", "$200", "textiles", "$500", "Done", "Back"] { assert!(text.contains(want), "{want:?} in {text:?}"); }
        assert!(!text.contains("$0"), "a row without a quantity shows no total");
        let y = |row: i32| TOP + row * ROW + 4;
        assert_eq!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (PLUS_X + 3, y(1))), decision::raw(&[Event::Key(b'+' as u16)]), "+ acts on the press");
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (PLUS_X + 3, y(1))).is_empty(), "the release of + sends nothing twice");
        assert_eq!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (MINUS_X + 3, y(1))), decision::raw(&[Event::Key(b'-' as u16)]));
        assert_eq!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (PLUS_X + 3, y(0))), decision::raw(&[Event::Key(keys::UP), Event::Key(b'+' as u16)]));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (NAME_X + 20, y(0))), decision::raw(&[Event::Key(keys::UP)]));
        assert!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (NAME_X + 20, y(0))).is_empty(), "the name waits for the release");
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (NAME_X + 20, y(1))).is_empty(), "the current row's name: nothing to send");
        assert_eq!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (PLUS_X + 3, y(2))), decision::raw(&[Event::Key(keys::DOWN), Event::Key(keys::DOWN), Event::Key(b'+' as u16)]));
        assert!(c.input(&s, &ui, &UiEvent::Down(Button::Left), (PLUS_X + 3, y(3))).is_empty(), "below the last row");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (DONE.x + 5, DONE.y + 5)), decision::raw(&[Event::Key(keys::ENTER)]), "Done = the list's Enter");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (BACK.x + 5, BACK.y + 5)), decision::raw(&[Event::Key(keys::ESC)]), "Back = the exit option");
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(keys::ENTER), (0, 0)), [Intent::Raw(Event::Key(keys::ENTER))], "keys pass through to the 1990 page");
        let mut d2 = cargo_decision(); d2.context.table = Some("page:2+15/3".into()); ui.decision = Some(d2);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        assert!(text.contains("textiles") && !text.contains("water"));
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (PLUS_X + 3, y(1))).is_empty(), "one row on this page");
        ui.decision = Some(cargo_decision());
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let look = crate::look::Look { shape: crate::look::Shape::Square, ..Default::default() };
        let m = super::super::render::Metrics::component([WIDTH * 2, HEIGHT * 2], (WIDTH, HEIGHT), &look);
        let pixels = super::super::render::compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m, &look);
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/cargo-preview"); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("cargo.png"), crate::plus::png::rgb(WIDTH * 2, HEIGHT * 2, &pixels).unwrap()).unwrap();
        let mut l = super::super::Layout::default(); super::super::components::register(&mut l, Arc::default());
        let mut ui = UiState::default(); l.arrange_for(&s, &ui, (640, 480));
        assert!(!l.draw_host(&s, &ui, None).iter().any(|(id, ..)| *id == ID));
        ui.decision = Some(cargo_decision()); l.arrange_for(&s, &ui, (640, 480));
        assert!(l.draw_host(&s, &ui, None).iter().any(|(id, ..)| *id == ID), "the cargo page floats over the ground layout");
        let mut regs = mt_engine::present::palette::DEFAULT_REGS; regs[1] = 5; regs[15] = 6; regs[9] = 1; regs[14] = 23;
        let ink = Ink::of(&regs); assert_eq!((ink.selected, ink.field, ink.panel, ink.ink, ink.dim), (9, 14, 7, 0, 8));
        let ink = Ink::of(&mt_engine::present::palette::DEFAULT_REGS); assert_eq!((ink.selected, ink.field, ink.dim), (1, 15, 8));
    }
}
