use crate::{ground::state::{Ground, Mem}, space::{Space, state as s}};
use super::{Scene, ScreenKind, scene::Party};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context {
    pub event: i8, pub arik: Option<i16>, pub pursuer: Option<u16>, pub station: Option<i16>,
    pub clock: Option<u32>,
    pub silent: bool,
}
impl Context {
    pub fn ground(g: &Ground) -> Self {
        let m = &g.mem;
        Self { event: m.event(), arik: Some(m.arik()),
            pursuer: (m.pursuer_active() != 0).then(|| m.read(Mem::npc(m.pursuer_index() as i16 + 1), 2) as u16),
            clock: Some(u32::from_le_bytes(m.bios_ticks)), station: None, silent: m.sound_enabled() == 0 }
    }
}
impl Scene {
    pub fn space(sp: &Space) -> Self {
        let mut scene = Self::program(ScreenKind::Space);
        scene.party = Party(sp.s.party.clone());
        scene.context.station = (sp.s.word(s::STATION_ACTIVE) != 0).then(|| sp.s.word(s::STATION));
        scene.context.clock = Some(sp.last_ticks);
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ground_metadata_copies_timer_event_pursuer_and_clock_without_writes() {
        use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
        let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::at(123)), Box::new(Overlay::new(None)));
        g.mem.set_event(9); g.mem.set_arik(300);
        let c = Context::ground(&g); assert_eq!((c.event, c.arik, c.clock, c.pursuer, c.station), (9, Some(300), Some(123), None, None));
        g.mem.set_pursuer_active(1); g.mem.set_pursuer_index(2); g.mem.write(Mem::npc(3), 0x12F, 2);
        let before = (g.mem.ds.clone(), g.mem.segs.clone());
        assert_eq!(Context::ground(&g).pursuer, Some(0x12F)); assert_eq!(before, (g.mem.ds, g.mem.segs));
    }
}
