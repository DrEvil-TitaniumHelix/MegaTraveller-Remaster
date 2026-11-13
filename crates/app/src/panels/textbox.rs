use super::{assets::Assets, choices::button, Anchor, Button, Component, Dock, Layout, Origin, Picture, Placement, Rect, Size, Surface, UiEvent, UiState};
use crate::mouse::decision;
use mt1::ui::{Intent, Scene};
use mt_engine::{io::{keys, Event}, play::{Charset, Decision, Kind, TextInput}};
use std::{cell::RefCell, sync::Arc};

pub const ID: &str = "Text input";
const WIDTH: u32 = 304;
const HEIGHT: u32 = 156;
const ROW: u32 = 14;

pub fn register(layout: &mut Layout, assets: Arc<Assets>) {
    layout.register(Box::new(Textbox::new(assets.clone())), placement(ID, (WIDTH, HEIGHT)));
    layout.register(Box::new(Ack), placement("Continue", (320, 200)));
    let _ = assets;
}

fn placement(title: &'static str, size: (u32, u32)) -> Placement {
    Placement { dock: Dock::Float(Anchor::Centre), size: Size { preferred: size, min: size }, order: 96,
        screens: &[], when: super::always, origin: Origin::New, optional: false, undockable: false, title }
}
pub fn active(ui: &UiState) -> bool {
    ui.decision.as_ref().is_some_and(|d| matches!(d.kind, Kind::Text | Kind::Number) && d.input.is_some())
}
pub fn before_scene(layout: &mut Layout, ui: &mut UiState, event: UiEvent, at: Option<(i32, i32)>) -> Vec<Intent> {
    let scene = Scene::program(mt1::ui::ScreenKind::Picture);
    layout.arrange_for(&scene, ui, (320, 200));
    let target = ui.capture.and_then(|id| layout.host(id).map(|host| {
        let local = match (host, at) { (super::Host::Dock(r), Some(at)) => (at.0 - r.x, at.1 - r.y), _ => (-1, -1) };
        (id, local)
    })).or_else(|| at.and_then(|at| layout.hit(&scene, None, at)));
    let result = if let Some((id, local)) = target.filter(|_| ui.decision.is_some()) {
        if matches!(event, UiEvent::Down(_)) { ui.capture = Some(id); }
        super::controls::dispatch(layout, id, &scene, ui, &event, local)
    } else { vec![] };
    if matches!(event, UiEvent::Up(_)) { ui.capture = None; }
    result
}
fn valid(input: &TextInput, value: &str) -> bool {
    value.len() < input.max_len && value.is_ascii() && value.bytes().all(|b| match &input.charset {
        Charset::Digits => b.is_ascii_digit(), Charset::Upper => b.is_ascii_uppercase() || b.is_ascii_digit(),
        Charset::Ascii(chars) => chars.as_bytes().contains(&b),
    }) && input.width.as_ref().is_none_or(|(limit, widths)| {
        value.bytes().try_fold(0u32, |n, b| n.checked_add(u32::from(*widths.get(b as usize)?))).is_some_and(|n| n <= *limit)
    })
}
fn alphabet(input: &TextInput) -> Vec<u8> {
    (32..=126).filter(|b: &u8| match &input.charset {
        Charset::Digits => b.is_ascii_digit(), Charset::Upper => b.is_ascii_uppercase() || b.is_ascii_digit(),
        Charset::Ascii(chars) => chars.as_bytes().contains(b),
    }).collect()
}
#[derive(Default)]
struct Draft { identity: String, value: String, focused: bool, page: usize }
pub struct Textbox { assets: Arc<Assets>, draft: RefCell<Draft> }
impl Textbox {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets, draft: RefCell::default() } }
    fn sync(&self, d: &Decision) {
        let identity = format!("{}:{}:{:?}:{:?}", d.id, d.state, d.context, d.input);
        let mut draft = self.draft.borrow_mut();
        if draft.identity != identity { *draft = Draft { identity, focused: true, ..Draft::default() }; }
    }
    fn field(size: (u32, u32)) -> Rect { Rect::new(4, 18, size.0.saturating_sub(8), 18) }
    fn grid(size: (u32, u32)) -> (usize, usize, u32) {
        let cols = (size.0 / 20).max(1) as usize;
        (cols, (size.1.saturating_sub(70) / ROW).max(1) as usize, size.0 / cols as u32)
    }
    fn controls(size: (u32, u32)) -> [Rect; 4] {
        let width = size.0 / 4;
        std::array::from_fn(|i| Rect::new(i as i32 * width as i32, size.1.saturating_sub(ROW) as i32, width, ROW))
    }
    fn cancel(d: &Decision) -> Vec<Intent> {
        d.options.iter().position(|o| o.id == "back").map_or_else(Vec::new, |i| decision::option(d, i))
    }
    fn submit(&self, d: &Decision) -> Vec<Intent> {
        let Some(input) = &d.input else { return vec![] };
        let draft = self.draft.borrow();
        if input.submit.is_empty() || !valid(input, &draft.value) { return vec![] }
        decision::raw(&draft.value.bytes().map(|b| Event::Key(u16::from(b))).chain(input.submit.iter().copied()).collect::<Vec<_>>())
    }
    fn edit(&self, input: &TextInput, key: u16) {
        let mut draft = self.draft.borrow_mut();
        if !draft.focused { return }
        if key == keys::BACKSPACE { draft.value.pop(); return }
        let Ok(mut b) = u8::try_from(key) else { return };
        if input.charset == Charset::Upper { b = b.to_ascii_uppercase(); }
        let mut value = draft.value.clone(); value.push(char::from(b));
        if valid(input, &value) { draft.value = value; }
    }
}
impl Component for Textbox {
    fn visible(&self, ui: &UiState) -> bool { active(ui) }
    fn draw(&self, _scene: &Scene, ui: &UiState, out: &mut Surface) {
        if !self.visible(ui) { return }
        let d = ui.decision.as_ref().unwrap(); self.sync(d);
        let input = d.input.as_ref().unwrap(); let draft = self.draft.borrow();
        let size = (out.w as u32, out.h as u32); out.indexed.fill(0);
        self.assets.text(out, d.prompt.as_deref().unwrap_or(if d.kind == Kind::Number { "Enter a number" } else { "Enter text" }), (4, 4), 14);
        let field = Self::field(size);
        button(&self.assets, out, field, "", true, draft.focused, None);
        let shown = (field.w.saturating_sub(12) / 6) as usize;
        let start = draft.value.len().saturating_sub(shown.saturating_sub(1));
        self.assets.text(out, &format!("{}{}", &draft.value[start..], if draft.focused { "_" } else { "" }), (field.x + 3, field.y + 4), 15);
        self.assets.text(out, if draft.focused { "Choose characters below or type" } else { "Click the box to enter text" }, (4, 39), 7);
        let chars = alphabet(input); let (cols, rows, width) = Self::grid(size); let count = cols * rows;
        for (slot, &b) in chars.iter().skip(draft.page * count).take(count).enumerate() {
            let r = Rect::new((slot % cols) as i32 * width as i32, 52 + (slot / cols) as i32 * ROW as i32, width, ROW);
            button(&self.assets, out, r, if b == b' ' { "_" } else { std::str::from_utf8(std::slice::from_ref(&b)).unwrap() }, true, false, None);
        }
        let cancel = d.options.iter().any(|o| o.id == "back" && o.enabled);
        for (r, (label, enabled)) in Self::controls(size).into_iter().zip([
            ("Erase", !draft.value.is_empty()), ("More", chars.len() > count),
            ("OK", !input.submit.is_empty() && valid(input, &draft.value)), ("Cancel", cancel),
        ]) { button(&self.assets, out, r, label, enabled, false, None); }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, event: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !self.visible(ui) { return vec![] }
        let d = ui.decision.as_ref().unwrap(); self.sync(d); let input = d.input.as_ref().unwrap();
        if let UiEvent::Key(key) = *event {
            match key { keys::ESC => return Self::cancel(d), keys::ENTER => return self.submit(d), _ => self.edit(input, key) }
            return vec![]
        }
        let size = ui.component_input.size;
        if *event != UiEvent::Up(Button::Left) || !Rect::new(0, 0, size.0, size.1).contains(at) { return vec![] }
        if Self::field(size).contains(at) { self.draft.borrow_mut().focused = true; return vec![] }
        let chars = alphabet(input); let (cols, rows, width) = Self::grid(size); let count = cols * rows;
        if let Some(i) = Self::controls(size).iter().position(|r| r.contains(at)) {
            match i {
                0 => { self.draft.borrow_mut().value.pop(); },
                1 => { let mut draft = self.draft.borrow_mut(); draft.page = (draft.page + 1) % chars.len().div_ceil(count).max(1); },
                2 => return self.submit(d), 3 => return Self::cancel(d), _ => unreachable!(),
            }
        } else if at.1 >= 52 && (at.1 - 52) < rows as i32 * ROW as i32 {
            let column = (at.0 as u32 / width.max(1)) as usize;
            if column >= cols { return vec![] }
            let slot = ((at.1 - 52) / ROW as i32) as usize * cols + column;
            let page = self.draft.borrow().page;
            if let Some(&b) = chars.get(page * count + slot).filter(|_| slot < count) {
                self.draft.borrow_mut().focused = true; self.edit(input, u16::from(b));
            }
        }
        vec![]
    }
}

pub struct Ack;
impl Component for Ack {
    fn visible(&self, ui: &UiState) -> bool { ui.decision.as_ref().is_some_and(|d| d.kind == Kind::Ack) }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        if self.visible(ui) { Picture::full().draw(scene, ui, out); }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, event: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !self.visible(ui) || *event != UiEvent::Up(Button::Left) { return vec![] }
        let size = ui.component_input.size;
        if !Rect::new(0, 0, size.0, size.1).contains(at) { return vec![] }
        let d = ui.decision.as_ref().unwrap();
        d.options.iter().position(|o| Some(&o.id) == d.default.as_ref()).or_else(|| d.options.iter().position(|o| o.id == "continue")).map_or_else(Vec::new, |i| decision::option(d, i))
    }
}

#[cfg(test)]
#[path = "textbox/native_tests.rs"]
mod native_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::Input, play::{Choice, Option as Opt, Pick, Play}, sim::tick::Counter};
    fn field(kind: Kind, charset: Charset) -> Decision {
        let mut d = Decision::new(kind, "t", "15B69");
        d.input = Some(TextInput { charset, max_len: 8, width: None, submit: vec![Event::Key(keys::ENTER)] });
        d.options.push(Opt::new("back", "Cancel", vec![Event::Key(keys::ESC)])); d
    }
    fn ui(d: Decision, size: (u32, u32)) -> UiState {
        let mut ui = UiState { decision: Some(d), ..UiState::default() }; ui.component_input.size = size; ui
    }
    fn scene() -> Scene { Scene::program(mt1::ui::ScreenKind::Picture) }
    fn native(d: &Decision, pick: Pick, count: usize) -> Vec<Intent> {
        let mut p = Play::new(); p.register(d); p.choose(Choice::new(p.pending().unwrap().id.clone(), pick)).unwrap();
        (0..count).map(|_| Intent::Raw(p.poll(&mut Counter::at(0)))).collect()
    }
    fn click(c: &impl Component, ui: &UiState, at: (i32, i32)) -> Vec<Intent> { c.input(&scene(), ui, &UiEvent::Up(Button::Left), at) }
    #[test]
    fn g6_bank_keyboard_focus_on_open_edits_and_submits() {
        let c = Textbox::new(Arc::default()); let u = ui(field(Kind::Number, Charset::Digits), (WIDTH, HEIGHT));
        for key in [b'1' as u16, b'2' as u16, keys::BACKSPACE, b'0' as u16, b'0' as u16, b'0' as u16] {
            c.input(&scene(), &u, &UiEvent::Key(key), (0, 0));
        }
        assert!(c.draft.borrow().focused);
        assert_eq!(c.draft.borrow().value, "1000");
        assert_eq!(c.input(&scene(), &u, &UiEvent::Key(keys::ENTER), (0, 0)), native(u.decision.as_ref().unwrap(), Pick::Number(1000), 10));
    }
    #[test]
    fn starport_number_focus_edit_submit_and_cancel_match_native() {
        let c = Textbox::new(Arc::default()); let mut u = ui(field(Kind::Number, Charset::Digits), (WIDTH, HEIGHT));
        c.input(&scene(), &u, &UiEvent::Key(b'9' as u16), (0, 0)); assert_eq!(c.draft.borrow().value, "9");
        c.input(&scene(), &u, &UiEvent::Key(keys::BACKSPACE), (0, 0));
        click(&c, &u, (8, 24));
        for key in [b'1' as u16, b'2' as u16, b'A' as u16, keys::BACKSPACE, b'3' as u16] { c.input(&scene(), &u, &UiEvent::Key(key), (0, 0)); }
        assert_eq!(c.draft.borrow().value, "13");
        let output = click(&c, &u, (180, 148)); assert_eq!(output, native(u.decision.as_ref().unwrap(), Pick::Number(13), 6));
        assert_eq!(click(&c, &u, (270, 148)), native(u.decision.as_ref().unwrap(), Pick::Id("back".into()), 2));
        u.decision.as_mut().unwrap().options[0].enabled = false; assert!(click(&c, &u, (270, 148)).is_empty());
        for event in [UiEvent::Down(Button::Left), UiEvent::Up(Button::Right), UiEvent::Move] { assert!(c.input(&scene(), &u, &event, (180, 148)).is_empty()); }
        assert!(click(&c, &u, (-1, 148)).is_empty());
    }
    #[test]
    fn cg_name_mouse_palette_erases_pages_and_respects_native_width() {
        let c = Textbox::new(Arc::default()); let mut d = field(Kind::Text, Charset::Ascii(" ABCD".into()));
        d.context.program = "cg".into(); d.context.routine = "5CC2".into();
        d.input.as_mut().unwrap().width = Some((8, vec![4; 128]));
        let mut u = ui(d, (WIDTH, HEIGHT));
        for x in [21, 41, 61] { click(&c, &u, (x, 54)); }
        assert_eq!(c.draft.borrow().value, "AB");
        click(&c, &u, (4, 148)); assert_eq!(c.draft.borrow().value, "A");
        click(&c, &u, (81, 54)); assert_eq!(c.draft.borrow().value, "AD");
        assert_eq!(click(&c, &u, (180, 148)), native(u.decision.as_ref().unwrap(), Pick::Text("AD".into()), 6));
        u.decision.as_mut().unwrap().state = "new wait".into();
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&scene(), &u, &mut out); assert!(c.draft.borrow().value.is_empty());
        assert!(out.text.visible.iter().any(|r| r.bytes == b"C"));
        u.decision.as_mut().unwrap().input.as_mut().unwrap().charset = Charset::Ascii((32..=126).map(char::from).collect());
        u.component_input.size = (120, 84); click(&c, &u, (35, 76));
        click(&c, &u, (1, 54)); assert_eq!(c.draft.borrow().value, "&");
    }
    #[test]
    fn text_limits_uppercase_empty_submit_and_visibility_follow_metadata() {
        let c = Textbox::new(Arc::default()); let mut d = field(Kind::Text, Charset::Upper); d.input.as_mut().unwrap().max_len = 3;
        let mut u = ui(d, (WIDTH, HEIGHT)); click(&c, &u, (8, 24));
        for b in b"ab4!" { c.input(&scene(), &u, &UiEvent::Key(u16::from(*b)), (0, 0)); }
        assert_eq!(c.draft.borrow().value, "AB");
        assert!(!valid(u.decision.as_ref().unwrap().input.as_ref().unwrap(), "a"));
        assert!(!valid(u.decision.as_ref().unwrap().input.as_ref().unwrap(), "\u{e9}"));
        for _ in 0..3 { click(&c, &u, (5, 148)); }
        assert_eq!(click(&c, &u, (180, 148)), decision::raw(&[Event::Key(keys::ENTER)]));
        u.decision.as_mut().unwrap().input.as_mut().unwrap().submit.clear(); assert!(click(&c, &u, (180, 148)).is_empty());
        for kind in [Kind::Menu, Kind::YesNo, Kind::List, Kind::Ack, Kind::Ground, Kind::Space, Kind::CombatTurn] {
            u.decision.as_mut().unwrap().kind = kind; assert!(!c.visible(&u)); assert!(click(&c, &u, (180, 148)).is_empty());
        }
        u.decision = None; assert!(!c.visible(&u));
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&scene(), &u, &mut out); assert!(out.text.visible.is_empty());
    }
    #[test]
    fn every_ack_message_and_button_is_the_advertised_route() {
        let c = Ack;
        for (program, routine) in [("t", "D2C3"), ("t", "161EE"), ("t", "save"), ("t", "restore"), ("cg", "7290"), ("s", "B287")] {
            let choices = if program == "t" { vec![("continue", "Continue", 32), ("back", "Back", keys::ESC)] } else { vec![("continue", "Continue", 32)] };
            let d = mt1::play_prompts::keys_decision(program, routine, Kind::Ack, &choices);
            let u = ui(d.clone(), (320, 200)); assert!(c.visible(&u));
            assert_eq!(click(&c, &u, (100, 100)), native(&d, Pick::Id("continue".into()), 2), "{program}.{routine}");
            assert_eq!(click(&c, &u, (300, 195)), native(&d, Pick::Id("continue".into()), 2), "no button strip: the bottom row is the page too");
        }
        for kind in [Kind::Text, Kind::Number, Kind::Menu, Kind::List, Kind::Ground, Kind::Space, Kind::CombatTurn, Kind::YesNo] {
            let u = ui(Decision::new(kind, "t", "other"), (320, 200)); assert!(!c.visible(&u)); assert!(click(&c, &u, (1, 190)).is_empty());
        }
    }
    #[test]
    fn title_before_scene_start_quit_capture_and_click_anywhere_is_start() {
        let mut layout = Layout::default(); super::super::components::register(&mut layout, Arc::default());
        let d = mt1::play_prompts::keys_decision("title", "0297", Kind::Ack, &[("start", "Start game", keys::ENTER), ("quit", "Quit", keys::ESC)]);
        for preset in [mt1::ui::Preset::Modern, mt1::ui::Preset::Classic] {
            let mut u = ui(d.clone(), (320, 200)); u.preset = preset;
            for x in [40, 240] {
                assert!(before_scene(&mut layout, &mut u, UiEvent::Down(Button::Left), Some((x, 190))).is_empty());
                assert_eq!(before_scene(&mut layout, &mut u, UiEvent::Up(Button::Left), Some((x, 190))), native(&d, Pick::Id("start".into()), 2));
            }
            assert_eq!(before_scene(&mut layout, &mut u, UiEvent::Up(Button::Left), Some((100, 100))), native(&d, Pick::Id("start".into()), 2));
            before_scene(&mut layout, &mut u, UiEvent::Down(Button::Left), Some((40, 190)));
            assert!(before_scene(&mut layout, &mut u, UiEvent::Up(Button::Left), None).is_empty()); assert!(u.capture.is_none());
            u.decision = None; assert!(before_scene(&mut layout, &mut u, UiEvent::Up(Button::Left), Some((40, 190))).is_empty());
            assert!(layout.draw(&scene(), &u).iter().all(|(id, _, _)| ![ID, "Continue", "Choices"].contains(id)));
        }
    }
}
