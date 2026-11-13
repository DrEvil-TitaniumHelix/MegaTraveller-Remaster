use mt1::ground::{
    state::{Far, Mem},
    walk, Ground,
};
use mt_engine::{
    data::{bin::Wr, container},
    io::{
        dump::{Step, Text},
        Files, Input, Overlay, Scripted,
    },
    sim::tick::Counter,
};
use std::{
    process::{Command, Stdio},
    time::{Duration, Instant},
};

fn map(rows: u8, cols: u8) -> Vec<u8> {
    let mut bytes = vec![0; 26];
    bytes[0] = rows;
    bytes[1] = cols;
    bytes[2..4].copy_from_slice(b"G1");
    let mut left = rows as usize * cols as usize * 240;
    while left != 0 {
        let n = left.min(32768);
        bytes.i16(-(n as i32) as i16);
        bytes.bytes(&[1, 1]);
        left -= n;
    }
    bytes
}
fn area(maps: &[Vec<u8>], npcs: u8, spawns: u8) -> Vec<u8> {
    let mut bytes = vec![0; 0x274];
    bytes[0x1c..0x20].fill(10);
    let mut off = bytes.len();
    for (i, map) in maps.iter().enumerate() {
        if i < 5 {
            bytes[0x268 + i * 2..0x26a + i * 2].copy_from_slice(&(off as u16).to_le_bytes());
        }
        bytes.extend_from_slice(map);
        off += map.len();
    }
    bytes[0x272..0x274].copy_from_slice(&(off as u16).to_le_bytes());
    bytes.push(npcs);
    bytes.resize(bytes.len() + npcs as usize * 32, 0);
    bytes.push(spawns);
    bytes.resize(bytes.len() + spawns as usize * 6, 0);
    bytes
}
fn ground(world: Option<Vec<u8>>) -> Ground {
    let mut files = Overlay::new(None);
    if let Some(bytes) = world {
        files.write("W.DAT", &bytes).unwrap();
    }
    let mut g = Ground::from_ds(
        vec![0; 65536],
        vec![0; Mem::PARTY_SIZE],
        Box::new(Scripted::new([])),
        Box::new(Counter::default()),
        Box::new(files),
    );
    g.mem.set_level(-1);
    g
}
fn file(area: Vec<u8>) -> Vec<u8> {
    container::encode(&[area]).unwrap()
}
fn assert_init_rejected(bytes: Vec<u8>) {
    let mut g = ground(Some(bytes));
    g.mem.set_leader(3);
    g.mem.set_x(44);
    g.mem.set_y(12);
    let (ds, segs, seed) = (g.mem.ds.clone(), g.mem.segs.clone(), g.rand.lcg.seed);
    assert!(!walk::init_world(&mut g, 0, 0));
    assert_eq!(g.mem.ds, ds, "failed init changed DS");
    assert_eq!(g.mem.segs, segs, "failed init changed heap/party");
    assert_eq!(g.rand.lcg.seed, seed);
    assert!(g.rand.trace.is_empty());
}

#[test]
fn world_expansion_is_bounded_before_decode() {
    assert_init_rejected(file(area(&[map(255, 255)], 0, 0)));
    for (rows, cols) in [(0, 1), (1, 0), (0, 0)] {
        assert_init_rejected(file(area(&[map(rows, cols)], 0, 0)));
    }
    assert_init_rejected(file(area(&vec![map(9, 10); 130], 0, 0)));
    let bytes = container::encode(&[area(&[map(1, 1)], 0, 0), area(&[map(255, 255)], 0, 0)]).unwrap();
    assert_init_rejected(bytes);
    let mut overrun = map(1, 1);
    overrun[26..28].copy_from_slice(&(-241i16).to_le_bytes());
    assert_init_rejected(file(area(&[overrun], 0, 0)));
}

#[test]
fn area_tables_stop_before_ds_globals() {
    for (npcs, spawns) in [(41, 0), (255, 0), (0, 31), (0, 255), (41, 31)] {
        assert_init_rejected(file(area(&[map(1, 1)], npcs, spawns)));
    }
    let world=file(area(&[map(1, 1)], 40, 30));
    let area_offset=u32::from_le_bytes(world[4..8].try_into().unwrap());
    let mut g = ground(Some(world));
    g.mem.set_leader(3);
    g.mem.set_u32(0x6b52, 0xaabbccdd);
    assert!(walk::init_world(&mut g, 0, 0));
    assert_eq!((g.mem.npc_count(), g.mem.u8(0x6b69)), (40, 30));
    assert_eq!(g.mem.leader(), 3);
    assert_eq!(g.mem.u32(0x6b52), area_offset);
    assert_eq!(g.mem.y(), 0, "spawn table must not touch Y");
}

#[test]
fn failed_level_load_preserves_all_state() {
    let good = file(area(&[map(1, 1), map(1, 1)], 0, 0));
    let mut g = ground(Some(good.clone()));
    assert!(walk::init_world(&mut g, 0, 0));
    assert!(walk::event_at(&mut g, 0, 0, 0));
    g.mem.set_previous_x(18);
    g.mem.set_previous_y(24);
    let mut stale = area(&[map(1, 1), map(1, 1)], 0, 0);
    stale[0x26a..0x26c].copy_from_slice(&0xffffu16.to_le_bytes());
    for bytes in [None, Some(vec![1, 2]), Some(file(stale)), Some(file(area(&[map(1, 1), map(0, 1)], 0, 0)))] {
        let mut files = Overlay::new(None);
        if let Some(bytes) = bytes {
            files.write("W.DAT", &bytes).unwrap();
        }
        g.files = Box::new(files);
        let (ds, segs, seed) = (g.mem.ds.clone(), g.mem.segs.clone(), g.rand.lcg.seed);
        assert!(!walk::event_at(&mut g, 0, 0, 1));
        assert_eq!(g.mem.ds, ds);
        assert_eq!(g.mem.segs, segs);
        assert_eq!(g.rand.lcg.seed, seed);
    }
    g.files.write("W.DAT", &good).unwrap();
    let first = g.mem.pointer(0xbc);
    let start = (first.seg as usize * 16 + g.mem.segs[&first.seg].len() + 15) & !15;
    g.mem.alloc((start / 16) as u16, 0x80000 - start);
    let (ds, segs) = (g.mem.ds.clone(), g.mem.segs.clone());
    assert!(!walk::event_at(&mut g, 0, 0, 1));
    assert_eq!(g.mem.ds, ds);
    assert_eq!(g.mem.segs, segs);
    for (world, area, level) in [(8, 0, 1), (0, 4, 1), (0, 0, -1), (0, 0, 8)] {
        assert!(!walk::event_at(&mut g, world, area, level));
        assert_eq!(g.mem.ds, ds);
        assert_eq!(g.mem.segs, segs);
    }
    g.files = Box::new(Overlay::new(None));
    assert!(walk::event_at(&mut g, 0, 0, 0));
    assert_eq!(g.mem.pointer(0xbc), first);
    assert_eq!((g.mem.x(), g.mem.y()), (18, 24));
}

struct TerminalInput;
impl Input for TerminalInput {
    fn next_key(&mut self) -> u16 {
        panic!("unexpected blocking input")
    }
    fn step(&mut self, step: &Step) {
        let messages: Vec<_> = step.text.iter().filter_map(|t| if let Text::Message(s) = t { Some(s.as_str()) } else { None }).collect();
        println!("TERMINAL tick={} messages={messages:?}", step.now);
    }
}
#[test]
fn terminal_flow_child() {
    let Ok(case) = std::env::var("MT_WALK_TERMINAL_TEST") else {
        return;
    };
    let mut g = ground(None);
    g.input = Box::new(TerminalInput);
    g.harness = true;
    g.mem.write_bytes(Mem::ds(0x4c7b), b"Party lost\0");
    g.mem.write_bytes(Mem::ds(0xe19), b"Ship lost\0");
    g.mem.write_bytes(Mem::ds(0x1000), b"You have failed.\0");
    match case.as_str() {
        "party" => walk::check_party_alive(&mut g),
        "ship" => {
            g.mem.set_countdown(0);
            walk::self_destruct_tick(&mut g);
        }
        "level" => {
            g.mem.set_event(1);
            walk::resolve_event(&mut g);
        }
        "world" => walk::enter_world(&mut g, 0, 0, 0, 0),
        "world-level" => {
            g.files.write("W.DAT", &file(area(&[], 0, 0))).unwrap();
            walk::enter_world(&mut g, 0, 0, 0, 0);
        }
        "rng-budget" => {
            g.harness = true;
            for _ in 0..=(1 << 20) {
                walk::random(&mut g, 10);
            }
        }
        "direct-rng-budget" => {
            g.rand.recording = true;
            for _ in 0..=(1 << 20) { g.rand.rand(); }
        }
        "text-budget" => {
            for _ in 0..100 { g.message_text(&"x".repeat(65536)); }
        }
        _ => panic!("unknown terminal test"),
    }
    panic!("terminal flow returned to simulation")
}
#[test]
fn game_over_and_load_failures_terminate() {
    for case in ["party", "ship", "level", "world", "world-level", "rng-budget", "direct-rng-budget", "text-budget"] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "terminal_flow_child", "--nocapture"])
            .env("MT_WALK_TERMINAL_TEST", case)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while child.try_wait().unwrap().is_none() {
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("terminal flow watchdog: {case}");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let out = child.wait_with_output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        let stderr = String::from_utf8_lossy(&out.stderr);
        let game_over = matches!(case, "party" | "ship");
        assert_eq!(out.status.code(), Some(if game_over { 1 } else { 3 }), "{case}: {stdout}\n{stderr}");
        if game_over {
            let why = if case == "party" { "Party lost" } else { "Ship lost" };
            assert!(stdout.contains(&format!("TERMINAL tick=100 messages=[\"{why}\", \"You have failed.\"]")), "{stdout}");
        } else if case.ends_with("rng-budget") {
            assert!(stderr.contains("RNG trace exceeds"), "{stderr}");
        } else if case == "text-budget" {
            assert!(stderr.contains("text log exceeds"), "{stderr}");
        } else {
            assert!(stderr.contains("unable to load W.DAT"), "{stderr}");
        }
    }
}

#[test]
fn live_rng_keeps_the_oracle_stream_without_retaining_draws() {
    let mut live = ground(None);
    let mut harness = ground(None);
    harness.harness = true;
    live.rand.lcg.seed = 4709;
    harness.rand.lcg.seed = 4709;
    for i in 0..100_000 {
        let bound = [0, 8, 25, -5, i16::MIN][i % 5];
        assert_eq!(walk::random(&mut live, bound), walk::random(&mut harness, bound));
        assert_eq!(live.mem.u32(0x5c20), harness.mem.u32(0x5c20));
    }
    assert_eq!(live.rand.lcg.seed, harness.rand.lcg.seed);
    assert_eq!(live.rand.trace.capacity(), 0);
    assert_eq!(harness.rand.trace.len(), 80_000);
    for _ in 0..100_000 { live.rand.rand(); live.message_text("live message"); }
    assert_eq!(live.rand.trace.capacity(), 0);
    assert_eq!(live.log.capacity(), 0);
}

#[test]
fn wait_ticks_restores_marked_deadline_after_overrun() {
    let mut g = ground(None);
    g.clock.set(100);
    g.mem.set_u32(0x4BDE, 20);
    walk::wait_ticks(&mut g, 3);
    assert_eq!(g.now(), 23);
    assert_eq!(g.mem.u32(0x4BDE), u32::MAX);
}

#[test]
fn item_stack_bits_do_not_open_blocked_terrain() {
    let mut g = ground(None);
    let p = Far::new(0x2000, Mem::DS);
    g.mem.write(p, 2, 1);
    assert_eq!(walk::foot_terrain(&mut g, p, 2), 0);
    g.mem.write(p, 3, 1);
    assert_eq!(walk::foot_terrain(&mut g, p, 2), 1);
}
