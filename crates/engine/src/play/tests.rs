use super::*;
use crate::{io::{Input, Scripted}, sim::tick::{Clock, Counter}};

fn menu() -> Decision {
    let mut d = Decision::new(Kind::Menu, "test", "select"); d.state = "s:fixture".into();
    d.options = vec![Option::new("first", "First", vec![Event::Key(13)]), Option::new("back", "Back", vec![Event::Key(27)])];
    d.options[1].enabled = false; d
}
fn command() -> Decision {
    let mut d = Decision::new(Kind::Ground, "test", "command");
    d.time = Some(TimeInput { max_ticks: 100, directions: [("east".into(), 0x14D)].into() }); d
}
fn pick(play: &mut Play, value: impl Into<Pick>) -> Result<(), String> { play.choose(Choice::new(play.pending().unwrap().id.clone(), value)) }
fn drain(play: &mut Play, count: usize, clock: &mut Counter) -> Vec<Event> { (0..count).map(|_| play.poll(clock)).collect() }

#[test]
fn fnv_known_vectors_and_framing() {
    assert_eq!(fnv(b""), 0xcbf29ce484222325); assert_eq!(fnv(b"a"), 0xaf63dc4c8601ec8c);
    let (mut a, mut b) = (Fnv::default(), Fnv::default());
    a.field(b"ab"); a.field(b"c"); b.field(b"a"); b.field(b"bc"); assert_ne!(a.finish(), b.finish());
}
#[test]
fn default_input_methods_do_nothing() {
    let mut input = Scripted::new([13]); assert!(!input.wants_decision());
    input.decide(&menu()); input.anchor("unused", &[0]); assert_eq!(input.next_key(), 13);
}
#[test]
fn refusals_are_transactional_and_ids_are_never_reused() {
    let mut play = Play::new(); play.decide(&menu()); let first = play.pending().unwrap().id.clone();
    for invalid in ["missing", "back", "wait:5"] { assert!(pick(&mut play, invalid).is_err()); assert_eq!(play.pending().unwrap().id, first); assert!(play.journal().entries.is_empty()); }
    assert!(play.choose(Choice::new("d:0", "first")).unwrap_err().contains("stale"));
    pick(&mut play, "first").unwrap(); assert!(play.pending().is_none());
    let mut clock = Counter::at(42); assert_eq!(drain(&mut play, 2, &mut clock), [Event::Key(13), Event::KeyUp(13)]); assert_eq!(clock.now, 42);
    play.decide(&menu()); assert_ne!(play.pending().unwrap().id, first);
    assert!(play.choose(Choice::new(first, "first")).unwrap_err().contains("stale"));
    assert_eq!(play.journal().entries[0].after.as_ref(), Some(&play.pending().unwrap().state));
}
#[test]
fn fake_primitive_uses_the_same_native_commit_with_no_ticks() {
    fn primitive(input: &mut dyn Input, clock: &mut Counter) -> i32 {
        if input.wants_decision() { input.decide(&menu()); }
        loop { if let Event::Key(key) = input.poll(clock) { return if key == 13 { 7 } else { -1 } } }
    }
    let (mut a, mut b) = (Counter::at(19), Counter::at(19));
    let mut play = Play::scripted([Choice::new("d:1", "first")]);
    assert_eq!(primitive(&mut Scripted::new([13]), &mut a), primitive(&mut play, &mut b)); assert_eq!(a, b);
    assert_eq!(play.poll(&mut b), Event::KeyUp(13)); assert_eq!(b.now, 19);
}
#[test]
fn nested_stack_and_outer_native_sequence() {
    let mut play = Play::new(); play.decide(&menu()); let outer = play.pending().unwrap().id.clone();
    assert!(!play.wants_decision()); play.decide(&menu()); assert_eq!(play.stack().len(), 2); assert_eq!(play.pending().unwrap().context.depth, 1);
    assert!(play.choose(Choice::new(outer.clone(), "first")).is_err()); pick(&mut play, "first").unwrap();
    assert_eq!(play.pending().unwrap().id, outer);
    assert!(pick(&mut play, "first").is_err());
    let mut clock = Counter::default(); drain(&mut play, 2, &mut clock); pick(&mut play, "first").unwrap();
    assert!(!play.wants_decision()); assert_eq!(play.next_key(), 13);
    assert!(play.wants_decision()); play.decide(&menu()); assert_eq!(play.stack().len(), 1);
    assert_eq!(play.poll(&mut clock), Event::KeyUp(13)); assert_eq!(clock.now, 0);
}
#[test]
fn wait_five_and_move_make_hold_break() {
    let mut play = Play::new(); let mut clock = Counter::at(u32::MAX - 2); play.decide(&command());
    for bad in ["wait:0", "wait:-1", "wait:+1", "wait:101", "wait:4294967296", "move:west:2", "wait:2:3"] { assert!(pick(&mut play, bad).is_err(), "{bad}"); }
    pick(&mut play, "wait:5").unwrap();
    assert_eq!(drain(&mut play, 5, &mut clock), vec![Event::Idle; 5]); assert_eq!(clock.now, 2); assert!(play.grant().is_none());
    assert_eq!(play.journal().entries[0].grant, Grant { requested: 5, used: 5, remaining: 0 });
    play.decide(&command()); pick(&mut play, "move:east:3").unwrap();
    assert_eq!(play.poll(&mut clock), Event::Key(0x14D)); assert_eq!(clock.now, 2);
    assert_eq!(drain(&mut play, 3, &mut clock), vec![Event::Idle; 3]);
    assert_eq!(play.poll(&mut clock), Event::KeyUp(0x14D)); assert_eq!(clock.now, 5);
}
#[test]
fn blocking_wait_cuts_grant_and_reports_unused_ticks() {
    let mut play = Play::new(); let mut clock = Counter::default(); play.decide(&command()); pick(&mut play, "move:east:10").unwrap();
    drain(&mut play, 3, &mut clock); assert_eq!(clock.now, 2);
    assert!(play.wants_decision()); assert!(!play.register(&command())); assert_eq!(play.sequence(), 1);
    assert!(play.register(&menu())); assert_eq!(play.sequence(), 2); assert!(play.grant().is_none());
    assert_eq!(play.journal().entries[0].grant, Grant { requested: 10, used: 2, remaining: 8 });
    assert_eq!(play.poll(&mut clock), Event::KeyUp(0x14D));
    pick(&mut play, "first").unwrap(); assert_eq!(play.poll(&mut clock), Event::Key(13)); assert_eq!(clock.now, 2);
}
#[test]
fn text_and_number_validate_before_native_playback() {
    let mut d = Decision::new(Kind::Text, "test", "text");
    d.input = Some(TextInput { max_len: 4, width: None, charset: Charset::Upper, submit: vec![Event::Key(13)] });
    let mut play = Play::new(); play.decide(&d);
    for s in ["ABCD", "a", "é", "A\n", " "] { assert!(pick(&mut play, Pick::Text(s.into())).is_err()); }
    pick(&mut play, Pick::Text("A1".into())).unwrap();
    assert_eq!(play.next_key(), b'A' as u16); assert_eq!(play.next_key(), b'1' as u16); assert_eq!(play.next_key(), 13);
    d.kind = Kind::Number; d.input.as_mut().unwrap().charset = Charset::Digits; play.decide(&d);
    assert!(pick(&mut play, Pick::Number(-1)).is_err()); assert!(pick(&mut play, Pick::Number(1000)).is_err());
    pick(&mut play, Pick::Number(0)).unwrap(); assert_eq!(play.next_key(), 48); assert_eq!(play.next_key(), 13);
}
#[test]
fn list_limits_money_and_native_quantity_sequence() {
    let mut d = Decision::new(Kind::List, "test", "shop");
    d.list = Some(List { rows: vec![Row { id: "row:3".into(), label: "row".into(), enabled: true, price: 10, limit: 4, qty: 0, min_qty: 0, input: vec![Event::Key(67)] }],
        current: 0, more: false, money: 25, increase: vec![Event::Key(43)], decrease: vec![Event::Key(45)], submit: vec![Event::Key(13)] });
    let mut play = Play::new(); play.decide(&d);
    for (row, qty) in [("absent", 1), ("row:3", 5), ("row:3", 3)] { assert!(pick(&mut play, Pick::Row { row: row.into(), qty }).is_err()); }
    pick(&mut play, Pick::Row { row: "row:3".into(), qty: 2 }).unwrap();
    assert_eq!((0..4).map(|_| play.next_key()).collect::<Vec<_>>(), [67, 43, 43, 13]);
}
#[test]
fn native_intents_and_observers_never_advance_time() {
    let mut play = Play::with_service(|p| pick(p, "act").unwrap());
    let mut d = Decision::new(Kind::CombatTurn, "test", "turn"); let mut option = Option::new("act", "Act", vec![]);
    option.intent = Some(NativeIntent::new(17usize)); d.options.push(option); play.decide(&d);
    assert_eq!(*play.intent().unwrap().downcast::<usize>().unwrap(), 17); assert!(play.intent().is_none());
    play.copy_scene::<Vec<u8>>(); play.scene(&vec![1u8, 2]); assert_eq!(play.scene_ref::<Vec<u8>>().unwrap(), &[1, 2]);
    play.present(&[1, 2, 3]); assert_eq!(play.frame(), &[1, 2, 3]);
    play.anchor("first", &[1]); play.anchor("second", &[2]); assert_eq!(play.latest_anchor().unwrap().image, [2]);
    assert_eq!(play.latest_anchor().unwrap().sequence, 1); assert_eq!(play.latest_anchor().unwrap().journal_at, 1);
}
#[test]
fn native_time_only_polls_and_explicit_breaks() {
    let mut play = Play::new(); let mut clock = Counter::default(); assert!(!play.key_pending());
    assert_eq!(play.poll(&mut clock), Event::Idle); assert_eq!(clock.now(), 1);
    let mut d = menu(); d.options[0].input.push(Event::KeyUp(13)); play.decide(&d); pick(&mut play, "first").unwrap();
    assert_eq!(drain(&mut play, 2, &mut clock), [Event::Key(13), Event::KeyUp(13)]); assert!(play.wants_decision());
    play.decide(&d); assert_eq!(play.sequence(), 2);
    assert!(play.start_sequence(10).is_err());
}
#[test]
fn replay_hashes_include_stack_and_sequence_and_unwind_is_distinct() {
    fn trace() -> Journal { let mut p = Play::new(); p.decide(&menu()); pick(&mut p, "first").unwrap(); p.next_key(); p.decide(&menu()); p.journal().clone() }
    assert_eq!(trace(), trace());
    let mut shifted = Play::new(); shifted.start_sequence(4).unwrap(); shifted.decide(&menu()); assert_eq!(shifted.pending().unwrap().id, "d:5");
    assert_ne!(trace().entries[0].before, shifted.pending().unwrap().state);
    let mut play = Play::new(); play.decide(&menu());
    let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| play.next_key())).unwrap_err();
    assert_eq!(*exit.downcast::<SessionExit>().unwrap(), SessionExit::AwaitingDecision);
    let root = Root { kind: RootKind::NewGame, image: None, start_tick: 7, scenario: None, gamefiles: "digest".into() };
    let expected = SessionExit::NewGame(root);
    let exit = std::panic::catch_unwind(|| expected.clone().leave()).unwrap_err(); assert_eq!(*exit.downcast::<SessionExit>().unwrap(), expected);
}

#[test]
fn data_mode_is_distinct_from_decision_observation() {
    struct Observer;
    impl Input for Observer { fn next_key(&mut self)->Key {13} fn wants_decision(&self)->bool {true} }
    assert!(Play::new().data_mode());assert!(!Observer.data_mode());assert!(Observer.wants_decision());
}
#[test]
fn proportional_width_and_minimum_quantity_refuse_before_playback() {
    let mut text=Decision::new(Kind::Text,"test","text");
    text.input=Some(TextInput {max_len:10,charset:Charset::Ascii("AB".into()),width:Some((5,vec![3;128])),submit:vec![Event::Key(13)]});
    let mut play=Play::new();play.decide(&text);let before=play.pending().unwrap().state.clone();
    assert!(pick(&mut play,Pick::Text("AB".into())).is_err());assert_eq!(play.pending().unwrap().state,before);
    pick(&mut play,Pick::Text("A".into())).unwrap();assert_eq!(play.next_key(),65);assert_eq!(play.next_key(),13);
    text.input.as_mut().unwrap().width=Some((100,vec![]));play.decide(&text);
    assert!(pick(&mut play,Pick::Text("A".into())).is_err());
    let mut list=Decision::new(Kind::List,"test","list");
    list.list=Some(List {rows:vec![Row {id:"one".into(),label:"One".into(),enabled:true,price:0,limit:1,qty:1,min_qty:1,input:vec![]}],
        current:0,more:false,money:0,increase:vec![],decrease:vec![],submit:vec![Event::Key(13)]});
    let mut play=Play::new();play.decide(&list);assert!(pick(&mut play,Pick::Row {row:"one".into(),qty:0}).is_err());
    pick(&mut play,Pick::Row {row:"one".into(),qty:1}).unwrap();assert_eq!(play.next_key(),13);
}

#[test]
fn snapshot_replay_checks_every_hash_grant_and_version_and_retains_clean_anchor() {
    let root = Root { kind: RootKind::NewGame, image: None, start_tick: 19, scenario: None, gamefiles: "digest".into() };
    let mut p = Play::new(); let mut clock = Counter::at(19);
    p.anchor("clean", &[1, 2]); p.decide(&menu()); pick(&mut p, "first").unwrap();
    p.anchor("busy", &[3]); assert_eq!(p.latest_anchor().unwrap().name, "clean");
    drain(&mut p, 2, &mut clock); p.decide(&menu());
    let snapshot = p.snapshot(&root, "unit", "unit", "now", "test").unwrap();
    assert_eq!(snapshot.root_sequence().unwrap(), 0); assert_eq!(snapshot.root.image, Some(vec![1, 2]));
    let mut r = Play::new(); r.restore(snapshot.clone()).unwrap(); r.decide(&menu());
    assert_eq!(drain(&mut r, 2, &mut clock), [Event::Key(13), Event::KeyUp(13)]);
    r.decide(&menu()); assert!(!r.replaying()); assert_eq!(r.pending().unwrap().state, snapshot.hash);
    for version in [1, 2, HASH_VERSION + 1] {
        let mut unsupported = snapshot.clone(); unsupported.journal.hash_version = version;
        assert!(Play::new().restore(unsupported).is_err());
    }
    let mut broken = snapshot.clone(); broken.journal.entries[0].after = None; assert!(broken.validate().is_err());
    let mut wrong = snapshot.clone(); wrong.journal.entries[0].grant.used = 1;
    let mut r = Play::new(); r.restore(wrong).unwrap(); r.decide(&menu()); drain(&mut r, 2, &mut clock);
    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.decide(&menu()))).unwrap_err();
    assert!(matches!(e.downcast_ref::<SessionExit>(), Some(SessionExit::ReplayMismatch(_))));
    let mut r = Play::new(); r.restore(snapshot).unwrap();
    let mut changed = menu(); changed.state = "different native state".into();
    let e = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| r.decide(&changed))).unwrap_err();
    assert!(matches!(e.downcast_ref::<SessionExit>(), Some(SessionExit::ReplayMismatch(_))));
    let mut mouse = Play::new(); mouse.anchor("clean", &[1]); mouse.set_mouse_position(23, 45); mouse.anchor("mouse", &[2]);
    assert_eq!(mouse.latest_anchor().unwrap().name, "mouse");
    let state=mouse.play_checkpoint().unwrap();let mut restored=Play::new();restored.resume_play_checkpoint(&state).unwrap();
    assert_eq!(restored.mouse(),mouse.mouse(),"live pointer state is part of the image");
}

#[test]
fn exhausted_sequences_are_rejected_before_restore_or_start() {
    let s = Snapshot { id: "exhausted".into(), name: String::new(), created: String::new(), build: String::new(),
        root: Root { kind: RootKind::Anchor, image: Some(vec![0]), start_tick: 0, scenario: None, gamefiles: String::new() },
        journal: Journal::default(), sequence: u64::MAX, hash: "s:test".into() };
    assert!(s.validate().is_err()); assert!(Play::new().restore(s).is_err());
    for sequence in [u64::MAX - 1, u64::MAX] { assert!(Play::new().start_sequence(sequence).is_err()); }
}

#[test]
fn registration_overflow_is_session_control() {
    for sequence in [u64::MAX - 1, u64::MAX] {
        let mut p = Play::new(); p.sequence = sequence;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| p.register(&menu())));
        let exit = result.expect_err("exhausted sequence must exit the session");
        assert_eq!(exit.downcast_ref::<SessionExit>(), Some(&SessionExit::SequenceExhausted));
        assert_eq!(p.sequence(), sequence); assert!(p.pending().is_none());
    }
    let mut p = Play::new(); p.start_sequence(u64::MAX - 2).unwrap(); p.register(&menu());
    assert_eq!(p.pending().unwrap().id, format!("d:{}", u64::MAX - 1));
    pick(&mut p, "first").unwrap(); drain(&mut p, 2, &mut Counter::default());
    let exit = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| p.register(&menu()))).unwrap_err();
    assert_eq!(exit.downcast_ref::<SessionExit>(), Some(&SessionExit::SequenceExhausted));
}

#[test]
fn public_decision_preserves_metadata_and_excludes_every_native_route() {
    let mut d = menu(); d.id = "d:1".into(); d.prompt = Some("Prompt".into()); d.default = Some("first".into());
    d.options[0].intent = Some(NativeIntent::new(17usize));
    d.list = Some(List { rows: vec![Row { id: "row:7".into(), label: "Item".into(), enabled: true, price: 25, limit: 4, qty: 2, min_qty: 1,
        input: vec![Event::Key(54321)] }], current: 0, more: true, money: 100, increase: vec![Event::Key(54322)],
        decrease: vec![Event::Key(54323)], submit: vec![Event::Key(54324)] });
    d.input = Some(TextInput { max_len: 12, width: Some((20, vec![3; 128])), charset: Charset::Ascii("ABC".into()), submit: vec![Event::Key(54325)] });
    d.time = Some(TimeInput { max_ticks: 42, directions: [("east".into(), 54326), ("west".into(), 54327)].into() });
    let expected = PublicDecision { id: d.id.clone(), kind: d.kind, prompt: d.prompt.clone(), default: d.default.clone(), context: d.context.clone(), state: d.state.clone(), movement: None,
        options: vec![PublicOption { id: "first".into(), label: "First".into(), enabled: true }, PublicOption { id: "back".into(), label: "Back".into(), enabled: false }],
        list: Some(PublicList { rows: vec![PublicRow { id: "row:7".into(), label: "Item".into(), enabled: true, price: 25, limit: 4, qty: 2, min_qty: 1 }], current: 0, more: true, money: 100 }),
        input: Some(PublicTextInput { max_len: 12, charset: Charset::Ascii("ABC".into()), width: Some((20, vec![3; 128])) }),
        time: Some(PublicTimeInput { max_ticks: 42, directions: vec!["east".into(), "west".into()] }) };
    assert_eq!(d.public(), expected);
    let mut changed = d.clone();
    for o in &mut changed.options { o.input = vec![Event::Key(1)]; o.intent = Some(NativeIntent::new(999usize)); }
    let list = changed.list.as_mut().unwrap(); list.rows[0].input = vec![Event::Key(2)];
    list.increase = vec![Event::Key(3)]; list.decrease = vec![Event::Key(4)]; list.submit = vec![Event::Key(5)];
    changed.input.as_mut().unwrap().submit = vec![Event::Key(6)];
    for key in changed.time.as_mut().unwrap().directions.values_mut() { *key = 7; }
    assert_eq!(changed.public(), expected, "no native route may influence the public DTO");
    let mut p = Play::new(); assert!(p.pending_public().is_none()); p.register(&d);
    let mut registered = expected; registered.state = p.pending().unwrap().state.clone();
    assert_eq!(p.pending_public(), Some(registered));
    pick(&mut p, "first").unwrap(); assert_eq!(p.next_key(), 13);
    assert_eq!(*p.intent().unwrap().downcast::<usize>().unwrap(), 17, "stripping must not alter internal playback");
}
