mod oracle_env;
use mt1::ground::{
    state::{Far, Mem},
    trace::{Frame, MAX_BYTES},
    walk::{self, Services},
    Ground,
};
use mt_engine::{
    io::{Dir, Scripted},
    sim::tick::Counter,
};
use std::{
    collections::{BTreeMap, VecDeque},
    io::{BufReader, Read},
    path::PathBuf,
    process::{Command, Stdio},
    time::{Duration, Instant},
};
const SEGS: [u16; 9] = [0, Mem::DS, 0x5000, 0x6000, 0x7000, 0x7100, 0x7200, 0x7300, 0x9000];
struct Reader<R>(R);
impl<R: Read> Reader<R> {
    fn u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.0.read_exact(&mut b).unwrap();
        u32::from_le_bytes(b)
    }
    fn bytes(&mut self) -> Vec<u8> {
        let n = self.u32() as usize;
        assert!(n < 2_000_000);
        let mut b = vec![0; n];
        self.0.read_exact(&mut b).unwrap();
        b
    }
    fn text(&mut self) -> String {
        String::from_utf8(self.bytes()).unwrap()
    }
    fn words(&mut self) -> Vec<u16> {
        (0..self.u32())
            .map(|_| {
                let mut b = [0; 2];
                self.0.read_exact(&mut b).unwrap();
                u16::from_le_bytes(b)
            })
            .collect()
    }
    fn writes(&mut self) -> Vec<(u32, Vec<u8>)> {
        (0..self.u32()).map(|_| (self.u32(), self.bytes())).collect()
    }
}
#[derive(Debug)]
struct Leaf {
    name: String,
    args: Vec<u16>,
    ret: u32,
    writes: Vec<(u32, Vec<u8>)>,
}
struct Script {
    leaves: VecDeque<Leaf>,
}
fn put(g: &mut Ground, at: u32, bytes: &[u8], traced: bool) {
    if traced {
        let seg =
            SEGS.iter().copied().filter(|&s| at >= s as u32 * 16 && at < s as u32 * 16 + 65536).max().expect("unmapped fixture write");
        g.mem.trace.write(seg, (at - seg as u32 * 16) as u16, bytes);
    }
    for seg in SEGS {
        let base = seg as u32 * 16;
        let block = if seg == Mem::DS { &mut g.mem.ds } else { g.mem.segs.get_mut(&seg).unwrap() };
        for (i, &v) in bytes.iter().enumerate() {
            let p = at + i as u32;
            if p >= base && p < base + block.len() as u32 {
                block[(p - base) as usize] = v
            }
        }
    }
}
impl Services for Script {
    fn call(&mut self, g: &mut Ground, name: &'static str, args: &[u16]) -> Option<u32> {
        if self.leaves.front().map(|l| l.name.as_str()) != Some(name) {
            return None;
        }
        let l = self.leaves.pop_front().unwrap();
        assert_eq!(l.args, args, "leaf {name} arguments");
        for (at, b) in &l.writes {
            put(g, *at, b, true)
        }
        Some(l.ret)
    }
}
#[derive(Debug)]
struct Expected {
    name: String,
    args: Vec<u16>,
    ax: u32,
    calls: Vec<(String, Vec<u16>, u32)>,
    writes: Vec<(u32, Vec<u8>)>,
}
fn footprint(writes: impl Iterator<Item = (u32, Vec<u8>)>) -> BTreeMap<u32, u8> {
    let mut b = BTreeMap::new();
    for (at, bytes) in writes {
        for (i, v) in bytes.into_iter().enumerate() {
            b.insert(at + i as u32, v);
        }
    }
    b
}
fn compare(actual: &Frame, e: &Expected, label: &str) {
    let entry = actual.entry.as_ref().expect("walk TRACE missing entry memory");
    assert_eq!((entry[0].0, entry[0].1, entry[0].2.len()), (Mem::DS, 0, 65536), "{label}: entry DS");
    assert_eq!(actual.name, e.name, "{label}");
    assert_eq!(actual.args, e.args, "{label}: arguments");
    assert_eq!(actual.ax.map(u32::from).unwrap_or(u32::MAX), e.ax, "{label}: AX");
    let calls: Vec<_> = actual.calls.iter().map(|c| (c.name.to_owned(), c.args.clone(), c.ret.unwrap_or(u32::MAX))).collect();
    assert_eq!(calls, e.calls, "{label}: {} callees", e.name);
    let actual = footprint(actual.writes.iter().map(|(s, o, b)| (*s as u32 * 16 + *o as u32, b.clone())));
    let expected = footprint(e.writes.clone().into_iter());
    if actual != expected {
        let bad: Vec<_> = actual
            .keys()
            .chain(expected.keys())
            .filter(|a| actual.get(a) != expected.get(a))
            .take(8)
            .map(|a| (format!("{a:x}"), actual.get(a), expected.get(a)))
            .collect();
        panic!("{label}: {} write footprint/value {bad:?}", e.name)
    }
}
fn export(root: &std::path::Path, out: &std::path::Path) -> bool {
    let Some((oracle, gf)) = oracle_env::both() else { return false };
    if std::env::var_os("MT_WALK_REUSE").is_some() {
        assert!(out.join("walk.bin").is_file());
        return true;
    }
    std::fs::create_dir_all(out).unwrap();
    let stdout = std::fs::File::create(out.join("export.log")).unwrap();
    let stderr = stdout.try_clone().unwrap();
    let mut child = Command::new("python")
        .arg("-B")
        .arg("-u")
        .arg(root.join("tools/m4a_dump_walk.py"))
        .arg(oracle)
        .arg(gf)
        .arg(out)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .expect("python exporter");
    let deadline = Instant::now() + Duration::from_secs(920);
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            let log = std::fs::read_to_string(out.join("export.log")).unwrap();
            println!("{log}");
            assert!(status.success() && !log.contains("PARTIAL"), "oracle export failed");
            break;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            let _ = child.wait();
            panic!("walk oracle watchdog")
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    true
}
#[test]
fn walk_oracle() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf();
    let out = root.join("target/m4a-walk-fixtures");
    if !export(&root, &out) {
        return;
    }
    let summary = std::fs::read_to_string(out.join("summary.txt")).unwrap();
    assert_eq!(summary.lines().next(), Some("composed_roots 1800"));
    let samples: usize = summary.lines().nth(1).unwrap().split_whitespace().nth(1).unwrap().parse().unwrap();
    assert_eq!(samples, 61627, "PARTIAL: incomplete walk proof fixtures");
    let mut r = Reader(BufReader::new(std::fs::File::open(out.join("walk.bin")).unwrap()));
    let mut magic = [0; 8];
    r.0.read_exact(&mut magic).unwrap();
    assert_eq!(&magic, b"MTWALK1\0");
    let mut invocations = 0;
    let mut peak_trace_bytes = 0;
    for _ in 0..samples {
        let label = r.text();
        let name = r.text();
        let args = r.words();
        let second = r.u32() as i16;
        let ax = r.u32();
        let init = r.writes();
        let leaves: VecDeque<Leaf> =
            (0..r.u32()).map(|_| Leaf { name: r.text(), args: r.words(), ret: r.u32(), writes: r.writes() }).collect();
        let expected: Vec<_> = (0..r.u32())
            .map(|_| Expected {
                name: r.text(),
                args: r.words(),
                ax: r.u32(),
                calls: (0..r.u32()).map(|_| (r.text(), r.words(), r.u32())).collect(),
                writes: r.writes(),
            })
            .collect();
        let mut lengths = [0usize; SEGS.len()];
        lengths[1] = 65536;
        lengths[8] = Mem::PARTY_SIZE;
        for (at, b) in init.iter().chain(leaves.iter().flat_map(|l| &l.writes)).chain(expected.iter().flat_map(|e| &e.writes)) {
            for (i, seg) in SEGS.iter().enumerate() {
                let base = *seg as u32 * 16;
                let end = at + b.len() as u32;
                if *at < base + 65536 && end > base {
                    lengths[i] = lengths[i].max((end.min(base + 65536) - base) as usize)
                }
            }
        }
        let mut g = Ground::from_ds(
            vec![0; 65536],
            vec![0; lengths[8]],
            Box::new(Scripted::new([])),
            Box::new(Counter::default()),
            Box::new(Dir(root.clone())),
        );
        for (seg, size) in SEGS.into_iter().zip(lengths) {
            if seg != Mem::DS {
                g.mem.alloc(seg, size)
            }
        }
        for (at, b) in init {
            put(&mut g, at, &b, false)
        }
        let entry: Vec<_> = std::iter::once((Mem::DS, 0, g.mem.ds.clone()))
            .chain(g.mem.segs.iter().map(|(&seg, bytes)| (seg, 0, bytes.clone())))
            .chain(std::iter::once((Mem::BIOS_TICKS.seg, Mem::BIOS_TICKS.off, g.mem.bios_ticks.to_vec())))
            .collect();
        let mut script = Script { leaves };
        g.mem.trace.on = true;
        let result = walk::replay(&mut g, &name, &args, second, &mut script);
        assert_eq!(g.mem.trace.depth(), 0, "{label}: unclosed TRACE");
        assert_eq!(g.mem.trace.done.last().unwrap().entry.as_ref(), Some(&entry), "{label}: exact root entry memory");
        peak_trace_bytes = peak_trace_bytes.max(g.mem.trace.retained_bytes());
        if ax != u32::MAX {
            assert_eq!(result, ax, "{label} root AX")
        }
        assert!(script.leaves.is_empty(), "{label}: unconsumed leaves {:?}", script.leaves);
        assert_eq!(g.mem.trace.done.len(), expected.len(), "{label}: invocation count");
        for (actual, e) in g.mem.trace.done.iter().zip(&expected) {
            compare(actual, e, &label);
            invocations += 1
        }
    }
    let mut b = [0; 1];
    assert_eq!(r.0.read(&mut b).unwrap(), 0);
    assert_eq!(invocations, 76228, "PARTIAL: incomplete walk invocation coverage");
    println!(
        "PASS walk: 1800 composed cases, {samples} replays, {invocations} invocations; args/AX/callees/write footprint/value 0 differ"
    );
    assert!(peak_trace_bytes <= MAX_BYTES);
    println!("PASS entry snapshots: {samples} exact root images, {invocations} captured entries; peak retained TRACE {peak_trace_bytes}/{MAX_BYTES} bytes");
    runtime(&out);
}
#[test]
fn hostile_inputs_are_bounded() {
    let mut g = Ground::from_ds(
        vec![255; 65536],
        vec![0; Mem::PARTY_SIZE],
        Box::new(Scripted::new([])),
        Box::new(Counter::default()),
        Box::new(Dir(std::env::temp_dir())),
    );
    g.mem.set_npc_mode(1);
    walk::move_party(&mut g, i16::MIN, i16::MAX, i16::MAX);
    walk::npc_can_step(&mut g, i16::MAX, i16::MAX, Far::new(u16::MAX, Mem::DS));
}
fn runtime(out: &std::path::Path) {
    let gf = PathBuf::from(std::env::var_os("MT1_GAMEFILES").unwrap());
    let worlds = mt_engine::data::world::decode(mt_engine::data::Variant::Mt1, &std::fs::read(gf.join("W.DAT")).unwrap()).unwrap();
    let before = std::fs::read(out.join("runtime-before.bin")).unwrap();
    let mut cases = 0;
    let mut levels = 0;
    for line in std::fs::read_to_string(out.join("runtime.txt")).unwrap().lines() {
        let t: Vec<_> = line.split_whitespace().collect();
        let n = |i: usize| t[i].parse::<i64>().unwrap();
        let mut g = Ground::from_ds(
            before[..65536].to_vec(),
            before[65536..].to_vec(),
            Box::new(Scripted::new([])),
            Box::new(Counter::default()),
            Box::new(Dir(gf.clone())),
        );
        g.harness = true;
        match t[0] {
            "random" => {
                g.rand.lcg.seed = n(1) as u32;
                g.mem.set_u32(0x5c20, n(1) as u32);
                assert_eq!(walk::random(&mut g, n(2) as i16) as u16, n(3) as u16, "{line}");
                assert_eq!(g.rand.lcg.seed, n(4) as u32, "{line}");
                assert_eq!(g.mem.u32(0x5c20), n(4) as u32, "{line}");
                assert_eq!(g.rand.trace.len(), usize::from(n(2) != 0));
            }
            "position" => {
                g.mem.set_map_blocks_high(n(3) as u8);
                g.mem.set_map_stride(n(4) as i16);
                walk::set_player_pos(&mut g, n(1) as i16, n(2) as i16);
                for (i, off) in [0x6b6c, 0x63f5, 0x6415, 0x6266, 0x6c27, 0x6420].into_iter().enumerate() {
                    assert_eq!(g.mem.u16(off), n(i + 5) as u16, "{line} offset {off:x}");
                }
            }
            "world" => {
                g.mem.alloc(0x6000, 65536);
                g.mem.write_bytes(Far::new(0, 0x6000), &before[65536..]);
                g.mem.set_pointer(0x6d65, Far::new(0, 0x6000));
                g.rand.lcg.seed = 1;
                assert!(walk::init_world(&mut g, n(1) as u16, n(2) as u16));
                let expected = std::fs::read(out.join(t[4])).unwrap();
                for (off, size) in [
                    (0x5e06, 0x268),
                    (0x6c2f, 12),
                    (0x5e02, 4),
                    (0xfa, 2),
                    (0x6b6c, 2),
                    (0x6c27, 2),
                    (0x63f5, 2),
                    (0x6420, 2),
                    (0x4cde, 8),
                    (0x6fc6, 10),
                    (0x6418, 1),
                    (0x6422, g.mem.npc_count() as usize * 46),
                    (0x6b69, 1),
                    (0x6340, g.mem.u8(0x6b69) as usize * 6),
                    (0x6b63, 1),
                    (0x6d12, 1),
                    (0x6d13, 4),
                ] {
                    assert_eq!(&g.mem.ds[off..off + size], &expected[off..off + size], "world {} {} DS:{off:x}", t[1], t[2]);
                }
                assert_eq!(g.mem.bytes(Far::new(0, 0x6000), Mem::PARTY_SIZE), expected[65536..], "world party");
                assert_eq!(g.rand.lcg.seed, n(3) as u32, "world RNG state");
                let draws: Vec<u16> = t[5].split(',').map(|s| s.parse().unwrap()).collect();
                assert_eq!(g.rand.trace, draws, "world RNG draws");
                let base = [0, 4, 7, 13, 16, 18, 21, 24][n(1) as usize];
                let mt_engine::data::world::Chunk::Area(area) = &worlds[base + n(2) as usize] else { panic!("missing area") };
                g.mem.set_pointer(0x6b6e, g.mem.pointer(0x6d65).add(0x602));
                g.mem.set_level(-1);
                let mut cached = Vec::new();
                for (level, off) in area.level_offs[..5].iter().enumerate() {
                    if !area.map_offs.contains(&(*off as usize)) {
                        continue;
                    }
                    assert!(walk::event_at(&mut g, n(1) as u16, n(2) as u16, level as i16), "load {line} level {level}");
                    let p = g.mem.pointer(0xbc);
                    cached.push((p, g.mem.segs[&p.seg].clone()));
                    for (p, bytes) in &cached {
                        assert_eq!(&g.mem.segs[&p.seg], bytes, "loading a level changed another cached map");
                    }
                    let mut ranges: Vec<_> = g.mem.segs.iter().map(|(&s, b)| (s as usize * 16, b.len())).collect();
                    ranges.push((Mem::DS as usize * 16, g.mem.ds.len()));
                    ranges.sort_unstable();
                    let mut end = 0;
                    for (at, size) in ranges {
                        assert!(at >= end && at + size <= 0xa0000, "overlapping or non-conventional TRACE memory at {at:x}");
                        end = at + size;
                    }
                    levels += 1;
                }
                assert!(walk::init_world(&mut g, n(1) as u16, n(2) as u16));
                for (p, _) in cached {
                    assert!(!g.mem.segs.contains_key(&p.seg), "area reload leaked cached map memory");
                }
            }
            _ => panic!("unknown fixture {line}"),
        }
        cases += 1;
    }
    assert_eq!(cases, 107);
    assert_eq!(levels, 64, "PARTIAL: incomplete real-level coverage");
    println!("PASS runtime: 107 unmodified T.exe RNG/position/world cases, 0 differ");
    println!("PASS map heap: {levels} real levels; disjoint conventional memory, cache preservation, and area reload cleanup");
    let mut terrain_cases = 0;
    let mut terrain = Ground::from_ds(
        vec![0; 65536],
        vec![0; Mem::PARTY_SIZE],
        Box::new(Scripted::new([])),
        Box::new(Counter::default()),
        Box::new(Dir(gf.clone())),
    );
    for line in std::fs::read_to_string(out.join("terrain.txt")).unwrap().lines() {
        let row: Vec<i16> = line.split_whitespace().map(|s| s.parse().unwrap()).collect();
        assert_eq!(row.len(), 3);
        terrain.mem.set_u8(0x2000, row[0] as u8);
        assert_eq!(walk::foot_terrain(&mut terrain, Mem::ds(0x2000), row[1]), row[2], "terrain {line}");
        terrain_cases += 1;
    }
    assert_eq!(terrain_cases, 3072, "PARTIAL: incomplete terrain oracle coverage");
    println!("PASS foot_terrain: {terrain_cases} walk.py + original 1BFCE (test at 1BFD4) cases, 0 differ");
    let mut g = Ground::from_ds(
        before[..65536].to_vec(),
        before[65536..].to_vec(),
        Box::new(Scripted::new([])),
        Box::new(Counter::default()),
        Box::new(Dir(gf)),
    );
    g.mem.set_party_u8(6, 2);
    g.mem.set_party_u8(7, 0);
    g.mem.set_party_u8(0xc1, g.mem.party_u8(0xc1) | 128);
    g.mem.set_outdoors(1);
    g.rand.lcg.seed = 1;
    assert!(walk::boot(&mut g));
    assert_eq!((g.mem.x(), g.mem.y(), g.mem.level(), g.mem.outdoors()), (44, 12, 0, 1));
    assert_eq!(g.mem.party().len(), Mem::PARTY_SIZE);
    let map = g.mem.pointer(0xbc);
    let bytes = g.mem.bytes(map, 3600);
    assert!(walk::event_at(&mut g, 2, 0, 0));
    assert_eq!(g.mem.pointer(0xbc), map);
    assert_eq!(g.mem.bytes(map, 3600), bytes);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
