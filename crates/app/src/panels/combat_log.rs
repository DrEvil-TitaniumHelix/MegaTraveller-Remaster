use super::{assets::Assets, controls, Component, Picture, Surface, UiEvent, UiState, Button};
use mt1::ui::{Intent, Preset, Scene, Unit};
use mt_engine::present::textlog::Run;
use std::{collections::VecDeque, sync::Arc};

const LIMIT: usize = 256;
pub fn command_hint(scene: &Scene) -> Option<&'static str> {
    let sq = &scene.squad;
    if sq.menu.is_some() { return Some("Enter: default. Esc: close menu.") }
    match sq.acting {
        Some(Unit::Traveller(_)) => Some("Tab: target. F: fire. E: end turn. O: orders. M: menu. ?: help"),
        Some(_) => None,
        None => matches!(sq.queue.first(), Some(Unit::Traveller(_))).then_some("Space: party first."),
    }
}
#[derive(Clone, Debug)]
pub enum Line { Glyphs(Vec<Run>), Text(String) }
#[derive(Clone, Default)]
pub struct History { pub messages: VecDeque<Line>, last: Vec<u8>, seen: u32 }
impl History {
    fn push(&mut self, line: Line) { if self.messages.len() == LIMIT { self.messages.pop_front(); } self.messages.push_back(line); }
    pub fn observe(&mut self, frame: &crate::pipeline::Frame) {
        let mut glyphs: Vec<_> = frame.text.visible.iter().filter(|r| r.page == frame.text.page
            && (169..200).contains(&r.y) && (1..192).contains(&r.x)
            && r.colour == 6 && r.shadow.is_some()).take(128).cloned().collect();
        glyphs.sort_by_key(|r| (r.y, r.x));
        let signature: Vec<_> = glyphs.iter().flat_map(|r| r.bytes.iter().copied()).take(1024).collect();
        if signature == self.last { return } self.last = signature;
        if glyphs.is_empty() { return }
        self.push(Line::Glyphs(glyphs));
    }
    pub fn observe_squad(&mut self, sq: &mt1::ui::scene::Squad) {
        let restored = self.seen == 0 || sq.log_n < self.seen;
        if sq.log_n < self.seen { self.messages.clear(); self.last.clear(); }
        let log = if restored && !sq.log_history.is_empty() { &sq.log_history } else { &sq.log };
        let new = if restored { log.len() } else { (sq.log_n - self.seen) as usize }.min(log.len());
        for l in &log[log.len() - new..] { self.push(Line::Text(l.clone())); }
        self.seen = sq.log_n;
    }
    pub(super) fn rows(&self, width: usize) -> Vec<Line> {
        self.rows_without_status(width, "", "")
    }
    fn rows_without_status(&self, width: usize, status: &str, hover: &str) -> Vec<Line> {
        if width == 0 { return Vec::new() }
        let mut rows = Vec::new();
        for (i, message) in self.messages.iter().enumerate() {
            if i + 1 == self.messages.len() && matches!(message, Line::Text(t) if !t.is_empty() && (t == status || t == hover)) { continue; }
            match message {
                Line::Text(t) => rows.extend(balanced(t, (width / 6).max(1)).into_iter().map(Line::Text)),
                Line::Glyphs(message) => {
                    let mut row = Vec::new(); let mut x = 0usize; let mut prior_y = None;
                    for glyph in message {
                        let w = glyph.width.max(1) as usize;
                        if x + w > width || prior_y.is_some_and(|y| y != glyph.y) {
                            if !row.is_empty() { rows.push(Line::Glyphs(std::mem::take(&mut row))); } x = 0;
                        }
                        let mut glyph = glyph.clone(); prior_y = Some(glyph.y); glyph.x = x as i64; x += w; row.push(glyph);
                    }
                    if !row.is_empty() { rows.push(Line::Glyphs(row)); }
                }
            }
        }
        rows
    }
}
pub struct CombatLog { assets: Arc<Assets> }
pub(super) fn visible_lines(ui: &UiState, height: usize) -> usize {
    (height.saturating_sub(9) / 9).min(if ui.log_expanded { 20 } else { ui.log_lines.clamp(2, 20) as usize })
}
pub(super) fn header(scene: &Scene, ui: &UiState, width: usize) -> Vec<String> {
        let name = scene.party.name(scene.leader.clamp(0, 4) as u8);
        let mut status = if !scene.squad.strip.is_empty() { scene.squad.strip.clone() } else if scene.squad.encounter { format!("Round {}  {}", scene.squad.round, name) } else { format!("{}  {}", if scene.combat { "Party split" } else { "Exploration" }, name) };
        if matches!(scene.squad.acting, Some(Unit::Npc(_))) && ui.replay.is_some() { status.push_str(" (Space skips)") }
        if !scene.squad.hover.is_empty() { status = format!("{status} | {}", scene.squad.hover) }
        let cols = (width / 6).max(1);
        let fit = |s: &str| if s.chars().count() <= cols { vec![s.to_string()] } else { balanced(s, cols) };
        let mut head = fit(&status);
        if let Some(hint) = command_hint(scene).filter(|_| scene.squad.hover.is_empty() && controls::hint(scene, ui)) {
            match head.last_mut() {
                Some(last) if last.chars().count() + 3 + hint.len() <= cols => { last.push_str(" | "); last.push_str(hint) }
                _ => head.extend(fit(hint)),
            }
        }
        head
}
pub fn balanced(text: &str, cols: usize) -> Vec<String> {
    let rows = super::side_panel::wrap(text, cols);
    if rows.len() < 2 { return rows; }
    (1..cols).map(|w| super::side_panel::wrap(text, w)).find(|r| r.len() == rows.len()).unwrap_or(rows)
}
impl CombatLog { pub fn new(assets: Arc<Assets>) -> Self { Self { assets } } }
impl Component for CombatLog {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        if ui.preset == Preset::Classic { Picture::new(super::components::LOG_RECT).draw(scene, ui, out); return }
        out.indexed.fill(9);
        let head = header(scene, ui, out.w);
        for (i, line) in head.iter().enumerate() { self.assets.text(out, line, (2, i as i32 * 9), 15); }
        let rows = ui.components.log.rows_without_status(out.w.saturating_sub(4), &scene.squad.strip, &scene.squad.hover);
        let count = visible_lines(ui, out.h).saturating_sub(head.len().saturating_sub(1));
        let end = rows.len().saturating_sub((ui.log_scroll.max(0) as usize).min(rows.len().saturating_sub(count)));
        let start = end.saturating_sub(count);
        for (i, row) in rows[start..end].iter().enumerate() {
            let y = ((head.len().max(1) + i) * 9) as i32;
            let Line::Glyphs(row) = row else { if let Line::Text(t) = row { self.assets.text(out, t, (2, y), 14); } continue };
            for run in row {
                let mut r = run.clone(); r.page = 0; r.x += 2; r.y = y as i64;
                let (w, h) = (r.erase_width(), r.erase_height());
                if w == 0 || w > 256 || h > 16 || r.bitmap.len() != w * h || r.background.len() != w * h { continue }
                for y in 0..h { for x in 0..w {
                    let k = y * w + x; let bg = out.get(r.x as i32 + x as i32, r.y as i32 + y as i32).unwrap_or(9);
                    let pixel = if r.bitmap[k] == r.background[k] { bg } else { r.bitmap[k] };
                    r.background[k] = bg; r.bitmap[k] = pixel; out.put(r.x as i32 + x as i32, r.y as i32 + y as i32, pixel);
                } }
                out.text.runs.push(r.clone()); out.text.visible.push(r);
            }
        }
    }
    fn input(&self, _scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        match *e {
            UiEvent::Wheel(n) => vec![Intent::LogScroll(n)],
            UiEvent::Key(76) => vec![Intent::LogExpand],
            UiEvent::Key(k) => vec![Intent::Raw(mt_engine::io::Event::Key(k))],
            UiEvent::Move => {
                if let Some(d) = ui.component_input.drag.filter(|d| d.button == Button::Left && (0..4).contains(&d.start.1)) {
                    let lines = (d.log_lines as i64 + (d.start.1 as i64 - at.1 as i64) / 9).clamp(2, 20) as u8;
                    return vec![Intent::LogResize(lines)];
                }
                Vec::new()
            }
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::controls::{self, Drag};
    #[test]
    fn starport_labels_never_become_strip_messages() {
        let mut history = History::default(); let mut frame = crate::pipeline::Frame::default();
        let mut surface = Surface::new(320, 200);
        Assets::default().text(&mut surface, "Shipyardrefuelrepair", (1, 178), 6);
        frame.text = surface.text; history.observe(&frame);
        assert!(history.messages.is_empty());
        for run in &mut frame.text.visible { run.shadow = Some(9); }
        history.observe(&frame);
        assert_eq!(history.messages.len(), 1, "a real shadowed 1990 strip message is retained");
        for run in &mut frame.text.visible { run.shadow = Some(0); run.y = 177; run.bytes[0] ^= 1; }
        history.observe(&frame);
        assert_eq!(history.messages.len(), 2, "the strip message is kept whatever the shadow colour and row step");
    }
    #[test]
    fn current_status_is_drawn_once_but_kept_in_history() {
        let mut scene = Scene::program(mt1::ui::ScreenKind::Ground);
        scene.squad.strip = "no target: press Tab".into();
        let mut ui = UiState::default(); ui.components.log.messages.push_back(Line::Text(scene.squad.strip.clone()));
        let log = CombatLog::new(Arc::default()); let mut out = Surface::new(380, 45);
        log.draw(&scene, &ui, &mut out);
        let text: String = out.text.visible.iter().map(|r| r.bytes[0] as char).collect();
        assert_eq!(text.matches("no target: press Tab").count(), 1);
        assert_eq!(ui.components.log.messages.len(), 1);
        scene.squad.hover = std::mem::take(&mut scene.squad.strip);
        let mut out = Surface::new(380, 45); log.draw(&scene, &ui, &mut out);
        let text: String = out.text.visible.iter().map(|r| r.bytes[0] as char).collect();
        assert_eq!(text.matches("no target: press Tab").count(), 1);
        scene.squad.hover.clear();
        scene.squad.strip = "Your move".into(); let mut out = Surface::new(380, 45); log.draw(&scene, &ui, &mut out);
        let text: String = out.text.visible.iter().map(|r| r.bytes[0] as char).collect();
        assert!(text.contains("no target: press Tab"), "prior feedback remains available");
    }
    #[test]
    fn scroll_resize_and_expand_are_presentation_intents() {
        let c = CombatLog::new(Arc::default()); let s = Scene::program(mt1::ui::ScreenKind::Ground); let mut ui = UiState::default();
        assert_eq!(c.input(&s, &ui, &UiEvent::Wheel(3), (0, 0)), [Intent::LogScroll(3)]);
        assert_eq!(c.input(&s, &ui, &UiEvent::Key(76), (0, 0)), [Intent::LogExpand]); assert_eq!(c.input(&s, &ui, &UiEvent::Key(108), (0, 0)), [Intent::Raw(mt_engine::io::Event::Key(108))], "plain l goes to the game (Light)");
        controls::begin(&mut ui, &UiEvent::Down(Button::Left), (4, 1), (192, 36));
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (4, i32::MIN)), [Intent::LogResize(20)]);
        ui.component_input.drag = Some(Drag { button: Button::Left, start: (0, 1), camera: (0.0, 0.0), log_lines: 3 });
        assert_eq!(c.input(&s, &ui, &UiEvent::Move, (0, i32::MAX)), [Intent::LogResize(2)]);
    }
    #[test]
    fn scroll_is_bounded_at_both_ends() {
        let mut ui = UiState { log_lines: 2, ..UiState::default() };
        for i in 0..5 { ui.components.log.messages.push_back(Line::Text(format!("line {i}"))); }
        assert!(ui.apply(Intent::LogScroll(100))); assert_eq!(ui.log_scroll, 3, "5 rows, 2 visible: the oldest view starts at row 0");
        assert!(ui.apply(Intent::LogScroll(-100))); assert_eq!(ui.log_scroll, 0);
        assert!(ui.apply(Intent::LogScroll(1))); assert_eq!(ui.log_scroll, 1);
        ui.log_expanded = true; assert!(ui.apply(Intent::LogScroll(100))); assert_eq!(ui.log_scroll, 0, "20 rows shown: nothing older to scroll to");
    }
    #[test]
    fn history_deduplicates_and_is_bounded() {
        let mut h = History::default(); let mut f = crate::pipeline::Frame::default();
        for i in 0..300 { f.text.visible = vec![Run { page: 0, x: 1, y: 169, colour: 6, shadow: Some(9), bytes: format!("{i}").into_bytes(), font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![9; 48], bitmap: vec![6; 48] }]; h.observe(&f); h.observe(&f); }
        assert_eq!(h.messages.len(), LIMIT); assert_eq!(h.rows(6).len(), LIMIT);
        f.text.visible.clear(); h.observe(&f); assert_eq!(h.messages.len(), LIMIT);
    }
    #[test]
    fn visible_rows_scroll_without_changing_the_history() {
        let mut ui = UiState { log_lines: 2, ..UiState::default() };
        for c in [2, 3, 4] { ui.components.log.messages.push_back(Line::Glyphs(vec![Run { page: 0, x: 1, y: 169, colour: c, shadow: None, bytes: vec![b'A' + c], font_id: 0, width: 6, descend: false, covered: false, background_width: 6, background_height: 8, background: vec![9; 48], bitmap: vec![c; 48] }])); }
        let log = CombatLog::new(Arc::default()); let scene = Scene::program(mt1::ui::ScreenKind::Ground);
        let mut latest = Surface::new(192, 27); log.draw(&scene, &ui, &mut latest);
        assert_eq!((latest.get(2, 9), latest.get(2, 18)), (Some(3), Some(4)));
        ui.log_scroll = 1; let mut earlier = Surface::new(192, 27); log.draw(&scene, &ui, &mut earlier);
        assert_eq!((earlier.get(2, 9), earlier.get(2, 18)), (Some(2), Some(3)));
        assert_eq!(ui.components.log.messages.len(), 3);
        log.draw(&scene, &ui, &mut Surface::new(0, 0));
    }
    #[test]
    fn balanced_wrap_leaves_no_orphan_tail() {
        let rows = balanced("Your move: Alexandra. Tab: target. F: fire. E: end turn. O: orders. M: menu. (party 3 vs 0)", 36);
        assert_eq!(rows.len(), super::super::side_panel::wrap("Your move: Alexandra. Tab: target. F: fire. E: end turn. O: orders. M: menu. (party 3 vs 0)", 36).len());
        let shortest = rows.iter().map(|r| r.chars().count()).min().unwrap();
        assert!(shortest * 2 >= 36, "an orphan tail: {rows:?}");
        assert!(rows.iter().all(|r| r.chars().count() <= 36));
        assert_eq!(balanced("short", 36), ["short"]);
    }
    #[test]
    fn squad_lines_join_the_history_once_and_the_status_line_reads_the_turn() {
        let mut h = History::default(); let mut sq = mt1::ui::scene::Squad::default();
        sq.log = vec!["Encounter!".into(), "Round 1 - initiative: party (Tactics 0 vs 0)".into()]; sq.log_n = 2; h.observe_squad(&sq); h.observe_squad(&sq);
        assert_eq!(h.messages.len(), 2, "the same lines are not appended twice");
        sq.log.push("thug (NPC 14) fires the 9mm advanced combat rifle at Vlaris: short: skill 0, 2D 3+4 +1 = 8 vs 7 hit by 1 (hit)".into()); sq.log_n = 3; h.observe_squad(&sq);
        assert_eq!(h.messages.len(), 3); assert!(h.rows(192).len() > 3, "the long shot line wraps");
        sq.log = (0..8).map(|i| format!("line {i}")).collect(); sq.log_n = 20; h.observe_squad(&sq); assert_eq!(h.messages.len(), 11, "more lines than the Scene carries: the eight it has");
        let log = CombatLog::new(Arc::default()); let mut ui = UiState::default(); ui.components.log = h;
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.encounter = true; s.squad.round = 1; s.squad.strip = "NPC turn: thug (NPC 14)".into(); s.squad.acting = Some(Unit::Npc(14));
        ui.replay = Some((1, 0.2)); ui.encounters = 1;
        let mut out = Surface::new(600, 36); log.draw(&s, &ui, &mut out);
        let line: String = { let mut r: Vec<_> = out.text.visible.iter().filter(|r| r.y == 0).map(|r| (r.x, r.bytes[0] as char)).collect(); r.sort(); r.iter().map(|r| r.1).collect() };
        assert_eq!(line, "NPC turn: thug (NPC 14) (Space skips)", "no hint while an NPC acts: no key works");
        assert!(!line.contains("click"), "{line}");
        assert!(out.text.visible.iter().any(|r| r.y == 27 && r.bytes == b"l"), "the last squad line in the bottom row");
    }
    #[test]
    fn hint_per_prompt_names_only_working_keys_and_fits_the_log_panel() {
        let mut s = Scene::program(mt1::ui::ScreenKind::Ground); s.squad.encounter = true; s.squad.round = 1; s.squad.active = true;
        s.squad.strip = "Round 1 - party to act".into(); s.squad.queue = vec![Unit::Traveller(0), Unit::Npc(14)];
        let start = command_hint(&s).unwrap(); assert!(start.contains("Space") && !start.contains("E:") && !start.contains('?'), "{start}");
        s.squad.acting = Some(Unit::Traveller(0)); s.squad.strip = "Your move: Magellan".into();
        let turn = command_hint(&s).unwrap();
        for key in ["Tab:", "F:", "E:", "O:", "M:"] { assert!(turn.contains(key), "{turn}"); } assert!(!turn.contains("Space"), "{turn}");
        assert!(turn.len() <= 63, "one row at 1531 x 1221: {turn}"); assert!(turn.ends_with("?: help"), "the row ends with the help key: {turn}");
        s.squad.acting = Some(Unit::Npc(14)); assert_eq!(command_hint(&s), None);
        s.squad.acting = None; s.squad.queue = vec![Unit::Npc(14)]; s.squad.strip = "Round 1 - initiative: NPCs (Tactics 0 vs 1)".into();
        assert_eq!(command_hint(&s), None, "NPCs to act: no party key works");
        s.squad.menu = Some(mt1::ground::orders::Menu { target: mt1::ui::Target::Portrait(0), items: Vec::new(), default: 0 }); assert!(command_hint(&s).unwrap().starts_with("Enter: default."));
        s.squad.menu = None; s.squad.acting = Some(Unit::Traveller(0));
        s.squad.strip = "Your move: Magellan. Continue (O): reload or change weapon for thug (NPC 14), walk 6".into();
        let log = CombatLog::new(Arc::default()); let mut ui = UiState::default(); ui.encounters = 1;
        for width in [382u32, 512] {
            let mut out = Surface::new(width, 36); log.draw(&s, &ui, &mut out);
            let mut rows: std::collections::BTreeMap<i64, Vec<(i64, char)>> = Default::default();
            for r in &out.text.visible { rows.entry(r.y).or_default().push((r.x, r.bytes[0] as char)); }
            let lines: Vec<String> = rows.values().map(|r| { let mut r = r.clone(); r.sort(); r.iter().map(|c| c.1).collect() }).collect();
            let text = lines.join(" ");
            for line in &lines { assert!(line.chars().count() <= width as usize / 6, "{width}: cut: {line}"); }
            for word in s.squad.strip.split(' ').chain(turn.split(' ')) { assert!(text.contains(word), "{width}: {word} missing from {text}"); }
        }
        ui.encounters = 2; let mut out = Surface::new(382, 36); log.draw(&s, &ui, &mut out);
        assert!(!out.text.visible.iter().any(|r| r.y == 18), "after the first encounter the status alone: no hint row");
    }
}
