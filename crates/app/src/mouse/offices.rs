mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
use crate::panels::{picture::Picture, Button, Component, UiEvent, UiState};
use mt1::{ground::starport, ui::{Intent, Scene, ScreenKind}};
use mt_engine::{io::{Event, Input}, play::{Decision, Kind}, present::textlog, sim::tick::Clock};
use std::{cell::RefCell, collections::VecDeque, path::Path, rc::Rc};

#[derive(Default)]
struct Seen { scene: Option<Scene>, text: String, held: Vec<String>, waits: Vec<String>, polls: usize }
struct Reader {
    seen: Rc<RefCell<Seen>>, keys: VecDeque<u16>, phrases: VecDeque<&'static str>,
    decision: Option<Decision>, ready: bool, delay: usize, mouse_ack: bool,
}
impl Input for Reader {
    fn next_key(&mut self) -> u16 { self.keys.pop_front().unwrap_or(27) }
    fn wants_decision(&self) -> bool { true }
    fn present(&mut self, rgb: &[u32]) {
        if let Some(log) = textlog::take_frame(rgb) {
            self.seen.borrow_mut().text = super::decision::line_texts(&log.visible)
                .into_iter().map(|(_, s)| s).collect::<String>();
        }
    }
    fn scene(&mut self, s: &dyn std::any::Any) {
        if let Some(s) = s.downcast_ref::<Scene>() { self.seen.borrow_mut().scene = Some(s.clone()); }
    }
    fn decide(&mut self, d: &Decision) {
        let text = self.seen.borrow().text.clone();
        self.seen.borrow_mut().waits.push(format!("{:?}: {text}", d.kind));
        self.decision = Some(d.clone());
        self.ready = d.kind == Kind::Ack || d.context.routine == "DE5E";
        if self.ready && self.phrases.front().is_some_and(|p| self.seen.borrow().text.contains(&super::decision::norm_label(p))) {
            self.delay = 180;
        }
    }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        self.seen.borrow_mut().polls += 1;
        assert!(self.seen.borrow().polls < 3000, "office failed to return");
        if !self.ready { clock.wait_tick(); return Event::Idle }
        if self.delay > 0 {
            let seen = self.seen.borrow();
            assert_eq!(seen.scene.as_ref().unwrap().screen, ScreenKind::Picture);
            assert!(seen.text.contains(&super::decision::norm_label(self.phrases.front().unwrap())), "reply was overwritten while idle: {}", seen.text);
            drop(seen);
            self.delay -= 1; clock.wait_tick();
            if self.delay != 0 { return Event::Idle }
            let phrase = self.phrases.pop_front().unwrap();
            self.seen.borrow_mut().held.push(phrase.into());
            if self.mouse_ack && self.decision.as_ref().is_some_and(|d| d.kind == Kind::Ack) {
                let ui = UiState { decision: self.decision.clone(), ..Default::default() };
                let seen = self.seen.borrow();
                let intents = Picture::full().input(seen.scene.as_ref().unwrap(), &ui, &UiEvent::Up(Button::Left), (100, 100));
                let events: Vec<_> = intents.into_iter().filter_map(|i| if let Intent::Raw(e) = i { Some(e) } else { None }).collect();
                assert_eq!(events, [Event::Key(32), Event::KeyUp(32)], "the visible page accepts a click");
                self.keys.pop_front();
                self.ready = false;
                return events[0];
            }
        }
        self.ready = false;
        Event::Key(self.next_key())
    }
}

fn office(scenario: &str, event: i8, keys: Vec<u16>, phrases: &[&'static str], mouse_ack: bool) -> Option<(mt1::ground::Ground, Vec<String>)> {
    let dir = oracle_env::gamefiles()?;
    textlog::set_enabled(true);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut g = crate::scenario::load(&root.join(scenario), &dir).unwrap().game;
    let seen = Rc::new(RefCell::new(Seen::default()));
    g.input = Box::new(Reader { seen: seen.clone(), keys: keys.into(), phrases: phrases.iter().copied().collect(), decision: None, ready: false, delay: 0, mouse_ack });
    crate::scenario::attach_scene(&mut g); g.mem.set_event(event);
    assert!(starport::office(&mut g));
    textlog::set_enabled(false);
    let held = seen.borrow().held.clone();
    assert_eq!(held, phrases, "presented waits: {:?}", seen.borrow().waits);
    Some((g, held))
}

#[test]
fn hospital_cure_waits_for_a_key_or_page_click_after_either_payer() {
    for mouse in [false, true] { for account in [false, true] {
        let mut keys = if account { vec![0x150; 5] } else { vec![] };
        keys.extend([13, 13, 32]);
        let Some((g, held)) = office("scenarios/h9c_hospital_efate.toml", 15, keys, &["You're cured."], mouse) else { return };
        assert_eq!(held, ["You're cured."], "the full reply must survive 180 idle ticks before acknowledgement");
        assert_eq!(g.mem.party_u8(0x127), g.mem.party_u8(0x125));
        assert_eq!(starport::cash(&g, if account { -2 } else { 0 }), if account { 30000 - 2784 } else { 36334 - 2784 });
        assert_eq!(g.scene.as_ref().unwrap().screen, Some(ScreenKind::Ground));
    } }
}

#[test]
fn rental_policy_and_refund_survive_until_the_next_office_choice() {
    let phrases = ["security deposit.", "credited with your deposit.", "You have not rented a vehicle."];
    for mouse in [false, true] {
    let Some((g, held)) = office("tests/atoms/scenarios/atom_175_zenaca.toml", 11,
        vec![13, 13, 13, 32, 0x13C, 32, 0x13C, 27], &phrases, mouse) else { return };
    assert_eq!(held, phrases);
    assert_eq!(g.mem.u8(0x6D31), 0);
    assert_eq!(starport::cash(&g, -2), 28000, "one rental and one deposit refund, no second refund");
    }
}

#[test]
fn library_information_waits_for_a_page_click() {
    let mut keys = vec![0x150; 5]; keys.extend([13, b'+' as u16, 13, 32]);
    let Some((g, held)) = office("tests/atoms/scenarios/atom_177_library.toml", 13,
        keys, &["swampy regions."], true) else { return };
    assert_eq!(held, ["swampy regions."]);
    assert_eq!(starport::cash(&g, -2), 8000);
    assert_eq!(g.scene.as_ref().unwrap().screen, Some(ScreenKind::Ground));
}
