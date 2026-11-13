use crate::{ground::Ground, save::{Mode, Saved}};
use mt_engine::play::{Decision, Fnv, HASH_VERSION};

pub mod commands;
pub mod menus;
pub mod turns;
pub mod anchors;
pub mod observation;
pub mod diagnostic;

pub fn hash(g: &mut Ground) -> String {
    let _profile = mt_engine::play::profile::span("ground_hash");
    let mut h = Fnv::default();
    h.write(&HASH_VERSION.to_le_bytes()); h.field(b"t");
    h.field(&Saved::capture(g, Mode::Ground, "").encode());
    h.field(format!("{:?}", g.squad).as_bytes());
    h.write(&[g.balance.grenade_range as u8, g.balance.npc_selective_autofire as u8]);
    h.write(&g.bios_key.to_le_bytes()); h.write(&[g.mouse_driver as u8]);
    match g.mouse_event {
        None => h.write(&[0]),
        Some(m) => { h.write(&[1]); for w in [m.event, m.buttons, m.col as u16, m.row as u16] { h.write(&w.to_le_bytes()); } }
    }
    h.write(&[g.launch_pending as u8, g.game_over as u8, g.restore_pending.is_some() as u8,
        g.scenario_boot as u8, g.boot_calibrating as u8]);
    match &g.boot_party { None => h.write(&[0]), Some(p) => { h.write(&[1]); h.field(p); } }
    match g.rand.force { None => h.write(&[0]), Some(f) => { h.write(&[1]); h.write(&f.to_le_bytes()); } }
    h.state()
}

pub fn decide(g: &mut Ground, mut d: Decision) {
    crate::ground::round::refreeze(g);
    if g.input.data_mode() {
        let tick = g.clock.now(); let state = observation::Observation::ground(g, tick); g.input.scene(&state);
    }
    d.state = hash(g); g.input.decide(&d);
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod coverage;
