use crate::panels::{assets::Assets, viewport::Camera, Button, Surface, UiEvent, UiState};
use mt1::ui::{Cell, Intent, Scene, ScreenKind, Target, Verb};
use mt_engine::io::{Event, Key};

#[derive(Clone, Copy, Debug)]
struct Press { at: (i32, i32), cell: Cell, dragged: bool }
#[derive(Clone, Copy, Debug)]
enum FollowUp {
    Door { planned: Cell, door: Cell, key: Key, level: i8, selected: u8, combat: bool, log_n: u32 },
    Facing { planned: Cell, door: Cell, key: Key, heading: i16, level: i8, selected: u8, combat: bool },
    Cancel { target: Target, selected: u8 },
}

#[derive(Clone, Debug, Default)]
pub struct Walk { press: Option<Press>, pending: Option<FollowUp> }

pub fn cell(scene: &Scene, ui: &UiState, at: (i32, i32)) -> Option<Cell> {
    crate::mouse::pointer::map_cell(scene, ui, ui.component_input.size, at, false).map(crate::mouse::pointer::MapCell::row_col)
}
fn travelled(a: (i32, i32), b: (i32, i32)) -> bool {
    let (x, y) = (a.0.abs_diff(b.0) as u64, a.1.abs_diff(b.1) as u64);
    x >= 6 || y >= 6 || x * x + y * y >= 36
}
pub fn standing(scene: &Scene) -> bool {
    scene.squad.active && scene.squad.destinations.get(scene.squad.selected as usize).is_some_and(Option::is_some)
}
pub fn drop_moves(queue: &mut std::collections::VecDeque<Intent>) -> bool {
    let before = queue.len(); queue.retain(|i| !matches!(i, Intent::Verb { verb: Verb::Move, .. })); queue.len() != before
}
pub fn retarget(queue: &mut std::collections::VecDeque<Intent>, intents: &[Intent]) {
    if intents.iter().any(|i| matches!(i, Intent::Verb { verb: Verb::Move, .. })) { drop_moves(queue); }
}
pub fn cancel_after_replay(e: UiEvent, scene: &Scene, ui: &UiState, queue: &std::collections::VecDeque<Intent>) -> bool {
    matches!(e, UiEvent::Key(27) | UiEvent::Up(Button::Right)) && ui.menu.is_none()
        && (standing(scene) || ui.walk.borrow().pending.is_some() || queue.iter().any(|i| matches!(i, Intent::Verb { verb: Verb::Move, .. })))
}
fn position(scene: &Scene) -> Cell { (scene.pos.0 / 2, scene.pos.1 / 2) }
pub fn label(scene: &Scene, ui: &UiState, out: &mut Surface, camera: Camera, assets: &Assets) {
    let text = scene.squad.label.trim();
    let Some(Target::Cell((r, c))) = ui.hover else { return };
    if text.is_empty() || out.w == 0 || out.h == 0 { return }
    let (w, h) = (text.len() as i32 * 6 + 4, 12);
    let at = |col: f32| ((col - camera.left) * camera.zoom).round() as i32;
    let mut x = at(c as f32 * 16.0 + 17.0);
    if x + w > out.w as i32 { x = at(c as f32 * 16.0 - 1.0) - w; }
    let x = x.clamp(0, (out.w as i32 - w).max(0));
    let y = (((r as f32 * 16.0 - camera.top) * camera.zoom).round() as i32).clamp(0, (out.h as i32 - h).max(0));
    for dy in 0..h { for dx in 0..w { out.put(x + dx, y + dy, if dy == 0 || dy == h - 1 || dx == 0 || dx == w - 1 { 15 } else { 1 }) } }
    assets.text(out, text, (x + 2, y + 2), 15);
}
pub fn door(scene: &Scene, at: Cell) -> bool {
    scene.map.cell(at.0 as i32, at.1 as i32).is_some_and(|b|
        (scene.map.interior == b'I' && b[1] > 235) || (b[0] & 1 == 0 && b[1] == 5))
}
fn approach(scene: &Scene, at: Cell) -> bool {
    let Some([f, t, occ]) = scene.map.cell(at.0 as i32, at.1 as i32) else { return false };
    if occ & 63 != 0 || (scene.combat && occ & 64 != 0) { return false }
    if (f & 1 == 0 && (t <= 14 || matches!(t, 38 | 39))) || (f & 1 != 0 && t <= 4) { return false }
    if scene.combat && scene.walkers.iter().enumerate().any(|(i, w)| i != scene.squad.selected as usize && w.active() && (w.x() / 2, w.y() / 2) == at) { return false }
    (f & 1 != 0 && t / 20 < 5) || t / 20 < 2
}
fn direction(from: Cell, to: Cell) -> Option<Key> {
    Some(match (to.0 - from.0, to.1 - from.1) {
        (-1, -1) => 0x147, (-1, 0) => 0x148, (-1, 1) => 0x149,
        (0, -1) => 0x14b, (0, 1) => 0x14d,
        (1, -1) => 0x14f, (1, 0) => 0x150, (1, 1) => 0x151, _ => return None,
    })
}
impl Walk {
    pub fn gesture(&mut self, e: &UiEvent, at: (i32, i32), cell: Option<Cell>) -> bool {
        match e {
            UiEvent::Down(Button::Left) => { self.press = cell.map(|cell| Press { at, cell, dragged: false }); false }
            UiEvent::Move => { if let Some(p) = &mut self.press { p.dragged |= travelled(p.at, at) || cell != Some(p.cell); } false }
            UiEvent::Up(Button::Left) => self.press.take().is_some_and(|p| !p.dragged && !travelled(p.at, at) && cell == Some(p.cell)),
            UiEvent::Down(_) | UiEvent::Wheel(_) | UiEvent::Key(_) => { self.press = None; false }
            _ => false,
        }
    }
    pub fn reset(&mut self) { self.press = None; self.pending = None; }
    pub fn cancel(&mut self, scene: &Scene, at: (i32, i32)) -> Option<Vec<Intent>> {
        let pending = self.pending.take().is_some(); self.press = None;
        if scene.screen != ScreenKind::Ground || !scene.squad.active { return None }
        if standing(scene) {
            let target = Target::Cell(position(scene));
            self.pending = Some(FollowUp::Cancel { target, selected: scene.squad.selected });
            Some(vec![Intent::OpenMenu { target, at }])
        } else { pending.then(Vec::new) }
    }
    pub fn move_to(&mut self, scene: &Scene, target: Cell) -> Vec<Intent> {
        self.pending = None;
        if !door(scene, target) { return vec![Intent::Verb { verb: Verb::Move, target: Target::Cell(target) }] }
        if scene.squad.encounter || scene.context.event != -1 { return Vec::new() }
        let from = position(scene);
        let mut candidates: Vec<_> = (-1..=1).flat_map(|dr| (-1..=1).map(move |dc| (target.0 + dr, target.1 + dc)))
            .filter(|&c| c != target && (c == from || approach(scene, c))).collect();
        candidates.sort_by_key(|c| { let (r, k) = ((c.0 - from.0) as i32, (c.1 - from.1) as i32); (r.abs().max(k.abs()), r * r + k * k) });
        let Some(planned) = candidates.first().copied() else { return Vec::new() };
        let Some(key) = direction(planned, target) else { return Vec::new() };
        if planned == from { return self.bump(scene, target, key) }
        self.pending = Some(FollowUp::Door { planned, door: target, key, level: scene.level, selected: scene.squad.selected, combat: scene.combat, log_n: scene.squad.log_n });
        vec![Intent::Verb { verb: Verb::Move, target: Target::Cell(planned) }]
    }
    fn bump(&mut self, scene: &Scene, door: Cell, key: Key) -> Vec<Intent> {
        let heading = match key { 0x148 => 0, 0x149 => 1, 0x14d => 2, 0x151 => 3, 0x150 => 4, 0x14f => 5, 0x14b => 6, _ => 7 };
        if scene.heading != heading {
            self.pending = Some(FollowUp::Facing { planned: position(scene), door, key, heading, level: scene.level, selected: scene.squad.selected, combat: scene.combat });
        }
        super::decision::raw(&[Event::Key(key)])
    }
    pub fn at_decision(&mut self, kind: mt_engine::play::Kind, scene: Option<&Scene>) -> Vec<Intent> {
        if kind == mt_engine::play::Kind::Ground { if let Some(scene) = scene { return self.at_ground_wait(scene) } }
        self.pending = None; Vec::new()
    }
    pub fn at_ground_wait(&mut self, scene: &Scene) -> Vec<Intent> {
        if !matches!(self.pending, Some(FollowUp::Facing { .. })) { return Vec::new() }
        self.observe(scene);
        match self.pending.take() {
            Some(FollowUp::Facing { key, heading, .. }) if scene.heading == heading => super::decision::raw(&[Event::Key(key)]),
            _ => Vec::new(),
        }
    }
    pub fn observe(&mut self, scene: &Scene) -> Vec<Intent> {
        let Some(pending) = self.pending.take() else { return Vec::new() };
        if scene.screen != ScreenKind::Ground || !scene.squad.active || scene.context.event != -1 { return Vec::new() }
        match pending {
            FollowUp::Door { planned, door: at, key, level, selected, combat, log_n } => {
                if scene.squad.menu.is_some() || scene.squad.encounter || scene.level != level || scene.combat != combat || scene.squad.selected != selected || !door(scene, at) { return Vec::new() }
                if scene.squad.log_n == log_n { self.pending = Some(pending); return Vec::new() }
                if position(scene) != planned || scene.pos != (planned.0 * 2, planned.1 * 2) { return Vec::new() }
                self.bump(scene, at, key)
            }
            FollowUp::Facing { planned, door: at, level, selected, combat, .. } => {
                if scene.pos == (planned.0 * 2, planned.1 * 2) && scene.squad.menu.is_none() && !scene.squad.encounter
                    && scene.level == level && scene.squad.selected == selected && scene.combat == combat && door(scene, at) { self.pending = Some(pending); }
                Vec::new()
            }
            FollowUp::Cancel { target, selected } => {
                if scene.squad.selected != selected { return Vec::new() }
                let Some(menu) = scene.squad.menu.as_ref().filter(|m| m.target == target) else { return Vec::new() };
                menu.items.iter().position(|i| i.op == mt1::ground::orders::Op::HoldHere && i.grey.is_none())
                    .and_then(|i| u8::try_from(i).ok()).map_or_else(|| vec![Intent::CloseMenu], |i| vec![Intent::MenuPick(i)])
            }
        }
    }
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    use crate::panels::{viewport::Viewport, Component};
    use mt1::ground::orders::{Menu, MenuItem, Op};
    use std::{cell::RefCell, collections::VecDeque, path::Path, rc::Rc, sync::Arc};

    fn scene() -> Scene {
        let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.context.event = -1;
        s.pos = (8, 8);
        let m = Arc::make_mut(&mut s.map); m.rows = 12; m.cols = 20; m.stride = 20; m.interior = b'I';
        m.cells = [1, 5, 0].repeat(12 * 20); s
    }
    fn tile(s: &mut Scene, c: Cell, bytes: [u8; 3]) {
        let m = Arc::make_mut(&mut s.map); let i = (c.0 as usize * m.cols as usize + c.1 as usize) * 3; m.cells[i..i + 3].copy_from_slice(&bytes);
    }
    fn door_scene() -> Scene { let mut s = scene(); tile(&mut s, (4, 9), [1, 236, 0]); s }
    fn click(v: &Viewport, s: &Scene, ui: &UiState, c: Cell) -> Vec<Intent> {
        let cam = Camera::new(s, ui, ui.component_input.size);
        let at = (((c.1 as f32 * 16.0 + 8.0 - cam.left) * cam.zoom) as i32, ((c.0 as f32 * 16.0 + 8.0 - cam.top) * cam.zoom) as i32);
        v.input(s, ui, &UiEvent::Down(Button::Left), at); v.input(s, ui, &UiEvent::Up(Button::Left), at)
    }
    #[test]
    fn gestures_require_one_cell_and_less_than_six_pixels_total_excursion() {
        let mut w = Walk::default(); let c = Some((3, 4));
        assert!(!w.gesture(&UiEvent::Up(Button::Left), (20, 20), c));
        for (at, accept) in [((25, 20), true), ((26, 20), false), ((24, 24), true), ((25, 24), false), ((i32::MIN, i32::MAX), false)] {
            w.gesture(&UiEvent::Down(Button::Left), (20, 20), c);
            assert_eq!(w.gesture(&UiEvent::Up(Button::Left), at, c), accept);
        }
        w.gesture(&UiEvent::Down(Button::Left), (20, 20), c);
        w.gesture(&UiEvent::Move, (40, 20), c);
        assert!(!w.gesture(&UiEvent::Up(Button::Left), (20, 20), c), "drag out and back");
        w.gesture(&UiEvent::Down(Button::Left), (20, 20), c);
        assert!(!w.gesture(&UiEvent::Up(Button::Left), (21, 20), Some((3, 5))));
        for e in [UiEvent::Wheel(1), UiEvent::Key(27), UiEvent::Down(Button::Middle)] {
            w.gesture(&UiEvent::Down(Button::Left), (20, 20), c); w.gesture(&e, (20, 20), c);
            assert!(!w.gesture(&UiEvent::Up(Button::Left), (20, 20), c));
        }
    }
    #[test]
    fn cell_mapping_rejects_outside_viewport_and_map() {
        let s = scene(); let mut ui = UiState { camera: (3.5, 6.0), ..UiState::default() };
        assert_eq!(cell(&s, &ui, (8, 8)), Some((0, 0)));
        for at in [(-1, 20), (192, 20), (20, 112), (i32::MAX, i32::MIN)] { assert_eq!(cell(&s, &ui, at), None) }
        ui.camera = (-20.0, -20.0); assert_eq!(cell(&s, &ui, (8, 8)), None);
    }
    #[test]
    fn door_arrival_is_one_shot_and_every_interrupt_aborts() {
        let s = door_scene(); let mut w = Walk::default();
        assert_eq!(w.move_to(&s, (4, 9)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((4, 8)) }]);
        let mut arrived = s.clone(); arrived.pos = (8, 16); arrived.heading = 2; arrived.squad.log_n += 1;
        assert_eq!(w.observe(&arrived), [Intent::Raw(Event::Key(0x14d)), Intent::Raw(Event::KeyUp(0x14d))]); assert!(w.observe(&arrived).is_empty());
        for kind in 0..9 {
            w.move_to(&s, (4, 9)); let mut next = arrived.clone();
            match kind {
                0 => next.pos = (8, 14), 1 => next.squad.encounter = true, 2 => next.context.event = 9,
                3 => next.screen = ScreenKind::Picture, 4 => next.level += 1, 5 => next.squad.selected = 1,
                6 => next.squad.menu = Some(Menu { target: Target::Cell((0, 0)), items: vec![], default: 0 }),
                7 => next.pos = (9, 16), _ => next.combat = true,
            }
            assert!(w.observe(&next).is_empty(), "interrupt {kind}"); assert!(w.observe(&arrived).is_empty(), "never resumes");
        }
        w.move_to(&s, (4, 9)); w.move_to(&s, (8, 8)); assert!(w.observe(&arrived).is_empty(), "retarget drops door");
        w.move_to(&s, (4, 9)); assert_eq!(w.cancel(&s, (0, 0)), Some(vec![])); assert!(w.observe(&arrived).is_empty());
    }
    #[test]
    fn door_approach_avoids_walls_occupants_and_unreachable_enclosure() {
        let mut s = door_scene(); let mut w = Walk::default();
        tile(&mut s, (4, 8), [1, 0, 0]);
        tile(&mut s, (3, 8), [1, 5, 14]);
        assert_eq!(w.move_to(&s, (4, 9)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((5, 8)) }]);
        for r in 3..=5 { for c in 8..=10 { if (r, c) != (4, 9) { tile(&mut s, (r, c), [1, 0, 0]); } } }
        assert!(w.move_to(&s, (4, 9)).is_empty());
        s.pos = (8, 16); assert_eq!(w.move_to(&s, (4, 9)), [Intent::Raw(Event::Key(0x14d)), Intent::Raw(Event::KeyUp(0x14d))]);
        s.squad.encounter = true; assert!(w.move_to(&s, (4, 9)).is_empty());
        assert_eq!(direction((0, 0), (0, 0)), None);
        for dr in -1..=1 { for dc in -1..=1 { if (dr, dc) != (0, 0) { assert!(direction((3, 3), (3 + dr, 3 + dc)).is_some()); } } }
    }
    #[test]
    fn cancel_selects_the_offered_hold_here_not_combat_hold() {
        let mut s = scene(); s.squad.destinations[0] = Some((4, 19)); let mut w = Walk::default();
        let target = Target::Cell((4, 4));
        assert_eq!(w.cancel(&s, (8, 8)), Some(vec![Intent::OpenMenu { target, at: (8, 8) }]));
        s.squad.menu = Some(Menu { target, default: 0, items: vec![
            MenuItem { key: b'H', label: "Hold".into(), op: Op::Hold, grey: None },
            MenuItem { key: b'H', label: "Hold here".into(), op: Op::HoldHere, grey: None },
        ] });
        assert_eq!(w.observe(&s), [Intent::MenuPick(1)]);
        w.cancel(&s, (8, 8)); s.squad.menu.as_mut().unwrap().items[1].grey = Some("unavailable".into());
        assert_eq!(w.observe(&s), [Intent::CloseMenu]);
        w.cancel(&s, (8, 8)); s.squad.menu = None; assert!(w.observe(&s).is_empty());
    }
    #[test]
    fn viewport_drag_does_not_move_and_a_fresh_click_retargets() {
        let s = scene(); let ui = UiState { camera: (6.0, 10.0), zoom: 0.5, ..UiState::default() }; let v = Viewport::new(Arc::default());
        assert_eq!(click(&v, &s, &ui, (4, 10)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((4, 10)) }]);
        v.input(&s, &ui, &UiEvent::Down(Button::Left), (80, 40));
        v.input(&s, &ui, &UiEvent::Move, (100, 40));
        assert!(v.input(&s, &ui, &UiEvent::Up(Button::Left), (100, 40)).is_empty());
        assert_eq!(click(&v, &s, &ui, (4, 12)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((4, 12)) }]);
    }
    #[test]
    fn grenade_mouse_menu_throw_and_end_turn_keep_the_window_playable() {
        use crate::panels::{components, controls, Host, Layout};
        use mt1::{ground::{fire, orders, round, state::{Mem, W_BUILTIN}}, ui::ScreenKind};
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut g = crate::scenario::load(&root.join("scenarios/efate_broken.toml"), Path::new(&files)).unwrap().game;
        struct Idle;
        impl mt_engine::io::Input for Idle {
            fn next_key(&mut self) -> u16 { panic!("grenade mouse route entered a keyboard wait") }
            fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { clock.wait_tick(); Event::Idle }
        }
        g.input = Box::new(Idle); crate::scenario::attach_scene(&mut g); round::pass(&mut g, &[]);
        orders::select(&mut g, 0);
        g.mem.write(Mem::walker(0).add(W_BUILTIN), 1, 1); g.mem.set_party_u8(0x1CB, 2);
        let from = round::traveller_cell(&g, 0); let target = (from.0 - 2, from.1);
        assert!(fire::throw_check(&g, 0, target).is_ok());
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        let mut ui = UiState { fixed_screen: true, ..Default::default() };
        let mut store = super::super::decision::Store::default();
        let sync = |g: &mt1::ground::Ground, ui: &mut UiState, layout: &mut Layout, store: &mut super::super::decision::Store| {
            let s = Scene::capture(g, ScreenKind::Ground, None);
            controls::observe(&s, None, ui);
            store.record(ui, &mt1::play::turns::decision(g).expect("a playable combat wait"));
            if s.squad.menu.is_none() { ui.menu = None; }
            layout.arrange_for(&s, ui, (640, 480)); s
        };
        let click = |g: &mut mt1::ground::Ground, ui: &mut UiState, layout: &Layout, s: &Scene, id, at: (i32, i32), button| {
            let Host::Dock(r) = layout.host(id).unwrap() else { panic!("docked component") };
            assert_eq!(layout.hit(s, None, (r.x + at.0, r.y + at.1)).unwrap().0, id);
            let mut pending = Vec::new();
            for event in [UiEvent::Move, UiEvent::Down(button), UiEvent::Up(button)] {
                for i in controls::dispatch(layout, id, s, ui, &event, at) {
                    assert!(!matches!(i, Intent::Raw(_)), "no keyboard fallback: {i:?}");
                    ui.apply(i); pending.push(i);
                }
            }
            round::pass(g, &pending);
        };
        let s = sync(&g, &mut ui, &mut layout, &mut store);
        let camera = Camera::new(&s, &ui, layout.host("Viewport").unwrap().size());
        let at = (((target.1 as f32 * 16.0 + 8.0 - camera.left) * camera.zoom) as i32,
                  ((target.0 as f32 * 16.0 + 8.0 - camera.top) * camera.zoom) as i32);
        click(&mut g, &mut ui, &layout, &s, "Viewport", at, Button::Right);
        let s = sync(&g, &mut ui, &mut layout, &mut store);
        assert!(layout.visible("Menu", &s), "right-clicking grenade ground must publish visible menu rows");
        assert_eq!(g.mem.party_u8(0x1CB), 2);
        assert!(!g.squad.as_ref().unwrap().p1.turns[0].attacked);
        let Host::Dock(r) = layout.host("Menu").unwrap() else { panic!("menu dock") };
        let menu = s.squad.menu.as_ref().unwrap();
        let index = menu.items.iter().position(|i| i.op == Op::Throw).unwrap() as u8;
        let at = (0..r.h as i32).find_map(|y| {
            controls::dispatch(&layout, "Menu", &s, &mut ui, &UiEvent::Up(Button::Left), (4, y))
                .contains(&Intent::MenuPick(index)).then_some((4, y))
        }).expect("drawn Throw row is clickable");
        click(&mut g, &mut ui, &layout, &s, "Menu", at, Button::Left);
        let s = sync(&g, &mut ui, &mut layout, &mut store);
        assert_eq!(g.mem.party_u8(0x1CB), 1);
        let squad = g.squad.as_ref().unwrap();
        assert!(squad.p1.turns[0].attacked && !squad.p1.turns[0].done);
        assert_eq!(squad.fire.pending.len(), 1);
        assert!(squad.p1.log.iter().any(|l| l.contains("throws an explosive grenade")));
        assert!(!squad.p1.log.iter().any(|l| l.contains("goes off")));
        let size = layout.host("Combat controls").unwrap().size();
        click(&mut g, &mut ui, &layout, &s, "Combat controls", (size.0 as i32 - 8, 8), Button::Left);
        let s = sync(&g, &mut ui, &mut layout, &mut store);
        assert!(g.squad.as_ref().unwrap().fire.pending.is_empty());
        assert!(g.squad.as_ref().unwrap().p1.log.iter().any(|l| l.contains("explosive grenade goes off")));
        assert_ne!(s.squad.selected, 0, "the next Traveller can act");
        assert!(!super::super::decision::modal(&ui));
        assert!(layout.visible("Combat controls", &s));
    }
    #[test]
    fn queued_retarget_and_cancel_preserve_unrelated_input() {
        let a = Intent::Verb { verb: Verb::Move, target: Target::Cell((2, 2)) };
        let b = Intent::Verb { verb: Verb::Move, target: Target::Cell((4, 4)) };
        let mut q = VecDeque::from([Intent::Select(0), a]);
        retarget(&mut q, &[Intent::Hover(None)]); assert_eq!(q.len(), 2);
        retarget(&mut q, &[b]); q.push_back(b); assert_eq!(q, [Intent::Select(0), b]);
        assert!(drop_moves(&mut q)); assert_eq!(q, [Intent::Select(0)]); assert!(!drop_moves(&mut q));
    }
    #[test]
    fn replay_skip_also_cancels_a_route_but_does_not_issue_a_new_order() {
        let mut s = scene(); let mut ui = UiState::default(); let mut q = VecDeque::new();
        assert!(!cancel_after_replay(UiEvent::Up(Button::Right), &s, &ui, &q));
        s.squad.destinations[0] = Some((4, 10));
        for e in [UiEvent::Up(Button::Right), UiEvent::Key(27)] { assert!(cancel_after_replay(e, &s, &ui, &q)); }
        assert!(!cancel_after_replay(UiEvent::Up(Button::Left), &s, &ui, &q));
        ui.menu = Some((Target::Cell((0, 0)), (0, 0))); assert!(!cancel_after_replay(UiEvent::Key(27), &s, &ui, &q));
        ui.menu = None; s.squad.destinations[0] = None;
        q.push_back(Intent::Verb { verb: Verb::Move, target: Target::Cell((4, 10)) });
        assert!(cancel_after_replay(UiEvent::Key(27), &s, &ui, &q));
    }
    #[test]
    fn facing_finishes_only_at_ground_wait_and_never_retries_an_interrupt() {
        let mut s = door_scene(); s.pos = (8, 16); s.heading = 4; let mut w = Walk::default();
        assert_eq!(w.move_to(&s, (4, 9)), [Intent::Raw(Event::Key(0x14d)), Intent::Raw(Event::KeyUp(0x14d))]);
        s.heading = 2; assert!(w.observe(&s).is_empty(), "animation is not a command wait");
        assert_eq!(w.at_ground_wait(&s), [Intent::Raw(Event::Key(0x14d)), Intent::Raw(Event::KeyUp(0x14d))]); assert!(w.at_ground_wait(&s).is_empty());
        for interruption in 0..5 {
            s.heading = 4; w.move_to(&s, (4, 9)); let mut next = s.clone(); next.heading = 2;
            match interruption { 0 => next.context.event = 10, 1 => next.squad.encounter = true, 2 => next.pos.0 += 2, 3 => next.combat = true, _ => next.screen = ScreenKind::Picture }
            assert!(w.at_ground_wait(&next).is_empty()); assert!(w.at_ground_wait(&s).is_empty());
        }
    }
    #[test]
    fn modal_decision_interrupt_discards_follow_up_even_when_scene_is_still_ground() {
        let mut s = door_scene(); s.pos = (8, 16); s.heading = 4;
        for kind in [mt_engine::play::Kind::Menu, mt_engine::play::Kind::Ack, mt_engine::play::Kind::CombatTurn] {
            let mut w = Walk::default(); w.move_to(&s, (4, 9));
            assert!(w.at_decision(kind, Some(&s)).is_empty()); s.heading = 2;
            assert!(w.at_decision(mt_engine::play::Kind::Ground, Some(&s)).is_empty()); s.heading = 4;
        }
        let mut w = Walk::default(); w.move_to(&s, (4, 9));
        assert!(w.at_decision(mt_engine::play::Kind::Ground, None).is_empty());
    }
    #[test]
    fn classic_product_map_uses_displayed_camera_and_rejects_drags() {
        use crate::panels::{components, Layout};
        use mt1::ui::Preset;
        let s = scene(); let mut ui = UiState { preset: Preset::Classic, camera: (100.0, 100.0), zoom: 2.0, ..UiState::default() };
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default()); layout.arrange_for(&s, &ui, (320, 200));
        crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Down(Button::Left), (104, 40));
        let intent = crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), (104, 40));
        assert_eq!(intent, [Intent::Verb { verb: Verb::Move, target: Target::Cell((2, 6)) }]);
        crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Down(Button::Left), (104, 40));
        assert!(crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), (120, 40)).is_empty());
    }
    #[test]
    fn automatic_classic_input_uses_the_page_even_with_modern_camera_memory() {
        use crate::panels::{components, Layout};
        use mt1::ui::Preset;
        let mut s = scene(); s.pos = (18, 26); s.view = (2, 4);
        let mut ui = UiState { fixed_screen: true, ..Default::default() };
        let camera = Camera::new(&s, &ui, (192, 112)); assert_ne!(camera.top, 16.0);
        let mut layout = Layout::default(); components::register(&mut layout, Arc::default());
        layout.arrange_for(&s, &UiState { preset: Preset::Classic, ..ui.clone() }, (320, 200));
        let at = (104, 40); let expected = (3, 8);
        assert_eq!(super::super::pointer::map_cell(&s, &ui, (192, 112), at, true).unwrap().row_col(), expected);
        crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Down(Button::Left), at);
        assert_eq!(crate::panels::controls::dispatch(&layout, "Viewport", &s, &mut ui, &UiEvent::Up(Button::Left), at),
            [Intent::Verb { verb: Verb::Move, target: Target::Cell(expected) }]);
    }
    #[test]
    fn synthetic_map_move_uses_sim_bfs_around_walls_and_refuses_closed_door() {
        use mt1::ground::{orders, state::{Far, Ground, Mem, MAP_PTR}};
        use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
        let mut s = scene(); s.pos = (10, 4);
        for r in 0..12 { if r != 7 { tile(&mut s, (r, 6), [1, 0, 0]); } }
        let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.set_pointer(MAP_PTR, Far::new(0, 0x8000)); g.mem.segs.insert(0x8000, s.map.cells.clone());
        g.mem.set_map_stride(20); g.mem.set_map_blocks_high(1);
        let ui = UiState { camera: (6.0, 10.0), zoom: 0.5, ..UiState::default() }; let v = Viewport::new(Arc::default());
        assert_eq!(click(&v, &s, &ui, (5, 10)), [Intent::Verb { verb: Verb::Move, target: Target::Cell((5, 10)) }]);
        let path = orders::find_path(&g, 0, (5, 2), (5, 10)).unwrap();
        assert!(path.contains(&(7, 6))); assert_eq!(path.last(), Some(&(5, 10)));
        tile(&mut s, (7, 6), [1, 237, 0]); g.mem.segs.insert(0x8000, s.map.cells.clone());
        assert!(orders::find_path(&g, 0, (5, 2), (5, 10)).is_none());
        assert!(orders::find_path(&g, 0, (5, 2), (7, 6)).is_none());
    }

    #[test]
    fn scrolled_mouse_beyond_page_keeps_target_and_sim_refuses_offpage_fire() {
        use mt1::{ground::{orders, round, state::Mem}, ui::Unit};
        use mt_engine::io::Input;
        let Some(files) = oracle_env::gamefiles() else { return };
        struct Idle;
        impl Input for Idle {
            fn next_key(&mut self) -> u16 { panic!("unexpected modal wait") }
            fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { clock.wait_tick(); Event::Idle }
        }
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut g = crate::scenario::load(&root.join("scenarios/efate_broken.toml"), Path::new(&files)).unwrap().game;
        g.input = Box::new(Idle); crate::scenario::attach_scene(&mut g);
        orders::select(&mut g, 2); g.mem.set_view_x(38); g.mem.set_view_y(0);
        let n = Mem::npc(14); let old = (g.mem.read_i(n.add(32), 2) as i16, g.mem.read_i(n.add(34), 2) as i16);
        mt1::ground::walk::release_cell(&mut g, old.0, old.1);
        g.mem.write(n.add(32), 52, 2); g.mem.write(n.add(34), 10, 2); mt1::ground::walk::set_cell_flags(&mut g, 52, 10, 14);
        let before = Scene::capture(&g, ScreenKind::Ground, None);
        let mut ui = UiState { fixed_screen: true, ..Default::default() }; ui.component_input.size = (384, 224);
        let v = Viewport::new(Arc::default()); Camera::new(&before, &ui, ui.component_input.size);
        for i in click(&v, &before, &ui, (24, 5)) { orders::intent(&mut g, i); }
        let s = Scene::capture(&g, ScreenKind::Ground, Some(&before.map)); assert_eq!(s.pos, (48, 10));
        assert_eq!(Camera::new(&s, &ui, (192, 112)).top, 20.0 * 16.0);
        assert!(!round::on_screen(&g, (26, 5)));
        let cam = Camera::new(&s, &ui, ui.component_input.size);
        let at = (((5.0 * 16.0 + 8.0 - cam.left) * cam.zoom) as i32, ((26.0 * 16.0 + 8.0 - cam.top) * cam.zoom) as i32);
        assert_eq!(cell(&s, &ui, at), Some((26, 5)));
        assert_eq!(super::super::pointer::map_cell(&s, &ui, ui.component_input.size, at, false).unwrap().row_col(), (26, 5));
        let target = Target::Unit(Unit::Npc(14));
        let right = v.input(&s, &ui, &UiEvent::Up(Button::Right), at);
        assert_eq!(right, [Intent::Hover(Some(target)), Intent::OpenMenu { target, at }]); assert_eq!(ui.info.get().map(|i| i.0), Some(target));
        let state = (g.mem.party().to_vec(), s.npcs.clone(), g.rand.lcg.seed, s.walkers);
        for i in right { orders::intent(&mut g, i); }
        let left = click(&v, &s, &ui, (26, 5)); assert_eq!(left, [Intent::Verb { verb: Verb::Fire, target }]);
        for i in left { orders::intent(&mut g, i); }
        let refused = Scene::capture(&g, ScreenKind::Ground, None);
        assert!(refused.squad.log.iter().any(|s| s.contains("target outside the screen: cannot fire")));
        assert_eq!(state, (g.mem.party().to_vec(), refused.npcs.clone(), g.rand.lcg.seed, refused.walkers), "no ammo, RNG, victim or standing target change");
        let to = (26, 6); let moves = click(&v, &s, &ui, to);
        assert_eq!(moves, [Intent::Verb { verb: Verb::Move, target: Target::Cell(to) }]);
        let from = round::traveller_cell(&g, 2); let path = orders::find_path(&g, 2, from, to).unwrap_or_default();
        for i in moves { orders::intent(&mut g, i); }
        let end = round::traveller_cell(&g, 2);
        assert!(end == from || path.contains(&end), "move must follow the requested square's route: {end:?}, {path:?}");
        let after = Scene::capture(&g, ScreenKind::Ground, None); assert_eq!(after.npcs, refused.npcs);
    }

    #[test]
    fn walk_label_is_a_small_box_by_the_square_and_never_the_stripe() {
        let mut s = scene(); let ui = UiState { camera: (6.0, 10.0), zoom: 1.0, hover: Some(Target::Cell((4, 10))), ..UiState::default() };
        let stripe = |s: &Scene| !crate::panels::tooltip::text(s, &UiState { hover: Some(Target::Cell((4, 10))), ..UiState::default() }).is_empty(); let assets = Assets::default();
        let cam = Camera::new(&s, &ui, (192, 112)); let blank = Surface::new(192, 112);
        let mut out = blank.clone(); label(&s, &ui, &mut out, cam, &assets); assert_eq!(out.indexed, blank.indexed, "no label: nothing drawn");
        s.squad.label = "4 squares".into(); assert!(!stripe(&s), "a walk text raises no stripe");
        label(&s, &ui, &mut out, cam, &assets);
        let (x, y, w, h) = (((10.0 * 16.0 + 17.0 - cam.left) * cam.zoom) as i32, ((4.0 * 16.0 - cam.top) * cam.zoom) as i32, 9 * 6 + 4, 12);
        for dy in 0..h { for dx in 0..w { let c = out.indexed[(y + dy) as usize * out.w + (x + dx) as usize]; if dy == 0 || dy == h - 1 || dx == 0 || dx == w - 1 { assert_eq!(c, 15, "border {dx},{dy}") } else { assert!(c == 1 || c == 15, "inside {dx},{dy}: {c}") } } }
        let painted: Vec<usize> = out.indexed.iter().enumerate().filter(|(_, &c)| c != 0).map(|(i, _)| i).collect();
        assert!(painted.iter().all(|&i| { let (px, py) = ((i % out.w) as i32, (i / out.w) as i32); (x..x + w).contains(&px) && (y..y + h).contains(&py) }), "nothing outside the box");
        let edge = UiState { hover: Some(Target::Cell((4, 10))), camera: (6.0, 10.0 - 4.0), ..ui.clone() }; let cam2 = Camera::new(&s, &edge, (192, 112));
        let mut out2 = blank.clone(); label(&s, &edge, &mut out2, cam2, &assets); let xs: Vec<usize> = out2.indexed.iter().enumerate().filter(|(_, &c)| c != 0).map(|(i, _)| i % out2.w).collect();
        assert!(xs.iter().max().unwrap() < &((10.0 * 16.0 - 1.0 - cam2.left) as usize), "at the right edge the box sits left of the square");
        s.squad.label.clear(); s.squad.hover = "Hostile (thug, NPC 14): no Attack left".into(); assert!(stripe(&s), "a battle message keeps the stripe");
    }
    #[test]
    fn efate_quiet_click_on_the_door_enters_the_building_with_zero_keys() {
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut g = crate::scenario::load(&root.join("scenarios/efate_quiet.toml"), Path::new(&files)).unwrap().game;
        g.scene = Some(mt1::ui::SceneHook::default()); mt1::ground::squad::set_classic(&mut g, false);
        let door = (31, 9); assert_eq!((g.mem.x() / 2, g.mem.y() / 2), (32, 9)); assert!(mt1::ground::orders::find_path(&g, 0, (32, 9), door).is_some(), "the door is a legal last step");
        let run = Rc::new(RefCell::new(Run::default())); let mut ui = UiState::default(); ui.component_input.size = (640, 400);
        g.input = Box::new(MouseHarness { shared: run.clone(), ui, viewport: Viewport::new(Arc::default()), target: door, remaining: VecDeque::new(), finish_on_modal: true });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = result { assert!(e.is::<HarnessStop>(), "unexpected harness panic"); }
        let r = run.borrow(); assert!(r.clicked); assert_eq!(r.keys, 0);
        assert!(r.arrived, "last {:?}, polls {}", r.last.as_ref().map(|s| (s.screen, s.pos, &s.squad.log)), r.polls);
        assert_ne!(r.last.as_ref().unwrap().screen, ScreenKind::Ground, "the building's screen");
    }
    #[test]
    fn efate_neutral_east_of_the_party_is_waited_for_by_key_and_by_click() {
        use mt1::ground::{orders, round, state::{Mem, N_ATTITUDE}, walk};
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut g = crate::scenario::load(&root.join("scenarios/efate_shop.toml"), Path::new(&files)).unwrap().game;
        g.scene = Some(mt1::ui::SceneHook::default()); mt1::ground::squad::set_classic(&mut g, false);
        let from = round::traveller_cell(&g, round::selected(&g)); let east = (from.0, from.1 + 1);
        let k = 14u8; let n = Mem::npc(k as i16); let old = round::npc_cell(&g, k);
        walk::release_cell(&mut g, old.0 * 2, old.1 * 2); g.mem.write(n.add(32), (east.0 * 2) as u16 as u32, 2); g.mem.write(n.add(34), (east.1 * 2) as u16 as u32, 2);
        g.mem.write(n.add(N_ATTITUDE), 2, 1); walk::set_cell_flags(&mut g, east.0 * 2, east.1 * 2, k as i16);
        assert!(round::neutral(&g, k) && !round::in_encounter(&g), "a calm passer-by, no encounter");
        let (d, before) = (orders::direction(&g, (0, 1)).unwrap(), g.clock.now()); g.mem.set_heading(d as i16);
        walk::move_party(&mut g, 0, 1, d as i16); let blocked = g.blocked_by.take(); assert_eq!(blocked, Some(k), "the key bump is refused by him");
        orders::wait_for(&mut g, k); assert!(g.clock.now() >= before + mt_engine::sim::sched::SECOND);
        let s = Scene::capture(&g, ScreenKind::Ground, None); assert!(s.squad.log.last().is_some_and(|l| l.starts_with("Waiting for ") && l.ends_with("to move")), "{:?}", s.squad.log);
        let to = (east.0, east.1 + 1); let before = g.clock.now();
        orders::intent(&mut g, Intent::Hover(Some(Target::Cell(to)))); orders::intent(&mut g, Intent::Verb { verb: Verb::Move, target: Target::Cell(to) });
        assert_eq!(g.waiting, Some((round::selected(&g), to)), "the click route waits: {:?}", Scene::capture(&g, ScreenKind::Ground, None).squad.log);
        assert!(g.clock.now() >= before + mt_engine::sim::sched::SECOND); assert_eq!(round::traveller_cell(&g, round::selected(&g)), from);
        let s = Scene::capture(&g, ScreenKind::Ground, None); assert_eq!(s.squad.label, "waiting"); assert!(!orders::read_route(&g, round::selected(&g)).is_empty(), "the route stands");
    }
    struct MouseHarness {
        shared: Rc<RefCell<Run>>, ui: UiState, viewport: Viewport, target: Cell,
        remaining: VecDeque<Cell>, finish_on_modal: bool,
    }
    #[derive(Default)]
    struct Run { clicked: bool, arrived: bool, polls: usize, keys: usize, log_start: u32, intents: VecDeque<Intent>, events: VecDeque<Event>, last: Option<Scene> }
    impl mt_engine::io::Input for MouseHarness {
        fn wants_decision(&self) -> bool { self.shared.borrow().events.is_empty() }
        fn decide(&mut self, d: &mt_engine::play::Decision) {
            let s = self.shared.borrow().last.clone();
            for i in self.ui.walk.borrow_mut().at_decision(d.kind, s.as_ref()) { if let Intent::Raw(e) = i { self.shared.borrow_mut().events.push_back(e); } }
        }
        fn next_key(&mut self) -> Key { self.shared.borrow_mut().keys += 1; 27 }
        fn key_pending(&mut self) -> bool { self.shared.borrow().events.iter().any(|e| matches!(e, Event::Key(_))) }
        fn closed(&self) -> bool { let r = self.shared.borrow(); r.arrived || r.polls >= 200 }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event {
            let mut r = self.shared.borrow_mut(); r.polls += 1;
            if r.arrived || r.polls >= 200 { std::panic::panic_any(HarnessStop); }
            if let Some(e) = r.events.pop_front() { if matches!(e, Event::Key(_)) { r.keys += 1; } e } else { clock.wait_tick(); Event::Idle }
        }
        fn scene(&mut self, any: &dyn std::any::Any) {
            let Some(s) = any.downcast_ref::<Scene>() else { return };
            let mut r = self.shared.borrow_mut(); r.last = Some(s.clone());
            if self.finish_on_modal && r.clicked && s.screen != ScreenKind::Ground { r.arrived = true; return }
            if s.screen != ScreenKind::Ground || !s.squad.active { return }
            if r.clicked && s.pos == (self.target.0 * 2, self.target.1 * 2) && s.squad.log_n > r.log_start {
                if let Some(next) = self.remaining.pop_front() { self.target = next; r.clicked = false; }
                else { r.arrived = true; }
            }
            let intents = if !r.clicked {
                self.ui.camera = (s.pos.0 as f32 / 2.0 + 0.5, s.pos.1 as f32 / 2.0 + 0.5); r.clicked = true; r.log_start = s.squad.log_n;
                click(&self.viewport, s, &self.ui, self.target)
            } else { self.ui.walk.borrow_mut().observe(s) };
            for i in intents { match i { Intent::Raw(e) => r.events.push_back(e), _ => r.intents.push_back(i) } }
        }
        fn intent(&mut self) -> Option<Box<dyn std::any::Any>> {
            if self.shared.borrow().arrived { std::panic::panic_any(HarnessStop); }
            self.shared.borrow_mut().intents.pop_front().map(|i| Box::new(i) as Box<dyn std::any::Any>)
        }
    }
    #[derive(Debug)]
    struct HarnessStop;
    #[test]
    fn efate_quiet_mouse_harness_walks_six_squares_with_zero_keys() {
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let mut g = crate::scenario::load(&root.join("scenarios/efate_quiet.toml"), Path::new(&files)).unwrap().game;
        g.scene = Some(mt1::ui::SceneHook::default());
        mt1::ground::squad::set_classic(&mut g, false);
        let run = Rc::new(RefCell::new(Run::default()));
        let mut ui = UiState::default(); ui.component_input.size = (640, 400);
        g.input = Box::new(MouseHarness { shared: run.clone(), ui, viewport: Viewport::new(Arc::default()), target: (26, 9), remaining: VecDeque::new(), finish_on_modal: false });
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        if let Err(e) = result { assert!(e.is::<HarnessStop>(), "unexpected harness panic"); }
        let r = run.borrow(); assert!(r.clicked); assert!(r.arrived, "last {:?}, polls {}", r.last.as_ref().map(|s| (&s.pos, &s.squad.log)), r.polls);
        assert_eq!(r.keys, 0); assert_eq!((g.mem.x(), g.mem.y()), (52, 18));
    }
    #[test]
    fn efate_doors_mouse_harness_stops_on_refusal_and_enters_starport() {
        let Some(files) = oracle_env::gamefiles() else { return };
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (file, target) in [("efate_door.toml", (6, 9)), ("m5_starport_desk_efate.toml", (33, 5)), ("m5_starport_desk_efate.toml", (33, 3))] {
            let mut g = crate::scenario::load(&root.join("scenarios").join(file), Path::new(&files)).unwrap().game;
            g.scene = Some(mt1::ui::SceneHook::default()); mt1::ground::squad::set_classic(&mut g, false);
            let s = Scene::capture(&g, ScreenKind::Ground, None);
            let starport = file.starts_with("m5_");
            let run = Rc::new(RefCell::new(Run::default())); let mut ui = UiState::default(); ui.component_input.size = (640, 400);
            g.input = Box::new(MouseHarness { shared: run.clone(), ui, viewport: Viewport::new(Arc::default()), target,
                remaining: if target == (33, 5) { VecDeque::from([(33, 3)]) } else { VecDeque::new() }, finish_on_modal: starport });
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
            if let Err(e) = result { assert!(e.is::<HarnessStop>(), "unexpected harness panic"); }
            let r = run.borrow(); assert!(r.clicked);
            if starport {
                assert!(r.arrived, "last {:?}, keys {}", r.last.as_ref().map(|s| (s.screen, s.pos, s.heading, &s.squad.log)), r.keys);
                assert_eq!(r.last.as_ref().unwrap().screen, ScreenKind::Picture, "the starport panel");
            }
            else { assert!(!r.arrived); assert_eq!(r.last.as_ref().unwrap().pos, s.pos); assert_eq!(r.keys, 1); assert!(r.events.is_empty()); assert_eq!(g.mem.key(), 0); }
        }
    }
}
