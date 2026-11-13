use crate::panels::UiState;
use mt_engine::{io::{Event, Input}, play::{Choice, Decision, Kind, Play}};
use mt1::ui::Intent;
use std::collections::VecDeque;

pub fn press(queue: &mut impl Extend<Event>, events: &[Event]) {
    for (i, &event) in events.iter().enumerate() {
        queue.extend([event]);
        if let Event::Key(k) = event {
            if events.get(i + 1) != Some(&Event::KeyUp(k)) { queue.extend([Event::KeyUp(k)]); }
        }
    }
}
pub fn raw(events: &[Event]) -> Vec<Intent> {
    let mut released = Vec::new(); press(&mut released, events);
    released.into_iter().map(Intent::Raw).collect()
}
pub fn released_route(intents: &[Intent]) -> Option<Vec<Event>> {
    let events: Vec<_> = intents.iter().filter_map(|i| if let Intent::Raw(e) = i { Some(*e) } else { None }).collect();
    events.windows(2).any(|p| matches!((p[0], p[1]), (Event::Key(a), Event::KeyUp(b)) if a == b)).then_some(events)
}
pub fn option(d: &Decision, index: usize) -> Vec<Intent> {
    let Some(o) = d.options.get(index).filter(|o| o.enabled) else { return vec![] };
    if o.intent.is_none() { return raw(&o.input) }
    let mut play = Play::new(); play.register(d);
    let id = play.pending().unwrap().id.clone();
    if play.choose(Choice::new(id, o.id.as_str())).is_err() { return vec![] }
    play.intent().and_then(|i| i.downcast::<Intent>().ok()).map(|i| vec![*i]).unwrap_or_default()
}
pub fn norm_label(s: &str) -> String {
    let out: String = s.chars().filter(|c| !c.is_whitespace() && !matches!(c, '^' | '|' | '$')).map(|c| c.to_ascii_lowercase()).collect();
    let b = out.as_bytes(); let mut keep = Vec::with_capacity(b.len()); let mut i = 0;
    while i < b.len() {
        if i > 0 && b[i] == b'c' && b.get(i + 1) == Some(&b'r') && b[i - 1].is_ascii_digit() && !b.get(i + 2).is_some_and(|c| c.is_ascii_alphabetic()) { i += 2; continue }
        keep.push(b[i]); i += 1;
    }
    String::from_utf8(keep).unwrap_or(out)
}
pub fn line_texts(runs: &[mt_engine::present::textlog::Run]) -> Vec<(i64, String)> {
    let mut ys: Vec<i64> = runs.iter().map(|r| r.y).collect(); ys.sort_unstable(); ys.dedup();
    ys.into_iter().map(|y| { let mut line: Vec<_> = runs.iter().filter(|r| r.y == y).collect(); line.sort_by_key(|r| r.x);
        (y, norm_label(&line.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>())) }).collect()
}
pub fn money_sign_first(runs: &mut [mt_engine::present::textlog::Run]) {
    let mut order: Vec<usize> = (0..runs.len()).collect(); order.sort_by_key(|&i| (runs[i].page, runs[i].y, runs[i].x));
    let glyph = |r: &mt_engine::present::textlog::Run| if r.bytes.len() == 1 { r.bytes[0] } else { 0 };
    let mut k = 0;
    while k < order.len() {
        let mut e = k;
        while e < order.len() && glyph(&runs[order[e]]).is_ascii_digit() && (e == k || { let (p, c) = (&runs[order[e - 1]], &runs[order[e]]); c.y == p.y && c.page == p.page && c.x == p.x + i64::from(p.width) }) { e += 1 }
        if e > k && e < order.len() && glyph(&runs[order[e]]) == b'$' && { let (p, c) = (&runs[order[e - 1]], &runs[order[e]]); c.y == p.y && c.page == p.page && c.x == p.x + i64::from(p.width) } {
            let mut x = runs[order[k]].x; let dollar = order[e];
            runs[dollar].x = x; x += i64::from(runs[dollar].width);
            for &i in &order[k..e] { runs[i].x = x; x += i64::from(runs[i].width); }
            k = e + 1;
        } else { k = if e > k { e } else { k + 1 } }
    }
}
pub fn flag_hotkeys(log: &mut mt_engine::present::textlog::TextLog, d: Option<&Decision>) {
    money_sign_first(&mut log.visible); money_sign_first(&mut log.runs);
    let Some(d) = d.filter(|d| matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::List | Kind::Space | Kind::Ground)) else { return };
    let letters: Vec<Option<u8>> = match &d.list {
        Some(l) => {
            let page = d.context.table.as_deref().and_then(|t| t.strip_prefix("page:")).and_then(|t| { let (f, r) = t.split_once('+')?; Some((f.parse::<usize>().ok()?, r.split_once('/')?.0.parse::<usize>().ok()?)) });
            let (first, rows) = page.unwrap_or((0, l.rows.len())); let mut taken = Vec::new();
            l.rows.iter().enumerate().map(|(i, r)| { if i < first || i >= first + rows { return None }
                let b = r.label.bytes().take_while(|&c| c < 128).find(u8::is_ascii_alphabetic)?.to_ascii_lowercase();
                if taken.contains(&b) { None } else { taken.push(b); Some(b) } }).collect()
        }
        None => crate::panels::choices::hotkeys(d).into_iter().zip(&d.options).map(|(h, o)| h.and_then(|i| o.label.as_bytes().get(i).map(|b| b.to_ascii_lowercase()))
            .or_else(|| { let keys: Vec<&Event> = o.input.iter().filter(|e| matches!(e, Event::Key(_))).collect();
                match keys.as_slice() { [Event::Key(k)] => u8::try_from(*k).ok().filter(u8::is_ascii_alphabetic).map(|b| b.to_ascii_lowercase()), _ => None } })).collect(),
    };
    if d.list.is_none() && d.options.iter().any(|o| o.input.first() == Some(&Event::Key(mt_engine::io::keys::ESC))) {
        let mut runs: Vec<(i64, i64, u8, u8)> = log.visible.iter().map(|r| (r.y, r.x, r.bytes.first().copied().unwrap_or(0), r.width)).collect(); runs.sort_unstable();
        let mut caps = Vec::new();
        for w in runs.windows(3) {
            let [(y, x, a, wd), (y1, x1, b, wd1), (y2, x2, c, wd2)] = *w else { continue };
            if (a, b, c) != (b'E', b's', b'c') || y1 != y || y2 != y || x1 != x + wd as i64 || x2 != x1 + wd1 as i64 { continue }
            if runs.iter().any(|&(ry, rx, rb, _)| ry == y && rx == x2 + wd2 as i64 && rb.is_ascii_alphabetic()) { continue }
            caps.extend([(x, y), (x1, y), (x2, y)]);
        }
        for r in log.visible.iter_mut().chain(log.runs.iter_mut()) { if caps.contains(&(r.x, r.y)) { r.colour |= crate::pipeline::text::KEYCAP; } }
    }
    for (i, rect) in native_rows(&log.visible, d) {
        let Some(letter) = letters.get(i).copied().flatten() else { continue };
        let real = d.list.is_some() || d.options.get(i).is_some_and(|o| matches!(o.input.as_slice(), [Event::Key(k)] if u8::try_from(*k).is_ok_and(|b| b.to_ascii_lowercase() == letter)));
        if !real {
            let mut inside: Vec<_> = log.visible.iter().filter(|r| rect.contains((r.x as i32, r.y as i32))).collect(); inside.sort_by_key(|r| (r.y, r.x));
            let text = norm_label(&inside.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>());
            if !d.options.get(i).is_some_and(|o| text.contains(norm_label(&o.label).as_str())) { continue }
        }
        let mut row: Vec<(i64, i64)> = log.visible.iter().filter(|r| rect.contains((r.x as i32, r.y as i32)) && r.colour & crate::pipeline::text::KEYCAP == 0 && r.bytes.first().is_some_and(|&b| b.to_ascii_lowercase() == letter)).map(|r| (r.x, r.y)).collect();
        row.sort(); let Some(&(x, y)) = row.first() else { continue };
        for r in log.visible.iter_mut().chain(log.runs.iter_mut()) { if r.x == x && r.y == y { r.colour |= crate::pipeline::text::UNDERLINE; } }
    }
}
pub fn guessed_screens() -> Vec<String> { GUESSED.lock().unwrap().iter().cloned().collect() }
static GUESSED: std::sync::Mutex<std::collections::BTreeSet<String>> = std::sync::Mutex::new(std::collections::BTreeSet::new());
fn guessed(d: &Decision) {
    let key = format!("{}.{}", d.context.program, d.context.routine);
    let mut seen = GUESSED.lock().unwrap();
    if !seen.contains(&key) { eprintln!("click targets guessed from drawn text (no published rectangles): {key}"); seen.insert(key); }
}
pub fn native_rows(runs: &[mt_engine::present::textlog::Run], d: &Decision) -> Vec<(usize, crate::panels::Rect)> {
    use crate::panels::Rect;
    let labels: Vec<String> = match &d.list { Some(l) => l.rows.iter().map(|r| norm_label(&r.label)).collect(), None => d.options.iter().map(|o| norm_label(&o.label)).collect() };
    let mut out: Vec<(usize, Rect)> = d.rects.iter().filter(|(_, r)| r[2] > 0 && r[3] > 0).map(|&(i, r)| (i, Rect::new(r[0], r[1], r[2] as u32, r[3] as u32))).collect();
    if let Some(cells) = d.context.table.as_deref().and_then(|t| t.strip_prefix("cells:")) {
        for c in cells.split(';') {
            let Some((i, r)) = c.split_once('=') else { continue };
            let v: Vec<i32> = r.split(',').filter_map(|n| n.parse().ok()).collect();
            if let (Ok(i), [l, t, w, h]) = (i.parse::<usize>(), v.as_slice()) { if *w > 0 && *h > 0 { out.push((i, Rect::new(*l, *t, *w as u32, *h as u32))); } }
        }
    }
    if !out.is_empty() { return out }
    if !labels.iter().all(String::is_empty) && matches!(d.kind, Kind::Menu | Kind::YesNo | Kind::List) { guessed(d); }
    let list = d.list.is_some();
    let mut ys: Vec<i64> = runs.iter().map(|r| r.y).collect(); ys.sort_unstable(); ys.dedup();
    struct Word { x0: i64, x1: i64, y: i64, h: i64, text: String }
    let mut lines: Vec<Vec<Word>> = Vec::new();
    for y in ys {
        let mut line: Vec<_> = runs.iter().filter(|r| r.y == y).collect(); line.sort_by_key(|r| r.x);
        let mut words: Vec<Word> = Vec::new(); let mut gap = false;
        for r in line {
            if r.bytes.iter().all(|&b| b == b' ') { continue }
            let right = r.x + (r.bytes.len() * r.width.max(1) as usize) as i64;
            let text = norm_label(&r.bytes.iter().map(|&b| b as char).collect::<String>());
            if text.is_empty() { gap = true; continue }
            let h = r.erase_height().max(8) as i64;
            match words.last_mut() {
                Some(w) if !gap && r.x - w.x1 <= i64::from(r.width.max(1)) / 2 => { w.x1 = w.x1.max(right); w.h = w.h.max(h); w.text.push_str(&text); }
                _ => words.push(Word { x0: r.x, x1: right, y, h, text }),
            }
            gap = false;
        }
        for w in &mut words { w.text = norm_label(&w.text); }
        words.retain(|w| !w.text.is_empty());
        if !words.is_empty() { lines.push(words); }
    }
    let rect = |ws: &[&Word]| { let x0 = ws.iter().map(|w| w.x0).min().unwrap(); let x1 = ws.iter().map(|w| w.x1).max().unwrap();
        let y0 = ws.iter().map(|w| w.y).min().unwrap(); let y1 = ws.iter().map(|w| w.y + w.h).max().unwrap();
        Rect::new(x0 as i32, y0 as i32, (x1 - x0).max(1) as u32, (y1 - y0).max(1) as u32) };
    let found = |out: &Vec<(usize, Rect)>, i: usize| out.iter().any(|(k, _)| *k == i);
    for words in &lines {
        let all: Vec<&Word> = words.iter().collect();
        let text = norm_label(&words.iter().map(|w| w.text.as_str()).collect::<String>());
        if let Some(i) = labels.iter().position(|l| !l.is_empty() && (*l == text || list && text.starts_with(l.as_str()))) {
            if !found(&out, i) { out.push((i, rect(&all))); continue }
        }
        let mut order: Vec<usize> = (0..labels.len()).filter(|&i| !labels[i].is_empty() && !found(&out, i)).collect(); order.sort_by_key(|&i| std::cmp::Reverse(labels[i].len()));
        let mut taken = vec![false; words.len()];
        for i in order {
            for s in 0..words.len() {
                let mut acc = String::new(); let mut e = s;
                while e < words.len() && acc.len() < labels[i].len() && !taken[e] { acc.push_str(&words[e].text); e += 1; }
                if acc == labels[i] { taken[s..e].iter_mut().for_each(|t| *t = true); out.push((i, rect(&all[s..e]))); break }
            }
        }
    }
    if d.list.is_none() && matches!(d.context.routine.as_str(), "16374" | "16A2F" | "shipyard.pay") {
        for (i, o) in d.options.iter().enumerate() {
            if found(&out, i) { continue }
            let Some(mt_engine::io::Event::Key(k)) = o.input.first() else { continue };
            let Some(letter) = u8::try_from(*k).ok().filter(u8::is_ascii_alphabetic).map(|b| b.to_ascii_uppercase()) else { continue };
            if let Some(w) = lines.iter().flatten().find(|w| w.text.as_bytes().first().is_some_and(|b| b.to_ascii_uppercase() == letter) && !w.text.contains(':') && !w.text.bytes().any(|b| b.is_ascii_digit() || b == b'$')) { out.push((i, rect(&[w]))); }
        }
    }
    if !list {
        if let Some(i) = d.options.iter().position(|o| o.input.first() == Some(&Event::Key(mt_engine::io::keys::ESC))) {
            if !found(&out, i) { if let Some(w) = lines.iter().flatten().find(|w| w.text == "esc") { out.push((i, rect(&[w]))); } }
        }
    }
    if d.kind == Kind::YesNo && out.is_empty() && labels.len() >= 2 {
        for (i, word) in ["yes", "no"].iter().enumerate() {
            if let Some(w) = lines.iter().flatten().find(|w| w.text == *word) { out.push((i, rect(&[w]))); }
        }
    }
    let open: Vec<usize> = (0..labels.len()).filter(|&i| !labels[i].is_empty() && !found(&out, i)).collect();
    for i in open {
        'cell: for (a, b) in lines.iter().zip(lines.iter().skip(1)) {
            for wa in a { for wb in b {
                if wb.y - wa.y <= 12 && wb.x0 < wa.x1 + 4 && wa.x0 < wb.x1 + 4 && format!("{}{}", wa.text, wb.text) == labels[i] { out.push((i, rect(&[wa, wb]))); break 'cell }
            } }
        }
    }
    out
}
pub fn native_click(runs: &[mt_engine::present::textlog::Run], d: &Decision, at: (i32, i32)) -> Vec<Intent> {
    let Some(&(i, _)) = native_rows(runs, d).iter().find(|(_, r)| r.contains(at)) else { return Vec::new() };
    match &d.list {
        Some(l) => l.rows.get(i).filter(|r| r.enabled).map_or_else(Vec::new, |row| raw(&row.input.iter().chain(&l.submit).copied().collect::<Vec<_>>())),
        None => option(d, i),
    }
}
pub const PANEL_BUTTONS: &[(&str, crate::panels::Rect, mt_engine::io::Key)] = {
    use crate::panels::Rect; use mt_engine::io::keys;
    &[("ENTER", Rect::new(193, 170, 65, 10), keys::ENTER), ("UP", Rect::new(193, 179, 65, 10), keys::UP), ("DOWN", Rect::new(193, 189, 65, 10), keys::DOWN),
      ("+", Rect::new(260, 170, 58, 10), b'+' as mt_engine::io::Key), ("-", Rect::new(260, 179, 58, 10), 0x14A), ("ESC", Rect::new(260, 189, 58, 10), keys::ESC),
      ("HOME", Rect::new(196, 141, 21, 10), keys::HOME), ("PAD UP", Rect::new(217, 141, 21, 10), keys::UP), ("PGUP", Rect::new(241, 141, 21, 10), keys::PGUP),
      ("PAD +", Rect::new(196, 151, 21, 10), b'+' as mt_engine::io::Key), ("PAD -", Rect::new(241, 151, 21, 10), 0x14A),
      ("END", Rect::new(196, 161, 21, 10), keys::END), ("PAD DOWN", Rect::new(217, 161, 21, 10), keys::DOWN), ("PGDN", Rect::new(241, 161, 21, 10), keys::PGDN)]
};
pub fn button_route(d: &Decision, at: (i32, i32)) -> Vec<Intent> {
    if d.context.program != "t" { return Vec::new() }
    let Some(&(_, _, key)) = PANEL_BUTTONS.iter().find(|(_, r, _)| r.contains(at)) else { return Vec::new() };
    let esc = || d.options.iter().find(|o| o.enabled && o.input.first() == Some(&Event::Key(mt_engine::io::keys::ESC))).map_or_else(|| raw(&[Event::Key(mt_engine::io::keys::ESC)]), |o| raw(&o.input));
    match (&d.list, key) {
        (Some(l), mt_engine::io::keys::ENTER) => raw(&l.submit),
        (Some(l), k) if k == b'+' as mt_engine::io::Key => raw(&l.increase),
        (Some(l), 0x14A) => raw(&l.decrease),
        (_, mt_engine::io::keys::ESC) => esc(),
        (_, k) => raw(&[Event::Key(k)]),
    }
}
pub fn modal(ui: &UiState) -> bool {
    ui.decision.as_ref().is_some_and(|d| !matches!(d.kind, Kind::Ground | Kind::Space | Kind::CombatTurn))
}

pub fn pointer_allowed(scene: &mt1::ui::Scene, id: &str, event: &crate::panels::UiEvent, at: (i32, i32)) -> bool {
    use crate::panels::{Button, UiEvent, picture::portrait};
    use mt1::ui::ScreenKind;
    !matches!(event, UiEvent::Down(Button::Right) | UiEvent::Up(Button::Right) | UiEvent::DoubleClick(Button::Right))
        || (id == "Viewport" && scene.screen == ScreenKind::Ground)
        || ((id == "Party strip" || id == "Picture" && scene.screen == ScreenKind::Record) && portrait(at).is_some())
}

#[derive(Default)]
pub struct Store { events: VecDeque<Event>, pub delivered: bool, refresh: bool, fallback: Option<Decision>, gun_release: bool, fresh: bool, restored: bool }
impl Store {
    pub fn fresh(&self) -> bool { self.fresh }
    pub fn settle(&mut self) { self.fresh = false; }
    pub fn wants(&self, ui: &UiState) -> bool {
        !self.playing() && (self.restored || !modal(ui) && (ui.decision.is_none() || self.refresh))
    }
    pub fn playing(&self) -> bool { self.events.iter().any(|e| !matches!(e, Event::KeyUp(_))) }
    pub fn pending(&self) -> bool { !self.events.is_empty() }
    pub fn record(&mut self, ui: &mut UiState, d: &Decision) { ui.decision = Some(d.clone()); self.refresh = false; self.fallback = None; self.gun_release = false; self.fresh = true; self.restored = false; }
    pub fn refresh(&mut self) { self.refresh = true; }
    pub fn queue(&mut self, ui: &mut UiState, events: &[Event]) {
        if events.is_empty() { return }
        let hold = ui.decision.as_ref().is_some_and(|d| d.context.program == "s" && d.kind == Kind::Menu)
            && events.iter().any(|e| matches!(e, Event::Key(k) if mt_engine::io::scan::make_code(*k).is_some_and(|c| c > 0x46 && c < 0x80)));
        if hold { while matches!(self.events.front(), Some(Event::KeyUp(_))) { self.events.pop_front(); } }
        self.gun_release |= hold;
        press(&mut self.events, events); self.fallback = None; ui.decision = None; self.restored = false;
    }
    pub fn pop(&mut self) -> Option<Event> {
        if self.gun_release && matches!(self.events.front(), Some(Event::KeyUp(_))) { self.delivered = false; return None; }
        let e = self.events.pop_front(); self.delivered = e.is_some(); e
    }
    pub fn key_pending(&self) -> bool { self.events.iter().any(|e| matches!(e, Event::Key(_))) }
    pub fn next_key(&mut self) -> Option<u16> {
        while let Some(e) = self.pop() { if let Event::Key(k) = e { return Some(k) } }
        None
    }
    pub fn hardware(&mut self, ui: &mut UiState, e: Event) {
        if let Event::Key(k) = e {
            if ui.decision.as_ref().is_some_and(|d| matches!(d.kind, Kind::Text | Kind::Number))
                && !matches!(mt_engine::io::scan::make_code(k), Some(1 | 0x1c)) { return }
            let code = mt_engine::io::scan::make_code(k);
            let answered = code.is_some() && ui.decision.as_ref().is_some_and(|d| d.options.iter().any(|o| matches!(o.input.first(), Some(Event::Key(o)) if mt_engine::io::scan::make_code(*o) == code)));
            self.fallback = if modal(ui) && !answered { ui.decision.take() } else { ui.decision = None; None };
        } else if matches!(e, Event::Mouse(_)) { ui.decision = None; self.restored = false; }
    }
    pub fn restore(&mut self, ui: &mut UiState) {
        if ui.decision.is_none() && !self.playing() { ui.decision = self.fallback.take(); self.restored |= ui.decision.is_some(); }
    }
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::keys, play::Option as Opt};
    #[test]
    fn periphery_right_click_is_inert_in_both_product_layouts() {
        use crate::panels::{self, Button, Layout, UiEvent};
        use mt1::ui::{Preset, Scene, ScreenKind};
        for preset in [Preset::Modern, Preset::Classic] {
            let mut layout = Layout::default(); panels::components::register(&mut layout, Default::default());
            let mut ui = UiState { preset, fixed_screen: true, ..Default::default() };
            for screen in [ScreenKind::Startup, ScreenKind::Cg, ScreenKind::Intro, ScreenKind::Ground,
                ScreenKind::Picture, ScreenKind::Record, ScreenKind::Space] {
                let scene = Scene::program(screen); layout.arrange_for(&scene, &ui, (640, 480));
                for id in ["Commands", "Side panel", "Combat controls", "Choices", "Menu", "Continue", "Text input", "Plus", "Combat log", "Minimap"] {
                    for event in [UiEvent::Down(Button::Right), UiEvent::Up(Button::Right), UiEvent::DoubleClick(Button::Right)] {
                        assert!(layout.input(id, &scene, &ui, &event, (10, 10)).is_empty(), "{preset:?} {screen:?} {id}");
                        assert!(panels::controls::dispatch(&layout, id, &scene, &mut ui, &event, (10, 10)).is_empty());
                    }
                }
                assert!(layout.input("Picture", &scene, &ui, &UiEvent::Up(Button::Right), (250, 160)).is_empty());
                assert!(layout.input("Party strip", &scene, &ui, &UiEvent::Up(Button::Right), (220, 10)).is_empty());
            }
            let scene = Scene::program(ScreenKind::Ground);
            assert!(pointer_allowed(&scene, "Viewport", &UiEvent::Up(Button::Right), (10, 10)));
            assert!(pointer_allowed(&scene, "Party strip", &UiEvent::Up(Button::Right), (20, 20)));
            assert!(pointer_allowed(&scene, "Commands", &UiEvent::Up(Button::Left), (10, 10)));
        }
    }
    #[test]
    fn efate_shop_click_route_releases_every_make_without_duplicate_breaks() {
        let route = [Event::Key(keys::DOWN), Event::KeyUp(keys::DOWN), Event::Key(keys::ENTER)];
        let mut q = Vec::new(); press(&mut q, &route);
        assert_eq!(q, [route[0], route[1], route[2], Event::KeyUp(keys::ENTER)]);
        let mut s = Store::default(); let mut ui = UiState::default();
        s.record(&mut ui, &Decision::new(Kind::List, "t", "11571"));
        assert!(!s.wants(&ui), "nested menu_getkey cannot replace the shop");
        s.queue(&mut ui, &route); assert!(ui.decision.is_none()); assert!(!s.wants(&ui));
        assert_eq!(s.pop(), Some(route[0])); assert!(s.delivered);
        assert_eq!(s.pop(), Some(route[1])); assert_eq!(s.next_key(), Some(keys::ENTER));
        assert!(s.wants(&ui), "a trailing break must not suppress the next menu");
        assert_eq!(s.pop(), Some(Event::KeyUp(keys::ENTER))); assert!(!s.pending());
        assert_eq!(released_route(&raw(&route)).unwrap(), q);
        assert!(released_route(&[Intent::Raw(Event::Key(keys::DOWN))]).is_none());
    }
    #[test]
    fn g7_gunner_arrow_survives_the_ten_scheduler_polls_before_consumption() {
        for routine in ["9FDB", "955C"] {
            let mut s = Store::default(); let mut ui = UiState::default();
            let d = Decision::new(Kind::Menu, "s", routine); s.record(&mut ui, &d);
            s.queue(&mut ui, &[Event::Key(keys::RIGHT)]);
            assert_eq!(s.pop(), Some(Event::Key(keys::RIGHT)));
            for _ in 0..10 { assert_eq!(s.pop(), None, "the ISR must still see the arrow make"); }
            assert!(s.wants(&ui), "the consumed arrow can publish the next decision");
            s.record(&mut ui, &d);
            assert_eq!(s.pop(), Some(Event::KeyUp(keys::RIGHT)));
            assert!(!s.pending());
        }
    }
    #[test]
    fn efate_broken_native_option_uses_its_intent_and_disabled_is_inert() {
        let mut d = Decision::new(Kind::CombatTurn, "t", "combat.turn");
        let mut o = Opt::new("end", "End turn", vec![]);
        o.intent = Some(mt_engine::play::NativeIntent::new(Intent::EndTurn)); d.options.push(o);
        assert_eq!(option(&d, 0), [Intent::EndTurn]);
        d.options[0].enabled = false; assert!(option(&d, 0).is_empty()); assert!(option(&d, 1).is_empty());
    }
    #[test]
    fn efate_quiet_hardware_replaces_wait_but_a_release_does_not() {
        let mut ui = UiState::default(); let mut s = Store::default();
        s.record(&mut ui, &Decision::new(Kind::Ground, "t", "6923")); assert!(!s.wants(&ui));
        s.refresh(); assert!(s.wants(&ui));
        s.hardware(&mut ui, Event::KeyUp(13)); assert!(ui.decision.is_some());
        s.hardware(&mut ui, Event::Key(13)); assert!(ui.decision.is_none());
        assert_eq!(s.next_key(), None); assert!(!s.key_pending());
    }
    #[test]
    fn starport_native_unknown_key_and_typing_keep_click_affordances() {
        let mut ui = UiState::default(); let mut s = Store::default();
        s.record(&mut ui, &Decision::new(Kind::Menu, "t", "AF79"));
        s.hardware(&mut ui, Event::Key(b'z' as u16)); assert!(ui.decision.is_none());
        s.restore(&mut ui); assert_eq!(ui.decision.as_ref().unwrap().kind, Kind::Menu);
        s.hardware(&mut ui, Event::Key(13)); s.record(&mut ui, &Decision::new(Kind::Text, "t", "15B69"));
        s.restore(&mut ui); s.hardware(&mut ui, Event::Key(b'A' as u16));
        assert_eq!(ui.decision.as_ref().unwrap().kind, Kind::Text);
        s.hardware(&mut ui, Event::Key(27)); assert!(s.wants(&ui));
    }
    #[test]
    fn a_key_that_answers_the_wait_is_not_restored() {
        let mut ui = UiState::default(); let mut s = Store::default();
        let rects = [mt_engine::io::mouse::Hotspot::default(); 5];
        s.record(&mut ui, &mt1::play::menus::selector(&rects, 0));
        s.hardware(&mut ui, Event::Key(27)); s.restore(&mut ui); assert!(ui.decision.is_none(), "Esc answered the picker");
        s.record(&mut ui, &mt1::play::menus::selector(&rects, 0));
        s.hardware(&mut ui, Event::Key(b'z' as u16)); s.restore(&mut ui); assert!(ui.decision.is_some(), "an unknown key changes nothing: kept");
    }

    enum Click { Option(usize), Row(usize), Add(usize), Ok, Published(usize) }
    struct ClickInput { store: Store, ui: UiState, choices: crate::panels::choices::Choices, clicks: VecDeque<Click>, polls: usize, seen: std::rc::Rc<std::cell::RefCell<Vec<Decision>>> }
    impl ClickInput {
        fn new(clicks: impl IntoIterator<Item = Click>) -> Self {
            let mut ui = UiState::default(); ui.component_input.size = (304, 180);
            Self { store: Store::default(), ui, choices: crate::panels::choices::Choices::new(std::sync::Arc::default()), clicks: clicks.into_iter().collect(), polls: 0, seen: Default::default() }
        }
        fn click(&mut self) {
            use crate::panels::{Button, Component, UiEvent};
            let scene = mt1::ui::Scene::program(mt1::ui::ScreenKind::Picture);
            if self.store.playing() || self.ui.decision.is_none() { return }
            let Some(click) = self.clicks.pop_front() else { return };
            let hit = |at| self.choices.input(&scene, &self.ui, &UiEvent::Up(Button::Left), at);
            let result = match click {
                Click::Published(i) => { let d = self.ui.decision.as_ref().unwrap(); let r = d.rects.iter().find(|(k, _)| *k == i).unwrap_or_else(|| panic!("{}.{} publishes no rectangle for {i}: {:?}", d.context.program, d.context.routine, d.rects)).1;
                    native_click(&[], d, (r[0] + r[2] / 2, r[1] + r[3] / 2)) }
                Click::Option(i) => hit((10, 27 + i as i32 * 12)),
                Click::Row(i) | Click::Add(i) => {
                    for _ in 0..i / 12 { hit((90, 174)); }
                    let picked = hit((10, 27 + (i % 12) as i32 * 12));
                    if matches!(click, Click::Add(_)) { hit((160, 174)) } else { picked }
                }
                Click::Ok => hit((210, 174)),
            };
            let events: Vec<_> = result.into_iter().map(|i| if let Intent::Raw(e) = i { e } else { panic!("not a native route") }).collect();
            assert!(!events.is_empty(), "the scripted click must answer the wait");
            self.store.queue(&mut self.ui, &events);
        }
    }
    impl Input for ClickInput {
        fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
        fn decide(&mut self, d: &Decision) { self.store.record(&mut self.ui, d); self.seen.borrow_mut().push(d.clone()); }
        fn next_key(&mut self) -> u16 { self.click(); self.store.next_key().expect("click has a key") }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event {
            self.polls += 1; assert!(self.polls < 10000, "click input stalled: {:?}", self.ui.decision);
            self.click(); if let Some(e) = self.store.pop() { e } else { clock.wait_tick(); Event::Idle }
        }
        fn arrived(&mut self) -> bool { false }
    }
    fn scenario(name: &str) -> Option<mt1::ground::Ground> {
        let Some(dir) = oracle_env::gamefiles() else { return None };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios").join(format!("{name}.toml"));
        Some(crate::scenario::load(&path, std::path::Path::new(&dir)).unwrap().game)
    }
    #[test]
    fn character_record_publishes_every_control_without_changing_state_or_identity() {
        use mt1::ground::{render::character, state::{HOTSPOTS_CHAR, HOTSPOTS_CHAR_N}};
        use crate::panels::{picture::Picture, Button, Component, UiEvent};
        let Some(mut g) = scenario("efate_quiet") else { return };
        g.squad = None; g.clock = Box::new(mt_engine::sim::tick::Counter::at(0));
        for (hint, active, inactive) in [(1, "cmd:pick", "cmd:move"), (0, "cmd:move", "cmd:pick"), (1, "cmd:pick", "cmd:move")] {
            character::hint(&mut g, hint);
            let input = ClickInput::new([]); let seen = input.seen.clone(); g.input = Box::new(input);
            let before = (g.mem.ds.clone(), g.mem.segs.clone(), g.fb.snapshot(), mt1::play::hash(&mut g));
            mt1::play_prompts::character(&mut g, 0);
            assert_eq!((g.mem.ds.clone(), g.mem.segs.clone(), g.fb.snapshot(), mt1::play::hash(&mut g)), before);
            let d = seen.borrow()[0].clone();
            assert_eq!(d.rects.len(), HOTSPOTS_CHAR_N + 1);
            let ui = UiState { decision: Some(d.clone()), ..Default::default() };
            let click = |at| Picture::full().input(&mt1::ui::Scene::program(mt1::ui::ScreenKind::Record), &ui, &UiEvent::Up(Button::Left), at);
            let index = |id: &str| d.options.iter().position(|o| o.id == id).unwrap();
            for n in 0..HOTSPOTS_CHAR_N {
                let at = HOTSPOTS_CHAR + 9 * n as u16;
                let scan = g.mem.u8(at + 8);
                let id = match scan {
                    0x2F => "cmd:activate".to_string(),
                    0x96..=0xA8 => format!("inventory:{}", scan - 0x96),
                    _ => d.options.iter().find(|o| matches!(o.input.as_slice(), [Event::Key(k)] if mt_engine::io::scan::make_code(*k) == Some(scan))).unwrap().id.clone(),
                };
                let r = [g.mem.i16(at + 2) as i32, g.mem.i16(at) as i32, g.mem.i16(at + 6) as i32, g.mem.i16(at + 4) as i32];
                assert!(d.rects.contains(&(index(&id), r)), "original record region {n}: {id} {r:?}");
                assert_eq!(click((r[0] + r[2] / 2, r[1] + r[3] / 2)), option(&d, index(&id)), "region {n}: {id}");
            }
            assert_eq!(click((120, 84)), option(&d, index(active)));
            assert!(!d.rects.iter().any(|(i, _)| *i == index(inactive)), "no stale Pick/Move target");
            assert_eq!(d.options[index("cmd:activate")].input, [Event::Key(keys::ENTER)]);
            assert_eq!(d.options[index("back")].input, [Event::Key(keys::ESC)]);
            assert!(d.rects.contains(&(index("back"), [275, 60, 29, 8])));
            assert_eq!(click((289, 64)), option(&d, index("back")));
            assert!(click((272, 64)).is_empty(), "gap between View and Close");
            let mut original = d.clone(); original.rects.clear();
            let mut published = Play::new(); published.register(&d);
            let mut unpublished = Play::new(); unpublished.register(&original);
            assert_eq!(published.pending().unwrap().id, unpublished.pending().unwrap().id, "rectangles do not change decision identity");
            assert_eq!(published.pending().unwrap().state, unpublished.pending().unwrap().state, "rectangles do not change the decision hash");
        }
    }
    fn record_clicks(g: mt1::ground::Ground, steps: &[(&'static str, &'static str, Option<(i32, i32)>)]) -> (mt1::ground::Ground, Vec<Decision>) {
        record_clicks_from(g, steps, None)
    }
    fn record_clicks_from(mut g: mt1::ground::Ground, steps: &[(&'static str, &'static str, Option<(i32, i32)>)], portrait: Option<u8>) -> (mt1::ground::Ground, Vec<Decision>) {
        use crate::panels::{picture::Picture, Button, Component, UiEvent};
        use mt1::ui::{Scene, ScreenKind};
        struct RecordWindow {
            store: Store, ui: UiState, scene: Scene, polls: usize,
            steps: VecDeque<(&'static str, &'static str, Option<(i32, i32)>)>,
            seen: std::rc::Rc<std::cell::RefCell<Vec<Decision>>>,
        }
        impl Input for RecordWindow {
            fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
            fn decide(&mut self, d: &Decision) { self.store.record(&mut self.ui, d); self.seen.borrow_mut().push(d.clone()); }
            fn next_key(&mut self) -> u16 { panic!("record reads scan events") }
            fn arrived(&mut self) -> bool { false }
            fn closed(&self) -> bool { self.steps.is_empty() && self.scene.screen == ScreenKind::Ground }
            fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event {
                self.polls += 1; assert!(self.polls < 10000, "record click stalled before {:?}", self.steps.front());
                if !self.store.playing() {
                    if let Some(d) = self.ui.decision.as_ref() {
                        let (routine, id, at) = self.steps.pop_front().expect("unexpected record wait");
                        assert_eq!(d.context.routine, routine);
                        let id = if id.is_empty() { d.default.as_deref().unwrap() } else { id };
                        let i = d.options.iter().position(|o| o.id == id).unwrap();
                        let at = at.unwrap_or_else(|| { let r = d.rects.iter().find(|(k, _)| *k == i).unwrap().1; (r[0] + r[2] / 2, r[1] + r[3] / 2) });
                        let got = Picture::full().input(&self.scene, &self.ui, &UiEvent::Up(Button::Left), at);
                        assert_eq!(got, option(d, i), "click {routine} {id} at {at:?}");
                        let events: Vec<_> = got.into_iter().map(|v| if let Intent::Raw(e) = v { e } else { panic!("not a native route") }).collect();
                        self.store.queue(&mut self.ui, &events);
                    }
                }
                self.store.pop().unwrap_or_else(|| { clock.wait_tick(); Event::Idle })
            }
            fn present(&mut self, _: &[u32]) { self.store.refresh(); }
            fn scene(&mut self, s: &dyn std::any::Any) { if let Some(s) = s.downcast_ref::<Scene>() { self.scene = s.clone(); self.store.refresh(); self.store.settle(); } }
        }
        let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        g.clock = Box::new(mt_engine::sim::tick::Counter::at(0)); g.scene = Some(mt1::ui::SceneHook::default());
        g.mem.set_u16(0x7000, 0); g.mem.set_i16(0x3FF7, 0);
        let mut window = RecordWindow { store: Store::default(), ui: UiState::default(), scene: Scene::program(ScreenKind::Record),
            polls: 0, steps: steps.iter().copied().collect(), seen: seen.clone() };
        if let Some(i) = portrait {
            use crate::panels::party_strip::PartyStrip;
            use mt1::ui::Unit;
            window.scene = Scene::capture(&g, ScreenKind::Ground, None);
            assert!(window.scene.squad.active, "exercise the product party strip");
            let strip = PartyStrip::new(std::sync::Arc::default()); let at = (26 + 40 * i as i32, 25);
            let click = |e| strip.input(&window.scene, &window.ui, &e, at);
            assert_eq!(click(UiEvent::Up(Button::Left)), [Intent::Select(i)], "left selects");
            assert_eq!(click(UiEvent::DoubleClick(Button::Left)), [Intent::Follow(Some(Unit::Traveller(i)))], "double centres");
            assert!(pointer_allowed(&window.scene, "Party strip", &UiEvent::Up(Button::Right), at));
            let events = released_route(&click(UiEvent::Up(Button::Right))).expect("right-click supplies a released record route");
            assert_eq!(events, [Event::Key(keys::F1 + i as u16), Event::KeyUp(keys::F1 + i as u16)]);
            window.store.queue(&mut window.ui, &events);
        }
        g.input = Box::new(window);
        if portrait.is_some() { mt1::ground::main_loop::run(&mut g) } else { mt1::ground::commands_character::run(&mut g, 0) }
        let seen = seen.borrow().clone();
        assert_eq!(seen.iter().map(|d| d.context.routine.as_str()).collect::<Vec<_>>(), steps.iter().map(|s| s.0).collect::<Vec<_>>());
        (g, seen)
    }
    #[test]
    fn character_record_view_close_and_pick_move_answer_window_clicks() {
        let Some(g) = scenario("efate_quiet") else { return };
        let party = g.mem.party().to_vec();
        let steps = [("A8F7", "cmd:activate", Some((246, 64))), ("D2C3", "continue", Some((10, 60))),
            ("A8F7", "cmd:pick", Some((120, 84))), ("DE5E", "", None),
            ("A8F7", "cmd:move", Some((120, 84))), ("DE5E", "", None), ("A8F7", "back", Some((289, 64)))];
        let (g, _) = record_clicks(g, &steps);
        assert_eq!(g.mem.i16(0x6EFC), 0, "the selected object was put back");
        assert_eq!(g.mem.party(), party, "View/Pick/Move/Close preserve the party and inventory");
        assert_eq!(g.mem.u16(0x7012), 1, "Close restored the ground mouse bounds");
    }
    #[test]
    fn character_record_marching_order_positions_answer_window_clicks() {
        let rects = [[270, 4, 27, 15], [258, 18, 27, 15], [285, 18, 27, 15], [258, 32, 27, 15], [285, 32, 27, 15]];
        let centre = |i: usize| { let r = rects[i]; Some((r[0] + r[2] / 2, r[1] + r[3] / 2)) };
        let ids = ["menu:de5e/0", "menu:de5e/1", "menu:de5e/2", "menu:de5e/3", "menu:de5e/4"];
        for (a, b) in [(0, 1), (2, 3), (4, 0)] {
            let Some(g) = scenario("../tests/atoms/scenarios/atom_105_party_order") else { return };
            let mut expected = g.mem.party().to_vec(); expected.swap(a + 1, b + 1);
            let (g, seen) = record_clicks(g, &[("A8F7", "cmd:orders", Some((230, 12))),
                ("DE5E", ids[a], centre(a)), ("DE5E", ids[b], centre(b)), ("A8F7", "back", Some((289, 64)))]);
            let orders = seen[0].options.iter().position(|o| o.id == "cmd:orders").unwrap();
            assert!(seen[0].rects.contains(&(orders, [208, 2, 45, 22])));
            for d in &seen[1..3] { assert_eq!(d.rects, rects.into_iter().enumerate().collect::<Vec<_>>(), "both picks publish the five original positions"); }
            assert_eq!(g.mem.party(), expected, "clicks swap only the two marching-order slots {a}/{b}");
        }
    }
    #[test]
    fn character_record_ground_right_click_opens_each_traveller() {
        for i in 0..5 {
            let Some(mut g) = scenario("../tests/atoms/scenarios/atom_130_character_pack") else { return };
            g.squad = Some(Default::default());
            let party = g.mem.party().to_vec();
            let (g, seen) = record_clicks_from(g, &[("A8F7", "back", Some((289, 64)))], Some(i));
            assert_eq!(seen[0].context.who, Some(i as usize));
            assert_eq!(g.mem.party(), party, "opening and closing a record preserves the party");
            assert_eq!(g.mem.u16(0x7012), 1);
        }
    }
    #[test]
    fn character_record_portrait_clicks_switch_all_five_travellers() {
        let Some(g) = scenario("../tests/atoms/scenarios/atom_130_character_pack") else { return };
        let party = g.mem.party().to_vec();
        let (g, seen) = record_clicks(g, &[("A8F7", "cmd:char/1", Some((66, 25))),
            ("A8F7", "cmd:char/2", Some((106, 25))), ("A8F7", "cmd:char/3", Some((146, 25))),
            ("A8F7", "cmd:char/4", Some((186, 25))), ("A8F7", "cmd:char/0", Some((26, 25))), ("A8F7", "back", Some((289, 64)))]);
        assert_eq!(seen.iter().map(|d| d.context.who.unwrap()).collect::<Vec<_>>(), [0, 1, 2, 3, 4, 0]);
        assert_eq!(g.mem.party(), party, "switching records preserves the party");
    }
    #[test]
    fn efate_quiet_real_game_menu_click_runs_the_original_selector() {
        use mt1::ground::{input::menu, state::Mem};
        let Some(mut g) = scenario("efate_quiet") else { return };
        let m = menu::Menu::read(&g.mem, Mem::ds(0x45fe)).unwrap();
        for j in 0..6 { g.mem.write_bytes(Mem::ds(0x45fe + 39 + j * 4), &[0; 4]); g.mem.set_u16(0x45fe + 63 + j * 2, 0); }
        for i in 0..m.labels.len() {
            g.input = Box::new(ClickInput::new([Click::Option(i)]));
            assert_eq!(menu::run_menu(&mut g, Mem::ds(0x45fe), 0, 0), if i + 1 == m.labels.len() { -1 } else { i as i16 }, "row {i}");
        }
        let start = std::time::Instant::now();
        for _ in 0..10 { std::hint::black_box(mt1::play::hash(&mut g)); }
        eprintln!("M8M ground hash: {:?} per registration; idle catalogue only refreshes on presentation changes", start.elapsed() / 10);
    }
    #[test]
    fn efate_shop_real_paged_selector_reaches_row_and_changes_quantity_once() {
        use mt1::ground::starport::ui::{self, Row, Shop};
        let Some(mut g) = scenario("efate_shop") else { return };
        let mut rows: Vec<_> = (0..22).map(|i| Row::new(format!("Item {i}").into_bytes(), 10, 3)).collect();
        g.input = Box::new(ClickInput::new([Click::Row(17)]));
        assert_eq!(ui::shop(&mut g, b"Pick", &mut rows, Shop::select(4)), 18);
        g.input = Box::new(ClickInput::new([Click::Add(17), Click::Ok]));
        assert_eq!(ui::shop(&mut g, b"Buy", &mut rows, Shop { rows: 4, select: false, money: 1000, room: 100, kind: 0, sign: 1 }), 18);
        assert_eq!(rows[17].quantity, 1);
        assert!(rows.iter().enumerate().all(|(i, r)| i == 17 || r.quantity == 0));
    }
    #[test]
    fn ground_menu_and_list_publish_rectangles_that_commit_their_choice() {
        use mt1::ground::{input::menu, starport::ui::{self, Row, Shop}, state::Mem};
        let Some(mut g) = scenario("efate_quiet") else { return };
        let m = menu::Menu::read(&g.mem, Mem::ds(0x45fe)).unwrap();
        for j in 0..6 { g.mem.write_bytes(Mem::ds(0x45fe + 39 + j * 4), &[0; 4]); g.mem.set_u16(0x45fe + 63 + j * 2, 0); }
        for i in 0..m.labels.len() {
            g.input = Box::new(ClickInput::new([Click::Published(i)]));
            assert_eq!(menu::run_menu(&mut g, Mem::ds(0x45fe), 0, 0), if i + 1 == m.labels.len() { -1 } else { i as i16 }, "menu row {i}");
        }
        let Some(mut g) = scenario("efate_shop") else { return };
        mt1::ground::starport::shop_menu::set_box(&mut g, 0);
        let mut rows: Vec<_> = (0..22).map(|i| Row::new(format!("Item {i}").into_bytes(), 10, 3)).collect();
        for i in 0..4 {
            g.input = Box::new(ClickInput::new([Click::Published(i)]));
            assert_eq!(ui::shop(&mut g, b"Pick", &mut rows, Shop::select(4)), i + 1, "list row {i}");
        }
    }
    #[test]
    fn bill_words_are_published_where_the_pen_drew_them() {
        use mt1::ground::starport::ui;
        use mt_engine::present::textlog;
        let Some(mut g) = scenario("efate_shop") else { return };
        g.input = Box::new(ClickInput::new([]));
        textlog::set_enabled(true);
        mt1::ground::starport::shop_menu::set_box(&mut g, 0);
        let h = g.mem.i16(0x6E08); ui::clear(&mut g); ui::limits(&mut g, h + 50, h + 90);
        ui::say(&mut g, b"Fill^All^Pay", true, 10);
        let words = ui::drawn_words(&mut g, b"Fill^All^Pay", &[b"Fill", b"All", b"Pay", b"Nope"]);
        let runs: Vec<_> = (0..2).flat_map(|p| textlog::frame(&g.fb, p).visible).collect();
        textlog::set_enabled(false);
        assert_eq!(words.len(), 4); assert_eq!(words[3], [0; 4], "a word not in the line has no rectangle");
        for (w, first) in words[..3].iter().zip([b'F', b'A', b'P']) {
            assert!(w[2] > 0 && w[3] == 8, "{w:?}");
            assert!(runs.iter().any(|r| r.bytes == [first] && r.x == w[0] as i64 && r.y == w[1] as i64), "{} drawn at {w:?}; runs {:?}", first as char, runs.iter().map(|r| (r.bytes.clone(), r.x, r.y)).collect::<Vec<_>>());
        }
        assert!(words[0][0] < words[1][0] && words[1][0] < words[2][0] && words[1][0] == h as i32 + 50 && words[2][0] == h as i32 + 90, "the two column stops: {words:?}");
    }
    #[test]
    fn casino_offers_spin_and_leave_as_published_words_before_and_after_a_spin() {
        let Some(mut g) = scenario("m5_casino_rich") else { return };
        let input = ClickInput::new([Click::Published(0), Click::Published(0), Click::Published(1)]); let seen = input.seen.clone();
        g.input = Box::new(input); g.clock = Box::new(mt_engine::sim::tick::Counter::at(0)); g.scene = Some(mt1::ui::SceneHook::default()); g.mem.set_u16(0x7000, 0);
        let who = g.mem.selected_char() as i16; let before = mt1::ground::starport::cash(&g, who);
        g.mem.set_event(12); assert!(mt1::ground::starport::office(&mut g), "the casino office (event 12)");
        let waits: Vec<Decision> = seen.borrow().iter().filter(|d| d.context.routine == "1AC9C").cloned().collect();
        assert_eq!(waits.len(), 3, "the welcome, after spin 1, after spin 2");
        for d in &waits {
            assert_eq!(d.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Spin ($100)", "Leave"]);
            assert_eq!(d.rects.iter().map(|r| r.0).collect::<Vec<_>>(), [0, 1], "both words published");
            assert!(d.rects.iter().all(|(_, r)| r[2] > 0 && r[3] == 8), "{:?}", d.rects);
            assert_eq!(crate::panels::choices::hotkeys(d), [Some(0), Some(0)], "S and L are the key letters");
        }
        assert!(waits[0].rects.iter().all(|(_, r)| r[1] < 141), "fx-clicks2 4: the welcome's words sit one blank line under the welcome text, not below row 141: {:?}", waits[0].rects);
        let after = mt1::ground::starport::cash(&g, who);
        assert!(after + 200 >= before && (after + 200 - before) % 10 == 0, "two paid spins (and any winnings): {before} -> {after}");
    }
    #[test]
    fn trade_in_offers_accept_and_cancel_as_published_words() {
        struct Script(VecDeque<Event>, std::rc::Rc<std::cell::RefCell<Vec<Decision>>>, usize, usize);
        impl Input for Script {
            fn wants_decision(&self) -> bool { true }
            fn decide(&mut self, d: &Decision) {
                let rows: Vec<&str> = d.list.iter().flat_map(|l| l.rows.iter().map(|r| r.label.as_str())).collect();
                let keys = |k: &[u16]| k.iter().map(|&k| Event::Key(k)).collect::<Vec<_>>();
                let answer = match d.context.routine.as_str() {
                    "161EE" => return,
                    "11571" if rows.iter().any(|r| r.starts_with("Party Account")) => return,
                    "supplies.trade-in" => {
                        self.3 += 1; let r = d.rects.iter().find(|(k, _)| *k == self.3 % 2).expect("published").1;
                        native_click(&[], d, (r[0] + r[2] / 2, r[1] + r[3] / 2)).into_iter().map(|i| if let Intent::Raw(e) = i { e } else { panic!("not a native route") }).collect() }
                    "11571" if rows.contains(&"Computers") && self.3 < 2 => keys(&[336, 336, 13]),
                    "1199B" if self.3 == 0 => keys(&[13]),
                    _ => keys(&[27]),
                };
                self.0 = answer.into(); self.1.borrow_mut().push(d.clone());
            }
            fn next_key(&mut self) -> u16 { panic!("no direct key read") }
            fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event { self.2 += 1; assert!(self.2 < 20000, "stalled"); self.0.pop_front().unwrap_or_else(|| { clock.wait_tick(); Event::Idle }) }
            fn arrived(&mut self) -> bool { false }
        }
        let Some(mut g) = scenario("m7_starport_before_save") else { return };
        let seen: std::rc::Rc<std::cell::RefCell<Vec<Decision>>> = Default::default();
        g.input = Box::new(Script(VecDeque::new(), seen.clone(), 0, 0)); g.clock = Box::new(mt_engine::sim::tick::Counter::at(0)); g.scene = Some(mt1::ui::SceneHook::default()); g.mem.set_u16(0x7000, 0);
        mt1::ground::starport::redraw(&mut g); mt1::ground::starport::supplies::run(&mut g).unwrap();
        let seen = seen.borrow(); let offers: Vec<usize> = seen.iter().enumerate().filter(|(_, d)| d.context.routine == "supplies.trade-in").map(|(i, _)| i).collect();
        assert_eq!(offers.len(), 2, "{:?}", seen.iter().map(|d| d.context.routine.clone()).collect::<Vec<_>>());
        for &i in &offers {
            let d = &seen[i];
            assert_eq!(d.options.iter().map(|o| o.label.as_str()).collect::<Vec<_>>(), ["Accept", "Cancel"]);
            assert_eq!(d.rects.iter().map(|r| r.0).collect::<Vec<_>>(), [0, 1], "both words published");
            assert!(d.rects.iter().all(|(_, r)| r[2] > 0 && r[3] == 8) && d.rects[0].1[0] < d.rects[1].1[0], "{:?}", d.rects);
            assert_eq!(crate::panels::choices::hotkeys(d), [Some(0), Some(0)], "A and C are the key letters");
        }
        let rows = |d: &Decision| d.list.as_ref().map_or(Vec::new(), |l| l.rows.iter().map(|r| r.label.clone()).collect::<Vec<_>>());
        assert!(rows(&seen[offers[0] + 1]).contains(&"Computers".to_string()), "Cancel returns to the categories: {:?}", rows(&seen[offers[0] + 1]));
        assert!(seen[offers[1] + 1].context.routine == "11571" && !rows(&seen[offers[1] + 1]).contains(&"Computers".to_string()), "Accept opens the computer list: {:?}", rows(&seen[offers[1] + 1]));
    }
    struct SpaceWindow { store: Store, ui: UiState, script: VecDeque<String>, log: std::rc::Rc<std::cell::RefCell<Vec<String>>>, shots: std::rc::Rc<std::cell::RefCell<Vec<Vec<u32>>>>, rgb: Vec<u32>, idle: usize, hw: VecDeque<Event>, held: usize, pictures: std::rc::Rc<std::cell::Cell<usize>> }
    impl SpaceWindow {
        fn serve(&mut self) {
            use crate::panels::{picture::Picture, Button, Component, UiEvent};
            self.store.restore(&mut self.ui);
            if self.store.playing() { return }
            if !self.hw.is_empty() { return }
            if let Some(step) = self.script.front().cloned().filter(|s| s.starts_with("key:") || s.starts_with("tap:") || s.starts_with("at:")) {
                if self.idle < 200 || !self.hw.is_empty() || self.store.pending() { return }
                if let Some(name) = step.strip_prefix("key:").or(step.strip_prefix("tap:")) {
                    use mt_engine::io::keys;
                    let k = match name { "esc" => keys::ESC, "enter" => keys::ENTER, "left" => keys::LEFT, "right" => keys::RIGHT, "up" => keys::UP, "down" => keys::DOWN, c if c.len() == 1 => c.as_bytes()[0].to_ascii_uppercase() as u16, _ => panic!("key {name}") };
                    self.hw.extend([Event::Key(k), Event::KeyUp(k)]); self.held = if step.starts_with("tap:") { 28 } else { 0 };
                } else {
                    let (x, y) = step[3..].split_once(',').map(|(x, y)| (x.parse().unwrap(), y.parse().unwrap())).unwrap();
                    let got = Picture::full().input(&mt1::ui::Scene::program(mt1::ui::ScreenKind::Space), &self.ui, &UiEvent::Up(Button::Left), (x, y));
                    let mut events: Vec<Event> = got.into_iter().filter_map(|i| if let Intent::Raw(e) = i { Some(e) } else { None }).collect();
                    if events.is_empty() { events.push(Event::Mouse(mt_engine::io::MouseEvent { event: 4, buttons: 0, col: x as i16, row: y as i16 })); }
                    if released_route(&events.iter().map(|e| Intent::Raw(*e)).collect::<Vec<_>>()).is_some() { self.store.queue(&mut self.ui, &events); } else { self.hw.extend(events); }
                }
                self.log.borrow_mut().push(format!("click {step}")); self.shots.borrow_mut().push(self.rgb.clone()); self.script.pop_front(); self.idle = 0;
                return
            }
            let (Some(d), Some(id)) = (self.ui.decision.clone(), self.script.front().cloned()) else { return };
            let Some(i) = d.options.iter().position(|o| o.id == id && o.enabled) else { return };
            let r = d.rects.iter().find(|(k, _)| *k == i).map(|r| r.1).or((d.kind == Kind::Ack).then_some([0, 0, 320, 200]))
                .unwrap_or_else(|| panic!("{}.{} publishes no rectangle for {id}: {:?}", d.context.program, d.context.routine, d.rects));
            let got = Picture::full().input(&mt1::ui::Scene::program(mt1::ui::ScreenKind::Space), &self.ui, &UiEvent::Up(Button::Left), (r[0] + r[2] / 2, r[1] + r[3] / 2));
            let events: Vec<Event> = got.into_iter().map(|i| if let Intent::Raw(e) = i { e } else { panic!("not a native route") }).collect();
            assert!(!events.is_empty(), "the click on {id} must answer {}.{}", d.context.program, d.context.routine);
            self.log.borrow_mut().push(format!("click {id}")); self.shots.borrow_mut().push(self.rgb.clone()); self.script.pop_front(); self.idle = 0;
            self.store.queue(&mut self.ui, &events);
        }
    }
    impl Input for SpaceWindow {
        fn wants_decision(&self) -> bool { self.store.wants(&self.ui) }
        fn decide(&mut self, d: &Decision) { self.store.record(&mut self.ui, d); self.log.borrow_mut().push(format!("{}.{} {}", d.context.program, d.context.routine, d.context.table.clone().unwrap_or_default())); }
        fn next_key(&mut self) -> u16 { self.serve(); self.store.next_key().expect("click has a key") }
        fn key_pending(&mut self) -> bool { self.serve(); self.store.key_pending() }
        fn poll(&mut self, clock: &mut dyn mt_engine::sim::tick::Clock) -> Event {
            self.serve();
            if let Some(e) = self.store.pop() { return e }
            if matches!(self.hw.front(), Some(Event::KeyUp(_))) && self.held < 30 { self.held += 1; }
            else if let Some(e) = self.hw.pop_front() { self.store.hardware(&mut self.ui, e); return e }
            self.idle += 1; assert!(self.idle < 20000, "stalled before {:?} at {:?}; log {:?}", self.script.front(), self.ui.decision.as_ref().map(|d| (d.context.routine.clone(), d.options.iter().map(|o| o.id.clone()).collect::<Vec<_>>())), { let l = self.log.borrow(); l[l.len().saturating_sub(12)..].to_vec() });
            clock.wait_tick(); Event::Idle
        }
        fn arrived(&mut self) -> bool { !self.store.delivered && self.hw.front().is_some_and(|e| !matches!(e, Event::KeyUp(_)) || self.held >= 30) }
        fn present(&mut self, rgb: &[u32]) { if self.rgb[..] != rgb[..] { self.rgb = rgb.to_vec(); self.store.refresh(); self.pictures.set(self.pictures.get() + 1); } }
        fn scene(&mut self, s: &dyn std::any::Any) { if s.is::<mt1::ui::Scene>() { self.store.refresh(); self.store.settle(); } }
    }
    thread_local! {
        static PICTURES: std::rc::Rc<std::cell::Cell<usize>> = Default::default(); }
    fn space_session(script: &[&str]) -> Option<(mt1::space::Space, Vec<String>, Vec<Vec<u32>>)> { space_session_on(script, false, false) }
    fn space_session_with(script: &[&str], hostile: bool) -> Option<(mt1::space::Space, Vec<String>, Vec<Vec<u32>>)> { space_session_on(script, hostile, false) }
    struct Hostile(bool);
    impl mt1::space::tick::Hooks for Hostile {
        fn update_cursor(&mut self, sp: &mut mt1::space::Space) {
            use mt1::space::state::{SHIP_COUNT, X, Y};
            if !self.0 { self.0 = true; if sp.s.word(SHIP_COUNT) < 1 { sp.s.set_word(SHIP_COUNT, 1); } sp.s.set_enc(0, 0, sp.s.word(X)); sp.s.set_enc(0, 2, sp.s.word(Y)); }
            sp.poll_input(); sp.update_software_cursor()
        }
    }
    struct Wall(mt_engine::sim::tick::Counter);
    impl mt_engine::sim::tick::Clock for Wall {
        fn now(&mut self) -> u32 { self.0.now() } fn set(&mut self, n: u32) { self.0.set(n) } fn wait_tick(&mut self) { self.0.wait_tick() }
        fn busy_wait(&mut self) { self.0.wait_tick() } fn ticks(&mut self) -> u32 { self.0.ticks() }
    }
    fn space_session_on(script: &[&str], hostile: bool, jump_ready: bool) -> Option<(mt1::space::Space, Vec<String>, Vec<Vec<u32>>)> {
        space_session_set(script, hostile, |sp| if jump_ready { sp.s.set_pbyte(0xAC, 3 | 0x20); sp.s.set_pbyte(0xAE, 4 | 0x20); })
    }
    fn look(name: &str, shots: &[Vec<u32>]) {
        let Some(dir) = std::env::var_os("MT_FX_JUMP_SHOTS") else { return };
        for (i, f) in shots.iter().enumerate() { let mut out = b"P6 320 200 255\n".to_vec(); out.extend(f.iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])); std::fs::write(std::path::Path::new(&dir).join(format!("{name}_{i}.ppm")), out).unwrap(); }
    }
    fn space_session_set(script: &[&str], hostile: bool, setup: impl FnOnce(&mut mt1::space::Space)) -> Option<(mt1::space::Space, Vec<String>, Vec<Vec<u32>>)> {
        use mt_engine::io::{Dir, Files};
        let dir = std::path::PathBuf::from(std::env::var_os("MT1_GAMEFILES").or_else(|| { eprintln!("SKIP: MT1_GAMEFILES unset"); None })?); let f = Dir(dir);
        let log: std::rc::Rc<std::cell::RefCell<Vec<String>>> = Default::default(); let shots: std::rc::Rc<std::cell::RefCell<Vec<Vec<u32>>>> = Default::default();
        let window = SpaceWindow { store: Store::default(), ui: UiState::default(), script: script.iter().map(|s| s.to_string()).collect(), log: log.clone(), shots: shots.clone(), rgb: Vec::new(), idle: 0, hw: VecDeque::new(), held: 0, pictures: PICTURES.with(|p| { p.set(0); p.clone() }) };
        let mut sp = mt1::space::Space::new(&f.read("S.DAT").unwrap(), f.read("DEFAULT.SAV").unwrap(), f.read("W.DAT").unwrap(), Box::new(window), Box::new(Wall(mt_engine::sim::tick::Counter::at(0))), Box::new(f)).unwrap();
        sp.product = true; if hostile { sp.hooks = Some(Box::new(Hostile(false))); }
        setup(&mut sp);
        sp.space_main(b"0A");
        let log = log.borrow().clone(); let shots = shots.borrow().clone(); Some((sp, log, shots))
    }
    fn answers(log: &[String]) -> Vec<(String, String)> {
        log.iter().enumerate().filter_map(|(i, l)| Some((l.strip_prefix("click ")?.to_string(), log.get(i + 1).cloned().unwrap_or_default()))).collect()
    }
    const LEAVE: [&str; 4] = ["back", "cmd:launch", "cmd:quit", "yes"];
    #[test]
    fn a_click_on_a_strip_cell_opens_that_station() {
        let script = ["cmd:stations", "cmd:station/3", "cmd:station/1", "cmd:station/2", "cmd:station/4", "cmd:station/0", "cmd:station/5", "continue"];
        let Some((_, log, _)) = space_session(&[&script[..], &LEAVE[..]].concat()) else { return };
        let got = answers(&log);
        let want = [("cmd:stations", "s.pilot station:10"), ("cmd:station/3", "s.121C station:13"), ("cmd:station/1", "s.9FDB station:11"), ("cmd:station/2", "s.9FDB station:12"),
            ("cmd:station/4", "s.engineer station:14"), ("cmd:station/0", "s.pilot station:10"), ("cmd:station/5", "s.notice "), ("continue", "s.pilot station:10"),
            ("back", "s.4FCB "), ("cmd:launch", "s.4FCB "), ("cmd:quit", "s.4ED0 "), ("yes", "")];
        assert_eq!(got, want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
    }
    struct Watch(Vec<u32>);
    impl mt1::space::tick::Hooks for Watch {
        fn update_cursor(&mut self, sp: &mut mt1::space::Space) {
            if sp.s.word(mt1::space::tick::ACTIVE + 2) != -1 {
                let deadline = sp.s.dword(mt1::space::tick::DEADLINES + 4);
                if self.0.last() != Some(&deadline) { self.0.push(deadline); }
                if sp.s.pbyte(0xC2) == 23 { assert_eq!(self.0.len(), 3, "armed once per point: {:?}", self.0); sp.dead = true; }
            }
            sp.poll_input(); sp.update_software_cursor()
        }
    }
    #[test]
    fn a_watched_repair_raises_the_condition() {
        let script = ["cmd:stations", "cmd:station/4", "cmd:repair", "menu:1c1c/0"];
        let Some((sp, log, _)) = space_session_set(&script, false, |sp| { sp.s.set_pbyte(0xC2, 25); sp.hooks = Some(Box::new(Watch(vec![]))); }) else { return };
        assert_eq!(sp.s.pbyte(0xC2), 23, "two points while watching: {log:#?}");
    }
    #[test]
    fn the_in_sys_travel_box_leaves_every_station() {
        for station in 0..8u16 {
            let cell = format!("cmd:station/{station}");
            let script = ["cmd:stations", cell.as_str(), "back", "cmd:launch", "cmd:quit", "yes"];
            let Some((_, log, _)) = space_session_set(&script, false, |sp| if station >= 5 { sp.s.set_pbyte(0xA4, 0); sp.s.set_pbyte(0xA0 + station, 5); }) else { return };
            let got = answers(&log);
            let back = got.iter().find(|(id, _)| id == "back").unwrap_or_else(|| panic!("station {station}: {log:#?}"));
            assert_eq!(back.1, "s.4FCB ", "station {station}: the box lands in flight: {log:#?}");
        }
    }
    #[test]
    fn a_click_on_an_unmanned_cell_says_assign_first() {
        let script = ["cmd:stations", "cmd:station/5", "continue", "cmd:station/6", "continue", "cmd:station/3", "cmd:station/7", "continue", "cmd:station/4"];
        let Some((sp, log, shots)) = space_session(&[&script[..], &LEAVE[..]].concat()) else { return };
        let want = [("cmd:stations", "s.pilot station:10"), ("cmd:station/5", "s.notice "), ("continue", "s.pilot station:10"),
            ("cmd:station/6", "s.notice "), ("continue", "s.pilot station:10"), ("cmd:station/3", "s.121C station:13"),
            ("cmd:station/7", "s.notice "), ("continue", "s.121C station:13"), ("cmd:station/4", "s.engineer station:14"),
            ("back", "s.4FCB "), ("cmd:launch", "s.4FCB "), ("cmd:quit", "s.4ED0 "), ("yes", "")];
        assert_eq!(answers(&log), want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
        let band = |f: &Vec<u32>| f[86 * 320..100 * 320].to_vec();
        assert!(band(&shots[2]) != band(&shots[1]), "the message is drawn");
        assert!(shots[2][..86 * 320] == shots[1][..86 * 320] && shots[2][100 * 320..] == shots[1][100 * 320..], "and only its own rows change");
        assert!(shots[3] == shots[1], "dismissed: the station's picture is as it was");
        if let Some(dir) = std::env::var_os("MT_FX_STATION_SHOTS") {
            for i in [2, 4, 7] { let mut out = b"P6 320 200 255\n".to_vec(); out.extend(shots[i].iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])); std::fs::write(std::path::Path::new(&dir).join(format!("no_one_{i}.ppm")), out).unwrap(); }
        }
        let crew: Vec<u8> = (0..12).map(|i| sp.s.pbyte(0xA0 + i)).collect();
        assert_eq!(crew, [1, 2, 3, 4, 5, 0, 0, 0, 0, 0, 0, 0], "nobody was moved");
    }
    #[test]
    fn the_jump_row_opens_the_map_and_the_map_marks_where_the_ship_is() {
        let Some((sp, log, shots)) = space_session_on(&["cmd:launch", "cmd:jump", "key:esc", "cmd:launch", "cmd:quit", "yes"], false, true) else { return };
        let got = answers(&log);
        let want = [("cmd:launch", "s.4FCB "), ("cmd:jump", "s.D7B4 station:-1"), ("key:esc", "s.4FCB "), ("cmd:launch", "s.4FCB "), ("cmd:quit", "s.4ED0 "), ("yes", "")];
        assert_eq!(got, want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
        assert_eq!(sp.s.pbyte(6), 2, "nobody jumped");
        let green = |right: bool| shots[2].iter().enumerate().filter(|(i, &p)| p & 0xFFFFFF == 0x00AA00 && (i % 320 >= 250) == right).count();
        assert!(green(false) > 200 && green(true) == 0, "the current system's label box is filled in its own colour: {} / {}", green(false), green(true));
    }
    #[test]
    fn the_jump_map_explains_itself_and_says_why_when_no_jump_is_possible() {
        let colour = |f: &Vec<u32>, rgb: u32, x: std::ops::Range<usize>, y: std::ops::Range<usize>| y.clone().flat_map(|y| x.clone().map(move |x| y * 320 + x)).filter(|&i| f[i] & 0xFFFFFF == rgb).count();
        let Some((sp, log, shots)) = space_session_on(&["cmd:launch", "cmd:jump", "back", "cmd:launch", "cmd:quit", "yes"], false, true) else { return };
        look("map", &shots);
        let got = answers(&log);
        assert_eq!(got[1..3], [("cmd:jump", "s.D7B4 station:-1"), ("back", "s.4FCB ")].map(|(a, b)| (a.to_string(), b.to_string())), "the drawn Cancel answers a click: {log:#?}");
        assert_eq!((sp.s.pbyte(6), sp.s.pword(0xF84)), (2, 0), "nobody jumped"); assert!(sp.s.pbyte(0xFC) >= 122, "no fuel went");
        let map = &shots[2];
        let (green, dim, blue) = (colour(map, 0x00AA00, 0..250, 30..170), colour(map, 0x555555, 100..200, 30..95), colour(map, 0x5555FF, 40..170, 100..170));
        assert!(green > 200 && dim > 800 && blue > 600, "here green {green}, out of reach dimmed {dim}, in reach blue {blue}");
        assert!(colour(map, 0xFFFFFF, 5..245, 11..20) > 150, "the instruction line stands above the first star");
        assert!(colour(map, 0xAAAAAA, 190..242, 157..168) > 300, "the Cancel box is drawn");
        let Some((sp, log, shots)) = space_session_set(&["cmd:launch", "cmd:jump", "continue", "cmd:launch", "cmd:quit", "yes"], false, |sp| { sp.s.set_pbyte(0xAC, 3 | 0x20); sp.s.set_pbyte(0xAE, 4 | 0x20); sp.s.set_pbyte(0xFC, 61); }) else { return };
        look("nojump", &shots);
        let got = answers(&log);
        assert_eq!(got[1..3], [("cmd:jump", "s.notice "), ("continue", "s.4FCB ")].map(|(a, b)| (a.to_string(), b.to_string())), "the reason is a message of its own; a click goes back: {log:#?}");
        assert!(!log.iter().any(|l| l.contains("D7B4")), "no selector: nothing flashes as if it could be chosen: {log:#?}");
        assert_eq!((sp.s.pbyte(6), sp.s.pword(0xF84)), (2, 0), "no jump: system, day"); assert!(sp.s.pbyte(0xFC) >= 59, "the fuel is not halved (the take-off burns a ton): {}", sp.s.pbyte(0xFC));
        let why = &shots[2];
        assert!(colour(why, 0x5555FF, 40..200, 30..170) == 0, "every other system is dimmed");
        assert!(colour(why, 0xFFFFFF, 12..244, 14..60) > 600, "the sentence stands over the map");
    }
    #[test]
    fn the_jump_is_asked_before_and_told_after() {
        let white = |f: &Vec<u32>, y: std::ops::Range<usize>| y.flat_map(|y| (12..244usize).map(move |x| y * 320 + x)).filter(|&i| f[i] & 0xFFFFFF == 0xFFFFFF).count();
        let Some((_, _, _)) = space_session_on(&["cmd:launch", "cmd:jump", "key:esc", "cmd:launch", "cmd:quit", "yes"], false, true) else { return };
        let without = PICTURES.with(|p| p.get());
        let Some((sp, log, shots)) = space_session_on(&["cmd:launch", "cmd:jump", "menu:d7b4/0", "no", "menu:d7b4/0", "yes", "continue", "cmd:quit", "yes"], false, true) else { return };
        assert!(PICTURES.with(|p| p.get()) > without + 60, "the film is seen in the window: {} pictures with the jump, {without} without", PICTURES.with(|p| p.get()));
        look("jump", &shots);
        let want = [("cmd:jump", "s.D7B4 station:-1"), ("menu:d7b4/0", "s.jump.confirm "), ("no", "s.D7B4 station:-1"), ("menu:d7b4/0", "s.jump.confirm "), ("yes", "s.notice "), ("continue", "s.4FCB "), ("cmd:quit", "s.4ED0 ")];
        assert_eq!(answers(&log)[1..8], want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
        assert!(sp.s.pbyte(6) == 3 && sp.s.pword(0xF84) == 7 && (55..=62).contains(&sp.s.pbyte(0xFC)), "in Louzy a week later on half the fuel: system {} day {} fuel {}", sp.s.pbyte(6), sp.s.pword(0xF84), sp.s.pbyte(0xFC));
        assert!(white(&shots[3], 14..60) > white(&shots[2], 14..60) + 300, "the question is drawn over the map");
        assert!(shots[4] == shots[2] || white(&shots[4], 20..60) == white(&shots[2], 20..60), "No: the map is as it was");
        assert!(white(&shots[6], 60..112) > 600, "the arrival is drawn over the view: {}", white(&shots[6], 60..112));
        assert!(white(&shots[7], 60..112) < 100, "and is gone when the flight goes on: {}", white(&shots[7], 60..112));
        let keys = ["cmd:launch", "key:j", "key:enter", "key:enter", "key:enter", "key:y", "key:esc", "cmd:quit", "yes"];
        let Some((sp, log, _)) = space_session_on(&keys, false, true) else { return };
        let next: Vec<String> = answers(&log).into_iter().map(|(_, w)| w).collect();
        assert_eq!(next[1..7], ["s.D7B4 station:-1", "s.jump.confirm ", "s.D7B4 station:-1", "s.jump.confirm ", "s.notice ", "s.4FCB "], "{log:#?}");
        assert!(sp.s.pbyte(6) == 3 && sp.s.pword(0xF84) == 7, "by keys: system {} day {}", sp.s.pbyte(6), sp.s.pword(0xF84));
    }
    #[test]
    fn a_click_on_a_jump_destination_starts_the_jump() {
        let Some((sp, log, _)) = space_session_on(&["cmd:launch", "cmd:jump", "menu:d7b4/0", "yes", "continue", "cmd:quit", "yes"], false, true) else { return };
        let got = answers(&log);
        assert_eq!(got[1], ("cmd:jump".to_string(), "s.D7B4 station:-1".to_string()), "{log:#?}");
        assert_eq!(got[2], ("menu:d7b4/0".to_string(), "s.jump.confirm ".to_string()), "the click answers the map: {log:#?}");
        assert!(sp.s.pbyte(6) != 2 && sp.s.pword(0xF84) == 7 && sp.s.pbyte(0xFC) < 70, "jumped: system {} day {} fuel {}", sp.s.pbyte(6), sp.s.pword(0xF84), sp.s.pbyte(0xFC));
    }
    #[test]
    fn the_computer_lists_answer_the_row_clicked() {
        let Some((sp, log, _)) = space_session(&["cmd:stations", "cmd:station/3", "cmd:load", "menu:d7b4/0", "menu:d7b4/0", "cmd:load", "menu:d7b4/2", "menu:d7b4/1", "back", "cmd:launch", "cmd:quit", "yes"]) else { return };
        let got = answers(&log);
        let want = [("cmd:load", "s.D7B4 station:13"), ("menu:d7b4/0", "s.D7B4 station:13"), ("menu:d7b4/0", "s.121C station:13"), ("cmd:load", "s.D7B4 station:13"), ("menu:d7b4/2", "s.D7B4 station:13"), ("menu:d7b4/1", "s.121C station:13")];
        assert_eq!(got[2..8], want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
        let programs: Vec<u8> = (0..5).map(|i| sp.s.pbyte(0xAC + i)).collect();
        assert_ne!(programs, [3, 0, 4, 1, 5], "the picks changed the computer: {programs:?}");
    }
    #[test]
    fn the_aim_and_the_tactical_map_answer_clicks() {
        let script = ["cmd:stations", "cmd:station/1", "cmd:target", "cmd:fire", "cmd:station/0", "cmd:tactical", "back",
            "cmd:assign", "menu:1c1c/4", "menu:1c1c/5", "cmd:identify", "ship/0", "key:esc", "key:esc", "cmd:quit", "yes"];
        let Some((_, log, _)) = space_session_with(&script, true) else { return };
        let got = answers(&log);
        let next = |click: &str| got.iter().find(|(c, _)| c == click).map(|(_, w)| w.as_str()).unwrap_or_else(|| panic!("{click} not clicked: {log:#?}"));
        assert_eq!((next("cmd:target"), next("cmd:fire")), ("s.955C station:11", "s.9FDB station:11"), "Target aims, Fire fires: {log:#?}");
        assert_eq!((next("cmd:tactical"), next("back")), ("s.6743.view station:10", "s.pilot station:10"), "the map opens and a click closes it: {log:#?}");
        assert_eq!((next("cmd:identify"), next("ship/0")), ("s.6743.select station:15", "s.communications station:15"), "the ship is a row: {log:#?}");
    }
    #[test]
    fn the_arrow_keys_turn_the_gun() {
        use mt1::space::combat::GUN_DIRECTION;
        for hostile in [false, true] {
            for open in [&["cmd:stations", "cmd:station/1"][..], &["cmd:stations", "key:v", "key:right", "key:enter"][..]] {
                let turns = ["tap:right", "tap:right", "tap:right", "tap:right", "tap:left", "key:right", "cmd:left", "cmd:right"];
                let fire = ["key:t", "key:enter"];
                let leave = if hostile { &["key:esc", "cmd:quit", "yes"][..] } else { &["key:esc", "cmd:launch", "cmd:quit", "yes"][..] };
                let Some((sp, log, shots)) = space_session_with(&[open, &turns[..], &fire[..], leave].concat(), hostile) else { return };
                let tail = &log[log.len().saturating_sub(24)..];
                assert_eq!(sp.s.word(GUN_DIRECTION), 4, "bearing 0, 7, 6, 5, 4 by four taps, 5 by the tap back, 4, 5, 4 ({open:?}, hostile {hostile}): {tail:?}");
                let first = open.len();
                for i in 0..turns.len() { assert!(shots[first + i] != shots[first + i + 1], "turn {i} changed the picture ({open:?}, hostile {hostile})"); }
                let got = answers(&log);
                assert!(got.iter().any(|(c, w)| c == "key:t" && w == "s.955C station:11"), "T aims ({open:?}, hostile {hostile}): {tail:?}");
                assert!(got.iter().any(|(c, w)| c == "key:enter" && w != "s.955C station:11"), "Enter fires and the aim ends ({open:?}, hostile {hostile}): {tail:?}");
            }
        }
    }
    #[test]
    fn the_missile_gun_says_each_step() {
        struct Armed(Hostile);
        impl mt1::space::tick::Hooks for Armed {
            fn update_cursor(&mut self, sp: &mut mt1::space::Space) {
                if !(self.0).0 { sp.s.set_byte(mt1::space::combat::MISSILES, 3); sp.s.set_word(mt1::space::combat::CHARGE + 2, 100); }
                self.0.update_cursor(sp)
            }
        }
        let script = ["cmd:stations", "cmd:station/2", "at:150,175", "tap:right", "tap:right", "tap:right", "tap:right", "cmd:target", "menu:d7b4/0", "cmd:fire", "key:esc", "cmd:quit", "yes"];
        let Some((_, log, shots)) = space_session_set(&script, true, |sp| { sp.s.set_pbyte(0xC9, 9); sp.s.set_pbyte(0xAC, 2 | 0x20); sp.s.set_pbyte(0xAD, 1 | 0x20); sp.hooks = Some(Box::new(Armed(Hostile(false)))); }) else { return };
        let bar = |i: usize| (89..100).flat_map(|y| shots[i][y * 320 + 124..y * 320 + 313].to_vec()).collect::<Vec<u32>>();
        if let Some(dir) = std::env::var_os("MT_QF12_SHOTS") { for i in [2, 3, 7, 8, 9] { let mut out = b"P6 320 200 255
".to_vec(); out.extend(shots[i].iter().flat_map(|p| [(p >> 16) as u8, (p >> 8) as u8, *p as u8])); std::fs::write(std::path::Path::new(&dir).join(format!("gun_{i}.ppm")), out).unwrap(); } }
        let lines = [bar(3), bar(7), bar(8), bar(9)];
        for a in 0..4 { for b in a + 1..4 { assert!(lines[a] != lines[b], "steps {a} and {b} read the same: {log:#?}"); } }
        assert!(bar(2) == bar(7), "the out-of-order words go again with the next key: {log:#?}");
        let got = answers(&log);
        assert!(got.iter().any(|(c, w)| c == "cmd:target" && w == "s.D7B4 station:12"), "Target asks for the tube: {log:#?}");
        assert!(got.iter().any(|(c, w)| c == "menu:d7b4/0" && w == "s.955C station:12"), "the tube opens the sight: {log:#?}");
    }
    #[test]
    fn a_flight_row_answers_a_click_every_time() {
        const STATIONS: &str = "at:283,72"; const COMMO: &str = "at:283,84"; const PAUSE: &str = "at:283,120";
        let script = [STATIONS, "key:esc", STATIONS, "key:esc", STATIONS, "key:esc", COMMO, COMMO, STATIONS, "key:esc", PAUSE, PAUSE, STATIONS, "key:esc", "cmd:launch", "cmd:quit", "yes"];
        let Some((_, log, shots)) = space_session(&script) else { return };
        let got = answers(&log);
        let opened: Vec<&str> = got.iter().filter(|(c, _)| c == &STATIONS[..]).map(|(_, w)| w.as_str()).collect();
        assert_eq!(opened, ["s.pilot station:10"; 5], "every click on the Stations row opens the stations: {log:#?}");
        let white = |i: usize| (86..114).flat_map(|y| (8..248).map(move |x| (x, y))).filter(|&(x, y)| shots[i][y * 320 + x] & 0xFFFFFF == 0xFFFFFF).count();
        assert!(white(7) > white(6) + 200 && white(8) < white(6) + 50, "Commo: its message, then flight again: {} {} {}", white(6), white(7), white(8));
        assert!(white(11) > white(10) + 40 && white(12) < white(10) + 20, "Pause: the badge, then flight again: {} {} {}", white(10), white(11), white(12));
    }
    #[test]
    fn assign_first_then_a_station_assigns() {
        let script = ["cmd:stations", "cmd:assign", "menu:1c1c/4", "menu:1c1c/5", "cmd:station/0", "cmd:station/5",
            "cmd:assign", "menu:1c1c/3", "menu:1c1c/6", "cmd:station/0", "cmd:station/6", "cmd:assign", "menu:1c1c/2", "menu:1c1c/7", "cmd:station/0", "cmd:station/7"];
        let Some((sp, log, _)) = space_session(&[&script[..], &LEAVE[..]].concat()) else { return };
        let got: Vec<(String, String)> = answers(&log).into_iter().filter(|(c, _)| !c.starts_with("menu:") || c.ends_with("/5") || c.ends_with("/6") || c.ends_with("/7")).collect();
        let want = [("cmd:stations", "s.pilot station:10"), ("cmd:assign", "s.1C1C station:10"), ("menu:1c1c/5", "s.communications station:15"), ("cmd:station/0", "s.pilot station:10"), ("cmd:station/5", "s.communications station:15"),
            ("cmd:assign", "s.1C1C station:15"), ("menu:1c1c/6", "s.medical station:16"), ("cmd:station/0", "s.pilot station:10"), ("cmd:station/6", "s.medical station:16"),
            ("cmd:assign", "s.1C1C station:16"), ("menu:1c1c/7", "s.medical station:17"), ("cmd:station/0", "s.pilot station:10"), ("cmd:station/7", "s.medical station:17"),
            ("back", "s.4FCB "), ("cmd:launch", "s.4FCB "), ("cmd:quit", "s.4ED0 "), ("yes", "")];
        assert_eq!(got, want.map(|(a, b)| (a.to_string(), b.to_string())), "{log:#?}");
        let crew: Vec<u8> = (0..12).map(|i| sp.s.pbyte(0xA0 + i)).collect();
        assert!(crew[..7] == [1, 2, 0, 0, 0, 5, 4] && crew[7..].contains(&3), "Engineering's traveller mans Communications, Computer's Medical, Gunner 2's a Crew berth: {crew:?}");
    }
    #[test]
    fn refuel_bill_words_carry_their_key_letters_and_esc_is_a_key_cap() {
        use mt_engine::present::textlog::{Run, TextLog};
        let mut d = Decision::new(Kind::Menu, "t", "16374");
        for (id, label, k) in [("fuel", "Buy one unit", b'F' as u16), ("all", "Fill tank", b'A' as u16), ("pay", "Pay", b'P' as u16), ("back", "Cancel", keys::ESC)] { d.options.push(Opt::new(id, label, vec![Event::Key(k)])); }
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs = Vec::new();
        for (text, x, y) in [("Fuel:", 10, 20), ("Fill", 10, 60), ("All", 70, 60), ("Pay", 130, 60), ("Esc or Pay", 10, 80)] { let mut cx = x; for c in text.bytes() { if c != b' ' { runs.push(glyph(cx, y, c)); } cx += 6; } }
        let mut log = TextLog { visible: runs, ..TextLog::default() }; flag_hotkeys(&mut log, Some(&d));
        let flagged: Vec<(i64, i64)> = log.visible.iter().filter(|r| r.colour & crate::pipeline::text::UNDERLINE != 0).map(|r| (r.x, r.y)).collect();
        assert_eq!(flagged, [(10, 60), (70, 60), (130, 60)], "F of Fill, A of All, P of Pay; never the readout or the prose");
        let caps: Vec<(i64, i64)> = log.visible.iter().filter(|r| r.colour & crate::pipeline::text::KEYCAP != 0).map(|r| (r.x, r.y)).collect();
        assert_eq!(caps, [(10, 80), (16, 80), (22, 80)], "Esc boxed, 'or Pay' not");
    }
    #[test]
    fn esc_or_pay_words_answer_and_money_reads_sign_first() {
        use mt_engine::present::textlog::{Run, TextLog};
        let mut d = Decision::new(Kind::Menu, "t", "shipyard.pay");
        for (id, label, k) in [("pay", "Pay", b'P' as u16), ("back", "Cancel", keys::ESC)] { d.options.push(Opt::new(id, label, vec![Event::Key(k)])); }
        let glyph = |x: i64, y: i64, c: u8, w: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: w, descend: false, covered: false, background_width: w as u16, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs = Vec::new(); let mut x = 162;
        for (c, w) in [(b'o', 6), (b'f', 5), (b' ', 4), (b'5', 7), (b'1', 3), (b'2', 6), (b'0', 7), (b'$', 6), (b'.', 2)] { runs.push(glyph(x, 14, c, w)); x += w as i64; }
        let mut x = 162;
        for (c, w) in [(b'E', 7), (b's', 6), (b'c', 6), (b' ', 4), (b'o', 6), (b'r', 6), (b' ', 4), (b'P', 7), (b'a', 7), (b'y', 6)] { runs.push(glyph(x, 22, c, w)); x += w as i64; }
        let rows = native_rows(&runs, &d);
        assert_eq!(rows.iter().map(|(i, r)| (*i, r.x, r.w)).collect::<Vec<_>>(), [(0, 201, 20), (1, 162, 19)], "Pay, then the Esc word");
        assert_eq!(native_click(&runs, &d, (210, 26)), option(&d, 0)); assert_eq!(native_click(&runs, &d, (170, 26)), option(&d, 1)); assert!(native_click(&runs, &d, (190, 26)).is_empty(), "'or' is nothing");
        let mut log = TextLog { visible: runs, ..TextLog::default() }; flag_hotkeys(&mut log, Some(&d));
        let caps: Vec<i64> = log.visible.iter().filter(|r| r.colour & crate::pipeline::text::KEYCAP != 0).map(|r| r.x).collect(); assert_eq!(caps, [162, 169, 175]);
        let under: Vec<(i64, i64)> = log.visible.iter().filter(|r| r.colour & crate::pipeline::text::UNDERLINE != 0).map(|r| (r.x, r.y)).collect(); assert_eq!(under, [(201, 22)], "P of Pay");
        let money: Vec<(i64, u8)> = { let mut m: Vec<_> = log.visible.iter().filter(|r| r.y == 14 && r.x >= 177).map(|r| (r.x, r.bytes[0])).collect(); m.sort(); m };
        assert_eq!(money, [(177, b'$'), (183, b'5'), (190, b'1'), (193, b'2'), (199, b'0'), (206, b'.')], "$5120. in the same span, the right edge kept");
    }
    #[test]
    fn money_sign_moves_only_a_digit_run_glued_to_its_dollar() {
        use mt_engine::present::textlog::Run;
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs = vec![glyph(10, 0, b'3'), glyph(16, 0, b'0'), glyph(22, 0, b'$'), glyph(40, 0, b'8'), glyph(46, 0, b'0'), glyph(60, 0, b'$'), glyph(10, 10, b'7'), glyph(20, 10, b'$'), glyph(0, 20, b'$'), glyph(10, 20, b'4'), glyph(16, 20, b'5'), glyph(22, 20, b'$')];
        money_sign_first(&mut runs);
        let laid: Vec<(i64, i64, u8)> = runs.iter().map(|r| (r.y, r.x, r.bytes[0])).collect();
        assert_eq!(laid, [(0, 16, b'3'), (0, 22, b'0'), (0, 10, b'$'), (0, 40, b'8'), (0, 46, b'0'), (0, 60, b'$'), (10, 10, b'7'), (10, 20, b'$'), (20, 0, b'$'), (20, 16, b'4'), (20, 22, b'5'), (20, 10, b'$')]);
    }
    #[test]
    fn money_sign_keeps_the_first_digit_of_an_amount_after_a_gap() {
        use mt_engine::present::textlog::Run;
        let glyph = |x: i64, c: u8| Run { page: 0, x, y: 0, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs = vec![glyph(0, b'2'), glyph(24, b'1'), glyph(30, b'5'), glyph(36, b'0'), glyph(42, b'0'), glyph(48, b'$')];
        money_sign_first(&mut runs); runs.sort_by_key(|r| r.x);
        assert_eq!(runs.iter().map(|r| r.bytes[0]).collect::<Vec<_>>(), b"2$1500");
    }
    #[test]
    fn native_rows_come_from_the_drawn_labels_and_a_click_answers_that_option() {
        use mt_engine::present::textlog::Run;
        let labels = ["Start New Game", "Continue Old Game", "Get Started Quick", "Return to DOS"];
        let mut d = Decision::new(Kind::Menu, "cg", "519A");
        for (i, l) in labels.iter().enumerate() {
            let mut input = vec![Event::Key(keys::DOWN); i]; input.push(Event::Key(keys::ENTER));
            d.options.push(mt_engine::play::Option::new(format!("menu:{i}"), *l, input));
        }
        d.options.push(mt_engine::play::Option::new("back", "Cancel", vec![Event::Key(keys::ESC)]));
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut runs: Vec<Run> = "WELCOME".bytes().enumerate().map(|(k, c)| glyph(120 + k as i64 * 6, 40, c)).collect();
        for (i, l) in labels.iter().enumerate() { let mut x = 100; for c in l.bytes() { if c != b' ' { runs.push(glyph(x, 70 + i as i64 * 10, c)); } x += 6; } }
        let rows = native_rows(&runs, &d);
        assert_eq!(rows.iter().map(|r| r.0).collect::<Vec<_>>(), [0, 1, 2, 3]);
        for (i, _) in labels.iter().enumerate() {
            let at = (130, 70 + i as i32 * 10 + 4);
            assert_eq!(native_click(&runs, &d, at), option(&d, i), "row {i}"); assert!(!option(&d, i).is_empty());
        }
        assert_eq!(native_click(&runs, &d, (130, 60)), [] as [Intent; 0], "between the title and the first row");
        assert!(native_click(&runs, &d, (130, 42)).is_empty(), "the title is not a row");
        assert!(native_click(&runs, &d, (99, 74)).is_empty() && native_click(&runs, &d, (300, 74)).is_empty(), "beside the rows");
    }
    #[test]
    fn panel_buttons_answer_a_t_list_through_its_own_routes() {
        use mt_engine::{io::keys, play::{List, Row}};
        let mut d = Decision::new(Kind::List, "t", "11571");
        d.options.push(Opt::new("exit", "Exit", vec![Event::Key(keys::ESC)]));
        d.list = Some(List { rows: vec![Row { id: "list:11571/0".into(), label: "Alaric^400$".into(), enabled: true, price: 400, limit: 1, min_qty: 1, qty: 1, input: vec![] }],
            current: 0, more: false, money: 0, increase: vec![], decrease: vec![], submit: vec![Event::Key(keys::ENTER)] });
        let centre = |name: &str| { let r = PANEL_BUTTONS.iter().find(|b| b.0 == name).unwrap().1; (r.x + r.w as i32 / 2, r.y + r.h as i32 / 2) };
        assert_eq!(button_route(&d, centre("ESC")), raw(&[Event::Key(keys::ESC)]));
        assert_eq!(button_route(&d, centre("ENTER")), raw(&[Event::Key(keys::ENTER)]));
        assert_eq!(button_route(&d, centre("DOWN")), raw(&[Event::Key(keys::DOWN)]));
        assert!(button_route(&d, centre("+")).is_empty(), "a select-only list has no + route");
        assert!(button_route(&d, (100, 100)).is_empty() && button_route(&d, (259, 175)).is_empty(), "off the buttons");
        d.context.program = "cg".into(); assert!(button_route(&d, centre("ESC")).is_empty(), "the DS:1ED2 table is T's");
    }
    #[test]
    fn the_fuel_bills_esc_or_pay_line_answers_pay() {
        use mt_engine::{present::textlog::Run};
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut d = Decision::new(Kind::Menu, "t", "shipyard.pay");
        d.options.push(mt_engine::play::Option::new("pay", "Pay", vec![Event::Key(80)]));
        d.options.push(mt_engine::play::Option::new("back", "Cancel", vec![Event::Key(27)]));
        let mut runs: Vec<Run> = Vec::new(); let mut x = 165;
        for c in "Esc or Pay".bytes() { runs.push(glyph(x, 22, c)); x += 6; }
        assert_eq!(native_click(&runs, &d, (165 + 7 * 6 + 3, 26)), option(&d, 0), "Pay = pay");
        assert_eq!(native_click(&runs, &d, (168, 26)), option(&d, 1), "the drawn Esc is a boxed key cap AND a click target, it answers the Esc option (Cancel)");
        assert!(native_click(&runs, &d, (165 + 4 * 6 + 3, 26)).is_empty(), "'or' is nothing");
    }
    #[test]
    fn drawn_yes_no_answer_a_quit_or_continue_prompt() {
        use mt_engine::present::textlog::Run;
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut d = Decision::new(Kind::YesNo, "s", "4ED0");
        d.options.push(mt_engine::play::Option::new("quit", "Quit", vec![Event::Key(b'Y' as u16)]));
        d.options.push(mt_engine::play::Option::new("continue", "Continue flight", vec![Event::Key(b'N' as u16)]));
        let mut runs: Vec<Run> = "Yes".bytes().enumerate().map(|(k, c)| glyph(200 + k as i64 * 6, 120, c)).collect();
        runs.extend("No".bytes().enumerate().map(|(k, c)| glyph(240 + k as i64 * 6, 120, c)));
        assert_eq!(native_click(&runs, &d, (205, 124)), option(&d, 0), "Yes = Quit");
        assert_eq!(native_click(&runs, &d, (243, 124)), option(&d, 1), "No = Continue flight");
        assert!(native_click(&runs, &d, (225, 124)).is_empty(), "the gap answers nothing");
    }
    #[test]
    fn published_rectangles_are_the_rows_and_the_drawn_text_is_not_consulted() {
        use mt_engine::present::textlog::Run;
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        let mut d = Decision::new(Kind::Menu, "t", "published.test");
        d.options.push(Opt::new("a", "Alpha", vec![Event::Key(b'A' as u16)])); d.options.push(Opt::new("b", "Beta", vec![Event::Key(b'B' as u16)]));
        let runs: Vec<Run> = "Beta".bytes().enumerate().map(|(k, c)| glyph(10 + k as i64 * 6, 50, c)).collect();
        assert_eq!(native_click(&runs, &d, (12, 54)), option(&d, 1), "unpublished: the guess, and the screen lists itself");
        assert!(guessed_screens().contains(&"t.published.test".to_string()));
        let mut ack = Decision::new(Kind::Ack, "t", "ack.test"); ack.options.push(Opt::new("continue", "Continue", vec![Event::Key(32)]));
        native_rows(&runs, &ack); assert!(!guessed_screens().contains(&"t.ack.test".to_string()), "a click-anywhere wait has no rows to guess");
        d.rects = vec![(0, [100, 20, 40, 10]), (1, [100, 30, 40, 10]), (1, [0, 0, 0, 0])];
        assert_eq!(native_rows(&runs, &d).iter().map(|(i, r)| (*i, r.x, r.y, r.w, r.h)).collect::<Vec<_>>(), [(0, 100, 20, 40, 10), (1, 100, 30, 40, 10)]);
        assert_eq!(native_click(&runs, &d, (110, 24)), option(&d, 0)); assert_eq!(native_click(&runs, &d, (110, 34)), option(&d, 1));
        assert!(native_click(&runs, &d, (12, 54)).is_empty(), "published wins: the drawn word is no longer a guess");
    }
    #[test]
    fn horizontal_yes_no_on_one_line_are_two_rows() {
        use mt_engine::{io::keys, present::textlog::Run};
        let glyph = |x: i64, y: i64, c: u8| Run { page: 0, x, y, colour: 15, shadow: None, bytes: vec![c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![], bitmap: vec![] };
        for (a, b) in [("Yes", "No"), ("Enlist", "Draft")] {
            let mut d = Decision::new(Kind::YesNo, "cg", "56A6");
            d.options.push(mt_engine::play::Option::new("yes", a, vec![Event::Key(keys::ENTER)]));
            d.options.push(mt_engine::play::Option::new("no", b, vec![Event::Key(keys::RIGHT), Event::Key(keys::ENTER)]));
            let mut runs = Vec::new(); let mut x = 200;
            for c in a.bytes() { runs.push(glyph(x, 118, c)); x += 6 } x += 6; let bx = x;
            for c in b.bytes() { runs.push(glyph(x, 118, c)); x += 6 }
            let rows = native_rows(&runs, &d);
            assert_eq!(rows.len(), 2, "{a} {b}: {rows:?}");
            assert_eq!(native_click(&runs, &d, (203, 122)), option(&d, 0), "{a}");
            assert_eq!(native_click(&runs, &d, (bx as i32 + 3, 122)), option(&d, 1), "{b}");
            assert!(native_click(&runs, &d, (bx as i32 - 3, 122)).is_empty(), "the gap between the words answers nothing");
        }
    }

}
