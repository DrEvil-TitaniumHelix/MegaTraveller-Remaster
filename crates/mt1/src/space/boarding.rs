use super::{Space, state::*, tick, guns::{StationUi, Rect, Result}};

#[cfg(test)]
mod tests;

impl Space {
    fn boarding_countdown_image(&mut self, step: i16) {
        self.encounter_layout("LED.LY", 12, 125, 140);
        self.encounter_layout("LED.LY", 10i16.wrapping_sub(step), 125, 140);
    }
    pub fn update_encounter_countdown(&mut self, mut step: i16, single: bool) -> i16 {
        if self.input.data_mode() && !self.play_time_wait && self.s.byte(0x4626) == 0 { return super::play::boarding_countdown(self,step,single); }
        if self.s.word(0x1CB) == 99 { return 99; }
        while self.ticks() < self.s.dword(tick::SNAPSHOT).wrapping_add(324) {
            while self.ticks() < self.s.dword(tick::SNAPSHOT).wrapping_add(step.wrapping_mul(36) as i32 as u32) {
                if self.s.byte(KEY) != 0 { return step; }
                self.update_cursor();
            }
            self.hide_cursor(); step = step.wrapping_add(1); self.r.front(&mut self.s);
            self.boarding_countdown_image(step); self.r.back(&mut self.s); self.show_cursor();
            if single && step < 10 { return step; }
        }
        self.s.set_dword(tick::SNAPSHOT, 65535); 0
    }
    pub fn draw_boarded_cargo(&mut self, own: &[Rect], other: &mut [Rect; 20], cargo: &mut [i16; 20], enemy: i16, initialize: bool, ui: &mut dyn StationUi) -> Result<()> {
        self.r.back(&mut self.s); self.r.layout(&self.s, 19, 83, 0);
        if self.s.word(0x1CB) != 99 { self.boarding_countdown_image(self.s.word(0x1CB)); }
        for (i, r) in own.iter().enumerate().take(20) { let value = self.s.pbyte(0xCA + i as u16); if value != 0 { self.encounter_icon(value as i16 + 19, r[0] + 1, r[1] / 8); } }
        let count = self.s.enemy(enemy, 2);
        let Some((image, slots)) = [2i16, 5, 8, 12, 20].into_iter().enumerate().find(|(_, n)| *n >= count) else {
            return Err("S:20D7 cargo count exceeds the original capacity table".into());
        };
        self.r.layout(&self.s, image as i16 + 25, 83, 0);
        let columns = if slots % 4 != 0 || slots == 20 { 5 } else { 4 };
        for i in 0..slots {
            let (y, x) = (i / columns * 17 + 113, i % columns * 16 + 224);
            other[i as usize] = [y, x, 18, 16];
            if initialize { cargo[i as usize] = if count > i { self.random(30) + 1 } else { 0 }; }
            if cargo[i as usize] != 0 { self.encounter_icon(cargo[i as usize] + 19, y + 1, x / 8); }
        }
        self.encounter_labels(0x6D4, 7, 83, 0); ui.call(self, "redraw_active_station", &[])?;
        self.r.flip(&mut self.s); ui.call(self, "redraw_active_station", &[])?; self.r.front(&mut self.s); Ok(())
    }
    fn boarding_damage(&mut self) {
        self.s.set_pbyte(0xC6, self.s.pbyte(0xC6).wrapping_add(60));
        if self.s.pbyte(0xC6) >= 100 { self.ship_destroyed(1); }
    }
    pub fn board_enemy_cargo(&mut self, enemy: i16, ui: &mut dyn StationUi) -> Result<()> {
        let own = self.encounter_rects(0x713, 20); let mut other = [[0; 4]; 20]; let mut cargo = [0; 20];
        let (mut enemy_selection, mut own_selection, mut key) = (0, 0, 1);
        self.s.set_word(0x1CB, if self.s.enemy(enemy, 24) == 1 { 0 } else { 99 });
        self.draw_boarded_cargo(&own, &mut other, &mut cargo, enemy, true, ui)?;
        self.snapshot_clock(); self.s.set_byte(0x4626, 1);
        while key != 27 {
            if self.input.data_mode() { super::play::station(self,"4707"); }
            let step = self.update_encounter_countdown(self.s.word(0x1CB), false); self.s.set_word(0x1CB, step);
            if step == 0 { self.boarding_damage(); key = 27; continue; }
            key = self.translate_station_key();
            if key == 84 {
                loop {
                    let from = ui.select(self, &other, self.s.enemy(enemy, 2), enemy_selection, None, true)?;
                    if from == -1 { break; }
                    if from == -2 { self.boarding_damage(); key = 27; break; }
                    enemy_selection = from + 1; if self.s.enemy(enemy, 2) < enemy_selection { enemy_selection = 0; }
                    let to = ui.select(self, &own, 20, own_selection, None, true)?;
                    if to == -1 { continue; }
                    if to == -2 { self.boarding_damage(); key = 27; break; }
                    own_selection = to + 1; if own_selection > 20 { own_selection = 0; }
                    let value = cargo.get_mut(from as usize).ok_or("S:236C invalid cargo selection")?;
                    let at = 0xCAu16.wrapping_add(to as u16); let old = self.s.pbyte(at); self.s.set_pbyte(at, *value as u8); *value = old as i16;
                    self.draw_boarded_cargo(&own, &mut other, &mut cargo, enemy, false, ui)?; break;
                }
            } else if key == 69 { key = 27; }
        }
        self.s.set_dword(tick::SNAPSHOT, 65535); self.s.set_byte(0x4626, 0); self.hide_cursor(); Ok(())
    }
}
