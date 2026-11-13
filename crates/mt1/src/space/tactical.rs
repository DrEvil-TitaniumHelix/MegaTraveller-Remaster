use super::{Space, state::*, combat::*, render::Render, tick};
use mt_engine::present::prim::Raster;

impl Space {
    pub fn combat_line(&mut self, y1: i16, x1: i16, y2: i16, x2: i16, color: u8) {
        self.r.fb.line(Raster::Wrap, Render::page(&self.s), y1 as i32, x1 as i32, y2 as i32, x2 as i32, color, 0);
    }
    pub fn redraw_tactical_radar(&mut self, mode: i16) -> i16 {
        self.hide_cursor();
        let mut colors = [self.s.pbyte(0xA1), self.s.pbyte(0xA2)];
        if self.s.word(VIDEO_MODE) == 2 { colors = [6; 2]; }
        let mut changed = 0;
        if self.ticks() > self.s.dword(tick::DEADLINES + 32) && self.s.word(ENCOUNTER) > 0 {
            let now = self.ticks(); self.s.set_dword(tick::DEADLINES + 32, now.wrapping_add(144));
            changed = self.advance_tactical_enemies();
        }
        self.s.set_word(TACTICAL_DRAWN, 1);
        if mode < 99 { self.copy_display_rectangle(0, 0, 200, 40); }
        self.r.back(&mut self.s); self.r.layout(&self.s, 0, 83, 0);
        for (gun, excluded) in [(0, 12), (1, 11)] {
            if self.s.word(STATION) != 15 && self.s.word(STATION) != excluded && self.s.pbyte(0xA1 + gun) != 0 {
                let direction = self.s.word(GUN_DIRECTION + 2 * gun);
                let previous = if direction == 0 { 7 } else { direction - 1 };
                for arc in [direction, previous] {
                    let at = 0xD96u16.wrapping_add((arc as u16).wrapping_mul(4));
                    self.combat_line(self.s.byte(at) as i16 + 88, self.s.byte(at.wrapping_add(1)) as i16 + 5,
                        self.s.byte(at.wrapping_add(2)) as i16 + 88, self.s.byte(at.wrapping_add(3)) as i16 + 5, colors[gun as usize]);
                }
            }
        }
        let (pr, pc) = (self.s.word(TACTICAL_X), self.s.word(TACTICAL_Y));
        if self.s.word(ENCOUNTER) > 0 {
            for row in pr.wrapping_sub(3)..pr.wrapping_add(3) { for col in pc.wrapping_sub(3)..pc.wrapping_add(3) {
                if !(0..10).contains(&row) || !(0..10).contains(&col) { continue; }
                let value = self.s.byte(grid_address(row, col)) as i16;
                if value > 0 && value < 100 {
                    self.r.layout(&self.s, self.s.enemy(value - 1, 0).wrapping_add(self.s.enemy(value - 1, 18)),
                        (col - (pc - 3)) * 15 + 85, (row - (pr - 3)) * 16 + 5);
                } else if (100..103).contains(&value) {
                    self.r.layout(&self.s, value + 80, (col - (pc - 3)) * 15 + 93, (row - (pr - 3)) * 16 + 13);
                }
            } }
        }
        self.r.layout(&self.s, self.s.word(HEADING).wrapping_add(110), 133, 54);
        self.r.layout(&self.s, 39, 88, 5);
        if mode < 99 { self.r.flip(&mut self.s); }
        self.show_cursor(); changed
    }
    fn map_reticle(&mut self, y: i16, x: i16, restore: bool) {
        let front = self.s.word(ACTIVE_PAGE) as usize & 1;
        if restore { self.copy(2, 187, 8, 13, 4, front, y, x / 8); }
        else { self.copy(front, y, x / 8, 13, 4, 2, 187, 8); self.r.layout(&self.s, 44, y, x); }
    }
    pub fn show_tactical_map(&mut self) -> Result<i16, String> {
        if self.s.word(ENCOUNTER) < 1 { return Ok(-1); }
        let page = Render::page(&self.s); self.copy(page, 0, 0, 200, 40, 3, 0, 0);
        self.s.set_word(TACTICAL_DRAWN, 1); self.r.back(&mut self.s);
        for col in 0..20 { for row in 0..12 {
            let tile = self.random(20); let page = Render::page(&self.s);
            self.copy(2, 96, tile * 2, 16, 2, page, row * 16, col * 2);
        } }
        for col in 0..20 { let tile = self.random(20); let page = Render::page(&self.s); self.copy(2, 96, tile * 2, 8, 2, page, 192, col * 2); }
        self.r.layout(&self.s, 24, 0, 0);
        let selecting = self.s.word(STATION_ACTIVE) != 0 && self.s.word(STATION) == 15;
        if selecting { let value = self.s.string(0x17D1); self.r.text(&self.s, &value, 186, 7, self.s.byte(0x4994)); }
        let mut entries = Vec::new();
        for row in 0..10 { for col in 0..10 {
            let value = self.s.byte(grid_address(row, col)) as i16;
            if value > 0 && value < 100 {
                let ship = value - 1; let (y, x) = (col * 18 + 8, row * 33 + 4);
                entries.push((ship, y, x));
                self.r.layout(&self.s, self.s.enemy(ship, 0).wrapping_add(self.s.enemy(ship, 18)), y, x);
            } else if (100..103).contains(&value) { self.r.layout(&self.s, value + 80, col * 18 + 16, row * 33 + 12); }
        } }
        self.r.layout(&self.s, self.s.word(HEADING).wrapping_add(110), self.s.word(TACTICAL_Y).wrapping_mul(18).wrapping_add(8), self.s.word(TACTICAL_X).wrapping_mul(33).wrapping_add(4));
        self.r.layout(&self.s, 39, 5, 5); self.r.flip(&mut self.s); self.r.front(&mut self.s);
        let mut key = 10;
        if selecting {
            if entries.is_empty() {
                if self.product { self.r.flip(&mut self.s); return Ok(-1); }
                return Err("S:6743 empty scan depends on caller stack residue".into());
            }
            let mut index = 0usize; let (_, y, x) = entries[index]; self.map_reticle(y, x, false);
            while key != 13 && key != 27 {
                if entries.len() > 1 && (1..=4).contains(&key) {
                    let (_, y, x) = entries[index]; self.map_reticle(y, x, true);
                    index = if key == 1 || key == 3 { (index + 1) % entries.len() } else { (index + entries.len() - 1) % entries.len() };
                    let (_, y, x) = entries[index]; self.map_reticle(y, x, false);
                }
                let played = self.product && self.observer.is_none() && !self.input.data_mode();
                if self.product { self.play_rects = entries.iter().map(|&(_, y, x)| [x as i32, y as i32, (320 - x as i32).min(32), 13]).collect(); }
                super::play::station_state(self,"6743.select",&(index as u32).to_le_bytes());
                self.play_rects.clear();
                if played {
                    while self.s.byte(KEY) == 0 && self.s.byte(PENDING_KEY) == 0 { self.update_cursor(); }
                    if self.s.byte(KEY) == 0x6E {
                        let (row, col) = (self.s.word(super::mouse::CALLBACK + 4), self.s.word(super::mouse::CALLBACK + 6));
                        if let Some(k) = entries.iter().position(|&(_, y, x)| (x..x + 32).contains(&col) && (y..y + 13).contains(&row)) {
                            self.s.set_byte(KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
                            let (_, y, x) = entries[index]; self.map_reticle(y, x, true);
                            index = k; let (_, y, x) = entries[index]; self.map_reticle(y, x, false);
                            key = 13; continue;
                        }
                    }
                }
                key = self.translate_station_key();
            }
            self.r.flip(&mut self.s); return Ok(if key == 27 { -1 } else { entries[index].0 });
        }
        while key != 88 && key != 27 { super::play::station(self,"6743.view"); key = self.translate_station_key(); }
        if self.s.word(VIDEO_MODE) == 0 { self.r.back(&mut self.s); self.r.flip(&mut self.s); }
        else {
            self.r.front(&mut self.s); let page = Render::page(&self.s); self.copy(3, 0, 0, 200, 40, page, 0, 0);
            self.r.back(&mut self.s); let page = Render::page(&self.s); self.copy(3, 0, 0, 200, 40, page, 0, 0);
        }
        Ok(-1)
    }
}
