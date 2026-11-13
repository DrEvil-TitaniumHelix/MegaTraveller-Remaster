use super::{Space, state::*, system, tick};

pub const TARGET_SHIP: u16 = 0x37A2;
pub const TARGET_Y: u16 = 0x37AE;
pub const TARGET_X: u16 = 0x37BC;
pub const TARGET_IMAGE: u16 = 0x37CA;
pub const GUN_DIRECTION: u16 = 0x01BA;
pub const CHARGE: u16 = 0x01BF;
pub const MISSILES: u16 = 0x387E;

pub fn grid_address(row: i16, column: i16) -> u16 {
    GRID.wrapping_add((row as u16).wrapping_mul(15)).wrapping_add(column as u16)
}

impl Space {
    pub fn tactical_ship_at(&self, row: i16, column: i16) -> i16 {
        if !(0..10).contains(&row) || !(0..10).contains(&column) { return 0; }
        let value = self.s.byte(grid_address(row, column));
        if value > 100 { 0 } else { value as i16 }
    }
    pub fn combat_program_loaded(&self, program: u8) -> i16 {
        (0..20).any(|i| { let p = self.s.pbyte(0xAC + i); p & 15 == program && p & 0x20 != 0 }) as i16
    }
    pub fn combat_skill(&mut self, crew: i16, skill: i16) -> i16 {
        if skill < 0 {
            let at = (crew as u16).wrapping_mul(256).wrapping_add(skill.wrapping_neg() as u16).wrapping_add(0x17B);
            self.s.set_pbyte(at, self.s.pbyte(at).wrapping_add(1));
        }
        1
    }
    pub fn resolve_player_shot(&mut self, weapon: i16, gun: i16, row_distance: i16, column_distance: i16) -> i16 {
        let weapon = weapon.wrapping_sub(1);
        let at = CHARGE + if gun != 0 { 2 } else { 0 };
        let charge = self.s.word(at);
        let mut empty = charge < 0;
        let scale = if weapon < 4 { self.s.set_word(at, 0); charge } else { 100 };
        if weapon > 3 && weapon < 8 {
            if charge < 25 { empty = true; } else { self.s.set_word(at, charge.wrapping_sub(25)); }
        }
        let distance = row_distance.wrapping_add(column_distance) / 2;
        let crew = self.s.pbyte(0xA1u16.wrapping_add(gun as u16)) as i16 - 1;
        let mut skill = 0i16;
        for (program, factor) in [(5, 5), (6, 5), (7, 7), (8, 9)] {
            skill = skill.wrapping_add(self.combat_program_loaded(program) * factor);
        }
        for (ability, factor) in [(0x44, 3), (0x42, 2), (0x38, 2), (0x3A, 1)] {
            skill = skill.wrapping_add(self.combat_skill(crew, ability) * factor);
        }
        skill = skill.wrapping_add((self.s.pbyte((crew as u16).wrapping_mul(256).wrapping_add(0x117)) / 5) as i16);
        let component = self.s.pbyte(0xC3u16.wrapping_add(gun as u16));
        let penalty = if component < 50 { (50 - component) as i16 / 10 } else { 0 };
        let threshold = distance.wrapping_mul(5).wrapping_add(penalty).wrapping_add(40)
            .wrapping_sub(skill).wrapping_sub(if weapon == 11 { 20 } else { 0 }).max(0);
        if self.random(100) <= threshold || empty { return 0; }
        (self.s.array(0xE84, weapon) as i32 * scale as i32 / 100) as i16
    }
    pub fn resolve_enemy_shot(&mut self, weapon: i16, row_distance: i16, column_distance: i16) -> i16 {
        let distance = row_distance.wrapping_add(column_distance) / 2;
        let pilot = self.s.pbyte(0xA0);
        let skill = if pilot != 0 { self.combat_skill(pilot as i16 - 1, 0x2E) } else { 0 };
        let mut defense = skill.wrapping_add((self.s.pbyte(0xC3) / 10) as i16);
        for program in [10, 11, 12] { defense = defense.wrapping_add(self.combat_program_loaded(program) * 2); }
        if weapon < 9 { defense = defense.wrapping_sub(20); }
        if self.random(100) > 60i16.wrapping_sub(distance.wrapping_mul(5)).wrapping_sub(defense) {
            self.combat_palette(0, 4);
            self.combat_effect(2);
            self.wait_with_input(3);
            self.combat_palette(0, 0);
            let bound = self.s.array(0xEA2, weapon);
            let damage = self.random(bound) as u8;
            self.s.set_pbyte(0xC6, self.s.pbyte(0xC6).wrapping_add(damage));
            let component = self.random(4) as u16;
            let damage = self.random(bound) as u8;
            let at = 0xC2u16.wrapping_add(component);
            self.s.set_pbyte(at, self.s.pbyte(at).wrapping_add(damage));
            if self.s.pbyte(at) > 100 { self.s.set_pbyte(at, 100); }
            if component == 0 && self.s.pbyte(at) >= 30 { let n = self.rand() % 9; self.s.set_pbyte(0xAC + n as u16, 0); }
            if self.s.pbyte(0xC6) > 100 { self.ship_destroyed(1); }
        }
        0
    }
    pub fn advance_tactical_enemies(&mut self) -> i16 {
        let encounter = self.s.word(ENCOUNTER);
        if encounter < 1 { return 0; }
        let mut active = 0;
        for slot in 0..4 {
            if self.s.array(TARGET_Y, slot) == -1 { continue; }
            active = 1;
            let ship = encounter.wrapping_sub(1).wrapping_mul(4).wrapping_add(slot);
            let (old_row, old_column) = (self.s.enemy(ship, 20), self.s.enemy(ship, 22));
            let (mut dest_row, mut dest_column) = (self.s.enemy(ship, 26), self.s.enemy(ship, 28));
            let (pr, pc) = (self.s.word(TACTICAL_X), self.s.word(TACTICAL_Y));
            if dest_row == -1 || dest_column == -1 { dest_row = pr; dest_column = pc; }
            if pr.wrapping_sub(2) < old_row && pr.wrapping_add(2) > old_row
                && pc.wrapping_sub(2) < old_column && pc.wrapping_add(2) > old_column && self.s.word(tick::ACTIVE + 12) == -1 {
                for weapon in 0..4 {
                    let w = self.s.enemy(ship, 4 + weapon * 2);
                    if w > 0 { self.resolve_enemy_shot(w, 2i16.wrapping_sub(old_row.wrapping_sub(pr).wrapping_abs()), 2i16.wrapping_sub(old_column.wrapping_sub(pc).wrapping_abs())); }
                }
                self.s.set_word(tick::ACTIVE + 12, 1);
                let now = self.ticks(); self.s.set_dword(tick::DEADLINES + 24, now.wrapping_add(5));
            }
            for (off, dest) in [(20, dest_row), (22, dest_column)] {
                let value = self.s.enemy(ship, off);
                self.s.set_enemy(ship, off, value.wrapping_add(if value < dest { 1 } else if value > dest { -1 } else { 0 }));
            }
            let (row, col) = (self.s.enemy(ship, 20), self.s.enemy(ship, 22));
            if col.wrapping_sub(1) <= dest_column && col.wrapping_add(1) >= dest_column
                && row.wrapping_sub(1) <= dest_row && row.wrapping_add(1) >= dest_row {
                if (dest_row == 0 || dest_column == 0) && self.s.enemy(ship, 16) < 500 && self.random(50) == 0 {
                    self.s.set_word(PREVIOUS_STATION, self.s.word(STATION)); self.s.set_word(STATION, -1);
                    if self.s.enc(encounter - 1, 6) == self.s.word(X) && self.s.enc(encounter - 1, 8) == self.s.word(Y) {
                        let world = self.random(self.s.word(system::COUNT));
                        self.s.set_enc(encounter - 1, 6, self.s.array(system::OBJECT_X, world));
                        self.s.set_enc(encounter - 1, 8, self.s.array(system::OBJECT_Y, world));
                    }
                    return 1;
                } else if self.s.enemy(ship, 16) > 500 {
                    self.s.set_enemy(ship, 26, -1); self.s.set_enemy(ship, 28, -1);
                } else if self.random(2) != 0 {
                    if self.random(2) != 0 { let d = self.random(10); self.s.set_enemy(ship, 26, d); self.s.set_enemy(ship, 28, 0); }
                    else { self.s.set_enemy(ship, 26, 0); let d = self.random(10); self.s.set_enemy(ship, 28, d); }
                }
            }
            let dest = grid_address(row.wrapping_add(pr), col.wrapping_add(pc));
            match self.s.byte(dest) {
                marker @ (100 | 101) => { let cap = if marker == 100 { 1000 } else { 750 }; if self.s.enemy(ship, 16) > cap { self.s.set_enemy(ship, 16, cap); } self.s.set_byte(dest, 0); }
                _ => {}
            }
            if self.s.byte(dest) != 0 { self.s.set_enemy(ship, 20, old_row); self.s.set_enemy(ship, 22, old_column); }
            else { let src = grid_address(pr.wrapping_add(old_row), pc.wrapping_add(old_column)); self.s.set_byte(dest, self.s.byte(src)); self.s.set_byte(src, 0); }
            let direction = Self::heading_from_delta(dest_row.wrapping_sub(self.s.enemy(ship, 20)), dest_column.wrapping_sub(self.s.enemy(ship, 22)));
            self.s.set_enemy(ship, 18, direction);
        }
        active
    }
}
