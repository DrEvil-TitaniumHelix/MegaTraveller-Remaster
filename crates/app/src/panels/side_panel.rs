use super::{assets::Assets, components::SIDE_RECT, Component, Picture, Rect, Surface, UiEvent, UiState};
use mt1::ui::{Intent, Scene, ScreenKind, Target, Unit};
use mt1::ground::rules;
use std::sync::Arc;

const PANEL_END: i32 = 87; const ROW: i32 = 9; const COLS: usize = 21;
pub struct SidePanel { assets: Arc<Assets> }
impl SidePanel {
    pub fn new(assets: Arc<Assets>) -> Self { Self { assets } }
    fn traveller(&self, scene: &Scene, out: &mut Surface) {
        let sel = scene.squad.selected.min(4); let hand = scene.walkers[sel as usize].hand();
        let mut panel = self.assets.walker_background();
        self.assets.text_fit(&mut panel, &scene.party.name(sel), Rect::new(5, 9, 121, 7), 0);
        self.assets.text_fit(&mut panel, if hand & 1 == 0 { "primary weapon" } else { "secondary weapon" }, Rect::new(5, 17, 100, 7), 0);
        self.assets.text_fit(&mut panel, "+", Rect::new(116, 17, 10, 7), 0);
        if let Some((id, mags, rounds)) = scene.party.hand(sel, hand) {
            let item = self.assets.items.get(id as usize);
            let name = item.map(|i| String::from_utf8_lossy(&i.name).trim_end_matches('\0').to_owned())
                .or_else(|| rules::weapon(id).map(|w| w.name.to_owned())).unwrap_or_default();
            if let Some(item) = item { self.assets.item_icon(&mut panel, item.icon, (8, 25)); }
            self.assets.text_fit(&mut panel, "ammo", Rect::new(59, 28, 36, 8), 0);
            self.assets.text_fit(&mut panel, &mags.to_string(), Rect::new(98, 28, 27, 8), sel + 1);
            self.assets.text_fit(&mut panel, &name, Rect::new(28, 41, 98, 7), 0);
            self.assets.text_fit(&mut panel, &format!("Rounds {rounds}"), Rect::new(5, 49, 73, 7), sel + 1);
            if rounds == 0 && rules::weapon(id).is_some_and(|w| w.rds > 0 && !matches!(w.profile, rules::Profile::Melee | rules::Profile::Thrown)) {
                self.assets.text_fit(&mut panel, "EMPTY", Rect::new(80, 49, 46, 7), 12);
            }
        }
        for i in 0..3 {
            self.assets.text_fit(&mut panel, ["exp", "chm", "rad"][i], Rect::new(12 + i as i32 * 22, 57, 20, 7), 0);
            let n = scene.party.0.get(0x1CB + sel as usize * 256 + i).copied().unwrap_or(0);
            self.assets.text_fit(&mut panel, &n.to_string(), Rect::new(12 + i as i32 * 22, 70, 20, 8), sel + 1);
        }
        let spent = super::combat_bar::spent(scene);
        for (label, y) in [("fire", 57), ("reload", 67), ("throw", 77)] { self.assets.text_fit(&mut panel, label, Rect::new(79, y, 47, 8), if spent && label == "fire" { 8 } else { 0 }); }
        out.cover(&panel, 0, 0);
    }
    pub fn place_lines(&self, scene: &Scene) -> Vec<(String, u8)> {
        let (w, a) = (scene.party.0.get(6).copied().unwrap_or(255), scene.party.0.get(7).copied().unwrap_or(255));
        let mut rows = Vec::new();
        if let Some(n) = self.assets.places.area_name_index(w, a).and_then(|i| self.assets.ds_string(0x15E1, i as usize)) {
            let mut c = n.chars(); let n: String = c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default();
            rows.push((format!("place: {n}"), 0));
        }
        if let Some(b) = self.assets.places.get(w, a, scene.level) { rows.extend(wrap(&format!("in {b}"), COLS - 1).into_iter().map(|r| (r, 0))); }
        rows
    }
    fn place(&self, scene: &Scene, out: &mut Surface) {
        let wrapped = |l: Vec<(String, u8)>| l.into_iter().flat_map(|(t, c)| wrap(&t, COLS - 1).into_iter().map(move |r| (r, c))).collect::<Vec<_>>();
        let (place, clock) = (self.place_lines(scene), wrapped(clock_lines(scene)));
        let draw = |out: &mut Surface, rows: &[(String, u8)], y: i32, h: u32| {
            let mut box_ = Surface::new(126, h); box_.indexed.fill(7);
            for (i, (r, c)) in rows.iter().enumerate() { self.assets.text_fit(&mut box_, r, Rect::new(3, 1 + i as i32 * 8, 120, 6), *c); }
            out.cover(&box_, 1, y);
        };
        if counting(scene) {
            let (first, rest) = place.split_first().map_or((None, &[][..]), |(f, r)| (Some(f.clone()), r));
            if let Some(f) = first { draw(out, &[f], 1, 9); }
            let room = ((PANEL_END - LED_END - 1) / 8) as usize;
            let below: Vec<_> = if clock.len() + rest.len() <= room { clock.into_iter().chain(rest.iter().cloned()).collect() } else { clock.into_iter().take(room).collect() };
            draw(out, &below, LED_END, 8 * below.len() as u32 + 1);
            return
        }
        let rows: Vec<_> = place.into_iter().chain(clock).collect();
        draw(out, &rows, 1, if rows.len() > 1 { (8 * rows.len() as u32 + 1).max(39) } else { 9 });
    }
}
pub struct Deadline { pub left: fn(&Scene) -> Option<String>, pub passed: fn(&Scene) -> Option<String> }
fn counting(s: &Scene) -> bool { (0..=99).contains(&s.countdown) && s.screen == ScreenKind::Ground }
const LED_END: i32 = 52;
fn pbyte(scene: &Scene, at: usize) -> u8 { scene.party.0.get(at).copied().unwrap_or(0) }
pub const DEADLINES: &[Deadline] = &[
    Deadline { left: |s| s.context.arik.filter(|&t| t > 0).map(|t| format!("Rescue Arik: {} min left", (i32::from(t) + 59) / 60)),
        passed: |s| (pbyte(s, 6) == 1 && pbyte(s, 0xC1) & 64 != 0 && s.context.arik == Some(0)).then(|| "Arik was moved to Chiros".into()) },
    Deadline { left: |s| counting(s).then(|| format!("Self destruct: {}", s.countdown)), passed: |_| None },
];
pub fn clock_lines(scene: &Scene) -> Vec<(String, u8)> {
    let day = u16::from_le_bytes([pbyte(scene, 0xF84), pbyte(scene, 0xF85)]);
    let deadlines = DEADLINES.iter().filter_map(|d| (d.left)(scene).or_else(|| (d.passed)(scene)).map(|t| (t, 4)));
    if counting(scene) { return deadlines.chain(std::iter::once((format!("Day {day}"), 0))).collect() }
    std::iter::once((format!("Day {day}"), 0)).chain(deadlines).collect()
}
pub fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut rows = Vec::new(); let mut row = String::new();
    for word in text.split_whitespace().flat_map(|w| { let c: Vec<char> = w.chars().collect(); c.chunks(cols.max(1)).map(|p| p.iter().collect::<String>()).collect::<Vec<_>>() }) {
        if !row.is_empty() && row.chars().count() + 1 + word.chars().count() > cols { rows.push(std::mem::take(&mut row)); }
        if !row.is_empty() { row.push(' '); } row.push_str(&word);
    }
    if !row.is_empty() { rows.push(row); }
    rows
}
impl Component for SidePanel {
    fn draw(&self, scene: &Scene, ui: &UiState, out: &mut Surface) {
        Picture::new(SIDE_RECT).draw(scene, ui, out);
        if !scene.squad.active || scene.screen != ScreenKind::Ground { return }
        if !scene.combat { self.place(scene, out); return }
        self.traveller(scene, out);
        let sq = &scene.squad; let sel = sq.selected.min(4);
        out.regs[10] = ui.frame.regs[sel as usize + 1];
        let hand = scene.walkers[sel as usize].hand();
        let weapon = scene.party.hand(sel, hand).map(|(id, mags, rounds)| (rules::weapon(id), mags, rounds));
        let preview_on = !sq.preview.is_empty() && matches!(ui.hover, Some(Target::Unit(Unit::Npc(_))));
        let below = out.h as i32 >= SIDE_RECT.h as i32 + 3 * ROW;
        if sq.encounter && (below || !preview_on) {
            let mut labels = Surface::new(11, 27); labels.indexed.fill(7);
            self.assets.text_fit(&mut labels, "S", Rect::new(1, 1, 8, 8), 9);
            self.assets.text_fit(&mut labels, "P", Rect::new(1, 15, 8, 8), 9);
            out.cover(&labels, 66, PANEL_END);
            let mut readout = Surface::new(51, 27);
            self.assets.text_fit(&mut readout, &format!("MP {}", sq.mp[sel as usize]), Rect::new(1, 0, 49, 7), 15);
            let attack = sq.pips[sel.min(4) as usize][1];
            self.assets.text_fit(&mut readout, if attack { "Attack 1" } else { "Attack 0" }, Rect::new(1, 7, 49, 6), if attack { 15 } else { 8 });
            if let Some((Some(w), ..)) = weapon { if w.auto > 0 { self.assets.text_fit(&mut readout, if sq.full_auto[sel as usize] { "AUTO" } else { "SEMI" }, Rect::new(1, 13, 49, 7), 14); } }
            self.assets.text_fit(&mut readout, &sq.tactics, Rect::new(1, 20, 49, 7), 7);
            out.cover(&readout, 77, PANEL_END);
        }
        if preview_on {
            let y0 = if below { SIDE_RECT.h as i32 + 1 } else { PANEL_END };
            let rows: Vec<(String, u8)> = sq.preview.iter().enumerate().flat_map(|(i, l)| wrap(l, COLS).into_iter().map(move |r| (r, if i == 0 { 15 } else { 7 }))).collect();
            let n = rows.len().min(((out.h as i32 - y0).max(0) / ROW) as usize);
            let mut preview = Surface::new(out.w as u32, (n as i32 * ROW + 1).min(out.h as i32 - y0).max(0) as u32);
            for (i, (row, colour)) in rows.iter().take(n).enumerate() { self.assets.text_fit(&mut preview, row, Rect::new(2, 1 + i as i32 * ROW, out.w.saturating_sub(4) as u32, 8), *colour); }
            out.cover(&preview, 0, y0);
        }
    }
    fn input(&self, scene: &Scene, ui: &UiState, e: &UiEvent, at: (i32, i32)) -> Vec<Intent> {
        if !crate::mouse::decision::pointer_allowed(scene, "Side panel", e, at) { return vec![]; }
        Picture::new(SIDE_RECT).input(scene, ui, e, at)
    }
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
    use super::*;
    #[test]
    fn side_buttons_use_the_native_keyboard_commands_and_ignore_right_click() {
        use super::super::Button;
        use mt_engine::io::Event;
        use mt1::ground::{input, state::{HOTSPOTS_GROUND, HOTSPOTS_GROUND_N}};
        let Some(dir) = oracle_env::gamefiles() else { return };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/efate_broken.toml");
        let mut g = crate::scenario::load(&root, std::path::Path::new(&dir)).unwrap().game;
        let scene = Scene::capture(&g, ScreenKind::Ground, None);
        let c = SidePanel::new(Arc::default()); let ui = UiState::default();
        for (at, scan) in [((94, 60), 0x21), ((94, 70), 0x13), ((94, 80), 0x14)] {
            assert!(c.input(&scene, &ui, &UiEvent::Up(Button::Right), at).is_empty());
            let out = c.input(&scene, &ui, &UiEvent::Up(Button::Left), at);
            let [Intent::Raw(Event::Mouse(event))] = out.as_slice() else { panic!("native callback required: {out:?}") };
            input::mouse_callback(&mut g, *event);
            input::special_key(&mut g, HOTSPOTS_GROUND, HOTSPOTS_GROUND_N);
            assert_eq!(g.mem.key(), scan, "button {at:?} must dispatch the same command as the keyboard");
        }
    }
    #[test]
    fn readout_and_preview_rows() {
        let mut s = Scene::program(ScreenKind::Ground); s.squad.active = true; s.squad.encounter = true; s.combat = true; s.squad.mp = [6; 5]; s.squad.pools = (3, 1); s.squad.tactics = "Tactics 2".into();
        s.party.0 = vec![0; 0x1000]; s.party.0[0x1CE + 42] = 1; s.party.0[0xA82] = 12; s.party.0[0xA82 + 1] = 2;
        let c = SidePanel::new(Arc::default()); let ui = UiState::default();
        s.squad.pips[0] = [true, true]; let mut out = Surface::new(128, 146); c.draw(&s, &ui, &mut out);
        {
            let colour = |out: &Surface, y: i64| out.text.visible.iter().find(|r| r.y == y && r.x >= 79).map(|r| r.colour);
            assert_eq!(colour(&out, 57), colour(&out, 67), "fresh turn: fire and reload alike");
            let mut spent = s.clone(); spent.squad.pips[0] = [true, false]; let mut o2 = Surface::new(128, 146); c.draw(&spent, &ui, &mut o2);
            assert_ne!(colour(&o2, 57), colour(&o2, 67), "spent: fire greyed"); assert!(super::super::combat_bar::spent(&spent) && !super::super::combat_bar::spent(&s));
            let texts = |out: &Surface| -> String { let mut rows: Vec<_> = out.text.visible.iter().map(|r| (r.y, r.x, r.bytes[0] as char)).collect(); rows.sort(); rows.iter().map(|r| r.2).collect() };
            assert!(texts(&out).contains("Attack 1") && texts(&o2).contains("Attack 0"), "the Attack line under MP: {} / {}", texts(&out), texts(&o2));
        }
        assert!(out.text.visible.iter().any(|r| r.bytes == b"E" && r.x == 80 && r.y == 49), "EMPTY beside the rounds: {:?}", out.text.visible.iter().map(|r| (r.x, r.y, r.bytes[0] as char)).collect::<Vec<_>>());
        let texts = |out: &Surface| -> String { let mut rows: Vec<_> = out.text.visible.iter().map(|r| (r.y, r.x, r.bytes[0] as char)).collect(); rows.sort(); rows.iter().map(|r| r.2).collect() };
        assert!(texts(&out).contains("MP 6") && texts(&out).contains("SEMI") && texts(&out).contains("Tactics 2") && !texts(&out).contains("Tac 3-1"), "{}", texts(&out));
        s.squad.preview = vec!["Hostile (thug, NPC 14)".into(), "Short: Routine 7+".into(), "DM +3: skill 2, Dex +1, moving -1, cover".into(), "needs 4+ on 2D (91%)".into(), "High pen: full damage on a hit".into()];
        let ui = UiState { hover: Some(Target::Unit(Unit::Npc(14))), ..UiState::default() };
        let mut out = Surface::new(128, 146); c.draw(&s, &ui, &mut out);
        assert!(texts(&out).contains("Hostile (thug, NPC") && texts(&out).contains("needs 4+ on 2D (91%)") && !texts(&out).contains("MP 6"), "{}", texts(&out));
        let mut tall = Surface::new(128, 226); c.draw(&s, &ui, &mut tall);
        assert!(texts(&tall).contains("MP 6") && texts(&tall).contains("High pen: full damage") && tall.text.visible.iter().any(|r| r.y >= 148), "{}", texts(&tall));
        assert_eq!(wrap("DM +3: skill 2, Dex +1, moving -1, cover", 21), ["DM +3: skill 2, Dex", "+1, moving -1, cover"]);
        let plain = UiState::default(); let mut out = Surface::new(128, 146); c.draw(&s, &plain, &mut out); assert!(!texts(&out).contains("Hostile"), "no NPC under the pointer: no preview");
    }
    #[test]
    fn the_day_and_a_running_deadline() {
        let mut s = Scene::program(ScreenKind::Ground); s.party.0 = vec![0; 0x1000]; s.party.0[0xF84] = 12; s.party.0[6] = 1;
        let text = |s: &Scene| clock_lines(s).into_iter().map(|(t, _)| t).collect::<Vec<_>>();
        assert_eq!(text(&s), ["Day 12"]);
        s.context.arik = Some(300); assert_eq!(text(&s), ["Day 12", "Rescue Arik: 5 min left"]);
        s.context.arik = Some(121); assert_eq!(text(&s)[1], "Rescue Arik: 3 min left");
        s.context.arik = Some(1); assert_eq!(text(&s)[1], "Rescue Arik: 1 min left");
        s.context.arik = Some(0); assert_eq!(text(&s), ["Day 12"], "stopped in time (or never started): no line");
        s.party.0[0xC1] = 64; let before = s.clone(); assert_eq!(text(&s), ["Day 12", "Arik was moved to Chiros"]); assert_eq!(s, before);
        s.party.0[6] = 5; assert_eq!(text(&s), ["Day 12"], "away from Neaera's system the outcome is not repeated");
        s.countdown = 100; assert_eq!(text(&s), ["Day 12"], "100 = no countdown");
        s.countdown = 42; assert_eq!(text(&s), ["Self destruct: 42", "Day 12"], "the countdown in words, first");
        s.countdown = 0; assert_eq!(text(&s), ["Self destruct: 0", "Day 12"], "zero is still the countdown (the shake and the last second)");
    }
}
