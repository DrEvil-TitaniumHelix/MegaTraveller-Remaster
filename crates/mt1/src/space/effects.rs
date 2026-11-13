use super::Space;

impl Space {
    pub fn combat_palette(&mut self, index: u16, color: u8) {
        self.s.set_byte(0x168u16.wrapping_add(index), color);
        self.r.fb.set_reg(index as usize, color);
    }
    pub fn combat_effect(&mut self, index: u16) { self.play_sound(index); }
}
