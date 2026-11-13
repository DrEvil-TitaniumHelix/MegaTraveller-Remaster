use crate::{ground::{Ground, round, rules::adapters}, ui::{Scene, ScreenKind, scene::Party}};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct Observation {
    pub tick: u32, pub scene: Scene, pub flight: Option<Flight>, pub draft: Option<Vec<u8>>,
    pub travellers: Vec<(u8, (i16, i16))>, pub encounter: Option<String>,
    pub npc_hits: BTreeMap<u8, Option<i32>>,
}
#[derive(Clone, Debug)]
pub struct Flight {
    pub position: (i16, i16), pub velocity: (i16, i16), pub heading: i16,
    pub thrust: i16, pub station: i16, pub encounter: i16,
}
impl Observation {
    pub fn program(screen: ScreenKind, tick: u32) -> Self {
        Self { tick, scene: Scene::program(screen), flight: None, draft: None, travellers: vec![], encounter: None, npc_hits: BTreeMap::new() }
    }
    pub fn ground(g: &Ground, tick: u32) -> Self {
        Self { tick, scene: Scene::capture(g, g.scene.as_ref().and_then(|s| s.screen).unwrap_or(ScreenKind::Ground), None),
            flight: None, draft: None, travellers: round::travellers(g).into_iter().map(|i| (i, round::traveller_cell(g, i))).collect(),
            encounter: g.squad.as_ref().and_then(|s| s.p1.encounter.as_ref()).map(|e| e.def.clone()),
            npc_hits: g.squad.as_ref().map(|s| (1..=32).map(|k| {
                let hits=s.pools.get(&k).copied().or_else(|| {
                    let f=adapters::npc(g,k,&s.compendium);
                    (f.source!=adapters::Source::Invulnerable).then_some(f.pool)
                }); (k,hits)
            }).collect()).unwrap_or_default() }
    }
    pub fn space(sp: &crate::space::Space, tick: u32) -> Self {
        use crate::space::state::*;
        let mut o = Self::program(ScreenKind::Space, tick);
        o.scene.party = Party(sp.s.party.clone());
        o.flight = Some(Flight { position: (sp.s.word(X), sp.s.word(Y)), velocity: (sp.s.word(VX), sp.s.word(VY)),
            heading: sp.s.word(HEADING), thrust: sp.s.word(THRUST), station: sp.s.word(STATION), encounter: sp.s.word(ENCOUNTER) }); o
    }
}
