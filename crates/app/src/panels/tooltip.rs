use super::{assets::Assets, Anchor, Component, Dock, Host, Layout, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene, ScreenKind};
use std::sync::Arc;

pub const ID: &str = "Tooltip";
pub struct Tooltip { assets: Arc<Assets> }
impl Tooltip {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Float(Anchor::Pointer), size: Size { preferred: (320, 12), min: (1, 12) }, order: 95,
            screens: &[ScreenKind::Ground, ScreenKind::Space], when: |_| true, origin: Origin::New,
            optional: false, undockable: false, title: ID }
    }
}

pub const FORMATION: &str = "Party formation";
pub fn formation(scene: &Scene) -> String {
    let p = &scene.party;
    let names: Vec<String> = p.order().iter().filter(|&&i| i < 5 && p.mask() & (1 << i) != 0).map(|&i| {
        let n = p.name(i); if !p.alive(i) { format!("{n} (dead)") } else if super::party_strip::down(scene, i) { format!("{n} (down)") } else { n }
    }).collect();
    let mut t = String::from("Marching order: ");
    match names.len() { 0 => t.push_str("nobody"), _ => {
        t.push_str(&names[0]); t.push_str(" leads");
        for pair in names[1..].chunks(2) { t.push_str("; "); t.push_str(&pair.join(", ")); }
    } }
    t
}
pub fn pinned(ui: &UiState) -> u32 {
    ui.info.get().filter(|(t, _)| Some(*t) == ui.hover).map_or(1, |(_, n)| n.max(1) as u32)
}
pub fn text(scene: &Scene, ui: &UiState) -> String {
    if let (ScreenKind::Ground, true, Some(("Party strip", _)), Some(mt1::ui::Target::Portrait(i))) = (scene.screen, scene.squad.active, ui.components.pointer, ui.hover) { return format!("{} / Right-click: record", scene.party.name(i)) }
    if let Some(p) = super::party_strip::picker_text(scene, ui) { return match super::party_strip::place_tip(scene, ui.hint) { Some(h) => format!("{h}. {p}"), None => p } }
    let mut t = scene.squad.hover.clone();
    if t.trim().is_empty() { if let Some(h) = super::party_strip::place_tip(scene, ui.hint) { return h } return match ui.hint { Some(FORMATION) => formation(scene), h => h.unwrap_or("").to_string() } }
    if let Some(s) = scene.squad.sight { t.push_str(if s.los { " / in view" } else { " / out of sight" }); }
    t
}

pub fn rows(scene: &Scene, ui: &UiState, cols: usize) -> Vec<String> {
    let cols = cols.max(1);
    std::iter::once(text(scene, ui)).chain(scene.squad.preview.iter().take(pinned(ui).saturating_sub(1) as usize).cloned())
        .flat_map(|l| { let l: String = l.chars().map(|c| if c.is_control() { ' ' } else { c }).collect(); let w = super::side_panel::wrap(&l, cols); if w.is_empty() { vec![String::new()] } else { w } }).collect()
}
pub fn visible(ui: &UiState) -> bool {
    ui.menu.is_none() && (picker(ui) || ui.hover.is_some() || ui.hint.is_some())
        && (picker(ui) || ui.components.pointer.is_some_and(|(id, at)| matches!(id, "Viewport" | "Party strip" | "Initiative order" | "Picture" | super::components::MINIMAP_ID) && at.0 >= 0 && at.1 >= 0))
}
fn picker(ui: &UiState) -> bool { super::party_strip::picking(ui) || { let n = ui.components.picker.lock().unwrap(); n.said_recently() || n.pending() } }

pub fn geometry(layout: &Layout, scene: &Scene, ui: &UiState) -> Option<(Rect, Option<u64>)> {
    if !visible(ui) || text(scene, ui).trim().is_empty() { return None }
    let pick = picker(ui) && super::party_strip::picker_text(scene, ui).is_some();
    let (id, at) = if pick { ("Party strip", (0, 0)) } else { ui.components.pointer? };
    if ui.host_kind(id) == super::HostKind::Hidden { return None }
    let h = layout.host(id)?;
    if !Rect::new(0, 0, h.size().0, h.size().1).contains(at) { return None }
    let (origin, parent, size) = match h {
        Host::Dock(r) => ((r.x, r.y), None, (r.w, r.h)),
        Host::Window { id, size } => ((0, 0), Some(id), size),
    };
    let beside = (pick || scene.squad.hover.trim().is_empty() && (ui.hint == Some(FORMATION) || super::party_strip::PLACE_TIPS.iter().any(|&t| ui.hint == Some(t))) && id == "Party strip").then_some(super::party_strip::FORMATION_RECT);
    let beside = if pick { beside.map(|b| Rect::new(super::party_strip::O_RECT.x, b.y, b.w, b.h)) } else { beside };
    let cols = beside.map_or(size.0, |b| (b.x as u32).min(size.0)).saturating_sub(if pick { 6 } else { 4 }) / 6;
    let lines = rows(scene, ui, cols as usize);
    let chars = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as u32;
    let width = (chars * 6 + 4).clamp(1, size.0.max(1));
    let height = (2 + 10 * lines.len() as u32).min(size.1);
    let r = match beside { Some(b) if width + 2 <= b.x as u32 => Rect::new(b.x - width as i32 - 2, b.y.min(size.1.saturating_sub(height) as i32), width, height), _ => rect(at, (width, height), size) };
    Some((Rect::new(r.x.saturating_add(origin.0), r.y.saturating_add(origin.1), r.w, r.h), parent))
}

pub fn rect(at: (i32, i32), (width, height): (u32, u32), size: (u32, u32)) -> Rect {
    let x = at.0.saturating_add(8).clamp(0, size.0.saturating_sub(width) as i32);
    let y = if i64::from(at.1) + 14 + i64::from(height) <= i64::from(size.1) { at.1.saturating_add(14) }
        else { at.1.saturating_sub(height as i32 + 4).max(0) };
    Rect::new(x, y.min(size.1.saturating_sub(height) as i32), width, height)
}

impl Component for Tooltip {
    fn visible(&self, ui: &UiState) -> bool { visible(ui) }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        out.indexed.fill(1);
        for (i, line) in rows(scene, ui, out.w.saturating_sub(4) / 6).iter().enumerate() { self.assets.text(out, line, (2, 2 + 10 * i as i32), 15); }
    }
    fn input(&self, _: &Scene, _: &UiState, _: &UiEvent, _: (i32, i32)) -> Vec<Intent> { Vec::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt1::ui::{Target, Unit};
    fn setup() -> (Layout, Scene, UiState) {
        let mut l = Layout::default(); super::super::components::register(&mut l, Arc::default());
        let mut s = Scene::program(ScreenKind::Ground); s.squad.hover = "Vlaris: ready".into();
        let ui = UiState { hover: Some(Target::Unit(Unit::Traveller(0))), ..UiState::default() };
        l.arrange_for(&s, &ui, (640, 400)); (l, s, ui)
    }
    #[test]
    fn text_is_only_the_sim_hover_and_has_one_line() {
        let (_, mut s, mut ui) = setup();
        {
            s.squad.active = true; s.party.0 = vec![0; 0x1000]; s.party.0[0x102..0x10a].copy_from_slice(b"Magellan");
            ui.hover = Some(Target::Portrait(0)); ui.components.pointer = Some(("Party strip", (20, 20)));
            assert_eq!(text(&s, &ui), "Magellan / Right-click: record");
            ui.components.pointer = Some(("Viewport", (20, 20))); assert_eq!(text(&s, &ui), "Vlaris: ready");
            ui.hover = Some(Target::Unit(Unit::Traveller(0))); ui.components.pointer = None; s.squad.active = false;
        }
        s.squad.hover = "Door: closed\nnext".into();
        let c = Tooltip::new(Arc::default()); let before = s.clone(); let mut out = Surface::new(200, 12);
        c.draw(&s, &ui, &mut out);
        let bytes: Vec<_> = out.text.visible.iter().flat_map(|r| r.bytes.iter().copied()).collect();
        assert_eq!(String::from_utf8(bytes).unwrap(), "Door: closed next");
        assert!(out.text.visible.iter().all(|r| r.y == 2)); assert_eq!(s, before);
        c.draw(&s, &ui, &mut Surface::new(0, 0));
        let mut small = Surface::new(40, 24); c.draw(&s, &ui, &mut small);
        s.squad.hover = "Hostile (thug, NPC 16): Short: Routine 7+, Dennar-and-a-very-long-name".into();
        let joined: String = rows(&s, &ui, 12).join(" ").split_whitespace().collect::<Vec<_>>().join("");
        assert_eq!(joined, s.squad.hover.split_whitespace().collect::<Vec<_>>().join(""), "every character of the hover text is laid out");
        assert!(rows(&s, &ui, 12).iter().all(|r| r.chars().count() <= 12), "{:?}", rows(&s, &ui, 12));
        s.squad.hover = "Door: closed
next".into();
        assert!(small.text.visible.iter().all(|r| r.bytes != b"~") && small.text.visible.iter().any(|r| r.y == 12), "{:?}", small.text.visible.iter().map(|r| (r.y, r.bytes[0] as char)).collect::<Vec<_>>());
        assert!(c.input(&s, &ui, &UiEvent::Up(super::super::Button::Left), (2, 2)).is_empty());
    }
    #[test]
    fn formation_hover_names_the_marching_order_beside_the_box() {
        let (mut l, mut s, mut ui) = setup(); s.squad.hover.clear(); s.squad.sight = None;
        s.party.0 = vec![0; 0x1000]; s.party.0[0] = 0b11111; s.party.0[1..6].copy_from_slice(&[0, 1, 2, 3, 4]);
        for (i, n) in ["Magellan", "Jurgen", "Effane", "Dennar", "Verti"].iter().enumerate() { let r = 0x102 + 256 * i; s.party.0[r..r + n.len()].copy_from_slice(n.as_bytes()); s.party.0[r + 0x25] = 9; s.party.0[r + 0x23] = 12; s.party.0[r + 0x24] = 3; }
        assert_eq!(formation(&s), "Marching order: Magellan leads; Jurgen, Effane; Dennar, Verti");
        s.party.0[0x102 + 256 * 2 + 0x25] = 2; s.party.0[0x102 + 256 * 3 + 0x25] = 0;
        assert_eq!(formation(&s), "Marching order: Magellan leads; Jurgen, Effane (down); Dennar (dead), Verti");
        s.party.0[1..6].copy_from_slice(&[4, 0, 1, 2, 3]); assert!(formation(&s).starts_with("Marching order: Verti leads; Magellan, Jurgen;"), "the order bytes rule");
        ui.hover = None; ui.hint = Some(FORMATION); l.set_host("Party strip", Host::Dock(Rect::new(0, 0, 320, 54)));
        for at in [(258, 2), (285, 20), (318, 50)] {
            ui.components.pointer = Some(("Party strip", at)); assert!(visible(&ui));
            let (r, _) = geometry(&l, &s, &ui).unwrap(); let f = super::super::party_strip::FORMATION_RECT;
            assert!(r.x + r.w as i32 <= f.x, "{at:?}: the text stands left of the box, never over it: {r:?}");
            assert!(r.x >= 0 && r.y >= 0 && r.x + r.w as i32 <= 320 && r.y + r.h as i32 <= 54, "{at:?}: inside the strip: {r:?}");
            assert_eq!(r.h, 22, "{at:?}: two wrapped rows in the room beside the box");
            let mut out = Surface::new(r.w, r.h); Tooltip::new(Arc::default()).draw(&s, &ui, &mut out);
            let drawn: String = out.text.visible.iter().flat_map(|run| run.bytes.iter().map(|&b| b as char)).collect();
            assert_eq!(drawn.split_whitespace().collect::<Vec<_>>().join(""), formation(&s).split_whitespace().collect::<Vec<_>>().join(""), "the whole order is drawn");
        }
    }
    #[test]
    fn tooltip_follows_the_pointer_in_each_host_and_stays_inside() {
        let (mut l, s, mut ui) = setup();
        for host in [Host::Dock(Rect::new(0, 54, 400, 200)), Host::Window { id: 7, size: (400, 200) }] {
            l.set_host("Viewport", host);
            for at in [(0, 0), (200, 50), (399, 199)] {
                ui.components.pointer = Some(("Viewport", at));
                let (r, parent) = geometry(&l, &s, &ui).unwrap();
                let (x0, y0, w, h) = if parent.is_some() { (0, 0, 400, 200) } else { (0, 54, 400, 254) };
                assert!(r.x >= x0 && r.y >= y0 && r.x + r.w as i32 <= w && r.y + r.h as i32 <= h, "{at:?}: {r:?}");
                assert_eq!(r.w, "Vlaris: ready".len() as u32 * 6 + 4, "{at:?}: the box is the text's width");
                assert_eq!(parent, match host { Host::Window { id, .. } => Some(id), _ => None });
            }
        }
        ui.components.pointer = Some(("Viewport", (-1, 0))); assert!(geometry(&l, &s, &ui).is_none());
        ui.components.pointer = Some(("Side panel", (1, 1))); assert!(!visible(&ui));
        ui.components.pointer = Some(("Viewport", (1, 1))); ui.plus.open = true; assert!(visible(&ui), "Plus is its own window; the game window's tooltip stays");
        ui.plus.open = false; ui.menu = Some((Target::Cell((0, 0)), (0, 0))); assert!(!visible(&ui));
        ui.menu = None; ui.hover = None; assert!(!visible(&ui));
        ui.hover = Some(Target::Cell((0, 0))); ui.set_host_kind("Viewport", super::super::HostKind::Hidden);
        assert!(geometry(&l, &s, &ui).is_none());
        assert!(geometry(&Layout::default(), &s, &ui).is_none());
        ui.set_host_kind("Viewport", super::super::HostKind::Docked);
        assert!(geometry(&Layout::default(), &s, &ui).is_none());
        ui.components.pointer = None; assert!(!visible(&ui));
    }
    #[test]
    fn right_click_pin_grows_the_box_with_the_preview_lines_and_a_thug_says_in_view() {
        let (mut l, mut s, mut ui) = setup(); ui.components.pointer = Some(("Viewport", (100, 100)));
        s.squad.preview = vec!["Short: Routine 7+".into(), "needs 7+ (58%)".into()];
        let thug = Target::Unit(Unit::Npc(14)); ui.hover = Some(thug);
        s.squad.sight = Some(mt1::ground::orders::Sight { from: (4, 4), to: (2, 7), los: true, cover: false, band: mt1::ground::rules::Band::Short });
        assert_eq!(text(&s, &ui), "Vlaris: ready / in view");
        s.squad.sight.as_mut().unwrap().los = false; assert_eq!(text(&s, &ui), "Vlaris: ready / out of sight");
        {
            let mut h = ui.clone(); h.hover = None; h.hint = Some(FORMATION); let mut e = s.clone(); e.squad.hover.clear(); e.squad.sight = None;
            assert!(text(&e, &h).starts_with("Marching order: "), "{}", text(&e, &h)); assert!(visible(&h)); assert_eq!(text(&s, &h), "Vlaris: ready / out of sight", "the sim's text wins");
            h.hint = None; assert!(!visible(&h)); assert_eq!(text(&e, &h), "");
        }
        assert_eq!(geometry(&l, &s, &ui).unwrap().0.h, 12, "unpinned: one line");
        ui.info.set(Some((thug, 3)));
        let pinned_box = geometry(&l, &s, &ui).unwrap().0;
        assert_eq!(pinned_box.h, 32, "pinned: hover + 2 preview lines");
        assert_eq!(pinned_box.w, "Vlaris: ready / out of sight".len() as u32 * 6 + 4, "pinned: as wide as the longest line");
        let mut long = s.clone(); long.squad.hover = "Hostile (Szen Sanai, NPC 32): no attack left / in view and more".into();
        l.set_host("Viewport", Host::Dock(Rect::new(100, 54, 192, 112))); ui.components.pointer = Some(("Viewport", (150, 60)));
        let r = geometry(&l, &long, &ui).unwrap().0;
        assert!(r.x >= 100 && r.x + r.w as i32 <= 292 && r.y >= 54 && r.y + r.h as i32 <= 166, "{r:?} inside the 192x112 map host");
        {
            let lines = rows(&long, &ui, (192 - 4) / 6); assert!(lines.len() >= 2 && lines.iter().all(|l| l.chars().count() <= 31), "{lines:?}");
            assert_eq!(r.h, 2 + 10 * lines.len() as u32, "one row per wrapped line"); assert!(lines.concat().replace(' ', "").starts_with(&long.squad.hover.replace(' ', "")), "nothing lost: {lines:?}");
            let c = Tooltip::new(Arc::default()); let mut out = Surface::new(r.w, r.h); c.draw(&long, &ui, &mut out);
            assert!(out.text.visible.iter().all(|run| run.bytes != b"~"), "no cut marker"); assert!(out.text.visible.iter().any(|run| run.y == 12), "the second row is drawn");
        }
        let c = Tooltip::new(Arc::default()); let mut out = Surface::new(320, 32); c.draw(&s, &ui, &mut out);
        let row = |y| String::from_utf8(out.text.visible.iter().filter(|r| r.y == y).flat_map(|r| r.bytes.iter().copied()).collect()).unwrap();
        assert_eq!([row(2), row(12), row(22)], ["Vlaris: ready / out of sight", "Short: Routine 7+", "needs 7+ (58%)"]);
        assert!(out.text.visible.iter().all(|r| matches!(r.y, 2 | 12 | 22)));
        ui.hover = Some(Target::Cell((8, 8))); assert_eq!(pinned(&ui), 1, "off the target the pin is inert");
        let mut blank = Scene::program(ScreenKind::Ground); blank.squad.hover = " \n".into();
        assert!(geometry(&l, &blank, &ui).is_none(), "a blank hover (and no window hint) shows no box");
    }
}
