use super::Key;
pub const ENTER: Key = 13;
pub const ESC: Key = 27;
pub const BACKSPACE: Key = 8;
pub const TAB: Key = 9;
pub const SPACE: Key = 32;
pub const fn ext(scan: u8) -> Key { 0x100 + scan as Key }
pub const UP: Key = ext(0x48);
pub const DOWN: Key = ext(0x50);
pub const LEFT: Key = ext(0x4B);
pub const RIGHT: Key = ext(0x4D);
pub const HOME: Key = ext(0x47);
pub const END: Key = ext(0x4F);
pub const PGUP: Key = ext(0x49);
pub const PGDN: Key = ext(0x51);
pub const F1: Key = ext(0x3B);
pub const F10: Key = ext(0x44);
#[cfg(test)]
mod t { use super::*; #[test] fn oracle_values() { assert_eq!((UP, DOWN, RIGHT, LEFT, ENTER, ESC), (0x148, 0x150, 0x14D, 0x14B, 13, 27)); } }
