use super::{keys, Key};

pub const ESC: u8 = 0x01; pub const BACKSPACE: u8 = 0x0E; pub const TAB: u8 = 0x0F; pub const ENTER: u8 = 0x1C; pub const SPACE: u8 = 0x39;
pub const F1: u8 = 0x3B; pub const F10: u8 = 0x44; pub const SCROLL_LOCK: u8 = 0x46;
pub const HOME: u8 = 0x47; pub const UP: u8 = 0x48; pub const PGUP: u8 = 0x49; pub const LEFT: u8 = 0x4B; pub const RIGHT: u8 = 0x4D;
pub const END: u8 = 0x4F; pub const DOWN: u8 = 0x50; pub const PGDN: u8 = 0x51; pub const INS: u8 = 0x52; pub const DEL: u8 = 0x53;

pub fn bios_key(scan: u8, ascii: Option<u8>) -> Option<Key> {
    if scan == SCROLL_LOCK { return None }
    match ascii { Some(a) if a != 0 => Some(a as Key), _ => Some(keys::ext(scan)) }
}
pub fn make_code(key: Key) -> Option<u8> {
    if key >= 0x100 { return u8::try_from(key - 0x100).ok() }
    const ROW1: &[u8] = b"1234567890-="; const ROW2: &[u8] = b"qwertyuiop[]"; const ROW3: &[u8] = b"asdfghjkl;'`"; const ROW4: &[u8] = b"\\zxcvbnm,./";
    let c = (key as u8).to_ascii_lowercase();
    let row = |r: &[u8], base: u8| r.iter().position(|&x| x == c).map(|i| base + i as u8);
    match key { 13 => Some(ENTER), 27 => Some(ESC), 8 => Some(BACKSPACE), 9 => Some(TAB), 32 => Some(SPACE), 43 => Some(0x4E), 42 => Some(0x37),
        _ => row(ROW1, 0x02).or_else(|| row(ROW2, 0x10)).or_else(|| row(ROW3, 0x1E)).or_else(|| row(ROW4, 0x2B)) }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn bios_values() {
        assert_eq!((bios_key(ENTER, Some(13)), bios_key(ESC, Some(27)), bios_key(0x1E, Some(b'a')), bios_key(0x1E, Some(b'A'))), (Some(13), Some(27), Some(97), Some(65)));
        assert_eq!((bios_key(UP, None), bios_key(DOWN, None), bios_key(LEFT, None), bios_key(RIGHT, None), bios_key(PGUP, None)), (Some(0x148), Some(0x150), Some(0x14B), Some(0x14D), Some(0x149)));
        assert_eq!((bios_key(SCROLL_LOCK, None), bios_key(F1, None), bios_key(0x1E, Some(1)), bios_key(0, Some(233)), bios_key(UP, Some(0))), (None, Some(keys::F1), Some(1), Some(233), Some(0x148)));
    }
    #[test]
    fn make_codes() {
        assert_eq!((make_code(13), make_code(27), make_code(b'o' as Key), make_code(b'O' as Key), make_code(b'1' as Key), make_code(b'5' as Key), make_code(b'z' as Key)), (Some(ENTER), Some(ESC), Some(0x18), Some(0x18), Some(0x02), Some(0x06), Some(0x2C)));
        assert_eq!((make_code(0x148), make_code(0x150), make_code(0x14B), make_code(0x14D), make_code(0x13B), make_code(b'+' as Key), make_code(b'-' as Key), make_code(b'=' as Key)), (Some(UP), Some(DOWN), Some(LEFT), Some(RIGHT), Some(F1), Some(0x4E), Some(0x0C), Some(0x0D)));
        assert_eq!((make_code(233), make_code(0x1FF), make_code(0), make_code(1)), (None, Some(0xFF), None, None));
    }
}
