use super::{assets::Assets, cargo, Anchor, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use crate::mouse::decision;
use mt_engine::play::{Decision, Kind, List};
use mt1::ui::{Intent, Scene, ScreenKind};
use std::sync::Arc;

pub const ID: &str = "Buy";
pub const WIDTH: u32 = 240; pub const HEIGHT: u32 = 64;
const YES: Rect = Rect::new(40, 44, 64, 14); const CANCEL: Rect = Rect::new(136, 44, 64, 14);

pub struct Buy { assets: Arc<Assets> }
impl Buy {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Float(Anchor::Centre), size: Size { preferred: (WIDTH, HEIGHT), min: (WIDTH, HEIGHT) }, order: 96,
            screens: &[ScreenKind::Ground, ScreenKind::Picture], when: super::always, origin: Origin::New, optional: false, undockable: false, title: ID }
    }
}
pub fn shop_list(d: &Decision) -> Option<&List> {
    let l = d.list.as_ref()?;
    (d.kind == Kind::List && d.context.program == "t" && d.context.routine == "11571" && cargo::cargo_list(d).is_none()
        && l.rows.iter().any(|r| r.label.contains('$'))).then_some(l)
}
pub fn open(ui: &UiState) -> Option<(&Decision, &List, usize)> {
    let d = ui.decision.as_ref()?; let l = shop_list(d)?; let (id, i) = ui.buy.as_ref()?;
    (*id == d.id && *i < l.rows.len()).then_some((d, l, *i))
}
fn selling(d: &Decision) -> bool { d.prompt.as_deref().is_some_and(|p| p.to_ascii_lowercase().contains("sell")) }
fn question(d: &Decision, label: &str, check_in: bool) -> String {
    if check_in { return "Check in for this fee?".into() }
    let parts: Vec<&str> = label.split('^').map(str::trim).filter(|p| !p.is_empty()).collect();
    let training = d.prompt.as_deref().is_some_and(|p| p.to_ascii_lowercase().contains("training"));
    let room = (WIDTH as usize - 12) / 6;
    let q = match (training, parts.as_slice()) {
        (true, [who, skill, _, ..]) => [format!("Buy {skill} training for {who}?"), format!("Buy {skill} training?")].into_iter().find(|q| q.len() <= room),
        (_, [item, ..]) => Some(format!("{} {item}?", if selling(d) { "Sell" } else { "Buy" })).filter(|q| q.len() <= room),
        _ => None,
    };
    q.unwrap_or_else(|| (if selling(d) { "Sell this item?" } else { "Buy this item?" }).into())
}
impl Component for Buy {
    fn visible(&self, ui: &UiState) -> bool { open(ui).is_some() }
    fn draw(&self, _scene: &Scene, ui: &UiState, out: &mut Surface) {
        let Some((d, l, i)) = open(ui) else { return }; let row = &l.rows[i];
        let c = cargo::Ink::of(&ui.frame.regs);
        out.regs = ui.frame.regs; out.indexed.fill(c.panel);
        for r in [Rect::new(0, 0, WIDTH, HEIGHT), Rect::new(1, 1, WIDTH - 2, HEIGHT - 2)] { cargo::frame(out, r, c.ink); }
        let check_in = d.prompt.as_deref().is_some_and(|p| p.to_ascii_lowercase().contains("check in"));
        let (name, price_text) = cargo::columns(&row.label);
        self.assets.text(out, &question(d, &row.label, check_in), (6, 5), c.ink);
        let price = if row.price > 0 { format!("${}", row.price) } else { price_text };
        self.assets.text(out, &name.chars().take(26).collect::<String>(), (6, 17), c.selected);
        self.assets.text(out, &price, (WIDTH as i32 - 6 - price.len() as i32 * 6, 17), c.ink);
        if l.money > 0 { self.assets.text(out, &format!("credits ${}", l.money), (6, 29), c.dim); }
        for (r, label) in [(YES, "Yes"), (CANCEL, "Cancel")] {
            cargo::fill(out, r, c.field); cargo::frame(out, r, c.ink);
            self.assets.text(out, label, (r.x + (r.w as i32 - label.len() as i32 * 6) / 2, r.y + 3), c.ink);
        }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let Some((_, l, i)) = open(ui) else { return Vec::new() };
        match *e {
            UiEvent::Up(Button::Left) if YES.contains(at) => {
                let mut events = l.rows[i].input.clone(); events.extend(l.increase.iter().copied()); events.extend(l.submit.iter().copied());
                let mut out = decision::raw(&events); out.push(Intent::BuyPrompt(None)); out
            }
            UiEvent::Up(Button::Left) if CANCEL.contains(at) => vec![Intent::BuyPrompt(None)],
            UiEvent::Key(k) => vec![Intent::Raw(mt_engine::io::Event::Key(k))],
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::{keys, Event}, play::Row};
    pub fn shop_decision() -> Decision {
        let mut d = Decision::new(Kind::List, "t", "11571"); d.id = "d:7".into();
        d.prompt = Some("This is the armor I have.".into()); d.context.table = Some("page:0+10/3".into());
        let rows = [("^9mm pistol^108$", 108u64), ("^body pistol^435$", 435), ("^jack^0$", 0)].iter().enumerate().map(|(i, (label, price))| Row {
            id: format!("list:11571/{i}"), label: (*label).into(), enabled: *price > 0, price: *price, limit: 3, qty: 0, min_qty: 0,
            input: vec![Event::Key(keys::DOWN); i],
        }).collect();
        d.list = Some(List { rows, current: 0, more: false, money: 29800, increase: vec![Event::Key(b'+' as u16)], decrease: vec![Event::Key(0x14A)], submit: vec![Event::Key(keys::ENTER)] });
        d
    }
    #[test]
    fn a_row_opens_the_pop_up_and_yes_buys_one_through_the_1990_keys() {
        let c = Buy::new(Arc::new(Assets::default())); let s = Scene::program(ScreenKind::Picture);
        let mut ui = UiState::default(); ui.decision = Some(shop_decision());
        assert!(shop_list(ui.decision.as_ref().unwrap()).is_some()); assert!(!c.visible(&ui), "closed until a row is clicked");
        let mut cargo = shop_decision(); cargo.prompt = Some("Cargo Available:".into()); assert!(shop_list(&cargo).is_none(), "the cargo page keeps its table");
        let mut select = shop_decision(); select.list.as_mut().unwrap().increase.clear(); assert!(shop_list(&select).is_some(), "a priced select list (weapons) is a shop");
        let mut choice = shop_decision(); choice.list.as_mut().unwrap().increase.clear(); for r in &mut choice.list.as_mut().unwrap().rows { r.label = "buy items".into(); } assert!(shop_list(&choice).is_none(), "a plain choice list is not");
        ui.buy = Some(("d:9".into(), 1)); assert!(!c.visible(&ui), "a stale row from another list is closed");
        ui.buy = Some(("d:7".into(), 1)); assert!(c.visible(&ui));
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        for want in ["Buy body pistol?", "body pistol", "$435", "credits $29800", "Yes", "Cancel"] { assert!(text.contains(want), "{want:?} in {text:?}"); }
        let yes = c.input(&s, &ui, &UiEvent::Up(Button::Left), (YES.x + 5, YES.y + 5));
        let mut want = decision::raw(&[Event::Key(keys::DOWN), Event::Key(b'+' as u16), Event::Key(keys::ENTER)]); want.push(Intent::BuyPrompt(None));
        assert_eq!(yes, want, "Yes = the row's route, +, Enter, then close");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (CANCEL.x + 5, CANCEL.y + 5)), [Intent::BuyPrompt(None)]);
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (5, 5)).is_empty());
        let mut sell = shop_decision(); sell.prompt = Some("Dennar has the following items to sell: ".into()); ui.decision = Some(sell);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert!(out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>().contains("Sell body pistol?"));
        let mut train = shop_decision(); train.prompt = Some("Training Eligibility".into()); train.list.as_mut().unwrap().rows[1].label = "Magellan^carousing^$10000".into();
        assert_eq!(question(&train, &train.list.as_ref().unwrap().rows[1].label, false), "Buy carousing training for Magellan?");
        assert!(ui.apply(Intent::BuyPrompt(Some(0)))); assert_eq!(ui.buy, Some(("d:7".into(), 0)), "opened on the pending list");
        assert!(ui.apply(Intent::BuyPrompt(None))); assert_eq!(ui.buy, None);
    }
}
