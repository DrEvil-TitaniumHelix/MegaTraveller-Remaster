use super::{combat, render, rules::{adapters, compendium::Compendium}, state::{Ground, Mem}, walk};
use crate::ui::Intent;
use std::collections::BTreeMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct State { pub encounter: bool, pub round: u16, pub pools: BTreeMap<u8, i32>, pub ablat_worn: [u8; 5], pub compendium: Compendium, pub derived: Vec<u8>,
    pub p1: super::round::State,
    pub orders: super::orders::State,
    pub fire: super::fire::State,
    pub p22: super::tactics::State, pub p6: super::standing::State }
impl State {
    pub fn with_compendium(compendium: Compendium) -> State { State { compendium, ..State::default() } }
    pub fn reset_level(&mut self) { self.pools.clear(); self.derived.clear(); self.p1.reset_level(); self.fire = Default::default(); self.p22 = Default::default(); self.p6 = Default::default(); self.encounter = false; self.round = 0; }
}

pub const SAVE_REFUSED: &str = "Not while hostiles are about.";

pub fn active(g: &Ground) -> bool { g.squad.is_some() }
pub fn save_refusal(g: &Ground) -> Option<&'static str> { g.squad.as_ref().filter(|s| s.encounter).map(|_| SAVE_REFUSED) }
pub fn set_classic(g: &mut Ground, classic: bool) {
    if classic { g.squad = None } else if g.squad.is_none() && g.scene.is_some() { g.squad = Some(State::default()) }
}
pub fn poll(g: &mut Ground) -> Vec<Intent> {
    let mut out = Vec::new();
    while let Some(i) = g.input.intent() {
        match i.downcast_ref::<Intent>() {
            Some(&Intent::ClassicCombat(c)) => set_classic(g, c),
            Some(&Intent::BalanceOptions(options)) => { if g.scene.is_some() { g.balance = options } },
            Some(&i) => out.push(i), None => {}
        }
    }
    out
}

pub fn two_d(g: &mut Ground) -> (i32, i32) { let a = walk::random(g, 6) as i32 + 1; (a, walk::random(g, 6) as i32 + 1) }
pub fn traveller_take(g: &mut Ground, who: u8, damage: i32) -> i32 { combat::take_hit(g, who as i32, 100, damage) }
pub fn note(g: &mut Ground, name: &'static str, args: &[u16]) { if g.squad.is_some() { g.mem.trace.call(name, args, None) } }
pub fn npc_pool(g: &mut Ground, k: u8) -> Option<i32> {
    let squad = g.squad.as_ref()?;
    if let Some(&p) = squad.pools.get(&k) { return Some(p) }
    let f = adapters::npc(g, k, &squad.compendium);
    if f.source == adapters::Source::Invulnerable { return None }
    let s = g.squad.as_mut()?;
    if f.source == adapters::Source::Derived && !s.derived.contains(&k) { s.derived.push(k); eprintln!("squad: NPC record {k} has no compendium entry: derived (R-9)") }
    s.pools.insert(k, f.pool);
    Some(f.pool)
}
pub fn npc_take(g: &mut Ground, k: u8, damage: i32) -> bool {
    let n = Mem::npc(k as i16);
    let (x, y) = (g.mem.read_i(n.add(32), 2) >> 1, g.mem.read_i(n.add(34), 2) >> 1);
    g.mem.write(n.add(0x26), 1, 1);
    let pool = npc_pool(g, k);
    let dmg = match pool { None => 0, Some(p) => (if damage == 255 { p >> 1 } else { damage }).clamp(0, p) };
    render::front(g); combat::flash_marker(g, dmg, x * 2, y * 2); render::back(g);
    if g.mem.read(g.mem.pointer(0x5E02).add(6), 1) > 8 { g.mem.set_provoked(2) }
    let Some(p) = pool else { return false };
    let left = p - dmg;
    if left > 0 { if let Some(s) = g.squad.as_mut() { s.pools.insert(k, left); } return false }
    if g.mem.read(n.add(13), 1) & 3 != 0 { g.mem.set_provoked(2) }
    if let Some(s) = g.squad.as_mut() { s.pools.remove(&k); }
    combat::npc_dies(g, x, y);
    true
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::state::Mem;
    use mt_engine::{io::{Input, Key, Overlay, Scripted}, sim::tick::Counter};
    struct Intents { inner: Scripted, queue: std::collections::VecDeque<Intent> }
    impl Input for Intents {
        fn next_key(&mut self) -> Key { self.inner.next_key() }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> mt_engine::io::Event { self.inner.poll(clock) }
        fn intent(&mut self) -> Option<Box<dyn std::any::Any>> { self.queue.pop_front().map(|i| Box::new(i) as Box<dyn std::any::Any>) }
    }
    fn ground(intents: &[Intent]) -> Ground {
        let input = Intents { inner: Scripted::new([]), queue: intents.iter().copied().collect() };
        Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(input), Box::new(Counter::default()), Box::new(Overlay::new(None)))
    }
    #[test]
    fn balance_options_follow_the_intent_path_and_survive_resets() {
        let options = super::super::rules::balance::Options { grenade_range: false, npc_selective_autofire: false };
        let mut g = ground(&[Intent::BalanceOptions(options)]);
        assert!(poll(&mut g).is_empty()); assert_eq!(g.balance, Default::default(), "faithful ignores the options intent");
        let mut g = ground(&[Intent::BalanceOptions(options), Intent::ClassicCombat(true), Intent::ClassicCombat(false)]);
        g.scene = Some(crate::ui::SceneHook::default()); g.squad = Some(State::default());
        let before = g.mem.party().to_vec();
        assert!(poll(&mut g).is_empty()); assert_eq!(g.balance, options);
        g.squad.as_mut().unwrap().reset_level(); assert_eq!(g.balance, options);
        assert_eq!(g.mem.party(), before, "preferences never write the original party block");
    }
    #[test]
    fn flag_save_refusal_and_the_options_switch() {
        let mut g = ground(&[]);
        assert!(!active(&g) && save_refusal(&g).is_none(), "faithful / harness: no squad state, saves never refused");
        set_classic(&mut g, false); assert!(g.squad.is_none(), "no scene = not product mode: the switch cannot turn squad mode on");
        g.scene = Some(crate::ui::SceneHook::default()); g.squad = Some(State::default());
        assert!(active(&g) && save_refusal(&g).is_none(), "squad mode outside an encounter: saving allowed (O-21: alerted hostiles do not block)");
        g.squad.as_mut().unwrap().encounter = true;
        assert_eq!(save_refusal(&g), Some(SAVE_REFUSED));
        set_classic(&mut g, true); assert!(g.squad.is_none() && save_refusal(&g).is_none());
        set_classic(&mut g, false); assert_eq!(g.squad, Some(State::default()));
        let mut g = ground(&[Intent::EndTurn, Intent::ClassicCombat(true), Intent::Select(2)]);
        g.scene = Some(crate::ui::SceneHook::default()); g.squad = Some(State::default());
        let rest = poll(&mut g);
        assert!(g.squad.is_none()); assert!(matches!(rest[..], [Intent::EndTurn, Intent::Select(2)]));
        assert!(poll(&mut g).is_empty());
    }
    #[test]
    fn npc_take_pools_never_write_0x0a() {
        use crate::ground::state::{Far, MAP_PTR};
        let mut g = ground(&[]);
        g.mem.alloc(0x8000, 20 * 12 * 3); g.mem.set_pointer(MAP_PTR, Far::new(0, 0x8000)); g.mem.set_u8(0x6D2B, 20);
        g.mem.alloc(0x9100, 46 * 140); g.mem.set_pointer(0x6D6D, Far::new(0, 0x9100));
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        g.scene = Some(crate::ui::SceneHook::default());
        g.squad = Some(State::with_compendium(Compendium::parse(crate::ground::rules::compendium::NPCS_TOML).unwrap()));
        let place = |g: &mut Ground, k: u8, picture: u8, hp: u8, level: u8, flags: u8, row: i32, col: i32| {
            let n = Mem::npc(k as i16);
            g.mem.write(n.add(9), picture as u32, 1); g.mem.write(n.add(10), hp as u32, 1); g.mem.write(n.add(11), level as u32, 1); g.mem.write(n.add(13), flags as u32, 1);
            g.mem.write(n.add(2), 0, 1); g.mem.write(n.add(32), (row * 2) as u32, 2); g.mem.write(n.add(34), (col * 2) as u32, 2);
            g.mem.write(Far::new((row * 20 + col) as u16 * 3 + 2, 0x8000), k as u32, 1);
        };
        place(&mut g, 14, 41, 12, 3, 0, 9, 15);
        let thug = g.squad.as_ref().unwrap().compendium.roles["thug"].hits;
        let pool = thug.0 + thug.1;
        assert_eq!(npc_pool(&mut g, 14), Some(pool));
        assert!(!npc_take(&mut g, 14, 2));
        let n = Mem::npc(14);
        assert_eq!((g.squad.as_ref().unwrap().pools[&14], g.mem.read(n.add(10), 1), g.mem.read(n.add(0x26), 1), g.mem.read(n.add(0x29), 1), g.mem.read(n.add(11), 1)), (pool - 2, 12, 1, 0, 3), "pool down 2; 0x0A, morale 0x29 and level 0x0B untouched");
        assert!(!npc_take(&mut g, 14, 255)); assert_eq!(g.squad.as_ref().unwrap().pools[&14], (pool - 2) - ((pool - 2) >> 1), "neural: half the pool (O-16)");
        assert!(npc_take(&mut g, 14, 99), "overkill: dead at 0");
        assert_eq!((g.mem.read(n.add(2), 1), g.mem.read(n.add(10), 1), g.squad.as_ref().unwrap().pools.get(&14), g.mem.provoked()), (20, 12, None, 0), "npc_dies: state 20; 0x0A still 12; a hostile's death provokes nobody on a law 0 world");
        place(&mut g, 17, 42, 5, 1, 2, 9, 16);
        assert!(npc_take(&mut g, 17, 50)); assert_eq!(g.mem.provoked(), 2, "the death of a non-hostile provokes the police (hit_npc's rule)");
        place(&mut g, 9, 55, 0xFF, 2, 2, 10, 15);
        assert_eq!(npc_pool(&mut g, 9), None); assert!(!npc_take(&mut g, 9, 30)); assert_eq!((g.mem.read(Mem::npc(9).add(10), 1), g.mem.read(Mem::npc(9).add(0x26), 1), g.mem.read(Mem::npc(9).add(2), 1)), (0xFF, 1, 0), "no pool, no damage, still hit, still there");
        place(&mut g, 6, 200, 19, 4, 0, 10, 16);
        assert_eq!(npc_pool(&mut g, 6), Some(19)); assert!(!npc_take(&mut g, 6, 3)); assert_eq!((g.squad.as_ref().unwrap().pools[&6], g.squad.as_ref().unwrap().derived.as_slice()), (16, &[6][..]));
        g.squad.as_mut().unwrap().reset_level(); assert!(g.squad.as_ref().unwrap().pools.is_empty());
        let seed = g.rand.lcg.seed; let (a, b) = two_d(&mut g); assert!((1..=6).contains(&a) && (1..=6).contains(&b) && g.rand.lcg.seed != seed);
        g.squad = None; assert_eq!(npc_pool(&mut g, 6), None);
    }
}
