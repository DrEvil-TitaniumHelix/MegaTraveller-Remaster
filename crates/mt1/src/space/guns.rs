use super::{Space, state::*, combat::*, render::Render, system};
use crate::lockstep::SpaceEvent;
use mt_engine::present::{ly, pic, prim::Raster};

pub type Result<T> = std::result::Result<T, String>;
pub type Rect = [i16; 4];

pub trait StationUi {
    fn call(&mut self, sp: &mut Space, name: &str, args: &[i16]) -> Result<i16>;
    fn select(&mut self, sp: &mut Space, boxes: &[Rect], count: i16, selected: i16,
        hotkeys: Option<&[u8]>, boarding: bool) -> Result<i16>;
}

impl Space {
    pub fn return_missiles_to_supplies(&mut self) {
        for tube in 0..4 {
            let value = self.s.byte(MISSILES + tube);
            if value != 0 {
                for slot in 0..20u16 {
                    if self.s.pbyte(0xDE + slot) == 0 { self.s.set_pbyte(0xDE + slot, value); break; }
                }
            }
        }
    }
    pub fn encounter_layout(&mut self, name: &str, image: i16, y: i16, x: i16) {
        if image >= 0 { ly::layout(&mut self.r.fb, Raster::Wrap, Render::page(&self.s), self.r.assets.layout(name), image as usize, y as i32, x as i32, &mut self.r.pen); }
    }
    pub fn encounter_icon(&mut self, icon: i16, y: i16, x: i16) {
        let (sy, sx) = ((icon / 20).wrapping_mul(16), (icon % 20).wrapping_mul(2));
        for (at, value) in [(0x49F2, sy), (0x49F4, 16), (0x49EC, sx), (0x49F6, 2)] { self.s.set_word(at, value); }
        pic::draw_rect(&mut self.r.fb, Render::page(&self.s), &self.r.assets.space,
            y as i32, x as i32 * 8, sy as i32, sx as i32 * 8, 16, 16, None);
    }
    pub fn encounter_labels(&mut self, at: u16, count: u16, y: i16, x: i16) {
        for i in 0..count {
            let p = at.wrapping_add(i * 9);
            if self.s.byte(p + 8) == 255 { break; }
            if let Some(text) = self.s.pointer_string(p + 4) {
                self.r.text(&self.s, &text, self.s.word(p).wrapping_add(y), self.s.word(p + 2).wrapping_add(x), self.s.byte(p + 8));
            }
        }
    }
    pub fn encounter_rects(&self, at: u16, count: usize) -> Vec<Rect> {
        (0..count).map(|i| std::array::from_fn(|j| self.s.word(at.wrapping_add(i as u16 * 8 + j as u16 * 2)))).collect()
    }
    pub fn draw_gun_view_targets(&mut self) {
        for i in (0..4).rev() {
            if self.s.array(TARGET_Y, i) > -1 {
                self.encounter_layout("GUNS.LY", self.s.array(TARGET_IMAGE, i), self.s.array(TARGET_X, i), self.s.array(TARGET_Y, i));
            }
        }
    }
    pub fn update_gun_view_targets(&mut self, direction: i16) {
        for i in (0..4).rev() {
            let index = direction.wrapping_mul(8).wrapping_add(i * 2);
            let ship = self.tactical_ship_at(self.s.array(0x29B, index).wrapping_add(self.s.word(TACTICAL_X)),
                self.s.array(0x29B, index + 1).wrapping_add(self.s.word(TACTICAL_Y)));
            if ship != 0 {
                let y = self.random(self.s.array(0x33B, i)).wrapping_add(self.s.array(0x333, i)); self.s.set_array(TARGET_Y, i, y);
                let x = self.random(20).wrapping_add(105); self.s.set_array(TARGET_X, i, x);
                let heading = self.s.enemy(ship - 1, 18).wrapping_add(8i16.wrapping_sub(direction)) % 8;
                let image = self.s.array(0x323, heading).wrapping_add((self.s.enemy(ship - 1, 0).wrapping_sub(110) / 10).wrapping_mul(8));
                self.s.set_array(TARGET_IMAGE, i, image); self.s.set_array(TARGET_SHIP, i, ship - 1);
                if i == 0 { self.s.set_array(TARGET_IMAGE, i, image.wrapping_add(1)); }
            } else { for at in [TARGET_SHIP, TARGET_Y, TARGET_X, TARGET_IMAGE] { self.s.set_array(at, i, -1); } }
        }
        self.draw_gun_view_targets();
    }
    pub fn redraw_gun_station(&mut self, gun: i16, ui: &mut dyn StationUi) -> Result<()> {
        let g = gun as u16;
        let direction = self.s.word(GUN_DIRECTION + if gun != 0 { 2 } else { 0 });
        self.r.back(&mut self.s); ui.call(self, "redraw_active_station", &[])?;
        self.r.layout(&self.s, 11, 83, 0);
        if self.s.pbyte(0xC8 + g) != 0 { self.redraw_tactical_radar(gun + 99); }
        self.update_gun_view_targets(direction); self.r.layout(&self.s, 3, 100, 123);
        let crew = self.s.pbyte(0xA1 + g);
        self.combat_palette(10, self.s.byte(if crew != 0 { 0x168 + crew as u16 } else { 0x16F }));
        if crew != 0 {
            let mut title = format!("GUN{}: ", gun + 1).into_bytes();
            let at = ((crew as u16 - 1) * 256).wrapping_add(0x102);
            title.extend((0..65536).map(|i| self.s.pbyte(at.wrapping_add(i as u16))).take_while(|&b| b != 0));
            self.r.text(&self.s, &title, 91, 132, 0);
        }
        let weapon = self.s.pbyte(0xC8 + g);
        if weapon != 0 {
            let kind = (weapon as i16 - 1) / 4; self.r.layout(&self.s, kind + 33, 83, 0);
            if kind < 2 { self.r.fill(&self.s, 176, 127, 5, self.s.word(CHARGE + if gun != 0 { 2 } else { 0 }), 9); }
            if kind == 0 || kind == 1 {
                let value = self.s.string(if kind == 0 { 0x183C } else { 0x1847 });
                self.r.text(&self.s, &value, 167, 128, 0); self.encounter_labels(0xE69, 2, 0, 0);
            } else if kind == 2 { self.draw_missile_inventory(); }
        }
        self.r.flip(&mut self.s); self.gun_say(gun); Ok(())
    }
    fn tar(&self, gun: i16) -> &'static str { if self.s.pbyte(0xC8 + gun as u16) == 9 { "Tar" } else { "Target" } }
    pub fn gun_line(&self, gun: i16) -> Option<Vec<u8>> {
        if !self.gun_note.is_empty() { return Some(self.gun_note.clone().into_bytes()); }
        let weapon = self.s.pbyte(0xC8 + gun as u16);
        Some((if weapon == 0 { return Some(format!("GUN{}: no weapon fitted", gun + 1).into_bytes()) }
            else if weapon == 9 && self.combat_program_loaded(2) == 0 { "COMP: run Launch first" }
            else if weapon == 9 && (0..4).all(|i| self.s.byte(MISSILES + i) == 0) { "Click Rel: load a tube" }
            else if self.s.word(CHARGE + if gun != 0 { 2 } else { 0 }) < 1 { "Gun charging: wait" }
            else if self.s.word(ENCOUNTER) < 1 { return None }
            else if (0..4).all(|i| self.s.array(TARGET_Y, i) == -1) { "Purple arrows: find hostile" }
            else if weapon == 9 { "Click Tar (red cross-hair)" }
            else { "Click Target" }).as_bytes().to_vec())
    }
    pub fn tube_line(&self) -> String {
        let loaded: Vec<String> = (0..4u16).filter(|&i| self.s.byte(MISSILES + i) != 0).map(|i| (i + 1).to_string()).collect();
        format!("Click tube {}", loaded.join(" or "))
    }
    pub fn gun_say(&mut self, gun: i16) {
        if !self.product || self.observer.is_some() || self.input.data_mode() { return }
        let Some(text) = self.gun_line(gun) else { return };
        let keep = ([0x49EE, 0x2A15, 0x2792, 0x2794].map(|at| (at, self.s.word(at))), self.r.pen);
        for front in [true, false] {
            if front { self.r.front(&mut self.s) } else { self.r.back(&mut self.s) }
            self.r.pen.op = 0; self.r.fill(&self.s, 89, 124, 11, 189, 10); self.r.text(&self.s, &text, 91, 132, 0);
            if self.s.pbyte(0xC8 + gun as u16) == 0 { self.r.text(&self.s, b"A shipyard", 116, 166, 7); self.r.text(&self.s, b"can fit one.", 126, 166, 7); }
        }
        for (at, v) in keep.0 { self.s.set_word(at, v); }
        self.r.pen = keep.1;
    }
    pub fn draw_missile_inventory(&mut self) {
        for i in 0..4u16 {
            let (y, x) = (self.s.word(0xF0E + i * 4), self.s.word(0xF10 + i * 4));
            let value = self.s.byte(MISSILES + i);
            if value != 0 { self.encounter_icon(value as i16 + 48, y, x / 8); }
            else { self.r.fill(&self.s, y, x + 1, 16, 15, 0); }
        }
        self.encounter_labels(0xF1E, 8, 83, 0);
    }
    pub fn reload_missile_tube(&mut self, ui: &mut dyn StationUi) -> Result<()> {
        self.copy_display_rectangle(0, 0, 200, 40); self.r.back(&mut self.s); self.r.layout(&self.s, 18, 88, 5);
        let cargo = self.encounter_rects(0x94, 20); let tubes = self.encounter_rects(0x194, 4);
        for (i, rect) in cargo.iter().enumerate() { let value = self.s.pbyte(0xDE + i as u16); if value != 0 { self.encounter_icon(value as i16 + 48, rect[0] + 1, rect[1] / 8); } }
        self.encounter_labels(0xEC6, 8, 83, 0); self.r.flip(&mut self.s); self.r.front(&mut self.s);
        let mut selected = 0;
        loop {
            selected = ui.select(self, &cargo, 20, selected, None, false)?;
            if selected == -1 { break; }
            let tube = ui.select(self, &tubes, 4, 0, Some(b"1234"), false)?;
            if tube == -1 { continue; }
            let at = 0xDEu16.wrapping_add(selected as u16); let mt = MISSILES.wrapping_add(tube as u16);
            let previous = self.s.pbyte(at); self.s.set_pbyte(at, self.s.byte(mt)); self.s.set_byte(mt, previous); break;
        }
        self.draw_missile_inventory(); self.r.flip(&mut self.s); Ok(())
    }
    fn target_bounds(&self, i: i16) -> Rect {
        let image = self.s.array(TARGET_IMAGE, i);
        let at = self.s.array(0x28D, image / 8).wrapping_mul(32).wrapping_add((image % 8) * 4);
        std::array::from_fn(|j| self.s.array(0x1CD, at.wrapping_add(j as i16)))
    }
    fn gun_reticle(&mut self, y: i16, x: i16, restore: bool) {
        let front = self.s.word(ACTIVE_PAGE) as usize & 1;
        if restore { self.copy(2, 187, 8, 13, 4, front, x, y / 8); }
        else { self.copy(front, x, y / 8, 13, 4, 2, 187, 8); self.r.layout(&self.s, 44, x, y); }
    }
    fn fire_lines(&mut self, weapon: i16, y: i16, x: i16, damage: i16, thick: bool) {
        let color = if damage != 0 { 10 } else if weapon > 9 { 7 } else { 6 };
        self.combat_effect(if weapon > 9 { 0 } else { 1 });
        self.combat_line(155, 157, x + 6, y + 7, color); self.combat_line(155, 277, x + 6, y + 7, color);
        if thick { self.combat_line(155, 158, x + 6, y + 7, color); self.combat_line(155, 276, x + 6, y + 7, color); }
    }
    fn target_damage(&mut self, weapon: i16, gun: i16, target: i16) -> i16 {
        let ship = self.s.array(TARGET_SHIP, target);
        self.resolve_player_shot(weapon, gun, 2i16.wrapping_sub(self.s.enemy(ship, 20).wrapping_sub(self.s.word(TACTICAL_X)).wrapping_abs()),
            2i16.wrapping_sub(self.s.enemy(ship, 22).wrapping_sub(self.s.word(TACTICAL_Y)).wrapping_abs()))
    }
    fn replenish_encounter(&mut self, gun: i16) {
        for _ in 0..2 {
            for skill in [-0x44, -0x42, -0x38, -0x3A] { self.combat_skill(self.s.pbyte(0xA1 + gun as u16) as i16 - 1, skill); }
            self.combat_skill(self.s.pbyte(0xA0) as i16 - 1, 0x2E);
        }
        let encounter = self.s.word(ENCOUNTER) - 1;
        let faction = self.s.enemy(encounter * 4, 14);
        let mut chance = self.s.byte(0x1BE);
        let kind = match faction {
            11 | 12 => { if faction == 12 { chance = chance.wrapping_add(2); } if chance != 0 { chance -= 1; } 0 }
            _ => { if faction == 13 { chance = chance.wrapping_add(2); } chance = chance.wrapping_add(2); faction }
        };
        self.s.set_byte(0x1BE, chance); self.create_encounter_group(encounter * 4, kind);
        let world = self.random(self.s.word(system::COUNT) - 1) + 1; self.s.set_enc(encounter, 4, 102);
        let dx = self.random(10); self.s.set_enc(encounter, 0, dx.wrapping_add(self.s.array(system::OBJECT_X, world)));
        let dy = self.random(10); self.s.set_enc(encounter, 2, dy.wrapping_add(self.s.array(system::OBJECT_Y, world)));
        if self.random(10) > 3 { self.s.set_enc(encounter, 6, -1); self.s.set_enc(encounter, 8, -1); }
        else {
            let world = self.random(self.s.word(system::COUNT) - 1) + 1;
            let dx = self.random(10); self.s.set_enc(encounter, 6, dx.wrapping_add(self.s.array(system::OBJECT_X, world)));
            let dy = self.random(10); self.s.set_enc(encounter, 8, dy.wrapping_add(self.s.array(system::OBJECT_Y, world)));
        }
    }
    pub fn aim_and_fire(&mut self, gun: i16, aim_y: &mut i16, aim_x: &mut i16, weapon: i16, ui: &mut dyn StationUi) -> Result<i16> {
        let (mut y, mut x) = (*aim_y, *aim_x);
        self.gun_note = "Sight on ship? Click Fire".into(); self.gun_say(gun); self.gun_note.clear();
        self.r.front(&mut self.s); self.gun_reticle(y, x, false);
        let result = 'aim: loop {
            super::play::station_state(self,"955C",&[y.to_le_bytes(),x.to_le_bytes(),gun.to_le_bytes(),weapon.to_le_bytes()].concat());
            if self.s.word(STATION) == -1 { break 1; }
            loop {
                let have_key = self.s.byte(KEY) != 0;
                self.event(SpaceEvent::AimKeyTest);
                if have_key || self.s.word(STATION) == -1 { break }
                if self.run_tasks(0) != 0 { self.redraw_gun_station(gun, ui)?; self.r.front(&mut self.s); self.gun_reticle(y, x, false); }
            }
            let command = self.translate_station_key();
            match command {
                1..=4 => {
                    self.gun_reticle(y, x, true);
                    match command { 1 => x = if x > 107 { x - 4 } else { 99 }, 2 => x = if x < 135 { x + 4 } else { 143 },
                        3 => y = if y > 130 { y - 4 } else { 122 }, _ => y = if y < 293 { y + 4 } else { 301 } }
                    self.gun_reticle(y, x, false);
                }
                13 | 70 | 84 => {
                    for i in 0..4 {
                        if self.s.array(TARGET_Y, i) == -1 { continue; }
                        let b = self.target_bounds(i); let (ty, tx) = (self.s.array(TARGET_Y, i), self.s.array(TARGET_X, i));
                        if ty.wrapping_add(b[1]) > y + 7 || ty.wrapping_add(b[3]) < y + 7 || tx.wrapping_add(b[0]) > x + 6 || tx.wrapping_add(b[2]) < x + 6 { continue; }
                        let damage = self.target_damage(weapon, gun, i); self.fire_lines(weapon, y, x, damage, self.s.pbyte(0xC8 + gun as u16) > 3);
                        let ship = self.s.array(TARGET_SHIP, i);
                        if self.s.enemy(ship, 16) > damage { self.s.set_enemy(ship, 16, self.s.enemy(ship, 16).wrapping_sub(damage)); self.gun_note = format!("{} Click {} again", if damage > 0 { "Hit." } else { "No damage." }, self.tar(gun)); break 'aim -1; }
                        if self.s.enemy(ship, 2) != 0 { self.board_enemy_cargo(ship, ui)?; }
                        self.s.set_array(TARGET_Y, i, -1); let mut remaining = false;
                        for row in 0..10 { for column in 0..10 {
                            let at = grid_address(row, column); let v = self.s.byte(at);
                            if v > 0 && v < 100 { if ship == v as i16 - 1 { self.s.set_byte(at, 0); } else { remaining = true; } }
                        } }
                        self.s.set_array(TARGET_SHIP, i, -1); self.s.set_array(TARGET_X, i, -1);
                        if remaining { self.gun_note = "Destroyed. Next hostile".into(); break 'aim -1; }
                        self.replenish_encounter(gun); break 'aim 1;
                    }
                    self.target_damage(weapon, gun, 4); self.fire_lines(weapon, y, x, 0, false); self.gun_note = format!("Missed. Click {} again", self.tar(gun)); break -1;
                }
                27 => { self.gun_reticle(y, x, true); break 0; }
                _ => {}
            }
        };
        *aim_y = y; *aim_x = x; Ok(result)
    }
    pub fn operate_gun_station(&mut self, gun: i16, ui: &mut dyn StationUi) -> Result<()> {
        self.redraw_gun_station(gun, ui)?;
        let original = self.s.word(STATION) - 10; let mut current = original;
        let (mut aim_y, mut aim_x) = (218i16, 115i16);
        while current == original {
            self.gun_say(gun);
            super::play::station_state(self,"9FDB",&[aim_y.to_le_bytes(),aim_x.to_le_bytes(),gun.to_le_bytes()].concat());
            self.show_cursor();
            loop {
                let have_key = self.s.byte(KEY) != 0;
                self.event(SpaceEvent::GunKeyTest);
                if have_key { break }
                if self.run_tasks(0) != 0 { self.redraw_gun_station(gun, ui)?; }
            }
            self.hide_cursor();
            if ui.call(self, "handle_station_key", &[])? != 0 { self.redraw_gun_station(gun, ui)?; continue; }
            self.gun_note.clear();
            match self.translate_station_key() {
                27 => { self.s.set_word(PREVIOUS_STATION, self.s.word(STATION)); self.s.set_word(STATION, -1); current = -1; }
                65 => { ui.call(self, "assign_station", &[])?; if self.s.word(STATION) - 10 == current { self.redraw_gun_station(gun, ui)?; } current = self.s.word(STATION) - 10; }
                88 => { self.show_tactical_map()?; }
                86 => { let v = ui.call(self, "choose_station", &[])?; if v != -1 { current = v; self.s.set_word(STATION, v + 10); } }
                82 => { if (self.s.pbyte(0xC8 + gun as u16) as i16 - 1) / 4 == 2 { self.reload_missile_tube(ui)?; self.redraw_gun_station(gun, ui)?; } }
                84 => {
                    let mut weapon = self.s.pbyte(0xC8 + gun as u16) as i16;
                    if weapon == 0 || (weapon == 9 && self.combat_program_loaded(2) == 0) { ui.call(self, "beep", &[])?; continue; }
                    if self.combat_program_loaded(1) != 0 {
                        for i in 0..4 { if self.s.array(TARGET_Y, i) != -1 {
                            let b = self.target_bounds(i); aim_y = b[1].wrapping_add(b[3]) / 2 + self.s.array(TARGET_Y, i) - 7;
                            aim_x = b[0].wrapping_add(b[2]) / 2 + self.s.array(TARGET_X, i) - 6; break;
                        } }
                    }
                    let (mut tube, mut missile) = (0, 0);
                    if weapon == 9 {
                        let boxes = self.encounter_rects(0x194, 4);
                        if self.product {
                            self.gun_note = self.tube_line(); self.gun_say(gun); self.gun_note.clear();
                            self.play_labels = (0..4u16).map(|i| format!("Tube {}: {}", i + 1, if self.s.byte(MISSILES + i) != 0 { "missile" } else { "empty" })).collect();
                        }
                        let choice = ui.select(self, &boxes, 4, 0, Some(b"1234"), false); self.play_labels.clear(); let choice = choice?;
                        if choice == -1 { continue; }
                        let at = MISSILES.wrapping_add(choice as u16); missile = self.s.byte(at);
                        if missile > 0 && missile < 3 {
                            let d = self.s.word(GUN_DIRECTION + if gun != 0 { 2 } else { 0 });
                            let row = self.s.array(0xF66, d * 2).wrapping_add(self.s.word(TACTICAL_X));
                            let col = self.s.array(0xF66, d * 2 + 1).wrapping_add(self.s.word(TACTICAL_Y));
                            if self.tactical_ship_at(row, col) == 0 { self.s.set_byte(grid_address(row, col), missile + 99); self.s.set_byte(at, 0); self.gun_note = "Missile away".into(); self.redraw_gun_station(gun, ui)?; }
                            else { self.gun_note = "Hostile too close".into(); }
                            continue;
                        } else if missile != 0 { tube = choice + 1; self.s.set_byte(at, 0); weapon = missile as i16 + 7; } else { self.gun_note = "That tube is empty".into(); continue; }
                    }
                    self.s.set_byte(KEY, 0);
                    if self.s.word(CHARGE + if gun != 0 { 2 } else { 0 }) < 1 { continue; }
                    let result = self.aim_and_fire(gun, &mut aim_y, &mut aim_x, weapon, ui)?;
                    if result == 1 { self.s.set_word(PREVIOUS_STATION, self.s.word(STATION)); self.s.set_word(STATION, -1); current = -1; }
                    else if result != 0 { self.redraw_gun_station(gun, ui)?; }
                    else if tube != 0 { self.s.set_byte(MISSILES + tube as u16 - 1, missile); }
                }
                command @ (3 | 4) => { if self.s.pbyte(0xC8 + gun as u16) != 0 {
                    let at = GUN_DIRECTION + if gun != 0 { 2 } else { 0 };
                    let d = self.s.word(at).wrapping_add(if command == 3 { 1 } else { -1 });
                    self.s.set_word(at, if d < 0 { 7 } else if d > 7 { 0 } else { d }); self.redraw_gun_station(gun, ui)?;
                } }
                70 | 49..=52 if self.s.pbyte(0xC8 + gun as u16) != 0 => self.gun_note = format!("Click {} first", self.tar(gun)),
                _ => {}
            }
        }
        self.gun_note.clear();
        Ok(())
    }
}
