use super::{band, Band};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Options { pub grenade_range: bool, pub npc_selective_autofire: bool }
impl Default for Options {
    fn default() -> Self { Self { grenade_range: true, npc_selective_autofire: true } }
}
impl Options {
    pub fn throw_band(self, squares: i32) -> Band {
        if self.grenade_range && (3..=5).contains(&squares) { Band::Short } else { band(squares) }
    }
    pub fn npc_bursts(self, auto: u8, ammo: u8, squares: i32, boxed: bool) -> bool {
        auto > 0 && ammo >= 2 && (!self.npc_selective_autofire || squares <= 5 || boxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn autofire_requires_the_row_and_supply_even_when_boxed_or_disabled() {
        for enabled in [false, true] { for distance in [4, 5, 6, 8] { for boxed in [false, true] {
            let options = Options { npc_selective_autofire: enabled, ..Default::default() };
            for (auto, ammo) in [(0, 20), (3, 0), (3, 1)] { assert!(!options.npc_bursts(auto, ammo, distance, boxed)); }
            assert_eq!(options.npc_bursts(3, 2, distance, boxed), !enabled || distance <= 5 || boxed);
        } } }
    }
}
