use super::{Space, state::*, system::*};

const SPRITE_W: u16 = 0; const VARIANT: u16 = 2; const WEAPON: u16 = 4; const FACTION: u16 = 14; const HULL: u16 = 16;
const E_DIR: u16 = 18; const E_X: u16 = 20; const E_Y: u16 = 22; const SPECIAL: u16 = 24; const E_DX: u16 = 26; const E_DY: u16 = 28;

impl Space {
    fn special(&mut self, i: i16) { if [11, 13].contains(&self.s.enemy(i, FACTION)) { self.s.set_enemy(i, SPECIAL, 1); } }
    pub fn create_ships(&mut self, first: i16, count: i16, mut faction: i16, kind: u8) {
        if faction == 0 {
            faction = match kind { 0 => self.random(12) + 1, 1 => self.random(3) + 11, _ => { let mut f = 12; while f == 12 { f = self.random(13) + 1; } f } };
        }
        for i in 0..count {
            let e = first.wrapping_add(i);
            let (sprite, variant, w0, w1, hull) = match kind { 0 => (170, self.random(2) + 1, 4, None, 300), 1 => (110, self.random(8) + 1, 3, Some(8), 1500), _ => (140, self.random(5) + 1, 1, Some(8), 1750) };
            self.s.set_enemy(e, SPRITE_W, sprite); self.s.set_enemy(e, VARIANT, variant); self.s.set_enemy(e, WEAPON, w0);
            if let Some(w1) = w1 { self.s.set_enemy(e, WEAPON + 2, w1); }
            self.s.set_enemy(e, FACTION, faction); self.s.set_enemy(e, HULL, hull);
            self.special(e);
        }
    }
    fn single(&mut self, first: i16, sprite: i16, variant: i16, weapons: &[i16], faction: i16, default_faction: i16, hull: i16) {
        self.s.set_enemy(first, SPRITE_W, sprite); self.s.set_enemy(first, VARIANT, variant);
        for (k, w) in weapons.iter().enumerate() { self.s.set_enemy(first, WEAPON + 2 * k as u16, *w); }
        self.s.set_enemy(first, FACTION, if faction != 0 { faction } else { default_faction });
        self.s.set_enemy(first, HULL, hull);
        self.special(first);
    }
    pub fn create_encounter_group(&mut self, first: i16, requested: i16) {
        let (kind, faction) = if requested > 0 && requested < 20 {
            (match self.s.byte(0x1BE) { 0..=3 => 0, 4 => 9, 5..=7 => 12, 8 => 10, 9 => 13, 10 | 11 => 11, 12 => 15, _ => 16 }, requested)
        } else { (self.random(19), 0) };
        match kind {
            0..=3 => { let v = self.random(20) + 1; let f = if faction != 0 { faction } else { self.random(12) + 1 }; self.single(first, 160, v, &[2, 2], f, f, 1500); }
            4 => self.create_ships(first, 3, faction, 0),
            5 | 6 => self.create_ships(first, 1, faction, 0),
            7..=9 => self.create_ships(first, 1, faction, 1),
            10 => self.create_ships(first, 2, faction, 1),
            11 => self.create_ships(first, 3, faction, 1),
            12 => self.create_ships(first, 1, faction, 2),
            13 => self.create_ships(first, 2, faction, 2),
            14 => self.single(first, 150, 0, &[3], faction, 13, 1500),
            15 | 16 => { let v = self.random(12) + 1; let f = if faction != 0 { faction } else { self.random(3) + 11 }; self.single(first, 120, v, &[3, 2, 8, 8], f, f, 2500); }
            17 | 18 => { let v = self.random(8) + 1; let f = if faction != 0 { faction } else { self.random(3) + 11 }; self.single(first, 130, v, &[2, 2, 8], f, f, 2000); }
            _ => {}
        }
    }
    pub fn initialize_system_encounters(&mut self) {
        for i in 0..20u16 { self.s.set_byte(ICON_SAVED + i, 0); }
        for i in 0..self.s.word(SHIP_COUNT).max(0) {
            self.create_encounter_group(i.wrapping_mul(4), 0);
            let n = self.s.word(COUNT);
            let world = self.random(n.wrapping_sub(1)).wrapping_add(1);
            self.s.set_enc(i, 4, 102);
            let dx = self.random(10); self.s.set_enc(i, 0, dx.wrapping_add(self.s.array(OBJECT_X, world)));
            let dy = self.random(10); self.s.set_enc(i, 2, dy.wrapping_add(self.s.array(OBJECT_Y, world)));
            let world = self.random(n.wrapping_sub(1)).wrapping_add(1);
            let radius = self.s.array(RADIUS, world) / 2;
            self.s.set_enc(i, 6, self.s.array(OBJECT_X, world).wrapping_add(radius));
            self.s.set_enc(i, 8, self.s.array(OBJECT_Y, world).wrapping_add(radius));
            self.r.fill(&self.s, 171, 0, 29, 320, 0);
            let draw = super::render::Render::page(&self.s);
            self.copy(draw, 171, 0, 29, 40, 2, 171, 0);
        }
    }
    pub fn heading_from_delta(dx: i16, dy: i16) -> i16 {
        match (dx, dy) {
            (1, 1) => 7, (-1, 1) => 5, (0, 1) => 6, (0, -1) => 2,
            _ if dy != 0 => if dx > 0 { 1 } else { 3 },
            _ => if dx > 0 { 0 } else { 4 },
        }
    }
    pub fn encounter_icons(&mut self, draw: bool) {
        let (px, py, n) = (self.s.word(X), self.s.word(Y), self.s.word(SHIP_COUNT));
        let other = (self.s.word(ACTIVE_PAGE) as usize & 1) ^ 1;
        if draw {
            for i in 0..n.max(0) {
                let (ex, ey) = (self.s.enc(i, 0), self.s.enc(i, 2));
                let row = ey.wrapping_sub(py).wrapping_mul(16).wrapping_add(100);
                let column_offset = self.s.array(RADIUS, i);
                if ex > px.wrapping_sub(7) && ex < px.wrapping_add(7) && ey.wrapping_add(column_offset) > py.wrapping_sub(5) && ey < py.wrapping_add(5) && row > 12 {
                    let at = ICON_SAVED.wrapping_add(i as u16); self.s.set_byte(at, self.s.byte(at).wrapping_add(1));
                    self.copy(other, row, ex.wrapping_sub(px).wrapping_mul(2).wrapping_add(16), 5, 1, 2, 171, i + 3);
                    let direction = self.s.enc(i, 4);
                    self.r.layout(&self.s, direction, row, ex.wrapping_sub(px).wrapping_mul(16).wrapping_add(128));
                }
            }
        } else {
            for i in (0..n.max(0)).rev() {
                let at = ICON_SAVED.wrapping_add(i as u16);
                if self.s.byte(at) != 0 {
                    let row = self.s.enc(i, 2).wrapping_sub(py).wrapping_mul(16).wrapping_add(100);
                    let col = self.s.enc(i, 0).wrapping_sub(px).wrapping_mul(2).wrapping_add(16);
                    if row > 11 && row < 165 && col > 1 && col < 30 { self.copy(2, 171, i + 3, 5, 1, other, row, col); }
                    self.s.set_byte(at, self.s.byte(at).wrapping_sub(1));
                }
            }
        }
    }
    pub fn init_encounter(&mut self) {
        for i in 0..100u16 { self.s.set_byte(GRID.wrapping_add(i), 0); }
        self.s.set_word(TACTICAL_X, 4); self.s.set_word(TACTICAL_Y, 4);
        self.s.set_byte(0x38C2, 255);
        let encounter = self.s.word(ENCOUNTER).wrapping_sub(1);
        for slot in 0..4i16 {
            for off in [0x37AE, 0x37BC, 0x37A2] { self.s.set_array(off, slot, -1); }
            let ship = encounter.wrapping_mul(4).wrapping_add(slot);
            self.s.set_enemy(ship, E_DIR, self.s.enc(encounter, 4).wrapping_sub(100));
            if self.s.enemy(ship, SPRITE_W) == 0 { continue; }
            let cell = loop {
                self.s.set_array(0x37AE, slot, 0);
                let mut ex = self.random(5); if self.s.enc(encounter, 0) > self.s.word(X) { ex = ex.wrapping_add(5); }
                self.s.set_enemy(ship, E_X, ex);
                let mut ey = self.random(5); if self.s.enc(encounter, 2) > self.s.word(Y) { ey = ey.wrapping_add(5); }
                self.s.set_enemy(ship, E_Y, ey);
                let cell = GRID.wrapping_add((ex as u16).wrapping_mul(15)).wrapping_add(ey as u16);
                if self.s.byte(cell) == 0 { break cell; }
            };
            self.s.set_byte(cell, (ship.wrapping_add(1) & 255) as u8);
            if self.s.enc(encounter, 6) == self.s.word(X) && self.s.enc(encounter, 8) == self.s.word(Y) {
                self.s.set_enemy(ship, E_DX, -1); self.s.set_enemy(ship, E_DY, -1);
            } else {
                let dx = self.random(10); self.s.set_enemy(ship, E_DX, dx);
                let dy = self.random(10); self.s.set_enemy(ship, E_DY, dy);
            }
        }
    }
    pub fn update_system_minimap(&mut self) {
        self.ev.minimap = self.ev.minimap.wrapping_add(1);
        self.event(crate::lockstep::SpaceEvent::Minimap);
        let colors: [i16; 7] = std::array::from_fn(|i| self.s.word(0x0E22 + 2 * i as u16));
        let rows: [i16; 12] = std::array::from_fn(|i| self.s.word(0x0E30 + 2 * i as u16));
        let columns: [i16; 12] = std::array::from_fn(|i| self.s.word(0x0E48 + 2 * i as u16));
        self.encounter_icons(false);
        let (px, py) = (self.s.word(X), self.s.word(Y));
        let mut quadrant = if py < 186 { 1 } else { 4 };
        if px < 186 { quadrant = if quadrant == 1 { 2 } else { 3 }; }
        if self.s.word(CURRENT) == -1 {
            for i in (quadrant - 1) * 3..quadrant * 3 { self.r.pixel(&self.s, rows[i as usize], columns[i as usize], 4); }
        }
        let (n_obj, n_enc) = (self.s.word(COUNT), self.s.word(SHIP_COUNT));
        let count = n_obj.max(n_enc);
        for i in 0..count.max(0) {
            if self.s.word(CURRENT) == -1 && i < n_obj {
                let kind = self.s.array(KIND, i);
                { let (a, b, c, d) = (self.s.array(OBJECT_X, i), self.s.array(OBJECT_Y, i), quadrant, *colors.get(kind as usize).unwrap_or(&0)); self.r.plot_minimap(&mut self.s, a, b, c, d as u8); }
            }
            let quotient = self.s.word(DRAW_COUNTER) / 30;
            if i < n_enc && quotient == 0 && self.s.word(CURRENT) == -1 && self.s.enc(i, 4) > 0 {
                { let (a, b, c, d) = (self.s.enc(i, 0), self.s.enc(i, 2), quadrant, 6); self.r.plot_minimap(&mut self.s, a, b, c, d as u8); }
            } else if i < n_enc && quotient != 0 && self.s.enc(i, 4) > -1 && self.random(10) > 6 {
                let (old_x, old_y) = (self.s.enc(i, 0), self.s.enc(i, 2));
                let mut arrived = 0;
                let mut delta = [0i16; 2];
                for (k, (coordinate, destination, player)) in [(0u16, 6u16, px), (2, 8, py)].into_iter().enumerate() {
                    let mut target = self.s.enc(i, destination);
                    let value = self.s.enc(i, coordinate);
                    if target == -1 { target = player; } else if value == target { arrived += 1; }
                    let step = if value < target { 1 } else if value > target { -1 } else { 0 };
                    self.s.set_enc(i, coordinate, value.wrapping_add(step));
                    delta[k] = step;
                }
                let (x, y) = (self.s.enc(i, 0), self.s.enc(i, 2));
                if x.wrapping_sub(2) < px && px < x.wrapping_add(2) && y.wrapping_sub(2) < py && py < y.wrapping_add(2) {
                    let chance = self.s.byte(0x1BE) as i16;
                    let trigger = self.random(chance) > 0 || self.s.enc(i, 6) == -1 || self.s.enemy(i.wrapping_mul(4), FACTION) == 11;
                    if trigger && self.s.word(CURRENT) == -1 {
                        for slot in 0..4 {
                            if self.s.enemy(i.wrapping_add(1).wrapping_mul(4).wrapping_add(slot), HULL) > 0x1F4 {
                                self.s.set_word(ENCOUNTER, i.wrapping_add(1)); self.init_encounter(); break;
                            }
                        }
                    }
                }
                if arrived == 2 {
                    let world = self.random(n_obj.wrapping_sub(1)).wrapping_add(1);
                    let radius = self.s.array(RADIUS, world) / 2;
                    self.s.set_enc(i, 6, self.s.array(OBJECT_X, world).wrapping_add(radius));
                    self.s.set_enc(i, 8, self.s.array(OBJECT_Y, world).wrapping_add(radius));
                } else {
                    self.s.set_enc(i, 4, Self::heading_from_delta(delta[0], delta[1]).wrapping_add(100));
                }
                if (x, y) != (old_x, old_y) {
                    { let (a, b, c, d) = (old_x, old_y, quadrant, 0); self.r.plot_minimap(&mut self.s, a, b, c, d as u8); }
                    { let (a, b, c, d) = (x, y, quadrant, 6); self.r.plot_minimap(&mut self.s, a, b, c, d as u8); }
                }
            }
        }
        if self.s.word(DRAW_COUNTER) / 30 != 0 { self.s.set_word(DRAW_COUNTER, 0); }
        let draw = super::render::Render::page(&self.s);
        self.copy(draw, 100, 16, 16, 2, 2, 171, 0);
        self.encounter_icons(true);
        self.s.set_word(DRAW_COUNTER, self.s.word(DRAW_COUNTER).wrapping_add(1));
        self.ev.minimap_returns = self.ev.minimap_returns.wrapping_add(1);
        self.event(crate::lockstep::SpaceEvent::MinimapReturn);
    }
}
