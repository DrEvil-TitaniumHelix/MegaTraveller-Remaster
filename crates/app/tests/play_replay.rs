#[path = "../../mt1/tests/oracle_env/mod.rs"]
mod oracle_env;
use megatraveller::{play_store, scenario};
use mt1::{ground::{self, input::menu, main_loop, render, state::Mem, Ground}, play, save::{self, Saved}};
use mt_engine::{io::{Dir, Event, Files, Input, Scripted}, play::{Choice, Kind, Pick, Play, Root, RootKind, SessionExit, Snapshot}, sim::tick::{Clock, Counter}};
use std::{cell::RefCell, collections::VecDeque, panic::{catch_unwind, AssertUnwindSafe}, path::{Path, PathBuf}, rc::Rc};

fn files() -> Option<PathBuf> { oracle_env::gamefiles() }
fn quick(dir: &Path) -> Ground {
    let files = Dir(dir.into());
    let mut g = Ground::new(&files.read("T.DAT").unwrap(), files.read("DEFAULT.SAV").unwrap(), Box::new(Scripted::new([])), Box::new(Counter::at(1234)), Box::new(files)).unwrap();
    g.mouse_driver = true; ground::start(&mut g);
    g.mem.set_party_u8(0xC1, g.mem.party_u8(0xC1) & 127); g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
    g.scenario_boot = true; assert!(render::game_init(&mut g)); g.scenario_boot = false; g
}
fn root(scenario: Option<&str>) -> Root {
    Root { kind: RootKind::NewGame, image: None, start_tick: 1234, scenario: scenario.map(String::from), gamefiles: "test BYO assets".into() }
}
fn boot(dir: &Path, root: &Root) -> Ground {
    if root.kind == RootKind::Anchor {
        let saved = Saved::decode(root.image.as_deref().unwrap()).unwrap();
        return save::ground_exact(saved.clone(), Box::new(Scripted::new([])), Box::new(Counter::at(saved.ticks)), Box::new(Dir(dir.into()))).unwrap()
    }
    if let Some(name) = &root.scenario {
        let mut g = scenario::load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios").join(format!("{name}.toml")), dir).unwrap().game;
        scenario::attach_scene(&mut g); g.clock = Box::new(Counter::at(1234)); return g
    }
    quick(dir)
}
#[derive(Debug)] struct Stop;
fn stopped(f: impl FnOnce()) { let e = catch_unwind(AssertUnwindSafe(f)).expect_err("session must stop at a decision"); if !e.is::<Stop>() { std::panic::resume_unwind(e); } }

struct Native { events: VecDeque<Event> }
impl Native { fn new(events: impl IntoIterator<Item = Event>) -> Self { Self { events: events.into_iter().collect() } } }
impl Input for Native {
    fn next_key(&mut self) -> u16 { loop { if let Some(Event::Key(k)) = self.events.pop_front() { return k } assert!(!self.events.is_empty(), "native input exhausted"); } }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        let e = self.events.pop_front().expect("native input exhausted"); if e == Event::Idle { clock.wait_tick(); } e
    }
}
fn keys(keys: &[u16]) -> Vec<Event> { keys.iter().flat_map(|&k| [Event::Key(k), Event::KeyUp(k)]).collect() }

#[test]
fn af79_every_game_option_and_two_levels_commit_like_native_input() {
    let Some(dir) = files() else { return };
    let original = menu::Menu::read(&quick(&dir).mem, Mem::ds(0x45FE)).unwrap();
    for i in 0..original.labels.len() {
        let mut a = quick(&dir); let mut b = quick(&dir);
        for g in [&mut a, &mut b] { for j in 0..6 { g.mem.write_bytes(Mem::ds(0x45FE + 39 + j * 4), &[0; 4]); g.mem.set_u16(0x45FE + 63 + j * 2, 0); } }
        a.input = Box::new(Native::new(keys(&[0x13B + i as u16])));
        b.input = Box::new(Play::scripted([Choice::new("d:1", format!("menu:af79.45FE/{i}").as_str())]));
        assert_eq!(menu::run_menu(&mut a, Mem::ds(0x45FE), 0, 0), menu::run_menu(&mut b, Mem::ds(0x45FE), 0, 0));
        assert_eq!(play::hash(&mut a), play::hash(&mut b), "Game option {i}");
    }
    for i in 0..2 {
        let mut a = quick(&dir); let mut b = quick(&dir);
        for g in [&mut a, &mut b] {
            let bytes = g.mem.ds[0x45FE..0x45FE + 75].to_vec(); g.mem.write_bytes(Mem::ds(0x4000), &bytes);
            g.mem.set_u8(0x4000 + 32, 3);
            for j in 0..6 { g.mem.write_bytes(Mem::ds(0x4000 + 39 + j * 4), &[0; 4]); g.mem.set_u16(0x4000 + 63 + j * 2, 0); }
            g.mem.set_pointer(0x45FE + 39, Mem::ds(0x4000));
        }
        a.input = Box::new(Native::new(keys(&[0x13B, 0x13B + i])));
        b.input = Box::new(Play::scripted([Choice::new("d:1", "menu:af79.45FE/0"), Choice::new("d:2", format!("menu:af79.4000/{i}").as_str())]));
        assert_eq!(menu::run_menu(&mut a, Mem::ds(0x45FE), 0, 0), menu::run_menu(&mut b, Mem::ds(0x45FE), 0, 0));
        assert_eq!(play::hash(&mut a), play::hash(&mut b), "nested option {i}");
    }
}

#[test]
fn de5e_extended_rows_use_native_arrows_and_refuse_invalid_ids() {
    let Some(dir) = files() else { return };
    let rects: Vec<_> = (0..12).map(|i| mt_engine::io::mouse::Hotspot { top: 160, left: 4 + i * 20, height: 16, width: 18, id: i as u8 + 1 }).collect();
    for (selected, target) in [(3, 11), (11, 10)] {
        let mut a = quick(&dir); let mut b = quick(&dir);
        let mut raw = vec![0x14D; (target + 12 - selected) % 12]; raw.push(0x11C);
        a.input = Box::new(Native::new(keys(&raw)));
        b.input = Box::new(Play::with_service(move |p| {
            let d = p.pending_public().unwrap();
            assert!(p.choose(Choice::new(&d.id, "menu:de5e/12")).is_err());
            assert_eq!(p.pending_public().unwrap().state, d.state); assert!(p.journal().entries.is_empty());
            p.choose(Choice::new(d.id, format!("menu:de5e/{target}").as_str())).unwrap();
        }));
        assert_eq!(menu::select(&mut a, &rects, selected as i16, 6, 7, None), target as i16);
        assert_eq!(menu::select(&mut b, &rects, selected as i16, 6, 7, None), target as i16);
        assert_eq!(play::hash(&mut a), play::hash(&mut b));
    }
}

#[test]
fn ground_move_grant_matches_native_make_eleven_idles_and_break() {
    let Some(dir) = files() else { return };
    let mut a = quick(&dir); let mut b = quick(&dir);
    let mut native = vec![Event::Key(0x14D)]; native.extend([Event::Idle; 11]); native.push(Event::KeyUp(0x14D));
    native.extend([Event::Idle; 128]);
    a.input = Box::new(Native::new(native));
    b.input = Box::new(Play::scripted([Choice::new("d:1", "move:east:11")]));
    play::commands::wait(&mut b);
    for _ in 0..13 {
        a.poll(); b.poll();
        assert_eq!(play::hash(&mut a), play::hash(&mut b));
        for g in [&mut a, &mut b] {
            if g.mem.key() == 0x4D { let (dx, dy, d) = g.mem.direction(6); ground::walk::move_party(g, dx, dy, d); }
        }
        assert_eq!(play::hash(&mut a), play::hash(&mut b));
        if a.mem.u8(0x6FEA) == 0xCD { break }
    }
    assert_eq!(a.mem.u8(0x6FEA), 0xCD, "the native break was consumed");
}

#[test]
fn blocking_native_menu_interrupts_a_ground_grant_and_reports_remainder() {
    let Some(dir) = files() else { return };
    let mut g = quick(&dir); let result = Rc::new(RefCell::new(None)); let out = result.clone();
    g.input = Box::new(Play::with_service(move |p| {
        let d = p.pending_public().unwrap();
        let pick = if d.kind == Kind::Ground { "move:east:11" } else {
            *out.borrow_mut() = Some(p.journal().entries[0].grant.clone()); "back"
        };
        p.choose(Choice::new(d.id, pick)).unwrap();
    }));
    play::commands::wait(&mut g); for _ in 0..4 { g.poll(); }
    assert_eq!(menu::run_menu(&mut g, Mem::ds(0x45FE), 0, 0), -1);
    let grant = result.borrow(); let grant = grant.as_ref().unwrap();
    assert_eq!((grant.requested, grant.used, grant.remaining), (11, 3, 8));
}

#[test]
fn squad_choices_match_component_intents_and_npc_turns_offer_nothing() {
    use mt1::{ground::{round, squad}, ui::{Intent, Target, Unit, Verb}};
    let Some(dir) = files() else { return };
    let root = root(Some("efate_broken")); let mut a = boot(&dir, &root); let mut b = boot(&dir, &root);
    let seed = a.rand.lcg.seed;
    b.input = Box::new(Play::scripted([Choice::new("d:1", "fire:npc/14"), Choice::new("d:2", "end_turn")]));
    for raw in [Intent::Verb { verb: Verb::Fire, target: Target::Unit(Unit::Npc(14)) }, Intent::EndTurn] {
        play::turns::wait(&mut b); let intents = squad::poll(&mut b);
        assert_eq!(intents, vec![raw]); round::pass(&mut a, &[raw]); round::pass(&mut b, &intents);
        assert_eq!(play::hash(&mut a), play::hash(&mut b));
    }
    assert_ne!(seed, a.rand.lcg.seed, "combat consumed the shared RNG");
    let s = b.squad.as_mut().unwrap(); let e = s.p1.encounter.as_mut().unwrap(); e.side = round::Side::Npcs; e.choosing = false; e.interrupt = None;
    assert!(play::turns::decision(&b).is_none());
    b.squad.as_mut().unwrap().p1.encounter.as_mut().unwrap().side = round::Side::Party;
    let who = round::selected(&b) as usize; b.squad.as_mut().unwrap().p1.turns[who].done = true;
    let d = play::turns::decision(&b).unwrap(); assert_eq!(d.options.len(), 1); assert_eq!(d.options[0].id, "end_turn");
}

#[test]
fn rapid_fire_with_one_target_confirms_through_the_native_round_key() {
    use mt1::{ground::{orders, round}, ui::{Target, Unit}};
    let Some(dir) = files() else { return };
    let root = root(Some("efate_broken")); let mut a = boot(&dir, &root); let mut b = boot(&dir, &root);
    for g in [&mut a, &mut b] {
        g.mem.set_party_u8(0xA82, 12); g.mem.set_party_u8(0xA82 + 2, 30);
        assert!(orders::fire_check_opts(g, 0, 14, mt1::ground::fire::Opts { rapid: true, ..Default::default() }).is_ok());
        orders::perform(g, orders::Op::Rapid, Target::Unit(Unit::Npc(14)));
        assert_eq!(g.squad.as_ref().unwrap().fire.rapid.as_deref(), Some(&[14][..]));
    }
    let seed = a.rand.lcg.seed;
    a.input = Box::new(Native::new(keys(&[0x11C]).into_iter().chain(std::iter::repeat_n(Event::Idle, 1000))));
    b.input = Box::new(Play::scripted([Choice::new("d:1", "fire:rapid")]));
    play::turns::wait(&mut b);
    for g in [&mut a, &mut b] { g.poll(); let key = g.mem.key(); assert!(round::key(g, key)); g.mem.set_key(0); }
    assert_eq!(play::hash(&mut a), play::hash(&mut b));
    assert!(b.squad.as_ref().unwrap().fire.rapid.is_none()); assert_ne!(seed, b.rand.lcg.seed);
}

#[test]
fn bank_list_number_journal_replays() {
    let Some(dir) = files() else { return };
    struct BankInput { prefix: VecDeque<Event>, play: Play }
    impl Input for BankInput {
        fn data_mode(&self) -> bool { self.prefix.is_empty() && self.play.data_mode() }
        fn next_key(&mut self) -> u16 { if self.prefix.is_empty() { return self.play.next_key() } loop { if let Some(Event::Key(k)) = self.prefix.pop_front() { return k } } }
        fn poll(&mut self, clock: &mut dyn Clock) -> Event { self.prefix.pop_front().unwrap_or_else(|| self.play.poll(clock)) }
        fn wants_decision(&self) -> bool { self.prefix.is_empty() && self.play.wants_decision() }
        fn decide(&mut self, d: &mt_engine::play::Decision) { self.play.decide(d); }
    }
    fn run(dir: &Path, restore: Option<Snapshot>) -> (Snapshot, String) {
        let mut g = quick(dir); let base = root(None);
        let out = Rc::new(RefCell::new(None)); let result = out.clone();
        let mut at = 0;
        let replay = restore.is_some();
        let mut input = Play::with_service(move |p| {
            let d = p.pending_public().unwrap();
            if replay || at == 3 { *result.borrow_mut() = Some(p.snapshot(&base, "bank", "bank", "now", "test").unwrap()); std::panic::resume_unwind(Box::new(Stop)); }
            let pick = match at {
                0 | 1 => { assert_eq!(d.kind, Kind::List); let row = d.list.as_ref().unwrap().rows.iter().find(|r| r.enabled).unwrap(); Pick::Row { row: row.id.clone(), qty: row.qty } },
                2 => { assert_eq!(d.kind, Kind::Number); Pick::Number(1) }, _ => unreachable!(),
            };
            at += 1; p.choose(Choice::new(d.id, pick)).unwrap();
        });
        if let Some(snapshot) = restore { input.restore(snapshot).unwrap(); }
        g.input = Box::new(BankInput { prefix: [Event::Key(0x150), Event::KeyUp(0x150), Event::Key(0x11C)].into(), play: input });
        stopped(|| ground::starport::enter(&mut g));
        let snapshot = out.borrow_mut().take().unwrap(); (snapshot, play::hash(&mut g))
    }
    let (snapshot, state) = run(&dir, None); let (_, replayed) = run(&dir, Some(snapshot)); assert_eq!(state, replayed);
}

#[test]
fn starport_event_tile_cuts_grant_at_p4b_blocking_decision() {
    let Some(dir) = files() else { return };
    let mut g = boot(&dir, &root(Some("m5_starport_desk_efate")));
    let out = Rc::new(RefCell::new(None)); let result = out.clone();
    g.input = Box::new(Play::with_service(move |p| {
        let d = p.pending_public().unwrap();
        if d.id == "d:1" { assert_eq!(d.kind, Kind::Ground); p.choose(Choice::new(d.id, "move:west:100")).unwrap(); }
        else { assert!(!d.kind.is_command()); *result.borrow_mut() = Some(p.journal().entries[0].grant.clone()); std::panic::resume_unwind(Box::new(Stop)); }
    }));
    stopped(|| main_loop::run(&mut g));
    assert_eq!(g.mem.event(), 10, "stopped inside the starport event at cell (33,3)");
    let result = out.borrow(); let grant = result.as_ref().unwrap();
    assert!(grant.remaining > 0); assert_eq!(grant.requested, 100); assert_eq!(grant.used + grant.remaining, 100);
}

fn record(dir: &Path, origin: &Root, picks: &[&str], restore: Option<Snapshot>) -> (Snapshot, String) {
    let effective = restore.as_ref().map_or(origin, |s| &s.root).clone();
    let mut g = boot(dir, &effective);
    let output = Rc::new(RefCell::new(None)); let out = output.clone(); let base = effective.clone();
    let mut picks: VecDeque<String> = picks.iter().map(|p| p.to_string()).collect();
    let mut answerer = Play::with_service(move |p| {
        let d = p.pending_public().unwrap();
        if let Some(pick) = picks.pop_front() {
            assert!(d.kind != Kind::CombatTurn || d.context.who.is_some());
            p.choose(Choice::new(d.id, Pick::Id(pick))).unwrap();
        } else {
            *out.borrow_mut() = Some(p.snapshot(&base, "replay", "Replay test", "2026-10-28", "test").unwrap());
            std::panic::resume_unwind(Box::new(Stop));
        }
    });
    if let Some(s) = restore { answerer.restore(s).unwrap(); }
    if effective.kind==RootKind::Anchor {
        let saved=Saved::decode(effective.image.as_deref().unwrap()).unwrap();
        let checkpoint=&saved.play.as_ref().unwrap().input;
        answerer.resume_play_checkpoint(checkpoint).unwrap();
    }
    g.input = Box::new(answerer);
    stopped(|| if effective.kind == RootKind::Anchor { main_loop::resume_exact(&mut g).unwrap() } else { main_loop::run(&mut g) });
    let snapshot = output.borrow_mut().take().unwrap(); (snapshot, play::hash(&mut g))
}

#[test]
fn ordinary_scripted_restore_ignores_anchor_modes_and_devices() {
    let Some(dir) = files() else { return };
    let mut source = quick(&dir); source.loop_vars.initialized = true; source.mem.set_key(0); source.mem.set_event(-1);
    source.squad = Some(ground::squad::State::default()); source.mouse_driver = true; source.bios_key = 0x147;
    source.balance = ground::rules::balance::Options { grenade_range: false, npc_selective_autofire: false };
    let saved = play::anchors::capture(&mut source, 4, 1234).unwrap();
    let saved = Saved::decode(&saved.encode()).unwrap();
    let mut ordinary = save::ground(saved.clone(), Box::new(Scripted::new([])), Box::new(Counter::at(saved.ticks)), Box::new(Dir(dir.clone())));
    assert!(!ordinary.input.wants_decision());
    assert!(ordinary.squad.is_none() && ordinary.scene.is_none(), "ordinary/faithful boot must ignore SQD");
    ordinary.mouse_driver = false; ordinary.bios_key = 0x155;
    let host_balance = ground::rules::balance::Options { grenade_range: false, npc_selective_autofire: true };
    ordinary.balance = host_balance;
    assert!(save::resume(&mut ordinary));
    assert!(ordinary.squad.is_none() && ordinary.scene.is_none(), "pending resume must also ignore SQD");
    assert_eq!((ordinary.mouse_driver, ordinary.bios_key), (false, 0x155));
    assert_eq!(ordinary.balance, host_balance);
    let mut host_squad = ground::squad::State::default(); host_squad.p1.done.push("host mode".into());
    ordinary.squad = Some(host_squad.clone()); ordinary.scene = Some(mt1::ui::SceneHook::default());
    saved.clone().install(&mut ordinary);
    assert_eq!(ordinary.balance, host_balance);
    assert_eq!(ordinary.squad.as_ref(), Some(&host_squad)); assert!(ordinary.scene.is_some());
    assert_eq!((ordinary.mouse_driver, ordinary.bios_key), (false, 0x155));
    let exact = save::ground_exact(saved.clone(), Box::new(Play::new()), Box::new(Counter::at(saved.ticks)), Box::new(Dir(dir))).unwrap();
    assert!(exact.squad.is_some() && exact.scene.is_some()); assert_eq!((exact.mouse_driver, exact.bios_key), (true, 0x147));
    assert_eq!(exact.balance, source.balance);
}

#[test]
fn valid_checksum_mouse_driver_and_balance_tamper_is_a_replay_mismatch() {
    let Some(dir) = files() else { return };
    let origin = root(None); let (snapshot, _) = record(&dir, &origin, &[], None);
    assert_eq!(snapshot.root.kind, RootKind::Anchor);
    for field in 0..3 {
        let mut tampered = snapshot.clone();
        let mut saved = Saved::decode(tampered.root.image.as_deref().unwrap()).unwrap();
        let meta = saved.play.as_mut().unwrap();
        match field {
            0 => meta.mouse_driver = !meta.mouse_driver,
            1 => meta.balance.grenade_range = !meta.balance.grenade_range,
            _ => meta.balance.npc_selective_autofire = !meta.balance.npc_selective_autofire,
        }
        tampered.root.image = Some(saved.encode());
        let result = catch_unwind(AssertUnwindSafe(|| record(&dir, &origin, &[], Some(tampered)))).unwrap_err();
        assert!(matches!(result.downcast_ref::<SessionExit>(), Some(SessionExit::ReplayMismatch(_))), "covered field {field}");
    }
}

#[test]
fn ground_and_combat_journals_replay_and_mid_encounter_restore_matches_tail() {
    let Some(dir) = files() else { return };
    let ground = root(None);
    let (snapshot, state) = record(&dir, &ground, &["wait:2", "cmd:game", "back", "move:east:11", "wait:3"], None);
    let (_, replayed) = record(&dir, &ground, &[], Some(snapshot.clone())); assert_eq!(state, replayed);
    let combat = root(Some("efate_broken"));
    let picks = ["fire:npc/14", "end_turn", "end_turn", "end_turn"];
    let (_, straight) = record(&dir, &combat, &picks, None);
    let (mid, _) = record(&dir, &combat, &picks[..2], None);
    assert_eq!(mid.root.kind, RootKind::Anchor, "complete combat image");
    let (tail, restored) = record(&dir, &combat, &picks[2..], Some(mid)); assert_eq!(straight, restored);
    let (_, replayed) = record(&dir, &combat, &[], Some(tail)); assert_eq!(straight, replayed);
    let tmp = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/m8p-core/snapshot-test").join(format!("{}-{}", std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
    let mut disk = snapshot.clone(); disk.id = format!("replay-{}-{}", std::process::id(), std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos());
    play_store::save(&tmp, &disk).unwrap(); let read = play_store::load(&tmp, &disk.id).unwrap();
    let (_, persisted) = record(&dir, &ground, &[], Some(read.clone())); assert_eq!(state, persisted);
    assert!(play_store::list(&tmp).unwrap().iter().any(|s| s.id == disk.id));
    assert!(play_store::load(&tmp, "../escape").is_err());
    assert_eq!(read.root.kind, RootKind::Anchor);
    let mut fault = read;
    let mut saved = Saved::decode(fault.root.image.as_deref().unwrap()).unwrap(); saved.seed ^= 1; fault.root.image = Some(saved.encode());
    let result = catch_unwind(AssertUnwindSafe(|| record(&dir, &ground, &[], Some(fault)))).unwrap_err();
    assert!(matches!(result.downcast_ref::<SessionExit>(), Some(SessionExit::ReplayMismatch(_))));
}

#[test]
fn zz_skip_report() { oracle_env::report() }
