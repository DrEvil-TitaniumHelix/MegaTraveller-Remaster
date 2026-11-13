pub mod state;
pub mod tick;
pub mod system;
pub mod assets;
pub mod encounters;
pub mod render;
pub mod flight;
pub mod input;
pub mod combat;
pub mod tactical;
mod effects;
pub mod guns;
pub mod boarding;
pub mod sounds;
use crate::ground::{self, Ground};
pub mod jump;
pub mod jump_render;
pub mod jump_host;
pub mod select;
use crate::lockstep::SpaceObserver;
use mt_engine::{io::{Files, Input}, sim::tick::Clock};
use state::*;
pub mod stations;
pub mod ui;
pub mod frame;
pub mod host;
pub mod mouse;
pub mod play;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit { Land, GameOver }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Events { pub reads: u32, pub rands: u32, pub minimap: u32, pub cursor: u32, pub view: u32, pub copies: u32, pub minimap_returns: u32 }

pub struct Space {
    pub s: State,
    pub r: render::Render,
    pub clock: Box<dyn Clock>,
    pub input: Box<dyn Input>,
    pub files: Box<dyn Files>,
    pub sounds: sounds::Sounds,
    pub observer: Option<Box<dyn SpaceObserver>>,
    pub hooks: Option<Box<dyn tick::Hooks>>,
    pub ev: Events,
    pub last_ticks: u32,
    pub screen: &'static str,
    pub dac: mt_engine::present::palette::Dac,
    pub product: bool,
    pub hud_mode: i16,
    pub switch_to: Option<i32>,
    pub dead: bool,
    pub scenario_entry: Option<String>,
    pub tooling: Option<Box<dyn ground::Tooling>>,
    pub mouse: mouse::Driver,
    pub(crate) play_command_poll: bool,
    pub(crate) play_time_wait: bool,
    pub(crate) play_labels: Vec<String>,
    pub(crate) gun_note: String,
    pub(crate) jump_asked: Option<u8>,
    pub(crate) play_back: Option<[i32; 4]>,
    pub(crate) play_tactical: bool,
    pub(crate) play_rects: Vec<[i32; 4]>,
    pub(crate) play_ground: Option<Vec<u8>>,
    pub(crate) play_flight: Option<flight::Flight>,
}
impl Space {
    pub fn new(s_dat: &[u8], party: Vec<u8>, world: Vec<u8>, input: Box<dyn Input>, clock: Box<dyn Clock>, files: Box<dyn Files>) -> Result<Self, String> {
        let s = State::new(s_dat, party, world)?;
        let assets = assets::Assets::load(files.as_ref())?;
        let r = render::Render::new(&s, assets)?;
        let sounds = sounds::Sounds::new(&s);
        Ok(Self { s, r, clock, input, files, sounds, observer: None, hooks: None, ev: Events::default(), last_ticks: 0, screen: "s", dac: mt_engine::present::palette::ega_dac(), product: false, hud_mode: -1, switch_to: None, dead: false, scenario_entry: None, tooling: None, mouse: mouse::Driver::default(), play_command_poll: false, play_time_wait: false, play_labels: Vec::new(), gun_note: String::new(), play_back: None, play_tactical: false, jump_asked: None, play_rects: Vec::new(),play_ground:None,play_flight:None })
    }
    pub fn space_main(&mut self, options: &[u8]) -> Exit {
        self.s.set_word(PARTY_PTR, 0); self.s.set_word(PARTY_PTR + 2, PARTY_SEG as i16);
        self.s.set_word(WORLD_PTR, 0); self.s.set_word(WORLD_PTR + 2, WORLD_SEG as i16);
        for tube in 0..4 { self.s.set_byte(combat::MISSILES + tube, 0); }
        let mut assignments = [-1i16; 5];
        for i in 0..6u16 { if self.s.pbyte((i << 8) + 0x127) <= self.s.pbyte((i << 8) + 0x126) { if let Some(a) = assignments.get_mut(i as usize) { *a = -2; } } }
        for i in 0..6u16 { if self.s.pbyte((i << 8) + 0x127) == 0 { if let Some(a) = assignments.get_mut(i as usize) { *a = -3; } } }
        for i in 0..12u16 {
            let station = self.s.pbyte(i + 0xA0);
            if station != 0 { let crew = station as usize - 1; if let Some(a) = assignments.get_mut(crew) { if *a == -1 { *a = i as i16; } } }
        }
        for i in 0..12u16 { self.s.set_pbyte(i + 0xA0, 0); }
        for i in 0..5 { if assignments[i] >= 0 { self.s.set_pbyte(assignments[i] as u16 + 0xA0, i as u8 + 1); } }
        let mut crew = 8u16;
        for i in 0..5 { if assignments[i] == -2 { while self.s.pbyte(crew + 0xA0) != 0 { crew += 1; } self.s.set_pbyte(crew + 0xA0, i as u8 + 1); crew += 1; } }
        for i in 0..5 { if assignments[i] == -1 { for c in 0..11u16 { if self.s.pbyte(c + 0xA0) == 0 { self.s.set_pbyte(c + 0xA0, i as u8 + 1); break; } } } }
        let mut options = options.to_vec(); options.resize(2, b'A');
        if options[0] == b'3' { self.s.set_byte(0x3A03, 1); options[0] = b'0'; }
        let system = self.s.pbyte(6) as i16;
        let r = self.rand() % 2;
        self.s.set_word(SHIP_COUNT, self.s.array(DANGER, system).wrapping_sub(r));
        self.s.set_word(WORLD_TIME, self.s.pword(0xF84));
        if let Err(e) = system::load(&mut self.s, self.files.as_mut(), system as u8) { eprintln!("{e}"); return Exit::GameOver; }
        self.r.initialize(&mut self.s);
        let t = self.read();
        let seed = match self.observer.take() { Some(mut o) => { let v = o.srand(self); self.observer = Some(o); v } None => None }
            .or_else(|| self.tooling.as_mut().and_then(|t| t.space_seed("S"))).unwrap_or(t as u16);
        self.s.srand(seed);
        self.event(crate::lockstep::SpaceEvent::Read);
        if options[1] != b'A' {
            if options[1] == b'C' { self.s.set_word(0x1A28, -1); }
            self.s.set_byte(sounds::LOADED, 1);
            self.sounds.load(&self.s, self.files.as_ref());
        }
        let flag = self.s.world.get(0x1223).copied().unwrap_or(0); self.s.set_byte(0x4627, flag);
        self.initialize_space_module();
        if let Some(mut t) = self.tooling.take() {
            if let Err(e) = t.space_launched(self) { eprintln!("{e}"); return Exit::GameOver; }
            self.tooling = Some(t);
        }
        let result = self.load_space_station_layouts();
        self.finish_space(result)
    }
    fn finish_space(&mut self,result:i16)->Exit {
        self.s.set_pbyte(7, if result > 0 { (self.s.array(system::WORLD, result) as u8).wrapping_sub(1) } else { 0 });
        if result > -1 {
            self.return_missiles_to_supplies();
            for slot in 0..20u16 { self.s.set_pbyte(slot + 0xAC, self.s.pbyte(slot + 0xAC) & 0xBF); }
            return Exit::Land;
        }
        self.stop("exit");
        Exit::GameOver
    }
    fn initialize_space_module(&mut self) {
        for i in 0..15u16 { self.s.set_word(tick::ACTIVE + 2 * i, -1); }
        self.initialize_system_encounters();
        self.init_mouse();
    }
    fn load_space_station_layouts(&mut self) -> i16 {
        self.frame();
        match self.scenario_entry.take().as_deref() {
            None | Some("flight") => {}
            Some("jump") => self.s.set_byte(KEY, 0x24),
            Some("station") => self.s.set_byte(KEY, 0x1F),
            Some(other) => { eprintln!("scenario: unknown space_entry `{other}`"); return -1; }
        }
        self.in_system_flight()
    }
    pub fn confirm_quit(&mut self) -> i16 {
        self.drop_stale_clicks();
        self.hide_cursor();
        self.r.back(&mut self.s);
        self.s.set_word(0x4868, 0);
        for (at, v) in [(0x49D2, 0), (0x49BA, 0), (0x49C6, 200), (0x49BC, 40)] { self.s.set_word(at, v); }
        self.hud(-2);
        let labels: Vec<(i16, i16, Option<Vec<u8>>, u8)> = (0..).map(|i| 0xD67u16 + 9 * i).take_while(|&at| self.s.byte(at + 8) != 255)
            .map(|at| (self.s.word(at), self.s.word(at + 2), self.s.pointer_string(at + 4), self.s.byte(at + 8))).collect();
        for (y, x, value, color) in &labels { if let Some(v) = value { self.r.text(&self.s, v, *y, *x, *color); } }
        let (a, b) = (self.s.string(0x178E), self.s.string(0x179F));
        self.r.text(&self.s, &a, 178, 137, self.s.byte(0x4994));
        self.r.text(&self.s, &b, 184, 137, self.s.byte(0x4994));
        self.r.flip(&mut self.s);
        self.show_cursor();
        play::confirm(self);
        loop {
            self.s.set_byte(KEY, 0);
            while self.s.byte(KEY) == 0 { self.update_cursor(); }
            let mut code = self.s.byte(KEY);
            if code == 0x6E {
                self.mouse_command(mouse::FLIGHT_HITS);
                code = match self.s.byte(KEY) { 0x1F => 0x15, 0x21 => 0x31, _ => continue };
            }
            if code == 0x15 { return 27; }
            if code == 0x31 || code == 1 { return b'N' as i16; }
        }
    }
    pub fn ship_destroyed(&mut self, result: i16) {
        self.r.front(&mut self.s);
        let color = if self.s.word(VIDEO_MODE) == 2 { 0 } else { 3 };
        self.s.set_byte(0x4994, color);
        self.r.fill(&self.s, 0, 0, 200, 320, 9);
        let (a, b) = (self.s.string(0x19C3), self.s.string(0x19DE));
        self.r.text(&self.s, &a, 100, 0, color);
        self.r.text(&self.s, &b, 110, 0, color);
        play::death(self);
        self.translate_station_key();
        self.s.set_pbyte(7, result as u8);
        self.stop("death");
        self.dead = true;
    }
    pub fn stop(&mut self, reason: &str) {
        if let Some(mut o) = self.observer.take() { o.stop(self, reason); self.observer = Some(o); }
    }
}

pub fn handoff(g: &mut Ground, entry: impl FnOnce(&mut [u8]) -> Result<Exit, String>) -> Result<Exit, String> {
    let pointer = g.mem.pointer(ground::state::PARTY_PTR);
    let party = g.mem.segs.get_mut(&pointer.seg)
        .and_then(|b| b.get_mut(pointer.off as usize..pointer.off as usize + ground::state::Mem::PARTY_SIZE))
        .ok_or("space: invalid shared party block")?;
    let exit = entry(party)?;
    if exit == Exit::Land { ground::return_from_space(g)?; }
    Ok(exit)
}
pub fn launch(g: &mut Ground) -> Result<Exit, String> {
    let s_dat = g.files().read("S.DAT").ok_or("launch: missing S.DAT")?;
    ground::game_shutdown(g);
    let parent=if g.input.data_mode() {crate::play::anchors::launch_parent(g).ok().map(|saved|saved.encode())} else {None};
    let world = g.mem.segs.get(&ground::state::Mem::PARTY_SEG.wrapping_add(0x100)).cloned().unwrap_or_default();
    let sound = g.sound_option;
    let observer = match g.lockstep.take() { Some(mut o) => { o.checkpoint(g, "launch"); let s = o.space(g); g.lockstep = Some(o); s } None => None };
    let input = std::mem::replace(&mut g.input, Box::new(mt_engine::io::Scripted::new([])));
    let clock = std::mem::replace(&mut g.clock, Box::new(mt_engine::sim::tick::Counter::default()));
    let files = std::mem::replace(&mut g.files, Box::new(mt_engine::io::Overlay::new(None)));
    let party = g.mem.party().to_vec();
    let mut sp = match Space::new(&s_dat, party, world, input, clock, files) {
        Ok(sp) => sp,
        Err(e) => return Err(e),
    };
    sp.observer = observer;
    sp.product = g.scene.is_some();
    sp.tooling = g.tooling.take();
    sp.play_ground=parent;
    let exit = sp.space_main(&[b'0', sound]);
    complete_launch(g,sp,exit)
}
pub fn resume_exact(g:&mut Ground,parent:Vec<u8>,image:play::image::Image)->Result<Exit,String> {
    let s_dat=g.files().read("S.DAT").ok_or("launch: missing S.DAT")?;
    let input=std::mem::replace(&mut g.input,Box::new(mt_engine::io::Scripted::new([])));
    let clock=std::mem::replace(&mut g.clock,Box::new(mt_engine::sim::tick::Counter::default()));
    let files=std::mem::replace(&mut g.files,Box::new(mt_engine::io::Overlay::new(None)));
    let mut sp=Space::new(&s_dat,image.party.clone(),image.world.clone(),input,clock,files)?;
    image.install(&mut sp)?;sp.play_ground=Some(parent);sp.tooling=g.tooling.take();
    let result=sp.resume_flight(image.flight);let exit=sp.finish_space(result);
    complete_launch(g,sp,exit)
}
fn complete_launch(g:&mut Ground,sp:Space,exit:Exit)->Result<Exit,String> {
    let Space { s, input, clock, files, observer, tooling, .. } = sp;
    g.input = input; g.clock = clock; g.files = files; g.tooling = tooling;
    if let (Some(mut o), Some(obs)) = (g.lockstep.take(), observer) { o.space_returned(g, obs); g.lockstep = Some(o); }
    handoff(g, |party| { party.copy_from_slice(&s.party); Ok(exit) })
}
