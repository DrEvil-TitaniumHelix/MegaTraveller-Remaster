#![cfg_attr(all(target_os = "windows", not(debug_assertions)), windows_subsystem = "windows")]
mod harness;
mod lockstep;
mod window;
mod menu;
mod scenario;
#[path = "setup/window.rs"]
mod setup;
#[cfg(test)]
mod save_tests;
use mt1::chargen::pool::Exit as PoolExit;
use mt1::screens::{cg::Screens, title::{Exit, TcRand, Title}};
use mt_engine::io::{Dir, Input, keys};
use mt_engine::data::{exepack, ly, pal, pic, world, Variant};
use mt_engine::present::{fb::Fb, ly as pre_ly, palette, pic as pre_pic, prim::Raster, text::{self, Font}};
use mt_engine::sim::tick::Clock;
use mt_engine::Game;
use std::path::{Path, PathBuf};
use std::io::{self, IsTerminal};
use std::{env, fs, panic, process};
use window::Win;

#[derive(Debug, Default, PartialEq)]
struct Args { mcp: bool, snapshot: Option<PathBuf>, play: bool, route: Option<PathBuf>, replay: Option<PathBuf>, out: Option<PathBuf>, scenario: Option<PathBuf>, scenario_dir: Option<PathBuf>, scenario_check: bool, save: Option<PathBuf>, gamefiles: Option<PathBuf>, scale: Option<usize>, square: bool, faithful: bool, viewer: bool, script: Option<PathBuf>, dump: Option<PathBuf>, seed: Option<u16>, disk: bool, ground: bool, party: Option<PathBuf>, now: Option<u32>, mouse: bool, keys: Option<PathBuf>, seeds: Option<PathBuf>, pokes: Option<PathBuf>, state: Option<PathBuf>, transport: Option<PathBuf>, speaker: Option<PathBuf>, save_at: Option<usize>, save_fault: bool }
fn parse(a: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut r, mut a) = (Args::default(), a.peekable());
    while let Some(x) = a.next() {
        match x.as_str() {
            "--mcp" => r.mcp = true,
            "--play" => { r.play = true; if a.peek().is_some_and(|s| !s.starts_with("--")) { r.route = a.next().map(PathBuf::from); } }
            "--replay" => r.replay = Some(a.next().ok_or("--replay needs a trace")?.into()),
            "--out" => r.out = Some(a.next().ok_or("--out needs a directory")?.into()),
            "--snapshot" => r.snapshot = Some(a.next().ok_or("--snapshot needs a JSON file")?.into()),
            "--scenario" => r.scenario = Some(a.next().ok_or("--scenario needs a file")?.into()),
            "--scenario-dir" => r.scenario_dir = Some(a.next().ok_or("--scenario-dir needs a directory")?.into()),
            "--scenario-check" => r.scenario_check = true,
            "--save" => r.save = Some(a.next().ok_or("--save needs a file")?.into()),
            "--gamefiles" => r.gamefiles = Some(a.next().ok_or("--gamefiles needs a directory")?.into()),
            "--scale" => r.scale = Some(a.next().and_then(|n| n.parse().ok()).filter(|&n| (1..=32).contains(&n)).ok_or("--scale needs 1..32")?),
            "--square" => r.square = true,
            "--faithful" => r.faithful = true,
            "--viewer" => r.viewer = true,
            "--script" => r.script = Some(a.next().ok_or("--script needs a file")?.into()),
            "--keys" => r.keys = Some(a.next().ok_or("--keys needs a file")?.into()),
            "--seeds" => r.seeds = Some(a.next().ok_or("--seeds needs a file")?.into()),
            "--pokes" => r.pokes = Some(a.next().ok_or("--pokes needs a file")?.into()),
            "--state" => r.state = Some(a.next().ok_or("--state needs a file")?.into()),
            "--transport" => r.transport = Some(a.next().ok_or("--transport needs a file")?.into()),
            "--speaker" => r.speaker = Some(a.next().ok_or("--speaker needs a file")?.into()),
            "--save-at" => r.save_at = Some(a.next().and_then(|n| n.parse().ok()).ok_or("--save-at needs a step number")?),
            "--save-fault" => r.save_fault = true,
            "--dump" => r.dump = Some(a.next().ok_or("--dump needs a directory")?.into()),
            "--seed" => r.seed = Some(a.next().and_then(|n| n.strip_prefix("0x").map_or_else(|| n.parse().ok(), |h| u16::from_str_radix(h, 16).ok())).ok_or("--seed needs 0..65535")?),
            "--now" => r.now = Some(a.next().and_then(|n| n.strip_prefix("0x").map_or_else(|| n.parse().ok(), |h| u32::from_str_radix(h, 16).ok())).ok_or("--now needs 0..4294967295")?),
            "--disk" => r.disk = true,
            "--ground" => r.ground = true,
            "--mouse" => r.mouse = true,
            "--party" => r.party = Some(a.next().ok_or("--party needs a file")?.into()),
            _ => return Err(format!("unknown argument {x}; usage: [--gamefiles D] [--mcp | --snapshot F | --play ROUTE [--out D] | --play --replay TRACE [--out D] | --build-id] [--scenario F [--scenario-check [--state F] [--save F]]] [--scenario-dir D] [--scale N] [--square] [--mouse] [--viewer] [--ground [--party F] [--now N]] [--script F --dump D [--seed N] [--disk]]")),
        }
    }
    if r.play {
        if r.route.is_some() == r.replay.is_some() { return Err("use --play ROUTE or --play --replay TRACE".into()) }
        let allowed = Args { play: true, route: r.route.clone(), replay: r.replay.clone(), out: r.out.clone(), gamefiles: r.gamefiles.clone(), ..Args::default() };
        if r != allowed { return Err("--play accepts only --out, --gamefiles, and either a route or --replay trace".into()) }
    } else if r.replay.is_some() || r.out.is_some() { return Err("--replay and --out need --play".into()) }
    if r.snapshot.is_some() && (r.faithful || r.mcp || r.viewer || r.ground || r.scenario.is_some() || r.scenario_check || r.script.is_some() || r.keys.is_some() || r.party.is_some() || r.seed.is_some() || r.now.is_some() || r.disk) {return Err("--snapshot is a product window boot mode; it cannot be combined with --faithful or another boot mode".into())}
    if r.mcp && (r.faithful || r.viewer || r.ground || r.script.is_some() || r.keys.is_some() || r.scenario.is_some()) { return Err("--mcp cannot be combined with faithful or another boot mode".into()) }
    if r.faithful && (r.scenario.is_some() || r.scenario_check || r.scenario_dir.is_some()) { return Err("--scenario is a test tool; faithful runs never take it".into()) }
    if r.scenario_check && r.scenario.is_none() { return Err("--scenario-check needs --scenario FILE".into()) }
    if r.save.is_some() && !r.scenario_check { return Err("--save needs --scenario FILE --scenario-check".into()) }
    if r.scenario.is_some() && (r.viewer || r.ground || r.script.is_some() || r.keys.is_some() || r.seeds.is_some() || r.pokes.is_some() || r.party.is_some() || r.transport.is_some() || r.speaker.is_some() || r.dump.is_some() || r.disk || r.now.is_some() || r.seed.is_some()) { return Err("--scenario cannot be combined with another boot or replay mode".into()) }
    if (r.script.is_some() || r.dump.is_some()) && !(r.faithful && r.script.is_some() && r.dump.is_some()) { return Err("--script and --dump go together and need --faithful".into()) }
    if r.party.is_some() && !r.ground { return Err("--party needs --ground".into()) }
    if r.now.is_some() && !(r.ground && r.script.is_some()) { return Err("--now needs --ground --faithful --script F --dump D".into()) }
    if r.save_fault && r.save_at.is_none() { return Err("--save-fault needs --save-at K".into()) }
    if r.keys.is_some() || r.seeds.is_some() || r.pokes.is_some() || (r.state.is_some() && !r.scenario_check) || r.transport.is_some() || r.speaker.is_some() || r.save_at.is_some() {
        if !(r.faithful && r.keys.is_some() && r.seeds.is_some() && r.state.is_some()) {return Err("lockstep needs --faithful --keys F --seeds F --state F".into())}
        if r.script.is_some() || r.dump.is_some() || r.seed.is_some() || r.disk || r.ground || r.viewer || r.mouse {return Err("lockstep cannot be combined with viewer/ground/legacy harness options".into())}
    }
    if r.faithful && r.script.is_none() && r.keys.is_none() { return Err("unknown argument --faithful (the game has one mode)".into()) }
    Ok(r)
}
fn die(msg: &str, code: i32) -> ! {
    eprintln!("{msg}");
    if code != 2 && io::stdin().is_terminal() { eprintln!("Press Enter to close"); let _ = io::stdin().read_line(&mut String::new()); }
    process::exit(code)
}
#[derive(Debug, PartialEq)]
struct View { n: usize, square: bool, faithful: bool }
fn view(a: &Args) -> View { if a.faithful { View { n: a.scale.unwrap_or(3), square: true, faithful: true } } else { View { n: a.scale.unwrap_or(0), square: a.square, faithful: false } } }
fn resolve(a: &Args, exe_dir: &Path, env: Option<String>) -> PathBuf {
    let beside = exe_dir.join("gamefiles");
    a.gamefiles.clone().or_else(|| beside.is_dir().then(|| beside.clone())).or_else(|| env.map(PathBuf::from)).unwrap_or(beside)
}
fn files(dir: &Path) -> Vec<(String, PathBuf)> {
    let mut v: Vec<_> = fs::read_dir(dir).into_iter().flatten().flatten().map(|e| (e.file_name().to_string_lossy().to_uppercase(), e.path())).collect();
    v.sort(); v
}
#[cfg(test)]
fn missing<'a>(have: &[(String, PathBuf)], req: &[&'a str]) -> Vec<&'a str> { req.iter().copied().filter(|r| !have.iter().any(|(n, _)| n == r)).collect() }

enum Kind { Pic(Vec<pic::Pic>), Ly(Vec<ly::Entry>), Font(Vec<ly::Entry>, Option<Font>), Map, Err }
fn cg_font(glyphs: Vec<ly::Entry>, cg: Option<&[u8]>) -> Option<Font> {
    let exe = cg?;
    let base = u16::from_le_bytes([*exe.get(8)?, *exe.get(9)?]) as usize * 16 + 0x924 * 16;
    Font::from_tables(glyphs, exe, base + 0x358E, base + 0x3CE5, 2)
}
const SAMPLE: &str = "text sample 0x20-0xFF (32 per line, 16 from 0xC0)";
struct Item { name: String, kind: Kind, entries: Vec<String> }
const PIC_SETS: [&str; 6] = ["X.DAT", "Y.DAT", "Z.DAT", "C.DAT", "OBJ.DAT", "SPOBJ.DAT"];
fn map_label(m: &world::Map) -> String { format!("{}x{} {}", m.hdr.cols, m.hdr.rows, String::from_utf8_lossy(&m.hdr.name).trim_end_matches('\0')) }
fn catalog(have: &[(String, PathBuf)], cg: Option<&[u8]>) -> Vec<Item> {
    have.iter().filter_map(|(n, p)| {
        let d = fs::read(p).ok()?;
        let err = |e: mt_engine::data::bin::Error| (Kind::Err, vec![format!("decode error: {e:?}")]);
        let (kind, entries) = if n.ends_with(".PIC") || PIC_SETS.contains(&n.as_str()) {
            match if n.ends_with(".PIC") { pic::decode(Variant::Mt1, &d).map(|p| vec![p]) } else { pic::decode_set(Variant::Mt1, &d) } {
                Ok(v) => { let e = v.iter().enumerate().map(|(i, p)| format!("#{i} {}x{}", p.wbytes4 as u32 * 8, p.h)).collect(); (Kind::Pic(v), e) }
                Err(e) => err(e),
            }
        } else if n.ends_with(".LY") {
            match ly::decode(&d) {
                Ok(v) if n == "FONT.LY" => {
                    let mut e: Vec<String> = v.iter().enumerate().map(|(i, x)| format!("glyph {i:#04x} {:?} ({} recs)", i as u8 as char, x.len())).collect();
                    e.push(SAMPLE.into()); let f = cg_font(v.clone(), cg); (Kind::Font(v, f), e)
                }
                Ok(v) => { let e = v.iter().enumerate().map(|(i, x)| format!("entry {i} ({} recs)", x.len())).collect(); (Kind::Ly(v), e) }
                Err(e) => err(e),
            }
        } else if n == "W.DAT" {
            match world::decode(Variant::Mt1, &d) {
                Ok(v) => (Kind::Map, v.iter().enumerate().flat_map(|(a, c)| match c {
                    world::Chunk::Area(x) => x.maps.iter().enumerate().map(|(m, mp)| format!("area {a} map {m} {}", map_label(mp))).collect::<Vec<_>>(),
                    world::Chunk::Inside(mp) => vec![format!("chunk {a} inside {}", map_label(mp))],
                }).collect()),
                Err(e) => err(e),
            }
        } else { return None };
        Some(Item { name: n.clone(), kind, entries })
    }).collect()
}
fn draw(fb: &mut Fb, it: &Item, e: usize) -> Result<(), String> {
    mt_engine::present::textlog::clear(fb);
    fb.planes.fill(0);
    let r = panic::catch_unwind(panic::AssertUnwindSafe(|| match &it.kind {
        Kind::Pic(v) => v.get(e).map(|p| pre_pic::draw(fb, 0, p, 0, 0, None)).ok_or("empty PIC set".to_string()),
        Kind::Ly(v) => Ok(pre_ly::layout(fb, Raster::Clip, 0, v, e, 0, 0, &mut pre_ly::Pen::new(15))),
        Kind::Font(g, _) if e < g.len() => Ok(pre_ly::layout(fb, Raster::Clip, 0, g, e, 0, 0, &mut pre_ly::Pen::new(15))),
        Kind::Font(_, Some(f)) => {
            let (a, b): (Vec<u8>, Vec<u8>) = ((0x20..0xC0).collect(), (0xC0..=0xFF).collect());
            for (i, row) in a.chunks(32).chain(b.chunks(16)).enumerate() { text::draw(fb, Raster::Clip, 0, f, row, 8 + 20 * i as i32, 8, 15, None); }
            Ok(())
        }
        Kind::Font(_, None) => Err("no CG.DAT width/flag tables (missing or not EXEPACK): per-glyph LY view only".to_string()),
        Kind::Map => Err("maps: no present API yet".to_string()),
        Kind::Err => Err(it.entries[0].clone()),
    }));
    r.unwrap_or_else(|p| Err(p.downcast_ref::<String>().cloned().or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or("panic".into())))
}
#[cfg(target_os = "windows")]
fn attach_console() {
    use windows_sys::Win32::Storage::FileSystem::{CreateFileW, FILE_SHARE_READ, FILE_SHARE_WRITE, OPEN_EXISTING};
    use windows_sys::Win32::System::Console::{AttachConsole, GetStdHandle, SetStdHandle, ATTACH_PARENT_PROCESS, STD_ERROR_HANDLE, STD_OUTPUT_HANDLE};
    unsafe {
        if AttachConsole(ATTACH_PARENT_PROCESS) == 0 { return }
        for id in [STD_OUTPUT_HANDLE, STD_ERROR_HANDLE] {
            let old = GetStdHandle(id);
            if !old.is_null() && old as isize != -1 { continue }
            let name: Vec<u16> = "CONOUT$".encode_utf16().chain([0]).collect();
            let out = CreateFileW(name.as_ptr(), 0x8000_0000 | 0x4000_0000, FILE_SHARE_READ | FILE_SHARE_WRITE, std::ptr::null(), OPEN_EXISTING, 0, std::ptr::null_mut());
            if !out.is_null() && out as isize != -1 { SetStdHandle(id, out); }
        }
    }
}
fn main() {
    #[cfg(target_os = "windows")]
    attach_console();
    if env::args_os().skip(1).eq([std::ffi::OsString::from("--build-id")]) { println!("{}", env!("MT_BUILD_HASH")); return; }
    let play_requested = env::args_os().any(|a| a == "--play");
    let args = parse(env::args_os().skip(1).map(|a| a.to_string_lossy().into_owned())).unwrap_or_else(|e| {
        if play_requested { eprintln!("bad route: {e}"); process::exit(megatraveller::mcp::executor::BAD_ROUTE); }
        die(&e, 2)
    });
    let exe_dir = env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_default();
    let dir = resolve(&args, &exe_dir, env::var("MT1_GAMEFILES").ok());
    if args.play {
        let out = args.out.unwrap_or_else(|| PathBuf::from("acceptance/local/play").join(format!("run-{}-{}", process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos())));
        let path = args.route.as_ref().or(args.replay.as_ref()).expect("validated --play input");
        process::exit(megatraveller::mcp::executor::execute(path, args.replay.is_some(), &out, dir, exe_dir));
    }
    if args.mcp {
        megatraveller::mcp::serve(dir, exe_dir).unwrap_or_else(|e| die(&e, 2)); return;
    }
    let save_slot = || mt1::save::Slot { dir: exe_dir.join("saves"), build: env!("MT_BUILD_HASH") };
    mt1::save::UTC_OFFSET.store(utc_offset(), std::sync::atomic::Ordering::Relaxed);
    let v = view(&args);
    mt1::ui::set_software_cursor(v.faithful);
    let request = scenario::Request::default();
    let mut restored = if let Some(path)=&args.snapshot {
        let assets=dir.clone();let live_request=request.clone();
        let scenarios=args.scenario_dir.clone().unwrap_or_else(||exe_dir.join("scenarios"));
        let resumed=megatraveller::mcp::launch::file_desktop(path,&dir,&exe_dir,request.clone(),move |_| {
            let mut win=Win::new(v.n,v.square,false,args.mouse).map_err(|e|e.to_string())?;
            win.panel_assets(&mut Dir(assets));win.scenarios(&scenarios,live_request)?;
            let clock=win.clock();Ok((Box::new(win),clock))
        }).unwrap_or_else(|e|die(&e,2));
        if resumed.is_none() {return}
        resumed
    } else {None};
    let headless = args.viewer || args.scenario.is_some() || args.keys.is_some() || args.script.is_some();
    let (g, mut dir) = (mt1::Mt1, dir);
    let verdict = mt1::gamefiles::verify(&dir);
    let mut early: Option<Box<Win>> = None;
    if !verdict.ok() {
        eprintln!("{}", verdict.text(&dir));
        if headless {
            if let (true, Some(path)) = (verdict.refuses(), &args.scenario) { die(&format!("scenario {}: gamefiles: {}", path.display(), verdict.missing.iter().chain(&verdict.code_differ).cloned().collect::<Vec<_>>().join(", ")), 2); }
            if verdict.refuses() { process::exit(1); }
        } else {
            let mut win = Win::new(v.n, v.square, v.faithful, args.mouse).unwrap_or_else(|e| die(&e, 1));
            match setup::screen(&mut win, &dir, &verdict, &exe_dir.join("gamefiles")) { Some(d) => dir = d, None => process::exit(1) }
            win.accept_drops(false); early = Some(Box::new(win));
        }
    }
    let dir = dir; let have = files(&dir);
    let prepared = args.scenario.as_deref().map(|path| scenario::load(path, &dir).unwrap_or_else(|e| die(&e, 2)));
    if args.scenario_check {
        if let Some(mut prepared) = prepared {
            prepared.report();
            if let Some(path) = &args.state {
                let record = serde_json::to_string(&prepared.record()).unwrap_or_else(|e| die(&e.to_string(), 2));
                fs::write(path, format!("{record}\n")).unwrap_or_else(|e| die(&format!("{}: {e}", path.display()), 2));
            }
            if let Some(path) = &args.save {
                let bytes = mt1::save::Saved::capture(&mut prepared.game, mt1::save::Mode::Ground, env!("MT_BUILD_HASH")).encode();
                if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) { fs::create_dir_all(dir).unwrap_or_else(|e| die(&format!("{}: {e}", dir.display()), 2)); }
                fs::write(path, &bytes).unwrap_or_else(|e| die(&format!("{}: {e}", path.display()), 2));
                println!("scenario saved: {} ({} bytes, save format {})", path.display(), bytes.len(), mt1::save::VERSION);
            }
        }
        return;
    }
    if let Some(prepared)=&prepared {prepared.require_playable().unwrap_or_else(|e|die(&e,2));}
    if let (Some(keys),Some(seeds),Some(state))=(&args.keys,&args.seeds,&args.state) {
        lockstep::run(&dir,keys,seeds,args.pokes.as_deref(),state,args.transport.as_deref(),args.speaker.as_deref(),args.save_at,args.save_fault).unwrap_or_else(|e|die(&e,3));
        return
    }
    let cg = have.iter().find(|(n, _)| n == "CG.DAT").and_then(|(_, p)| fs::read(p).ok()).and_then(|d| exepack::unpack(&d).ok()).map(|u| u.exe);
    if args.viewer { viewer(&g, &dir, &v, &args, &have, cg); return }
    let ly_file = |n: &str| have.iter().find(|(f, _)| f == n).and_then(|(_, p)| fs::read(p).ok()).and_then(|d| ly::decode(&d).ok()).unwrap_or_else(|| die(&format!("{n}: cannot decode"), 1));
    let (cg_exe, font_ly, cg_ly) = (cg.unwrap_or_else(|| die("CG.DAT: not an EXEPACK executable", 1)), ly_file("FONT.LY"), ly_file("CG.LY"));
    let read = |n: &str| have.iter().find(|(f, _)| f == n).and_then(|(_, p)| fs::read(p).ok()).unwrap_or_else(|| die(&format!("{n}: cannot read"), 1));
    let party_file = |p: &Option<PathBuf>| p.as_ref().map_or_else(|| read("DEFAULT.SAV"), |f| fs::read(f).unwrap_or_else(|e| die(&format!("{}: {e}", f.display()), 1)));
    if let (Some(script), Some(dump), true) = (&args.script, &args.dump, args.ground) {
        let lines = harness::lines(script).unwrap_or_else(|e| die(&e, 2));
        let mut g = harness::ground(&read("T.DAT"), party_file(&args.party), lines, dump, args.seed, &dir).unwrap_or_else(|e| die(&e, 1));
        if let Some(now) = args.now { g.clock = Box::new(mt_engine::sim::tick::Counter::at(now)); }
        if args.mouse { g.mouse_driver = true }
        mt1::ground::run(&mut g);
        process::exit(0)
    }
    if let (Some(script), Some(dump)) = (&args.script, &args.dump) {
        let keys = harness::keys(script).unwrap_or_else(|e| die(&e, 2));
        let mut s = harness::screens(cg_exe, font_ly, cg_ly, keys, dump, args.seed, args.disk, &dir).unwrap_or_else(|e| die(&e, 1));
        let t = s.ticks() as u16; s.rng.srand(t);
        mt1::screens::cg::run(&mut s);
        process::exit(0)
    }
    let mut win = if restored.is_some() {None} else {Some(early.take().unwrap_or_else(|| Box::new(Win::new(v.n, v.square, v.faithful, args.mouse).unwrap_or_else(|e| die(&e, 1)))))};
    if let Some(win)=&mut win {win.panel_assets(&mut Dir(dir.clone())); mt1::save::AUTOSAVE.store(win.autosave(), std::sync::atomic::Ordering::Relaxed); mt1::record::set_no_unused_skills(win.disable_unused_skills());}
    let squad = restored.as_ref().map_or_else(||!v.faithful && !win.as_ref().unwrap().classic_combat(),|g|g.squad.is_some());
    let mut balance = restored.as_ref().map_or_else(||win.as_ref().unwrap().balance_options(),|g|g.balance);
    let squad_state = || {
        let path = [exe_dir.join("data").join("npcs.toml"), PathBuf::from("data/npcs.toml")].into_iter().find(|p| p.exists()).unwrap_or_else(|| exe_dir.join("data").join("npcs.toml"));
        let compendium = fs::read_to_string(&path).map_err(|e| e.to_string()).and_then(|t| mt1::ground::rules::compendium::Compendium::parse(&t))
            .unwrap_or_else(|e| { eprintln!("{}: {e}: NPCs use the derived numbers (R-9)", path.display()); Default::default() });
        mt1::ground::squad::State::with_compendium(compendium)
    };
    if !v.faithful && win.is_some() {
        win.as_mut().unwrap().scenarios(args.scenario_dir.as_deref().unwrap_or(&exe_dir.join("scenarios")), request.clone()).unwrap_or_else(|e| die(&e, 1));
    }
    if let Some(mut prepared) = prepared {
        let win=win.take().expect("scenario window");
        prepared.report();
        prepared.game.balance = balance;
        let mut clock = win.clock(); clock.set(prepared.game.clock.now());
        prepared.game.clock = clock;
        prepared.game.input = win;
        scenario::attach_scene(&mut prepared.game);
        prepared.game.mouse_driver = true;
        prepared.game.save_slot = Some(save_slot());
        prepared.game.tooling = Some(Box::new(scenario::Loader::desktop(request, dir, prepared.extensions)));
        let loaded = prepared.game.squad.take();
        prepared.game.squad = squad.then(|| loaded.unwrap_or_else(squad_state));
        mt1::ground::main_loop::run(&mut prepared.game);
        return;
    }
    let dac = palette::ega_dac();
    let startup = have.iter().find(|(n, _)| n == "STARTUP.EXE").and_then(|(_, p)| fs::read(p).ok()).and_then(|d| exepack::unpack(&d).ok()).map(|u| u.exe);
    let pic_file = |n: &str| have.iter().find(|(f, _)| f == n).and_then(|(_, p)| fs::read(p).ok()).and_then(|d| pic::decode(Variant::Mt1, &d).ok());
    let (mut input,mut clock):(Box<dyn Input>,Box<dyn Clock>)=if let Some(g)=restored.take() {(g.input,g.clock)}else {let win=win.take().unwrap();let clock=win.clock();(win,clock)};
    if !v.faithful { mt_engine::present::textlog::set_enabled(true); }
    let mut title = startup.and_then(|s| Title::new(&s, ly_file("TITLE.LY"), ly_file("FONT.LY"), pic_file("TIT.PIC")?, pic_file("ZOD.PIC")?));
    loop {
        match title.as_mut() {
            Some(t) => {
                let mut rand = TcRand { seed: clock.now() as u16 as u32 };
                let faithful = v.faithful;
                if t.run(&mut || rand.next(), clock.as_mut(), input.as_mut(), |t, i| { i.present(&t.fb.rgb(t.display, &dac)); if !faithful { i.scene(&mt1::ui::Scene::program(mt1::ui::ScreenKind::Startup)) } }) == Exit::Quit { process::exit(1) }
            }
            None => eprintln!("STARTUP.EXE/TITLE.LY/TIT.PIC/ZOD.PIC: title skipped"),
        }
        if input.closed() { return }
        let t_dat = read("T.DAT");
        let (input2, clock2, start): (Box<dyn Input>, Box<dyn Clock>, Result<mt1::save::Saved, Vec<u8>>) = if args.ground { (input, clock, Err(party_file(&args.party))) } else {
            let mut s = Screens::new(cg_exe.clone(), font_ly.clone(), cg_ly.clone(), input, clock, Box::new(Dir(dir.clone()))).unwrap_or_else(|| die("CG.DAT: tables not found", 1));
            s.snapshots = !v.faithful;
            let t = s.ticks() as u16; s.rng.srand(t);
            let (exit, party, picked) = loop {
                let (exit, party) = mt1::screens::cg::run(&mut s);
                if exit != PoolExit::Continue { break (exit, party, None) }
                if let Some(saved) = mt1::save::continue_old(&mut s, &save_slot().dir) { break (exit, party, Some(saved)) }
            };
            let sav = read("DEFAULT.SAV");
            let start = match exit {
                PoolExit::Play => Err(mt1::ground::party::block(&mt1::ground::party::initial(), &party, &s.ds, &mut s.rng)
                    .unwrap_or_else(|| die("CG: invalid initial party block", 1))),
                PoolExit::Quick => Err(sav),
                PoolExit::Continue => Ok(picked.expect("Continue Old Game leaves the menu with a save")),
                PoolExit::Dos => return,
            };
            (s.input, s.clock, start)
        };
        let restored = start.is_ok();
        let mut g = match start {
            Ok(saved) => mt1::save::ground(saved, input2, clock2, Box::new(Dir(dir.clone()))),
            Err(block) => mt1::ground::Ground::new(&t_dat, block, input2, clock2, Box::new(Dir(dir.clone()))).unwrap_or_else(|| die("T.DAT: not the EXEPACK program, or the party block is not 0xF86 bytes", 1)),
        };
        g.save_slot = Some(save_slot());
        g.mouse_driver = !v.faithful || args.mouse;
        g.scene = (!v.faithful).then(mt1::ui::SceneHook::default);
        if !v.faithful { g.tooling = Some(Box::new(scenario::Loader::desktop(request.clone(), dir.clone(), scenario::Extensions::default()))); }
        g.squad = squad.then(squad_state);
        g.balance = balance;
        if restored { mt1::ground::main_loop::run(&mut g) } else { mt1::ground::run(&mut g) }
        if !g.game_over || g.input.closed() { return }
        balance = g.balance;
        input = g.input; clock = g.clock;
    }
}
fn utc_offset() -> i32 {
    #[cfg(windows)] {
        use windows_sys::Win32::{Foundation::FILETIME, Storage::FileSystem::FileTimeToLocalFileTime};
        let t = 0x01DC_0000_0000_0000u64;
        let (utc, mut local) = (FILETIME { dwLowDateTime: t as u32, dwHighDateTime: (t >> 32) as u32 }, FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 });
        if unsafe { FileTimeToLocalFileTime(&utc, &mut local) } != 0 { return ((((local.dwHighDateTime as u64) << 32 | local.dwLowDateTime as u64) as i64 - t as i64) / 10_000_000) as i32 }
    }
    0
}
fn viewer(g: &mt1::Mt1, dir: &Path, v: &View, args: &Args, have: &[(String, PathBuf)], cg: Option<Vec<u8>>) {
    println!("{} viewer; gamefiles {}; faithful {}", g.name(), dir.display(), v.faithful);
    if v.faithful && args.square { println!("--faithful: --square ignored (square pixels)") }
    let items = catalog(have, cg.as_deref());
    let (dac, mut fb) = (palette::ega_dac(), Fb::new());
    if let Some(r) = have.iter().find(|(n, _)| n == "MAP.PAL").and_then(|(_, p)| fs::read(p).ok()).and_then(|d| pal::decode(&d).ok()).and_then(|p| palette::regs(&p)) { fb.regs = r }
    let mut win = Win::new(v.n, v.square, v.faithful, args.mouse).unwrap_or_else(|e| die(&e, 1));
    win.panel_assets(&mut Dir(dir.to_path_buf()));
    let (mut f, mut e, mut status) = (0usize, 0usize, "Enter draws".to_string());
    win.present(&fb.rgb(0, &dac));
    loop {
        let it = items.get(f);
        win.set_title(&format!("{} [{}/{}] {} - {}", it.map_or("(no viewable files)", |i| &i.name),
            f + 1, items.len(), it.and_then(|i| i.entries.get(e)).map_or("", String::as_str), status));
        let (nf, count) = (items.len().max(1), it.map_or(1, |i| i.entries.len().max(1)));
        match win.next_key() {
            keys::ESC => break,
            keys::DOWN => { f = (f + 1) % nf; e = 0; }
            keys::UP => { f = (f + nf - 1) % nf; e = 0; }
            keys::RIGHT => e = (e + 1) % count,
            keys::LEFT => e = (e + count - 1) % count,
            keys::ENTER => if let Some(item) = it {
                status = draw(&mut fb, item, e).map_or_else(|m| m, |_| "drawn".into());
                win.present(&fb.rgb(0, &dac));
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn mcp_boot_is_headless_and_excludes_other_modes() {
        let parse=|args: &[&str]| super::parse(args.iter().map(|s|s.to_string()));
        assert!(parse(&["--mcp"]).unwrap().mcp);
        for flag in ["--faithful","--viewer","--ground"] { assert!(parse(&["--mcp",flag]).is_err()); }
        for flag in ["--scenario","--script","--keys"] { assert!(parse(&["--mcp",flag,"file"]).is_err()); }
    }
    #[test]
    fn lockstep_options_are_a_distinct_faithful_mode() {
        let parse = |args:&[&str]| super::parse(args.iter().map(|s|s.to_string()));
        let args=parse(&["--faithful","--keys","a.keys","--seeds","a.json","--state","out.jsonl","--pokes","p.json","--transport","t.json"]).unwrap();
        assert_eq!(args.keys.as_deref(),Some(Path::new("a.keys")));
        for extra in ["--viewer","--ground","--disk","--mouse"] {
            assert!(parse(&["--faithful","--keys","a","--seeds","s","--state","o",extra]).is_err());
        }
        assert!(parse(&["--keys","a","--seeds","s","--state","o"]).is_err());
        assert!(parse(&["--faithful","--pokes","p"]).is_err());
        assert!(parse(&["--faithful","--speaker","s.jsonl"]).is_err());
        assert_eq!(parse(&["--scenario","a.toml","--scenario-check","--save","s.mtsav"]).unwrap().save.as_deref(),Some(Path::new("s.mtsav")));
        assert!(parse(&["--scenario","a.toml","--save","s.mtsav"]).is_err()); assert!(parse(&["--faithful","--scenario","a.toml","--scenario-check","--save","s.mtsav"]).is_err());
        assert_eq!(parse(&["--faithful","--keys","a","--seeds","s","--state","o","--save-at","12","--save-fault"]).map(|a|(a.save_at,a.save_fault)),Ok((Some(12),true)));
        assert!(parse(&["--faithful","--save-at","12"]).is_err()); assert!(parse(&["--faithful","--keys","a","--seeds","s","--state","o","--save-fault"]).is_err());
        assert_eq!(parse(&["--faithful","--keys","a","--seeds","s","--state","o","--speaker","sp"]).unwrap().speaker.as_deref(),Some(Path::new("sp")));
    }
    #[test]
    fn picker_and_resolution() {
        let d = env::temp_dir().join(format!("mt1-viewer-{}", process::id()));
        fs::create_dir_all(&d).unwrap();
        let p = pic::Pic { h: 1, wbytes4: 1, pal: [0; 16], px: vec![0x12, 0x34, 0x56, 0x78], tail: vec![] };
        fs::write(d.join("a.pic"), pic::encode(Variant::Mt1, &p).unwrap()).unwrap();
        fs::write(d.join("OBJ.DAT"), pic::encode_set(Variant::Mt1, &[p.clone(), p]).unwrap()).unwrap();
        let mut g: Vec<ly::Entry> = vec![vec![]; 0x42];
        g[1] = vec![ly::Rec { op: 4, colour: 1, y1: 0, x1: 0, y2: 0, x2: 0 }]; g[2] = vec![ly::Rec { op: 4, colour: 0x10, y1: 0, x1: 0, y2: 0, x2: 0 }];
        g[0x41] = vec![ly::Rec { op: 2, colour: 15, y1: 0, x1: 0, y2: 3, x2: 3 }];
        fs::write(d.join("FONT.LY"), ly::encode(&g).unwrap()).unwrap();
        fs::write(d.join("X.DAT"), pic::encode_set(Variant::Mt1, &[]).unwrap()).unwrap();
        fs::write(d.join("TIT.PIC"), b"junk").unwrap();
        fs::write(d.join("TRAV.MSG"), b"").unwrap();
        let have = files(&d);
        let c = catalog(&have, None);
        assert_eq!(c.iter().map(|i| (i.name.as_str(), i.entries.len())).collect::<Vec<_>>(), [("A.PIC", 1), ("FONT.LY", 0x42 + 1), ("OBJ.DAT", 2), ("TIT.PIC", 1), ("X.DAT", 0)]);
        assert_eq!(c[0].entries[0], "#0 8x1"); assert!(c[1].entries[1].starts_with("glyph 0x01")); assert_eq!(c[1].entries[0x42], SAMPLE); assert!(c[3].entries[0].starts_with("decode error"));
        let mut fb = Fb::new();
        assert!(draw(&mut fb, &c[3], 0).unwrap_err().starts_with("decode error"));
        assert_eq!(draw(&mut fb, &c[4], 0).unwrap_err(), "empty PIC set");
        assert_eq!(draw(&mut fb, &c[0], 0), Ok(())); assert_eq!((0..8).map(|x| fb.get(0, 0, x)).collect::<Vec<_>>(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(draw(&mut fb, &c[1], 1), Ok(())); assert_eq!(fb.get(0, 0, 0), 1); assert_eq!(fb.planes.iter().filter(|&&b| b != 0).count(), 1);
        assert_eq!(draw(&mut fb, &c[1], 2), Ok(())); assert_eq!(fb.get(0, 0, 0), 15);
        assert!(draw(&mut fb, &c[1], 0x42).unwrap_err().starts_with("no CG.DAT"));
        let mut exe = vec![0u8; 0x9240 + 0x3DE5]; exe[0x9240 + 0x358E..0x9240 + 0x368E].fill(5);
        let c = catalog(&have, Some(&exe));
        assert_eq!(draw(&mut fb, &c[1], 0x42), Ok(()));
        assert_eq!(fb.get(0, 28, 13), 15); assert_eq!(fb.get(0, 31, 16), 15); assert_eq!(fb.get(0, 28, 12), 0); assert_eq!(fb.get(0, 32, 13), 0);
        let m = missing(&have, mt1::Mt1.required_files());
        assert_eq!(m.len(), 64 - 5); assert!(m.contains(&"A0.PIC") && !m.contains(&"OBJ.DAT") && !m.contains(&"TRAV.MSG"));
        let a = parse(["--scale", "3", "--gamefiles", "X"].into_iter().map(String::from)).unwrap();
        assert_eq!(a, Args { gamefiles: Some("X".into()), scale: Some(3), ..Args::default() });
        assert!(parse(["--faithful"].into_iter().map(String::from)).is_err(), "no windowed 1990 mode");
        assert!(parse(["--faithful", "--scale", "4"].into_iter().map(String::from)).is_err());
        let h = parse(["--faithful", "--script", "k.txt", "--dump", "out", "--seed", "0x1267", "--disk"].into_iter().map(String::from)).unwrap();
        assert_eq!((h.script.as_deref(), h.dump.as_deref(), h.seed, h.disk), (Some(Path::new("k.txt")), Some(Path::new("out")), Some(0x1267), true));
        assert!(parse(["--script", "k"].into_iter().map(String::from)).is_err());
        let g = parse(["--faithful", "--ground", "--party", "p.sav", "--script", "k", "--dump", "d"].into_iter().map(String::from)).unwrap();
        assert!(g.ground && g.party.as_deref() == Some(Path::new("p.sav"))); assert!(parse(["--party", "p"].into_iter().map(String::from)).is_err());
        assert_eq!(parse(["--faithful", "--ground", "--script", "k", "--dump", "d", "--now", "0x460"].into_iter().map(String::from)).unwrap().now, Some(1120));
        for args in [&["--now", "1"][..], &["--ground", "--now", "1"], &["--now", "-1"], &["--now", "4294967296"]] { assert!(parse(args.iter().map(|s| s.to_string())).is_err()) }
        assert!(parse(["--square"].into_iter().map(String::from)).unwrap().square);
        let pv = |a: &[&str]| view(&parse(a.iter().map(|s| s.to_string())).unwrap());
        assert_eq!(pv(&["--scale", "3"]), View { n: 3, square: false, faithful: false });
        assert_eq!(pv(&[]), View { n: 0, square: false, faithful: false });
        assert!(parse(["--scale", "0"].into_iter().map(String::from)).is_err()); assert!(parse(["--bogus"].into_iter().map(String::from)).is_err());
        assert_eq!(resolve(&a, &d, None), PathBuf::from("X"));
        let none = Args::default();
        assert_eq!(resolve(&none, &d, Some("E".into())), PathBuf::from("E"));
        fs::create_dir(d.join("gamefiles")).unwrap();
        assert_eq!(resolve(&none, &d, Some("E".into())), d.join("gamefiles"));
        assert_eq!(resolve(&none, Path::new("nowhere"), None), Path::new("nowhere").join("gamefiles"));
        fs::remove_dir_all(&d).unwrap();
    }
}
#[cfg(test)]
#[test]
fn snapshot_flag_accepts_product_and_rejects_faithful() {
    let args=|| vec!["--snapshot".to_string(),"saved.json".into()];
    assert!(parse(args().into_iter()).is_ok());
    assert!(parse(args().into_iter().chain(["--scale".into(),"2".into(),"--square".into()])).is_ok());
    for other in [vec!["--faithful"],vec!["--mcp"],vec!["--viewer"],vec!["--ground"],
        vec!["--play","route.json"],vec!["--play","--replay","trace.jsonl"],vec!["--build-id"],vec!["--scenario","test.toml"]] {
        assert!(parse(args().into_iter().chain(other.into_iter().map(str::to_owned))).is_err());
    }
    assert!(parse(["--snapshot".into()].into_iter()).is_err());
}
