pub(crate) mod schema;
pub(crate) mod symbolic;
use mt_engine::{
    data::{Variant, exepack, ly, pic},
    io::{Dir, Files, Key, Overlay, Scripted, mouse::MouseEvent},
    sim::tick::{Clock, Counter},
};
use mt1::{
    ground::{self, Ground, state::Far},
    lockstep::{SpaceEvent, SpaceObserver},
    screens::{
        cg::Screens,
        title::{TcRand, Title},
    },
    space::{self, Space},
};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::{self, File},
    io::{Read, Write},
    path::{Path, PathBuf},
    rc::Rc,
};

const MAX_BYTES: usize = 1024 * 1024;
#[derive(Clone, Debug)]
struct Scan {
    code: u8,
    ascii: u8,
}
#[derive(Clone, Debug)]
struct Step {
    tick: u32,
    key: String,
    scans: Vec<Scan>,
}
#[derive(Clone, Deserialize)]
struct Seed {
    step: usize,
    prog: String,
    seed: u16,
}
#[derive(Clone, Deserialize)]
struct PokeByte {
    target: String,
    value: u8,
}
#[derive(Clone, Deserialize)]
struct Poke {
    step: usize,
    hex: String,
    bytes: Vec<PokeByte>,
}
#[derive(Clone, Deserialize)]
struct Mark {
    name: String,
    tick: u32,
}
#[derive(Clone, Deserialize)]
struct SeedTime {
    prog: String,
    tick: u32,
    step: usize,
}
#[derive(Clone, Deserialize)]
struct ReadTime {
    tick: u32,
}
#[derive(Clone, Deserialize)]
struct Stop {
    step: usize,
    reason: String,
}
#[derive(Clone, Deserialize)]
struct SEvent {
    k: String,
    #[serde(default)]
    step: usize,
    at: [u32; 6],
    #[serde(default)]
    minimap_returns: Option<u32>,
    #[serde(default)]
    phase: Option<String>,
    #[serde(default)]
    tick: Option<u32>,
}
impl SEvent {
    fn reached(&self, at: &[u32; 6], minimap_returns: u32) -> bool {
        self.at.iter().zip(at).all(|(a, b)| a <= b)
            && self.minimap_returns.is_none_or(|n| n <= minimap_returns)
    }
    fn exact(&self, at: &[u32; 6], minimap_returns: u32) -> bool {
        self.at == *at && self.minimap_returns.is_none_or(|n| n == minimap_returns)
    }
}
#[derive(Default, Deserialize)]
struct STransport {
    reads: Vec<(u32, u32)>,
    events: Vec<SEvent>,
}
#[derive(Default, Deserialize)]
struct Transport {
    seeds: Vec<SeedTime>,
    checkpoints: Vec<Mark>,
    #[serde(rename = "cgReads")]
    cg_reads: Vec<ReadTime>,
    stop: Option<Stop>,
    #[serde(default)]
    s: Option<STransport>,
}

fn text(path: &Path) -> Result<String, String> {
    let f = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut b = vec![];
    f.take(MAX_BYTES as u64 + 1)
        .read_to_end(&mut b)
        .map_err(|e| e.to_string())?;
    if b.len() > MAX_BYTES {
        return Err("lockstep input exceeds 1 MiB".into());
    }
    String::from_utf8(b).map_err(|e| e.to_string())
}
fn read_json(path: &Path) -> Result<Value, String> {
    serde_json::from_str(&text(path)?).map_err(|e| format!("{}: {e}", path.display()))
}
fn scans(token: &str) -> Result<Vec<Scan>, String> {
    if token.is_empty() || token.starts_with("!w ") || token.starts_with("!m ") {
        return Ok(vec![]);
    }
    let table: Value =
        serde_json::from_str(include_str!("../../../../docs/lockstep/keys.json")).unwrap();
    let (name, mode) = if token.starts_with('{') && token.ends_with('}') && token.len() > 2 {
        let inner = &token[1..token.len() - 1];
        if let Some(n) = inner.strip_suffix('+') {
            (n, 1)
        } else if let Some(n) = inner.strip_suffix('-') {
            (n, 2)
        } else {
            (inner, 0)
        }
    } else {
        (token, 0)
    };
    let (code, ascii) = if name.len() == 3
        && name.starts_with('s')
        && name[1..].bytes().all(|c| c.is_ascii_hexdigit())
    {
        (u8::from_str_radix(&name[1..], 16).unwrap(), 0)
    } else {
        let row = table
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["name"] == name)
            .ok_or_else(|| format!("unknown key {token:?}"))?;
        (
            row["make"].as_u64().unwrap() as u8,
            row["ascii"].as_u64().unwrap() as u8,
        )
    };
    if code >= 128 {
        return Err("raw make scancode must be below 80h".into());
    }
    let mut out = vec![];
    if mode != 2 {
        out.push(Scan { code, ascii })
    }
    if mode != 1 {
        out.push(Scan {
            code: code | 128,
            ascii: 0,
        })
    }
    Ok(out)
}
fn parse(t: &str) -> Result<Vec<Step>, String> {
    if t.len() > MAX_BYTES {
        return Err("keys exceeds 1 MiB".into());
    }
    let mut steps = vec![];
    let mut ticks = 0u32;
    for (i, line) in t.lines().enumerate() {
        let line = line.split('#').next().unwrap().trim();
        if line.len() > 1024 {
            return Err(format!("line {} exceeds 1 KiB", i + 1));
        }
        if line.is_empty() {
            continue;
        }
        let (w, k) = line.split_once(char::is_whitespace).unwrap_or((line, ""));
        if !w.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("line {}: invalid WAIT", i + 1));
        }
        let wait = w.parse::<u32>().map_err(|_| "invalid WAIT")?;
        ticks = ticks
            .checked_add(wait)
            .filter(|&n| n <= 200000)
            .ok_or("replay exceeds 200000 ticks")?;
        let key = if k.trim() == "SPACE" { " " } else { k.trim() }.to_string();
        if key.starts_with("!w ") {
            let p: Vec<_> = key.split_whitespace().collect();
            if p.len() != 3
                || u32::from_str_radix(p[1], 16).is_err()
                || schema::unhex(p[2])?.is_empty()
            {
                return Err("invalid poke line".into());
            }
        }
        let scans = scans(&key).map_err(|e| format!("line {}: {e}", i + 1))?;
        if key.starts_with("!m ") && click(&key).is_none() {
            return Err(format!("line {}: invalid click line (`!m <col> <row> L|R`)", i + 1));
        }
        steps.push(Step {
            tick: ticks.saturating_sub(1),
            key,
            scans,
        });
        if steps.len() > 65536 {
            return Err("too many steps".into());
        }
    }
    if steps.is_empty() {
        return Err("empty lockstep script".into());
    }
    Ok(steps)
}

fn click(key: &str) -> Option<MouseEvent> {
    let p: Vec<_> = key.split_whitespace().collect();
    if p.len() != 4 { return None; }
    let (col, row) = (p[1].parse::<i16>().ok().filter(|c| (0..320).contains(c))?, p[2].parse::<i16>().ok().filter(|r| (0..200).contains(r))?);
    let event = match p[3] { "L" => 4, "R" => 0x10, _ => return None };
    Some(MouseEvent { event, buttons: 0, col, row })
}
struct Replay {
    steps: Vec<Step>,
    next: usize,
    seeds: Vec<Seed>,
    seed_next: usize,
    pokes: Vec<Poke>,
    pending: VecDeque<Scan>,
    pending_mouse: VecDeque<MouseEvent>,
    pending_tick: u32,
    setup: Vec<u8>,
    cg_late_max: u32,
    schema: Value,
    fields: Vec<schema::Field>,
    output: File,
    path: PathBuf,
    marks: VecDeque<Mark>,
    cg_reads: VecDeque<ReadTime>,
    seed_times: Vec<SeedTime>,
    t_seed: Option<u32>,
    party: Vec<u8>,
    pokes_done: usize,
    polls: u32,
    stop: Option<Stop>,
    s_reads: VecDeque<(u32, u32)>,
    s_events: VecDeque<SEvent>,
    s_tick: u32,
    s_base: [u32; 7], s_last: [u32; 7],
    save_at: Option<usize>, save_fault: bool, save: Option<Value>,
}
type Shared = Rc<RefCell<Replay>>;
fn offset(raw: &[u32; 7], base: &[u32; 7]) -> ([u32; 6], u32) {
    (std::array::from_fn(|i| raw[i] + base[i]), raw[6] + base[6])
}
impl Replay {
    fn open(
        keys: &Path,
        seeds: &Path,
        pokes: Option<&Path>,
        state: &Path,
        transport: Option<&Path>,
    ) -> Result<Self, String> {
        let steps = parse(&text(keys)?)?;
        let seeds: Vec<Seed> = serde_json::from_value(read_json(seeds)?["seeds"].clone())
            .map_err(|e| e.to_string())?;
        let pokes: Vec<Poke> = match pokes {
            Some(p) => {
                serde_json::from_value(read_json(p)?["pokes"].clone()).map_err(|e| e.to_string())?
            }
            None => vec![],
        };
        if seeds.len() > 1024 || pokes.len() > 65536 {
            return Err("sidecar count exceeds budget".into());
        }
        if seeds.first().is_none_or(|s| s.prog != "STARTUP") {
            return Err("align: replay must begin with STARTUP srand".into());
        }
        if seeds.windows(2).any(|w| w[0].step > w[1].step)
            || seeds.iter().any(|s| {
                s.step >= steps.len() || !["STARTUP", "CG", "T", "S"].contains(&s.prog.as_str())
            })
        {
            return Err("invalid seed order/program/step".into());
        }
        for (i, s) in steps.iter().enumerate() {
            let p: Vec<_> = pokes.iter().filter(|p| p.step == i).collect();
            if s.key.starts_with("!w ") {
                if p.len() != 1 {
                    return Err(format!(
                        "step {i}: exactly one symbolic poke sidecar required"
                    ));
                }
                let expected = s.key.split_whitespace().nth(2).unwrap();
                let b = schema::unhex(expected)?;
                if p[0].hex != expected
                    || p[0].bytes.iter().map(|b| b.value).collect::<Vec<_>>() != b
                {
                    return Err(format!("step {i}: poke bytes disagree with keys"));
                }
            } else if !p.is_empty() {
                return Err(format!("step {i}: unexpected poke sidecar"));
            }
        }
        if pokes.iter().any(|p| p.step >= steps.len()) {
            return Err("poke step outside script".into());
        }
        let tr: Transport = match transport {
            Some(p) => serde_json::from_value(read_json(p)?).map_err(|e| e.to_string())?,
            None => Transport::default(),
        };
        if !tr.seeds.is_empty()
            && (tr.seeds.len() > seeds.len()
                || tr
                    .seeds
                    .iter()
                    .zip(&seeds)
                    .any(|(a, b)| a.prog != b.prog || a.step != b.step))
        {
            return Err("align: transport/seed order differs".into());
        }
        if tr.seeds.iter().any(|s| s.tick > 200000)
            || tr.checkpoints.iter().any(|m| {
                m.tick > 200000
                    || ![
                        "show_message",
                        "init_world",
                        "load_map",
                        "tileset_loaded",
                        "main_loop",
                        "main_loop_ready",
                    ]
                    .contains(&m.name.as_str())
            })
            || tr.cg_reads.iter().any(|m| m.tick > 200000)
            || tr.checkpoints.windows(2).any(|w| w[0].tick > w[1].tick)
            || tr.cg_reads.windows(2).any(|w| w[0].tick > w[1].tick)
            || tr.stop.as_ref().is_some_and(|s| {
                s.step >= steps.len() || !["S", "save", "restore", "death", "exit"].contains(&s.reason.as_str())
            })
        {
            return Err("invalid clock transport".into());
        }
        let sch = schema::schema();
        let mut fields = schema::fields(&sch["properties"]["party"], "party", 0);
        for (sec, base) in [("t", 0), ("cg", 0), ("startup", 0), ("s", 0)] {
            fields.extend(schema::fields(&sch["properties"][sec], sec, base));
        }
        for i in 0..18 {
            for (k, name) in ["x", "y", "active", "dstx", "dsty"].iter().enumerate() {
                fields.push(schema::Field { path: format!("s.ships[{i}].{name}"), offset: 0x3ab0 + 10 * i + 2 * k, width: 2, text: false, hex: false });
            }
        }
        for i in 0..32 {
            let p = format!("t.monsters[{i}]");
            let off = 0x63f4 + (i + 1) * 46;
            fields.extend(schema::fields(
                &schema::node(&sch["properties"]["t"])["properties"]["monsters"]["items"],
                &p,
                off,
            ));
            fields.push(schema::Field {
                path: format!("{p}.raw"),
                offset: off,
                width: 46,
                text: false,
                hex: true,
            });
        }
        for i in 0..15 {
            fields.push(schema::Field {
                path: format!("cg.pool[{i}]"),
                offset: 0x1840 + i * 256,
                width: 256,
                text: false,
                hex: true,
            });
        }
        for field in symbolic::catalog() {
            if !fields.iter().any(|f| f.path == field.path) { fields.push(field); }
        }
        if let Some(parent) = state.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?
        }
        let output = File::create(state).map_err(|e| e.to_string())?;
        Ok(Self {
            steps,
            next: 0,
            seeds,
            seed_next: 0,
            pokes,
            pending: VecDeque::new(),
            pending_mouse: VecDeque::new(),
            pending_tick: 0,
            setup: Vec::new(),
            cg_late_max: 0,
            schema: sch,
            fields,
            output,
            path: state.into(),
            marks: tr.checkpoints.into(),
            cg_reads: tr.cg_reads.into(),
            seed_times: tr.seeds,
            t_seed: None,
            party: ground::party::initial(),
            pokes_done: 0,
            polls: 0,
            stop: tr.stop,
            s_reads: tr.s.as_ref().map_or(VecDeque::new(), |s| s.reads.iter().copied().collect()),
            s_events: tr.s.map_or(VecDeque::new(), |s| s.events.into()),
            s_tick: 0,
            s_base: [0; 7], s_last: [0; 7],
            save_at: None, save_fault: false, save: None,
        })
    }
    fn fail(&mut self, msg: &str) -> ! {
        self.finish("error", Some(self.next), msg, 3)
    }
    fn finish(&mut self, status: &str, stop: Option<usize>, reason: &str, code: i32) -> ! {
        let (status, reason, code) = match self.save_at {
            Some(k) if self.save.is_none() && status != "error" => ("error", format!("save-at {k}: step never sampled in T"), 3),
            _ => (status, reason.to_string(), code),
        };
        let sum = json!({"status":status,"save":self.save,"ref_steps":self.steps.len(),"emitted":self.next,"stop_step":stop,"reason":reason,"cg_late_max":self.cg_late_max,"setup":String::from_utf8_lossy(&self.setup),"seeds_used":self.seed_next,"seeds_unmodeled":self.seeds[self.seed_next..].iter().map(|s|json!({"prog":s.prog,"step":s.step})).collect::<Vec<_>>(),"pokes_read_back":self.pokes_done});
        if let Err(e) = self
            .output
            .flush()
            .and_then(|_| fs::write(self.path.with_extension("summary.json"), format!("{sum}\n")))
        {
            eprintln!("lockstep output: {e}");
            std::process::exit(3)
        }
        println!("LOCKSTEP {sum}");
        std::process::exit(code)
    }
    fn seed(&mut self, prog: &str) -> u16 {
        let Some(s) = self.seeds.get(self.seed_next).cloned() else {
            self.fail(&format!("align: missing srand for {prog}"))
        };
        if s.prog != prog || s.step != self.next {
            self.fail(&format!(
                "align: srand {} expected {} at step {}, got {prog} at step {}",
                self.seed_next, s.prog, s.step, self.next
            ))
        }
        self.seed_next += 1;
        s.seed
    }
    fn seed_tick(&self) -> Option<u32> {
        self.seed_times.get(self.seed_next).map(|s| s.tick)
    }
    fn due(&mut self, now: u32) -> Option<(Step, Vec<PokeByte>)> {
        self.polls += 1;
        if self.polls > 2000000 {
            self.fail("poll budget exceeded")
        }
        if self.next == self.steps.len() {
            self.finish("complete", None, "end of script", 0)
        }
        let step = self.steps[self.next].clone();
        if now < step.tick {
            return None;
        }
        if let Some(stop) = self.stop.clone().filter(|s| s.step == self.next) {
            if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("horizon reached in due() at {now}") }
            self.finish(
                "horizon",
                Some(stop.step),
                &format!(
                    "measured {} horizon; native screen not reached",
                    stop.reason
                ),
                0,
            )
        }
        let bytes = self
            .pokes
            .iter()
            .find(|p| p.step == self.next)
            .map_or(vec![], |p| p.bytes.clone());
        Some((step, bytes))
    }
    fn record(&mut self, step: Step, mut record: Value, bytes: &[PokeByte]) {
        if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("sample step {} tick {}", self.next, step.tick) }
        record["step"] = self.next.into();
        record["key"] = step.key.into();
        for b in bytes {
            if schema::byte(&record, &b.target) != Some(b.value) {
                self.fail(&format!("poke read-back failed: {}", b.target))
            }
        }
        if !bytes.is_empty() {
            self.pokes_done += 1
        }
        if let Err(e) = serde_json::to_writer(&mut self.output, &record)
            .and_then(|_| self.output.write_all(b"\n").map_err(serde_json::Error::io))
        {
            eprintln!("state write: {e}");
            std::process::exit(3)
        }
        self.pending.extend(step.scans);
        self.pending_mouse.extend(click(&self.steps[self.next].key));
        self.pending_tick = step.tick;
        self.next += 1;
    }
    fn base(&self, prog: &str, rng: Value, now: u32) -> Value {
        let mut out = Map::new();
        for (n, v) in [
            ("v", 1.into()),
            ("step", 0.into()),
            ("key", "".into()),
            ("prog", prog.into()),
            ("rng", rng),
            ("bios", json!({"ticks":now})),
        ] {
            out.insert(n.into(), v);
        }
        Value::Object(out)
    }
    fn section(&self, record: &mut Value, name: &str, value: Value) {
        let old = record.as_object_mut().unwrap();
        let bios = old.shift_remove("bios").unwrap();
        old.insert(name.into(), value);
        old.insert("bios".into(), bios);
    }
    fn party(&self, b: &[u8]) -> Value {
        let s = &self.schema["properties"]["party"];
        let p = schema::decode(s, b);
        assert_eq!(
            schema::encode_party(s, &p).expect("party encode"),
            b,
            "party byte round trip"
        );
        p
    }
    fn poke_offset(&mut self, target: &str) -> (String, usize) {
        symbolic::resolve(&self.fields, target).unwrap_or_else(|e| self.fail(&e))
    }
}

fn rng(cg: Option<u32>, t: Option<u32>) -> Value {
    json!({"STARTUP":null,"CG":cg,"T":t,"S":null})
}
fn cg_record(r: &Replay, s: &mut Screens) -> Value {
    let now = s.clock.now();
    let mut record = r.base("CG", rng(Some(s.rng.state), r.t_seed), now);
    r.section(&mut record, "party", r.party(&r.party));
    let mut ds = s.ds.image().to_vec();
    ds[0x15b0..0x15b0 + 57].copy_from_slice(&s.work);
    let mut cg = schema::decode(&r.schema["properties"]["cg"], &ds);
    let raw = cg.as_object_mut().unwrap().shift_remove("raw").unwrap();
    cg["pool"] = Value::Array(
        (0..15)
            .map(|i| schema::hex(&ds[0x1840 + i * 256..0x1940 + i * 256]).into())
            .collect(),
    );
    cg["raw"] = raw;
    r.section(&mut record, "cg", cg);
    record
}
fn t_record(r: &Replay, g: &mut Ground) -> Value {
    let now = g.clock.now();
    let mut record = r.base("T", rng(None, Some(g.rand.lcg.seed)), now);
    r.section(&mut record, "party", r.party(g.mem.party()));
    let mut t = schema::decode(&r.schema["properties"]["t"], &g.mem.ds);
    let law = g.mem.pointer(0x5e02);
    t["world_law"] = if law == Far::default() {
        Value::Null
    } else {
        g.mem.read(law.add(6), 1).into()
    };
    let map = g.mem.pointer(0xbc);
    let len = g.mem.u16(0x6d2b) as usize * g.mem.u8(0x5c3a) as usize * 12 * 3;
    t["map_sha1"] = if map != Far::default() && len > 0 && len < 65536 {
        schema::sha1(&g.mem.bytes(map, len)).into()
    } else {
        Value::Null
    };
    t["monsters"]=Value::Array((1..=32).map(|i| {
        let at=0x63f4+i*46;json!({"level":g.mem.u8(at+2),"x":g.mem.i16(at+32),"y":g.mem.i16(at+34),"raw":schema::hex(&g.mem.ds[at as usize..at as usize+46])})
    }).collect());
    for poke in &r.pokes {
        for byte in &poke.bytes { symbolic::emit_requested(&mut t,g,&r.fields,&byte.target); }
    }
    r.section(&mut record, "t", t);
    record
}
fn set_byte(value: &mut u32, k: usize, b: u8) {
    *value = (*value & !(255u32 << (k * 8))) | ((b as u32) << (k * 8));
}
fn cg_visit(r: &mut Replay, s: &mut Screens) {
    while let Some((step, bytes)) = r.due(s.clock.ticks()) {
        for b in &bytes {
            let (sec, at) = r.poke_offset(&b.target);
            match sec.as_str() {
                "party" => r.party[at] = b.value,
                "rng.CG" => set_byte(&mut s.rng.state, at, b.value),
                "rng.T" => {
                    if let Some(seed) = &mut r.t_seed {
                        set_byte(seed, at, b.value)
                    } else {
                        r.fail("T RNG not resident")
                    }
                }
                "cg" => {
                    if (0x15b0..0x15e9).contains(&at) {
                        s.work[at - 0x15b0] = b.value
                    } else if at == 0x2780 {
                        s.ds.set_u8(at, b.value)
                    } else {
                        r.fail(&format!(
                            "CG symbolic write has no live owner: {}",
                            b.target
                        ))
                    }
                }
                _ => r.fail(&format!("unmodeled poke {}", b.target)),
            }
        }
        let record = cg_record(r, s);
        r.record(step, record, &bytes);
    }
    s.rng.trace.clear();
    s.log.clear();
}
fn t_visit(r: &mut Replay, g: &mut Ground) {
    while let Some((step, bytes)) = r.due(g.clock.ticks()) {
        for b in &bytes {
            if let Err(e) = symbolic::write(g, &r.fields, &b.target, b.value) { r.fail(&e) }
        }
        let record = t_record(r, g);
        r.record(step, record, &bytes);
        if r.save_at == Some(r.next - 1) && r.save.is_none() { save_round_trip(r, g) }
    }
}
fn save_round_trip(r: &mut Replay, g: &mut Ground) {
    use mt1::save::{Mode, Saved};
    let k = r.next - 1;
    if g.boot_calibrating { r.fail(&format!("save-at {k}: inside the boot phase (no save point)")) }
    let mode = if matches!(g.mem.event(), 9 | 10) { Mode::Starport } else { Mode::Ground };
    let bytes = Saved::capture(g, mode, env!("MT_BUILD_HASH")).encode();
    let path = r.path.with_file_name(format!("save-{k}.mtsav"));
    let back = match fs::write(&path, &bytes).and_then(|_| fs::read(&path)) { Ok(b) => b, Err(e) => r.fail(&format!("save file {}: {e}", path.display())) };
    if back != bytes { r.fail("save file read back differs") }
    g.mem.ds.fill(0xA5); g.mem.segs.clear(); g.mem.bios_ticks = [0xA5; 4]; g.rand.lcg.seed = 0xA5A5A5A5; g.clock.set(0xA5A5A5A5);
    g.landing = !g.landing; g.sound_option = 0; g.speaker_refresh = 0xA5; g.fb.regs = [0xA5; 16]; g.dac = [[0xA5; 3]; 256];
    let mut saved = match Saved::decode(&back) { Ok(s) => s, Err(e) => r.fail(&format!("save decode: {e}")) };
    if r.save_fault { let s = saved.seed & !0xFF; saved.seed = if s == saved.seed { s | 1 } else { s } }
    saved.install(g);
    r.save = Some(json!({"step": k, "tick": g.clock.ticks(), "mode": format!("{mode:?}").to_lowercase(), "bytes": bytes.len(), "file": path, "sha256": sha256_hex(&bytes), "fault": r.save_fault}));
}
struct CgTap(Shared);
impl mt1::lockstep::CgObserver for CgTap {
    fn key(&mut self, s: &mut Screens) -> Key {
        let mut r = self.0.borrow_mut();
        let target = r.cg_reads.pop_front().map(|m| m.tick);
        if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("cg read at {} measured {target:?}", s.clock.ticks()) }
        if let Some(t) = target.filter(|&t| s.clock.ticks() > t) {
            let late = s.clock.ticks() - t;
            r.cg_late_max = r.cg_late_max.max(late);
        }
        loop {
            cg_visit(&mut r, s);
            if target.is_none_or(|t| s.clock.ticks() >= t) {
                while let Some(scan) = r.pending.pop_front() {
                    if scan.code < 128 {
                        return if scan.ascii != 0 {
                            scan.ascii as Key
                        } else {
                            0x100 + scan.code as Key
                        };
                    }
                }
            }
            s.clock.wait_tick();
        }
    }
    fn delay(&mut self, s: &mut Screens, ticks: i16) -> bool {
        let mut r = self.0.borrow_mut();
        if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("cg delay({ticks}) at {}", s.clock.ticks()) }
        let end = s.clock.ticks().saturating_add(ticks.max(0) as u32);
        while s.clock.ticks() < end {
            cg_visit(&mut r, s);
            if r.pending.iter().any(|s| s.code < 128) {
                r.pending.clear();
                return true;
            }
            r.pending.clear();
            s.clock.wait_tick();
        }
        cg_visit(&mut r, s);
        false
    }
}
struct SpeakerLog { shared: Shared, inner: Scripted, out: File }
impl mt_engine::io::Input for SpeakerLog {
    fn next_key(&mut self) -> Key { self.inner.next_key() }
    fn poll(&mut self, clock: &mut dyn Clock) -> mt_engine::io::Event { self.inner.poll(clock) }
    fn audio(&mut self, n: u16, hz: u32, levels: &[u8], _rest: u8) {
        let step = self.shared.borrow().next;
        let line = json!({"step": step, "sound": n, "hz": hz, "samples": levels.len(), "sha256": sha256_hex(levels)});
        if writeln!(self.out, "{line}").is_err() { eprintln!("--speaker: write failed"); std::process::exit(3) }
    }
}
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2];
    let mut h: [u32; 8] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let mut m = data.to_vec(); m.push(0x80); while m.len() % 64 != 56 { m.push(0) } m.extend((data.len() as u64 * 8).to_be_bytes());
    for block in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes([block[4*i], block[4*i+1], block[4*i+2], block[4*i+3]]) }
        for i in 16..64 { let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3); let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10); w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1) }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let t1 = hh.wrapping_add(e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25)).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
            let t2 = (a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22)).wrapping_add((a & b) ^ (a & c) ^ (b & c));
            hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) { *x = x.wrapping_add(y) }
    }
    h.iter().map(|x| format!("{x:08x}")).collect()
}
fn s_record(r: &Replay, sp: &Space, sample_tick: Option<u32>) -> Value {
    use space::state::*;
    let now = sample_tick.unwrap_or_else(|| r.steps.get(r.next).map_or(sp.last_ticks, |s| s.tick));
    let mut record = r.base("S", json!({"STARTUP":null,"CG":null,"T":null,"S":sp.s.dword(RNG)}), now);
    r.section(&mut record, "party", r.party(&sp.s.party));
    let mut s = schema::decode(&r.schema["properties"]["s"], sp.s.image());
    let name = |at: u16| sp.s.pointer_string(at).map(|b| b.iter().map(|&c| c as char).collect::<String>());
    s["system_names"] = Value::Array((0..8).map(|i| name(SYSTEM_NAMES + 4 * i).unwrap_or_default().into()).collect());
    s["world_names"] = Value::Array((0..28).map(|i| name(WORLD_NAMES + 4 * i).unwrap_or_default().into()).collect());
    s["ships"] = Value::Array((0..18).map(|i| json!({"x": sp.s.enc(i, 0), "y": sp.s.enc(i, 2), "active": sp.s.enc(i, 4), "dstx": sp.s.enc(i, 6), "dsty": sp.s.enc(i, 8)})).collect());
    r.section(&mut record, "s", s);
    record
}
struct SpaceTap(Shared);
impl SpaceObserver for SpaceTap {
    fn read(&mut self, _sp: &mut Space) -> Option<u32> {
        let mut r = self.0.borrow_mut();
        match r.s_reads.front_mut() {
            Some(front) => { let t = front.0; front.1 -= 1; let done = front.1 == 0; if done { r.s_reads.pop_front(); } r.s_tick = t; Some(t) }
            None => r.fail("align: measured S clock reads exhausted"),
        }
    }
    fn event(&mut self, sp: &mut Space, _e: SpaceEvent) {
        let mut r = self.0.borrow_mut();
        let raw = [sp.ev.reads, sp.ev.rands, sp.ev.minimap, sp.ev.cursor, sp.ev.view, sp.ev.copies, sp.ev.minimap_returns];
        let (at, minimap_returns) = offset(&raw, &r.s_base);
        r.s_last = std::array::from_fn(|i| raw[i] + r.s_base[i]);
        if std::env::var_os("MT1_LOCKSTEP_TRACE_S").is_some() { eprintln!("S {:?} at {:?} next {:?}", _e, at, r.s_events.front().map(|e| (e.k.clone(), e.step, e.at))) }
        if let Some(from) = std::env::var("MT1_LOCKSTEP_TRACE_S_FROM").ok().and_then(|v| v.parse::<u32>().ok()) {
            if at[0] >= from { eprintln!("S {:?} at {:?} last_ticks {} key {} snap {:x} dl7 {} dl9 {} fuel {} charge {} {}", _e, at, sp.last_ticks, sp.s.byte(space::state::KEY), sp.s.dword(0x188), sp.s.dword(0x347A), sp.s.dword(0x3482), sp.s.pbyte(0xFC), sp.s.word(0x1BF), sp.s.word(0x1C1)) }
        }
        while r.s_events.front().is_some_and(|ev| ev.reached(&at, minimap_returns)) {
            if let Some(ev) = r.s_events.front() {
                if ev.exact(&at, minimap_returns) {
                    match ev.phase.as_deref() {
                        Some("wait_key_test") if _e != SpaceEvent::WaitKeyTest => break,
                        Some("gun_key_test") if _e != SpaceEvent::GunKeyTest => break,
                        Some("aim_key_test") if _e != SpaceEvent::AimKeyTest => break,
                        Some("wait_key_test" | "gun_key_test" | "aim_key_test") | None => {},
                        Some(other) => { let msg = format!("invalid S transport phase {other}"); r.fail(&msg) }
                    }
                }
            }
            let ev = r.s_events.pop_front().unwrap();
            if !ev.exact(&at, minimap_returns) { let msg = format!("align: S event {} at {:?} minimap_returns {:?}, native at {:?} minimap_returns {} (step {})", ev.k, ev.at, ev.minimap_returns, at, minimap_returns, r.next); r.fail(&msg) }
            match ev.k.as_str() {
                "s" => {
                    if ev.step != r.next { let msg = format!("align: S sample for step {} while the native is at step {}", ev.step, r.next); r.fail(&msg) }
                    let step = r.steps[r.next].clone();
                    let bytes = r.pokes.iter().find(|p| p.step == r.next).map_or(vec![], |p| p.bytes.clone());
                    for b in &bytes {
                        let (sec, k) = r.poke_offset(&b.target);
                        match sec.as_str() {
                            "party" => sp.s.set_pbyte(k as u16, b.value),
                            "s" => sp.s.set_byte(k as u16, b.value),
                            "rng.S" => { let mut v = sp.s.dword(space::state::RNG); set_byte(&mut v, k, b.value); sp.s.set_dword(space::state::RNG, v) }
                            _ => r.fail(&format!("unmodeled poke {}", b.target)),
                        }
                    }
                    let record = s_record(&r, sp, ev.tick);
                    r.record(step, record, &bytes);
                    if r.next == r.steps.len() { r.finish("complete", None, "end of script", 0) }
                }
                "i" => match r.pending.pop_front() {
                    Some(scan) => sp.keyboard_isr(scan.code),
                    None => r.fail("align: measured INT 9 without a queued scan"),
                },
                "m" => match r.pending_mouse.pop_front() {
                    Some(m) => sp.mouse_callback(m),
                    None => r.fail("align: measured INT 33h callback without a queued click"),
                },
                other => r.fail(&format!("invalid S transport event {other}")),
            }
        }
    }
    fn srand(&mut self, _sp: &mut Space) -> Option<u16> { Some(self.0.borrow_mut().seed("S")) }
    fn stop(&mut self, sp: &mut Space, reason: &str) {
        let mut r = self.0.borrow_mut();
        let stop = r.next;
        let reason = format!("{reason} at S clock {}", sp.last_ticks);
        r.finish("stopped", Some(stop), &reason, 0)
    }
}
struct GroundTap(Shared);
impl mt1::lockstep::GroundObserver for GroundTap {
    fn poll(&mut self, g: &mut Ground) {
        let mut r = self.0.borrow_mut();
        if r.pending_tick < g.clock.ticks() {
            if let Some(scan) = r.pending.pop_front() {
                ground::input::keyboard_scan(g, scan.code);
                return;
            }
        }
        t_visit(&mut r, g);
        g.clock.wait_tick();
        g.sync_bios_clock();
        if r.pending_tick < g.clock.ticks() {
            if let Some(scan) = r.pending.pop_front() {
                ground::input::keyboard_scan(g, scan.code);
            }
        }
    }
    fn space(&mut self, _g: &mut Ground) -> Option<Box<dyn SpaceObserver>> {
        { let mut r = self.0.borrow_mut(); r.s_base = r.s_last; }
        Some(Box::new(SpaceTap(self.0.clone())))
    }
    fn space_returned(&mut self, g: &mut Ground, _space: Box<dyn SpaceObserver>) {
        let (target, seed) = { let mut r = self.0.borrow_mut(); let t = r.s_tick; (t, r.seed("T")) };
        if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("landing return: T counter {} -> {target}, seed {seed}", g.clock.ticks()) }
        while g.clock.ticks() < target { g.clock.wait_tick() }
        g.sync_bios_clock();
        g.rand.force = Some(seed);
    }
    fn checkpoint(&mut self, g: &mut Ground, name: &str) {
        if name == "launch" {
            let target = { let r = self.0.borrow(); r.seed_times.get(r.seed_next).filter(|s| s.prog == "S").map(|s| s.tick) };
            if let Some(target) = target { while g.clock.ticks() < target { self.poll(g) } }
            return;
        }
        let target = {
            let mut r = self.0.borrow_mut();
            if r.marks.front().is_some_and(|m| m.name == name) {
                r.marks.pop_front().map(|m| m.tick)
            } else {
                None
            }
        };
        if let Some(target) = target {
            if g.clock.ticks() > target {
                self.0.borrow_mut().fail(&format!(
                    "align: {name} reached {} after measured {target}",
                    g.clock.ticks()
                ))
            }
            while g.clock.ticks() < target {
                self.poll(g)
            }
        }
    }
    fn stop(&mut self, g: &mut Ground, reason: &str) {
        let mut r = self.0.borrow_mut();
        if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("lockstep_stop {reason} at {}", g.clock.ticks()) }
        let stop = r.next;
        let reason = format!("{reason} at tick {}", g.clock.ticks());
        r.finish("stopped", Some(stop), &reason, 0)
    }
}

pub fn run(
    dir: &Path,
    keys: &Path,
    seeds: &Path,
    pokes: Option<&Path>,
    state: &Path,
    transport: Option<&Path>,
    speaker: Option<&Path>,
    save_at: Option<usize>,
    save_fault: bool,
) -> Result<(), String> {
    let mut r = Replay::open(keys, seeds, pokes, state, transport)?;
    if save_at.is_some_and(|k| k >= r.steps.len()) { return Err("--save-at step outside the script".into()) }
    r.save_at = save_at; r.save_fault = save_fault;
    let speaker = speaker.map(|p| File::create(p).map_err(|e| format!("{}: {e}", p.display()))).transpose()?;
    let files = Dir(dir.into());
    let read = |n: &str| files.read(n).ok_or_else(|| format!("missing {n}"));
    let unpack = |n: &str| {
        exepack::unpack(&read(n)?)
            .map(|p| p.exe)
            .map_err(|e| format!("{n}: {e:?}"))
    };
    let layout = |n: &str| ly::decode(&read(n)?).map_err(|e| format!("{n}: {e:?}"));
    let picture = |n: &str| pic::decode(Variant::Mt1, &read(n)?).map_err(|e| format!("{n}: {e:?}"));
    let mut title = Title::new(
        &unpack("STARTUP.EXE")?,
        layout("TITLE.LY")?,
        layout("FONT.LY")?,
        picture("TIT.PIC")?,
        picture("ZOD.PIC")?,
    )
    .ok_or("title tables")?;
    let mut clock = Counter::default();
    let _setup_seed = r.seed("STARTUP");
    let title_tick = r
        .seed_tick()
        .or_else(|| {
            r.seeds
                .get(r.seed_next)
                .map(|s| r.steps[s.step.saturating_sub(1)].tick + 1)
        })
        .unwrap_or(200001);
    while clock.ticks() < title_tick {
        while let Some((step, bytes)) = r.due(clock.ticks()) {
            if !bytes.is_empty() {
                r.fail("poke during unmodeled STARTUP setup")
            }
            let record = r.base("STARTUP", json!({}), clock.now());
            r.record(step, record, &bytes);
        }
        let answers: Vec<u8> = r.pending.iter().filter(|s| s.code < 128 && s.ascii != 0).map(|s| s.ascii).collect();
        r.setup.extend(answers);
        r.pending.clear();
        clock.wait_tick();
    }
    let mut rand = TcRand {
        seed: r.seed("STARTUP") as u32,
    };
    title.star_field(&mut || rand.next());
    let t_tick = r.seed_tick();
    loop {
        while let Some((step, bytes)) = r.due(clock.ticks()) {
            if !bytes.is_empty() {
                r.fail("poke into STARTUP is unmodeled")
            }
            let record = r.base(
                "STARTUP",
                json!({"STARTUP":rand.seed,"CG":null,"T":null,"S":null}),
                clock.now(),
            );
            r.record(step, record, &bytes);
        }
        if t_tick.is_some_and(|t| clock.ticks() >= t) {
            break;
        }
        let key = r.pending.iter().any(|s| s.code < 128);
        let answers: Vec<u8> = r.pending.iter().filter(|s| s.code < 128 && s.ascii != 0).map(|s| s.ascii).collect();
        r.setup.extend(answers);
        r.pending.clear();
        if t_tick.is_none() && key {
            clock.wait_tick();
            break;
        }
        clock.wait_tick();
    }
    r.pending.clear();
    r.t_seed = Some(r.seed("T") as u32);
    let mut s = Screens::new(
        unpack("CG.DAT")?,
        layout("FONT.LY")?,
        layout("CG.LY")?,
        Box::new(Scripted::new([])),
        Box::new(clock),
        Box::new(Dir(dir.into())),
    )
    .ok_or("CG tables")?;
    s.menu_mouse = (160, 100);
    let mut overlay = Overlay::new(Some(dir));
    overlay.hide("UCHARS.DAT");
    s.overlay = Some(overlay);
    s.rng.srand(r.seed("CG"));
    s.rng.srand(r.seed("CG"));
    let shared = Rc::new(RefCell::new(r));
    s.lockstep = Some(Box::new(CgTap(shared.clone())));
    let (exit, selected) = mt1::screens::cg::run(&mut s);
    let block = match exit {
        mt1::chargen::pool::Exit::Quick => read("DEFAULT.SAV")?,
        mt1::chargen::pool::Exit::Play => {
            ground::party::block(&shared.borrow().party, &selected, &s.ds, &mut s.rng)
                .ok_or("party conversion")?
        }
        other => {
            let mut r = shared.borrow_mut();
            let stop = r.next;
            r.finish("stopped", Some(stop), &format!("CG {other:?}"), 0)
        }
    };
    let seed = shared.borrow().t_seed.unwrap();
    let input: Box<dyn mt_engine::io::Input> = match speaker {
        Some(out) => Box::new(SpeakerLog { shared: shared.clone(), inner: Scripted::new([]), out }),
        None => Box::new(Scripted::new([])),
    };
    let mut g = Ground::new(
        &read("T.DAT")?,
        block,
        input,
        s.clock,
        Box::new(Dir(dir.into())),
    )
    .ok_or("T tables")?;
    g.rand.force = Some(seed as u16);
    g.sound_option = match shared.borrow().setup.get(2) { Some(b'1') => b'A', Some(b'2') => b'B', _ => b'C' };
    g.mouse_driver = true;
    ground::start(&mut g);
    g.lockstep = Some(Box::new(GroundTap(shared.clone())));
    ground::run(&mut g);
    let mut r = shared.borrow_mut();
    let stop = r.next;
    r.finish("stopped", Some(stop), "ground returned", 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn second_space_instance_offsets_all_seven_counters() {
        let last = [6745386, 1465, 1323, 15579045, 10, 6809, 1323];
        assert_eq!(offset(&[0; 7], &last), ([6745386, 1465, 1323, 15579045, 10, 6809], 1323), "a fresh Space starts at the previous position");
        assert_eq!(offset(&[2, 0, 1, 3, 0, 4, 1], &last), ([6745388, 1465, 1324, 15579048, 10, 6813], 1324));
        assert_eq!(offset(&[2, 0, 1, 3, 0, 4, 1], &[0; 7]), ([2, 0, 1, 3, 0, 4], 1), "the first instance is unshifted");
    }
    #[test]
    fn minimap_return_distinguishes_same_copy_position() {
        let at = [6326771, 1285, 1316, 15160400, 4, 6662];
        let event = SEvent { k: "s".into(), step: 672, at, minimap_returns: Some(1316), phase: None, tick: None };
        assert!(!event.reached(&at, 1315), "icon pass still pending");
        assert!(event.reached(&at, 1316));
        assert!(event.exact(&at, 1316));
        assert!(!event.exact(&at, 1317), "an extra return must fail alignment");
        let mut extra_copy = at; extra_copy[5] += 1;
        assert!(event.reached(&extra_copy, 1316));
        assert!(!event.exact(&extra_copy, 1316), "copy mismatches remain strict");
        let old: SEvent = serde_json::from_value(json!({"k":"s", "step":672, "at":at})).unwrap();
        assert!(old.exact(&at, 1315), "legacy transport remains readable");
    }
    #[test]
    fn grammar_and_scan_order() {
        let s = parse("# comment\n10 SPACE\n0 {Up+}\n2 {Up-}\n0 {s4C}\n0 A\n").unwrap();
        assert_eq!((s[0].tick, s[0].key.as_str()), (9, " "));
        assert_eq!(
            s.iter()
                .map(|s| s.scans.iter().map(|s| s.code).collect::<Vec<_>>())
                .collect::<Vec<_>>(),
            [
                vec![0x39, 0xb9],
                vec![0x48],
                vec![0xc8],
                vec![0x4c, 0xcc],
                vec![0x1e, 0x9e]
            ]
        );
        assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        for bad in [
            "-1 a",
            "1 {Bogus}",
            "1 {sFF}",
            "200001 a",
            "0 !w FF 0",
            "0 \\",
            "",
        ] {
            assert!(parse(bad).is_err(), "{bad}")
        }
    }
    #[test]
    fn mouse_steps_preserve_coordinates_and_release_event() {
        let steps = parse("10 !m 303 147 L\n0 !m 0 199 R\n").unwrap();
        assert_eq!(steps[0].tick, 9);
        assert!(steps.iter().all(|s| s.scans.is_empty()));
        assert_eq!(click(&steps[0].key), Some(MouseEvent { event: 4, buttons: 0, col: 303, row: 147 }));
        assert_eq!(click(&steps[1].key), Some(MouseEvent { event: 0x10, buttons: 0, col: 0, row: 199 }));
        for bad in ["0 !m -1 0 L", "0 !m 320 0 L", "0 !m 0 200 R", "0 !m 0 -1 R", "0 !m 0 0 M", "0 !m 0 0", "0 !m 0 0 L extra"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
