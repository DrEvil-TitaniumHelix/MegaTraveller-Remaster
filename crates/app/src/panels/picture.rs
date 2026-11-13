use super::{always, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState, Button, FRAME};
use mt1::ui::{Intent, Scene, ScreenKind, Unit};
use mt_engine::{io::{mouse, Event, MouseEvent}, play::Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picture { pub rect: Rect }
pub fn flight_control(ui: &UiState, (col, row): (i16, i16)) -> Option<(&'static str, u8)> {
    let (c, r) = (col as i32, row as i32);
    let inside = |x: i32, y: i32, w: i32, h: i32| (x..x + w).contains(&c) && (y..y + h).contains(&r);
    let word_at = |y: i64| ui.frame.text.visible.iter().filter(|t| t.y == y && t.x >= 255).map(|t| t.bytes.clone()).next().unwrap_or_default();
    if inside(253, 67, 61, 10) { return Some(("Crew stations - S", b'S')) }
    if inside(253, 79, 61, 10) { return Some(("Communications - C", b'C')) }
    if inside(253, 91, 61, 10) { return Some(if word_at(93).first() == Some(&b'L') { ("Land - L", b'L') } else { ("Jump - J", b'J') }) }
    if inside(253, 103, 61, 10) { return Some(if word_at(105).first() == Some(&b'T') { ("Take off - T", b'T') } else { ("Quit flight - Q", b'Q') }) }
    if inside(253, 115, 61, 10) { return Some(("Pause - P", b'P')) }
    if inside(293, 157, 21, 20) { return Some(("Stop - keypad 5", b'5')) }
    if inside(293, 139, 21, 17) { return Some(("Thrust up - Up arrow", 0)) }
    if inside(293, 178, 21, 17) { return Some(("Thrust down - Down arrow", 0)) }
    if inside(252, 138, 22, 15) { return Some(("Rotate left - Left arrow", 0)) }
    if inside(268, 155, 23, 14) { return Some(("Rotate right - Right arrow", 0)) }
    None
}
pub fn station_control(ui: &UiState, (col, row): (i16, i16)) -> Option<(&'static str, Option<Vec<mt_engine::io::Key>>)> {
    let (c, r) = (col as i32, row as i32);
    let inside = |x: i32, y: i32, w: i32, h: i32| (x..x + w).contains(&c) && (y..y + h).contains(&r);
    let routine = ui.decision.as_ref().map(|d| d.context.routine.clone()).unwrap_or_default();
    let title = match routine.as_str() { "pilot" => "PILOT".to_string(), "9FDB" | "955C" => "GUN".into(), "121C" => "COMPUTER".into(), "engineer" => "ENGINEER".into(), "medical" => "MED".into(),
        _ => {
            let all: String = ui.frame.text.visible.iter().map(|t| String::from_utf8_lossy(&t.bytes).into_owned()).collect();
            ["PILOT:", "GUN1:", "GUN2:", "COMPUTER:", "ENGINEER:", "COMMUNICATIONS:", "COMMS:", "MEDICAL:"].iter().find(|k| all.contains(*k)).map(|k| k.trim_end_matches(':').to_string()).unwrap_or_default() } };
    if inside(209, 3, 43, 20) { return Some(("Back to flight - Esc", None)) }
    if inside(23, 61, 49, 10) { return Some(("Assign crew - A", None)) }
    if inside(23, 72, 49, 10) { return Some(("Choose station - V", None)) }
    if inside(5, 88, 14, 12) { return Some(("Tactical map - X", None)) }
    const STATIONS: [&str; 8] = ["Pilot station - V, F1", "Gunner 1 - V, F2", "Gunner 2 - V, F3", "Computer - V, F4", "Engineering - V, F5", "Communications - V, F6", "Medical - V, F7", "Ship's bay - V, F8"];
    if (56..86).contains(&r) && c >= 73 && c < 73 + 8 * 28 {
        let i = ((c - 73) / 28) as usize;
        return Some((STATIONS[i], Some(vec![0x100 + 0x60 + i as mt_engine::io::Key])))
    }
    let t = title.as_str();
    if t.starts_with("PILOT") {
        if inside(148, 116, 18, 13) { return Some(("Thrust up - Up arrow", None)) } if inside(148, 131, 18, 13) { return Some(("Thrust down - Down arrow", None)) }
        if inside(128, 152, 20, 15) { return Some(("Rotate left - Left arrow", None)) } if inside(150, 168, 18, 16) { return Some(("Rotate right - Right arrow", None)) }
    } else if t.starts_with("COMPUTER") {
        if inside(124, 179, 33, 14) { return Some(("Load program - L", None)) } if inside(166, 179, 33, 14) { return Some(("Run program - R", None)) } if inside(208, 179, 33, 14) { return Some(("Stop program - S", None)) }
    } else if t.starts_with("ENGINEER") {
        if inside(127, 169, 40, 17) { return Some(("Repair component - R", None)) }
    } else if t.starts_with("COMM") {
        if inside(125, 101, 40, 15) { return Some(("Communications - Enter", None)) } if inside(168, 101, 18, 15) { return Some(("Channel up - Up arrow", None)) } if inside(186, 101, 18, 15) { return Some(("Channel down - Down arrow", None)) }
    } else if t.starts_with("MED") {
        if inside(169, 173, 48, 16) { return Some(("Medical - H", None)) } if inside(219, 172, 48, 16) { return Some(("Medical - R", None)) }
    } else if t.starts_with("GUN") {
        if inside(204, 155, 14, 6) { return Some(("Turn gun left - Left arrow", None)) } if inside(221, 155, 14, 6) { return Some(("Turn gun right - Right arrow", None)) }
        if ui.decision.as_ref().is_some_and(|d| d.rects.iter().any(|(_, r)| *r == [209, 168, 22, 23] || *r == [144, 167, 16, 26])) {
            if inside(209, 168, 22, 23) { return Some(("Rel = Reload missiles - R", None)) } if inside(233, 168, 22, 23) { return Some(("Tar = Target: aim, then click a tube - T", None)) } if inside(257, 168, 22, 23) { return Some(("Fire (after Target) - F", None)) }
            const TUBES: [&str; 4] = ["Missile tube 1 - click Tar first, then this - 1", "Missile tube 2 - click Tar first, then this - 2", "Missile tube 3 - click Tar first, then this - 3", "Missile tube 4 - click Tar first, then this - 4"];
            if inside(144, 167, 64, 26) { let i = ((c - 144) / 16) as usize; return Some((TUBES[i], Some(vec![b'1' as mt_engine::io::Key + i as mt_engine::io::Key]))) }
        } else {
            if inside(235, 168, 34, 22) { return Some(("Target - T", None)) } if inside(271, 168, 33, 22) { return Some(("Fire (after Target) - F", None)) }
        }
    }
    None
}
pub(super) fn flag_hotkeys(log: &mut mt_engine::present::textlog::TextLog, ui: &UiState) {
    if !ui.decision.as_ref().is_some_and(|d| d.prompt.as_deref().is_some_and(mt1::save::position_list)) { crate::mouse::decision::flag_hotkeys(log, ui.decision.as_ref()); }
}
pub(super) fn flag_flight_keys(log: &mut mt_engine::present::textlog::TextLog, ui: &UiState) {
    let mut first: Vec<(u8, i64, i64)> = Vec::new();
    for r in log.visible.iter().filter(|r| r.x >= 253 && (67..125).contains(&r.y)) {
        let Some((_, key)) = flight_control(ui, (r.x as i16, r.y as i16)) else { continue };
        if key == 0 || !r.bytes.first().is_some_and(|b| b.eq_ignore_ascii_case(&key)) { continue }
        match first.iter_mut().find(|(k, _, y)| *k == key && *y == r.y) { Some(f) => if r.x < f.1 { f.1 = r.x }, None => first.push((key, r.x, r.y)) }
    }
    for (_, x, y) in first { for r in log.visible.iter_mut().chain(log.runs.iter_mut()) { if r.x == x && r.y == y { r.colour |= crate::pipeline::text::UNDERLINE; } } }
}
pub fn desk_switch(ui: &UiState, (col, row): (i16, i16)) -> Vec<Intent> {
    use mt1::ground::starport::{cells, LEAVE_ROW as L};
    let Some(d) = ui.decision.as_ref().filter(|d| d.context.program == "t" && d.context.routine != "13FF2") else { return Vec::new() };
    if ui.buy.as_ref().is_some_and(|(id, _)| *id == d.id) { return Vec::new() }
    let runs = &ui.frame.text.visible;
    if !crate::mouse::decision::line_texts(runs).iter().any(|(y, l)| *y == 8 && l.ends_with("starport")) { return Vec::new() }
    let on = |r: &[i32; 4], x: i32, y: i32| (r[0]..r[0] + r[2]).contains(&x) && (r[1]..r[1] + r[3]).contains(&y);
    let tiles = cells().map(|(_, r)| (r, mt_engine::io::keys::ENTER)).chain([([L.1 as i32, L.0 as i32, L.3 as i32, L.2 as i32], mt_engine::io::keys::ESC)]);
    let Some((_, key)) = tiles.into_iter().find(|(r, _)| on(r, col as i32, row as i32) && runs.iter().any(|t| on(r, t.x as i32, t.y as i32))) else { return Vec::new() };
    [Event::Mouse(MouseEvent { event: mouse::RIGHT_RELEASE, buttons: 0, col, row }), Event::Key(key), Event::KeyUp(key)].map(Intent::Raw).to_vec()
}
pub fn portrait(at: (i32, i32)) -> Option<u8> {
    ((6..206).contains(&at.0) && (0..50).contains(&at.1)).then(|| ((at.0 - 6) / 40) as u8)
}
fn drop_tip(ui: &UiState) -> Option<Intent> { ui.hint.map(|_| Intent::Hint(None)) }
impl Picture {
    pub const fn new(rect: Rect) -> Picture { Picture { rect } }
    pub const fn full() -> Picture { Picture::new(FRAME) }
    pub const fn placement(title: &'static str, rect: Rect) -> Placement {
        Placement { dock: Dock::Fill, size: Size { preferred: (rect.w, rect.h), min: (rect.w, rect.h) }, order: 0, screens: &[], when: always,
            origin: Origin::Classic1990(rect), optional: false, undockable: true, title }
    }
    fn game(&self, at: (i32, i32)) -> Option<(i16, i16)> {
        let (col, row) = (self.rect.x.checked_add(at.0)?, self.rect.y.checked_add(at.1)?);
        ((0..320).contains(&col) && (0..200).contains(&row) && at.0 >= 0 && at.1 >= 0 && (at.0 as u32) < self.rect.w && (at.1 as u32) < self.rect.h).then_some((col as i16, row as i16))
    }
}
impl Component for Picture {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        out.blit(&ui.frame.indexed, 320, 200, self.rect, 0, 0);
        out.regs = ui.frame.regs;
        out.text.page = ui.frame.text.page; out.text.regs = ui.frame.text.regs; out.text.overflowed = ui.frame.text.overflowed;
        let (x0, y0, x1, y1) = (self.rect.x as i64, self.rect.y as i64, self.rect.x as i64 + self.rect.w as i64, self.rect.y as i64 + self.rect.h as i64);
        let inside = |r: &mt_engine::present::textlog::Run| r.x < x1 && r.y < y1 && r.x.saturating_add(r.erase_width().max(1) as i64) > x0 && r.y.saturating_add(r.erase_height().max(1) as i64) > y0;
        let shift = |r: &mt_engine::present::textlog::Run| { let mut r = r.clone(); r.x -= x0; r.y -= y0; r };
        out.text.runs = ui.frame.text.runs.iter().filter(|r| inside(r)).map(shift).collect();
        out.text.visible = ui.frame.text.visible.iter().filter(|r| inside(r)).map(shift).collect();
        flag_hotkeys(&mut out.text, ui);
        if scene.screen == ScreenKind::Space && self.rect == FRAME { flag_flight_keys(&mut out.text, ui); }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if *e == UiEvent::Up(Button::Left) { if let Some(at) = self.game(at) { let switch = desk_switch(ui, at); if !switch.is_empty() { return switch } } }
        if let Some(d) = ui.decision.as_ref().filter(|d| matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::List)) {
            if d.context.program == "t" && d.context.routine == "13FF2" {
                if let Some((col, row)) = self.game(at) {
                    let (c, r) = (col as i32, row as i32); let l = mt1::ground::starport::LEAVE_ROW;
                    let control = ((l.1 as i32..l.1 as i32 + l.3 as i32).contains(&c) && (l.0 as i32..l.0 as i32 + l.2 as i32).contains(&r)).then_some(("Leave starport - Esc", Event::Key(mt_engine::io::keys::ESC)));
                    match e {
                        UiEvent::Move => return vec![Intent::Hover(None), Intent::Hint(control.map(|c| c.0))],
                        UiEvent::Up(Button::Left) => if let Some((_, key)) = control { return vec![Intent::Raw(key)] },
                        _ => {}
                    }
                }
            }
            if *e == UiEvent::Up(Button::Left) {
                let Some((col, row)) = self.game(at) else { return Vec::new() };
                if let Some(l) = super::buy::shop_list(d) {
                    if ui.buy.as_ref().is_some_and(|(id, _)| *id == d.id) { return Vec::new() }
                    let rows = crate::mouse::decision::native_rows(&ui.frame.text.visible, d);
                    if let Some(&(i, _)) = rows.iter().find(|(_, r)| r.contains((col as i32, row as i32))) {
                        if l.rows.get(i).is_some_and(|r| r.enabled) { return vec![Intent::BuyPrompt(Some(i))] }
                    }
                    let button = crate::mouse::decision::button_route(d, (col as i32, row as i32));
                    let quantity = |e: &Vec<Event>| !e.is_empty() && button == crate::mouse::decision::raw(e);
                    return if quantity(&l.increase) || quantity(&l.decrease) { Vec::new() } else { button };
                }
                let hit = crate::mouse::decision::native_click(&ui.frame.text.visible, d, (col as i32, row as i32));
                if !hit.is_empty() { return hit }
                let button = crate::mouse::decision::button_route(d, (col as i32, row as i32));
                if !button.is_empty() { return button }
                if d.context.program != "s" && !(d.context.program == "t" && col >= 193 && row >= 141) { return Vec::new() }
            } else if matches!(e, UiEvent::Up(_) | UiEvent::DoubleClick(_)) { return Vec::new() }
        }
        if let Some(d) = ui.decision.as_ref().filter(|d| d.kind == Kind::Ack) {
            if *e == UiEvent::Up(Button::Left) && self.game(at).is_some() {
                let i = d.options.iter().position(|o| Some(&o.id) == d.default.as_ref()).unwrap_or(0);
                return crate::mouse::decision::option(d, i);
            }
            if matches!(e, UiEvent::Up(_) | UiEvent::DoubleClick(_)) { return Vec::new() }
        }
        if scene.screen == ScreenKind::Space && self.rect == FRAME {
            if let Some((col, row)) = self.game(at) {
                if let Some(d) = ui.decision.as_ref().filter(|d| d.context.program == "s" && d.kind == Kind::YesNo) {
                    if *e == UiEvent::Move {
                        let row = crate::mouse::decision::native_rows(&ui.frame.text.visible, d).into_iter().find(|(_, r)| r.contains((col as i32, row as i32))).map(|(i, _)| i);
                        return vec![Intent::Hover(None), Intent::Hint(row.map(|i| if i == 0 { "Quit - Y" } else { "Continue flight - N" }))];
                    }
                }
                let station = ui.decision.as_ref().is_some_and(|d| d.context.program == "s" && d.context.routine != "4FCB" && d.kind != Kind::YesNo);
                if station {
                    let control = station_control(ui, (col, row));
                    match e {
                        UiEvent::Move => return vec![Intent::Hover(None), Intent::Hint(control.as_ref().map(|c| c.0))],
                        UiEvent::Up(Button::Left) => if let Some((_, Some(keys))) = control { return drop_tip(ui).into_iter().chain(keys.iter().map(|&k| Intent::Raw(Event::Key(k)))).collect() },
                        _ => {}
                    }
                } else {
                    let control = flight_control(ui, (col, row));
                    match e {
                        UiEvent::Move => return vec![Intent::Hover(None), Intent::Hint(control.map(|c| c.0))],
                        UiEvent::Up(Button::Left) if control.is_some_and(|c| c.1 == b'P') => return vec![Intent::Raw(Event::Key(b'P' as u16))],
                        UiEvent::Up(Button::Left) if control.is_some_and(|c| c.1 == b'5') => return vec![Intent::Raw(Event::Key(0x14C))],
                        UiEvent::Up(Button::Left) if control.is_some_and(|c| c.1 != 0) && ui.hint.is_some() => return vec![Intent::Hint(None), Intent::Raw(Event::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col, row }))],
                        _ => {}
                    }
                }
            }
        }
        match e {
            UiEvent::DoubleClick(Button::Left) if (scene.screen == ScreenKind::Ground && self.rect == super::components::PARTY_RECT)
                || (scene.screen == ScreenKind::Record && self.rect == FRAME) => {
                portrait(at).map(|slot| Intent::Follow(Some(Unit::Traveller(slot)))).into_iter().collect()
            }
            UiEvent::Key(k) if ui.hint.is_some() => vec![Intent::Hint(None), Intent::Raw(Event::Key(*k))],
            UiEvent::Key(k) => vec![Intent::Raw(Event::Key(*k))],
            UiEvent::Up(Button::Left | Button::Right) => self.game(at).map(|(col, row)| {
                let event = if *e == UiEvent::Up(Button::Left) { mouse::LEFT_RELEASE } else { mouse::RIGHT_RELEASE };
                Intent::Raw(Event::Mouse(MouseEvent { event, buttons: 0, col, row }))
            }).into_iter().collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_missile_gun_has_tips_for_its_tubes_and_boxes() {
        let mut ui = UiState::default();
        let mut d = mt_engine::play::Decision::new(mt_engine::play::Kind::Menu, "s", "9FDB");
        ui.decision = Some(d.clone());
        assert_eq!(station_control(&ui, (260, 175)).map(|c| c.0), Some("Target - T"), "the laser's Target box");
        assert_eq!(station_control(&ui, (170, 175)), None, "no tubes on a laser");
        d.rects.push((0, [209, 168, 22, 23])); ui.decision = Some(d);
        assert_eq!(station_control(&ui, (260, 175)).map(|c| c.0), Some("Fire (after Target) - F"), "the missile gun's Fire box");
        assert_eq!(station_control(&ui, (170, 175)), Some(("Missile tube 2 - click Tar first, then this - 2", Some(vec![b'2' as mt_engine::io::Key]))));
    }
    #[test]
    fn a_tip_never_outlives_the_click_or_key_that_changes_the_screen() {
        let (scene, p) = (Scene::program(ScreenKind::Space), Picture::full());
        let mut ui = UiState::default();
        assert_eq!(p.input(&scene, &ui, &UiEvent::Move, (260, 95)), [Intent::Hover(None), Intent::Hint(Some("Jump - J"))]);
        let release = Intent::Raw(Event::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: 260, row: 95 }));
        assert_eq!(p.input(&scene, &ui, &UiEvent::Up(Button::Left), (260, 95)), [release.clone()], "no tip up: the bare 1990 release");
        assert_eq!(p.input(&scene, &ui, &UiEvent::Key(b'J' as u16), (260, 95)), [Intent::Raw(Event::Key(b'J' as u16))], "no tip up: the key alone");
        ui.hint = Some("Jump - J");
        assert_eq!(p.input(&scene, &ui, &UiEvent::Up(Button::Left), (260, 95)), [Intent::Hint(None), release], "the row's click drops the tip, then goes to the 1990 hit table");
        assert_eq!(p.input(&scene, &ui, &UiEvent::Key(b'J' as u16), (260, 95)), [Intent::Hint(None), Intent::Raw(Event::Key(b'J' as u16))]);
    }
    #[test]
    fn portrait_double_click_follows_and_preserves_both_raw_releases() {
        let ui = UiState::default();
        for (screen, picture) in [(ScreenKind::Ground, Picture::new(super::super::components::PARTY_RECT)), (ScreenKind::Record, Picture::full())] {
            let scene = Scene::program(screen); let before = scene.clone();
            for slot in 0..5 {
                let at = (6 + slot as i32 * 40, 20);
                let raw = vec![Intent::Raw(Event::Mouse(MouseEvent { event: mouse::LEFT_RELEASE, buttons: 0, col: at.0 as i16, row: at.1 as i16 }))];
                assert!(picture.input(&scene, &ui, &UiEvent::Down(Button::Left), at).is_empty());
                assert_eq!(picture.input(&scene, &ui, &UiEvent::Up(Button::Left), at), raw);
                assert_eq!(picture.input(&scene, &ui, &UiEvent::DoubleClick(Button::Left), at), [Intent::Follow(Some(Unit::Traveller(slot)))]);
                assert_eq!(picture.input(&scene, &ui, &UiEvent::Up(Button::Left), at), raw);
            }
            for at in [(-1, 20), (5, 20), (206, 20), (20, 50), (i32::MAX, i32::MIN)] {
                assert!(picture.input(&scene, &ui, &UiEvent::DoubleClick(Button::Left), at).is_empty());
            }
            assert!(picture.input(&scene, &ui, &UiEvent::DoubleClick(Button::Right), (20, 20)).is_empty());
            assert_eq!(scene, before);
        }
    }
    #[test]
    fn native_welcome_rows_answer_through_the_layout_at_two_window_sizes() {
        use mt_engine::{io::keys, play::{Decision, Kind}, present::textlog::Run};
        use crate::{mouse::decision, panels::{components, Host, Layout}};
        let labels = ["Start New Game", "Continue Old Game", "Get Started Quick", "Return to DOS"];
        let mut d = Decision::new(Kind::Menu, "cg", "519A");
        for (i, l) in labels.iter().enumerate() { let mut input = vec![Event::Key(keys::DOWN); i]; input.push(Event::Key(keys::ENTER)); d.options.push(mt_engine::play::Option::new(format!("menu:{i}"), *l, input)); }
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs = Vec::new();
        for (i, l) in labels.iter().enumerate() { let mut x = 100; for c in l.bytes() { if c != b' ' { runs.push(glyph(x, 150 + i as i64 * 10, c)); } x += 6; } }
        let scene = Scene::program(ScreenKind::Cg);
        let mut ui = UiState::default(); ui.decision = Some(d.clone()); ui.frame.text.visible = runs;
        let mut layout = Layout::default(); components::register(&mut layout, std::sync::Arc::default());
        let mut hits = 0;
        for size in [(320, 200), (640, 400)] {
            layout.arrange_for(&scene, &ui, size);
            let r = match layout.host("Picture").unwrap() { Host::Dock(r) => r, _ => Rect::new(0, 0, 320, 200) };
            for (i, _) in labels.iter().enumerate() {
                let at = (r.x + 130, r.y + 150 + i as i32 * 10 + 4);
                if layout.hit(&scene, None, at).map(|h| h.0) != Some("Picture") { continue }
                assert_eq!(layout.route(&scene, &mut ui, None, UiEvent::Up(Button::Left), at), decision::option(&d, i), "{size:?} row {i}"); hits += 1;
            }
            let outside = (r.x + 130, r.y + 140);
            assert!(layout.route(&scene, &mut ui, None, UiEvent::Up(Button::Left), outside).is_empty() || layout.hit(&scene, None, outside).map(|h| h.0) != Some("Picture"), "{size:?} above the rows: nothing");
            assert!(layout.route(&scene, &mut ui, None, UiEvent::Up(Button::Right), (r.x + 130, r.y + 154)).is_empty(), "{size:?} right click never acts");
        }
        assert!(hits >= 4, "rows reached through the Picture at both sizes: {hits}");
    }

    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    enum Step {  Tile(i16),  Rect(usize), Key(u16),  Hidden(i16) }
    struct Done;
    struct DeskWindow { store: crate::mouse::decision::Store, ui: UiState, script: std::collections::VecDeque<Step>, log: std::rc::Rc<std::cell::RefCell<Vec<String>>>, hw: std::collections::VecDeque<Event>, idle: usize, rgb: Vec<u32>, walked: bool, held: usize }
    fn tile(n: i16) -> (i32, i32) {
        let l = mt1::ground::starport::LEAVE_ROW;
        let r = if n == 0 { [l.1 as i32, l.0 as i32, l.3 as i32, l.2 as i32] } else { mt1::ground::starport::cells().find(|c| c.0 == n).unwrap().1 };
        (r[0] + r[2] / 2, r[1] + r[3] / 2)
    }
    impl DeskWindow {
        fn serve(&mut self) {
            use mt_engine::play::Kind;
            self.store.restore(&mut self.ui);
            if self.store.pending() || !self.hw.is_empty() || self.idle < 60 { return }
            let Some(d) = self.ui.decision.clone() else { return };
            if d.kind == Kind::Ground {
                if !self.walked { self.walked = true; self.idle = 0; self.hw.extend([Event::Key(mt_engine::io::keys::LEFT), Event::KeyUp(mt_engine::io::keys::LEFT)]); }
                else if self.script.is_empty() { std::panic::resume_unwind(Box::new(Done)) }
                return
            }
            let Some(step) = self.script.pop_front() else { return };
            let at = match step {
                Step::Key(k) => { self.hw.extend([Event::Key(k), Event::KeyUp(k)]); self.log.borrow_mut().push(format!("key {k}")); self.idle = 0; return }
                Step::Hidden(n) => { let (x, y) = tile(n); assert!(desk_switch(&self.ui, (x as i16, y as i16)).is_empty(), "tile {n} is hidden under {}: no switch", d.context.routine); self.log.borrow_mut().push(format!("hidden {n}")); self.idle = 0; return }
                Step::Tile(n) => { self.log.borrow_mut().push(format!("tile {n}")); tile(n) }
                Step::Rect(i) => { let r = d.rects.iter().find(|(k, _)| *k == i).unwrap_or_else(|| panic!("{} publishes no rectangle {i}: {:?}", d.context.routine, d.rects)).1; self.log.borrow_mut().push(format!("rect {i}")); (r[0] + r[2] / 2, r[1] + r[3] / 2) }
            };
            let got = Picture::full().input(&Scene::program(ScreenKind::Picture), &self.ui, &UiEvent::Up(Button::Left), at);
            let events: Vec<Event> = got.iter().map(|i| if let Intent::Raw(e) = i { *e } else { panic!("not a native route: {i:?}") }).collect();
            assert!(!events.is_empty(), "the click at {at:?} must answer {}.{}; log {:?}", d.context.program, d.context.routine, self.log.borrow());
            if crate::mouse::decision::released_route(&got).is_some() { self.store.queue(&mut self.ui, &events); } else { self.hw.extend(events); }
            self.idle = 0;
        }
    }
    impl mt_engine::io::Input for DeskWindow {
        fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
        fn decide(&mut self, d: &mt_engine::play::Decision) {
            self.store.record(&mut self.ui, d);
            if d.kind != mt_engine::play::Kind::Ground { self.log.borrow_mut().push(format!("{}.{} {}", d.context.program, d.context.routine, d.prompt.clone().unwrap_or_default())); }
        }
        fn next_key(&mut self) -> mt_engine::io::Key { loop { if let Event::Key(k) = self.poll(&mut mt_engine::sim::tick::Counter::default()) { return k } } }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event {
            self.serve();
            if let Some(e) = self.store.pop() { return e }
            if matches!(self.hw.front(), Some(Event::KeyUp(_))) && self.held < 30 { self.held += 1; }
            else if let Some(e) = self.hw.pop_front() { self.held = 0; self.store.hardware(&mut self.ui, e); return e }
            self.idle += 1; assert!(self.idle < 30000, "stalled at {:?}; log {:?}", self.ui.decision.as_ref().map(|d| d.context.routine.clone()), self.log.borrow());
            clock.wait_tick(); Event::Idle
        }
        fn arrived(&mut self) -> bool { false }
        fn present(&mut self, rgb: &[u32]) {
            if self.rgb[..] == rgb[..] { return }
            self.rgb = rgb.to_vec(); *self.ui.frame = crate::pipeline::Frame::from_rgb(rgb); self.store.refresh();
            if crate::mouse::decision::line_texts(&self.ui.frame.text.visible).iter().any(|(_, l)| l.contains("gamesaved")) { let mut log = self.log.borrow_mut(); if log.last().map(String::as_str) != Some("Game saved.") { log.push("Game saved.".into()); } }
        }
        fn scene(&mut self, s: &dyn std::any::Any) { if s.is::<Scene>() { self.store.refresh(); self.store.settle(); } }
    }
    #[test]
    fn a_click_on_another_starport_tile_leaves_the_open_service() {
        use mt1::ground::starport::cash;
        let Some(dir) = oracle_env::gamefiles() else { return }; let dir = std::path::PathBuf::from(dir);
        let prepared = crate::scenario::load(&std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml"), &dir).unwrap();
        let mut g = prepared.game;
        let slot = std::env::temp_dir().join(format!("mt-fx-starport-{}", std::process::id())); std::fs::create_dir_all(&slot).unwrap();
        g.save_slot = Some(mt1::save::Slot { dir: slot.clone(), build: "test" });
        g.mem.set_party_u8(0xFC, 50);
        let log: std::rc::Rc<std::cell::RefCell<Vec<String>>> = Default::default();
        let script = [Step::Tile(16), Step::Tile(2), Step::Rect(0), Step::Tile(16), Step::Tile(13), Step::Rect(0), Step::Rect(0), Step::Rect(0), Step::Rect(0), Step::Tile(11),
            Step::Tile(16), Step::Rect(0), Step::Hidden(11), Step::Hidden(0), Step::Key(27), Step::Tile(0)];
        mt_engine::present::textlog::set_enabled(true);
        g.input = Box::new(DeskWindow { store: Default::default(), ui: UiState::default(), script: script.into(), log: log.clone(), hw: Default::default(), idle: 0, rgb: Vec::new(), walked: false, held: 0 });
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(1234)); g.scene = Some(mt1::ui::SceneHook::default());
        g.tooling = Some(Box::new(crate::scenario::Loader::new(Default::default(), dir.clone(), prepared.extensions)));
        if g.squad.is_none() { g.squad = Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap())); }
        let before = ((-2..5).filter(|w| *w != -1).map(|w| cash(&g, w)).collect::<Vec<_>>(), g.mem.party_u8(0xFC));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| mt1::ground::main_loop::run(&mut g)));
        mt_engine::present::textlog::set_enabled(false);
        let saved = slot.join("game1.mtsav").exists(); let _ = std::fs::remove_dir_all(&slot);
        if let Err(e) = r { if !e.is::<Done>() { std::panic::resume_unwind(e) } }
        let log = log.borrow();
        let want = ["t.13FF2 ", "tile 16", "t.1199B Who will pay for purchases?", "tile 2", "t.11571 Save to which position?", "rect 0", "Game saved.", "t.13FF2 ",
            "tile 16", "t.1199B Who will pay for purchases?", "tile 13", "t.11571 Which type of fuel?",
            "rect 0", "t.1199B Who will pay for refueling?", "rect 0", "t.16374 ", "rect 0", "t.16374 ", "rect 0", "t.16374 ", "tile 11", "t.1199B Who will recieve the money?",
            "tile 16", "t.1199B Who will pay for purchases?", "rect 0", "t.11571 Weapon System                 Cost", "hidden 11", "hidden 0", "key 27", "t.1199B Who will pay for purchases?", "tile 0"];
        assert_eq!(log[..], want[..], "each click, then the wait that answered it; after the last one the yard");
        assert!(saved, "the save was written");
        assert_eq!(((-2..5).filter(|w| *w != -1).map(|w| cash(&g, w)).collect::<Vec<_>>(), g.mem.party_u8(0xFC)), before, "nothing bought, paid or changed on the way out");
        let table: Vec<(i16, [i32; 4])> = (0..13u16).map(|i| { let at = 0x1F59 + 9 * i; (g.mem.u8(at + 8) as i16 - 200, [g.mem.i16(at + 2) as i32, g.mem.i16(at) as i32, g.mem.i16(at + 6) as i32, g.mem.i16(at + 4) as i32]) }).collect();
        assert_eq!(mt1::ground::starport::cells().collect::<Vec<_>>(), table, "starport::cells = DS:1F59");
    }
}
