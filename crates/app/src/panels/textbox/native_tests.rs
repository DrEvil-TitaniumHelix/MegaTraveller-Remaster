mod oracle_env { include!("../../../../mt1/tests/oracle_env/mod.rs"); }
use super::*;
use mt_engine::{data::{exepack, ly, pic, Variant}, io::{Dir, Files, Input}, play::{Choice, Pick, Play}, sim::tick::{Clock, Counter}};
use std::{collections::VecDeque, path::{Path, PathBuf}, rc::Rc};

fn files() -> Option<PathBuf> { oracle_env::gamefiles() }
fn click(layout: &Layout, ui: &mut UiState, scene: &Scene, id: &'static str, local: (i32, i32)) -> Vec<Intent> {
    let super::super::Host::Dock(r) = layout.host(id).unwrap() else { panic!("docked test") };
    let (hit, at) = layout.hit(scene, None, (r.x + local.0, r.y + local.1)).unwrap(); assert_eq!(hit, id);
    let mut result = vec![];
    for event in [UiEvent::Move, UiEvent::Down(Button::Left), UiEvent::Up(Button::Left)] {
        result.extend(super::super::controls::dispatch(layout, id, scene, ui, &event, at));
    }
    result
}
fn choose(d: &Decision, pick: Pick, preset: mt1::ui::Preset) -> Vec<Event> {
    let mut layout = Layout::default(); super::super::components::register(&mut layout, Arc::default());
    let scene = Scene::program(if d.context.program == "s" { mt1::ui::ScreenKind::Space } else { mt1::ui::ScreenKind::Picture });
    let mut ui = UiState { decision: Some(d.clone()), preset, ..UiState::default() };
    layout.arrange_for(&scene, &ui, (320, 200));
    let intents = match &pick {
        Pick::Id(id) => {
            let index = d.options.iter().position(|o| o.id == *id).unwrap();
            if d.kind == Kind::Ack {
                if index == 0 { click(&layout, &mut ui, &scene, "Continue", (100, 100)) } else { crate::mouse::decision::option(d, index) }
            } else if d.kind == Kind::Space {
                for _ in 0..index / 8 { assert!(click(&layout, &mut ui, &scene, "Flight", (240, 30)).is_empty()); }
                let slot = index % 8;
                click(&layout, &mut ui, &scene, "Flight", ((slot % 4) as i32 * 80 + 2, (slot / 4) as i32 * 12 + 2))
            } else {
                for _ in 0..index / 12 { assert!(click(&layout, &mut ui, &scene, "Choices", (200, 174)).is_empty()); }
                click(&layout, &mut ui, &scene, "Choices", (10, 26 + (index % 12) as i32 * 12))
            }
        }
        Pick::Text(value) => {
            let chars = alphabet(d.input.as_ref().unwrap()); let (cols, rows, width) = Textbox::grid((WIDTH, HEIGHT)); let count = cols * rows;
            let mut page = 0;
            click(&layout, &mut ui, &scene, ID, (8, 24));
            for byte in value.bytes() {
                let i = chars.iter().position(|&b| b == byte).unwrap(); let target = i / count;
                while page != target { click(&layout, &mut ui, &scene, ID, (90, 148)); page = (page + 1) % chars.len().div_ceil(count); }
                let slot = i % count;
                assert!(click(&layout, &mut ui, &scene, ID, ((slot % cols) as i32 * width as i32 + 1, 54 + (slot / cols) as i32 * ROW as i32)).is_empty());
            }
            click(&layout, &mut ui, &scene, ID, (180, 148))
        }
        _ => panic!("test pick"),
    };
    let events: Vec<_> = intents.into_iter().map(|i| if let Intent::Raw(e) = i { e } else { panic!("native route") }).collect();
    if d.options.iter().find(|o| matches!(&pick, Pick::Id(id) if o.id == *id)).is_some_and(|o| !o.enabled) {
        assert!(events.is_empty()); return events
    }
    let comparison = if d.kind == Kind::Number { if let Pick::Text(v) = &pick { Pick::Number(v.parse().unwrap()) } else { pick } } else { pick };
    let mut p = Play::new(); p.register(d); p.choose(Choice::new(p.pending().unwrap().id.clone(), comparison)).unwrap();
    let expected: Vec<_> = (0..events.len()).map(|_| p.poll(&mut Counter::at(0))).collect();
    assert_eq!(events, expected, "{}.{}", d.context.program, d.context.routine);
    assert!(!p.key_pending(), "click consumed the entire option, including navigation");
    events
}
type Plan = Rc<RefCell<VecDeque<(&'static str, &'static str, Pick)>>>;
struct Clicks { ui: UiState, store: decision::Store, plan: Plan }
impl Clicks {
    fn new(plan: Vec<(&'static str, &'static str, Pick)>) -> (Self, Plan) {
        let plan = Rc::new(RefCell::new(plan.into())); (Self { ui: UiState::default(), store: decision::Store::default(), plan: plan.clone() }, plan)
    }
}
impl Input for Clicks {
    fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
    fn decide(&mut self, d: &Decision) {
        let (program, routine, pick) = self.plan.borrow_mut().pop_front().expect("unexpected native wait");
        assert_eq!((&*d.context.program, &*d.context.routine), (program, routine));
        self.store.record(&mut self.ui, d);
        self.store.queue(&mut self.ui, &choose(d, pick, mt1::ui::Preset::Modern));
    }
    fn next_key(&mut self) -> u16 { self.store.next_key().expect("click route exhausted") }
    fn key_pending(&mut self) -> bool { self.store.key_pending() }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        if let Some(e) = self.store.pop() { e } else { clock.wait_tick(); Event::Idle }
    }
}

#[test]
fn boot_title_0297_real_start_and_quit_use_only_pointer_dispatch() {
    let Some(dir) = files() else { return }; let f = Dir(dir);
    let startup = exepack::unpack(&f.read("STARTUP.EXE").unwrap()).unwrap().exe;
    let font = ly::decode(&f.read("FONT.LY").unwrap()).unwrap();
    let layouts = ly::decode(&f.read("TITLE.LY").unwrap()).unwrap();
    for (id, expected) in [("start", mt1::screens::title::Exit::Game), ("quit", mt1::screens::title::Exit::Quit)] {
        let mut title = mt1::screens::title::Title::new(&startup, layouts.clone(), font.clone(),
            pic::decode(Variant::Mt1, &f.read("TIT.PIC").unwrap()).unwrap(), pic::decode(Variant::Mt1, &f.read("ZOD.PIC").unwrap()).unwrap()).unwrap();
        let (mut input, plan) = Clicks::new(vec![("title", "0297", Pick::Id(id.into()))]);
        assert_eq!(title.run(&mut || 12345, &mut Counter::at(0), &mut input, |_, _| {}), expected);
        assert!(plan.borrow().is_empty());
    }
}
#[ignore = "the action list is disabled; set choices::ENABLED to run"]
#[test]
fn boot_chargen_real_menus_yesno_name_and_ack_accept_clicks() {
    use mt1::ui::{CgUi, Menu};
    let Some(dir) = files() else { return }; let f = Dir(dir.clone());
    let (input, plan) = Clicks::new(vec![
        ("cg", "519A", Pick::Id("menu:519a.80:30/2".into())),
        ("cg", "56A6", Pick::Id("menu:56a6.80:30/1".into())),
        ("cg", "1AA2", Pick::Id("no".into())),
        ("cg", "5CC2", Pick::Text("ELLA".into())),
        ("cg", "7290", Pick::Id("continue".into())),
        ("cg", "519A", Pick::Id("back".into())),
    ]);
    let mut cg = mt1::screens::cg::Screens::new(exepack::unpack(&f.read("CG.DAT").unwrap()).unwrap().exe,
        ly::decode(&f.read("FONT.LY").unwrap()).unwrap(), ly::decode(&f.read("CG.LY").unwrap()).unwrap(),
        Box::new(input), Box::new(Counter::at(0)), Box::new(f)).unwrap();
    let menu = Menu { items: &["One", "Two", "Three"], highlight: 4, row: 80, col: 30, allow_unique: 1, border: 2, color: 0, extra: &[1, 0, 3], initial: -1 };
    assert_eq!(cg.menu(&menu), 2); assert_eq!(cg.hmenu(&menu), 1); assert!(!cg.yes_no(&["Yes", "No"]));
    assert_eq!(cg.name(15).as_deref(), Some("ELLA")); assert_eq!(cg.read_key(), 32); assert_eq!(cg.menu(&menu), -1);
    assert!(plan.borrow().is_empty());
}
fn ground(dir: &Path) -> mt1::ground::Ground {
    crate::scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml"), dir).unwrap().game
}
#[test]
fn g6_casino_draws_during_spin_and_queues_escape_until_result() {
    struct Keys { polls: usize, frames: Rc<RefCell<usize>> }
    impl Input for Keys {
        fn next_key(&mut self)->u16 { panic!("casino uses native scan polls") }
        fn present(&mut self,_:&[u32]) { *self.frames.borrow_mut()+=1; }
        fn poll(&mut self, clock:&mut dyn Clock)->Event {
            self.polls+=1; clock.wait_tick();
            assert!(self.polls<65,"Escape during the spin was lost");
            if self.polls==10 { assert!(*self.frames.borrow()>5,"slots were not presented during the opening spin"); }
            match self.polls {1=>Event::Key(32),2=>Event::KeyUp(32),30=>Event::Key(27),31=>Event::KeyUp(27),_=>Event::Idle}
        }
    }
    let Some(dir)=files() else {return};
    let mut g=crate::scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_casino_rich.toml"),&dir).unwrap().game;
    let frames=Rc::new(RefCell::new(0));
    g.input=Box::new(Keys {polls:0,frames:frames.clone()});g.clock=Box::new(Counter::at(0));
    g.scene=Some(mt1::ui::SceneHook::default());g.mem.set_u16(0x7000,0);
    mt1::ground::starport::casino::run(&mut g);
    assert_eq!(g.mem.i16(0x6e0a),0);assert!(*frames.borrow()>5);
}
#[test]
fn starport_desk_real_number_primitive_and_scan_wait_accept_click_routes() {
    let Some(dir) = files() else { return }; let mut g = ground(&dir);
    let (input, plan) = Clicks::new(vec![("t", "15B69", Pick::Text("123".into())), ("t", "D2C3", Pick::Id("back".into()))]);
    g.input = Box::new(input);
    assert_eq!(mt1::ground::starport::input_field(&mut g, 8, false), Some(b"123".to_vec()));
    assert_eq!(mt1::ground::input::wait_scan(&mut g), 1); assert!(plan.borrow().is_empty());
}
struct Capture(Rc<RefCell<Vec<Decision>>>);
impl Input for Capture {
    fn wants_decision(&self) -> bool { true }
    fn decide(&mut self, d: &Decision) { self.0.borrow_mut().push(d.clone()); }
    fn next_key(&mut self) -> u16 { panic!("capture only") }
}
#[test]
fn g6_station_choices_name_stations_and_current_travellers() {
    use mt1::space::{host::{StationHost, TOP_FRAME}, stations::Host, state};
    let Some(dir) = files() else { return }; let f = Dir(dir.clone());
    let mut sp = mt1::space::Space::new(&f.read("S.DAT").unwrap(), f.read("DEFAULT.SAV").unwrap(), vec![0;0x2000],
        Box::new(mt_engine::io::Scripted::new([])), Box::new(Counter::at(0)), Box::new(f)).unwrap();
    sp.product = true; sp.s.set_pbyte(0xa0, 1); sp.s.set_pbyte(0xa1, 2); sp.s.set_pbyte(0xa6, 0);
    let decisions = Rc::new(RefCell::new(vec![])); sp.input = Box::new(Capture(decisions.clone()));
    let mut host = StationHost::new(&mut sp, vec![0;65536], TOP_FRAME);
    for hotkeys in [0x19ba, 0x19a8, 0x19b1] {
        host.decision_wait("1C1C", Some((8,0)), &[0x355,state::DS as i32,8,0,12,8,hotkeys,state::DS as i32,1]);
    }
    for d in decisions.borrow().iter() {
        assert!(d.options[0].label.contains("Pilot") && d.options[0].label.contains("Magellan"), "{:?}", d.options);
        assert!(d.options[1].label.contains("Gunner"));
        assert!(d.options.iter().all(|o| !o.label.starts_with("Choice ")));
    }
    assert_eq!(decisions.borrow().len(),3);
}
fn all_options(decisions: &[Decision]) {
    for d in decisions { for preset in [mt1::ui::Preset::Modern, mt1::ui::Preset::Classic] {
        for option in &d.options { choose(d, Pick::Id(option.id.clone()), preset); }
    } }
}
#[ignore = "the action list is disabled; set choices::ENABLED to run"]
#[test]
fn starport_character_bridge_casino_trade_shipyard_all_options_match_native() {
    let Some(dir) = files() else { return }; let mut g = ground(&dir);
    let captured = Rc::new(RefCell::new(vec![])); g.input = Box::new(Capture(captured.clone()));
    mt1::play_prompts::character(&mut g, 0);
    mt1::play_prompts::bridge(&mut g, 0xff, 1); mt1::play_prompts::bridge(&mut g, 0, 1);
    for routine in ["1AC9C", "supplies.trade-in", "shipyard.pay", "16374", "16A2F"] {
        mt1::play_prompts::named(&mut g, routine, &[("pay", "Pay", b'P' as u16), ("back", "Back", keys::ESC)], &[]);
    }
    all_options(&captured.borrow());
}
#[ignore = "the action list is disabled; set choices::ENABLED to run"]
#[test]
fn efate_orbit_real_flight_stations_selectors_and_quit_have_native_buttons() {
    use mt1::space::{self, state::*};
    let Some(dir) = files() else { return }; let f = Dir(dir);
    let captured = Rc::new(RefCell::new(vec![]));
    let mut sp = space::Space::new(&f.read("S.DAT").unwrap(), f.read("DEFAULT.SAV").unwrap(), f.read("W.DAT").unwrap(),
        Box::new(Capture(captured.clone())), Box::new(Counter::at(0)), Box::new(f)).unwrap();
    sp.s.set_byte(KEY, 0); sp.s.set_byte(PENDING_KEY, 0); sp.s.set_word(ENCOUNTER, -1);
    space::play::flight(&mut sp);
    for station in 10..=17 { sp.s.set_word(STATION, station); space::play::station(&mut sp, "crew"); }
    space::play::selector(&mut sp, "selector", 13, 3, None); space::play::confirm(&mut sp); space::play::death(&mut sp);
    assert_eq!(captured.borrow()[0].kind, Kind::Space); all_options(&captured.borrow());
}
