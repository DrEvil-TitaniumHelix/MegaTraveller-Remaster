use super::trace::Trace;
use crate::screens::title::TcRand;
use mt_engine::data::exepack;
use mt_engine::io::{dump::{Step, Text}, mouse::MouseEvent, scan, Event, Files, Input, Key, Overlay};
use mt_engine::present::{fb::Fb, palette::{self, Dac}};
use mt_engine::sim::tick::Clock;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Far { pub off: u16, pub seg: u16 }
impl Far {
    pub const fn new(off: u16, seg: u16) -> Far { Far { off, seg } }
    pub fn add(self, n: i32) -> Far { Far { off: self.off.wrapping_add(n as u16), seg: self.seg } }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Write { pub seg: u16, pub off: u16, pub bytes: Vec<u8> }
pub struct Mem {
    pub ds: Vec<u8>, pub segs: BTreeMap<u16, Vec<u8>>, pub trace: Trace,
    pub bios_ticks: [u8; 4],
}
impl Mem {
    pub const DS: u16 = 0x3062;
    pub const BIOS_TICKS: Far = Far::new(0x6C, 0x40);
    fn bios_index(p: Far) -> Option<usize> { (p.seg == Self::BIOS_TICKS.seg && (Self::BIOS_TICKS.off..Self::BIOS_TICKS.off + 4).contains(&p.off)).then(|| (p.off - Self::BIOS_TICKS.off) as usize) }
    pub const PARTY_SEG: u16 = 0x9000;
    pub const PARTY_SIZE: usize = 0xF86;
    pub fn trace_enter(&mut self, name: &'static str, args: &[u16]) {
        if !self.trace.on { return }
        self.trace.enter(name, args);
        self.trace.capture_entry(std::iter::once((Self::DS, 0, self.ds.as_slice())).chain(self.segs.iter().filter(|(s, _)| **s != Self::DS).map(|(&s, b)| (s, 0, b.as_slice())))
            .chain(std::iter::once((Self::BIOS_TICKS.seg, Self::BIOS_TICKS.off, self.bios_ticks.as_slice()))));
    }
    fn block(&self, seg: u16) -> Option<&[u8]> { if seg == Self::DS { Some(&self.ds) } else { self.segs.get(&seg).map(Vec::as_slice) } }
    fn block_mut(&mut self, seg: u16) -> Option<&mut Vec<u8>> { if seg == Self::DS { Some(&mut self.ds) } else { self.segs.get_mut(&seg) } }
    pub fn alloc(&mut self, seg: u16, size: usize) { self.segs.insert(seg, vec![0; size]); }
    pub fn read(&self, p: Far, width: usize) -> u32 {
        let b = self.block(p.seg); let mut v = 0u32;
        for i in (0..width.min(4)).rev() {
            let at = p.add(i as i32);
            let byte = match Self::bios_index(at) { Some(k) => self.bios_ticks[k], None => b.and_then(|b| b.get(at.off as usize)).copied().unwrap_or(0) };
            v = (v << 8) | byte as u32
        }
        v
    }
    pub fn read_i(&self, p: Far, width: usize) -> i32 { let v = self.read(p, width); match width { 1 => v as u8 as i8 as i32, 2 => v as u16 as i16 as i32, _ => v as i32 } }
    pub fn write(&mut self, p: Far, value: u32, width: usize) {
        let bytes: Vec<u8> = (0..width.min(4)).map(|i| (value >> (8 * i)) as u8).collect();
        self.write_bytes(p, &bytes)
    }
    pub fn bytes(&self, p: Far, n: usize) -> Vec<u8> { (0..n).map(|i| self.read(p.add(i as i32), 1) as u8).collect() }
    pub fn write_bytes(&mut self, p: Far, bytes: &[u8]) {
        self.trace.write(p.seg, p.off, bytes);
        for (i, &v) in bytes.iter().enumerate() {
            let at = p.add(i as i32);
            if let Some(k) = Self::bios_index(at) { self.bios_ticks[k] = v }
            else if let Some(x) = self.block_mut(p.seg).and_then(|b| b.get_mut(at.off as usize)) { *x = v }
        }
    }
    pub fn cstr(&self, p: Far) -> String { (0..65536).map(|i| self.read(p.add(i), 1) as u8).take_while(|&b| b != 0).map(|b| b as char).collect() }
    pub fn ds(off: u16) -> Far { Far::new(off, Self::DS) }
    pub fn u8(&self, off: u16) -> u8 { self.read(Self::ds(off), 1) as u8 }
    pub fn i8(&self, off: u16) -> i8 { self.u8(off) as i8 }
    pub fn u16(&self, off: u16) -> u16 { self.read(Self::ds(off), 2) as u16 }
    pub fn i16(&self, off: u16) -> i16 { self.u16(off) as i16 }
    pub fn u32(&self, off: u16) -> u32 { self.read(Self::ds(off), 4) }
    pub fn set_u8(&mut self, off: u16, v: u8) { self.write(Self::ds(off), v as u32, 1) }
    pub fn set_u16(&mut self, off: u16, v: u16) { self.write(Self::ds(off), v as u32, 2) }
    pub fn set_i16(&mut self, off: u16, v: i16) { self.set_u16(off, v as u16) }
    pub fn set_u32(&mut self, off: u16, v: u32) { self.write(Self::ds(off), v, 4) }
    pub fn pointer(&self, off: u16) -> Far { Far::new(self.u16(off), self.u16(off.wrapping_add(2))) }
    pub fn set_pointer(&mut self, off: u16, p: Far) { self.set_u16(off, p.off); self.set_u16(off.wrapping_add(2), p.seg) }
    pub fn party(&self) -> &[u8] { self.segs.get(&Self::PARTY_SEG).map(Vec::as_slice).unwrap_or(&[]) }
    pub fn party_u8(&self, off: u16) -> u8 { self.read(self.pointer(PARTY_PTR).add(off as i32), 1) as u8 }
    pub fn party_u16(&self, off: u16) -> u16 { self.read(self.pointer(PARTY_PTR).add(off as i32), 2) as u16 }
    pub fn set_party_u8(&mut self, off: u16, v: u8) { let p = self.pointer(PARTY_PTR).add(off as i32); self.write(p, v as u32, 1) }
    pub fn set_party_u16(&mut self, off: u16, v: u16) { let p = self.pointer(PARTY_PTR).add(off as i32); self.write(p, v as u32, 2) }
    pub fn direction(&self, i: u16) -> (i16, i16, i16) { let b = DIRECTIONS.wrapping_add(i.wrapping_mul(3)); (self.i8(b) as i16, self.i8(b.wrapping_add(1)) as i16, self.i8(b.wrapping_add(2)) as i16) }
    pub fn walker(i: i16) -> Far { Self::ds(WALKERS.wrapping_add((i as u16).wrapping_mul(WALKER_SIZE))) }
    pub fn npc(i: i16) -> Far { Self::ds(NPCS.wrapping_add((i as u16).wrapping_mul(NPC_SIZE))) }
    pub fn sound_data(&self,i:u16)->Vec<u8> {
        if i>=19 {return Vec::new()}
        let p=self.pointer(0x6F4C+4*i);let n=self.u32(0x6F00+4*i) as usize;
        if p==Far::default() {return Vec::new()}
        if n>64000 || p.off as usize+n>65536 {eprintln!("ground: invalid VMF evidence size");std::process::exit(3)}
        self.bytes(p,n)
    }
}
macro_rules! globals { ($($name:ident / $set:ident : $off:literal $w:literal $t:ty;)*) => {
    impl Mem { $(
        #[doc = concat!("DS:", stringify!($off))]
        pub fn $name(&self) -> $t { self.read_i(Self::ds($off), $w) as $t }
        pub fn $set(&mut self, v: $t) { self.write(Self::ds($off), v as u32, $w) }
    )* }
} }
globals! {
    orders / set_orders: 0x94 1 u8; heading / set_heading: 0xE1 2 i16; countdown / set_countdown: 0xE7 1 u8; level / set_level: 0xE8 1 i8;
    event / set_event: 0xE9 1 i8; encounter_mode / set_encounter_mode: 0x6C3B 1 u8; individual / set_individual: 0x6C42 1 u8;
    npc_mode / set_npc_mode: 0x6D11 1 u8; combat / set_combat: 0x6D0E 2 i16; leader / set_leader: 0x6B6A 2 i16; x / set_x: 0x6B6C 2 i16;
    y / set_y: 0x63F5 2 i16; view_x / set_view_x: 0x6415 2 i16; view_y / set_view_y: 0x6266 2 i16; phase / set_phase: 0x641A 2 i16;
    event_id / set_event_id: 0x6B73 2 i16; npc_count / set_npc_count: 0x6418 1 u8; npc_attacking / set_npc_attacking: 0x6D1D 1 u8;
    party_fired / set_party_fired: 0x6D18 1 u8; provoked / set_provoked: 0x6B76 1 u8; map_stride / set_map_stride: 0x6D2B 2 i16;
    outdoors / set_outdoors: 0x6D20 1 u8; atmosphere / set_atmosphere: 0x5E16 1 u8; air_warning / set_air_warning: 0x6B63 1 u8;
    shot_sound / set_shot_sound: 0x6C3F 1 i8; extra_sound / set_extra_sound: 0x6B91 1 i8; impact_sound / set_impact_sound: 0x6C2E 1 u8;
    turning / set_turning: 0x6D22 1 u8; previous_x / set_previous_x: 0x6C27 2 i16; previous_y / set_previous_y: 0x6420 2 i16;
    move_delay / set_move_delay: 0x6C40 2 i16; encounter / set_encounter: 0x6B56 1 u8; pending_event / set_pending_event: 0x6D2D 1 u8;
    saved_mode / set_saved_mode: 0x6D30 1 u8; move_back / set_move_back: 0x6D10 1 u8; update_event / set_update_event: 0x6D32 1 u8;
    event_value / set_event_value: 0x6D21 1 u8; key / set_key: 0x6FE4 1 u8; cursor_visible / set_cursor_visible: 0x7011 1 u8;
    redraw_pending / set_redraw_pending: 0x6D17 1 u8; sound_enabled / set_sound_enabled: 0x6D0B 1 u8; arik / set_arik: 0x6D0C 2 i16;
    message_deadline / set_message_deadline: 0x6B92 4 u32; target_deadline / set_target_deadline: 0x6D19 4 u32;
    world_pending / set_world_pending: 0xB7 1 u8; debug / set_debug: 0x6C29 1 u8; selected_char / set_selected_char: 0x6B8F 1 u8;
    ship_count / set_ship_count: 0x6B96 1 u8; cheat / set_cheat: 0xB6 1 u8; panel_flag / set_panel_flag: 0x292 1 u8;
    pursuer_active / set_pursuer_active: 0x6B75 1 u8; pursuer_index / set_pursuer_index: 0x6B90 1 u8;
    vehicle_condition / set_vehicle_condition: 0x6D23 1 u8; interior_kind / set_interior_kind: 0x6FB3 1 u8;
    map_blocks_high / set_map_blocks_high: 0x5C3A 1 u8;
}
pub const PARTY_PTR: u16 = 0x6D65; pub const MAP_PTR: u16 = 0xBC; pub const DIRECTIONS: u16 = 0x95;
pub const MOUSE_WORDS: u16 = 0x6FF8; pub const HOTSPOTS_GROUND: u16 = 0x0120; pub const HOTSPOTS_GROUND_N: usize = 20; pub const HOTSPOTS_CHAR: u16 = 0x3E41; pub const HOTSPOTS_CHAR_N: usize = 43;
pub const WALKERS: u16 = 0x5C54; pub const WALKER_SIZE: u16 = 86; pub const NPCS: u16 = 0x63F4; pub const NPC_SIZE: u16 = 46;
pub const W_ACTIVE: i32 = 0; pub const W_FACING: i32 = 1; pub const W_HAND: i32 = 2; pub const W_BUILTIN: i32 = 3; pub const W_WEAPON_CHOICE: i32 = 4; pub const W_TARGET: i32 = 5; pub const W_TIMER: i32 = 6;
pub const W_X: i32 = 8; pub const W_Y: i32 = 10; pub const W_AIM_X: i32 = 12; pub const W_AIM_Y: i32 = 14; pub const W_WAYPOINT_X: i32 = 16; pub const W_WAYPOINT_Y: i32 = 18; pub const W_PHASE: i32 = 20; pub const W_ROUTE_LENGTH: i32 = 22;
pub const N_LEVEL: i32 = 2; pub const N_HOME_X: i32 = 3; pub const N_HOME_Y: i32 = 5; pub const N_MOVEMENT: i32 = 7; pub const N_AGGRESSION: i32 = 10; pub const N_ATTITUDE: i32 = 13; pub const N_ARMED: i32 = 31;
pub const N_X: i32 = 32; pub const N_Y: i32 = 34; pub const N_FACING: i32 = 36; pub const N_STEPS: i32 = 37; pub const N_REACTIVE: i32 = 38; pub const N_TURN: i32 = 39; pub const N_ANIMATION: i32 = 40; pub const N_WEAPON: i32 = 42;

#[derive(Clone, Debug, Default)]
pub struct Rand { pub lcg: TcRand, pub trace: Vec<u16>, pub force: Option<u16>, pub recording: bool }
impl Rand {
    pub fn srand(&mut self, seed: u32) { self.lcg.seed = self.force.map_or(seed, |f| f as u32) }
    pub fn rand(&mut self) -> u16 {
        if self.recording && self.trace.len() >= 1 << 20 {
            eprintln!("ground: runaway: RNG trace exceeds 1048576 draws without a step boundary");
            std::process::exit(3)
        }
        let v = self.lcg.next() as u16;
        if self.recording { self.trace.push(v) }
        v
    }
    pub fn random(&mut self, n: u16) -> u16 {
        if n == 0 { 0 } else { (self.rand() as i32 % n as i16 as i32) as u16 }
    }
    pub fn take_trace(&mut self) -> Vec<u16> { std::mem::take(&mut self.trace) }
}
pub fn ds_image(t_dat: &[u8]) -> Option<Vec<u8>> {
    let exe = exepack::unpack(t_dat).ok()?.exe;
    let word = |at: usize| Some(u16::from_le_bytes([*exe.get(at)?, *exe.get(at + 1)?]));
    let header = word(8)? as usize * 16;
    let ds_start = 0x2062 * 16;
    let base = header + ds_start;
    if exe.len() <= base { return None }
    let mut ds = exe[base..].to_vec(); ds.resize(0x10000, 0);
    let relocations = word(6)? as usize;
    let table = word(0x18)? as usize;
    for i in 0..relocations {
        let at = word(table + i * 4)? as usize + word(table + i * 4 + 2)? as usize * 16;
        if let Some(off) = at.checked_sub(ds_start).filter(|&off| off + 1 < ds.len()) {
            let value = u16::from_le_bytes([ds[off], ds[off+1]]).wrapping_add(0x1000);
            ds[off..off+2].copy_from_slice(&value.to_le_bytes());
        }
    }
    Some(ds)
}
pub struct Ground {
    pub landing: bool,
    pub launch_pending: bool,
    pub game_over: bool,
    pub tooling: Option<Box<dyn super::Tooling>>,
    pub scenario_boot: bool,
    pub lockstep: Option<Box<dyn crate::lockstep::GroundObserver>>,
    pub sound_option: u8,
    pub mem: Mem, pub fb: Fb, pub dac: Dac, pub rand: Rand,
    pub input: Box<dyn Input>, pub clock: Box<dyn Clock>, pub files: Box<dyn Files>, pub overlay: Option<Overlay>,
    pub log: Vec<Text>, pub screen: &'static str,
    pub frames: Vec<(String, Vec<u8>)>, frame_n: usize,
    pub mouse_event: Option<MouseEvent>,
    pub(crate) boot_party: Option<Vec<u8>>,
    pub mouse_driver: bool,
    pub boot_diagnostics: bool,
    pub speaker_refresh: u8,
    pub boot_calibrating: bool,
    pub harness: bool,
    pub blocked_by: Option<u8>, pub waiting: Option<(u8, crate::ui::Cell)>,
    pub main_wait: bool,
    pub dark_told: bool,
    pub escapes: u32,
    pub fixture: Vec<(u16, u8)>,
    draw_depth: u32, draw_mark: u32,
    pub fault_delay: Option<(Fault, u32)>,
    pub scene: Option<crate::ui::SceneHook>,
    pub save_slot: Option<crate::save::Slot>, pub restore_pending: Option<crate::save::Saved>,
    pub squad: Option<super::squad::State>,
    pub balance: super::rules::balance::Options,
    pub bios_key: u16,
    pub loop_vars: super::loop_vars::LoopVars,
    pub play_resume: super::main_loop::Resume,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault { DelayEntry, DelayBody, ElapseNested }
pub const MAX_FRAMES: usize = 4096; pub const MAX_TRACE: usize = 1 << 20;
pub const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
impl Ground {
    pub fn tool_poll(&mut self) -> bool {
        let Some(mut tool) = self.tooling.take() else { return false };
        let replaced = tool.poll(self);
        self.tooling = Some(tool);
        replaced
    }
    pub fn new(t_dat: &[u8], party: Vec<u8>, input: Box<dyn Input>, clock: Box<dyn Clock>, files: Box<dyn Files>) -> Option<Ground> {
        if party.len() != Mem::PARTY_SIZE { return None }
        let ds = ds_image(t_dat)?;
        let pointer = ds[PARTY_PTR as usize..PARTY_PTR as usize + 4].to_vec();
        let mut g = Ground::from_ds(ds, vec![0; Mem::PARTY_SIZE], input, clock, files);
        g.mem.ds[PARTY_PTR as usize..PARTY_PTR as usize + 4].copy_from_slice(&pointer);
        g.rand.lcg.seed = g.mem.u32(0x5C20);
        g.fb.regs = std::array::from_fn(|i| i as u8);
        g.boot_party = Some(party);
        Some(g)
    }
    pub fn from_ds(ds: Vec<u8>, party: Vec<u8>, input: Box<dyn Input>, mut clock: Box<dyn Clock>, files: Box<dyn Files>) -> Ground {
        let mut mem = Mem { ds, segs: BTreeMap::new(), trace: Trace::default(), bios_ticks: clock.now().to_le_bytes() };
        mem.ds.resize(0x10000, 0); mem.segs.insert(Mem::PARTY_SEG, party);
        mem.set_pointer(PARTY_PTR, Far::new(0, Mem::PARTY_SEG));
        Ground { landing: false, launch_pending: false, game_over: false, tooling: None, scenario_boot: false, lockstep: None, sound_option: b'B', mem, fb: Fb::new(), dac: palette::ega_dac(), rand: Rand::default(), input, clock, files, overlay: None, log: vec![], screen: "t", frames: vec![], frame_n: 0, mouse_event: None, boot_party: None, mouse_driver: false, boot_diagnostics: false, speaker_refresh: 0, boot_calibrating: false, harness: false, blocked_by: None, waiting: None, main_wait: false, escapes: 0, dark_told: false, fixture: vec![], draw_depth: 0, draw_mark: 0, fault_delay: None, scene: None, save_slot: None, restore_pending: None, squad: None, balance: Default::default(), bios_key: 0, loop_vars: Default::default(), play_resume:Default::default() }
    }
    pub fn elapse_to(&mut self,tick:u32)->Result<(),String> {
        if self.harness && self.draw_depth>0 {return Err(format!("clock advance inside an open draw routine (depth {})",self.draw_depth))}
        self.elapse(tick,true)
    }
    fn elapse(&mut self,tick:u32,boundaries:bool)->Result<(),String> {
        if self.clock.ticks()>tick {return Err(format!("clock already past checkpoint ({} > {tick})",self.clock.ticks()))}
        while self.clock.ticks()<tick || (boundaries && self.input.boundary_due()) {
            let event=self.input.elapse_poll(self.clock.as_mut())?;
            self.sync_bios_clock();
            self.deliver_event(event);
        }
        Ok(())
    }
    fn checkpoint(&mut self,name:&str) { self.checkpoint_at(name,true) }
    fn checkpoint_at(&mut self,name:&str,boundaries:bool) {
        self.observe_boot(&format!("arrive_{name}"),&[]);
        if self.draw_depth>0 && self.clock.ticks()!=self.draw_mark {
            eprintln!("ground: {name}: clock advanced inside an open draw routine ({} at the last checkpoint, {} now)",self.draw_mark,self.clock.ticks());
            std::process::exit(3)
        }
        let result=self.input.clock_checkpoint(name).and_then(|tick|tick.map_or(Ok(()),|tick|self.elapse(tick,boundaries)));
        if let Err(e)=result {eprintln!("ground: {name}: {e}");std::process::exit(3)}
        self.draw_mark=self.clock.ticks();
        self.observe_boot(&format!("checkpoint_{name}"),&[]);
    }
    pub fn enter(&mut self, name: &str) {
        if !self.harness { return }
        let tick = self.clock.ticks();
        match self.fault_delay {
            Some((Fault::DelayEntry, t)) if t == tick => { self.fault_delay = None; self.poll() }
            Some((Fault::ElapseNested, t)) if t == tick && self.draw_depth > 0 => { self.fault_delay = None; let _ = self.elapse(tick + 1, true); }
            _ => {}
        }
        self.checkpoint_at(&format!("enter_{name}"),false);
        self.draw_depth += 1;
        if self.fault_delay == Some((Fault::DelayBody, self.clock.ticks())) { self.fault_delay = None; self.poll() }
    }
    pub fn phase_checkpoint(&mut self,name:&str)->bool {
        if !self.harness {return false}
        match self.input.take_checkpoint(name) {
            None => false,
            Some(tick) if tick==self.clock.ticks() => {self.observe_boot(&format!("checkpoint_{name}"),&[]);true}
            Some(tick) => {eprintln!("ground: {name}: measured at tick {tick}, reached at {}",self.clock.ticks());std::process::exit(3)}
        }
    }
    pub fn boot_checkpoint(&mut self,name:&str) {
        self.lockstep_checkpoint(name);
        if !self.harness || !self.boot_calibrating {return}
        self.checkpoint(name)
    }
    pub fn lockstep_checkpoint(&mut self,name:&str) {
        if !self.boot_calibrating { return }
        if let Some(mut observer) = self.lockstep.take() {
            observer.checkpoint(self, name); self.lockstep = Some(observer);
        }
    }
    pub fn now(&mut self) -> u32 { let now = self.clock.now(); self.boot_row("get_ticks", &[], now); now }
    pub fn sync_bios_clock(&mut self) {
        let now = self.clock.now();
        if self.mem.read(Mem::BIOS_TICKS, 4) != now { self.mem.write(Mem::BIOS_TICKS, now, 4) }
    }
    pub fn set_clock(&mut self, now: u32) { self.clock.set(now); self.sync_bios_clock() }
    pub fn observe_boot(&mut self, name: &str, args: &[i16]) {
        if !self.boot_diagnostics { return }
        let tick = self.clock.now(); self.boot_row(name, args, tick);
    }
    fn boot_row(&mut self, name: &str, args: &[i16], tick: u32) {
        if !self.boot_diagnostics || !self.harness || !self.input.streams_steps() { return }
        let row = format!("{{\"name\":{name:?},\"args\":{args:?},\"tick\":{tick},\"rng\":{},\"x\":{},\"y\":{},\"flag\":{},\"key\":{},\"phase\":{},\"page\":{}}}\n",
            self.rand.lcg.seed, self.mem.x(), self.mem.y(), self.mem.party_u8(0xC1),self.mem.key(),self.mem.phase(),self.mem.u16(0x52F4));
        self.input.stream_step_file("boot.jsonl", row.as_bytes(), true);
    }
    pub fn files(&mut self) -> &mut dyn Files { match &mut self.overlay { Some(o) => o, None => self.files.as_mut() } }
    pub fn message_text(&mut self, s: &str) {
        if self.harness { self.log_text(Text::Message(s.to_string())) }
        if self.input.data_mode() { self.input.scene(&Text::Message(s.to_string())); }
    }
    pub fn log_text(&mut self, item: Text) {
        if !self.harness { return }
        fn bytes(t: &Text) -> usize { match t {
            Text::Text(s) | Text::Message(s) => s.capacity(),
            Text::Menu(v) => v.capacity() * std::mem::size_of::<String>() + v.iter().map(String::capacity).sum::<usize>(),
        } }
        let cap = if self.log.len() == self.log.capacity() { self.log.capacity().saturating_mul(2).max(4) } else { self.log.capacity() };
        let used = self.log.iter().fold(cap * std::mem::size_of::<Text>(), |n, t| n.saturating_add(bytes(t)));
        if used.saturating_add(bytes(&item)) > 4 * 1024 * 1024 {
            eprintln!("ground: runaway: text log exceeds 4194304 bytes without a step boundary");
            std::process::exit(3)
        }
        if cap > self.log.capacity() { self.log.reserve_exact(cap - self.log.len()) }
        self.log.push(item);
    }
    pub fn frame(&mut self, name: &str) {
        if self.harness && self.input.streams_steps() {
            let file = format!("frame_{:04}_{name}.bin", self.frame_n);
            self.input.stream_step_file(&file, &self.fb.snapshot(), false);
            self.frame_n += 1;
        } else if self.harness {
            if self.frames.len() >= MAX_FRAMES { eprintln!("ground: runaway: {MAX_FRAMES} frames without a step boundary ({name})"); std::process::exit(3) }
            let fail = || -> ! { eprintln!("ground: runaway: frame byte budget exceeded ({MAX_FRAME_BYTES})"); std::process::exit(3) };
            if name.len() > MAX_FRAME_BYTES / 2 { fail() }
            let file = format!("frame_{:04}_{name}.bin", self.frame_n);
            let cap = if self.frames.len() == self.frames.capacity() { self.frames.capacity().saturating_mul(2).max(4) } else { self.frames.capacity() };
            let used = self.frames.iter().fold(cap * std::mem::size_of::<(String, Vec<u8>)>(), |n, (s, b)| n.saturating_add(s.capacity()).saturating_add(b.capacity()));
            if used.saturating_add(file.capacity()).saturating_add(self.fb.planes.len()).saturating_add(self.fb.regs.len()) > MAX_FRAME_BYTES { fail() }
            if cap > self.frames.capacity() { self.frames.reserve_exact(cap - self.frames.len()) }
            self.frames.push((file, self.fb.snapshot())); self.frame_n += 1;
        }
        let rgb = self.fb.rgb((self.mem.u16(0x52F4) & 1) as usize, &self.dac); self.input.present(&rgb); self.snapshot();
        if self.harness { self.draw_depth = self.draw_depth.saturating_sub(1); self.checkpoint(name) }
    }
    fn snapshot(&mut self) {
        let Some(hook) = self.scene.take() else { return };
        let mut s = crate::ui::Scene::capture(self, hook.screen.unwrap_or(crate::ui::ScreenKind::Ground), hook.map.as_ref());
        s.shake = hook.shake;
        self.input.scene(&s);
        self.scene = Some(crate::ui::SceneHook { map: Some(s.map), ..hook });
    }
    pub fn screen_kind(&mut self, kind: crate::ui::ScreenKind) { if let Some(h) = &mut self.scene { h.screen = Some(kind) } }
    fn check_trace(&self) { if self.mem.trace.done.len() > MAX_TRACE { eprintln!("ground: runaway: {MAX_TRACE} trace records without a step boundary"); std::process::exit(3) } }
    pub fn stream_trace(&mut self) {
        if self.harness && self.input.streams_steps() && !self.mem.trace.done.is_empty() {
            let json = self.mem.trace.take_walk_jsonl();
            self.input.stream_step_file("trace.jsonl", json.as_bytes(), true);
        }
    }
    pub fn state_buffers(&self)->Vec<(String,Vec<u8>)> {
        let mut buffers = Vec::new();
        for i in 0..9u16 {
            let (pointer,header) = if i==0 {(0xBC,0x5C3A)} else {(0x606E+4*(i-1),0x6096+26*(i-1))};
            let p=self.mem.pointer(pointer);
            let size=self.mem.u8(header) as usize*12*self.mem.u8(header+1) as usize*20*3;
            let data=if p==Far::default() {vec![]} else {
                if size>65535 || p.off as usize+size>65536 {eprintln!("ground: invalid map evidence length");std::process::exit(3)}
                self.mem.bytes(p,size)
            };
            buffers.push((if i==0 {"map_active.bin".to_string()} else {format!("map_{}.bin",i-1)},data));
        }
        let p=self.mem.pointer(0xB8);let mut message=Vec::new();
        if p!=Far::default() {
            for i in 0..(65536-p.off as usize) {let b=self.mem.read(p.add(i as i32),1) as u8;message.push(b);if b==0 {break}}
            if message.last()!=Some(&0) {eprintln!("ground: unterminated message evidence");std::process::exit(3)}
        }
        buffers.push(("message.bin".to_string(),message));
        for (name,slot) in [("font.ly",0x7131),("led.ly",0x6261),("ground.ly",0x641C),("top.ly",0x63F7)] {
            let p=self.mem.pointer(slot);let mut encoded=Vec::new();
            if p!=Far::default() {
                let count=self.mem.read(p,1) as u8;encoded.push(count);
                let table=Far::new(self.mem.read(p.add(1),2) as u16,self.mem.read(p.add(3),2) as u16);
                for i in 0..count as i32 {
                    let entry=table.add(4*i);let ptr=Far::new(self.mem.read(entry,2) as u16,self.mem.read(entry.add(2),2) as u16);
                    let mut records=Vec::new();
                    for j in 0..256 {
                        let at=ptr.add(j*10);if self.mem.read(at,1)==0 {break}
                        if j==255 {eprintln!("ground: layout evidence budget exceeded");std::process::exit(3)}
                        records.extend(self.mem.bytes(at,10));
                    }
                    encoded.push((records.len()/10) as u8);encoded.extend(records);
                }
            }
            buffers.push((name.to_string(),encoded));
        }
        let p=self.mem.pointer(0x6D69);let table=self.mem.pointer(0x6D71);
        let mut data=Vec::new();let mut index=Vec::new();
        if p!=Far::default() {
            for i in 0..(65536-p.off as usize) {
                let b=self.mem.read(p.add(i as i32),1) as u8;data.push(b);if b==b'@' {break}
                if b==0 {
                    if index.len()/2>=2050 {eprintln!("ground: dictionary evidence budget exceeded");std::process::exit(3)}
                    let at=table.add(index.len() as i32*2);
                    let address=self.mem.read(at,2)+16*self.mem.read(at.add(2),2);
                    let base=p.off as u32+16*p.seg as u32;
                    let Some(relative)=address.checked_sub(base).filter(|&v|v<65536) else {eprintln!("ground: invalid dictionary pointer");std::process::exit(3)};
                    index.extend_from_slice(&(relative as u16).to_le_bytes());
                }
            }
            if data.last()!=Some(&b'@') {eprintln!("ground: unterminated dictionary evidence");std::process::exit(3)}
        }
        buffers.push(("dictionary.bin".to_string(),data));buffers.push(("dictionary_index.bin".to_string(),index));
        for i in 0..19 {buffers.push((format!("sound_{i}.bin"),self.mem.sound_data(i)))}
        buffers
    }
    pub fn dump_step(&mut self) {
        self.stream_trace();
        let (rand, text, frames, trace) = (self.rand.take_trace(), std::mem::take(&mut self.log), std::mem::take(&mut self.frames), self.mem.trace.take_walk_jsonl());
        let vars = [("x", self.mem.x() as i64), ("y", self.mem.y() as i64), ("level", self.mem.level() as i64), ("key", self.mem.key() as i64), ("leader", self.mem.leader() as i64), ("orders", self.mem.orders() as i64)];
        let party = self.mem.party().to_vec();let buffers=self.state_buffers();
        let mut files: Vec<(&str, &[u8])> = vec![("party.bin", &party), ("ds.bin", &self.mem.ds), ("trace.jsonl", trace.as_bytes())];
        for (n,b) in &buffers {files.push((n,b))}
        for (n, b) in &frames { files.push((n, b)) }
        if let Some(o) = &self.overlay { let f = o.written(); self.input.end_files(&f) }
        let now = self.clock.ticks();
        let s = Step { key: None, ticks: None, mouse: None, now, rand: &rand, rand_state: self.rand.lcg.seed, text: &text, screen: self.screen, planes: &self.fb.planes, regs: &self.fb.regs, record: &[], vars: &vars, files: &files };
        self.input.step(&s);
    }
    pub fn poll(&mut self) {
        const ESCAPE_CAP: u32 = 200;
        if let Some(mut observer) = self.lockstep.take() {
            observer.poll(self); self.lockstep = Some(observer); return
        }
        self.check_trace();
        if !self.tooling.as_ref().is_some_and(|t| t.pending()) { self.escapes = 0 }
        else if !self.main_wait && self.mem.key() == 0 && self.escapes < ESCAPE_CAP { self.escapes += 1; self.mem.set_key(1); return }
        if self.harness && self.draw_depth > 0 { eprintln!("ground: poll inside an open draw routine (depth {})", self.draw_depth); std::process::exit(3) }
        loop {
            let before = self.clock.ticks();
            crate::play::diagnostic::offer_ground(self);
            let event = self.input.poll(self.clock.as_mut());
            self.sync_bios_clock();
            if event == Event::Boundary { self.dump_step(); continue }
            if self.harness && event == Event::Idle && self.clock.ticks() == before { continue }
            self.deliver_event(event);
            if !self.harness { while self.input.arrived() {
                crate::play::diagnostic::offer_ground(self);
                match self.input.poll(self.clock.as_mut()) { e @ (Event::Key(_) | Event::KeyUp(_) | Event::Mouse(_)) => self.deliver_event(e), _ => break }
            } }
            super::input::read_mouse(self); super::render::cursor_track(self);
            while self.harness && self.input.boundary_due() {
                match self.input.poll(self.clock.as_mut()) { Event::Boundary => self.dump_step(), Event::Idle => {}, e => self.apply_event(e) }
            }
            break;
        }
    }
    fn apply_event(&mut self, event: Event) {
        self.deliver_event(event);
        super::input::read_mouse(self);
        super::render::cursor_track(self);
    }
    fn deliver_event(&mut self,event:Event) {
        match event {
            Event::Boundary => self.dump_step(),
            Event::Key(k) => { self.bios_key = k; if let Some(s) = scan::make_code(k) { super::input::keyboard_scan(self, s) } }
            Event::KeyUp(k) => { if let Some(s)=scan::make_code(k) {super::input::keyboard_scan(self,s|0x80)} }
            Event::Mouse(m) => super::input::mouse_callback(self, m),
            Event::Idle => {}
        }
    }
    pub fn get_key(&mut self) -> Key {
        self.observe_boot("get_key", &[]);
        let rgb = self.fb.rgb((self.mem.u16(0x52F4) & 1) as usize, &self.dac); self.input.present(&rgb); self.snapshot();
        let code = super::input::wait_scan(self);
        match code {
            1 => 27, 0x1C => 13, 0x0E => 8, 0x0F => 9,
            _ => { let ascii = self.mem.u8(super::input::ASCII + code as u16); if ascii != 0 { ascii as Key } else { 0x100 + code as Key } }
        }
    }
    pub fn get_scan(&mut self) -> u8 { super::input::wait_scan(self) }
    pub fn lockstep_stop(&mut self, reason: &str) {
        if let Some(mut observer) = self.lockstep.take() {
            observer.stop(self, reason); self.lockstep = Some(observer);
        }
    }
}
#[cfg(test)]
mod t {
    use super::*;
    use mt_engine::io::{Dir, Scripted};
    use mt_engine::sim::tick::Counter;
    #[test]
    fn lockstep_marks_only_in_the_boot_phase() {
        struct Names(std::rc::Rc<std::cell::RefCell<Vec<String>>>);
        impl crate::lockstep::GroundObserver for Names {
            fn poll(&mut self,_g:&mut Ground) {}
            fn checkpoint(&mut self,_g:&mut Ground,name:&str) { self.0.borrow_mut().push(name.to_string()) }
            fn stop(&mut self,_g:&mut Ground,_reason:&str) {}
        }
        let seen=std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut g=Ground::from_ds(vec![0;65536],vec![0;Mem::PARTY_SIZE],Box::new(Scripted::new([])),Box::new(Counter::default()),Box::new(Overlay::new(None)));
        g.lockstep=Some(Box::new(Names(seen.clone())));
        g.boot_calibrating=true; g.lockstep_checkpoint("init_world"); g.lockstep_checkpoint("load_map");
        g.boot_calibrating=false; g.lockstep_checkpoint("init_world"); g.lockstep_checkpoint("load_map");
        assert_eq!(*seen.borrow(),["init_world","load_map"]);
        assert!(g.lockstep.is_some());
    }
    #[test]
    fn ground_logs_only_show_message() {
        let mut g=Ground::from_ds(vec![0;65536],vec![0;Mem::PARTY_SIZE],Box::new(Scripted::new([])),Box::new(Counter::default()),Box::new(Overlay::new(None)));
        g.harness=true;
        super::super::input::ui::text(&mut g,b"drawing only",0,0);
        super::super::render::message(&mut g,b"Hello, Efate.");
        assert_eq!(g.log.len(),1);
        assert!(matches!(&g.log[0],Text::Message(s) if s=="Hello, Efate."));
    }
    #[test]
    fn boot_clock_delivers_irqs_without_polling_mouse_or_simulation() {
        use mt_engine::io::{Harness,script::Line};
        let dir=std::env::temp_dir().join(format!("mt-boot-clock-{}",std::process::id()));
        let lines=mt_engine::io::script::lines("at message 9\nkey 13\nticks 8\nmouse 17 100 4\nticks 1\n").unwrap();
        assert!(matches!(lines[0],Line::At{tick:9,..}));
        let mut h=Harness::lines(lines,dir.clone(),false);h.max_polls=64;
        let mut g=Ground::from_ds(fake_t_dat(),vec![0;Mem::PARTY_SIZE],Box::new(h),Box::new(Counter::default()),Box::new(Overlay::new(None)));
        g.harness=true;g.mem.set_u16(0x7012,1);g.mem.set_u16(0x6FFE,12);
        let tick=g.input.clock_checkpoint("message").unwrap().unwrap();
        assert!(g.input.clock_checkpoint("missing").unwrap_err().contains("missing"));
        g.elapse_to(tick).unwrap();
        assert_eq!((g.clock.now(),g.mem.key(),g.mem.u16(0x6FFE),g.mem.x()),(9,0x6E,12,44));
        assert_eq!(g.mem.u8(0x6FEA),0x9C);
        assert_eq!(g.mem.i16(super::super::input::CALLBACK+4),100);
        assert!(g.frames.is_empty() && g.rand.trace.is_empty());
        assert!(std::fs::read_to_string(dir.join("step_0004/step.json")).unwrap().contains("\"now\":9,"));
        assert!(g.elapse_to(8).unwrap_err().contains("already past"));
        assert!(g.elapse_to(10).unwrap_err().contains("ran out"));
        assert_eq!(g.input.clock_checkpoint("after_the_script").unwrap(),None);
        std::fs::remove_dir_all(dir).unwrap();
    }
    fn fake_t_dat() -> Vec<u8> { let mut ds = vec![0u8; 0x10000]; ds[0x6B6C] = 44; ds[0x6B6D] = 0; ds[0xE8] = 0xFF; ds[0x95..0x98].copy_from_slice(&[0, 0xFE, 3]); ds[0x120..0x129].copy_from_slice(&[0, 0, 6, 0, 50, 0, 40, 0, 0x3B]); ds }
    #[test]
    fn image_and_accessors() {
        let ds = fake_t_dat();
        let mut mem = Mem { ds, segs: BTreeMap::new(), trace: Trace::default(), bios_ticks: [0; 4] };
        mem.segs.insert(Mem::PARTY_SEG, vec![0; Mem::PARTY_SIZE]); mem.set_pointer(PARTY_PTR, Far::new(0, Mem::PARTY_SEG));
        assert_eq!((mem.x(), mem.level(), mem.key()), (44, -1, 0)); mem.set_level(3); mem.set_x(-2); assert_eq!((mem.i8(0xE8), mem.u16(0x6B6C)), (3, 0xFFFE));
        assert_eq!(mem.direction(0), (0, -2, 3)); assert_eq!(mem.pointer(PARTY_PTR), Far::new(0, 0x9000));
        for i in 0..=u16::MAX {
            let at = (0x95 + 3 * i as usize) & 0xFFFF;
            assert_eq!(mem.direction(i), (mem.ds[at] as i8 as i16, mem.ds[(at + 1) & 0xFFFF] as i8 as i16, mem.ds[(at + 2) & 0xFFFF] as i8 as i16));
        }
        mem.set_party_u16(0xF84, 7); assert_eq!((mem.party_u16(0xF84), mem.party()[0xF84]), (7, 7)); mem.set_party_u8(0x102, 0xFF); assert_eq!(mem.party_u8(0x102), 0xFF);
        assert_eq!(mem.read(Far::new(0xF85, 0x9000), 2), 0);
        mem.write(Far::new(0xF85, 0x9000), 0xABCD, 2); assert_eq!(mem.party()[0xF85], 0xCD);
        assert_eq!(mem.read(Far::new(0, 0x1234), 4), 0); mem.write(Far::new(0, 0x1234), 1, 1);
        mem.alloc(0x1234, 8); mem.write(Far::new(6, 0x1234), 0x11223344, 4); assert_eq!(mem.bytes(Far::new(6, 0x1234), 4), [0x44, 0x33, 0, 0]);
        assert_eq!(Mem::walker(2).off, 0x5C54 + 172); assert_eq!(Mem::npc(1).off, 0x63F4 + 46); assert_eq!(Far::new(0xFFFF, 1).add(2).off, 1);
        mem.write_bytes(Mem::ds(0x100), b"Hi\0x"); assert_eq!(mem.cstr(Mem::ds(0x100)), "Hi");
        assert_eq!(mt_engine::io::mouse::table(&mem.ds[HOTSPOTS_GROUND as usize..], 1).next().unwrap().id, 0x3B);
        let mut r = Rand::default(); r.recording = true; r.force = Some(5); r.srand(99); assert_eq!(r.lcg.seed, 5); r.force = None; r.srand(0); assert_eq!(r.rand(), 0); assert_eq!(r.random(10), 6); assert_eq!(r.take_trace(), [0, 346]);
    }
    #[test]
    fn trace_entry_captures_ds_party_and_heap() {
        let mut mem = Mem { ds: vec![0; 65536], segs: BTreeMap::new(), trace: Trace::default(), bios_ticks: 0x1234u32.to_le_bytes() };
        mem.alloc(Mem::PARTY_SEG, 8); mem.alloc(0x8000, 16); mem.trace.on = true;
        mem.trace_enter("outer", &[]); mem.set_u8(7, 42);
        mem.trace_enter("inner", &[]); mem.write(Far::new(3, 0x8000), 99, 1); mem.trace.exit(None);
        mem.trace.exit(None);
        let entry = mem.trace.done[0].entry.as_ref().unwrap();
        assert_eq!(entry.iter().map(|r| (r.0, r.1, r.2.len())).collect::<Vec<_>>(), [(Mem::DS, 0, 65536), (0x8000, 0, 16), (Mem::PARTY_SEG, 0, 8), (0x40, 0x6C, 4)]);
        assert_eq!(entry[3].2, [0x34, 0x12, 0, 0]);
        assert_eq!(entry[0].2[7], 42); assert_eq!(entry[1].2[3], 0);
        assert_eq!(mem.trace.done[1].entry.as_ref().unwrap()[0].2[7], 0);
        assert_eq!(mem.read(Mem::BIOS_TICKS, 4), 0x1234); assert_eq!(mem.read(Far::new(0x6E, 0x40), 4), 0);
        mem.write(Mem::BIOS_TICKS, 0xABCD, 4); assert_eq!((mem.bios_ticks, mem.read(Far::new(0x6D, 0x40), 1)), (0xABCDu32.to_le_bytes(), 0xAB));
        mem.trace.take_jsonl(); assert_eq!(mem.trace.retained_bytes(), 0);
        mem.trace_enter("same_output", &[]); mem.trace.exit(Some(0));
        mem.write(Far::new(1, Mem::PARTY_SEG), 1, 1);
        mem.trace_enter("same_output", &[]); mem.trace.exit(Some(0));
        assert_eq!(mem.trace.done[0].writes, mem.trace.done[1].writes);
        assert_ne!(mem.trace.done[0].entry, mem.trace.done[1].entry);
        assert!(mem.trace.take_jsonl().contains("\"entry\":[[12386,0,["));
    }
    #[test]
    fn blocking_reads_preserve_absolute_ticks_and_pending_release() {
        use mt_engine::io::{Harness, script::Line};
        let dir = std::env::temp_dir().join(format!("mt-ground-blocking-{}", std::process::id()));
        let mut script = vec![Line::Ticks(400)];
        for _ in 0..6 { script.extend([Line::Key(13), Line::Ticks(120)]) }
        script.extend([Line::Key(0x150), Line::Ticks(40)]);
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(Harness::lines(script, dir.clone(), false)), Box::new(Counter::at(7)), Box::new(Dir(dir.clone())));
        g.harness = true;
        for i in 0..6 {
            g.message_text(&format!("page {i}"));
            assert_eq!(g.get_key(), 13); assert_eq!(g.now(), 411 + 120 * i)
        }
        assert_eq!(g.get_key(), 0x150); assert_eq!((g.now(), g.mem.key()), (1131, 0));
        g.mem.set_key(0x4D);
        for _ in 0..3 { g.poll(); assert_eq!(g.mem.key(), 0x4D) }
        g.poll(); assert_eq!((g.now(), g.mem.key()), (1135, 0));
        for _ in 0..32 { g.poll() }
        assert_eq!(g.now(), 1167);
        let step = |n: usize| std::fs::read_to_string(dir.join(format!("step_{n:04}/step.json"))).unwrap();
        assert!(step(0).contains("\"now\":7,"));
        assert!(step(3).contains("\"now\":527,"));
        assert!(step(15).contains("\"now\":1167,") && step(15).contains("\"ticks\":40,"));
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 16);
        let evidence = (0..15).map(step).collect::<Vec<_>>().join("");
        for i in 0..6 { assert_eq!(evidence.matches(&format!("page {i}")).count(), 1); }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn blocking_harness_mouse_keeps_callback_clip_and_clock() {
        use mt_engine::io::{Harness, script::Line};
        let dir = std::env::temp_dir().join(format!("mt-ground-blocking-mouse-{}", std::process::id()));
        let mouse = MouseEvent { event: 4, buttons: 0, col: 100, row: 100 };
        let script = [Line::Ticks(3), Line::Mouse(mouse), Line::Key(13), Line::Ticks(12)];
        let mut harness = Harness::lines(script, dir.clone(), false); harness.max_polls = 64;
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(harness), Box::new(Counter::at(17)), Box::new(Dir(dir.clone())));
        g.harness = true; g.mem.set_u16(0x7000, 0xFFFF); g.mem.set_i16(0x7012, 1);
        g.message_text("before mouse wait");
        assert_eq!(g.get_key(), 0x16E); assert_eq!((g.now(), g.mem.key()), (20, 0));
        assert_eq!(g.mouse_event, Some(mouse));
        assert_eq!((g.mem.i16(super::super::input::CALLBACK + 6), g.mem.i16(super::super::input::CALLBACK + 4)), (100, 100));
        assert_eq!((g.mem.i16(0x6FFC), g.mem.i16(0x6FFE)), (100, 46));
        let live = g.input.mouse(); assert_eq!((live.col, live.row), (100, 46));
        assert_eq!(g.get_key(), 13); assert_eq!(g.now(), 24);
        for _ in 0..8 { g.poll() }
        assert_eq!(g.now(), 32);
        let first = std::fs::read_to_string(dir.join("step_0000/step.json")).unwrap();
        assert!(first.contains("before mouse wait") && first.contains("\"now\":17,"));
        let mouse_wait = std::fs::read_to_string(dir.join("step_0002/step.json")).unwrap();
        assert!(mouse_wait.contains("\"mouse\":[100,100,4]") && mouse_wait.contains("\"now\":20,"));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn assigned_clock_value_is_not_elapsed_time() {
        use mt_engine::io::Harness;
        let dir = std::env::temp_dir().join(format!("mt-assigned-poll-{}", std::process::id()));
        let mut h = Harness::lines(mt_engine::io::script::lines("ticks 2
").unwrap(), dir.clone(), false); h.max_polls = 16;
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(h), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.harness = true; g.set_clock(1);
        g.poll();
        assert_eq!((g.clock.now(), g.clock.ticks()), (1, 1));
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn draw_entry_elapse_leaves_the_boundary_to_the_frame() {
        use mt_engine::io::Harness;
        let dir = std::env::temp_dir().join(format!("mt-draw-entry-{}", std::process::id()));
        let lines = mt_engine::io::script::lines("at enter_redraw_view 3
at enter_draw_walkers 4
at draw_walkers 4
at redraw_view 5
ticks 4
key 13
ticks 2
").unwrap();
        let mut h = Harness::lines(lines, dir.clone(), false); h.max_polls = 64;
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(h), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.harness = true; g.poll(); assert_eq!(g.clock.ticks(), 1);
        g.enter("redraw_view"); assert_eq!(g.clock.ticks(), 3);
        g.enter("draw_walkers"); assert_eq!(g.clock.ticks(), 4);
        assert!(g.input.boundary_due() && !dir.join("step_0001").exists());
        g.frame("draw_walkers"); assert!(dir.join("step_0001/step.json").exists() && dir.join("step_0002/step.json").exists());
        assert!(std::fs::read_to_string(dir.join("step_0001/step.json")).unwrap().contains("\"now\":4,"));
        g.frame("redraw_view"); assert_eq!(g.clock.ticks(), 5);
        assert_eq!(g.frames.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>(), ["frame_0001_redraw_view.bin"]);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn window_poll_applies_every_arrived_scan_to_the_latch() {
        struct Events(std::collections::VecDeque<Event>);
        impl mt_engine::io::Input for Events {
            fn next_key(&mut self) -> mt_engine::io::Key { 27 }
            fn poll(&mut self, _: &mut dyn mt_engine::sim::tick::Clock) -> Event { self.0.pop_front().unwrap_or(Event::Idle) }
            fn arrived(&mut self) -> bool { !self.0.is_empty() }
        }
        let run = |events: &[Event]| {
            let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(Events(events.iter().copied().collect())), Box::new(Counter::default()), Box::new(Dir(std::env::temp_dir())));
            g.poll(); (g.mem.key(), g.now())
        };
        let (right, up, c) = (Event::Key(0x14D), Event::Key(0x148), Event::Key(b'c' as mt_engine::io::Key));
        assert_eq!(run(&[right, right, right, Event::KeyUp(0x14D)]), (0, 0));
        assert_eq!(run(&[right, right, Event::KeyUp(0x14D), up]), (0x48, 0));
        assert_eq!(run(&[c, Event::KeyUp(b'c' as mt_engine::io::Key)]), (0x2E, 0));
        assert_eq!(run(&[right, Event::KeyUp(b'c' as mt_engine::io::Key)]), (0, 0));
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(Events([right, Event::KeyUp(0x14D)].into())), Box::new(Counter::default()), Box::new(Dir(std::env::temp_dir())));
        g.harness = true; g.poll(); assert_eq!(g.mem.key(), 0x4D);
    }
    #[test]
    fn ground_new_and_poll() {
        let d = std::env::temp_dir();
        assert!(Ground::new(b"MZ junk", vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Dir(d.clone()))).is_none());
        let mut g = Ground::from_ds(fake_t_dat(), vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([0x148, 13])), Box::new(Counter::default()), Box::new(Dir(d)));
        assert_eq!(g.mem.pointer(PARTY_PTR), Far::new(0, 0x9000));
        g.poll(); assert_eq!(g.mem.key(), 0x48); g.mem.set_key(0); g.poll(); assert_eq!(g.mem.key(), 0x1C); g.poll(); assert_eq!((g.mem.key(), g.now()), (0x1C, 1));
        assert_eq!(g.mem.read(Mem::BIOS_TICKS, 4), 1); g.set_clock(77); assert_eq!(g.mem.read(Mem::BIOS_TICKS, 4), 77);
        assert_eq!(g.mem.u16(MOUSE_WORDS), 0); g.frame("redraw_view"); assert!(g.frames.is_empty()); g.harness = true; g.frame("redraw_view"); g.frame("message"); assert_eq!(g.frames[1].0, "frame_0001_message.bin"); assert_eq!(g.frames[0].1.len(), 4 * 65536 + 16);
        g.message_text("Hello"); g.dump_step(); assert!(g.frames.is_empty() && g.log.is_empty());
        g.input = Box::new(Scripted::new([27]));
        assert_eq!(g.get_key(), 27);
    }
}
