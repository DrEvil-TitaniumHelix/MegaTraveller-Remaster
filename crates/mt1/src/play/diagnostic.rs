use crate::{ground::{squad, Ground}, save::{Mode, Saved}};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Map { pub area: [u8; 3], pub rows: u16, pub cols: u16, pub tiles: Vec<u8>, pub party: (i16, i16), pub seen: Vec<u8> }
#[derive(Clone, Debug)]
pub struct State { pub hash: String, pub encounter: bool,
    pub map: Option<Map>,
    pub scenario_error: Option<String> }
pub fn ground(g: &mut Ground) -> State {
    State { hash: super::hash(g), encounter: squad::save_refusal(g).is_some(), map: Some(map(g)), scenario_error: g.tooling.as_ref().and_then(|t| t.last_error()) }
}
pub fn map(g: &Ground) -> Map {
    use crate::ground::{commands, seen};
    let (rows, cols) = seen::dims(g); let cell0 = commands::cell(g, 0, 0);
    let tiles = (0..rows as i32 * cols as i32 * 3).map(|i| g.mem.read(cell0.add(i), 1) as u8).collect();
    Map { area: seen::key(g), rows, cols, tiles, party: (g.mem.x() >> 1, g.mem.y() >> 1), seen: seen::current(g) }
}
pub fn space(sp: &mut crate::space::Space) -> State {
    State { hash: crate::space::play::hash(sp), encounter: sp.s.word(crate::space::state::ENCOUNTER) >= 0, map: None, scenario_error: None }
}
pub fn cg(s: &mut crate::screens::cg::Screens) -> State {
    State { hash: s.play_hash(), encounter: false, map: None, scenario_error: None }
}
pub fn offer_ground(g: &mut Ground) {
    if g.input.diagnostics() { let state = ground(g); g.input.scene(&state); }
}
pub fn anchor(g: &mut Ground) {
    if g.input.diagnostics() && !g.input.data_mode() && g.loop_vars.initialized
        && g.restore_pending.is_none() && !g.game_over
        && squad::save_refusal(g).is_none() {
        let saved = Saved::capture(g, Mode::Ground, "");
        g.input.anchor("window:t.6923", &saved.encode());
    }
}
