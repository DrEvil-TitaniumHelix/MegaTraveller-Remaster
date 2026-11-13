use super::{assets::Assets, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use mt1::{ground::orders::Op, ui::{Intent, Scene, ScreenKind, Target, Unit}};
use mt_engine::{io::Event, play::Kind};
use std::sync::Arc;

pub const ID: &str = "Combat controls";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action { Fire, Reload, Orders, Party, EndTurn }
const ACTIONS: [Action; 5] = [Action::Fire, Action::Reload, Action::Orders, Action::Party, Action::EndTurn];

#[derive(Clone, Copy, Debug)]
struct Pending { op: Op, target: Target, selected: u8, round: u16, level: i8, encounter: bool }
#[derive(Clone, Debug, Default)]
pub struct State { pending: Option<Pending>, armed: Option<Pending> }

pub struct CombatBar { assets: Arc<Assets> }
impl CombatBar {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Top, size: Size { preferred: (320, 22), min: (320, 22) }, order: 6,
            screens: &[ScreenKind::Ground], when: |s| s.squad.active, origin: Origin::New,
            optional: false, undockable: true, title: ID }
    }
}
fn active(scene: &Scene) -> bool { scene.screen == ScreenKind::Ground && scene.squad.active }
pub fn spent(scene: &Scene) -> bool { scene.squad.encounter && !scene.squad.pips[scene.squad.selected.min(4) as usize][1] }
fn target(scene: &Scene) -> Option<Target> {
    let k = scene.walkers.get(scene.squad.selected as usize)?.target();
    (k > 0).then_some(Target::Unit(Unit::Npc(k as u8)))
}
impl Action {
    fn label(self, scene: &Scene) -> &'static str { match self {
        Self::Fire => "Fire", Self::Reload => "Reload", Self::Orders => "Orders",
        Self::Party => if scene.combat { "Regroup" } else { "Break" }, Self::EndTurn => "End turn",
    } }
    fn menu(self, scene: &Scene) -> Option<(Op, Target)> { match self {
        Self::Fire => target(scene).map(|t| (Op::Fire, t)),
        Self::Reload => Some((Op::Reload, Target::Unit(Unit::Traveller(scene.squad.selected)))),
        Self::Party => Some((Op::Party, Target::Unit(Unit::Traveller(scene.squad.selected)))),
        _ => None,
    } }
    pub(super) fn enabled(self, scene: &Scene) -> bool { active(scene) && match self {
        Self::Fire => target(scene).is_some() && !spent(scene), Self::Reload => scene.combat,
        Self::Party => if scene.combat { !scene.squad.encounter } else { scene.encounter_mode == 0 },
        Self::Orders | Self::EndTurn => scene.squad.encounter && scene.squad.to_act.is_some(),
    } }
    fn intents(self, scene: &Scene, at: (i32, i32)) -> Vec<Intent> {
        if !self.enabled(scene) { return Vec::new() }
        if let Some((_, target)) = self.menu(scene) { vec![Intent::OpenMenu { target, at }] }
        else if self == Self::Orders { vec![Intent::Continue] }
        else { vec![Intent::EndTurn] }
    }
}
fn rect(index: usize, width: u32) -> Rect {
    let w = width / 5;
    Rect::new((index as u32 * w) as i32, 0, w, 20)
}
fn action_at(at: (i32, i32), width: u32) -> Option<Action> {
    ACTIONS.into_iter().enumerate().find_map(|(i, a)| rect(i, width).contains(at).then_some(a))
}
fn choice<'a>(ui: &'a UiState, scene: &Scene, id: &str) -> Option<&'a mt_engine::play::Option> {
    let d = ui.decision.as_ref().filter(|d| d.kind == Kind::CombatTurn && d.context.who == Some(scene.squad.selected as usize))?;
    d.options.iter().find(|o| o.id == id)
}
impl Component for CombatBar {
    fn visible(&self, ui: &UiState) -> bool { ui.decision.as_ref().is_none_or(|d| matches!(d.kind, Kind::Ground | Kind::CombatTurn)) }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        if !active(scene) { return }
        out.indexed.fill(0);
        if choice(ui, scene, "first:party").is_some() {
            for (i, id) in ["first:party", "first:npcs"].into_iter().enumerate() {
                if let Some(o) = choice(ui, scene, id) { self.assets.text(out, &o.label, (i as i32 * out.w as i32 / 2 + 2, 6), if o.enabled { 15 } else { 8 }); }
            }
            return
        }
        for (i, a) in ACTIONS.into_iter().enumerate() {
            let r = rect(i, out.w as u32);
            for y in r.y..r.y + r.h as i32 { for x in r.x..r.x + r.w as i32 - 1 { out.put(x, y, 1); } }
            let rapid = (a == Action::Fire).then(|| choice(ui, scene, "fire:rapid")).flatten();
            self.assets.text(out, if rapid.is_some() { "Confirm" } else { a.label(scene) }, (r.x + 2, r.y + 6), if rapid.map_or_else(|| a.enabled(scene), |o| o.enabled) { 15 } else { 8 });
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, event: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !active(scene) { return Vec::new() }
        if let UiEvent::Key(k) = *event { return vec![Intent::Raw(Event::Key(k))] }
        if *event != UiEvent::Up(Button::Left) { return Vec::new() }
        let width = ui.component_input.size.0;
        if choice(ui, scene, "first:party").is_some() {
            if width == 0 || !Rect::new(0, 0, width, 20).contains(at) { return Vec::new() }
            let id = if at.0 < width as i32 / 2 { "first:party" } else { "first:npcs" };
            return choice(ui, scene, id).filter(|o| o.enabled).map_or_else(Vec::new, |o| crate::mouse::decision::raw(&o.input))
        }
        if action_at(at, width) == Some(Action::Fire) {
            if let Some(o) = choice(ui, scene, "fire:rapid") { return if o.enabled { crate::mouse::decision::raw(&o.input) } else { Vec::new() } }
        }
        action_at(at, ui.component_input.size.0).map_or_else(Vec::new, |a| a.intents(scene, at))
    }
}

pub fn track(id: &str, scene: &Scene, ui: &mut UiState, event: &UiEvent, at: (i32, i32), intents: &mut Vec<Intent>) {
    if !active(scene) { ui.components.combat = State::default(); return }
    if matches!(event, UiEvent::Down(_) | UiEvent::Key(_)) { ui.components.combat.pending = None; }
    if *event != UiEvent::Up(Button::Left) || intents.is_empty() { return }
    if intents.iter().any(|i| matches!(i, Intent::Raw(_))) { return }
    if id != ID && choice(ui, scene, "fire:rapid").is_some() { ui.components.combat.pending = None; return }
    if id != ID { return }
    let requested = action_at(at, ui.component_input.size.0).and_then(|a| a.menu(scene));
    let Some((op, target)) = requested else { ui.components.combat.pending = None; return };
    ui.components.combat.pending = Some(Pending { op, target, selected: scene.squad.selected, round: scene.squad.round, level: scene.level, encounter: scene.squad.encounter });
    *intents = vec![Intent::Hover(Some(target)), Intent::OpenMenu { target, at }];
}

pub fn observe(scene: &Scene, ui: &mut UiState) -> Vec<Intent> {
    if ui.components.combat.armed.is_some_and(|p| !current(p, scene)) { ui.components.combat.armed = None; }
    let Some(p) = ui.components.combat.pending else { return Vec::new() };
    if !current(p, scene) {
        ui.components.combat.pending = None; return Vec::new()
    }
    let Some(menu) = &scene.squad.menu else { return Vec::new() };
    ui.components.combat.pending = None;
    if menu.target != p.target { return Vec::new() }
    let pick = menu.items.iter().position(|i| i.op == p.op).and_then(|n| u8::try_from(n).ok());
    if p.op == Op::SetTarget && pick.is_some_and(|n| menu.items[n as usize].grey.is_none()) { ui.components.combat.armed = Some(p); }
    vec![pick.map_or(Intent::CloseMenu, Intent::MenuPick)]
}
fn current(p: Pending, scene: &Scene) -> bool {
    active(scene) && scene.squad.selected == p.selected && scene.squad.round == p.round && scene.level == p.level && scene.squad.encounter == p.encounter
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    use crate::panels::{components, controls, Host, Layout};
    use mt1::{ground::{self, orders::{Menu, MenuItem}, round, Ground}, ui::Verb};
    use mt_engine::io::Input;

    fn scene() -> Scene {
        let mut s = Scene::program(ScreenKind::Ground);
        s.combat = true; s.squad.active = true; s.squad.encounter = true; s.squad.round = 1; s.squad.to_act = Some(0);
        s.party.0 = vec![0; 0x1000]; s.party.0[0] = 31;
        s
    }
    fn requested(s: &Scene, ui: &mut UiState, op: Op, t: Target) {
        ui.components.combat.pending = Some(Pending { op, target: t, selected: s.squad.selected, round: s.squad.round, level: s.level, encounter: s.squad.encounter });
    }
    fn menu(s: &mut Scene, t: Target, ops: &[Op]) {
        s.squad.menu = Some(Menu { target: t, default: 0, items: ops.iter().map(|&op| MenuItem { op, label: format!("{op:?}"), key: b'X', grey: None }).collect() });
    }
    #[test]
    fn native_menu_order_refusal_and_missing_action_are_authoritative() {
        let mut s = scene(); let mut ui = UiState::default(); let t = Target::Unit(Unit::Npc(14));
        requested(&s, &mut ui, Op::SetTarget, t);
        assert!(observe(&s, &mut ui).is_empty());
        menu(&mut s, t, &[Op::Fire, Op::Reload, Op::SetTarget]);
        assert_eq!(observe(&s, &mut ui), [Intent::MenuPick(2)]);
        assert!(observe(&s, &mut ui).is_empty(), "pick once");
        requested(&s, &mut ui, Op::Fire, t);
        s.squad.menu.as_mut().unwrap().items[0].grey = Some("chamber empty".into());
        assert_eq!(observe(&s, &mut ui), [Intent::MenuPick(0)], "the sim explains a refused action");
        requested(&s, &mut ui, Op::Party, t);
        assert_eq!(observe(&s, &mut ui), [Intent::CloseMenu]);
        requested(&s, &mut ui, Op::Fire, t);
        s.squad.menu.as_mut().unwrap().target = Target::Portrait(1);
        assert!(observe(&s, &mut ui).is_empty(), "never pick in a different menu");
    }
    #[test]
    fn one_left_click_on_a_thug_fires_through_the_sim_verb_without_a_two_click_menu() {
        let mut s = scene(); s.walkers[0].0[5] = 14;
        for (id, t) in [("Viewport", 14), ("Initiative order", 14), ("Viewport", 15)] {
            let mut ui = UiState::default(); let fire = Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(t)) };
            let mut out = vec![fire];
            track(id, &s, &mut ui, &UiEvent::Up(Button::Left), (20, 20), &mut out);
            assert_eq!(out, [fire], "{id}: the Fire verb reaches the sim untouched"); assert!(ui.components.combat.pending.is_none());
            assert!(observe(&s, &mut ui).is_empty());
        }
    }
    #[test]
    fn changed_context_or_new_input_cancels_pending_action() {
        let s = scene(); let t = Target::Unit(Unit::Npc(14));
        for change in 0..6 {
            let mut ui = UiState::default(); requested(&s, &mut ui, Op::Fire, t);
            let mut next = s.clone(); menu(&mut next, t, &[Op::Fire]);
            match change { 0 => next.screen = ScreenKind::Record, 1 => next.squad.active = false, 2 => next.squad.selected = 1,
                3 => next.squad.round += 1, 4 => next.level += 1, _ => next.squad.encounter = false }
            assert!(observe(&next, &mut ui).is_empty()); assert!(ui.components.combat.pending.is_none());
        }
        for event in [UiEvent::Down(Button::Left), UiEvent::Key(27)] {
            let mut ui = UiState::default(); requested(&s, &mut ui, Op::Fire, t);
            track(ID, &s, &mut ui, &event, (0, 0), &mut Vec::new());
            assert!(ui.components.combat.pending.is_none());
        }
    }
    #[test]
    fn buttons_hit_only_their_cells_and_keep_shortcuts_hidden() {
        let c = CombatBar::new(Arc::default()); let mut s = scene(); s.walkers[0].0[5] = 14;
        let mut ui = UiState::default();
        for width in [192, 320, 641] {
            ui.component_input.size = (width, 44);
            for (i, a) in ACTIONS.into_iter().enumerate() {
                let r = rect(i, width); let at = (r.x + 3, r.y + 7);
                assert_eq!(action_at(at, width), Some(a));
                assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), at), a.intents(&s, at));
                for event in [UiEvent::Down(Button::Left), UiEvent::Up(Button::Right), UiEvent::Move, UiEvent::DoubleClick(Button::Left)] {
                    assert!(c.input(&s, &ui, &event, at).is_empty());
                }
            }
            for at in [(-1, 0), (0, -1), (width as i32, 0), (0, 44), (1, 20)] { assert!(action_at(at, width).is_none()); }
            assert!(ACTIONS.iter().enumerate().all(|(i, _)| rect(i, width).y + rect(i, width).h as i32 <= 22), "every button inside the 22 px bar at width {width}");
            c.draw(&s, &ui, &mut Surface::new(width, 44));
        }
        assert!(!Action::Party.enabled(&s)); s.squad.encounter = false; assert!(Action::Party.enabled(&s));
        s.combat = false; assert_eq!(Action::Party.label(&s), "Break"); assert!(!Action::Reload.enabled(&s));
        s.encounter_mode = 1; assert!(!Action::Party.enabled(&s));
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(b'R' as u16), (0, 0)), [Intent::Raw(Event::Key(b'R' as u16))]);
        s.squad.active = false; assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (1, 1)).is_empty());
        c.draw(&s, &ui, &mut Surface::new(0, 0));
    }

    #[test]
    fn synthetic_squad_decision_and_pointer_sequence_share_native_intents() {
        use mt_engine::play::{Choice, Decision, Kind, NativeIntent, Option as ChoiceOption, Play};
        let mut s = scene();
        let map = Arc::make_mut(&mut s.map); map.rows = 12; map.cols = 20; map.cells = vec![0; 12 * 20 * 3];
        s.walkers[2].0[0] = 1; s.walkers[2].0[8..10].copy_from_slice(&8i16.to_le_bytes()); s.walkers[2].0[10..12].copy_from_slice(&10i16.to_le_bytes());
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        layout.set_host("Viewport", Host::Dock(Rect::new(0, 0, 192, 112)));
        let mut ui = UiState { camera: (4.5, 5.5), ..UiState::default() };
        let mut d = Decision::new(Kind::CombatTurn, "t", "combat.turn");
        let routes = [("select:2", Intent::Select(2)), ("move:cell/5,6", Intent::Verb { verb: Verb::Move, target: Target::Cell((5, 6)) }),
            ("menu:1", Intent::MenuPick(1)), ("menu:0", Intent::MenuPick(0))];
        for &(id, intent) in &routes { let mut o = ChoiceOption::new(id, id, vec![]); o.intent = Some(NativeIntent::new(intent)); d.options.push(o); }
        let mut actual = Vec::new();
        for at in [(96, 56), (112, 72)] {
            controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Down(Button::Left), at);
            actual.extend(controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), at));
        }
        let t = Target::Unit(Unit::Npc(14));
        for op in [Op::SetTarget, Op::Fire] {
            let mut out = vec![Intent::Verb { verb: Verb::Fire, target: t }];
            track("Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), (128, 56), &mut out);
            assert_eq!(out, [Intent::Verb { verb: Verb::Fire, target: t }], "one click fires through the sim verb"); assert!(ui.components.combat.pending.is_none());
            requested(&s, &mut ui, op, t);
            menu(&mut s, t, &[Op::Fire, Op::SetTarget]); actual.extend(observe(&s, &mut ui)); s.walkers[0].0[5] = 14;
        }
        assert_eq!(actual.len(), routes.len());
        let mut play = Play::new();
        for ((id, _), actual) in routes.into_iter().zip(actual) {
            play.decide(&d); let decision = play.pending().unwrap().id.clone(); play.choose(Choice::new(decision, id)).unwrap();
            let native = *play.intent().unwrap().downcast::<Intent>().unwrap(); assert_eq!(native, actual);
        }
    }

    #[test]
    fn initiative_choice_and_rapid_confirmation_use_registered_events_with_releases() {
        use mt_engine::play::{Decision, Option as ChoiceOption};
        let s = scene(); let mut ui = UiState::default(); ui.component_input.size = (320, 44);
        let mut d = Decision::new(Kind::CombatTurn, "t", "combat.turn"); d.context.who = Some(0);
        d.options = vec![ChoiceOption::new("first:party", "Party first", vec![Event::Key(0x139)]), ChoiceOption::new("first:npcs", "NPCs first", vec![Event::Key(0x131)])];
        ui.decision = Some(d); let c = CombatBar::new(Arc::default());
        for (at, key) in [((5, 8), 0x139), ((200, 8), 0x131)] {
            let mut out = c.input(&s, &ui, &UiEvent::Up(Button::Left), at);
            track(ID, &s, &mut ui, &UiEvent::Up(Button::Left), at, &mut out);
            assert_eq!(out, [Intent::Raw(Event::Key(key)), Intent::Raw(Event::KeyUp(key))]);
        }
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (0, 44)).is_empty());
        ui.decision.as_mut().unwrap().options[1].enabled = false;
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (200, 8)).is_empty());
        c.draw(&s, &ui, &mut Surface::new(320, 44));
        ui.decision.as_mut().unwrap().options = vec![ChoiceOption::new("fire:rapid", "Fire selected targets", vec![Event::Key(0x11C), Event::KeyUp(0x11C)])];
        let add = Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }; let mut targets = vec![add];
        track("Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), (100, 50), &mut targets);
        assert_eq!(targets, [add], "the native rapid-fire target picker keeps NPC clicks");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (5, 8)), [Intent::Raw(Event::Key(0x11C)), Intent::Raw(Event::KeyUp(0x11C))]);
        c.draw(&s, &ui, &mut Surface::new(320, 44));
        ui.decision.as_mut().unwrap().options[0].enabled = false;
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (5, 8)).is_empty());
        ui.decision.as_mut().unwrap().context.who = Some(1); assert!(choice(&ui, &s, "fire:rapid").is_none());
        ui.decision.as_mut().unwrap().kind = Kind::Menu; assert!(!c.visible(&ui));
    }

    struct MouseOnly;
    impl Input for MouseOnly {
        fn next_key(&mut self) -> u16 { panic!("mouse combat reached a keyboard wait") }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { clock.wait_tick(); Event::Idle }
    }
    fn game() -> Option<Ground> {
        let Some(dir) = oracle_env::gamefiles() else { return None };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml");
        let mut g = crate::scenario::load(&path, std::path::Path::new(&dir)).unwrap().game;
        crate::scenario::attach_scene(&mut g); g.input = Box::new(MouseOnly); round::pass(&mut g, &[]); Some(g)
    }
    fn snapshot(g: &Ground) -> Scene { Scene::capture(g, ScreenKind::Ground, None) }
    fn drain(g: &mut Ground, ui: &mut UiState, intents: Vec<Intent>) {
        assert!(intents.iter().all(|i| !matches!(i, Intent::Raw(_))), "zero native key/mouse events in combat sequence: {intents:?}");
        for &i in &intents { ui.apply(i); }
        round::pass(g, &intents);
        for _ in 0..4 {
            let scene = snapshot(g);
            if scene.squad.menu.is_none() { ui.menu = None; }
            let followup = observe(&scene, ui);
            if followup.is_empty() { return }
            round::pass(g, &followup);
        }
        panic!("menu transaction did not settle")
    }
    fn click(g: &mut Ground, layout: &mut Layout, ui: &mut UiState, id: &'static str, at: (i32, i32)) {
        let scene = snapshot(g); layout.arrange_for(&scene, ui, (640, 480));
        let Host::Dock(r) = layout.host(id).unwrap() else { panic!("expected dock") };
        let window = (r.x + at.0, r.y + at.1);
        for event in [UiEvent::Move, UiEvent::Down(Button::Left), UiEvent::Up(Button::Left)] {
            let scene = snapshot(g);
            let (hit, local) = layout.hit_mapped(&scene, None, window, |r| r).unwrap(); assert_eq!(hit, id);
            let intents = controls::dispatch(layout, hit, &scene, ui, &event, local);
            drain(g, ui, intents);
        }
    }
    fn cell_click(g: &mut Ground, layout: &mut Layout, ui: &mut UiState, cell: (i16, i16)) {
        let scene = snapshot(g); layout.arrange_for(&scene, ui, (640, 480));
        ui.camera = (cell.0 as f32 + 0.5, cell.1 as f32 + 0.5); ui.follow = None;
        let camera = crate::panels::viewport::Camera::new(&scene, ui, layout.host("Viewport").unwrap().size());
        let at = (((cell.1 as f32 * 16.0 + 8.0 - camera.left) * camera.zoom) as i32,
            ((cell.0 as f32 * 16.0 + 8.0 - camera.top) * camera.zoom) as i32);
        click(g, layout, ui, "Viewport", at);
    }
    struct KeyScript { events: std::collections::VecDeque<Event>, mp: std::rc::Rc<std::cell::RefCell<Vec<i32>>>, idle: usize }
    #[derive(Debug)]
    struct KeyStop;
    impl Input for KeyScript {
        fn next_key(&mut self) -> u16 { mt_engine::io::keys::ESC }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { match self.events.pop_front() { Some(Event::Idle) => { clock.wait_tick(); Event::Idle } Some(e) => e, None => { self.idle += 1; if self.idle > 40 { std::panic::panic_any(KeyStop) } clock.wait_tick(); Event::Idle } } }
        fn scene(&mut self, s: &dyn std::any::Any) { if let Some(s) = s.downcast_ref::<Scene>() { self.mp.borrow_mut().push(s.squad.mp[s.squad.selected.min(4) as usize]) } }
    }
    #[test]
    fn key_step_publishes_the_charged_mp() {
        use mt_engine::io::keys;
        let Some(mut g) = game() else { return };
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default()); let mut ui = UiState::default();
        let from = round::traveller_cell(&g, 0); cell_click(&mut g, &mut layout, &mut ui, from); assert_eq!(round::selected(&g), 0);
        let dest = (-1..=1).flat_map(|r| (-1..=1).map(move |c| (from.0 + r, from.1 + c))).filter(|&c| c != from).find(|&c| {
            ground::orders::menu_for(&g, Target::Cell(c)).items.iter().any(|i| matches!(i.op, Op::Move(_)) && i.grey.is_none())
        }).unwrap_or_else(|| panic!("no nearby move from {from:?}"));
        let key = match (dest.0 - from.0, dest.1 - from.1) { (-1, 0) => keys::UP, (1, 0) => keys::DOWN, (0, -1) => keys::LEFT, (0, 1) => keys::RIGHT, (-1, -1) => keys::HOME, (-1, 1) => keys::PGUP, (1, -1) => keys::END, (1, 1) => keys::PGDN, d => panic!("{d:?}") };
        let before = snapshot(&g).squad.mp[0]; let seen = std::rc::Rc::new(std::cell::RefCell::new(vec![]));
        g.input = Box::new(KeyScript { events: [Event::Idle, Event::Key(key), Event::KeyUp(key), Event::Idle, Event::Idle, Event::Key(key), Event::KeyUp(key), Event::Idle, Event::Idle].into(), mp: seen.clone(), idle: 0 });
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ground::main_loop::run(&mut g))) { assert!(e.is::<KeyStop>(), "unexpected panic"); }
        assert_ne!(round::traveller_cell(&g, 0), from, "the key step walked");
        let charged = snapshot(&g).squad.mp[0]; assert!(charged < before, "MP charged: {before} -> {charged}");
        assert_eq!(seen.borrow().last().copied(), Some(charged), "the last published readout carries the charged MP {charged}: {:?}", seen.borrow());
    }
    #[test]
    fn efate_broken_pointer_only_select_move_target_fire_end_turn() {
        let Some(mut g) = game() else { return };
        let started = std::time::Instant::now(); let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        let mut ui = UiState::default();
        click(&mut g, &mut layout, &mut ui, "Party strip", (60, 20)); assert_eq!(round::selected(&g), 1);
        let from = round::traveller_cell(&g, 0); cell_click(&mut g, &mut layout, &mut ui, from); assert_eq!(round::selected(&g), 0);
        let before_mp = snapshot(&g).squad.mp[0];
        let dest = (-1..=1).flat_map(|r| (-1..=1).map(move |c| (from.0 + r, from.1 + c))).filter(|&c| c != from).find(|&c| {
            ground::orders::menu_for(&g, Target::Cell(c)).items.iter().any(|i| matches!(i.op, Op::Move(_)) && i.grey.is_none())
        }).unwrap_or_else(|| panic!("no nearby move from {from:?}, turn {:?}, menu {:?}", snapshot(&g).squad.to_act, ground::orders::menu_for(&g, Target::Cell((from.0 - 1, from.1)))));
        cell_click(&mut g, &mut layout, &mut ui, dest);
        assert_eq!(round::traveller_cell(&g, 0), dest); assert!(snapshot(&g).squad.mp[0] < before_mp);
        let foe = round::npc_cell(&g, 14); let ammo = snapshot(&g).party.hand(0, 0).unwrap().2; let pips = snapshot(&g).squad.pips[0];
        let seq = snapshot(&g).squad.fx.seq;
        cell_click(&mut g, &mut layout, &mut ui, foe);
        assert_eq!(snapshot(&g).walkers[0].target(), 14); assert_ne!(snapshot(&g).squad.pips[0], pips, "the shot spends Attack");
        assert!(snapshot(&g).party.hand(0, 0).unwrap().2 < ammo); assert!(snapshot(&g).squad.fx.seq > seq);
        assert!(g.squad.as_ref().unwrap().p1.log.iter().any(|s| s.contains("fires")));
        let who = round::selected(&g) as usize; let logged = snapshot(&g).squad.log_n;
        let r = rect(4, 640); click(&mut g, &mut layout, &mut ui, ID, (r.x + 8, r.y + 8));
        assert!(g.squad.as_ref().unwrap().p1.turns[who].done); assert!(snapshot(&g).squad.log_n > logged);
        assert!(!g.input.closed()); assert!(started.elapsed().as_secs() < 60);
    }
    fn button_case(action:Option<Action>,standing:bool) {
        let Some(mut g)=game() else {return};ground::orders::select(&mut g,0);
        if standing {
            ground::orders::set_target(&mut g,14);
            let task=ground::orders::fire_check(&g,0,14).unwrap().task;
            let seed=(1..20000u32).find(|seed| {g.rand.lcg.seed=*seed;let dice=ground::squad::two_d(&mut g);!ground::rules::resolve(&task,dice).hit}).unwrap();g.rand.lcg.seed=seed;
        } else {ground::standing::hold_fire(&mut g);}
        let ammo=g.mem.party_u8(0xA84);let s=snapshot(&g);let mut ui=UiState::default();ui.component_input.size=(320,44);
        let intents=if let Some(action)=action {let index=ACTIONS.iter().position(|a|*a==action).unwrap();let r=rect(index,320);CombatBar::new(Arc::default()).input(&s,&ui,&UiEvent::Up(Button::Left),(r.x+5,r.y+8))}
            else {super::super::party_strip::PartyStrip::new(Arc::default()).input(&s,&ui,&UiEvent::Up(Button::Left),(220,10))};
        assert!(!intents.is_empty());for i in intents {ground::orders::intent(&mut g,i);}
        assert_eq!(g.mem.party_u8(0xA84),ammo-u8::from(standing));
        assert_eq!(g.squad.as_ref().unwrap().p1.turns[0].done,action!=Some(Action::Orders));
    }
    #[test] fn combat_bar_end_turn_fires_and_ends(){button_case(Some(Action::EndTurn),true);button_case(Some(Action::EndTurn),false);}
    #[test] fn party_strip_end_turn_fires_and_ends(){button_case(None,true);button_case(None,false);}
    #[test] fn orders_button_fires_and_keeps_move(){button_case(Some(Action::Orders),true);button_case(Some(Action::Orders),false);}

    #[test]
    fn every_button_matches_its_order_operation() {
        for (action, scan) in [(Action::Fire, 0x21), (Action::Reload, 0x13), (Action::Orders, round::KEY_O), (Action::EndTurn, round::KEY_SPACE)] {
            let (Some(mut a), Some(mut b)) = (game(), game()) else { return };
            for g in [&mut a, &mut b] { ground::orders::set_target(g, 14); if action == Action::Reload { g.mem.set_party_u8(0xA82 + 2, 0); } }
            let s = snapshot(&a); let mut ui = UiState::default(); ui.component_input.size = (320, 44);
            let index = ACTIONS.iter().position(|&a| a == action).unwrap(); let r = rect(index, 320); let at = (r.x + 5, r.y + 8);
            let mut intents = CombatBar::new(Arc::default()).input(&s, &ui, &UiEvent::Up(Button::Left), at);
            track(ID, &s, &mut ui, &UiEvent::Up(Button::Left), at, &mut intents); drain(&mut a, &mut ui, intents);
            if action == Action::EndTurn { ground::orders::intent(&mut b, Intent::EndTurn); } else { assert!(round::key(&mut b, scan)); }
            round::pass(&mut b, &[]);
            assert_eq!(a.mem.party().iter().zip(b.mem.party()).enumerate().filter(|(_, (a, b))| a != b).map(|(i, (a,b))| (i,*a,*b)).collect::<Vec<_>>(), [], "{action:?}: party and ammunition");
            assert_eq!(a.rand.lcg.seed, b.rand.lcg.seed, "{action:?}: dice");
            assert_eq!(a.squad.as_ref().unwrap().p1.turns, b.squad.as_ref().unwrap().p1.turns, "{action:?}: turn costs");
            assert_eq!(a.squad.as_ref().unwrap().p1.log, b.squad.as_ref().unwrap().p1.log, "{action:?}: outcomes");
        }
        for broken in [false, true] {
            let Some(mut g) = game() else { return };
            g.squad.as_mut().unwrap().encounter = false; g.squad.as_mut().unwrap().p1.encounter = None;
            g.mem.set_combat(i16::from(broken)); g.mem.set_key(0);
            let s = snapshot(&g); let mut ui = UiState::default(); ui.component_input.size = (320, 44);
            let at = (rect(3, 320).x + 4, 8); let mut intents = Action::Party.intents(&s, at);
            track(ID, &s, &mut ui, &UiEvent::Up(Button::Left), at, &mut intents);
            for i in intents { ground::orders::intent(&mut g, i); }
            for i in observe(&snapshot(&g), &mut ui) { ground::orders::intent(&mut g, i); }
            assert_eq!(g.mem.key(), 0x19, "P scan, broken={broken}");
            assert!(g.squad.as_ref().unwrap().orders.menu.is_none());
        }
    }
}
