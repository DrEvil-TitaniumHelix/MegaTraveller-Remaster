use crate::ground::{render, state::{Far, Mem, PARTY_PTR}, walk, Ground};
use mt_engine::{io::{Files, Input}, present::{fb::Fb, palette::Dac}, sim::tick::Clock};
use std::{collections::BTreeMap, path::PathBuf};

pub const MAGIC: &[u8; 8] = b"MT1SAVE\0";
pub const VERSION: u16 = 1;
pub const FILE: &str = "game1.mtsav";
const MAX_FILE: usize = 16 << 20;
const MAX_SEGS: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode { Ground = 0, Starport = 1 }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Saved {
    pub build: String, pub mode: Mode,
    pub ds: Vec<u8>, pub segs: BTreeMap<u16, Vec<u8>>, pub bios_ticks: [u8; 4],
    pub seed: u32, pub ticks: u32, pub now: u32,
    pub landing: bool, pub sound_option: u8, pub speaker_refresh: u8,
    pub regs: [u8; 16], pub dac: Dac,
    pub loop_vars: Option<crate::ground::loop_vars::LoopVars>,
    pub squad: Option<crate::play::anchors::Squad>,
    pub play: Option<crate::play::anchors::Meta>,
}

fn fnv(b: &[u8]) -> u64 { b.iter().fold(0xcbf29ce484222325u64, |h, &x| (h ^ x as u64).wrapping_mul(0x100000001b3)) }
fn section(out: &mut Vec<u8>, tag: &[u8; 4], body: &[u8]) { out.extend_from_slice(tag); out.extend_from_slice(&(body.len() as u32).to_le_bytes()); out.extend_from_slice(body) }
struct Reader<'a> { b: &'a [u8], at: usize }
impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self.at.checked_add(n).filter(|&e| e <= self.b.len()).ok_or_else(|| format!("save: truncated at byte {}", self.at))?;
        let s = &self.b[self.at..end]; self.at = end; Ok(s)
    }
    fn u8(&mut self) -> Result<u8, String> { Ok(self.take(1)?[0]) }
    fn u16(&mut self) -> Result<u16, String> { Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap())) }
    fn u32(&mut self) -> Result<u32, String> { Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap())) }
    fn u64(&mut self) -> Result<u64, String> { Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap())) }
}
fn le32(b: &[u8]) -> u32 { u32::from_le_bytes(b.try_into().unwrap()) }

impl Saved {
    pub fn capture(g: &mut Ground, mode: Mode, build: &str) -> Saved {
        Saved { build: build.into(), mode, ds: g.mem.ds.clone(), segs: g.mem.segs.clone(), bios_ticks: g.mem.bios_ticks, seed: g.rand.lcg.seed,
            ticks: g.clock.ticks(), now: g.clock.now(), landing: g.landing, sound_option: g.sound_option, speaker_refresh: g.speaker_refresh,
            regs: g.fb.regs, dac: g.dac, loop_vars: Some(g.loop_vars.clone()), squad: None, play: None }
    }
    pub fn encode(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(0x10400 + self.segs.values().map(|s| s.len() + 10).sum::<usize>());
        b.extend_from_slice(MAGIC); b.extend_from_slice(&VERSION.to_le_bytes());
        let build = &self.build.as_bytes()[..self.build.len().min(255)]; b.push(build.len() as u8); b.extend_from_slice(build);
        section(&mut b, b"MODE", &[self.mode as u8]);
        section(&mut b, b"DS  ", &self.ds);
        for (seg, bytes) in &self.segs { let mut body = seg.to_le_bytes().to_vec(); body.extend_from_slice(bytes); section(&mut b, b"SEG ", &body) }
        section(&mut b, b"RNG ", &self.seed.to_le_bytes());
        let mut clk = self.ticks.to_le_bytes().to_vec(); clk.extend_from_slice(&self.now.to_le_bytes()); clk.extend_from_slice(&self.bios_ticks);
        section(&mut b, b"CLK ", &clk);
        section(&mut b, b"GRND", &[self.landing as u8, self.sound_option, self.speaker_refresh]);
        section(&mut b, b"REGS", &self.regs);
        section(&mut b, b"DAC ", &self.dac.concat());
        if let Some(vars) = &self.loop_vars { section(&mut b, b"LOOP", &vars.encode()); }
        if let Some(squad) = &self.squad { section(&mut b, b"SQD ", &squad.encode()); }
        if let Some(play) = &self.play { section(&mut b, b"PLAY", &play.encode()); }
        let sum = fnv(&b); b.extend_from_slice(b"END "); b.extend_from_slice(&sum.to_le_bytes()); b
    }
    pub fn decode(bytes: &[u8]) -> Result<Saved, String> {
        if bytes.len() > MAX_FILE { return Err(format!("save: {} bytes exceeds the {} MiB limit", bytes.len(), MAX_FILE >> 20)) }
        let mut r = Reader { b: bytes, at: 0 };
        if r.take(8)? != MAGIC { return Err("save: not a MegaTraveller 1 Plus save (bad magic)".into()) }
        let version = r.u16()?;
        if version > VERSION { return Err(format!("save: format {version} is newer than this build reads (format {VERSION})")) }
        let n = r.u8()? as usize; let build = String::from_utf8_lossy(r.take(n)?).into_owned();
        let (mut mode, mut ds, mut segs, mut rng, mut clk, mut grnd, mut regs, mut dac) = (None, None, BTreeMap::new(), None, None, None, None, None);
        let mut loop_vars = None;
        let (mut squad, mut play) = (None, None);
        loop {
            let tag: [u8; 4] = r.take(4)?.try_into().unwrap();
            if &tag == b"END " {
                let body_end = r.at - 4; let sum = r.u64()?;
                if r.at != bytes.len() { return Err("save: bytes after the end record".into()) }
                if fnv(&bytes[..body_end]) != sum { return Err("save: checksum mismatch (the file is damaged)".into()) }
                break
            }
            let len = r.u32()? as usize; let body = r.take(len)?;
            match &tag {
                b"SQD " => {
                    if squad.is_some() { return Err("save: SQD: repeated section".into()) }
                    squad = Some(crate::play::anchors::Squad::decode(body)?);
                }
                b"PLAY" => {
                    if play.is_some() { return Err("save: PLAY: repeated section".into()) }
                    play = Some(crate::play::anchors::Meta::decode(body)?);
                }
                b"LOOP" => {
                    if loop_vars.is_some() { return Err("save: LOOP: repeated section".into()) }
                    loop_vars = Some(crate::ground::loop_vars::LoopVars::decode(body)?);
                }
                b"MODE" => mode = Some(match body { [0] => Mode::Ground, [1] => Mode::Starport, _ => return Err("save: MODE: unknown mode".into()) }),
                b"DS  " => { if body.len() != 0x10000 { return Err(format!("save: DS: {} bytes, expected 65536", body.len())) } ds = Some(body.to_vec()) }
                b"SEG " => {
                    if body.len() < 2 || body.len() > 2 + 0x10000 { return Err(format!("save: SEG: bad length {}", body.len())) }
                    let seg = u16::from_le_bytes([body[0], body[1]]);
                    if segs.insert(seg, body[2..].to_vec()).is_some() { return Err(format!("save: SEG {seg:04X} repeated")) }
                    if segs.len() > MAX_SEGS { return Err(format!("save: more than {MAX_SEGS} segments")) }
                }
                b"RNG " => { if body.len() != 4 { return Err("save: RNG: bad length".into()) } rng = Some(le32(body)) }
                b"CLK " => { if body.len() != 12 { return Err("save: CLK: bad length".into()) } clk = Some((le32(&body[..4]), le32(&body[4..8]), body[8..12].try_into().unwrap())) }
                b"GRND" => { if body.len() != 3 { return Err("save: GRND: bad length".into()) } grnd = Some((body[0] != 0, body[1], body[2])) }
                b"REGS" => regs = Some(body.try_into().map_err(|_| "save: REGS: bad length".to_string())?),
                b"DAC " => {
                    if body.len() != 768 { return Err("save: DAC: bad length".into()) }
                    let mut d = [[0u8; 3]; 256]; for (i, c) in body.chunks_exact(3).enumerate() { d[i].copy_from_slice(c) } dac = Some(d)
                }
                _ => {}
            }
        }
        let miss = |what: &str| format!("save: missing {what} section");
        let (ticks, now, bios_ticks) = clk.ok_or_else(|| miss("CLK"))?;
        let (landing, sound_option, speaker_refresh) = grnd.ok_or_else(|| miss("GRND"))?;
        let s = Saved { build, mode: mode.ok_or_else(|| miss("MODE"))?, ds: ds.ok_or_else(|| miss("DS"))?, segs, bios_ticks, seed: rng.ok_or_else(|| miss("RNG"))?,
            ticks, now, landing, sound_option, speaker_refresh, regs: regs.ok_or_else(|| miss("REGS"))?, dac: dac.ok_or_else(|| miss("DAC"))?, loop_vars, squad, play };
        s.check()?; Ok(s)
    }
    fn check(&self) -> Result<(), String> {
        let p = PARTY_PTR as usize;
        let ptr = Far::new(u16::from_le_bytes([self.ds[p], self.ds[p + 1]]), u16::from_le_bytes([self.ds[p + 2], self.ds[p + 3]]));
        let party = self.segs.get(&ptr.seg).ok_or("save: the party block's segment is missing")?;
        if party.len() < ptr.off as usize + Mem::PARTY_SIZE { return Err("save: the party block is short".into()) }
        Ok(())
    }
    pub fn install(self, g: &mut Ground) {
        g.mem.ds = self.ds; g.mem.segs = self.segs; g.mem.bios_ticks = self.bios_ticks;
        let shift = self.now.wrapping_sub(g.clock.now());
        if let Some(c) = g.squad.as_mut().and_then(|s| s.p1.clock.as_mut()) { *c = c.wrapping_add(shift) }
        g.rand.lcg.seed = self.seed; g.clock.set(self.now);
        g.landing = self.landing; g.sound_option = self.sound_option; g.speaker_refresh = self.speaker_refresh;
        g.loop_vars = self.loop_vars.unwrap_or_default();
        g.fb = Fb::new(); g.fb.regs = self.regs; g.dac = self.dac;
        g.mouse_event = None; g.launch_pending = false; g.game_over = false; g.scenario_boot = false; g.boot_calibrating = false; g.screen = "t";
    }
}

#[derive(Clone, Debug)]
pub struct Slot { pub dir: PathBuf, pub build: &'static str }
impl Slot { pub fn path(&self) -> PathBuf { self.dir.join(FILE) } }

pub fn available(g:&Ground)->bool {g.save_slot.is_some() || g.input.live_save_location().is_some()}
fn location(g:&Ground)->Option<(PathBuf,&'static str)> {
    g.save_slot.as_ref().map(|s|(s.dir.clone(),s.build))
        .or_else(||g.input.live_save_location().map(|(dir,build)|(dir.to_path_buf(),build)))
}

pub const GAMES: u8 = 8;
pub const AUTOS: u8 = 3;
pub const QUICK: &str = "quick.mtsav";
pub static UTC_OFFSET: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);
pub const SAVE_TITLE: &str = "Save to which position?";
pub const RESTORE_TITLE: &str = "Restore which game?";
pub fn position_list(title: &str) -> bool { title == SAVE_TITLE || title == RESTORE_TITLE }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pos { Game(u8), Quick,  Auto(u8) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Info { pub place: String, pub day: u16, pub credits: u32 }
#[derive(Clone, Debug)]
pub struct Entry { pub pos: Pos, pub file: String,  pub saved: Option<(std::time::SystemTime, Result<Info, String>)> }
impl Saved {
    pub fn info(&self) -> Info {
        let mem = Mem { ds: self.ds.clone(), segs: self.segs.clone(), trace: Default::default(), bios_ticks: self.bios_ticks };
        let name = |off: u16| { let i = mem.u8(off); if i == 255 { String::new() } else { let n = mem.cstr(mem.pointer(0x15E1 + i as u16 * 4)); n.chars().take(16).enumerate().map(|(k, c)| if k == 0 { c.to_ascii_uppercase() } else { c }).collect() } };
        let (system, world) = (name(0x5E1F), name(0x5E20));
        let mut place = if system == world { system } else { [system, world].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" / ") };
        if place.is_empty() { place = "In flight".into() }
        if self.mode == Mode::Starport { place += " starport" }
        let party = mem.pointer(PARTY_PTR);
        let credits = (0..5u16).filter(|&i| mem.party_u8(0x127 + i * 256) != 0).fold(mem.read(party.add(0x88), 4), |n, i| n.wrapping_add(mem.read(party.add(0x1FE + i as i32 * 256), 4)));
        Info { place, day: mem.party_u16(0xF84), credits }
    }
}
fn stamp(t: std::time::SystemTime) -> String {
    let secs = t.duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64) + UTC_OFFSET.load(std::sync::atomic::Ordering::Relaxed) as i64;
    let (z, rem) = (secs.div_euclid(86400) + 719468, secs.rem_euclid(86400));
    let doe = z.rem_euclid(146097); let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); let mp = (5 * doy + 2) / 153;
    let month = ["Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec", "Jan", "Feb"][mp as usize];
    format!("{} {month} {:02}:{:02}", doy - (153 * mp + 2) / 5 + 1, rem / 3600, rem % 3600 / 60)
}
impl Entry {
    pub fn name(&self) -> String { match self.pos { Pos::Game(n) => n.to_string(), Pos::Quick => "Quick".into(), Pos::Auto(n) => format!("Autosave {n}") } }
    pub fn label(&self) -> String { match self.pos { Pos::Game(n) => format!("Game {n}"), _ => self.name() } }
    pub fn used(&self) -> bool { self.saved.is_some() }
    pub fn place(&self) -> &str { match &self.saved { None => "Empty", Some((_, Ok(i))) => &i.place, Some((_, Err(_))) => "(damaged file)" } }
}
pub fn rows(list: &[Entry]) -> Vec<[String; 3]> {
    let info = |e: &Entry| match &e.saved { Some((t, Ok(i))) => Some((i.day.to_string(), commas(i.credits), stamp(*t))), _ => None };
    let (d, c) = list.iter().filter_map(info).fold((1, 1), |(d, c), i| (d.max(i.0.len()), c.max(i.1.len())));
    list.iter().map(|e| [e.label(), e.place().into(), match (&e.saved, info(e)) {
        (_, Some((day, cr, at))) => format!("Day {day:<d$}  Cr {cr:<c$}  {at:>12}"),
        (Some((t, _)), None) => format!("{:>w$}", stamp(*t), w = d + c + 23),
        _ => String::new() }]).collect()
}
fn commas(n: u32) -> String { let d = n.to_string(); d.bytes().enumerate().flat_map(|(i, c)| (i > 0 && (d.len() - i) % 3 == 0).then_some(',').into_iter().chain([c as char])).collect() }
fn read_path(path: &std::path::Path) -> Result<Saved, String> {
    let len = std::fs::metadata(path).map_err(|e| format!("restore: {}: {e}", path.display()))?.len();
    if len > MAX_FILE as u64 { return Err(format!("restore: {}: {len} bytes exceeds the {} MiB limit", path.display(), MAX_FILE >> 20)) }
    Saved::decode(&std::fs::read(path).map_err(|e| format!("restore: {}: {e}", path.display()))?)
}
pub fn entries(dir: &std::path::Path) -> Vec<Entry> {
    let load = |pos: Pos, file: String| {
        let path = dir.join(&file);
        let saved = std::fs::metadata(&path).ok().filter(|m| m.is_file()).map(|m| (m.modified().unwrap_or(std::time::UNIX_EPOCH), read_path(&path).map(|s| s.info())));
        Entry { pos, file, saved }
    };
    let mut v: Vec<Entry> = (1..=GAMES).map(|n| load(Pos::Game(n), format!("game{n}.mtsav"))).collect();
    v.push(load(Pos::Quick, QUICK.into()));
    let mut autos: Vec<Entry> = (1..=AUTOS).map(|n| load(Pos::Auto(n), format!("auto{n}.mtsav"))).collect();
    autos.sort_by_key(|e| std::cmp::Reverse(e.saved.as_ref().map(|s| s.0)));
    for (i, e) in autos.iter_mut().enumerate() { e.pos = Pos::Auto(i as u8 + 1) }
    v.extend(autos); v
}
pub fn list(g: &Ground) -> Vec<Entry> { location(g).map_or(vec![], |(dir, _)| entries(&dir)) }

pub fn continue_old<H: crate::ui::CgUi>(ui: &mut H, dir: &std::path::Path) -> Option<Saved> {
    use crate::chargen::tables::{menu_args, strs};
    let banner = |ui: &mut H, s: &str| { ui.fill_rect(0, 0, 49, 320, 2, 3); ui.set_text_color(0); ui.draw_centered(s, 20); ui.set_text_color(2) };
    let used: Vec<Entry> = entries(dir).into_iter().filter(Entry::used).collect();
    let pick = match used.len() {
        0 => { banner(ui, "No saved game."); let _ = ui.delay(36); return None }
        1 => 0,
        _ => {
            let rows = rows(&used); let name = rows.iter().map(|r| r[0].len()).max().unwrap_or(1);
            let place = 53usize.saturating_sub(name + 2 + rows.iter().map(|r| r[2].len()).max().unwrap_or(0));
            let lines: Vec<String> = rows.iter().map(|r| format!("{:<name$} {:<place$.place$} {}", r[0], r[1], r[2])).collect();
            banner(ui, "Continue which game?"); ui.fill_rect(49, 0, 151, 320, 0, 3); ui.set_text_color(2);
            let i = ui.menu(&menu_args(&strs(&lines), &[4, 55, 8, 0, 2, 0, 1, 0, 3], -1));
            ui.layout(12, 0, 0);
            if i < 0 { return None } i as usize
        }
    };
    read_path(&dir.join(&used[pick].file)).map_err(|e| { eprintln!("{e}"); banner(ui, "That saved game cannot be read."); let _ = ui.delay(36); }).ok()
}

fn stage(dir: &std::path::Path, file: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    let tmp = dir.join(format!("{file}.tmp")); std::fs::create_dir_all(dir)?; std::fs::write(&tmp, bytes)?; Ok(tmp)
}
fn put(dir: &std::path::Path, file: &str, bytes: &[u8]) -> Result<(), String> {
    let path = dir.join(file);
    stage(dir, file, bytes).and_then(|tmp| std::fs::rename(tmp, &path)).map_err(|e| format!("save: {}: {e}", path.display()))
}
pub fn write(g: &mut Ground, mode: Mode) -> Result<(), String> { write_file(g, mode, FILE) }
pub fn write_file(g: &mut Ground, mode: Mode, file: &str) -> Result<(), String> {
    if let Some(refused) = crate::ground::squad::save_refusal(g) { return Err(refused.into()) }
    let (dir, build) = location(g).ok_or("save: no save location")?;
    put(&dir, file, &Saved::capture(g, mode, build).encode())
}
pub fn read(g: &Ground) -> Result<Saved, String> { read_file(g, FILE) }
pub fn read_file(g: &Ground, file: &str) -> Result<Saved, String> { read_path(&location(g).ok_or("restore: no save location")?.0.join(file)) }
pub fn ground(saved: Saved, input: Box<dyn Input>, clock: Box<dyn Clock>, files: Box<dyn Files>) -> Ground {
    let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], input, clock, files);
    saved.clone().install(&mut g); g.restore_pending = Some(saved); g
}
pub fn ground_exact(mut saved: Saved, input: Box<dyn Input>, clock: Box<dyn Clock>, files: Box<dyn Files>) -> Result<Ground, String> {
    if saved.mode != Mode::Ground || !saved.loop_vars.as_ref().is_some_and(|v| v.initialized) {
        return Err("play: exact resume requires a ground anchor with initialized LOOP state".into())
    }
    let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], input, clock, files);
    let (squad, play) = (saved.squad.take(), saved.play.take());
    saved.install(&mut g);
    if let Some(squad) = squad {
        g.squad = squad.state().clone(); g.scene = g.squad.as_ref().map(|_| crate::ui::SceneHook::default());
    }
    if let Some(play) = play {
        g.bios_key = play.bios_key; g.mouse_driver = play.mouse_driver; g.balance = play.balance;
        g.play_resume=play.resume; g.fb.planes=play.planes;
        mt_engine::present::textlog::restore(&g.fb,&play.text,&play.input.frame)?;
        g.scenario_boot=play.scenario_boot;
        g.input.resume_play_checkpoint(&play.input)?;
        if !play.input.frame.is_empty() {g.input.present(&play.input.frame);}
    }
    Ok(g)
}
pub fn restored(g: &mut Ground) {
    let Some(s) = g.squad.as_mut() else { return };
    s.encounter = false; s.round = 0;
    let p = &mut s.p1; p.encounter = None; p.acting = None; p.turns = Default::default(); p.strip.clear(); p.explore_ticks = 0; p.walked = 0;
    for n in p.npcs.values_mut() { n.engaged = false; n.acted = false }
}
pub const KEY_QUICK_SAVE: u8 = 0x40;
pub const KEY_QUICK_LOAD: u8 = 0x43;
pub const KEY_SAVE: u8 = 0x5B;
pub const KEY_LOAD: u8 = 0x5C;
const AUTO_DUE: u8 = 0xFF;
pub fn is_key(k: u8) -> bool { matches!(k, KEY_QUICK_SAVE | KEY_QUICK_LOAD | KEY_SAVE | KEY_LOAD) }
pub static AUTOSAVE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
thread_local! { static DUE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; }
pub fn window(g: &Ground) -> bool { g.scene.is_some() && !g.harness && g.lockstep.is_none() && !g.input.data_mode() }
fn player(g: &Ground) -> bool { window(g) && available(g) }
pub fn key(g: &mut Ground, k: u8) -> bool {
    if !is_key(k) || !player(g) || (k == KEY_QUICK_LOAD && g.mem.debug() != 0) { return false }
    DUE.set(k); true
}
pub fn landed(g: &Ground) { if player(g) { DUE.set(AUTO_DUE) } }
pub(crate) fn note(g: &mut Ground, line: impl Into<String>) {
    let line = line.into();
    if g.squad.is_some() { crate::ground::orders::log(g, line) } else { render::message(g, &line.bytes().collect::<Vec<u8>>()) }
    g.mem.set_redraw_pending(1);
}
pub fn autosave(g: &mut Ground, mode: Mode) {
    if !AUTOSAVE.load(std::sync::atomic::Ordering::Relaxed) || !player(g) || crate::ground::squad::save_refusal(g).is_some() { return }
    let Some(oldest) = list(g).into_iter().filter(|e| matches!(e.pos, Pos::Auto(_))).last() else { return };
    match write_file(g, mode, &oldest.file) { Ok(()) => if g.squad.is_some() { note(g, "Autosaved") }, Err(e) => eprintln!("autosave: {e}") }
}
fn serve(g: &mut Ground, due: u8) {
    const FIGHT: &str = "The game cannot be saved in a fight.";
    let fight = crate::ground::squad::save_refusal(g).is_some();
    match due {
        AUTO_DUE => autosave(g, Mode::Ground),
        KEY_QUICK_SAVE | KEY_SAVE if fight => note(g, FIGHT),
        KEY_QUICK_SAVE => { let r = write_file(g, Mode::Ground, QUICK); note(g, r.err().unwrap_or_else(|| "Quick-saved".into())) }
        KEY_QUICK_LOAD if !list(g).iter().any(|e| e.pos == Pos::Quick && e.used()) => note(g, "No quick-save yet."),
        KEY_QUICK_LOAD => if crate::ground::starport::ask_over_ground(g, "Load the quick-save?") {
            match read_file(g, QUICK) { Ok(s) => g.restore_pending = Some(s), Err(e) => note(g, e) }
        },
        KEY_SAVE => crate::ground::starport::save_over_ground(g, false),
        KEY_LOAD => crate::ground::starport::save_over_ground(g, true),
        _ => {}
    }
}
pub fn resume(g: &mut Ground) -> bool {
    let due = DUE.replace(0); if due != 0 && player(g) { serve(g, due) }
    let Some(s) = g.restore_pending.take() else { return false };
    let mode = s.mode; s.install(g); restored(g);
    match mode { Mode::Ground => render::main_screen(g, 0), Mode::Starport => walk::resume_starport(g) }
    true
}

#[cfg(test)]
mod t {
    use super::*;
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    fn synthetic() -> Ground {
        let mut ds = vec![0u8; 0x10000]; for (i, b) in ds.iter_mut().enumerate() { *b = (i * 7 % 251) as u8 }
        let party: Vec<u8> = (0..Mem::PARTY_SIZE).map(|i| (i % 13) as u8).collect();
        let mut g = Ground::from_ds(ds, party, Box::new(Scripted::new([])), Box::new(Counter::at(1234)), Box::new(Overlay::new(None)));
        g.mem.alloc(0x9100, 0x2000); g.mem.write(Far::new(0x1223, 0x9100), 1, 1); g.mem.alloc(0x8800, 1024);
        g.rand.lcg.seed = 0xDEADBEEF; g.landing = true; g.sound_option = b'C'; g.speaker_refresh = 0x10; g.fb.regs[3] = 9; g.dac[7] = [1, 2, 3];
        g.clock.set(1240); g.sync_bios_clock(); g
    }
    #[test]
    fn identity_capture_restore_capture() {
        let mut a = synthetic();
        a.loop_vars = crate::ground::loop_vars::LoopVars { initialized: true, second: mt_engine::sim::sched::Job::at(1200, 18),
            air: mt_engine::sim::sched::Job::at(1199, 108), npc: mt_engine::sim::sched::Job::at(1205, 18),
            mouse_state: 2, now: 1240, pass_tick: 1239, last: 1238, changed: -1, short: 1248, first_idle: true };
        let s = Saved::capture(&mut a, Mode::Ground, "abc123");
        let bytes = s.encode(); let d = Saved::decode(&bytes).unwrap(); assert_eq!(d, s);
        let mut b = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::at(s.ticks)), Box::new(Overlay::new(None)));
        d.install(&mut b);
        assert_eq!(Saved::capture(&mut b, Mode::Ground, "abc123").encode(), bytes);
        assert_eq!((b.mem.ds, b.mem.segs, b.rand.lcg.seed, b.clock.now(), b.clock.ticks()), (a.mem.ds, a.mem.segs, 0xDEADBEEF, 1240, 1234));
        assert_eq!((b.landing, b.sound_option, b.speaker_refresh, b.fb.regs[3], b.dac[7]), (true, b'C', 0x10, 9, [1, 2, 3]));
        assert_eq!(b.loop_vars, a.loop_vars);
    }
    #[test]
    fn old_saves_and_malformed_loop_sections() {
        let mut s = Saved::capture(&mut synthetic(), Mode::Ground, "old"); s.loop_vars = None;
        let decoded = Saved::decode(&s.encode()).unwrap(); assert_eq!(decoded, s);
        let mut g = synthetic(); g.loop_vars.initialized = true; decoded.install(&mut g); assert_eq!(g.loop_vars, Default::default());
        assert!(crate::ground::main_loop::resume_exact(&mut g).is_err());
        assert!(ground_exact(s.clone(), Box::new(Scripted::new([])), Box::new(Counter::at(s.ticks)), Box::new(Overlay::new(None))).is_err());
        for payload in [vec![], vec![0; 32], vec![0; 34], { let mut b = vec![0; 33]; b[0] = 2; b }, { let mut b = vec![0; 33]; b[2] = 2; b }] {
            let mut bytes = s.encode(); bytes.truncate(bytes.len() - 12); section(&mut bytes, b"LOOP", &payload);
            let sum = fnv(&bytes); bytes.extend_from_slice(b"END "); bytes.extend_from_slice(&sum.to_le_bytes());
            assert!(Saved::decode(&bytes).unwrap_err().contains("LOOP"));
        }
        let payload = crate::ground::loop_vars::LoopVars::default().encode();
        let mut bytes = s.encode(); bytes.truncate(bytes.len() - 12);
        section(&mut bytes, b"LOOP", &payload); section(&mut bytes, b"LOOP", &payload);
        let sum = fnv(&bytes); bytes.extend_from_slice(b"END "); bytes.extend_from_slice(&sum.to_le_bytes());
        assert!(Saved::decode(&bytes).unwrap_err().contains("repeated"));
    }
    #[test]
    fn hostile_input_errs_never_panics() {
        let mut g = synthetic(); let good = Saved::capture(&mut g, Mode::Starport, "x").encode();
        assert!(Saved::decode(&good).is_ok());
        assert!(Saved::decode(&[]).is_err());
        for n in (0..good.len()).step_by(61).chain(good.len() - 20..good.len()) { assert!(Saved::decode(&good[..n]).is_err(), "prefix {n}") }
        let mut bad = good.clone(); bad[0] = b'X'; assert!(Saved::decode(&bad).unwrap_err().contains("magic"));
        let mut newer = good.clone(); newer[8..10].copy_from_slice(&(VERSION + 1).to_le_bytes()); assert!(Saved::decode(&newer).unwrap_err().contains("newer"));
        for i in (0..good.len()).step_by(97).chain(0..40) { let mut f = good.clone(); f[i] ^= 1 << (i % 8); assert!(Saved::decode(&f).is_err(), "flip {i}") }
        let mut extra = good.clone(); extra.push(0); assert!(Saved::decode(&extra).is_err());
        assert!(Saved::decode(&good.repeat(2)).is_err());
        let body_end = good.len() - 12; let mut ext = good[..body_end].to_vec(); section(&mut ext, b"NAME", b"slot one"); let sum = fnv(&ext); ext.extend_from_slice(b"END "); ext.extend_from_slice(&sum.to_le_bytes());
        assert_eq!(Saved::decode(&ext).unwrap(), Saved::decode(&good).unwrap());
        let mut s = Saved::decode(&good).unwrap(); s.segs.remove(&Mem::PARTY_SEG); assert!(Saved::decode(&s.encode()).unwrap_err().contains("party"));
        let mut s = Saved::decode(&good).unwrap(); s.ds.truncate(100); assert!(Saved::decode(&s.encode()).unwrap_err().contains("DS"));
        assert!(Saved::decode(&vec![0; MAX_FILE + 1]).unwrap_err().contains("limit"));
    }
    #[test]
    fn live_save_location_uses_the_ordinary_player_format() {
        struct Live(PathBuf);
        impl Input for Live {
            fn next_key(&mut self)->u16 {27}
            fn live_save_location(&self)->Option<(&std::path::Path,&'static str)> {Some((&self.0,"live-window"))}
        }
        let dir=std::env::temp_dir().join(format!("mt-live-save-{}",std::process::id()));
        let mut g=synthetic();g.input=Box::new(Live(dir.clone()));
        let expected=Saved::capture(&mut g,Mode::Starport,"live-window");
        write(&mut g,Mode::Starport).unwrap();
        assert_eq!(read(&g).unwrap(),expected);
        assert!(expected.squad.is_none() && expected.play.is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn several_positions_old_slot_and_an_interrupted_write() {
        let dir = std::env::temp_dir().join(format!("mt-save-pos-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir);
        let mut g = synthetic(); g.save_slot = Some(Slot { dir: dir.clone(), build: "t" });
        assert!(list(&g).iter().all(|e| !e.used()) && list(&g).len() == (GAMES + 1 + AUTOS) as usize);
        g.mem.set_party_u16(0xF84, 33); write_file(&mut g, Mode::Starport, "game3.mtsav").unwrap(); let three = g.mem.ds.clone();
        g.mem.set_party_u16(0xF84, 55); g.mem.ds[0x200] ^= 0xFF; write_file(&mut g, Mode::Ground, "game5.mtsav").unwrap();
        let s3 = read_file(&g, "game3.mtsav").unwrap(); assert_eq!((s3.ds == three, s3.info().day, s3.mode), (true, 33, Mode::Starport));
        assert_eq!(read_file(&g, "game5.mtsav").unwrap().info().day, 55);
        s3.install(&mut g); assert_eq!((g.mem.ds == three, g.mem.party_u16(0xF84)), (true, 33));
        write(&mut g, Mode::Starport).unwrap();
        let l = list(&g); assert_eq!(l.iter().filter(|e| e.used()).map(|e| e.name()).collect::<Vec<_>>(), ["1", "3", "5"]);
        let r = rows(&l); assert_eq!((l[0].file.as_str(), &r[1]), (FILE, &["Game 2".to_string(), "Empty".into(), String::new()]));
        assert!(r[2][1].ends_with("starport") && r[2][2].starts_with("Day 33  Cr "), "{:?}", r[2]);
        assert_eq!((r[0][2].len(), r[2][2].len(), r[4][2].find("Cr"), &r[8][0], &r[9][0]), (r[4][2].len(), r[4][2].len(), r[0][2].find("Cr"), &"Quick".to_string(), &"Autosave 1".to_string()));
        let old = std::fs::read(dir.join("game3.mtsav")).unwrap();
        g.mem.set_party_u16(0xF84, 99); let bytes = Saved::capture(&mut g, Mode::Ground, "t").encode();
        let tmp = stage(&dir, "game3.mtsav", &bytes[..bytes.len() / 2]).unwrap(); assert!(tmp.exists());
        assert_eq!(std::fs::read(dir.join("game3.mtsav")).unwrap(), old); assert_eq!(read_file(&g, "game3.mtsav").unwrap().info().day, 33);
        assert_eq!(list(&g).iter().filter(|e| e.used()).count(), 3);
        write_file(&mut g, Mode::Ground, "game3.mtsav").unwrap(); assert!(!tmp.exists()); assert_eq!(read_file(&g, "game3.mtsav").unwrap().info().day, 99);
        std::fs::write(dir.join("game2.mtsav"), b"junk").unwrap(); assert_eq!(list(&g)[1].place(), "(damaged file)");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn quick_save_autosave_rotation_and_nothing_in_a_fight() {
        let dir = std::env::temp_dir().join(format!("mt-save-quick-{}", std::process::id())); let _ = std::fs::remove_dir_all(&dir);
        let mut plain = synthetic(); plain.save_slot = Some(Slot { dir: dir.clone(), build: "t" });
        assert!(!key(&mut plain, KEY_QUICK_SAVE), "no product scene (faithful): F6 stays the 1990's (nothing)"); landed(&plain); resume(&mut plain); assert!(!dir.exists());
        let mut g = synthetic(); g.save_slot = Some(Slot { dir: dir.clone(), build: "t" }); g.scene = Some(crate::ui::SceneHook::default()); g.squad = Some(Default::default());
        let last = |g: &Ground| g.squad.as_ref().unwrap().p1.log.last().cloned().unwrap_or_default();
        let days = |g: &Ground| list(g).iter().filter(|e| e.used()).map(|e| (e.name(), read_file(g, &e.file).unwrap().info().day)).collect::<Vec<_>>();
        assert!(!key(&mut g, 0x3B) && key(&mut g, KEY_QUICK_SAVE)); assert!(!dir.join(QUICK).exists(), "served at the loop top, not at the key");
        g.mem.set_party_u16(0xF84, 7); assert!(!resume(&mut g));
        assert_eq!((days(&g), last(&g).as_str(), read_file(&g, QUICK).unwrap().mode), (vec![("Quick".to_string(), 7)], "Quick-saved", Mode::Ground));
        g.mem.set_debug(0); assert!(key(&mut g, KEY_QUICK_LOAD)); DUE.set(0); g.mem.set_debug(1); assert!(!key(&mut g, KEY_QUICK_LOAD), "debug flag on: F9 stays the 1990 cheat"); g.mem.set_debug(0);
        for day in 1..=4 { g.mem.set_party_u16(0xF84, day); landed(&g); resume(&mut g); std::thread::sleep(std::time::Duration::from_millis(15)); }
        assert_eq!((days(&g)[1..].to_vec(), last(&g).as_str()), (vec![("Autosave 1".to_string(), 4), ("Autosave 2".to_string(), 3), ("Autosave 3".to_string(), 2)], "Autosaved"));
        g.squad.as_mut().unwrap().encounter = true; g.mem.set_party_u16(0xF84, 99); let before = days(&g);
        key(&mut g, KEY_QUICK_SAVE); resume(&mut g); assert_eq!(last(&g), "The game cannot be saved in a fight.");
        landed(&g); resume(&mut g); autosave(&mut g, Mode::Starport); assert_eq!(days(&g), before, "nothing is written in a fight");
        g.squad.as_mut().unwrap().encounter = false;
        AUTOSAVE.store(false, std::sync::atomic::Ordering::Relaxed); landed(&g); resume(&mut g); AUTOSAVE.store(true, std::sync::atomic::Ordering::Relaxed);
        assert_eq!(days(&g), before, "autosave = false: none");
        autosave(&mut g, Mode::Starport); let newest = list(&g)[GAMES as usize + 1].file.clone();
        assert_eq!((days(&g)[1..].to_vec(), read_file(&g, &newest).unwrap().mode), (vec![("Autosave 1".to_string(), 99), ("Autosave 2".to_string(), 4), ("Autosave 3".to_string(), 3)], Mode::Starport), "the oldest is replaced and lists first");
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn credits_read_with_commas() { assert_eq!([0, 999, 1000, 451002, 1451002, u32::MAX].map(commas), ["0", "999", "1,000", "451,002", "1,451,002", "4,294,967,295"]); }
    #[test]
    fn stamp_is_a_civil_date() {
        let at = |s: u64| stamp(std::time::UNIX_EPOCH + std::time::Duration::from_secs(s));
        assert_eq!((at(0), at(951782400 + 3600 * 14 + 300), at(1794405900)), ("1 Jan 00:00".to_string(), "29 Feb 14:05".to_string(), "11 Nov 14:05".to_string()));
    }
    #[test]
    fn slot_round_trip_through_a_real_file() {
        let dir = std::env::temp_dir().join(format!("mt-save-{}", std::process::id()));
        let mut g = synthetic(); g.save_slot = Some(Slot { dir: dir.clone(), build: "t" });
        write(&mut g, Mode::Starport).unwrap();
        let s = read(&g).unwrap(); assert_eq!(s.mode, Mode::Starport); assert_eq!(s.ds, g.mem.ds);
        assert!(!dir.join(format!("{FILE}.tmp")).exists());
        std::fs::write(dir.join(FILE), b"junk").unwrap(); assert!(read(&g).is_err());
        std::fs::remove_dir_all(dir).unwrap();
        g.save_slot = None; assert!(write(&mut g, Mode::Ground).is_err()); assert!(read(&g).is_err());
    }
}
