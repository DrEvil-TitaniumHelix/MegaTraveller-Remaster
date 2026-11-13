use super::{assets::Assets, components::PARTY_RECT, picture::portrait, Button, Component, Picture, Rect, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Preset, Scene, ScreenKind, Target, Unit};
use mt_engine::io::Event;
use std::sync::Arc;

pub const O_RECT: Rect = Rect::new(208, 2, 45, 22);
const PIP_Y: i32 = 49; const PIP_M: [u8; 5] = [0b101, 0b111, 0b111, 0b101, 0b101]; const PIP_A: [u8; 5] = [0b010, 0b101, 0b111, 0b101, 0b101];
pub const FORMATION_RECT: Rect = Rect::new(257, 1, 62, 50);
pub const MARCH: [Rect; 5] = [Rect::new(270, 4, 27, 15), Rect::new(258, 18, 27, 15), Rect::new(285, 18, 27, 15), Rect::new(258, 32, 27, 15), Rect::new(285, 32, 27, 15)];
const CAPTION: Rect = Rect::new(210, 28, 42, 21);
pub const NAMES_H: u32 = 10;
const NAMES_Y: i32 = PARTY_RECT.h as i32;
fn names_row(self_: &PartyStrip, scene: &Scene, ui: &UiState, out: &mut Surface) {
    if out.h <= PARTY_RECT.h as usize { return }
    for y in NAMES_Y..out.h as i32 { for x in 0..out.w as i32 { out.put(x, y, 0); } }
    let hot = named(scene, ui);
    for i in 0..5u8 {
        if scene.party.mask() & (1 << i) == 0 { continue }
        let c = if !scene.party.alive(i) { 8 } else if hot == Some(i) { 11 } else { 15 };
        let n = scene.party.name(i); let w = (n.len() as u32 * 6).min(38);
        self_.assets.text_fit(out, &n, Rect::new(6 + i as i32 * 40 + (38 - w as i32) / 2 - 2, NAMES_Y + 1, w.max(1), 8), c);
    }
    for (t, r) in [("Orders", O_RECT), ("Marching order", FORMATION_RECT)] {
        let w = (t.len() as u32 * 6).min(r.w); self_.assets.text_fit(out, t, Rect::new(r.x + (r.w - w) as i32 / 2, NAMES_Y + 1, w, 8), 7);
    }
}
pub const PLACES: [&str; 5] = ["Front (leader)", "Middle left", "Middle right", "Rear left", "Rear right"];
pub const PLACE_TIPS: [&str; 5] = ["Marching order 1", "Marching order 2", "Marching order 3", "Marching order 4", "Marching order 5"];
fn who(scene: &Scene, k: usize) -> Option<String> { let i = scene.party.order()[k]; (i < 5 && scene.party.mask() & (1 << i) != 0 && scene.party.alive(i)).then(|| scene.party.name(i)) }
pub fn place_tip(scene: &Scene, hint: Option<&str>) -> Option<String> {
    let k = PLACE_TIPS.iter().position(|&t| Some(t) == hint)?; Some(format!("{}: {}", PLACES[k], who(scene, k).unwrap_or_else(|| "nobody".into())))
}
#[derive(Clone, Default, Debug)]
pub struct Note { before: Option<[u8; 5]>, clicks: Vec<usize>, missed: bool, done: bool, said: Option<(String, std::time::Instant)>, swap: Option<([u8; 5], [u8; 5], Option<u32>)> }
const SAID_FOR: std::time::Duration = std::time::Duration::from_secs(5);
impl Note {
    pub fn said_recently(&self) -> bool { self.said.as_ref().is_some_and(|(_, at)| at.elapsed() < SAID_FOR) }
    pub fn pending(&self) -> bool { self.before.is_some() }
}
fn observe(scene: &Scene, ui: &UiState) {
    let mut n = ui.components.picker.lock().unwrap(); let order = scene.party.order();
    if picking(ui) {
        if n.before.is_none() { *n = Note { before: Some(order), ..Note::default() } }
    } else if let Some(before) = n.before {
        let ended = before != order || n.done || n.clicks.len() != 1 || n.clicks.first().is_some_and(|&k| who(scene, k).is_none())
            || ui.decision.as_ref().is_some_and(|d| d.context.routine != "DE5E");
        if !ended { return }
        let name = |i: u8| if i < 5 { scene.party.name(i) } else { "nobody".into() };
        let moved: Vec<usize> = (0..5).filter(|&k| before[k] != order[k]).collect();
        let clicked = match n.clicks[..] { [a, b, ..] if a != b && who(scene, a).is_some() && who(scene, b).is_some() => Some((a, b)), _ => None };
        let mut swap = None;
        let t = match (&moved[..], &n.clicks[..]) {
            ([a, b], _) => format!("{} and {} swapped places.", name(before[*a]), name(before[*b])),
            (_, _) if moved.is_empty() && clicked.is_some() => { let (a, b) = clicked.unwrap(); let mut after = before; after.swap(a, b); swap = Some((before, after, scene.context.clock));
                format!("{} and {} swapped places.", name(before[a]), name(before[b])) }
            (_, [k, ..]) if who(scene, *k).is_none() => format!("Nobody is at {}: nothing changed.", PLACES[*k]),
            (_, [a, b, ..]) if a == b => "The same place was clicked twice: nothing changed.".into(),
            _ => "Marching order unchanged.".into(),
        };
        *n = Note { said: Some((t, std::time::Instant::now())), swap, ..Note::default() };
    }
}
pub fn picker_text(scene: &Scene, ui: &UiState) -> Option<String> {
    if scene.screen != ScreenKind::Ground { return None }
    observe(scene, ui); let n = ui.components.picker.lock().unwrap();
    if !picking(ui) { return n.said.as_ref().filter(|_| n.said_recently()).map(|(t, _)| t.clone()) }
    let miss = if n.missed { "Not one of the five places. " } else { "" };
    Some(match n.clicks.first() {
        None => format!("{miss}Marching order: click the Traveller to move. Done = stop."),
        Some(&k) => { let who = who(scene, k).unwrap_or_else(|| "this Traveller".into());
            format!("{miss}{who} picked (framed). Now click the place to put {who}: the two swap. Done = stop.") }
    })
}
pub fn expire(ui: &UiState) -> bool { let mut n = ui.components.picker.lock().unwrap(); let up = n.said.is_some() && !n.said_recently(); if up { n.said = None } up }
fn shown_order(scene: &Scene, ui: &UiState) -> Option<[u8; 5]> {
    let mut n = ui.components.picker.lock().unwrap();
    match n.swap { Some((before, after, clock)) if scene.party.order() == before && scene.context.clock == clock => Some(after), Some(_) => { n.swap = None; None } None => None }
}
pub fn picking(ui: &UiState) -> bool { ui.decision.as_ref().is_some_and(|d| d.context.routine == "DE5E" && d.rects.first() == Some(&(0, [270, 4, 27, 15]))) }
pub fn named(scene: &Scene, ui: &UiState) -> Option<u8> {
    match ui.hover { Some(Target::Portrait(i) | Target::Unit(Unit::Traveller(i))) => return Some(i), _ => {} }
    let (id, at) = ui.components.pointer?; if id != "Party strip" { return None }
    MARCH.iter().position(|r| r.contains(at)).map(|k| scene.party.order()[k]).filter(|&i| i < 5)
}
fn frame(out: &mut Surface, r: Rect, c: u8) { let (w, h) = (r.w as i32, r.h as i32); for dx in 0..w { out.put(r.x + dx, r.y, c); out.put(r.x + dx, r.y + h - 1, c); } for dy in 0..h { out.put(r.x, r.y + dy, c); out.put(r.x + w - 1, r.y + dy, c); } }
const F1: u16 = 0x13B;
pub struct PartyStrip { assets: Arc<Assets> }
impl PartyStrip { pub fn new(assets: Arc<Assets>) -> Self { Self { assets } } }
pub fn down(scene: &Scene, i: u8) -> bool { let p = &scene.party; p.alive(i) && p.hits(i) <= p.split(i) }
impl Component for PartyStrip {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        observe(scene, ui);
        let swapped; let scene = match shown_order(scene, ui) { Some(o) => { let mut s = scene.clone(); s.party.0[1..6].copy_from_slice(&o); swapped = s; &swapped } None => scene };
        Picture::new(PARTY_RECT).draw(scene, ui, out);
        if ui.preset != Preset::Classic && scene.screen == ScreenKind::Ground { names_row(self, scene, ui, out) }
        if !scene.squad.active || scene.screen != ScreenKind::Ground { return }
        out.cover(&self.assets.party_box(scene), 256, 0);
        let sq = &scene.squad;
        let flashing = |i: u8| ui.replay.is_some_and(|(seq, t)| seq == sq.fx.seq && t < 0.8) && sq.fx.hits.iter().any(|&(u, ..)| u == Unit::Traveller(i));
        for i in 0..5u8 {
            if scene.party.mask() & (1 << i) == 0 || !scene.party.alive(i) { continue }
            let x = 6 + i as i32 * 40;
            if sq.encounter {
                let [m, a] = sq.pips[i as usize];
                for (k, lit, colour, glyph) in [(0, m, 10u8, PIP_M), (1, a, 14u8, PIP_A)] {
                    let (bx, by) = (x + 2 + k * 12, PIP_Y);
                    for dy in 0..5 { for dx in 0..10 { out.put(bx + dx, by + dy, if lit { colour } else { 8 }); } }
                    for (dy, row) in glyph.iter().enumerate() { for dx in 0..3 { if row & (4 >> dx) != 0 { out.put(bx + 3 + dx as i32, by + dy as i32, if lit { 0 } else { 7 }); } } }
                }
            }
            if sq.evade[i as usize] { self.assets.text(out, "E", (x + 22, 43), 11); }
            if sq.hold[i as usize] { self.assets.text(out, "H", (x + 28, 43), 11); }
            match sq.stance[i as usize] { "crosshair" => self.assets.text(out, "T", (x + 34, 43), 14), "boot" => self.assets.text(out, "R", (x + 34, 43), 14), _ => {} }
            if down(scene, i) { for dy in 0..9 { for dx in 0..28 { out.put(x + 2 + dx, 20 + dy, 8); } } self.assets.text(out, "DOWN", (x + 4, 21), 0); }
            if flashing(i) { for k in 0..2 { for dx in 0..34 { out.put(x + dx, 1 + k, 12); out.put(x + dx, 48 - k, 12); } for dy in 1..49 { out.put(x + k, dy, 12); out.put(x + 33 - k, dy, 12); } } }
        }
        let fill = |out: &mut Surface, r: Rect, c: u8| for dy in 0..r.h as i32 { for dx in 0..r.w as i32 { out.put(r.x + dx, r.y + dy, c); } };
        let line = |out: &mut Surface, t: &str, y: i32, c: u8| self.assets.text_fit(out, t, Rect::new(O_RECT.x + 2, y, O_RECT.w - 4, 8), c);
        if sq.encounter && sq.to_act.is_some() {
            fill(out, O_RECT, 1); self.assets.text(out, "END", (O_RECT.x + 13, O_RECT.y + 3), 15); self.assets.text(out, "TURN", (O_RECT.x + 10, O_RECT.y + 12), 15);
        } else if ui.preset != Preset::Classic {
            fill(out, O_RECT, 0);
            let (a, b, c) = if picking(ui) { ("Done", "(Esc)", 14) } else { ("Change", "order", if scene.combat || sq.encounter { 8 } else { 15 }) };
            line(out, a, O_RECT.y + 2, c); line(out, b, O_RECT.y + 12, c);
        }
        if ui.preset == Preset::Classic { return }
        fill(out, CAPTION, 0);
        for (t, y) in [("March", CAPTION.y + 1), ("order", CAPTION.y + 11)] { self.assets.text_fit(out, t, Rect::new(CAPTION.x + 1, y, CAPTION.w - 8, 8), 15); }
        self.assets.text(out, ">", (CAPTION.x + CAPTION.w as i32 - 6, CAPTION.y + 6), 15);
        let order = scene.party.order();
        for (k, r) in MARCH.iter().enumerate() {
            if order[k] >= 5 || scene.party.mask() & (1 << order[k]) == 0 { continue }
            if k == 0 { self.assets.text(out, "1", (r.x - 10, r.y + 4), 15) } else { fill(out, Rect::new(r.x + 1, r.y + 1, 7, 9), 0); self.assets.text(out, &(k + 1).to_string(), (r.x + 2, r.y + 2), 15) }
        }
        if picking(ui) { if let Some(&k) = ui.components.picker.lock().unwrap().clicks.first() { frame(out, MARCH[k], 15); frame(out, Rect::new(MARCH[k].x + 1, MARCH[k].y + 1, MARCH[k].w - 2, MARCH[k].h - 2), 15) } }
        if let Some(i) = named(scene, ui) {
            frame(out, Rect::new(6 + i as i32 * 40, 1, 34, 48), 11);
            if let Some(k) = order.iter().position(|&w| w == i) { frame(out, MARCH[k], 11) }
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if picking(ui) && O_RECT.contains(at) && !scene.squad.encounter {
            return match e { UiEvent::Up(Button::Left) => { ui.components.picker.lock().unwrap().done = true; vec![Intent::Raw(Event::Key(0x101)), Intent::Raw(Event::KeyUp(0x101))] } UiEvent::Move => vec![Intent::Hover(None), Intent::Hint(None)], _ => Vec::new() }
        }
        if picking(ui) && matches!(e, UiEvent::Up(Button::Left)) {
            let mut n = ui.components.picker.lock().unwrap();
            match MARCH.iter().position(|r| r.contains(at)) { Some(k) => { n.clicks.push(k); n.missed = false } None => n.missed = true }
        }
        if at.1 >= NAMES_Y {
            let slot = portrait((at.0, 20)).filter(|&s| scene.party.mask() & (1 << s) != 0);
            return match (*e, slot) {
                (UiEvent::Move, s) => vec![Intent::Hover(s.map(Target::Portrait))],
                (UiEvent::Up(Button::Right), Some(s)) => vec![Intent::Raw(Event::Key(F1 + s as u16)), Intent::Raw(Event::KeyUp(F1 + s as u16))],
                (UiEvent::Up(Button::Left), Some(s)) if scene.squad.active => vec![Intent::Select(s)],
                _ => Vec::new(),
            }
        }
        if !scene.squad.active || scene.screen != ScreenKind::Ground { return Picture::new(PARTY_RECT).input(scene, ui, e, at) }
        let slot = portrait(at).filter(|&s| scene.party.mask() & (1 << s) != 0);
        match (*e, slot) {
            (UiEvent::Up(Button::Left), _) if O_RECT.contains(at) && scene.squad.encounter => vec![Intent::EndTurn],
            (UiEvent::Up(Button::Left), Some(s)) => vec![Intent::Select(s)],
            (UiEvent::DoubleClick(Button::Left), Some(s)) => vec![Intent::Follow(Some(Unit::Traveller(s)))],
            (UiEvent::Up(Button::Right), Some(s)) => vec![Intent::Raw(Event::Key(F1 + s as u16)), Intent::Raw(Event::KeyUp(F1 + s as u16))],
            (UiEvent::Move, None) if MARCH.iter().any(|r| r.contains(at)) => vec![Intent::Hover(None), Intent::Hint(MARCH.iter().position(|r| r.contains(at)).map(|k| PLACE_TIPS[k]))],
            (UiEvent::Move, None) if FORMATION_RECT.contains(at) => vec![Intent::Hover(None), Intent::Hint(Some(super::tooltip::FORMATION))],
            (UiEvent::Move, s) => vec![Intent::Hover(s.map(Target::Portrait))],
            (UiEvent::Key(k), _) => vec![Intent::Raw(Event::Key(k))],
            (UiEvent::Up(Button::Left | Button::Right), None) => Picture::new(PARTY_RECT).input(scene, ui, e, at),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene {
        let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.squad.encounter = true; s.squad.to_act = Some(0);
        s.party.0 = vec![0; 0x1000]; s.party.0[0] = 0b111;
        for i in 0..3usize { let r = 0x102 + 256 * i; s.party.0[r + 0x25] = 9; s.party.0[r + 0x23] = 12; s.party.0[r + 0x24] = 3; }
        s.party.0[0x102 + 256 * 2 + 0x25] = 2;
        s.squad.pips = [[true, false], [true, true], [false, false], [false, false], [false, false]]; s.squad.evade[1] = true;
        s
    }
    #[test]
    fn pips_down_end_turn_and_the_squad_clicks() {
        let c = PartyStrip::new(Arc::default()); let s = scene(); let ui = UiState::default();
        let mut out = Surface::new(320, 54); c.draw(&s, &ui, &mut out);
        assert_eq!((out.get(9, 49), out.get(21, 49), out.get(49, 49), out.get(61, 49)), (Some(10), Some(8), Some(10), Some(14)), "Trav1 Move lit / Attack dark; Trav2 both lit (10 x 5 pips at row 49)");
        assert_eq!((out.get(12, 50), out.get(24, 51), out.get(12, 49)), (Some(0), Some(7), Some(10)), "the M's middle on a lit pip in ink, the A's bar on a dark pip in grey, the M's top gap shows the pip");
        assert!(down(&s, 2) && !down(&s, 0)); assert_eq!(out.get(90, 22), Some(8), "DOWN band over Trav3");
        assert_eq!(out.get(O_RECT.x + 1, O_RECT.y + 1), Some(1), "END TURN over the O rectangle");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 20)), [Intent::Select(0)]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (60, 20)), [Intent::Select(1)]);
        assert_eq!(c.input(&s, &ui, &UiEvent::DoubleClick(Button::Left), (60, 20)), [Intent::Follow(Some(Unit::Traveller(1)))]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Right), (20, 20)), [Intent::Raw(Event::Key(F1)), Intent::Raw(Event::KeyUp(F1))]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (220, 10)), [Intent::EndTurn]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (20, 20)), [Intent::Hover(Some(Target::Portrait(0)))]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (260, 3)), [Intent::Hover(None), Intent::Hint(Some(super::super::tooltip::FORMATION))], "the formation box names itself");
        assert!(matches!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (170, 20))[..], [Intent::Raw(_)]), "an empty slot: the 1990 hotspot");
        let mut s = s; s.squad.fx.seq = 5; s.squad.fx.hits.push((Unit::Traveller(0), (1, 1), 3));
        let ui = UiState { replay: Some((5, 0.2)), ..UiState::default() }; let mut out = Surface::new(320, 54); c.draw(&s, &ui, &mut out);
        assert_eq!((out.get(6, 1), out.get(46, 1)), (Some(12), Some(0)));
        s.squad.active = false; assert!(matches!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 20))[..], [Intent::Raw(_)]), "Classic 1990 combat: the 1990 pointer");
    }
    #[test]
    fn the_party_blocks_explain_themselves() {
        let c = PartyStrip::new(Arc::default()); let mut s = scene(); s.squad.encounter = false; s.squad.to_act = None;
        s.party.0[1..6].copy_from_slice(&[1, 0, 2, 5, 5]);
        let words = |out: &Surface| out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>();
        let draw = |ui: &UiState| { let mut out = Surface::new(320, 54); c.draw(&s, ui, &mut out); out };
        let ui = UiState::default(); let out = draw(&ui);
        assert!(words(&out).contains("Changeorder") && words(&out).contains("Marchorder>"), "{}", words(&out));
        assert!(words(&out).contains("123") && !words(&out).contains('4'), "positions 1-3 numbered, empty 4-5 not: {}", words(&out));
        assert_eq!(out.get(MARCH[1].x + 1, MARCH[1].y + 1), Some(0), "a number's black corner on position 2");
        assert_ne!(out.get(6, 1), Some(11), "nobody named: no frame");
        let ui = UiState { decision: Some({ let mut d = mt_engine::play::Decision::new(mt_engine::play::Kind::Menu, "t", "DE5E"); d.rects = MARCH.iter().enumerate().map(|(i, r)| (i, [r.x, r.y, r.w as i32, r.h as i32])).collect(); d }), ..UiState::default() };
        assert!(picking(&ui) && { let w = words(&draw(&ui)).replace(' ', ""); w.contains("Done") && w.contains("(Esc)") }, "the Orders button is the picker's Done");
        let mut ui = UiState::default(); ui.components.pointer = Some(("Party strip", (MARCH[0].x + 5, MARCH[0].y + 5)));
        assert_eq!(named(&s, &ui), Some(1), "the pointer on position 1 names its Traveller");
        let out = draw(&ui); assert_eq!((out.get(46, 1), out.get(MARCH[0].x, MARCH[0].y)), (Some(11), Some(11)), "card 2 and position 1 framed together");
        let ui = UiState { hover: Some(Target::Portrait(0)), ..UiState::default() }; let out = draw(&ui);
        assert_eq!((out.get(6, 1), out.get(MARCH[1].x, MARCH[1].y)), (Some(11), Some(11)), "card 1 hovered: its position 2 framed too");
        let ui = UiState { preset: Preset::Classic, ..UiState::default() }; assert!(!words(&draw(&ui)).contains("March"), "Classic keeps the 1990 art");
    }
    #[test]
    fn the_marching_order_picker_says_what_to_click() {
        let c = PartyStrip::new(Arc::default()); let mut s = scene(); s.squad.encounter = false; s.squad.to_act = None;
        for (i, n) in ["Magellan", "Jurgen", "Effane"].iter().enumerate() { let r = 0x102 + 256 * i; s.party.0[r..r + n.len()].copy_from_slice(n.as_bytes()); }
        s.party.0[1..6].copy_from_slice(&[0, 1, 2, 5, 5]);
        let mut ui = UiState::default(); let mid = (MARCH[1].x + 5, MARCH[1].y + 5);
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, mid), [Intent::Hover(None), Intent::Hint(Some(PLACE_TIPS[1]))]);
        ui.hint = Some(PLACE_TIPS[1]); assert_eq!(super::super::tooltip::text(&s, &ui), "Middle left: Jurgen");
        ui.hint = Some(PLACE_TIPS[3]); assert_eq!(super::super::tooltip::text(&s, &ui), "Rear left: nobody");
        ui.hint = None; assert_eq!(picker_text(&s, &ui), None, "no picker, nothing said");
        let wait = |id: &str| { let mut d = mt_engine::play::Decision::new(mt_engine::play::Kind::Menu, "t", "DE5E"); d.id = id.into(); d.rects = MARCH.iter().enumerate().map(|(i, r)| (i, [r.x, r.y, r.w as i32, r.h as i32])).collect(); Some(d) };
        ui.decision = wait("d:1");
        assert!(picker_text(&s, &ui).unwrap().starts_with("Marching order: click the Traveller to move"));
        assert!(super::super::tooltip::visible(&ui), "shown wherever the pointer is");
        c.input(&s, &ui, &UiEvent::Up(Button::Left), (300, 49)); assert!(picker_text(&s, &ui).unwrap().starts_with("Not one of the five places."), "a missed click says why");
        c.input(&s, &ui, &UiEvent::Up(Button::Left), mid); ui.decision = None;
        assert_eq!(picker_text(&s, &ui), None, "the real window's gap between the two waits ends nothing");
        ui.decision = wait("d:1");
        let mut out = Surface::new(320, 54); c.draw(&s, &ui, &mut out); assert_eq!(out.get(MARCH[1].x, MARCH[1].y + 5), Some(15), "the first pick framed white");
        assert!(picker_text(&s, &ui).unwrap().starts_with("Jurgen picked (framed). Now click the place to put Jurgen"), "{:?}", picker_text(&s, &ui));
        c.input(&s, &ui, &UiEvent::Up(Button::Left), (MARCH[0].x + 5, MARCH[0].y + 5)); ui.decision = None;
        s.party.0[1..6].copy_from_slice(&[1, 0, 2, 5, 5]);
        assert_eq!(picker_text(&s, &ui).as_deref(), Some("Magellan and Jurgen swapped places."));
        assert!(super::super::tooltip::visible(&ui), "the result stays a moment");
        ui.decision = wait("d:3"); picker_text(&s, &ui); c.input(&s, &ui, &UiEvent::Up(Button::Left), (MARCH[4].x + 5, MARCH[4].y + 5)); ui.decision = None;
        assert_eq!(picker_text(&s, &ui).as_deref(), Some("Nobody is at Rear right: nothing changed."));
        ui.decision = wait("d:4"); picker_text(&s, &ui); c.input(&s, &ui, &UiEvent::Up(Button::Left), mid); ui.decision = wait("d:5"); picker_text(&s, &ui);
        c.input(&s, &ui, &UiEvent::Up(Button::Left), mid); ui.decision = None;
        assert_eq!(picker_text(&s, &ui).as_deref(), Some("The same place was clicked twice: nothing changed."));
        ui.decision = wait("d:6"); picker_text(&s, &ui); let mut out = Surface::new(320, 54); c.draw(&s, &ui, &mut out);
        assert!(out.text.runs.iter().any(|r| r.bytes == b"D"), "the Orders button reads Done while picking");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (O_RECT.x + 5, O_RECT.y + 5)), [Intent::Raw(Event::Key(0x101)), Intent::Raw(Event::KeyUp(0x101))], "Done = Esc");
        ui.decision = None; assert_eq!(picker_text(&s, &ui).as_deref(), Some("Marching order unchanged."));
        let before = s.party.order(); ui.decision = wait("d:7"); picker_text(&s, &ui);
        c.input(&s, &ui, &UiEvent::Up(Button::Left), (MARCH[0].x + 5, MARCH[0].y + 5)); c.input(&s, &ui, &UiEvent::Up(Button::Left), mid); ui.decision = None;
        assert_eq!(picker_text(&s, &ui).as_deref(), Some("Jurgen and Magellan swapped places."));
        let mut after = before; after.swap(0, 1); assert_eq!(shown_order(&s, &ui), Some(after), "the strip shows the swap at once");
        s.context.clock = Some(99); assert_eq!(shown_order(&s, &ui), None, "a later Scene rules");
    }
    #[test]
    fn the_names_row_says_who_is_who() {
        let c = PartyStrip::new(Arc::default()); let mut s = scene();
        for (i, n) in ["Magellan", "Jurgen", "Effane"].iter().enumerate() { let r = 0x102 + 256 * i; s.party.0[r..r + n.len()].copy_from_slice(n.as_bytes()); }
        let row = |out: &Surface, x0: i64, x1: i64| out.text.runs.iter().filter(|r| r.y > 54 && r.x >= x0 && r.x < x1).flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>();
        let ui = UiState::default(); let mut out = Surface::new(320, 54 + NAMES_H); c.draw(&s, &ui, &mut out);
        assert_eq!([row(&out, 0, 45), row(&out, 45, 85), row(&out, 85, 125)], ["Magellan", "Jurgen", "Effane"], "each name whole under its card");
        assert_eq!((row(&out, 200, 256), row(&out, 256, 320)), ("Orders".into(), "Marching order".into()));
        assert!(out.text.runs.iter().filter(|r| r.y > 54).all(|r| r.x + r.width as i64 <= 320), "nothing past the strip");
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (60, 58)), [Intent::Hover(Some(Target::Portrait(1)))], "a name hovers like its card");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Right), (20, 58)), [Intent::Raw(Event::Key(F1)), Intent::Raw(Event::KeyUp(F1))]);
        s.squad.active = false; assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (220, 58)).is_empty(), "never a 1990 click into the map rows");
        let ui = UiState { preset: Preset::Classic, ..UiState::default() }; let mut out = Surface::new(320, 54 + NAMES_H); c.draw(&s, &ui, &mut out);
        assert_eq!(row(&out, 0, 320), "", "Classic: no row");
    }
}
