pub(crate) mod codec;
use codec::Value;
use crate::{ground::{squad, Ground}, save::{Mode, Saved}};
use mt_engine::play::HASH_VERSION;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Squad(Option<squad::State>);
impl Squad {
    pub fn capture(state: &Option<squad::State>) -> Result<Self, String> {
        Ok(Self(state.clone()))
    }
    pub fn state(&self) -> &Option<squad::State> { &self.0 }
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new(); HASH_VERSION.put(&mut out); self.0.put(&mut out);
        out
    }
    pub fn decode(mut input: &[u8]) -> Result<Self, String> {
        if u32::get(&mut input)? != HASH_VERSION { return Err("play: unsupported SQD hash version".into()) }
        let state = Value::get(&mut input)?;
        if !input.is_empty() { return Err("play: trailing SQD bytes".into()) }
        Self::capture(&state)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meta {
    pub sequence: u64, pub start_tick: u32, pub bios_key: u16, pub mouse_driver: bool,
    pub balance: crate::ground::rules::balance::Options,
    pub resume: crate::ground::main_loop::Resume, pub planes: Vec<u8>, pub input: mt_engine::play::Checkpoint,
    pub space: Option<crate::space::play::image::Image>,
    pub scenario_boot: bool,
    pub host: Vec<u8>,
    pub text: Option<mt_engine::present::textlog::SurfaceImage>,
}
impl Meta {
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![]; HASH_VERSION.put(&mut out); self.sequence.put(&mut out); self.start_tick.put(&mut out); self.bios_key.put(&mut out); self.mouse_driver.put(&mut out);
        self.balance.grenade_range.put(&mut out); self.balance.npc_selective_autofire.put(&mut out);
        self.resume.put(&mut out); self.planes.put(&mut out); self.input.put(&mut out); self.space.put(&mut out); self.scenario_boot.put(&mut out); self.host.put(&mut out); self.text.put(&mut out); out
    }
    pub fn decode(mut input: &[u8]) -> Result<Self, String> {
        if u32::get(&mut input)? != HASH_VERSION { return Err("play: unsupported PLAY hash version".into()) }
        let v = Self { sequence: Value::get(&mut input)?, start_tick: Value::get(&mut input)?, bios_key: Value::get(&mut input)?, mouse_driver: Value::get(&mut input)?,
            balance: crate::ground::rules::balance::Options { grenade_range: Value::get(&mut input)?, npc_selective_autofire: Value::get(&mut input)? },
            resume:Value::get(&mut input)?, planes:Value::get(&mut input)?, input:Value::get(&mut input)?, space:Value::get(&mut input)?, scenario_boot:Value::get(&mut input)?, host:Value::get(&mut input)?, text:Value::get(&mut input)? };
        if v.planes.len()!=4*mt_engine::present::fb::PLANE || !matches!(v.input.frame.len(),0|64000) { return Err("play: invalid frame size".into()) }
        if !input.is_empty() { return Err("play: trailing PLAY bytes".into()) } Ok(v)
    }
}

pub fn capture(g: &mut Ground, sequence: u64, start_tick: u32) -> Result<Saved, String> {
    if !g.loop_vars.initialized || g.mem.key() != 0 || g.mem.event() != -1 || g.mouse_event.is_some() ||
        g.launch_pending || g.game_over || g.restore_pending.is_some() || g.boot_party.is_some() || g.boot_calibrating || g.rand.force.is_some() {
        return Err("play: not a quiescent ground loop top".into())
    }
    let squad = Squad::capture(&g.squad)?;
    let mut saved = Saved::capture(g, Mode::Ground, "");
    saved.squad = Some(squad); saved.play = Some(Meta { sequence, start_tick, bios_key: g.bios_key, mouse_driver: g.mouse_driver, balance: g.balance,
        resume:g.play_resume, planes:g.fb.planes.clone(), input:g.input.play_checkpoint().unwrap_or_default(), space:None, scenario_boot:g.scenario_boot, host:vec![], text:mt_engine::present::textlog::capture(&g.fb) }); Ok(saved)
}
pub fn launch_parent(g:&mut Ground)->Result<Saved,String> {
    let pending=g.launch_pending; let key=g.mem.key();let event=g.mem.event();
    g.launch_pending=false;g.mem.set_key(0);g.mem.set_event(-1);
    let result=capture(g,0,0);
    g.launch_pending=pending;g.mem.set_key(key);g.mem.set_event(event);
    result.map(|mut saved| {saved.ds=g.mem.ds.clone();saved})
}
pub fn offer(g: &mut Ground) {
    boundary(g,crate::ground::main_loop::Resume::Top);
}
pub fn boundary(g: &mut Ground, resume: crate::ground::main_loop::Resume) {
    if g.input.data_mode() && g.input.wants_decision() && g.input.play_checkpoint().is_some() {
        g.play_resume=resume;
        if let Ok(saved) = capture(g, 0, 0) { g.input.anchor("t.6923", &saved.encode()); }
    }
}
pub fn seal(image: &[u8], sequence: u64, start_tick: u32) -> Result<Vec<u8>, String> {
    let mut saved = Saved::decode(image)?;
    let meta = saved.play.as_mut().ok_or("play: anchor missing PLAY")?;
    if saved.squad.is_none() { return Err("play: anchor missing SQD".into()) }
    meta.sequence = sequence; meta.start_tick = start_tick; Ok(saved.encode())
}
