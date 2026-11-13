use super::state::Ground;

pub const SEG: u16 = 0x8D00;
pub fn key(g: &Ground) -> [u8; 3] { [g.mem.u8(0x5E1F), g.mem.u8(0x5E20), g.mem.level() as u8] }
pub fn dims(g: &Ground) -> (u16, u16) { (g.mem.map_blocks_high() as u16 * 12, g.mem.map_stride().max(0) as u16) }
fn record(blob: &[u8], key: [u8; 3], (rows, cols): (u16, u16)) -> Option<usize> {
    let mut at = 0;
    while at + 7 <= blob.len() {
        let (r, c) = (u16::from_le_bytes([blob[at + 3], blob[at + 4]]), u16::from_le_bytes([blob[at + 5], blob[at + 6]]));
        let n = (r as usize * c as usize).div_ceil(8);
        if blob[at..at + 3] == key && (r, c) == (rows, cols) { return Some(at + 7) }
        at += 7 + n;
    }
    None
}
pub fn mark(g: &mut Ground) {
    if g.scene.is_none() { return }
    let (key, (rows, cols)) = (key(g), dims(g));
    if rows == 0 || cols == 0 { return }
    let (r0, c0) = ((g.mem.view_x() >> 1).max(0) as u16, (g.mem.view_y() >> 1).max(0) as u16);
    let blob = g.mem.segs.entry(SEG).or_default();
    let at = match record(blob, key, (rows, cols)) { Some(at) => at, None => {
        blob.extend_from_slice(&key); blob.extend_from_slice(&rows.to_le_bytes()); blob.extend_from_slice(&cols.to_le_bytes());
        let at = blob.len(); blob.resize(at + (rows as usize * cols as usize).div_ceil(8), 0); at } };
    for r in r0..(r0 + 7).min(rows) { for c in c0..(c0 + 12).min(cols) { let i = r as usize * cols as usize + c as usize; blob[at + i / 8] |= 1 << (i % 8) } }
}
pub fn current(g: &Ground) -> Vec<u8> {
    let (key, (rows, cols)) = (key(g), dims(g));
    let Some(blob) = g.mem.segs.get(&SEG) else { return Vec::new() };
    record(blob, key, (rows, cols)).map(|at| blob[at..at + (rows as usize * cols as usize).div_ceil(8)].to_vec()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bit(bits: &[u8], cols: u16, r: u16, c: u16) -> bool { let i = r as usize * cols as usize + c as usize; bits.get(i / 8).is_some_and(|b| b >> (i % 8) & 1 != 0) }
    #[test]
    fn seen_squares_accumulate_per_area_and_reach_the_diagnostic_state() {
        let mut g = crate::ground::orders::t::ground(); let (rows, cols) = dims(&g); assert_eq!((rows, cols), (36, 60));
        assert!(current(&g).is_empty(), "nothing seen yet");
        g.mem.set_view_x(38); g.mem.set_view_y(0); mark(&mut g);
        let bits = current(&g); assert_eq!(bits.len(), (36 * 60usize).div_ceil(8));
        assert!(bit(&bits, cols, 19, 0) && bit(&bits, cols, 25, 11) && !bit(&bits, cols, 26, 0) && !bit(&bits, cols, 19, 12) && !bit(&bits, cols, 18, 5));
        g.mem.set_view_x(50); g.mem.set_view_y(20); mark(&mut g); let bits = current(&g);
        assert!(bit(&bits, cols, 19, 0) && bit(&bits, cols, 31, 21), "a union over time");
        let level = g.mem.level(); g.mem.set_level(level + 1); mark(&mut g); assert!(!bit(&current(&g), cols, 19, 0), "another level: its own record"); g.mem.set_level(level);
        assert!(bit(&current(&g), cols, 19, 0), "the first area is intact");
        let d = crate::play::diagnostic::ground(&mut g).map.expect("the Plus window's map");
        assert_eq!((d.rows, d.cols, d.tiles.len(), d.party), (36, 60, 36 * 60 * 3, (22, 5))); assert!(bit(&d.seen, cols, 19, 0));
        g.scene = None; g.mem.set_view_x(0); mark(&mut g); assert!(!bit(&current(&g), cols, 0, 0), "faithful mode records nothing");
    }
}
