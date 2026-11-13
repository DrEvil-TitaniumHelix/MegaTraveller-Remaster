use super::*;
use mt_engine::{data::{exepack, ly, pic, Variant}, io::{Dir, Files, Overlay}, sim::tick::{Clock, Counter}};
use mt1::{ground::{self, Ground}, screens::{cg::Screens, title::{Title, TcRand}}, ui::SceneHook};

pub(super) fn fingerprint(dir: &Path) -> Result<String, String> {
    let files = Dir(dir.into()); let mut h = mt_engine::play::Fnv::default();
    for name in mt1::REQUIRED_FILES {
        let bytes = files.read_limited(name, 16 << 20).map_err(|e| format!("gamefiles {name}: {e}"))?;
        h.field(name.as_bytes()); h.field(&bytes);
    }
    Ok(h.state())
}
fn files(dir: &Path) -> Box<dyn Files> { let mut f = Overlay::new(Some(dir)); f.hide("UCHARS.DAT"); Box::new(f) }
fn read(dir: &Path, name: &str) -> Result<Vec<u8>, String> { Dir(dir.into()).read_limited(name, 16 << 20).map_err(|e| format!("{name}: {e}")) }
fn squad() -> Result<mt1::ground::squad::State, String> {
    let compendium = mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml"))?;
    Ok(mt1::ground::squad::State::with_compendium(compendium))
}
pub(super) fn validate(root: &Root, dir: &Path) -> Result<(), String> {
    if root.gamefiles != fingerprint(dir)? { return Err("snapshot gamefiles digest differs from these assets".into()) }
    if let Some(bytes) = &root.image { mt1::save::Saved::decode(bytes)?; }
    if root.kind == RootKind::NewGame {
        if let Some(path) = root.scenario.as_deref().and_then(|s| s.strip_prefix("scenario:")) {
            crate::scenario::load(Path::new(path), dir)?.require_playable()?;
        }
    }
    Ok(())
}
pub(super) fn anchor_ready(image:&[u8],loaded:Option<(u8,u8,i8)>)->bool {
    let Some(loaded)=loaded else{return false};
    let Ok(saved)=mt1::save::Saved::decode(image) else{return false};
    let mem=ground::state::Mem{ds:saved.ds,segs:saved.segs,bios_ticks:saved.bios_ticks,trace:Default::default()};
    loaded==(mem.party_u8(6),mem.party_u8(7),mem.level())
}
struct Loader { inner:crate::scenario::Loader, observed:Rc<RefCell<Observed>> }
impl Loader {
    fn remember(&self) {self.observed.borrow_mut().extensions=serde_json::to_vec(&self.inner.extensions).expect("serializable scenario continuation");}
}
impl ground::Tooling for Loader {
    fn poll(&mut self,g:&mut Ground)->bool {
        let changed=self.inner.poll(g); self.remember();
        self.observed.borrow_mut().loaded=Some((g.mem.party_u8(6),g.mem.party_u8(7),g.mem.level()));changed
    }
    fn space_launched(&mut self,sp:&mut mt1::space::Space)->Result<(),String> {let result=self.inner.space_launched(sp);self.remember();result}
    fn space_seed(&mut self,prog:&str)->Option<u16> {let seed=self.inner.space_seed(prog);self.remember();seed}
}
pub(super) fn attach(g:&mut Ground,dir:&Path,extensions:crate::scenario::Extensions,observed:Rc<RefCell<Observed>>,exact:bool) {
    let mut inner=if exact {
        let location=(g.mem.party_u8(6),g.mem.party_u8(7),g.mem.level());
        observed.borrow_mut().loaded=Some(location);
        let mut inner=crate::scenario::Loader::resume_at(dir.into(),location);inner.extensions=extensions;inner
    } else {crate::scenario::Loader::new(Default::default(),dir.into(),extensions)};
    if let Some(request)=&observed.borrow().desktop {inner.desktop_request(request.clone());}
    let loader=Loader{inner,observed};loader.remember();g.tooling=Some(Box::new(loader));
}
pub(super) fn run(root: &Root, dir: &Path, exe_dir: &Path, input: Box<dyn Input>, observed:Rc<RefCell<Observed>>) -> Result<(), String> {
    run_session(root,dir,exe_dir,input,observed).map(|_|())
}
pub(super) fn run_session(root: &Root, dir: &Path, exe_dir: &Path, mut input: Box<dyn Input>, observed:Rc<RefCell<Observed>>) -> Result<Option<Ground>, String> {
    mt_engine::present::textlog::set_enabled(true);
    let mut clock: Box<dyn Clock> = input.play_clock(Box::new(Counter::at(root.start_tick)));
    if root.kind != RootKind::NewGame {
        let mut saved = mt1::save::Saved::decode(root.image.as_deref().ok_or("missing save image")?)?;
        let space=if root.kind==RootKind::Anchor {saved.play.as_mut().and_then(|m|m.space.take())} else {None};
        let parent=space.as_ref().map(|_|saved.encode());
        clock = input.play_clock(Box::new(Counter::at(saved.ticks)));
        let host:HostState=saved.play.as_ref().filter(|m|!m.host.is_empty()).map(|m|serde_json::from_slice(&m.host).map_err(|e|format!("play: invalid host continuation: {e}"))).transpose()?.unwrap_or_default();
        observed.borrow_mut().story=host.story;
        observed.borrow_mut().messages=host.messages;
        let mut g = if root.kind == RootKind::Anchor {
            mt1::save::ground_exact(saved, input, clock, files(dir))?
        } else { mt1::save::ground(saved, input, clock, files(dir)) };
        g.scene = Some(SceneHook::default());
        if g.squad.is_none() { g.squad = Some(squad()?); }
        attach(&mut g,dir,host.extensions,observed,root.kind==RootKind::Anchor);
        if let Some(space)=space {
            if mt1::space::resume_exact(&mut g,parent.unwrap(),space)?==mt1::space::Exit::Land {mt1::ground::main_loop::run(&mut g);}else{g.game_over=true;}
            return Ok(Some(g))
        }
        if root.kind == RootKind::Anchor {
            mt1::ground::main_loop::resume_exact(&mut g)?;
        } else {
            mt1::ground::main_loop::run(&mut g);
        }
        return Ok(Some(g))
    }
    let start = root.scenario.as_deref().unwrap_or("quickstart");
    if let Some(path) = start.strip_prefix("scenario:") {
        let prepared = crate::scenario::load(Path::new(path), dir)?; let mut g = prepared.game;
        g.input = input; g.clock = clock; g.scene = Some(SceneHook::default());
        if g.squad.is_none() { g.squad = Some(squad()?); }
        attach(&mut g,dir,prepared.extensions,observed,false);
        mt1::ground::main_loop::run(&mut g); return Ok(Some(g))
    }
    let (mut input, title) = (input, start == "boot");
    let block = loop {
        if title {
            let exe = exepack::unpack(&read(dir, "STARTUP.EXE")?).map_err(|e| format!("STARTUP: {e:?}"))?.exe;
            let mut title = Title::new(&exe, ly::decode(&read(dir, "TITLE.LY")?).map_err(|e| format!("TITLE.LY: {e:?}"))?,
                ly::decode(&read(dir, "FONT.LY")?).map_err(|e| format!("FONT.LY: {e:?}"))?,
                pic::decode(Variant::Mt1, &read(dir, "TIT.PIC")?).map_err(|e| format!("TIT.PIC: {e:?}"))?,
                pic::decode(Variant::Mt1, &read(dir, "ZOD.PIC")?).map_err(|e| format!("ZOD.PIC: {e:?}"))?).ok_or("invalid title assets")?;
            let mut rng = TcRand { seed: clock.now() as u16 as u32 };
            if title.run(&mut || rng.next(), clock.as_mut(), input.as_mut(), |t, i| i.present(&t.fb.rgb(t.display, &mt_engine::present::palette::ega_dac()))) == mt1::screens::title::Exit::Quit { return Ok(None) }
        }
        if start == "quickstart" { break read(dir, "DEFAULT.SAV")? }
        let exe = exepack::unpack(&read(dir, "CG.DAT")?).map_err(|e| format!("CG: {e:?}"))?.exe;
        let mut s = Screens::new(exe, ly::decode(&read(dir, "FONT.LY")?).map_err(|e| format!("FONT.LY: {e:?}"))?,
            ly::decode(&read(dir, "CG.LY")?).map_err(|e| format!("CG.LY: {e:?}"))?, input, clock, files(dir)).ok_or("invalid CG assets")?;
        s.snapshots = true; let tick = s.ticks() as u16; s.rng.srand(tick);
        let (exit, party, picked) = loop {
            let (exit, party) = mt1::screens::cg::run(&mut s);
            if exit != mt1::chargen::pool::Exit::Continue { break (exit, party, None) }
            let Some(dir) = s.input.live_save_location().map(|(d,_)| d.to_path_buf()) else { break (exit, party, None) };
            if let Some(saved) = mt1::save::continue_old(&mut s, &dir) { break (exit, party, Some(saved)) }
        };
        let block = match exit {
            mt1::chargen::pool::Exit::Play => ground::party::block(&ground::party::initial(), &party, &s.ds, &mut s.rng).ok_or("invalid generated party")?,
            mt1::chargen::pool::Exit::Quick => read(dir, "DEFAULT.SAV")?,
            mt1::chargen::pool::Exit::Continue => match picked {
                None => return Err(format!("Use game_load with a remaster save path (default: {})", exe_dir.join("saves/game1.mtsav").display())),
                Some(saved) => {
                    let mut g = mt1::save::ground(saved, s.input, s.clock, files(dir));
                    g.mouse_driver = true; g.scene = Some(SceneHook::default()); g.squad = Some(squad()?);
                    attach(&mut g,dir,Default::default(),observed,false);
                    mt1::ground::main_loop::run(&mut g); return Ok(Some(g))
                }
            },
            mt1::chargen::pool::Exit::Dos => return Ok(None),
    };
    input = s.input; clock = s.clock; break block
    };
    let mut g = Ground::new(&read(dir, "T.DAT")?, block, input, clock, files(dir)).ok_or("invalid T.DAT or party block")?;
    g.mouse_driver = true; g.scene = Some(SceneHook::default()); g.squad = Some(squad()?);
    attach(&mut g,dir,Default::default(),observed,false);
    ground::run(&mut g); Ok(Some(g))
}
