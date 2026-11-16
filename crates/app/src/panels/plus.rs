use super::{assets::Assets, Anchor, Button, Component, Dock, Origin, Placement, Rect, Size, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene, scene::PlusAction as A};
use crate::{look::Look, panels::render::Metrics, pipeline};
use mt_engine::io::keys;
use std::sync::Arc;
pub const ID: &str = "Plus";
pub const WIDTH: u32 = 320;
pub const HEIGHT: u32 = 250;
pub const COLS: usize = 52;
pub const HELP_ROWS: usize = 24;
pub const JOURNAL_ROWS: usize = 14;
pub const CHARACTER_ROWS: usize = 22;
pub const BUG: u8 = 0; pub const HELP: u8 = 1; pub const JOURNAL: u8 = 2; pub const MAP: u8 = 3; pub const CHARACTERS: u8 = 4;
pub const TEST: u8 = 16;
pub const LOG: u8 = 9;
pub const LAST_TAB: u8 = 255;
pub const ORDER: [u8; 7] = [JOURNAL, LOG, CHARACTERS, MAP, TEST, HELP, BUG];
const NAMES: [&str; 17] = ["Bug", "Help", "Journal", "Map", "Characters", "", "", "", "", "Log", "", "", "", "", "", "", "Test"];
pub fn main_tab(tab: u8) -> u8 { if tab >= TEST { TEST } else if tab == LOG { LOG } else if tab >= CHARACTERS { CHARACTERS } else { tab } }
pub fn member(tab: u8) -> u8 { tab.saturating_sub(CHARACTERS).min(4) }
pub fn sheet_index(tab: u8) -> usize { tab.saturating_sub(TEST) as usize }
pub fn next_tab(tab: u8, back: bool) -> u8 { let n = ORDER.len(); let i = ORDER.iter().position(|&t| t == main_tab(tab)).unwrap_or(0); ORDER[(i + if back { n - 1 } else { 1 }) % n] }
pub fn next_member(mask: u8, member: u8, back: bool) -> u8 {
    (1..=5).map(|k| (member as usize + if back { 5 - k } else { k }) % 5).find(|&m| mask & (1 << m) != 0).unwrap_or(member as usize) as u8
}
const BOX_COLS: usize = 38;
const TAB_W: u32 = 42; const TAB_STEP: i32 = 45;
const fn strip(i: usize, y: i32) -> Rect { Rect::new(4 + i as i32 * TAB_STEP, y, TAB_W, 16) }
fn strip_rects<const N: usize>(names: &[String], y: i32) -> [Rect; N] {
    let cap = (WIDTH as i32 - 8 - 3 * (N as i32 - 1)) / N as i32;
    let w = names.iter().filter_map(|n| text_width(n)).max().map_or(TAB_W as i32, |w| (w + 8).max(TAB_W as i32)).min(cap) as u32;
    std::array::from_fn(|i| Rect::new(4 + i as i32 * (w as i32 + 3), y, w, 16))
}
fn tab_rects() -> [Rect; 7] { strip_rects(&ORDER.map(|t| NAMES[t as usize].to_string()), 4) }
fn member_names(scene: &Scene) -> [String; 5] { let mask = scene.party.mask(); std::array::from_fn(|i| if mask & (1 << i) != 0 { scene.party.name(i as u8).chars().take(9).collect() } else { String::new() }) }
fn member_rects(scene: &Scene) -> [Rect; 5] { strip_rects(&member_names(scene), 28) }
fn centred(r: Rect, text: &str) -> i32 { r.x + (r.w as i32 - text_width(text).unwrap_or(text.len() as i32 * CELL)).max(0) / 2 }
const KINDS: [(&str, i32, u32); 3] = [("Bug", 4, 48), ("QoL request", 56, 120), ("Question", 180, 96)];
const BUG_BOX: Rect = Rect::new(4, 50, 312, 114);
const SAVE: Rect = Rect::new(4, 170, 120, 16);
const NOTE_BOX: Rect = Rect::new(4, 26, 312, 40);
const ADD: Rect = Rect::new(4, 72, 88, 16);
const TABS: [Rect; 7] = [strip(0, 4), strip(1, 4), strip(2, 4), strip(3, 4), strip(4, 4), strip(5, 4), strip(6, 4)];
const MEMBERS: [Rect; 5] = [strip(0, 28), strip(1, 28), strip(2, 28), strip(3, 28), strip(4, 28)];
const PREV: Rect = strip(0, 28); const NEXT: Rect = strip(1, 28); const PASS: Rect = strip(4, 28); const FAIL: Rect = strip(5, 28);
const PASS_FILL: u8 = 10; const FAIL_FILL: u8 = 12;
pub const BOXES: [Rect; 22] = [TABS[0], TABS[1], TABS[2], TABS[3], TABS[4], TABS[5], TABS[6],
    Rect::new(4, 28, 48, 16), Rect::new(56, 28, 120, 16), Rect::new(180, 28, 96, 16), BUG_BOX, SAVE, ADD,
    MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], MEMBERS[4], PREV, NEXT, PASS, FAIL];
const BUTTONS: [Rect; 21] = [TABS[0], TABS[1], TABS[2], TABS[3], TABS[4], TABS[5], TABS[6], BOXES[7], BOXES[8], BOXES[9], SAVE, ADD,
    MEMBERS[0], MEMBERS[1], MEMBERS[2], MEMBERS[3], MEMBERS[4], PREV, NEXT, PASS, FAIL];
const PANEL: u8 = 7; const INK: u8 = 0; const FIELD: u8 = 15; const SELECTED: u8 = 9; const DIM: u8 = 8;
const BOLD: u8 = 0x40;
pub fn party_register(slot: u8) -> u8 { slot % 5 + 1 }
const KEY_COLS: usize = 22; const DASH_X: i32 = 96; const KEY_X: i32 = DASH_X + 10;
const DASH: u8 = 0x1F;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HelpRow { Blank, Heading(String), Entry(String, String), Prose(String),
    Title(String, u8) }
pub fn player_help(scene: &Scene, decision: Option<&mt_engine::play::Decision>) -> Vec<HelpRow> {
    type Section = (&'static str, &'static [(&'static str, &'static str)], &'static [(&'static str, &'static str)]);
    const EXPLORE: Section = ("Exploring on foot", &[
        ("Arrows / keypad", "Walk, eight ways (PgUp NE, PgDn SE, End SW, keypad 7 NW)"),
        ("X", "Choose a square: arrows move the cursor, Enter walks there, Esc cancels"),
        ("1-5", "Select a Traveller"), ("Tab / Shift+Tab", "Next / previous Traveller"),
        ("C", "Converse with the person you face"), ("I", "Items: take or use"), ("V", "Vehicle: leave, status"),
        ("P", "Party: break up or regroup"), ("O", "Swap marching order (when grouped)"), ("G", "Game menu: restore, quit, sound"),
        ("F1-F5", "A Traveller's record"), ("Home", "Follow the selected Traveller"), ("[ / ]", "Older / newer log line"), ("L", "Long log"),
        ("Esc", "Back; the quit menu"), ("?", "This help"), ("F12", "The Plus window")],
        &[("Click a square", "Walk there"), ("Click a Traveller", "Select; double-click follows; right-click opens the record"),
          ("Click a person", "Converse"), ("Click an item", "Take it"), ("Right-click a target", "Its actions"),
          ("Command buttons", "Game, Party, Items, Vehicle, the records")]);
    const FIGHT: Section = ("A fight", &[
        ("Space or O", "Round start: we go first"), ("N", "Round start: they go first"), ("1-5", "Select a Traveller"),
        ("Arrows", "Step one square (move points)"), ("Tab / Shift+Tab", "Next / previous thug in sight: the target"),
        ("F or Enter", "Fire at the target"), ("R", "Reload"), ("Q", "Semi or full auto"), ("=", "Swap hand"),
        ("T", "Grenade: E / C / R picks the type, then Tab or X aims it"), ("M", "Menu on the target, else the selected Traveller"),
        ("+ / -", "Tactical points"), ("H", "Hold here"), ("Shift+H", "Hold, interrupt"), ("Ctrl+H", "Hold fire"),
        ("O or Space", "Carry out orders, keep the turn"), ("E", "End the turn"), ("Esc", "Back out one level")],
        &[("Click a Traveller", "Select"), ("Click a square", "Move there"), ("Click an enemy", "Fire at it"), ("Right-click", "Actions"),
          ("END TURN button", "End the turn"), ("Combat buttons", "Reload, orders, end turn")]);
    const STARPORT: Section = ("Starport, shops and offices", &[
        ("Up / Down", "Move in the list"), ("Enter", "Choose"), ("Esc", "Leave"), ("+ / -", "Quantity"),
        ("B or F / A / P", "Refuel: buy one unit, all you can afford, pay"), ("Digits, Backspace", "An amount"), ("Space", "Spin the casino reels"),
        ("Save", "Starport list, second item")],
        &[("Click a row", "Select it; click it again to choose"), ("Click a shop item", "Buy or sell one: Yes or Cancel"), ("Cargo page [-] [+]", "A lot less / more; hold to repeat; Done / Back"), ("Refuel words", "Buy one unit, All, Pay: click or key"), ("OK / Back / Cancel", "As named"), ("Click an office", "Enter it")]);
    const SPACE: Section = ("In space", &[
        ("Up / Down", "Thrust"), ("Left / Right", "Turn"), ("Keypad 5", "Brake"), ("T", "Take off"), ("L", "Land at a port world"),
        ("J", "Jump screen: arrows or 1-8 pick a system, Enter jumps, Esc cancels"), ("S", "Stations"), ("C", "Comms"),
        ("Esc or I", "Back to flight"), ("Q", "Quit (no save)"),
        ("A", "Station: assign a crew member"), ("V", "Station: view another station"), ("V, then F1-F8", "Station: switch to Pilot, Gunner 1, Gunner 2, Computer, Engineering, Communications, Medical, Crew (or click its icon)"), ("X", "Station: tactical map"),
        ("Left / Right", "Gunner: turn the gun"), ("T", "Gunner: aim"), ("Arrows", "Gunner: move the reticle"), ("Enter or F", "Gunner: fire"),
        ("R", "Gunner: reload missiles"), ("PgUp / PgDn, Space", "A station's side menu")],
        &[("Click a flight control", "Thrust, turn, stop, land, jump, stations"), ("Click a station", "Open it"), ("Click a system", "Pick it on the jump map"), ("Back", "Return")]);
    const RECORD: Section = ("A Traveller's record", &[
        ("Esc", "Close"), ("Up / Down, 1-8", "Choose an action"), ("Enter", "Detail"),
        ("P", "Pick an item: open another record to give it, Esc drops it"), ("M", "Move an item"), ("U", "Use or equip"), ("Q", "Unequip"),
        ("PgUp / PgDn", "Skill pages")],
        &[("Click an action", "As named"), ("Click a Traveller", "That record"), ("Close", "Return")]);
    const PLUS: Section = ("The Plus window", &[
        ("F12", "Open or close Plus"), ("?", "Help"), ("Tab / Shift+Tab", "Next / previous tab"),
        ("Left / Right", "Next Traveller (Characters); report kind (Bug)"), ("Enter", "Save the report or note; PASS on Test"),
        ("Shift+Enter", "FAIL on Test"), ("PgUp / PgDn, wheel", "Scroll"), ("Esc", "Close")],
        &[("Click a tab", "Open it"), ("Click a name", "That Traveller"), ("PASS / FAIL", "Record the test")]);
    const WINDOW: Section = ("The game window", &[
        ("F11", "Fullscreen"), ("Ctrl+S", "Pixel shape"), ("Ctrl+Shift+L", "Testing scenarios")],
        &[("Menu bar", "File, Look, View, Scenario, Help, Plus page (the purple button)")]);
    use mt1::ui::ScreenKind as K;
    let fight = scene.screen == K::Ground && (scene.squad.active && scene.squad.encounter || scene.combat);
    let starport = matches!(scene.screen, K::Ground | K::Picture) && matches!(scene.context.event, 9 | 10) || scene.screen == K::Picture;
    let first: Section = match scene.screen { K::Space => SPACE, K::Record => RECORD, _ if fight => FIGHT, _ if starport => STARPORT, _ => EXPLORE };
    let mut rows = Vec::new();
    if let Some(d) = decision.filter(|d| d.kind != mt_engine::play::Kind::CombatTurn) {
        let mut labels: Vec<&str> = d.options.iter().filter(|o| o.enabled).map(|o| o.label.as_str()).collect();
        if let Some(list) = &d.list {
            labels.extend(list.rows.iter().filter(|r| r.enabled).map(|r| r.label.as_str()));
            if !list.increase.is_empty() { labels.push("+"); } if !list.decrease.is_empty() { labels.push("-"); } labels.push("OK");
        }
        if !labels.is_empty() { rows.push(HelpRow::Heading("On this screen now".into())); rows.push(HelpRow::Prose(labels.join(", "))); rows.push(HelpRow::Blank); }
    }
    for (title, keys, mouse) in std::iter::once(first).chain([EXPLORE, FIGHT, STARPORT, SPACE, RECORD, PLUS, WINDOW].into_iter().filter(|s| s.0 != first.0)) {
        rows.push(HelpRow::Heading(title.into()));
        if title == FIGHT.0 {
            rows.push(HelpRow::Prose("How turns work: the two sides take turns, one unit at a time. On a party turn you pick any Traveller who has not acted yet; the names in the party slots are only the default order, not a queue you must follow. The bar above the map shows who is up next. Clicking a name there selects that Traveller; it never skips an enemy's turn, the other side always gets its own.".into()));
            rows.push(HelpRow::Blank);
        }
        for (k, v) in keys { rows.push(HelpRow::Entry((*k).into(), (*v).into())); }
        rows.push(HelpRow::Heading(format!("{title}: mouse")));
        for (k, v) in mouse { rows.push(HelpRow::Entry((*k).into(), (*v).into())); }
        rows.push(HelpRow::Blank);
    }
    while matches!(rows.last(), Some(HelpRow::Blank)) { rows.pop(); }
    rows
}
const CELL: i32 = 3;
pub fn wrap_px(text: &str, max_px: i32) -> Vec<String> {
    let max_chars = (max_px / CELL).max(1) as usize;
    let Some(face) = crate::font_menu::selected(&crate::font_menu::plus_font()) else { return crate::plus::help::lines(text, (max_px / 6).max(1) as usize) };
    let fits = |s: &str| s.chars().count() <= max_chars && width_in(&face, s) <= max_px;
    let mut out = Vec::new();
    for para in crate::plus::help::lines(text, usize::MAX) {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let mut chunks: Vec<String> = Vec::new();
            for ch in word.chars() {
                match chunks.last_mut() { Some(c) if fits(&format!("{c}{ch}")) => c.push(ch), _ => chunks.push(ch.to_string()) }
            }
            for chunk in chunks {
                let candidate = if line.is_empty() { chunk.clone() } else { format!("{line} {chunk}") };
                if !line.is_empty() && !fits(&candidate) { out.push(std::mem::take(&mut line)); line = chunk; } else { line = candidate; }
            }
        }
        out.push(line);
    }
    if out.is_empty() { out.push(String::new()); }
    out
}
pub fn help_lines(rows: &[HelpRow]) -> Vec<(i32, String, u8)> {
    let mut out = Vec::new();
    for row in rows {
        match row {
            HelpRow::Blank => out.push((4, String::new(), INK)),
            HelpRow::Heading(h) => out.push((4, h.clone(), INK | BOLD)),
            HelpRow::Title(t, reg) => out.push((4, t.clone(), (reg & 15) | BOLD)),
            HelpRow::Prose(p) => out.extend(wrap_px(p, WIDTH as i32 - 8).into_iter().map(|l| (4, l, INK))),
            HelpRow::Entry(k, v) => {
                let key: String = crate::plus::help::lines(k, KEY_COLS).into_iter().next().unwrap_or_default();
                for (i, l) in wrap_px(v, WIDTH as i32 - 4 - KEY_X).into_iter().enumerate() {
                    out.push(if i == 0 { (4, format!("{key}\n{l}"), INK) } else { (KEY_X, l, INK) });
                }
            }
        }
    }
    out
}

pub struct Plus { assets: Arc<Assets> }
impl Plus {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    pub fn placement() -> Placement {
        Placement { dock: Dock::Float(Anchor::Centre), size: Size { preferred: (WIDTH, HEIGHT), min: (WIDTH, HEIGHT) },
            order: 100, screens: &[], when: super::always, origin: Origin::New, optional: false, undockable: false, title: ID }
    }
    fn fill(out: &mut Surface, r: Rect, c: u8) { for y in r.y..r.y + r.h as i32 { for x in r.x..r.x + r.w as i32 { out.put(x, y, c); } } }
    fn frame(out: &mut Surface, r: Rect, c: u8) {
        for x in r.x..r.x + r.w as i32 { out.put(x, r.y, c); out.put(x, r.y + r.h as i32 - 1, c); }
        for y in r.y..r.y + r.h as i32 { out.put(r.x, y, c); out.put(r.x + r.w as i32 - 1, y, c); }
    }
    fn label(&self, out: &mut Surface, text: &str, at: (i32, i32), colour: u8, _under: u8) { run(out, text, at, colour); }
    fn boxed(&self, out: &mut Surface, r: Rect, label: &str, bg: u8, ink: u8, double: bool) {
        Self::fill(out, r, bg); Self::frame(out, r, INK);
        if double { Self::frame(out, Rect::new(r.x + 1, r.y + 1, r.w - 2, r.h - 2), INK); }
        self.label(out, label, (centred(r, label), r.y + (r.h as i32 - 8) / 2), ink, bg);
    }
    fn button(&self, out: &mut Surface, r: Rect, label: &str, selected: bool) {
        self.boxed(out, r, label, if selected { SELECTED } else { FIELD }, if selected { FIELD } else { INK }, false);
    }
    fn member_tab(&self, out: &mut Surface, r: Rect, label: &str, reg: u8, selected: bool) { self.boxed(out, r, label, reg, FIELD, selected); }
    fn field(&self, out: &mut Surface, r: Rect, text: &str) {
        Self::fill(out, r, FIELD); Self::frame(out, r, INK);
        let rows = (r.h as usize).saturating_sub(8) / 9;
        let lines = wrap(text, BOX_COLS);
        let skip = lines.len().saturating_sub(rows);
        let shown = &lines[skip..];
        let mut caret = (r.x + 4, r.y + 4);
        for (i, line) in shown.iter().enumerate() {
            let y = r.y + 4 + i as i32 * 9;
            self.assets.text(out, line, (r.x + 4, y), INK);
            caret = (r.x + 4 + text_width(line).unwrap_or(line.len() as i32 * 6), y);
        }
        if caret.1 + 9 < r.y + r.h as i32 - 1 { self.assets.text(out, &" ".repeat(BOX_COLS), (r.x + 4, caret.1 + 9), FIELD); }
        for y in 0..8 { out.put(caret.0, caret.1 + y, INK); }
    }
    fn rows(&self, out: &mut Surface, text: &str, top: i32, rows: usize, scroll: usize, colour: u8, max_px: i32, scrollable: bool) {
        let lines: Vec<(i32, String, u8)> = wrap_px(text, max_px).into_iter().map(|l| (4, l, colour)).collect();
        self.lines(out, &lines, top, rows, scroll, scrollable);
    }
    fn lines(&self, out: &mut Surface, lines: &[(i32, String, u8)], top: i32, rows: usize, scroll: usize, scrollable: bool) {
        let put = |out: &mut Surface, l: &(i32, String, u8), y: i32| {
            if let Some((key, meaning)) = l.1.split_once(['\n']) {
                run(out, key, (l.0, y), l.2);
                if key.ends_with(':') { run(out, meaning, (DASH_X, y), l.2); }
                else { run(out, std::str::from_utf8(&[DASH]).unwrap_or("-"), (DASH_X, y), l.2); run(out, meaning, (KEY_X, y), l.2); }
            } else { run(out, &l.1, (l.0, y), l.2); }
        };
        if !scrollable || lines.len() <= rows {
            for (i, line) in lines.iter().take(rows).enumerate() { put(out, line, top + i as i32 * 9); }
            return;
        }
        let start = scroll.min(max_scroll(lines.len(), rows));
        let (mut y, mut avail) = (top, rows);
        if start > 0 { run(out, "^ more above: wheel or PgUp", (4, y), DIM); y += 9; avail -= 1; }
        let below = start + avail < lines.len(); if below { avail -= 1; }
        for (i, line) in lines.iter().skip(start).take(avail).enumerate() { put(out, line, y + i as i32 * 9); }
        if below { run(out, "v more below: wheel or PgDn", (4, y + avail as i32 * 9), DIM); }
    }
}
const PITCH: f32 = 9.0; const BASELINE: f32 = 6.0;
fn px(face: &crate::font_menu::Face, sy: f32) -> f32 {
    let line = face.font.horizontal_line_metrics(100.0).map(|m| (m.ascent - m.descent) / 100.0).filter(|h| *h > 0.0).unwrap_or(1.2);
    (PITCH * sy / line).max(1.0)
}
fn lay(face: &crate::font_menu::Face, text: &str, px: f32) -> (Vec<(char, f32)>, f32) {
    let (mut pen, mut out, mut prev) = (0.0f32, Vec::new(), None);
    for c in text.chars() {
        if let Some(p) = prev { pen += face.font.horizontal_kern(p, c, px).unwrap_or(0.0); }
        out.push((c, pen)); pen += face.font.metrics(c, px).advance_width; prev = Some(c);
    }
    (out, pen)
}
pub fn text_width(text: &str) -> Option<i32> {
    let face = crate::font_menu::selected(&crate::font_menu::plus_font())?;
    Some(width_in(&face, text))
}
fn width_in(face: &crate::font_menu::Face, text: &str) -> i32 { lay(face, text, px(face, 1.0)).1.round() as i32 }
type Line = (String, bool, bool, u8, i64, i64, i64, Vec<u8>);
fn lines(log: &mt_engine::present::textlog::TextLog) -> Vec<Line> {
    let mut runs: Vec<_> = log.visible.iter().filter(|r| r.page == log.page && r.width > 0 && !r.bytes.is_empty()).collect();
    runs.sort_by_key(|r| (r.y, r.x));
    let mut out: Vec<Line> = Vec::new();
    for r in runs {
        let b = r.bytes[0];
        let c = if b == DASH { '\u{2014}' } else { pipeline::text::shown(b).or_else(|| (0x20..0x7f).contains(&b).then_some(b as char)).unwrap_or(' ') };
        match out.last_mut() {
            Some(l) if l.6 == r.y && l.5 == r.x => { l.0.push(c); l.5 = r.x + r.width as i64; l.7.push(r.colour & 15); }
            _ => out.push((c.to_string(), r.colour & pipeline::text::DIMMED != 0, r.colour & BOLD != 0, r.colour & 15, r.x, r.x + r.width as i64, r.y, vec![r.colour & 15])),
        }
    }
    out
}
pub fn compose(surfaces: impl IntoIterator<Item = (Rect, Surface)>, m: Metrics, look: &Look) -> Vec<u32> {
    let Some(regular) = crate::font_menu::selected(look.text_font()) else { return super::render::compose(surfaces, m, look) };
    let heavy = crate::font_menu::selected(&crate::font_menu::plus_font_bold()).unwrap_or_else(|| regular.clone());
    let mut out = pipeline::background::paint(m.pixels.out_width, m.pixels.out_height, look);
    for (r, mut s) in surfaces {
        let p = m.rect(r);
        if p.width == 0 || p.height == 0 || out.is_empty() || s.text.overflowed || s.w == 0 || s.h == 0 { continue; }
        pipeline::text::restore_background(&mut s.indexed, (s.w, s.h), &s.text, look);
        let colors = pipeline::palette::map(&s.indexed, &s.regs, look);
        let scaled = pipeline::scale::scale_surface(&colors, s.w, s.h, p.width, p.height, look);
        if scaled.len() != p.width * p.height { continue; }
        for y in 0..p.height { let at = (p.y + y) * p.out_width + p.x; out[at..at + p.width].copy_from_slice(&scaled[y * p.width..(y + 1) * p.width]); }
        let (sx, sy) = (p.width as f32 / s.w as f32, p.height as f32 / s.h as f32);
        let size = px(&regular, sy);
        let registers = pipeline::palette::map(&(0u8..16).collect::<Vec<_>>(), &s.regs, look);
        for (text, dimmed, bold, _reg, x0, x1, y, regs) in lines(&s.text) {
            let face = if bold { &heavy } else { &regular };
            let coloured = |reg: u8| ((1..=5).contains(&reg) || reg == ERROR_INK).then(|| registers[reg as usize]);
            let centred = BUTTONS.iter().find(|b| x0 >= b.x as i64 && x1 <= (b.x + b.w as i32) as i64 && b.contains((x0 as i32, y as i32))
                && b.contains((x1 as i32 - 1, y as i32)) && ((x0 + x1) - (2 * b.x as i64 + b.w as i64)).abs() <= 8);
            let limit = centred.map_or((s.w as i64 - x0).max(x1 - x0) as f32 * sx, |b| (b.w as f32 - 4.0) * sx);
            let (mut glyphs, mut width) = lay(face, &text, size);
            let mut px = size;
            if width > limit && width > 0.0 { px = (size * limit / width).max(1.0); (glyphs, width) = lay(face, &text, px); }
            let left = centred.map_or(x0 as f32 * sx, |b| (b.x as f32 + b.w as f32 / 2.0) * sx - width / 2.0);
            let baseline = (y as f32 + BASELINE) * sy;
            for (k, (c, pen)) in glyphs.into_iter().enumerate() {
                let party = regs.get(k).copied().and_then(coloured);
                let Some((g, alpha)) = crate::font_menu::font_operation(|| face.font.rasterize(c, px)) else { continue };
                let (gx, gy) = ((left + pen).round() as i64 + g.xmin as i64, baseline.round() as i64 - (g.ymin + g.height as i32) as i64);
                for row in 0..g.height { for col in 0..g.width {
                    let a = alpha[row * g.width + col]; if a == 0 { continue; }
                    let (x, y) = (gx + col as i64, gy + row as i64);
                    if x < 0 || y < 0 || x >= p.width as i64 || y >= p.height as i64 { continue; }
                    let dst = &mut out[(p.y + y as usize) * p.out_width + p.x + x as usize];
                    let ink = match party { Some(c) if *dst & 0xffffff != c & 0xffffff => c, _ => pipeline::text::readable_ink(*dst, dimmed) };
                    *dst = if a == 255 { ink } else { blend(*dst, ink, a) };
                } }
            }
        }
    }
    out
}
fn blend(dst: u32, src: u32, a: u8) -> u32 {
    let a = a as u32;
    [0, 8, 16].into_iter().map(|s| ((((dst >> s) & 255) * (255 - a) + ((src >> s) & 255) * a + 127) / 255) << s).sum()
}
fn run(out: &mut Surface, text: &str, at: (i32, i32), colour: u8) {
    let colour = if colour == DIM { DIM | pipeline::text::DIMMED } else { colour };
    let mut x = at.0;
    for c in text.bytes().take(1024) {
        if x as i64 + CELL as i64 > out.w as i64 { break; }
        let background: Vec<u8> = (0..8).flat_map(|dy| (0..CELL).map(move |dx| (dx, dy))).map(|(dx, dy)| out.get(x + dx, at.1 + dy).unwrap_or(0)).collect();
        let run = mt_engine::present::textlog::Run { page: out.text.page, x: x as i64, y: at.1 as i64, colour, shadow: None, bytes: vec![c], font_id: 0, width: CELL as u8,
            descend: false, covered: false, background_width: CELL as u16, background_height: 8, bitmap: background.clone(), background };
        out.text.visible.push(run.clone()); out.text.runs.push(run); x += CELL;
    }
}
pub const LOG_ROWS: usize = 24;
pub type LogRow = (bool, Vec<(String, u8)>);
pub fn log_rows(scene: &Scene, ui: &UiState) -> Vec<LogRow> {
    let names: Vec<(String, u8)> = (0..5u8).filter(|&i| scene.party.mask() & (1 << i) != 0).map(|i| (scene.party.name(i), party_register(i))).filter(|(n, _)| n.len() >= 2).collect();
    let mut out = Vec::new();
    for message in &ui.components.log.messages {
        let text = match message {
            super::combat_log::Line::Text(t) => t.clone(),
            super::combat_log::Line::Glyphs(runs) => {
                let mut runs: Vec<_> = runs.iter().collect(); runs.sort_by_key(|r| (r.y, r.x));
                let (mut s, mut end) = (String::new(), None::<i64>);
                for r in runs { if end.is_some_and(|e| r.x > e + 3) { s.push(' '); } s.extend(r.bytes.first().and_then(|&b| pipeline::text::shown(b))); end = Some(r.x + r.width as i64); }
                s
            }
        };
        let bold = text.starts_with("Round ") || text == "Encounter!";
        for (i, row) in wrap_px(&text, WIDTH as i32 - 8).into_iter().enumerate() { out.push((bold && i == 0, name_spans(&row, &names))); }
    }
    out
}
fn name_spans(row: &str, names: &[(String, u8)]) -> Vec<(String, u8)> {
    let mut spans: Vec<(String, u8)> = vec![(String::new(), INK)];
    let chars: Vec<char> = row.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let hit = names.iter().find(|(n, _)| {
            let n: Vec<char> = n.chars().collect();
            chars[i..].starts_with(&n) && (i == 0 || !chars[i - 1].is_alphanumeric()) && chars.get(i + n.len()).is_none_or(|c| !c.is_alphanumeric())
        });
        match hit {
            Some((n, reg)) => { spans.push((n.clone(), *reg)); spans.push((String::new(), INK)); i += n.chars().count(); }
            None => { spans.last_mut().unwrap().0.push(chars[i]); i += 1; }
        }
    }
    spans.retain(|(s, _)| !s.is_empty()); spans
}
impl Plus {
    fn log_page(&self, out: &mut Surface, rows: &[LogRow], top: i32, avail_rows: usize, from_newest: usize) {
        if rows.is_empty() { run(out, "No log lines yet.", (4, top), DIM); return; }
        let max = max_scroll(rows.len(), avail_rows); let start = max - from_newest.min(max);
        let (mut y, mut avail) = (top, avail_rows);
        if start > 0 { run(out, "^ more above: wheel, PgUp, Home", (4, y), DIM); y += 9; avail -= 1; }
        let below = start + avail < rows.len(); if below { avail -= 1; }
        for (bold, spans) in rows.iter().skip(start).take(avail) {
            let mut x = 4;
            for (text, reg) in spans { run(out, text, (x, y), if *bold { reg | BOLD } else { *reg }); x += text.chars().count() as i32 * CELL; }
            y += 9;
        }
        if below { run(out, "v more below: wheel, PgDn, End", (4, y), DIM); }
    }
}
pub fn max_scroll(lines: usize, rows: usize) -> usize { if lines <= rows { 0 } else { lines - rows + 1 } }
impl Plus {
}
pub fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = vec![String::new()];
    for word in text.split(' ') {
        for (i, chunk) in word.chars().collect::<Vec<_>>().chunks(width.max(1)).enumerate() {
            if out.last().is_some_and(|l| !l.is_empty() && l.chars().count() + 1 + chunk.len() > width) { out.push(String::new()); }
            let line = out.last_mut().unwrap();
            if i == 0 && !line.is_empty() { line.push(' '); }
            line.extend(chunk);
        }
    }
    out
}
fn tab_at(at: (i32, i32)) -> Option<u8> { tab_rects().iter().position(|r| r.contains(at)).map(|i| ORDER[i]) }
fn kind_at(at: (i32, i32)) -> Option<u8> {
    KINDS.iter().position(|(_, x, w)| Rect::new(*x, 28, *w, 16).contains(at)).map(|i| i as u8)
}
fn member_at(scene: &Scene, at: (i32, i32)) -> Option<u8> { let mask = scene.party.mask(); member_rects(scene).iter().position(|r| r.contains(at)).filter(|&i| mask & (1 << i) != 0).map(|i| i as u8) }
pub fn character_text(scene: &Scene, assets: &Assets, member: u8) -> String {
    character_rows(scene, assets, member).into_iter().map(|r| match r {
        HelpRow::Blank => String::new(), HelpRow::Heading(h) => h, HelpRow::Title(t, _) => t, HelpRow::Prose(p) => p,
        HelpRow::Entry(k, v) => if k.ends_with(':') { format!("{k} {v}") } else { format!("{k}: {v}") },
    }).collect::<Vec<_>>().join("\n")
}
pub fn character_rows(scene: &Scene, assets: &Assets, member: u8) -> Vec<HelpRow> {
    let p = &scene.party;
    if p.mask() & (1 << member) == 0 { return vec![HelpRow::Prose("No Traveller in this slot on this screen.".into())]; }
    let b = |at: usize| p.0.get(at).copied().unwrap_or(0);
    let rec = 0x102 + 256 * (member as usize & 7);
    let u32_at = |at: usize| u32::from_le_bytes([b(at), b(at + 1), b(at + 2), b(at + 3)]);
    let upp: Vec<u8> = (0..6).map(|i| b(rec + 0x14 + i)).collect();
    let item = |slot: usize| -> Option<String> {
        let n = u16::from_le_bytes([b(rec + 0xCC + 2 * slot), b(rec + 0xCD + 2 * slot)]) as usize;
        if n == 0 || n > 256 { return None; }
        let o = 0xA82 + 5 * (n - 1);
        let def = assets.items.get(b(o) as usize);
        let name = def.map_or_else(|| format!("item {}", b(o)), |d| d.name.iter().take_while(|&&c| c != 0).map(|&c| c as char).collect());
        Some(if def.is_some_and(|d| d.r#type == 2) && (b(o + 1) | b(o + 2)) != 0 { format!("{name} ({} mags, {} rounds)", b(o + 1), b(o + 2)) } else { name })
    };
    let mut carried: Vec<(String, usize)> = Vec::new();
    for slot in 0..19 { if let Some(name) = item(slot) { match carried.iter_mut().find(|(n, _)| *n == name) { Some(e) => e.1 += 1, None => carried.push((name, 1)) } } }
    let carried: Vec<String> = carried.into_iter().map(|(n, k)| if k > 1 { format!("{n} x{k}") } else { n }).collect();
    let skills: Vec<String> = (0..80).filter(|&i| mt1::record::shown_skill(i as u8, b(rec + 0x29 + i)) != 0).map(|i| {
        use mt1::ground::rules::adapters::{SKILL_JOT, SKILL_LEADER, SKILL_RECON, SKILL_TACTICS};
        let name = assets.ds_string(0x1479, i).map(capitalised).unwrap_or_else(|| match i as u8 { SKILL_TACTICS => "Tactics".into(), SKILL_LEADER => "Leader".into(), SKILL_RECON => "Recon".into(), SKILL_JOT => "Jack-of-all-Trades".into(), _ => format!("Skill {i}") });
        format!("{name}-{}", b(rec + 0x29 + i))
    }).collect();
    let (career, rank) = (b(rec + 0x20) as usize, b(rec + 0x21) as usize);
    let service = assets.ds_string(0x1465, career).map(capitalised).map(|c| {
        let title = if career == 3 { None } else { assets.ds_u8(0x13F6 + rank * 5 + career).and_then(|t| assets.ds_string(0x1419, t as usize)).map(capitalised) };
        match title { Some(t) => format!("ex-{c} {t}"), None => format!("ex-{c}") }
    });
    let none = || "none".to_string();
    let entry = |k: &str, v: String| HelpRow::Entry(format!("{k}:"), v);
    let mut rows = vec![HelpRow::Title(p.name(member), party_register(member))];
    if let Some(service) = service { rows.push(HelpRow::Prose(service)); }
    rows.extend([HelpRow::Blank, HelpRow::Heading("Attributes".into()),
        entry("Strength", upp[0].to_string()), entry("Dexterity", upp[1].to_string()), entry("Endurance", upp[2].to_string()),
        entry("Intelligence", upp[3].to_string()), entry("Education", upp[4].to_string()), entry("Social standing", upp[5].to_string()),
        HelpRow::Blank, HelpRow::Heading("Health".into()),
        entry("Hits", format!("{} of {}", p.hits(member), p.max_hits(member))), entry("Unconscious at", format!("{} hits", p.split(member))), entry("Oxygen", p.oxygen(member).to_string()),
        HelpRow::Blank, HelpRow::Heading("Money".into()),
        entry("Cash", format!("${}", u32_at(rec + 0xFC))), entry("Party fund", format!("${}", u32_at(0x88))),
        HelpRow::Blank, HelpRow::Heading("Hands".into()),
        entry("Primary", item(21).unwrap_or_else(none)), entry("Secondary", item(22).unwrap_or_else(none)),
        HelpRow::Blank, HelpRow::Heading("Worn".into()),
        entry("Suit", item(20).unwrap_or_else(none)), entry("Accessory", item(19).unwrap_or_else(none)),
        HelpRow::Blank, HelpRow::Heading("Skills".into())]);
    if skills.is_empty() { rows.push(HelpRow::Prose(none())); }
    for s in &skills { let (name, level) = s.rsplit_once('-').unwrap_or((s, "")); rows.push(entry(name, level.into())); }
    rows.push(HelpRow::Blank); rows.push(HelpRow::Heading(format!("Carried ({} of 19 slots)", (0..19).filter(|&s| item(s).is_some()).count())));
    if carried.is_empty() { rows.push(HelpRow::Prose("nothing".into())); }
    for c in carried { let (name, count) = c.rsplit_once(" x").filter(|(_, n)| n.chars().all(|d| d.is_ascii_digit())).unwrap_or((&c, "1")); rows.push(entry(name, format!("x{count}"))); }
    rows
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Sheet { pub id: String, pub title: String, pub open_at: String, pub scenario: Option<String>, pub setup: String, pub actions: Vec<String>, pub evaluate: Vec<String>, pub file: String }
pub fn parse_sheet(text: &str, file: &str) -> Sheet {
    let mut s = Sheet { file: file.into(), ..Default::default() };
    let mut section = "";
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("# ") { let (id, title) = rest.split_once(' ').unwrap_or((rest, "")); s.id = id.into(); s.title = title.trim().into(); }
        else if let Some(v) = t.strip_prefix("OPEN AT:") { s.open_at = v.trim().into(); section = ""; }
        else if let Some(v) = t.strip_prefix("SCENARIO:") { s.scenario = Some(v.trim().into()).filter(|v: &String| !v.is_empty()); section = ""; }
        else if let Some(v) = t.strip_prefix("SETUP KEYS:") { s.setup = v.trim().into(); section = ""; }
        else if t == "ACTIONS:" { section = "do"; } else if t == "EVALUATE:" { section = "see"; }
        else if section == "do" && line.starts_with(' ') { if let Some((_, v)) = t.split_once(". ") { s.actions.push(v.trim().into()); } }
        else if section == "see" && line.starts_with(' ') { if let Some(v) = t.strip_prefix("- ") { s.evaluate.push(v.trim().into()); } }
    }
    s
}
pub fn sheets() -> &'static [Sheet] {
    static SHEETS: std::sync::OnceLock<Vec<Sheet>> = std::sync::OnceLock::new();
    SHEETS.get_or_init(|| {
        let root = crate::plus::root();
        #[allow(unused_mut)] let mut dirs = vec![root.join("data/tests/atoms"), root.join("tests/atoms")];
        #[cfg(any(test, debug_assertions))] dirs.push(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/atoms"));
        let dir = dirs.into_iter().find(|d| d.is_dir());
        let mut files: Vec<_> = dir.and_then(|d| std::fs::read_dir(d).ok()).into_iter().flatten().flatten().map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "md")).collect();
        files.sort();
        files.into_iter().filter_map(|p| Some(parse_sheet(&std::fs::read_to_string(&p).ok()?, p.file_name()?.to_str()?))).collect()
    })
}
pub fn sheet_for_scenario(stem: &str) -> Option<usize> { sheets().iter().position(|s| s.scenario.as_deref() == Some(stem)) }
pub const TEST_ROWS: usize = 22;
pub fn test_rows(sheet: Option<&Sheet>, index: usize, count: usize, note: &str) -> Vec<HelpRow> {
    let Some(s) = sheet else { return vec![HelpRow::Prose(if count == 0 { "No test sheets found (data/tests/atoms or tests/atoms)." } else { "No such sheet." }.into())] };
    let mut rows = vec![HelpRow::Heading(format!("{} {}  ({} of {})", s.id, s.title, index + 1, count)), HelpRow::Blank,
        HelpRow::Entry("Open at".into(), s.open_at.clone())];
    if let Some(sc) = &s.scenario { rows.push(HelpRow::Entry("Scenario".into(), sc.clone())); }
    if !s.setup.is_empty() { rows.push(HelpRow::Entry("Setup".into(), s.setup.clone())); }
    rows.push(HelpRow::Blank); rows.push(HelpRow::Heading("Do".into()));
    for (i, a) in s.actions.iter().enumerate() { rows.push(HelpRow::Entry(format!("{}.", i + 1), a.clone())); }
    rows.push(HelpRow::Blank); rows.push(HelpRow::Heading("See".into()));
    for e in &s.evaluate { rows.push(HelpRow::Entry("[ ]".into(), e.clone())); }
    rows.push(HelpRow::Blank); rows.push(HelpRow::Heading("Note for FAIL (type it, then click FAIL)".into()));
    rows.push(HelpRow::Prose(format!("{note}_")));
    rows
}
fn date(secs: u64) -> String {
    let z = (secs / 86400) as i64 + 719468; let era = z.div_euclid(146097); let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153; let d = doy - (153 * mp + 2) / 5 + 1; let m = if mp < 10 { mp + 3 } else { mp - 9 };
    format!("{:04}-{:02}-{:02} {:02}:{:02}", yoe + era * 400 + i64::from(m <= 2), m, d, secs / 3600 % 24, secs / 60 % 60)
}
pub fn record(root: &std::path::Path, sheet: &Sheet, pass: bool, note: &str, build: &str, now: u64) -> std::io::Result<String> {
    use std::io::Write;
    let dir = root.join("data/tests"); std::fs::create_dir_all(&dir)?;
    let note = crate::plus::one_line(note).replace('|', "/");
    let scenario = sheet.scenario.as_deref().unwrap_or("-");
    let append = |name: &str, header: &str, row: String| -> std::io::Result<()> {
        let path = dir.join(name); let new = !path.exists();
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(&path)?;
        if new { writeln!(f, "{header}")?; }
        writeln!(f, "{row}")
    };
    append("ATOMS.md", "# Test verdicts from the game's Test tab

| sheet | verdict | when | build | scenario | note |
|---|---|---|---|---|---|",
        format!("| {} | {} | {} | {} | {} | {} |", sheet.id, if pass { "PASS" } else { "FAIL" }, date(now), build, scenario, note))?;
    if !pass { append("FAILED.md", "# Findings from the game's Test tab

| when | sheet | finding | scenario | build |
|---|---|---|---|---|",
        format!("| {} | {} {} | {} | {} | {} |", date(now), sheet.id, sheet.title, if note.is_empty() { "(no note)" } else { &note }, scenario, build))?; }
    Ok(format!("Recorded {} for {} in data/tests/ATOMS.md{}", if pass { "PASS" } else { "FAIL" }, sheet.id, if pass { "" } else { " and FAILED.md" }))
}
fn capitalised(s: String) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
static SCENARIO_ERROR: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub fn scenario_error_set(error: Option<String>) { if let Ok(mut e) = SCENARIO_ERROR.lock() { *e = error.filter(|e| !e.trim().is_empty()); } }
pub fn scenario_error() -> Option<String> { SCENARIO_ERROR.lock().ok().and_then(|e| e.clone()) }
const ERROR_INK: u8 = 12;
fn bug_offset() -> i32 { if scenario_error().is_some() { 9 } else { 0 } }
fn shifted(r: Rect, dy: i32) -> Rect { Rect::new(r.x, r.y + dy, r.w, r.h) }
pub const MAP_TEXT: &str = "The area map shows on the ground: every square ever inside the view, the rest dark.";
static MAP_SEEN: std::sync::Mutex<Option<mt1::play::diagnostic::Map>> = std::sync::Mutex::new(None);
pub fn map_seen_set(map: Option<mt1::play::diagnostic::Map>) { if let Ok(mut m) = MAP_SEEN.lock() { *m = map; } }
pub fn map_seen() -> Option<mt1::play::diagnostic::Map> { MAP_SEEN.lock().ok().and_then(|m| m.clone()) }
pub const MAP_BODY: Rect = Rect::new(4, 26, 312, 220);
pub fn map_cell_size(rows: u16, cols: u16) -> i32 { ((MAP_BODY.w / cols.max(1) as u32).min(MAP_BODY.h / rows.max(1) as u32)).max(1) as i32 }
impl Plus {
fn draw_map(&self, scene: &Scene, out: &mut Surface) {
    let m = &scene.map;
    let seen = map_seen().filter(|s| s.rows == m.rows && s.cols as u32 > 0);
    if m.rows == 0 || m.cols == 0 { self.rows(out, MAP_TEXT, MAP_BODY.y, 4, 0, DIM, WIDTH as i32 - 8, false); return; }
    let cs = map_cell_size(m.rows, m.cols);
    let (ox, oy) = (MAP_BODY.x + (MAP_BODY.w as i32 - cs * m.cols as i32) / 2, MAP_BODY.y + (MAP_BODY.h as i32 - cs * m.rows as i32) / 2);
    let bits = seen.as_ref().map(|s| (s.cols as usize, &s.seen));
    let is_seen = |r: usize, c: usize| bits.is_some_and(|(cols, b)| { let i = r * cols + c; b.get(i / 8).is_some_and(|byte| byte & (1 << (i % 8)) != 0) });
    for r in 0..m.rows as usize { for c in 0..m.cols as usize {
        let (x0, y0) = (ox + c as i32 * cs, oy + r as i32 * cs);
        if !is_seen(r, c) { Self::fill(out, Rect::new(x0, y0, cs as u32, cs as u32), INK); continue; }
        let tile = m.cell(r as i32, c as i32).map(|[f, tile, _]| (f, tile));
        let bitmap = tile.and_then(|(f, _)| if f & 1 != 0 { self.assets.terrain.get(m.tileset as usize) } else { self.assets.objects.get(4) });
        for dy in 0..cs { for dx in 0..cs {
            let colour = match (bitmap, tile) { (Some(b), Some((_, t))) => b.get(t as i32 % 20 * 16 + dx * 16 / cs, t as i32 / 20 * 16 + dy * 16 / cs).unwrap_or(DIM), _ => DIM };
            out.put(x0 + dx, y0 + dy, colour);
        } }
    } }
    let dot = |out: &mut Surface, (row, col): (f32, f32), colour: u8, size: i32| {
        let (cx, cy) = (ox + (col * cs as f32) as i32 + cs / 2, oy + (row * cs as f32) as i32 + cs / 2);
        let h = size / 2; for y in -h..=h { for x in -h..=h { out.put(cx + x, cy + y, colour); } }
    };
    for (i, n) in scene.npcs.iter().enumerate().take(32) {
        if !n.present() { continue; }
        if let Some(p) = super::controls::position(scene, mt1::ui::Unit::Npc(i as u8 + 1)) {
            if is_seen(p.0.max(0.0) as usize, p.1.max(0.0) as usize) { dot(out, p, super::minimap::attitude(n.attitude()), (cs / 2).max(1)); }
        }
    }
    if let Some(p) = super::controls::position(scene, mt1::ui::Unit::Traveller(scene.leader.clamp(0, 4) as u8)) { dot(out, p, party_register(scene.leader.clamp(0, 4) as u8), cs.max(3)); }
    if bits.is_none_or(|(_, b)| b.is_empty()) { self.rows(out, "Nothing seen yet in this area.", MAP_BODY.y, 1, 0, DIM, WIDTH as i32 - 8, false); }
}
}
pub fn scroll_lines(scene: &Scene, ui: &UiState, assets: &Assets) -> (Vec<(i32, String, u8)>, usize) {
    let plain = |text: String| wrap_px(&text, WIDTH as i32 - 8).into_iter().map(|l| (4, l, INK)).collect();
    match main_tab(ui.plus.tab) {
        HELP => (help_lines(&player_help(scene, ui.decision.as_ref())), HELP_ROWS),
        JOURNAL => (plain(ui.plus.journal.text()), JOURNAL_ROWS),
        LOG => (log_rows(scene, ui).into_iter().map(|(_, spans)| (4, spans.into_iter().map(|(t, _)| t).collect::<String>(), INK)).collect(), LOG_ROWS),
        CHARACTERS => (help_lines(&character_rows(scene, assets, member(ui.plus.tab))), CHARACTER_ROWS),
        TEST => { let all = sheets(); let i = sheet_index(ui.plus.tab); (help_lines(&test_rows(all.get(i), i, all.len(), &ui.plus.status)), TEST_ROWS) }
        _ => (Vec::new(), 1),
    }
}
impl Component for Plus {
    fn visible(&self, ui: &UiState) -> bool { ui.plus.open }
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        out.regs = mt_engine::present::palette::DEFAULT_REGS;
        let ground = matches!(scene.screen, mt1::ui::ScreenKind::Ground | mt1::ui::ScreenKind::Picture | mt1::ui::ScreenKind::Record);
        if ground { out.regs[1..6].copy_from_slice(&ui.frame.regs[1..6]); }
        if ground && main_tab(ui.plus.tab) == MAP { out.regs = ui.frame.regs; for i in [INK, PANEL, DIM, FIELD] { out.regs[i as usize] = mt_engine::present::palette::DEFAULT_REGS[i as usize]; } }
        out.indexed.fill(PANEL);
        for x in 0..out.w as i32 { out.put(x, 20, INK); }
        let tabs = tab_rects();
        for (i, &t) in ORDER.iter().enumerate() {
            let r = tabs[i]; let active = main_tab(ui.plus.tab) == t; let name = NAMES[t as usize];
            let bg = if active { FIELD } else { DIM };
            Self::fill(out, r, bg); Self::frame(out, r, INK);
            let colour = if active { INK } else { FIELD };
            self.label(out, name, (centred(r, name), r.y + 4), colour, bg);
        }
        match main_tab(ui.plus.tab) {
            BUG => {
                let dy = bug_offset();
                if let Some(e) = scenario_error() { run(out, &crate::plus::one_line(&format!("Scenario could not load: {e}")).chars().take(70).collect::<String>(), (4, 27), ERROR_INK | BOLD); }
                for (i, (name, x, w)) in KINDS.iter().enumerate() { self.button(out, Rect::new(*x, 28 + dy, *w, 16), name, ui.plus.kind == i as u8); }
                let (draft, submit) = (ui.plus.draft.as_str(), "Save report");
                #[cfg(feature = "dev")]
                let (draft, submit) = if ui.plus.bridge.editing_note { (ui.plus.bridge.note.as_str(), "Add note") } else { (draft, submit) };
                self.field(out, shifted(BUG_BOX, dy), draft);
                self.button(out, shifted(SAVE, dy), submit, false);
                let status = if ui.plus.status.is_empty() { "Saved with the current screen, for this build." } else { ui.plus.status.as_str() };
                #[cfg(feature = "dev")]
                let status = if ui.plus.bridge.message.is_empty() { status } else { ui.plus.bridge.message.as_str() };
                self.rows(out, status, 192 + dy, 3, 0, if status.starts_with("Saved with") { DIM } else { INK }, WIDTH as i32 - 8, false);
                if status.starts_with("Saved to ") {
                    use crate::plus::bug::{ISSUES, SEND};
                    let used = wrap_px(status, WIDTH as i32 - 8).len().min(3) as i32;
                    let (a, b) = ISSUES.split_at(ISSUES.find("/MegaTraveller").map_or(0, |i| i + 1));
                    self.lines(out, &[SEND, a, b].map(|l| (4, l.to_string(), INK)), 192 + dy + used * 9, 3, 0, false);
                }
                #[cfg(feature = "dev")]
                if ui.plus.report.is_some() { ui.plus.bridge.draw(&self.assets, out); }
            }
            HELP => { let (lines, rows) = scroll_lines(scene, ui, &self.assets); self.lines(out, &lines, 26, rows, ui.plus.scroll, true); }
            JOURNAL => {
                self.field(out, NOTE_BOX, &ui.plus.note);
                self.button(out, ADD, "Add note", false);
                let (lines, rows) = scroll_lines(scene, ui, &self.assets); self.lines(out, &lines, 94, rows, ui.plus.scroll, true);
                self.rows(out, &ui.plus.status, 226, 2, 0, INK, WIDTH as i32 - 8, false);
            }
            MAP => self.draw_map(scene, out),
            LOG => self.log_page(out, &log_rows(scene, ui), 26, LOG_ROWS, ui.plus.scroll),
            TEST => {
                self.button(out, PREV, "< Prev", false); self.button(out, NEXT, "Next >", false);
                self.boxed(out, PASS, "PASS", PASS_FILL, INK, false); self.boxed(out, FAIL, "FAIL", FAIL_FILL, FIELD, false);
                let (lines, rows) = scroll_lines(scene, ui, &self.assets); self.lines(out, &lines, 50, rows, ui.plus.scroll, true);
            }
            _ => {
                let names = member_names(scene);
                for (i, r) in member_rects(scene).iter().enumerate() {
                    if names[i].is_empty() { continue; }
                    self.member_tab(out, *r, &names[i], party_register(i as u8), member(ui.plus.tab) == i as u8);
                }
                let (lines, rows) = scroll_lines(scene, ui, &self.assets); self.lines(out, &lines, 50, rows, ui.plus.scroll, true);
            }
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        let action = match *e {
            UiEvent::Key(keys::ESC) => A::Close,
            UiEvent::Key(keys::TAB) => A::Tab(next_tab(ui.plus.tab, false)),
            UiEvent::Key(keys::PGUP) => A::Scroll(-10), UiEvent::Key(keys::PGDN) => A::Scroll(10),
            UiEvent::Key(keys::UP) => A::Scroll(-1), UiEvent::Key(keys::DOWN) => A::Scroll(1),
            UiEvent::Key(keys::ENTER) => A::Submit, UiEvent::Key(8) => A::Backspace,
            UiEvent::Key(k @ 32..=255) => A::Text(char::from(k as u8)),
            UiEvent::Wheel(n) => A::Scroll(n.saturating_neg()),
            UiEvent::Up(Button::Left) => match (tab_at(at), main_tab(ui.plus.tab)) {
                (Some(tab), _) => A::Tab(tab),
                (None, BUG) if shifted(SAVE, bug_offset()).contains(at) => A::Submit,
                (None, BUG) => match kind_at((at.0, at.1 - bug_offset())) { Some(kind) => A::Kind(kind), None => return Vec::new() },
                (None, JOURNAL) if ADD.contains(at) => A::Submit,
                (None, CHARACTERS) => match member_at(scene, at) { Some(m) => A::Tab(CHARACTERS + m), None => return Vec::new() },
                (None, TEST) if PASS.contains(at) => A::Kind(0),
                (None, TEST) if FAIL.contains(at) => A::Kind(1),
                (None, TEST) if PREV.contains(at) || NEXT.contains(at) => {
                    let n = sheets().len().max(1); let i = sheet_index(ui.plus.tab);
                    A::Tab(TEST + ((i + if PREV.contains(at) { n - 1 } else { 1 }) % n).min(LAST_TAB as usize - TEST as usize) as u8)
                }
                _ => return Vec::new(),
            },
            _ => return Vec::new(),
        };
        vec![Intent::Plus(action)]
    }
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    use super::super::Layout;
    static BUG_GEOMETRY: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn bug_lock() -> std::sync::MutexGuard<'static, ()> { BUG_GEOMETRY.lock().unwrap_or_else(|e| e.into_inner()) }
    #[test]
    fn click_targets_and_text_annotations_stay_inside_plus() {
        let _geometry = bug_lock();
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets.clone());
        let s = Scene::program(mt1::ui::ScreenKind::Ground); let mut ui = UiState::default();
        for (x, tab) in [(20, JOURNAL), (70, LOG), (115, CHARACTERS), (160, MAP), (205, TEST), (250, HELP), (295, BUG)] { assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (x, 10)), [Intent::Plus(A::Tab(tab))], "x {x}"); }
        assert_eq!(next_tab(BUG, false), JOURNAL); assert_eq!(next_tab(JOURNAL, true), BUG); assert_eq!(next_tab(CHARACTERS + 3, false), MAP); assert_eq!(next_tab(TEST + 40, false), HELP);
        ui.plus.tab = TEST + 1;
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (PASS.x + 5, 35)), [Intent::Plus(A::Kind(0))]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (FAIL.x + 5, 35)), [Intent::Plus(A::Kind(1))]);
        if sheets().len() > 2 { assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (NEXT.x + 5, 35)), [Intent::Plus(A::Tab(TEST + 2))]); assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (PREV.x + 5, 35)), [Intent::Plus(A::Tab(TEST))]); }
        ui.plus.tab = 0;
        assert_eq!((next_member(0b10101, 0, false), next_member(0b10101, 0, true), next_member(0, 2, false)), (2, 4, 2));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (100, 35)), [Intent::Plus(A::Kind(1))]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 178)), [Intent::Plus(A::Submit)]);
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (100, 100)).is_empty(), "the text box itself is not a button");
        ui.plus.tab = 2; assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 80)), [Intent::Plus(A::Submit)]);
        assert!(c.input(&s, &ui, &UiEvent::Up(Button::Right), (20, 80)).is_empty());
        ui.plus.tab = CHARACTERS; assert!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 35)).is_empty(), "no Traveller in the slot");
        let mut party = s.clone(); party.party = mt1::ui::scene::Party(vec![0; 0xF86]); party.party.0[0] = 0b101;
        let members = member_rects(&party);
        assert_eq!(c.input(&party, &ui, &UiEvent::Up(Button::Left), (members[2].x + 5, 35)), [Intent::Plus(A::Tab(CHARACTERS + 2))]);
        assert!(c.input(&party, &ui, &UiEvent::Up(Button::Left), (members[1].x + 5, 35)).is_empty(), "slot 1 is empty");
        let mut base = Surface::new(320, 250); assets.text(&mut base, "old", (20, 30), 15);
        let mut overlay = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut overlay); base.cover(&overlay, 0, 0);
        assert!(!base.text.visible.iter().any(|r| r.x == 20 && r.y == 30));
    }
    #[test]
    fn draft_wraps_keeping_every_character_and_no_retro_prompts_remain() {
        let _geometry = bug_lock();
        assert_eq!(wrap("", 10), [""]);
        assert_eq!(wrap("a *b* #c", 10), ["a *b* #c"]);
        assert_eq!(wrap("one two three", 7), ["one two", "three"]);
        assert_eq!(wrap("abcdefghijkl", 5), ["abcde", "fghij", "kl"]);
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets);
        let s = Scene::program(mt1::ui::ScreenKind::Ground); let mut ui = UiState::default(); ui.plus.open = true;
        for tab in [BUG, HELP, JOURNAL, MAP, CHARACTERS, TEST, LOG] {
            ui.plus.tab = tab; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
            let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
            for retro in ["paused", "Left/Right", "Type here", "[Save", "Esc close"] { assert!(!text.contains(retro), "tab {tab}: {retro}"); }
            assert!(text.contains("Bug") && text.contains("Help") && text.contains("Journal") && text.contains("Map") && text.contains("Characters") && text.contains("Test"));
            if tab == TEST { assert!(text.contains("PASS") && text.contains("FAIL") && text.contains("Prev")); }
            if tab == 0 { assert!(text.contains("QoL request") && text.contains("Question") && text.contains("Save report")); }
            if tab == MAP { assert!(text.contains("area map")); }
            if tab == CHARACTERS { assert!(text.contains("No Traveller")); }
        }
    }
    #[test] #[ignore]
    fn plus_characters_proof_png() {
        let _geometry = bug_lock();
        use crate::panels::render::Metrics;
        let Some(path) = std::env::var_os("MT_PLUS_PNG") else { return };
        let Some(dir) = oracle_env::gamefiles() else { return };
        let assets = Arc::new(Assets::load(&mut mt_engine::io::Dir(std::path::PathBuf::from(dir)))); let c = Plus::new(assets.clone());
        let s = Scene::program(mt1::ui::ScreenKind::Ground); let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = CHARACTERS;
        let look = Look { shape: crate::look::Shape::Square, font: crate::font_menu::plus_font(), ..Look::default() }; let size = [WIDTH * 3, HEIGHT * 3];
        let mut party = s.clone(); party.party = mt1::ui::scene::Party(vec![0; 0xF86]); party.party.0[0] = 0b11111;
        for (i, name) in ["Magellan", "Jurgen", "Effane", "Dennar", "Verti"].iter().enumerate() { for (k, c) in name.bytes().enumerate() { party.party.0[0x102 + 256 * i + k] = c; } }
        let rec = 0x102; party.party.0[rec + 0x14..rec + 0x1A].copy_from_slice(&[6, 8, 11, 13, 11, 8]);
        party.party.0[rec + 0x23] = 21; party.party.0[rec + 0x24] = 6; party.party.0[rec + 0x25] = 21; party.party.0[rec + 0x26] = 45;
        party.party.0[rec + 0xFC..rec + 0x100].copy_from_slice(&36334u32.to_le_bytes()); party.party.0[0x88..0x8C].copy_from_slice(&198u32.to_le_bytes());
        ui.frame.regs[1..6].copy_from_slice(&[5, 4, 32, 62, 1]);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&party, &ui, &mut out);
        let m = Metrics::component(size, (WIDTH, HEIGHT), &look);
        let pixels = super::compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m, &look);
        std::fs::write(path, crate::plus::png::rgb(size[0], size[1], &pixels).unwrap()).unwrap();
    }
    #[test]
    fn label_ink_stays_inside_every_box_and_overflow_is_announced() {
        let _geometry = bug_lock();
        use crate::panels::render::Metrics;
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets.clone());
        let s = Scene::program(mt1::ui::ScreenKind::Ground); let mut ui = UiState::default(); ui.plus.open = true;
        ui.plus.draft = "Door at the top of the stairs sticks. Second line of the description here.".into(); ui.plus.kind = 1;
        let look = Look { shape: crate::look::Shape::Square, ..Look::default() }; let size = [WIDTH * 2, HEIGHT * 2];
        let look = Look { font: crate::font_menu::plus_font(), ..look };
        let mut party = s.clone(); party.party = mt1::ui::scene::Party(vec![0; 0xF86]); party.party.0[0] = 0b11011;
        for (i, name) in [(0usize, "Dennar"), (1, "Alaric"), (3, "Averylongtravellername"), (4, "V")] { for (k, c) in name.bytes().enumerate() { party.party.0[0x102 + 256 * i + k] = c; } }
        let tabs = &BOXES[..7];
        let with = |extra: &[Rect]| -> Vec<Rect> { tabs.iter().chain(extra).copied().collect() };
        for (tab, boxes) in [(BUG, &BOXES[..12]), (HELP, tabs), (LOG, tabs), (JOURNAL, &with(&[NOTE_BOX, ADD])[..]), (MAP, tabs),
            (CHARACTERS, &with(&{ let m = member_rects(&party); [m[0], m[1], m[3], m[4]] })[..]), (TEST, &with(&[PREV, NEXT, PASS, FAIL])[..])] {
            ui.plus.tab = tab; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&party, &ui, &mut out);
            let m = Metrics::component(size, (WIDTH, HEIGHT), &look);
            {
            let pixels = compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out.clone())], m, &look);
            let at = |x: usize, y: usize| pixels[y * m.pixels.out_width + x];
            let panel = at(m.pixels.out_width - 2, m.pixels.out_height - 2);
            for r in boxes {
                let p = m.rect(*r); let border = at(p.x, p.y);
                assert_ne!(border, panel, "tab {tab} box {r:?}: border drawn");
                for x in p.x..p.x + p.width { for y in [p.y, p.y + p.height - 1] { assert_eq!(at(x, y), border, "tab {tab} box {r:?}: ink on the top/bottom edge at ({x},{y})"); } }
                for y in p.y..p.y + p.height { for x in [p.x, p.x + p.width - 1] { assert_eq!(at(x, y), border, "tab {tab} box {r:?}: ink on a side edge at ({x},{y})"); } }
                let below = at(p.x, p.y + p.height + 1);
                for x in p.x..p.x + p.width { assert_eq!(at(x, p.y + p.height + 1), below, "tab {tab} box {r:?}: ink below the box at x={x}"); }
            }
            }
        }
        ui.plus.tab = 1; ui.plus.scroll = 0; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        assert!(text.contains("more below") && !text.contains("more above"), "help overflow is announced");
        ui.plus.scroll = 1000; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        assert!(text.contains("more above") && !text.contains("more below"), "the last page says more above only");
        let total = crate::plus::help::lines(&crate::plus::help::text(&s), COLS).len();
        assert!(total > HELP_ROWS && max_scroll(total, HELP_ROWS) == total - HELP_ROWS + 1 && max_scroll(3, HELP_ROWS) == 0);
        assert_eq!(c.input(&s, &ui, &UiEvent::Wheel(-1), (100, 100)), [Intent::Plus(A::Scroll(1))], "wheel scrolls");
    }
    #[test]
    fn plus_composes_proportionally_in_its_own_face() {
        let _geometry = bug_lock();
        let face = crate::font_menu::selected(&crate::font_menu::plus_font()).expect("a Plus face always resolves (builtin fallback)");
        let (narrow, wide) = (text_width("iiiiiiii").unwrap(), text_width("MMMMMMMM").unwrap());
        assert!(narrow > 0 && narrow <= wide && text_width("") == Some(0), "{narrow} {wide}");
        if face.family == "Segoe UI" { assert!(narrow * 2 < wide, "Segoe UI is proportional: {narrow} vs {wide}"); }
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets); let s = Scene::program(mt1::ui::ScreenKind::Ground);
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = 0; ui.plus.draft = "iiii".into();
        let look = Look { font: crate::font_menu::plus_font(), shape: crate::look::Shape::Square, ..Look::default() };
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert_eq!(lines(&out.text).iter().filter(|l| l.0 == "Save report").count(), 1, "one run per character becomes one line");
        let m = Metrics::component([WIDTH * 2, HEIGHT * 2], (WIDTH, HEIGHT), &look);
        let pixels = compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out.clone())], m, &look);
        assert_eq!(pixels.len(), (WIDTH * HEIGHT * 4) as usize);
        let dark = |r: Rect| { let p = m.rect(r); (p.y..p.y + p.height).flat_map(|y| (p.x..p.x + p.width).map(move |x| (x, y))).filter(|&(x, y)| pixels[y * m.pixels.out_width + x] & 0xffffff < 0x404040).count() };
        let (sl, sr) = (dark(Rect::new(SAVE.x + 1, SAVE.y + 1, SAVE.w / 2 - 1, SAVE.h - 2)), dark(Rect::new(SAVE.x + SAVE.w as i32 / 2, SAVE.y + 1, SAVE.w / 2 - 1, SAVE.h - 2)));
        assert!(sl > 0 && sr > 0 && (sl as f32 / sr as f32 - 1.0).abs() < 0.6, "centred label ink {sl} {sr}");
        assert!(dark(Rect::new(BUG_BOX.x + 2, BUG_BOX.y + 2, 60, 12)) > 0 && dark(Rect::new(BUG_BOX.x + 160, BUG_BOX.y + 2, 100, 12)) == 0, "draft laid from the left");
        ui.plus.tab = 1; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let pixels = compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m, &look);
        let dark = |r: Rect| { let p = m.rect(r); (p.y..p.y + p.height).flat_map(|y| (p.x..p.x + p.width).map(move |x| (x, y))).filter(|&(x, y)| pixels[y * m.pixels.out_width + x] & 0xffffff < 0x404040).count() };
        for row in 0..HELP_ROWS as i32 { let y = 26 + row * 9; if dark(Rect::new(0, y, 320, 9)) > 0 { assert!(dark(Rect::new(4, y, 12, 9)) > 0 || dark(Rect::new(KEY_X, y, 12, 9)) > 0, "help row {row} starts in a column"); } }
        let mut party = s.clone(); party.party = mt1::ui::scene::Party(vec![0; 0xF86]); party.party.0[0] = 0b11;
        for (k, c) in b"Dennar".iter().enumerate() { party.party.0[0x102 + 256 + k] = *c; }
        ui.plus.tab = CHARACTERS + 1; ui.frame.regs[1..6].copy_from_slice(&[5, 4, 32, 62, 1]);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&party, &ui, &mut out);
        assert_eq!(out.regs[2], 4, "the strip's registers are borrowed from the frame");
        assert!(lines(&out.text).iter().any(|l| l.0 == "Dennar" && l.2 && l.3 == 2), "the name row is bold in register 2: {:?}", lines(&out.text));
        let m2 = m; let pixels = compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m2, &look);
        let red = |x: usize, y: usize| { let v = pixels[y * m2.pixels.out_width + x]; (v >> 16) & 255 > 128 && (v >> 8) & 255 < 80 && v & 255 < 80 };
        let members = member_rects(&party);
        let p1 = m2.rect(members[1]); assert!(red(p1.x + 6, p1.y + 6), "Dennar's tab is red (inside the double frame)");
        let p0 = m2.rect(members[0]); assert!(!red(p0.x + 6, p0.y + 6));
        let title = m2.rect(Rect::new(4, 50, 60, 9));
        assert!((title.y..title.y + title.height).flat_map(|y| (title.x..title.x + title.width).map(move |x| (x, y))).any(|(x, y)| red(x, y)), "the name is laid in red ink");
        let original = Look { font: crate::look::FontChoice::Original, ..look.clone() };
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert_eq!(compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out.clone())], m, &original), super::super::render::compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m, &original));
    }
    #[test]
    fn character_text_reads_the_record() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground);
        assert!(character_text(&s, &Assets::default(), 0).starts_with("No Traveller"));
        let mut p = vec![0u8; 0xF86]; p[0] = 0b11; let rec = 0x102 + 256;
        for (k, c) in b"Dennar".iter().enumerate() { p[rec + k] = *c; }
        p[rec + 0x14..rec + 0x1A].copy_from_slice(&[7, 10, 11, 8, 12, 5]);
        p[rec + 0x23] = 20; p[rec + 0x24] = 15; p[rec + 0x25] = 12; p[rec + 0x29 + 66] = 2; p[rec + 0x29 + 12] = 1;
        p[rec + 0xFC..rec + 0x100].copy_from_slice(&111167u32.to_le_bytes()); p[0x88..0x8C].copy_from_slice(&500u32.to_le_bytes());
        for (n, bytes) in [(1, [2, 3, 20, 0, 0]), (2, [1, 0, 0, 0, 0]), (3, [1, 0, 0, 0, 0]), (4, [2, 3, 20, 0, 0]), (5, [7, 0, 0, 0, 0])] { p[0xA82 + 5 * (n - 1)..0xA82 + 5 * n].copy_from_slice(&bytes); }
        for (slot, n) in [(21usize, 1u16), (0, 2), (3, 3), (5, 4), (20, 5)] { p[rec + 0xCC + 2 * slot..rec + 0xCE + 2 * slot].copy_from_slice(&n.to_le_bytes()); }
        s.party = mt1::ui::scene::Party(p);
        let mut assets = Assets::default();
        for name in ["", "Medkit", "Auto pistol", "", "", "", "", "Cloth armour"] {
            let mut it = mt_engine::data::itm::Item { name: [0; 29], r#type: 0, weight: 0, price: 0, f25: 0, icon: 0, ammo_flags: 0, charges: 0, f29: 0, f2a: 0, tech_level: 0, law_level: 0, f2d: 0 };
            for (k, c) in name.bytes().enumerate() { it.name[k] = c; } if name == "Auto pistol" { it.r#type = 2; } assets.items.push(it);
        }
        let text = character_text(&s, &assets, 1);
        for want in ["Dennar", "Strength: 7", "Dexterity: 10", "Endurance: 11", "Intelligence: 8", "Education: 12", "Social standing: 5", "Hits: 12 of 20", "Unconscious at: 15 hits", "Cash: $111167", "Party fund: $500",
            "Primary: Auto pistol (3 mags, 20 rounds)", "Secondary: none", "Suit: Cloth armour", "Accessory: none", "Skill 12: 1", "Tactics: 2", "Carried (3 of 19 slots)", "Medkit: x2", "Auto pistol (3 mags, 20 rounds): x1"] {
            assert!(text.contains(want), "{want:?} in {text:?}");
        }
        let rows = character_rows(&s, &assets, 1);
        assert_eq!(rows[0], HelpRow::Title("Dennar".into(), 2), "slot 1 wears register 2 (red on the strip)");
        assert!(!matches!(rows[1], HelpRow::Prose(_)), "no career line without the game files");
        if let Some(dir) = std::env::var_os("MT1_GAMEFILES") {
            let real = Assets::load(&mut mt_engine::io::Dir(std::path::PathBuf::from(dir)));
            let text = character_text(&s, &real, 1);
            assert!(text.contains("Tactics: 2") && !text.contains("Skill 12"), "{text}");
            let named = capitalised(real.ds_string(0x1479, 12).unwrap()); assert!(text.contains(&format!("{named}: 1")), "{text}");
            let rows = character_rows(&s, &real, 1);
            assert!(matches!(&rows[1], HelpRow::Prose(l) if l.starts_with("ex-")), "{:?}", rows[1]);
        }
        assert_eq!(rows.iter().filter(|r| matches!(r, HelpRow::Heading(_))).count(), 7, "Attributes, Health, Money, Hands, Worn, Skills, Carried");
        assert_eq!(party_register(0), 1); assert_eq!(party_register(4), 5);
        let (scroll, rows) = scroll_lines(&s, &{ let mut ui = UiState::default(); ui.plus.tab = CHARACTERS + 1; ui }, &assets);
        assert_eq!((scroll.len(), rows), (wrap_px(&text, WIDTH as i32 - 8).len(), CHARACTER_ROWS));
        for l in wrap_px(&text, WIDTH as i32 - 8) { assert!(text_width(&l).unwrap() <= WIDTH as i32 - 8 && l.chars().count() <= ((WIDTH as i32 - 8) / CELL) as usize, "{l:?}"); }
        let narrow = "i ".repeat(60); assert!(wrap_px(&narrow, 300)[0].chars().count() > 52 || text_width("i").is_none());
        assert_eq!(wrap_px("", 100), [""]);
    }
    #[test]
    fn help_reads_like_a_manual_page() {
        use mt_engine::play::{Decision, Kind, Option as Opt};
        let s = Scene::program(mt1::ui::ScreenKind::Ground);
        let rows = player_help(&s, None);
        assert_eq!(rows[0], HelpRow::Heading("Exploring on foot".into()), "the current screen's section first");
        assert_eq!(rows[1], HelpRow::Entry("Arrows / keypad".into(), "Walk, eight ways (PgUp NE, PgDn SE, End SW, keypad 7 NW)".into()));
        let text = format!("{rows:?}");
        for forbidden in ["crates/", ".rs:", "-> ", "DS:", "oracle", "lane", "M8"] { assert!(!text.contains(forbidden), "no source references: {forbidden}"); }
        let headings: Vec<&str> = rows.iter().filter_map(|r| if let HelpRow::Heading(h) = r { Some(h.as_str()) } else { None }).collect();
        assert!(headings.contains(&"A fight") && headings.contains(&"A fight: mouse") && headings.contains(&"The Plus window") && headings.contains(&"In space: mouse"), "{headings:?}");
        assert!(rows.iter().filter(|r| matches!(r, HelpRow::Entry(k, _) if k.len() <= KEY_COLS)).count() >= 80, "one row per key, keys fit the column");
        assert!(rows.iter().all(|r| !matches!(r, HelpRow::Entry(_, v) if v.contains('\n'))), "one line per key");
        let mut fight = s.clone(); fight.squad.active = true; fight.squad.encounter = true;
        assert_eq!(player_help(&fight, None)[0], HelpRow::Heading("A fight".into()));
        let mut port = s.clone(); port.context.event = 9;
        assert_eq!(player_help(&port, None)[0], HelpRow::Heading("Starport, shops and offices".into()));
        assert_eq!(player_help(&Scene::program(mt1::ui::ScreenKind::Space), None)[0], HelpRow::Heading("In space".into()));
        let mut d = Decision::new(Kind::Ground, "t", "6923"); d.options.push(Opt::new("cmd:game", "Game", vec![]));
        let mut off = Opt::new("no", "Unavailable action", vec![]); off.enabled = false; d.options.push(off);
        let with = player_help(&s, Some(&d));
        assert_eq!((&with[0], &with[1]), (&HelpRow::Heading("On this screen now".into()), &HelpRow::Prose("Game".into())));
        assert!(!format!("{with:?}").contains("Unavailable"));
        let page = help_lines(&rows);
        assert_eq!(page[0], (4, "Exploring on foot".into(), INK | BOLD));
        assert!(page[1].1.starts_with("Arrows / keypad\nWalk, eight ways") && page[1].0 == 4, "{:?}", page[1]);
        assert!(page.iter().filter(|l| l.1.contains('\n')).all(|l| l.1.split_once('\n').unwrap().0.len() <= KEY_COLS), "the key fits its column");
        assert!(page.iter().any(|l| l.0 == KEY_X), "a wrapped meaning continues in its column");
        assert!(!page.windows(2).any(|w| w[0].1.is_empty() && w[1].1.is_empty()), "never two blank rows");
        let wide = wrap_px("WWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWWW", 300);
        assert!(wide.len() >= 2 && wide.iter().all(|l| text_width(l).unwrap_or(0) <= 300 && l.len() <= 75), "{wide:?}");
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets.clone());
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = HELP;
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let laid = lines(&out.text);
        assert!(laid.iter().any(|l| l.0 == "Exploring on foot" && l.2), "bold heading");
        assert!(laid.iter().filter(|l| l.0 == "\u{2014}" && !l.2 && l.4 == DASH_X as i64).count() >= 10 && laid.iter().filter(|l| l.4 == KEY_X as i64).count() >= 10, "dashes in their column, meanings in theirs: {laid:?}");
        assert!(laid.iter().any(|l| l.0 == "[ / ]"), "brackets are shown: {:?}", laid.iter().map(|l| l.0.as_str()).filter(|t| t.contains('/')).collect::<Vec<_>>());
        assert!(rows.iter().all(|r| !matches!(r, HelpRow::Entry(k, _) if k.len() > KEY_COLS)), "every key fits the column uncut");
        let (scroll, rows) = scroll_lines(&s, &ui, &assets); assert_eq!((scroll.len() > HELP_ROWS, rows), (true, HELP_ROWS));
    }
    #[test]
    fn test_sheets_parse_and_verdicts_are_recorded() {
        let s = parse_sheet("# T058 Buy one lot of cargo\n\nOPEN AT: The Efate starport screen\nSCENARIO: m5_starport_desk_efate\nSETUP KEYS: snapshot x.json\nACTIONS:\n  1. Press Down\n  2. Press Enter\nEVALUATE:\n  - Who will pay lists the five\n  - Cargo Available lists\nOracle: notes here\n", "058_buy.md");
        assert_eq!((s.id.as_str(), s.title.as_str(), s.open_at.as_str(), s.scenario.as_deref(), s.setup.as_str()), ("T058", "Buy one lot of cargo", "The Efate starport screen", Some("m5_starport_desk_efate"), "snapshot x.json"));
        assert_eq!(s.actions, ["Press Down", "Press Enter"]); assert_eq!(s.evaluate, ["Who will pay lists the five", "Cargo Available lists"]);
        let rows = test_rows(Some(&s), 3, 10, "door sticks");
        assert_eq!(rows[0], HelpRow::Heading("T058 Buy one lot of cargo  (4 of 10)".into()));
        assert!(rows.contains(&HelpRow::Entry("1.".into(), "Press Down".into())) && rows.contains(&HelpRow::Entry("[ ]".into(), "Cargo Available lists".into())));
        assert_eq!(rows.last(), Some(&HelpRow::Prose("door sticks_".into())));
        assert!(matches!(test_rows(None, 0, 0, "").as_slice(), [HelpRow::Prose(p)] if p.starts_with("No test sheets")));
        let all = sheets();
        assert!(all.len() >= 80 && all.iter().all(|s| s.id.starts_with('T') && !s.title.is_empty()), "{} sheets", all.len());
        assert!(all.iter().filter(|s| s.scenario.is_some()).count() >= 48, "the sheets that name their scenario");
        assert_eq!(sheet_for_scenario("efate_quiet").map(|i| all[i].id.as_str()), Some("T002")); assert_eq!(sheet_for_scenario("no_such"), None);
        assert_eq!(date(0), "1970-01-01 00:00"); assert_eq!(date(1_762_700_000), "2025-11-09 14:53");
        let root = std::env::temp_dir().join(format!("mt-test-tab-{}", std::process::id())); let _ = std::fs::remove_dir_all(&root);
        assert_eq!(record(&root, &s, true, "", "abc1234", 1_762_700_000).unwrap(), "Recorded PASS for T058 in data/tests/ATOMS.md");
        assert_eq!(record(&root, &s, false, "the | list is empty", "abc1234", 1_762_700_060).unwrap(), "Recorded FAIL for T058 in data/tests/ATOMS.md and FAILED.md");
        let atoms = std::fs::read_to_string(root.join("data/tests/ATOMS.md")).unwrap();
        assert!(atoms.starts_with("# Test verdicts") && atoms.contains("| T058 | PASS | 2025-11-09 14:53 | abc1234 | m5_starport_desk_efate |  |") && atoms.contains("| T058 | FAIL | 2025-11-09 14:54 | abc1234 | m5_starport_desk_efate | the / list is empty |"), "{atoms}");
        let findings = std::fs::read_to_string(root.join("data/tests/FAILED.md")).unwrap();
        assert!(findings.contains("| 2025-11-09 14:54 | T058 Buy one lot of cargo | the / list is empty | m5_starport_desk_efate | abc1234 |"), "{findings}");
        assert_eq!(findings.lines().count(), 5, "one header, one row");
        std::fs::remove_dir_all(&root).unwrap();
    }
    #[test]
    fn map_tab_shows_seen_squares_only() {
        use mt1::play::diagnostic::Map;
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets);
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground);
        let (rows, cols) = (12u16, 20u16);
        let mut cells = vec![0u8; rows as usize * cols as usize * 3]; for i in 0..rows as usize * cols as usize { cells[i * 3] = 1; cells[i * 3 + 1] = 3; }
        s.map = Arc::new(mt1::ui::scene::MapCells { rows, cols, stride: cols, cells, tileset: 0, interior: 0 });
        s.pos = (2 * 4, 2 * 6); s.leader = 1;
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = MAP;
        let mut seen = vec![0u8; (rows as usize * cols as usize).div_ceil(8)];
        for r in 0..rows as usize { for col in 0..10usize { let i = r * cols as usize + col; seen[i / 8] |= 1 << (i % 8); } }
        map_seen_set(Some(Map { area: [0, 0, 0], rows, cols, tiles: Vec::new(), party: (4, 6), seen }));
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let cs = map_cell_size(rows, cols); assert_eq!(cs, 15);
        let (ox, oy) = (MAP_BODY.x + (MAP_BODY.w as i32 - cs * cols as i32) / 2, MAP_BODY.y + (MAP_BODY.h as i32 - cs * rows as i32) / 2);
        let at = |out: &Surface, r: i32, col: i32| out.indexed[(oy + r * cs + cs / 2) as usize * out.w + (ox + col * cs + cs / 2) as usize];
        assert_eq!(at(&out, 0, 15), INK, "an unseen square is dark"); assert_eq!(at(&out, 11, 19), INK);
        assert_eq!(at(&out, 0, 0), DIM, "a seen square shows its tile (no bitmaps in the test assets: the placeholder colour)"); assert_eq!(at(&out, 11, 9), DIM);
        assert_eq!(at(&out, 4, 6), party_register(1), "the party square in the leader's colour");
        map_seen_set(None);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert_eq!(at(&out, 0, 0), INK, "without a seen set nothing shows");
        let text: String = out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
        assert!(text.contains("Nothing seen yet"));
    }
    #[test]
    fn a_saved_report_shows_where_to_send_it() {
        use crate::plus::bug::{ISSUES, SEND};
        let _geometry = bug_lock();
        let c = Plus::new(Arc::new(Assets::default())); let s = Scene::program(mt1::ui::ScreenKind::Ground);
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = BUG;
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert!(!lines(&out.text).iter().any(|l| l.0.contains("github")), "nothing to send before a report is saved");
        for path in [r"C:\Games\MT\acceptance\local\bugs\20261113-101500-mt1".to_string(), format!(r"C:\Home\tester\{}acceptance\local\bugs\20261113-101500-mt1", r"Long folder name\".repeat(8))] {
            ui.plus.status = format!("Saved to {path}");
            let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
            let laid = lines(&out.text);
            let at = laid.iter().position(|l| l.0 == SEND).unwrap_or_else(|| panic!("{laid:?}"));
            assert_eq!(format!("{}{}", laid[at + 1].0, laid[at + 2].0), ISSUES, "the address, whole, on the two rows under it");
            assert!(laid[at..at + 3].iter().all(|l| l.6 + 8 <= HEIGHT as i64 && l.0.chars().count() <= COLS), "inside the page: {:?}", &laid[at..at + 3]);
            assert!(laid[at].6 > laid[at - 1].6, "under the saved path, not over it");
        }
    }
    #[test]
    fn scenario_error_shows_red_on_the_bug_tab() {
        let _geometry = bug_lock();
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets); let s = Scene::program(mt1::ui::ScreenKind::Ground);
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = BUG;
        scenario_error_set(Some("scenarios/efate_x.toml: place: no such area".into()));
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let laid = lines(&out.text);
        assert!(laid.iter().any(|l| l.0.starts_with("Scenario could not load: scenarios/efate_x.toml") && l.2 && l.3 == ERROR_INK && l.6 == 27), "{laid:?}");
        assert!(laid.iter().any(|l| l.0 == "QoL request" && l.6 == 28 + 9 + 4), "the kinds row moved down a row");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (100, 35 + 9)), [Intent::Plus(A::Kind(1))], "the shifted kind takes the click");
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (20, 178 + 9)), [Intent::Plus(A::Submit)]);
        scenario_error_set(Some("   ".into())); assert_eq!(scenario_error(), None, "a blank error is no error");
        scenario_error_set(None);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        assert!(!lines(&out.text).iter().any(|l| l.0.starts_with("Scenario could not load")) && lines(&out.text).iter().any(|l| l.0 == "QoL request" && l.6 == 32));
        assert_eq!(c.input(&s, &ui, &UiEvent::Up(Button::Left), (100, 35)), [Intent::Plus(A::Kind(1))]);
    }
    #[test]
    fn log_tab_shows_the_whole_history_newest_last() {
        let assets = Arc::new(Assets::default()); let c = Plus::new(assets); let mut s = Scene::program(mt1::ui::ScreenKind::Ground);
        s.party = mt1::ui::scene::Party(vec![0; 0xF86]); s.party.0[0] = 0b11;
        for (i, name) in [(0usize, "Magellan"), (1, "Vlaris")] { for (k, ch) in name.bytes().enumerate() { s.party.0[0x102 + 256 * i + k] = ch; } }
        let mut ui = UiState::default(); ui.plus.open = true; ui.plus.tab = LOG;
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let text = |out: &Surface| out.text.runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>();
        assert!(text(&out).contains("No log lines yet"));
        let mut sq = mt1::ui::scene::Squad::default();
        sq.log = (0..40).map(|i| if i % 10 == 0 { format!("Round {}", i / 10 + 1) } else { format!("line {i}: thug (NPC 14) fires the 9mm advanced combat rifle at Vlaris: short: skill 0, 2D 3+4 +1 = 8 vs 7 hit by 1 (hit)") }).collect();
        sq.log_n = 40; ui.components.log.observe_squad(&sq);
        let rows = log_rows(&s, &ui);
        assert!(rows.len() > 40, "long lines wrap: {} rows", rows.len());
        assert_eq!(rows[0], (true, vec![("Round 1".into(), INK)]), "a round header is bold");
        let hit = rows.iter().find(|(_, sp)| sp.iter().any(|(t, _)| t.contains("fires"))).unwrap();
        assert!(hit.1.iter().any(|(t, reg)| t == "Vlaris" && *reg == party_register(1)), "{hit:?}");
        assert_eq!(name_spans("Vlarisx and Vlaris.", &[("Vlaris".into(), 2)]), [("Vlarisx and ".into(), INK), ("Vlaris".into(), 2), (".".into(), INK)]);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let t = text(&out); assert!(t.contains("line 39") && !t.contains("Round 1") && t.contains("more above") && !t.contains("more below"), "opens at the newest");
        let laid = lines(&out.text);
        assert!(laid.iter().any(|l| l.7.iter().any(|&r| r == party_register(1)) && l.0.contains("Vlaris")), "the name keeps its register inside the laid line");
        assert!(laid.iter().any(|l| l.0.starts_with("Round 4") && l.2), "bold header");
        let (plain, n) = scroll_lines(&s, &ui, &Assets::default()); assert_eq!((plain.len(), n), (rows.len(), LOG_ROWS));
        ui.plus.scroll = 100_000; let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out);
        let t = text(&out); assert!(t.contains("Round 1") && !t.contains("line 39") && t.contains("more below"), "scrolled to the oldest");
        ui.plus.scroll = 0; sq.log.push("Vlaris reloads.".into()); sq.log_n = 41; ui.components.log.observe_squad(&sq);
        let mut out = Surface::new(WIDTH, HEIGHT); c.draw(&s, &ui, &mut out); assert!(text(&out).contains("reloads"));
        let look = Look { font: crate::font_menu::plus_font(), shape: crate::look::Shape::Square, ..Look::default() };
        let m = Metrics::component([WIDTH * 2, HEIGHT * 2], (WIDTH, HEIGHT), &look);
        let pixels = compose(vec![(Rect::new(0, 0, WIDTH, HEIGHT), out)], m, &look);
        let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../local/plus-preview"); std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("log_unit.png"), crate::plus::png::rgb(WIDTH * 2, HEIGHT * 2, &pixels).unwrap()).unwrap();
        assert_eq!(next_tab(JOURNAL, false), LOG); assert_eq!(next_tab(CHARACTERS + 2, true), LOG); assert_eq!(main_tab(LOG), LOG);
    }
    #[test]
    fn plus_is_never_drawn_on_the_main_host() {
        let mut l = Layout::default(); super::super::components::register(&mut l, Arc::default());
        let mut ui = UiState::default();
        for screen in [mt1::ui::ScreenKind::Startup, mt1::ui::ScreenKind::Cg, mt1::ui::ScreenKind::Ground, mt1::ui::ScreenKind::Picture, mt1::ui::ScreenKind::Space] {
            let s = Scene::program(screen);
            for preset in [mt1::ui::Preset::Modern, mt1::ui::Preset::Classic] {
                ui.preset = preset; ui.plus.open = true;
                l.set_host(ID, super::super::Host::Window { id: 7, size: (WIDTH, HEIGHT) });
                l.arrange_for(&s, &ui, (320, 200));
                assert!(!l.draw_host(&s, &ui, None).iter().any(|(id, ..)| *id == ID), "{screen:?} {preset:?}");
                assert!(l.draw_host(&s, &ui, Some(7)).iter().any(|(id, ..)| *id == ID));
                assert_eq!(l.input(ID, &s, &ui, &UiEvent::Key(27), (0, 0)), [Intent::Plus(A::Close)]);
                assert_eq!(l.input(ID, &s, &ui, &UiEvent::Key(9), (0, 0)), [Intent::Plus(A::Tab(JOURNAL))], "Tab from Bug wraps to the strip's first tab");
            }
        }
    }
}
