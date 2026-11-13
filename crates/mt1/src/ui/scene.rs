use crate::ground::state::{Ground, Mem, MAP_PTR, NPCS, NPC_SIZE, PARTY_PTR, WALKERS, WALKER_SIZE};
use std::sync::Arc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ScreenKind {
    Picture,
    Startup,
    Cg,
    Ground,
    Intro,
    Record,
    Space,
}
pub type Cell = (i16, i16);
pub type Traveller = u8;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit { Traveller(Traveller), Npc(u8) }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Target { Cell(Cell), Unit(Unit), Object(u8), Portrait(Traveller) }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Verb { Move, Fire, Take, Converse, Hold, Give }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Preset { #[default] Modern, Classic }
pub type ComponentId = &'static str;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlusAction { Open, Close, Tab(u8), Kind(u8), Text(char), Backspace, Submit, Scroll(i32) }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Intent {
    Plus(PlusAction),
    Hover(Option<Target>), Focus(Unit), PanTo(Cell), Pan(i32, i32), Zoom(f32), Follow(Option<Unit>),
    Hint(Option<&'static str>),
    BuyPrompt(Option<usize>),
    OpenMenu { target: Target, at: (i32, i32) }, CloseMenu, LogScroll(i32), LogResize(u8), LogExpand, Layout(Preset),
    Undock(ComponentId), Dock(ComponentId), Hide(ComponentId), ResetLayout,
    Select(Traveller), Verb { verb: Verb, target: Target }, MenuPick(u8), EndTurn, Continue,
    ClassicCombat(bool),
    BalanceOptions(crate::ground::rules::balance::Options),
    Raw(mt_engine::io::Event),
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MapCells {
    pub rows: u16, pub cols: u16, pub stride: u16,
    pub cells: Vec<u8>,
    pub tileset: u8, pub interior: u8,
}
impl MapCells {
    pub fn cell(&self, row: i32, col: i32) -> Option<[u8; 3]> {
        if row < 0 || col < 0 || row >= self.rows as i32 || col >= self.cols as i32 { return None }
        let at = (row as usize * self.cols as usize + col as usize) * 3;
        self.cells.get(at..at + 3).map(|c| [c[0], c[1], c[2]])
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Walker(pub [u8; WALKER_SIZE as usize]);
impl Walker {
    fn w(&self, at: usize) -> i16 { i16::from_le_bytes([self.0[at], self.0[at + 1]]) }
    pub fn active(&self) -> bool { self.0[0] != 0 }
    pub fn facing(&self) -> u8 { self.0[1] }
    pub fn hand(&self) -> u8 { self.0[2] }
    pub fn target(&self) -> i8 { self.0[5] as i8 }
    pub fn x(&self) -> i16 { self.w(8) }
    pub fn y(&self) -> i16 { self.w(10) }
    pub fn aim(&self) -> (i16, i16) { (self.w(12), self.w(14)) }
    pub fn waypoint(&self) -> (i16, i16) { (self.w(16), self.w(18)) }
    pub fn phase(&self) -> i16 { self.w(20) }
    pub fn route_len(&self) -> i16 { self.w(22) }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Npc(pub [u8; NPC_SIZE as usize]);
impl Npc {
    fn w(&self, at: usize) -> i16 { i16::from_le_bytes([self.0[at], self.0[at + 1]]) }
    pub fn level(&self) -> u8 { self.0[2] }
    pub fn home(&self) -> (i16, i16) { (self.w(3), self.w(5)) }
    pub fn movement(&self) -> u8 { self.0[7] }
    pub fn picture(&self) -> u8 { self.0[9] }
    pub fn sprite(&self) -> u8 { self.0[8] }
    pub fn aggression(&self) -> u8 { self.0[10] }
    pub fn attitude(&self) -> u8 { self.0[13] }
    pub fn armed(&self) -> u8 { self.0[31] }
    pub fn was_hit(&self) -> bool { self.0[0x26] != 0 }
    pub fn x(&self) -> i16 { self.w(32) }
    pub fn y(&self) -> i16 { self.w(34) }
    pub fn facing(&self) -> u8 { self.0[36] }
    pub fn steps(&self) -> u8 { self.0[37] }
    pub fn animation(&self) -> u8 { self.0[40] }
    pub fn weapon(&self) -> u8 { self.0[42] }
    pub fn present(&self) -> bool { self.0[32..36] != [0; 4] }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Object(pub [u8; 9]);
impl Object {
    pub fn next(&self) -> u8 { self.0[2] }
    pub fn glyph(&self) -> u8 { self.0[3] }
}
pub const OBJECTS: u16 = 0x602; pub const OBJECT_COUNT: usize = 128;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Party(pub Vec<u8>);
impl Party {
    fn b(&self, at: usize) -> u8 { self.0.get(at).copied().unwrap_or(0) }
    fn rec(i: Traveller) -> usize { 0x102 + 256 * (i as usize & 7) }
    pub fn mask(&self) -> u8 { self.b(0) }
    pub fn order(&self) -> [Traveller; 5] { std::array::from_fn(|i| self.b(1 + i)) }
    pub fn name(&self, i: Traveller) -> String { self.0.get(Self::rec(i)..).unwrap_or(&[]).iter().take_while(|&&b| b != 0).take(32).map(|&b| b as char).collect() }
    pub fn alive(&self, i: Traveller) -> bool { self.b(Self::rec(i) + 0x25) != 0 }
    pub fn hits(&self, i: Traveller) -> u8 { self.b(Self::rec(i) + 0x25) }
    pub fn max_hits(&self, i: Traveller) -> u8 { self.b(Self::rec(i) + 0x23) }
    pub fn split(&self, i: Traveller) -> u8 { self.b(Self::rec(i) + 0x24) }
    pub fn oxygen(&self, i: Traveller) -> u8 { self.b(Self::rec(i) + 0x26) }
    pub fn hand(&self, i: Traveller, hand: u8) -> Option<(u8, u8, u8)> {
        let at = Self::rec(i) + 0xCC + 2 * (21 + (hand & 1) as usize);
        let n = u16::from_le_bytes([self.b(at), self.b(at + 1)]) as usize;
        if n == 0 || n > 256 { return None }
        let o = 0xA82 + 5 * (n - 1);
        Some((self.b(o), self.b(o + 1), self.b(o + 2)))
    }
    pub fn objects(&self) -> Vec<Object> {
        (0..OBJECT_COUNT).map(|i| { let at = OBJECTS as usize + 9 * i; Object(std::array::from_fn(|k| self.b(at + k))) }).collect()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Squad {
    pub encounter: bool, pub round: u16, pub queue: Vec<Unit>,
    pub strip: String, pub log: Vec<String>, pub pips: [[bool; 2]; 5], pub to_act: Option<Traveller>, pub acting: Option<Unit>, pub alerted: u8,
    pub active: bool, pub selected: Traveller, pub menu: Option<crate::ground::orders::Menu>, pub hover: String, pub path: Vec<(Cell, u8)>,
    pub label: String,
    pub markers: Vec<Cell>,
    pub preview: Vec<String>, pub sight: Option<crate::ground::orders::Sight>,
    pub fx: crate::ground::round::Fx,
    pub mp: [i32; 5], pub evade: [bool; 5], pub hold: [bool; 5], pub full_auto: [bool; 5], pub pools: (i32, i32), pub destinations: [Option<Cell>; 5],
    pub log_n: u32,
    pub log_history: Vec<String>,
    pub tactics: String, pub stance: [&'static str; 5],
}
impl Squad {
    pub fn capture(g: &Ground) -> Squad {
        use crate::ground::{orders, round};
        let Some(s) = g.squad.as_ref() else { return Squad::default() };
        let p = &s.p1;
        let (preview, sight) = if s.orders.hover.is_empty() { (Vec::new(), None) } else { orders::preview(g, s.orders.target) };
        let label = !s.orders.hover.is_empty() && matches!(s.orders.target, Some(Target::Cell(_))) && !crate::ground::fire::aims_at_squares(g, round::selected(g));
        let pips = std::array::from_fn(|i| { let t = p.turns[i]; [s.encounter && !t.done && t.mp > 0, s.encounter && !t.done && !t.attacked] });
        Squad { encounter: s.encounter, round: s.round, queue: round::queue(g), strip: p.strip.clone(), log: p.log.iter().rev().take(8).rev().cloned().collect(), pips,
            to_act: match p.acting { Some(Unit::Traveller(t)) => Some(t), _ => None }, acting: p.acting, alerted: round::alerted_count(g) as u8,
            active: true, selected: round::selected(g), menu: s.orders.menu.clone(), path: s.orders.path.clone(),
            hover: if label { String::new() } else { s.orders.hover.clone() }, label: if label { s.orders.hover.clone() } else { String::new() },
            markers: s.fire.pending.iter().filter(|p| p.launcher).map(|p| p.cell).collect(),
            preview, sight, fx: p.fx.clone(), mp: std::array::from_fn(|i| p.turns[i].mp), evade: std::array::from_fn(|i| p.turns[i].evade), hold: std::array::from_fn(|i| p.turns[i].hold),
            full_auto: s.orders.full_auto, pools: p.encounter.as_ref().map_or((0, 0), |e| e.pools), destinations: std::array::from_fn(|i| orders::destination(g, i as u8)), log_n: p.logged,
            log_history: p.log.clone(), tactics: crate::ground::tactics::hud(g), stance: std::array::from_fn(|i| crate::ground::standing::stance(g, i as u8)) }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct MapArt {
    pub slots: Vec<u8>,
    pub phase: i16, pub last_pos: (i16, i16), pub moving: bool,
    pub individual: u8, pub orders: u8, pub npc_mode: u8,
    pub dark: bool, pub pen: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    pub context: super::context::Context,
    pub screen: ScreenKind,
    pub map: Arc<MapCells>,
    pub view: (i16, i16),
    pub pos: (i16, i16), pub heading: i16,
    pub leader: i16, pub combat: bool, pub level: i8, pub encounter_mode: u8, pub countdown: u8,
    pub shake: i8,
    pub walkers: [Walker; 5],
    pub npcs: Vec<Npc>, pub npc_count: u8,
    pub objects: Vec<Object>,
    pub party: Party,
    pub squad: Squad,
    pub art: MapArt,
}
impl Scene {
    pub fn program(screen: ScreenKind) -> Scene {
        Scene { context: Default::default(), screen, map: Arc::default(), view: (0, 0), pos: (0, 0), heading: 0, leader: 0, combat: false, level: 0, encounter_mode: 0, countdown: 100, shake: 0,
            walkers: [Walker([0; WALKER_SIZE as usize]); 5], npcs: Vec::new(), npc_count: 0, objects: Vec::new(), party: Party(Vec::new()), squad: Squad::default(), art: MapArt::default() }
    }
    pub fn capture(g: &Ground, screen: ScreenKind, last: Option<&Arc<MapCells>>) -> Scene {
        let m = &g.mem;
        let map = {
            let (rows, cols, stride) = (m.u8(0x5C3A) as u16 * 12, m.u8(0x5C3B) as u16 * 20, m.map_stride() as u16);
            let interior = m.u8(0x6FB3);
            let tileset = if interior.eq_ignore_ascii_case(&b'I') { 3 } else { m.u8(0x6FB4).wrapping_sub(b'1') };
            let p = m.pointer(MAP_PTR);
            let n = (rows as usize * cols as usize * 3).min(65535);
            let cells = if p == crate::ground::state::Far::default() || n == 0 { Vec::new() } else { m.bytes(p, n) };
            let fresh = MapCells { rows, cols, stride, cells, tileset, interior };
            match last { Some(a) if **a == fresh => a.clone(), _ => Arc::new(fresh) }
        };
        let walkers = std::array::from_fn(|i| { let p = Mem::walker(i as i16); Walker(std::array::from_fn(|k| m.read(p.add(k as i32), 1) as u8)) });
        let npcs = (1..=32).map(|i| { let p = Mem::npc(i); Npc(std::array::from_fn(|k| m.read(p.add(k as i32), 1) as u8)) }).collect();
        let party = Party(m.bytes(m.pointer(PARTY_PTR), Mem::PARTY_SIZE));
        let objects = party.objects();
        debug_assert_eq!((WALKERS, NPCS), (0x5C54, 0x63F4));
        Scene { context: super::context::Context::ground(g), screen, map, view: (m.view_x(), m.view_y()), pos: (m.x(), m.y()), heading: m.heading(), leader: m.leader(), combat: m.combat() != 0,
            level: m.level(), encounter_mode: m.encounter_mode(), countdown: m.countdown(), shake: 0, walkers, npcs, npc_count: m.npc_count(), objects, party, squad: Squad::capture(g),
            art: MapArt { slots: (0..64).map(|i| m.u8(0x4CDE + i)).collect(), phase: m.phase(), last_pos: (m.i16(0x285), m.i16(0x287)), moving: m.u16(0xE3) != 0,
                individual: m.individual(), orders: m.orders(), npc_mode: m.npc_mode(), dark: matches!(m.level(), 1 | 2) && m.party_u16(0xF82) == 0, pen: m.u8(0x52C8) } }
    }
}
#[derive(Clone, Debug, Default)]
pub struct SceneHook { pub screen: Option<ScreenKind>, pub map: Option<Arc<MapCells>>, pub shake: i8 }

#[cfg(test)]
mod t {
    use super::*;
    use mt_engine::io::{Input, Key, Overlay, Scripted};
    use mt_engine::sim::tick::Counter;
    use std::{cell::RefCell, rc::Rc};
    type Seen = Rc<RefCell<(usize, Option<Scene>)>>;
    struct Counting { inner: Scripted, seen: Seen }
    impl Input for Counting {
        fn next_key(&mut self) -> Key { self.inner.next_key() }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> mt_engine::io::Event { self.inner.poll(clock) }
        fn scene(&mut self, s: &dyn std::any::Any) { let mut seen = self.seen.borrow_mut(); seen.0 += 1; seen.1 = s.downcast_ref::<Scene>().cloned() }
    }
    fn ground(product: bool) -> (Ground, Seen) {
        let mut ds = vec![0u8; 0x10000];
        ds[0x5C3A] = 1; ds[0x5C3B] = 1; ds[0x6D2B] = 20; ds[0x6FB3] = b'O'; ds[0x6FB4] = b'2';
        ds[0xBC..0xC0].copy_from_slice(&[0, 0, 0, 0x80]);
        ds[0x6415] = 7; ds[0x6266] = 9; ds[0x6B6C] = 44; ds[0x63F5] = 10; ds[0x6B6A] = 2; ds[0x6D0E] = 1; ds[0xE8] = 3;
        ds[0x5C54 + 8] = 21; ds[0x5C54] = 1;
        ds[0x63F4 + 46 + 32] = 5; ds[0x63F4 + 46 + 13] = 2; ds[0x63F4 + 46 + 9] = 7;
        let mut party = vec![0u8; Mem::PARTY_SIZE]; party[0] = 0b11; party[1..6].copy_from_slice(&[1, 0, 2, 3, 4]);
        party[0x102..0x106].copy_from_slice(b"Vlar"); party[0x102 + 0x25] = 9; party[0x102 + 0x23] = 12; party[0x602 + 9 + 3] = 0x31; party[0x602 + 9 + 2] = 3;
        let seen: Seen = Rc::default();
        let mut g = Ground::from_ds(ds, party, Box::new(Counting { inner: Scripted::new([13, 13, 13]), seen: seen.clone() }), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.alloc(0x8000, 12 * 20 * 3); g.mem.write(crate::ground::state::Far::new(3 * 21 + 1, 0x8000), 0x42, 1);
        if product { g.scene = Some(SceneHook::default()) }
        (g, seen)
    }
    #[test]
    fn capture_is_a_pure_copy_with_named_fields() {
        let (g, _) = ground(true);
        let before = (g.mem.ds.clone(), g.mem.segs.clone(), g.fb.planes.clone(), g.fb.regs, g.rand.lcg.seed, g.mem.trace.done.len());
        let s = Scene::capture(&g, ScreenKind::Ground, None);
        assert_eq!(before, (g.mem.ds.clone(), g.mem.segs.clone(), g.fb.planes.clone(), g.fb.regs, g.rand.lcg.seed, g.mem.trace.done.len()));
        assert_eq!((s.screen, s.view, s.pos, s.leader, s.combat, s.level), (ScreenKind::Ground, (7, 9), (44, 10), 2, true, 3));
        assert_eq!((s.map.rows, s.map.cols, s.map.stride, s.map.tileset, s.map.cells.len()), (12, 20, 20, 1, 720));
        assert_eq!(s.map.cell(1, 1), Some([0, 0x42, 0])); assert_eq!(s.map.cell(12, 0), None);
        assert!(s.walkers[0].active() && s.walkers[0].x() == 21 && !s.walkers[1].active());
        assert_eq!((s.npcs.len(), s.npcs[0].x(), s.npcs[0].attitude(), s.npcs[0].picture(), s.npcs[0].present(), s.npcs[1].present()), (32, 5, 2, 7, true, false));
        assert_eq!((s.party.mask(), s.party.order(), s.party.name(0).as_str(), s.party.alive(0), s.party.hits(0), s.party.max_hits(0), s.party.alive(1)), (3, [1, 0, 2, 3, 4], "Vlar", true, 9, 12, false));
        assert_eq!((s.objects.len(), s.objects[1].glyph(), s.objects[1].next()), (128, 0x31, 3));
        assert_eq!(s.squad, Squad::default());
        let s2 = Scene::capture(&g, ScreenKind::Ground, Some(&s.map)); assert!(Arc::ptr_eq(&s.map, &s2.map));
        let mut g = g; g.mem.write(crate::ground::state::Far::new(0, 0x8000), 1, 1);
        let s3 = Scene::capture(&g, ScreenKind::Ground, Some(&s.map)); assert!(!Arc::ptr_eq(&s.map, &s3.map)); assert_eq!(s3.map.cell(0, 0), Some([1, 0, 0]));
        let p = Scene::program(ScreenKind::Cg); assert_eq!((p.screen, p.npcs.len(), p.party.mask()), (ScreenKind::Cg, 0, 0));
    }
    #[test]
    fn faithful_never_snapshots_product_snapshots_after_every_present() {
        let (mut g, seen) = ground(false);
        g.frame("redraw_view"); g.get_key(); g.screen_kind(ScreenKind::Record);
        assert_eq!(seen.borrow().0, 0);
        let (mut g, seen) = ground(true);
        g.frame("redraw_view"); assert_eq!(seen.borrow().0, 1);
        g.get_key(); assert_eq!((seen.borrow().0, seen.borrow().1.as_ref().unwrap().screen), (2, ScreenKind::Ground));
        g.screen_kind(ScreenKind::Record); g.frame("sheet"); assert_eq!((seen.borrow().0, seen.borrow().1.as_ref().unwrap().screen), (3, ScreenKind::Record));
        assert!(g.input.intent().is_none());
    }
}
