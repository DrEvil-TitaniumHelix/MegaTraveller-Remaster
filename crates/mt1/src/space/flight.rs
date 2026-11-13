use super::{Space, render::Render, state::*, system::*};

#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Flight {pub scroll:[i16;10],pub old_x:i16,pub old_y:i16,pub command:i16,pub stop:bool,pub old_direction:i16}

pub const NO_ONE_AT: [&str; 8] = ["PILOT", "GUNNER 1", "GUNNER 2", "COMPUTER", "ENGINEERING", "COMMS", "MEDICAL", "SICK BAY"];
impl Space {
    fn draw_page(&self) -> usize { Render::page(&self.s) }
    fn marker(&mut self) { let d = self.draw_page(); self.copy(2, 171, 0, 16, 2, d, 100, 16); }
    fn heading_image(&mut self) {
        let h = self.s.word(HEADING); self.r.layout(&self.s, h, 100, 128);
        if self.s.word(MOVING) != 0 { self.r.layout(&self.s, h.wrapping_add(120), 100, 128); }
    }
    fn docked(&self) -> bool { let c = self.s.word(CURRENT); c >= 0 && (self.s.array(WORLD, c) != 0 || self.s.array(KIND, c) == 4) }
    fn fast(&self) -> bool { self.s.word(VX).abs() >= 3 || self.s.word(VY).abs() >= 3 }
    fn draw_thrust(&mut self, color: u8) { let t = self.s.word(THRUST); self.r.thrust(&mut self.s, t, color); if self.s.word(STATION_ACTIVE) == 0 { self.readout(true) } }

    pub fn draw_system_view(&mut self, redraw_only: bool) -> i16 {
        self.r.back(&mut self.s);
        let (draw, front) = (self.draw_page(), (self.s.word(ACTIVE_PAGE) & 1) as usize);
        let scroll = self.s.word(VIEW);
        let (mut x0, mut x1, mut y0, mut y1) = (0i32, 15i32, 0i32, 10i32);
        let case: Option<([i16; 8], fn(&mut (i32, i32, i32, i32)))> = match scroll {
            1 => Some(([12, 3, 144, 28, 28, 1, 116, 14], |b| { b.2 = 9; b.1 = 1; })),
            3 => Some(([12, 1, 144, 28, 28, 3, 116, 18], |b| { b.2 = 9; b.0 = 14; })),
            7 => Some(([28, 3, 144, 28, 12, 1, 84, 14], |b| { b.3 = 1; b.1 = 1; })),
            5 => Some(([28, 1, 144, 28, 12, 3, 84, 18], |b| { b.3 = 1; b.0 = 14; })),
            2 => Some(([12, 1, 144, 30, 28, 1, 116, 16], |b| { b.2 = 9; })),
            6 => Some(([28, 1, 144, 30, 12, 1, 84, 16], |b| { b.3 = 1; })),
            4 => Some(([12, 1, 160, 28, 12, 3, 100, 18], |b| { b.0 = 14; })),
            0 => Some(([12, 3, 160, 28, 12, 1, 100, 14], |b| { b.1 = 1; })),
            _ => None,
        };
        if let Some((a, bounds)) = case {
            let mut b = (x0, x1, y0, y1); bounds(&mut b); (x0, x1, y0, y1) = b;
            self.copy(front, a[0], a[1], a[2], a[3], draw, a[4], a[5]);
            self.copy(2, 171, 0, 16, 2, draw, a[6], a[7]);
        }
        let mut touching = false;
        if !redraw_only { self.s.set_word(CURRENT, -1); }
        if y0 > 0 || y1 != 10 {
            for x in 0..15 { for y in y0..y1 { let t = self.random(20); self.copy(2, 96, (t * 2) as i16, 16, 2, draw, ((9 - y) * 16 + 12) as i16, ((14 - x) * 2 + 1) as i16); } }
        }
        if x0 > 0 || x1 != 15 || (x0, y0, x1, y1) == (0, 0, 15, 10) {
            for x in x0..x1 { for y in 0..10 { let t = self.random(20); self.copy(2, 96, (t * 2) as i16, 16, 2, draw, ((9 - y) * 16 + 12) as i16, ((14 - x) * 2 + 1) as i16); } }
        }
        let (px, py) = (self.s.word(X), self.s.word(Y));
        for i in 0..self.s.word(COUNT) {
            let (ox, oy, size) = (self.s.array(OBJECT_X, i), self.s.array(OBJECT_Y, i), self.s.array(RADIUS, i));
            if !redraw_only {
                if self.s.array(KIND, i) != 3 && ox.wrapping_sub(5) <= px && ox.wrapping_add(size).wrapping_add(5) >= px && oy.wrapping_sub(5) <= py && oy.wrapping_add(size).wrapping_add(5) >= py {
                    let half = size / 2;
                    for (off, center, player) in [(VX, ox.wrapping_add(half), px), (VY, oy.wrapping_add(half), py)] {
                        let delta = if center > player { half.wrapping_add(1) } else if center < player { half.wrapping_add(1).wrapping_neg() } else { 0 };
                        self.s.set_word(off, self.s.word(off).wrapping_add(delta));
                    }
                    for off in [VX, VY] { let v = self.s.word(off).clamp(-16, 16); self.s.set_word(off, v); }
                }
                if ox.wrapping_sub(1) <= px && ox.wrapping_add(size) >= px && oy.wrapping_sub(1) <= py && oy.wrapping_add(size) >= py {
                    touching = true;
                    if i.wrapping_neg() != self.s.word(DEPARTED) { self.s.set_word(CURRENT, i); self.s.set_word(DEPARTED, -55); }
                }
            }
            if ox.wrapping_add(size) > px.wrapping_sub(8) && ox < px.wrapping_add(8) && oy.wrapping_add(size) > py.wrapping_sub(6) && oy < py.wrapping_add(5) {
                let sprite = self.s.array(SPRITE, i);
                self.object(sprite, ox.wrapping_sub(px).wrapping_add(8), oy.wrapping_sub(py).wrapping_add(6), size);
            }
        }
        if !touching { self.s.set_word(DEPARTED, -55); }
        self.copy(draw, 100, 16, 16, 2, 2, 171, 0);
        if self.s.word(HEADING) == 0 { self.s.set_word(HEADING, 92); }
        let h = self.s.word(HEADING); self.r.layout(&self.s, h, 100, 128);
        let current = self.s.word(CURRENT);
        let mode = if current > -1 && self.s.array(WORLD, current) != 0 && !redraw_only { Some(1) } else if current > -1 && self.s.array(KIND, current) == 4 { Some(55) } else { None };
        let result = if let Some(mode) = mode {
            self.r.front(&mut self.s); self.hud(mode);
            self.r.back(&mut self.s); self.hud(mode);
            self.s.set_word(MOVING, 0);
            1
        } else {
            if self.s.word(MOVING) != 0 { self.r.layout(&self.s, h.wrapping_add(120), 100, 128); }
            0
        };
        self.ev.view = self.ev.view.wrapping_add(1);
        self.event(crate::lockstep::SpaceEvent::View);
        result
    }

    fn restore_flight(&mut self) {
        self.drop_stale_clicks();
        self.s.set_word(DIRECTION, self.s.word(HEADING).wrapping_sub(89));
        self.r.back(&mut self.s); self.r.layout(&self.s, 21, 0, 0);
        let c = self.s.word(CURRENT);
        let mode = if self.s.array(KIND, c) == 4 { 55 } else { c };
        self.hud(mode);
        self.hide_cursor();
        self.s.set_word(VIEW, -1); self.s.set_word(QUADRANT, -1); self.s.set_byte(MOUSE_BUTTONS, 0);
        if self.draw_system_view(true) != 0 { self.draw_thrust(7); }
        self.heading_image();
        let c = self.s.word(CURRENT);
        if c >= 0 && self.s.array(KIND, c) == 4 { let d = self.draw_page(); self.copy(2, 112, 0, 59, 8, d, 2, 31); }
        else if c >= 0 && self.s.array(WORLD, c) != 0 { let d = self.draw_page(); self.copy(2, 112, 8, 59, 8, d, 2, 31); }
        else { self.r.update_minimap_quadrant(&mut self.s); }
        self.flip_with_cursor();
        self.s.set_word(STATION_ACTIVE, 0); self.s.set_word(VIEW, 0);
        if self.s.word(CURRENT) == -1 { self.draw_thrust(9); }
        self.copy_display_rectangle(0, 0, 200, 40);
        self.mouse_bounds(false); self.show_cursor();
    }

    pub fn in_system_flight(&mut self) -> i16 {self.flight_inner(None)}
    pub fn resume_flight(&mut self,flight:Flight)->i16 {self.flight_inner(Some(flight))}
    pub(super) fn pause(&mut self, text: &[u8]) {
        if self.input.data_mode() { return }
        self.paused_badge(text); self.hold();
    }
    fn hold(&mut self) {
        let start = self.ticks();
        loop {
            self.poll_input();
            if self.s.byte(KEY) != 0 || self.s.byte(PENDING_KEY) != 0 { break }
        }
        let k = if self.s.byte(KEY) != 0 { self.s.byte(KEY) } else { self.s.byte(PENDING_KEY) };
        if matches!(k, 0x19 | 0x39 | 110) { self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0); }
        let gap = self.ticks().wrapping_sub(start);
        for i in 0..10u16 { let at = super::tick::DEADLINES + 4 * i; let v = self.s.dword(at); if v != 0 { self.s.set_dword(at, v.wrapping_add(gap)); } }
        if self.s.dword(super::tick::SNAPSHOT) != 0xFFFF { self.snapshot_clock(); }
        self.s.set_word(VIEW, -1);
    }
    fn no_comms(&mut self) { self.say(b"NO ONE AT THE COMMS STATION", b"SEAT ONE: STATIONS, ASSIGN") }
    fn say(&mut self, top: &[u8], hint: &[u8]) {
        if !self.product || self.observer.is_some() || self.input.data_mode() { return }
        let w = top.len() as i16 * 8 + 16; let x = 8 + (240 - w) / 2;
        if !hint.is_empty() { for front in [true, false] {
            if front { self.r.front(&mut self.s) } else { self.r.back(&mut self.s) }
            self.r.fill(&self.s, 100, x, 14, w, 0); self.r.text(&self.s, hint, 103, x + (w - hint.len() as i16 * 8) / 2, 15);
        } }
        self.pause(top);
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
    }
    fn arrived(&mut self) {
        let Some(asked) = self.jump_asked.take() else { return };
        let name = |s: u8| super::jump_host::proper(&self.s.pointer_string(SYSTEM_NAMES + s as u16 * 4).unwrap_or_default());
        let (here, fuel) = (self.s.pbyte(6), self.s.pbyte(0xFC));
        let first = if here == asked { format!("You have arrived in the {} system.", name(here)) } else { format!("MISJUMP: the ship came out in the {} system, not {}.", name(here), name(asked)) };
        let mut lines = self.wrap(&format!("{first} Seven days have passed. Fuel: {fuel} tons."), 216);
        lines.push(b"Press a key or click to go on.".to_vec());
        let (h, keep) = (lines.len() as i32 * 10 + 7, ([VIEW, 0x49EE, 0x2A15, 0x2792, 0x2794].map(|at| (at, self.s.word(at))), self.r.pen));
        for p in 0..2 { self.r.fb.copy(p, 60, 0, h, 40, 7, h * p as i32, 0); }
        for front in [true, false] {
            if front { self.r.front(&mut self.s) } else { self.r.back(&mut self.s) }
            self.panel(&lines, &[], 0, 60);
        }
        self.r.front(&mut self.s);
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
        self.drop_stale_clicks(); super::play::notice(self);
        self.hold();
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
        for p in 0..2 { self.r.fb.copy(7, h * p as i32, 0, h, 40, p, 60, 0); }
        for (at, v) in keep.0 { self.s.set_word(at, v); }
        self.r.pen = keep.1;
    }
    pub(super) fn no_one_at(&mut self, station: i32) {
        if !self.product || self.observer.is_some() || self.input.data_mode() { return }
        let Some(name) = usize::try_from(station).ok().and_then(|i| NO_ONE_AT.get(i)) else { return };
        let keep = ([VIEW, 0x49EE, 0x2A15, 0x2792, 0x2794].map(|at| (at, self.s.word(at))), self.r.pen);
        for p in 0..2 { self.r.fb.copy(p, 86, 0, 14, 40, 7, 14 * p as i32, 0); }
        super::play::notice(self);
        self.say(format!("NO ONE AT {name}: ASSIGN FIRST").as_bytes(), b"");
        for p in 0..2 { self.r.fb.copy(7, 14 * p as i32, 0, 14, 40, p, 86, 0); }
        for (at, v) in keep.0 { self.s.set_word(at, v); }
        self.r.pen = keep.1;
    }
    fn flight_inner(&mut self,mut resume:Option<Flight>) -> i16 {
        let (scroll,mut old_x,mut old_y,mut command,mut stop)=if let Some(f)=&resume {(f.scroll,f.old_x,f.old_y,f.command,f.stop)} else {
        let scroll: [i16; 10] = std::array::from_fn(|i| self.s.word(0xD82 + 2 * i as u16));
        self.s.set_word(VIEW, -1);
        self.s.set_word(0x34CC, 1); let now = self.ticks(); self.s.set_dword(0x3482, now.wrapping_add(18));
        self.s.set_word(0x34C8, 1); let now = self.ticks(); self.s.set_dword(0x347A, now.wrapping_add(72));
        self.s.set_dword(0x347E, 0); self.s.set_word(HEADING, 90); self.s.set_word(MOVING, 0);
        self.r.update_minimap_quadrant(&mut self.s);
        let (old_x, old_y) = (self.s.word(X), self.s.word(Y));
        self.mouse_bounds(false); self.show_cursor();
        (scroll,old_x,old_y,0x1357i16,false)
        };
        let mut opening = true;
        loop {
            if resume.is_none() {
            if (old_x, old_y) != (self.s.word(X), self.s.word(Y)) {
                let q = self.s.word(QUADRANT); self.r.plot_minimap(&mut self.s, old_x, old_y, q, 0); self.r.update_minimap_quadrant(&mut self.s);
            }
            self.r.back(&mut self.s);
            if self.fast() || self.s.word(VIEW) == -1 {
                if self.draw_system_view(false) != 0 { self.draw_thrust(7); }
                self.marker(); self.update_system_minimap(); self.heading_image(); if self.product { self.panel_words() } self.flip_with_cursor();
                self.s.set_word(DRAW_COUNTER, self.s.word(DRAW_COUNTER).wrapping_add(30));
            } else {
                let (a, d) = (self.s.word(ACTIVE_PAGE) as usize & 1, self.draw_page());
                self.copy(a, 12, 1, 160, 30, d, 12, 1);
                self.marker(); self.update_system_minimap(); self.heading_image(); if self.product { self.panel_words() } self.flip_with_cursor();
            }
            if self.s.word(VIEW) == -1 { self.s.set_word(VIEW, 0); }
            if self.docked() {
                let c = self.s.word(CURRENT);
                if self.s.array(KIND, c) == 4 && self.s.pbyte(0xFC) < 124 && self.s.word(DRAW_COUNTER) % 10 == 0 && self.s.pbyte(0xC1) & 24 == 24 {
                    self.s.set_pbyte(0xFC, self.s.pbyte(0xFC).wrapping_add(1));
                    if self.s.pbyte(0xFC) != 0 {
                        let f = self.s.pbyte(0xFC) as i16;
                        self.r.front(&mut self.s); self.r.fill(&self.s, 183, 4, 5, f, 9);
                        self.r.back(&mut self.s); self.r.fill(&self.s, 183, 4, 5, f, 9);
                    }
                }
                self.draw_thrust(7);
                for f in [VX, VY, THRUST, MOVING] { self.s.set_word(f, 0); }
            }
            self.run_tasks(-1);
            if self.dead { return -1; }
            if self.s.word(MOVING) == 0 { self.draw_thrust(7); self.s.set_word(THRUST, 0); }
            }
            let old_direction=resume.take().map_or_else(||self.s.word(DIRECTION),|f|f.old_direction);
            super::play::image::offer(self,Flight {scroll,old_x,old_y,command,stop,old_direction});
            super::play::flight(self);
            let mut handle = self.s.byte(KEY) != 0 || self.s.byte(PENDING_KEY) != 0 || self.s.word(ENCOUNTER) > -1;
            if handle {
                if self.s.byte(KEY) == 0 && self.s.byte(PENDING_KEY) != 0 { let k = self.s.byte(PENDING_KEY); self.s.set_byte(KEY, k); }
                if self.s.byte(KEY) == 110 {
                    self.mouse_command(super::mouse::FLIGHT_HITS);
                    if self.s.byte(KEY) == 110 { self.s.set_byte(KEY, 0); handle = false; }
                    else if self.s.word(CURRENT) >= 0 {
                        if self.s.byte(KEY) == 36 { self.s.set_byte(KEY, 38); }
                        if self.s.byte(KEY) == 16 { self.s.set_byte(KEY, 20); }
                    }
                }
            }
            if opening && self.product && !self.input.data_mode() && !self.docked() && self.s.word(ENCOUNTER) < 0 { opening = false; self.pause(b"PAUSED - press P to fly"); continue }
            opening = false;
            if handle && self.product && self.s.word(ENCOUNTER) < 0 && self.s.byte(KEY) == 0x19 { self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.pause(b"PAUSED"); continue }
            if handle && self.product && self.s.word(ENCOUNTER) < 0 && crate::save::is_key(self.s.byte(KEY)) { self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.say(b"NO SAVE OR LOAD IN FLIGHT", b"LAND FIRST"); continue }
            if handle {
                self.hide_cursor();
                command = if self.s.word(ENCOUNTER) > -1 { 83 } else { self.translate_station_key() };
                self.show_cursor();
                match command {
                    1 | 2 => {
                        if self.s.pbyte(0xFC) == 0 { self.s.set_word(MOVING, 0); }
                        else if !self.docked() {
                            stop = false; self.draw_thrust(7);
                            if command == 1 {
                                if self.s.word(THRUST) < 2 { self.s.set_word(THRUST, self.s.word(THRUST).wrapping_add(1)); }
                                self.draw_thrust(9); self.s.set_word(MOVING, (self.s.word(THRUST) != 0) as i16);
                            } else {
                                if self.s.word(THRUST) > -2 { self.s.set_word(THRUST, self.s.word(THRUST).wrapping_sub(1)); }
                                self.s.set_word(MOVING, (self.s.word(THRUST) != 0) as i16); self.draw_thrust(9);
                            }
                        }
                    }
                    9 => stop = true,
                    3 | 4 => if self.s.pbyte(0xC3) < 100 {
                        let d = self.s.word(DIRECTION).wrapping_add(if command == 3 { 1 } else { -1 });
                        self.s.set_word(DIRECTION, if d > 8 { 1 } else if d < 1 { 8 } else { d });
                    },
                    84 => if self.s.word(CURRENT) >= 0 && self.s.pbyte(0xFC) != 0 {
                        self.s.set_word(THRUST, 2);
                        self.r.front(&mut self.s); self.hud(99); self.hud(-1); self.draw_thrust(9);
                        self.r.back(&mut self.s); self.hud(99); self.hud(-1); self.draw_thrust(9);
                        self.s.set_word(VIEW, -1);
                        self.s.set_word(DEPARTED, self.s.word(CURRENT).wrapping_neg()); self.s.set_word(CURRENT, -1);
                        self.s.set_word(VIEW, self.s.word(HEADING).wrapping_sub(90)); self.s.set_word(MOVING, 1);
                    },
                    74 => if self.s.word(CURRENT) == -1 {
                        self.hide_cursor(); self.mouse_bounds(true);
                        let from = self.s.pbyte(6);
                        if let Err(e) = self.jump_screen() { eprintln!("jump: {e}"); if self.product { self.restore_flight(); self.pause(b"JUMP FAILED - press a key"); } }
                        else if self.s.pbyte(6) != from { self.arrived(); }
                        self.jump_asked = None;
                        self.mouse_bounds(false); self.show_cursor();
                    },
                    76 => if self.s.word(CURRENT) != -1 && self.s.array(WORLD, self.s.word(CURRENT)) != 0 {
                        for frame in 0..3 {
                            self.snapshot_clock();
                            if frame == 0 { self.r.front(&mut self.s); }
                            let a = self.s.word(ACTIVE_PAGE) as usize & 1; self.copy(2, 171, 0, 16, 2, a, 100, 16);
                            if frame < 2 { let h = if frame == 0 { self.s.word(HEADING).wrapping_sub(9) } else { 89 }; self.r.layout(&self.s, h, 100, 128); }
                            self.wait_with_input(8);
                        }
                        command = 27;
                    },
                    27 => command = 78,
                    81 | 67 | 83 => {
                        if command == 81 && self.s.word(CURRENT) == -1 {
                            command = self.confirm_quit();
                            if command != 27 { self.restore_flight(); }
                        } else if command == 83 || self.s.pbyte(0xA5) != 0 {
                            if command != 83 { self.s.set_word(PREVIOUS_STATION, 15); }
                            let saved_thrust = self.s.word(THRUST);
                            self.mouse_bounds(true);
                            command = 0;
                            while command < self.s.word(SHIP_COUNT) {
                                let (sx, sy) = (self.s.enc(command, 0), self.s.enc(command, 2));
                                let (px, py) = (self.s.word(X), self.s.word(Y));
                                if sx.wrapping_sub(4) < px && px < sx.wrapping_add(4) && sy.wrapping_sub(4) < py && py < sy.wrapping_add(4) {
                                    self.s.set_word(ENCOUNTER, command.wrapping_add(1)); self.init_encounter(); break;
                                }
                                command = command.wrapping_add(1);
                            }
                            self.s.set_word(STATION_ACTIVE, 1); self.s.set_word(TACTICAL_DRAWN, 0);
                            self.hide_cursor(); self.run_stations(); self.hide_cursor();
                            if self.dead { return -1; }
                            if self.s.word(ENCOUNTER) > -1 {
                                if self.s.pbyte(0xC6) > 100 { self.ship_destroyed(1); }
                                for i in 0..225u16 { self.s.set_byte(GRID.wrapping_add(i), 0); }
                                self.s.set_word(ENCOUNTER, self.s.word(ENCOUNTER).wrapping_neg());
                                if self.s.word(TACTICAL_X) != 4 || self.s.word(TACTICAL_Y) != 4 { self.s.set_word(CURRENT, -1); }
                                self.s.set_word(X, self.s.word(X).wrapping_add(self.s.word(TACTICAL_X)).wrapping_sub(4));
                                self.s.set_word(Y, self.s.word(Y).wrapping_add(self.s.word(TACTICAL_Y)).wrapping_sub(4));
                                self.s.set_word(MOVING, (self.s.word(THRUST) != 0) as i16); self.s.set_word(CURRENT, -1);
                            } else { self.s.set_word(THRUST, saved_thrust); }
                            self.restore_flight();
                        } else if command == 67 { self.no_comms(); }
                    }
                    _ => { self.wait_with_input(-1); }
                }
            }
            if stop { self.draw_thrust(7); for f in [THRUST, MOVING, VX, VY] { self.s.set_word(f, 0); } stop = false; }
            self.readout(true);
            if self.fast() || self.s.word(THRUST) != 0 || self.s.word(DIRECTION) != old_direction {
                if self.s.word(VX) != 0 || self.s.word(VY) != 0 { self.snapshot_clock(); }
                let (dx, dy) = match self.s.word(DIRECTION) { 1 => (1, 0), 2 => (1, -1), 3 => (0, -1), 4 => (-1, -1), 5 => (-1, 0), 6 => (-1, 1), 7 => (0, 1), 8 => (1, 1), _ => (0, 0) };
                let t = self.s.word(THRUST);
                self.s.set_word(VX, self.s.word(VX).wrapping_add((dx as i16).wrapping_mul(t)));
                self.s.set_word(VY, self.s.word(VY).wrapping_add((dy as i16).wrapping_mul(t)));
                for off in [VX, VY] { let v = self.s.word(off).clamp(-16, 16); self.s.set_word(off, v); }
                let speed = (self.s.word(VX).abs() + self.s.word(VY).abs()) / 2;
                let (xc, yc) = (velocity_class(self.s.word(VX)), velocity_class(self.s.word(VY)));
                self.s.set_word(VIEW, scroll[(xc * 3 + yc) as usize]);
                (old_x, old_y) = (self.s.word(X), self.s.word(Y));
                for (pos, vel) in [(Y, VY), (X, VX)] {
                    let v = self.s.word(vel);
                    if v < -2 { self.s.set_word(pos, self.s.word(pos).wrapping_sub(1)); } else if v > 2 { self.s.set_word(pos, self.s.word(pos).wrapping_add(1)); }
                }
                for (pos, vel) in [(X, VX), (Y, VY)] {
                    if self.s.word(pos) < 5 { self.s.set_word(pos, 5); self.s.set_word(vel, 0); }
                    if self.s.word(pos) > 0x16F { self.s.set_word(pos, 0x16F); self.s.set_word(vel, 0); }
                }
                self.ticks();
                if !self.fast() && self.s.word(DIRECTION).wrapping_add(89) != self.s.word(HEADING) {
                    self.r.back(&mut self.s); self.marker();
                    self.s.set_word(HEADING, self.s.word(DIRECTION).wrapping_add(89)); self.heading_image();
                } else if self.s.word(VX) != 0 || self.s.word(VY) != 0 {
                    self.s.set_word(HEADING, self.s.word(DIRECTION).wrapping_add(89));
                    self.wait_with_input(18 - speed);
                }
            }
            let c = self.s.word(CURRENT);
            if [26, 37].contains(&self.s.array(SPRITE, c)) {
                if self.s.pbyte(0xC6) < 100 { self.s.set_pbyte(0xC6, self.s.pbyte(0xC6).wrapping_add(1)); } else { self.ship_destroyed(1); }
            }
            if self.dead { return -1; }
            if command == 27 { return self.s.word(CURRENT); }
        }
    }
}
pub fn velocity_class(value: i16) -> i16 { if value < -2 { 0 } else if value > 2 { 2 } else { 1 } }
#[cfg(test)]
mod t {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir, Event, Input, Key}, sim::tick::{Clock, Counter}};
    struct After(u32);
    impl Input for After {
        fn next_key(&mut self) -> Key { 27 }
        fn poll(&mut self, c: &mut dyn Clock) -> Event { if self.0 == 0 { Event::Key(32) } else { self.0 -= 1; c.wait_tick(); Event::Idle } }
    }
    #[test]
    fn pause_moves_every_task_deadline() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(After(5)), Box::new(Counter::at(10)), Box::new(Dir(dir))).unwrap();
        sp.product = true; sp.r.initialize(&mut sp.s);
        for i in 0..10u16 { sp.s.set_dword(super::super::tick::DEADLINES + 4 * i, 12 + i as u32); }
        sp.pause(b"PAUSED");
        let after: Vec<u32> = (0..10u16).map(|i| sp.s.dword(super::super::tick::DEADLINES + 4 * i)).collect();
        assert_eq!(after, (0..10).map(|i| 17 + i).collect::<Vec<u32>>());
    }
    #[test]
    fn the_readout_shows_the_thrust_and_the_dry_tank_and_moves_no_sim_byte() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(After(0)), Box::new(Counter::at(10)), Box::new(Dir(dir))).unwrap();
        sp.r.initialize(&mut sp.s); sp.s.set_word(CURRENT, -1);
        let ink = |sp: &Space, (y0, x0, h, w): (i32, i32, i32, i32), c: u8| (0..2).map(|p| (y0..y0 + h).flat_map(|y| (x0..x0 + w).map(move |x| (y, x))).filter(|&(y, x)| sp.r.fb.get(p, y, x) == c).count()).collect::<Vec<_>>();
        let (number, words) = ((158, 293, 19, 21), (175, 135, 21, 151));
        let draw = |sp: &mut Space| { let before = (sp.s.image().to_vec(), sp.s.party.clone(), sp.r.pen.colour, sp.r.pen.op); sp.readout(true); assert!(before == (sp.s.image().to_vec(), sp.s.party.clone(), sp.r.pen.colour, sp.r.pen.op), "a sim byte or the pen moved"); };
        draw(&mut sp); assert_eq!((ink(&sp, number, 7), ink(&sp, words, 15)), (vec![0, 0], vec![0, 0]), "faithful draws nothing");
        sp.product = true; draw(&mut sp);
        assert!(ink(&sp, number, 0).iter().all(|&n| n > 5) && ink(&sp, words, 15).iter().all(|&n| n > 100), "0 and the fuel words on both pages");
        sp.s.set_pbyte(0xFC, 9); sp.s.set_word(THRUST, 2); draw(&mut sp);
        assert!(ink(&sp, number, 9).iter().all(|&n| n > 5) && ink(&sp, number, 0) == [0, 0] && ink(&sp, words, 15) == [0, 0], "2 in the forward ink; the words go when there is fuel");
        sp.s.set_word(THRUST, -1); draw(&mut sp);
        assert!(ink(&sp, number, 2).iter().all(|&n| n > 8) && ink(&sp, number, 9) == [0, 0], "-1 in the reverse ink");
    }
    #[test]
    fn the_fuel_monitor_title_carries_the_percent() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(After(0)), Box::new(Counter::at(10)), Box::new(Dir(dir))).unwrap();
        sp.r.initialize(&mut sp.s); sp.s.set_word(CURRENT, -1); sp.frame();
        assert_eq!([0u8, 1, 62, 124].map(super::super::render::fuel_percent), [0, 1, 50, 100]);
        let shot = |sp: &Space| (0..2).map(|p| (174..183).map(|y| (72..128).map(|x| sp.r.fb.get(p, y, x)).collect::<Vec<_>>()).collect::<Vec<_>>()).collect::<Vec<_>>();
        let plain = shot(&sp);
        let draw = |sp: &mut Space, fuel: u8| { sp.s.set_pbyte(0xFC, fuel); let before = (sp.s.image().to_vec(), sp.s.party.clone()); sp.readout(true); assert!(before == (sp.s.image().to_vec(), sp.s.party.clone()), "a sim byte moved"); shot(sp) };
        assert_eq!(draw(&mut sp, 62), plain, "faithful draws nothing");
        sp.product = true;
        let (half, full, none) = (draw(&mut sp, 62), draw(&mut sp, 124), draw(&mut sp, 0));
        for s in [&half, &full, &none] {
            assert_eq!(s[0], s[1], "both pages");
            assert!(s[0][0] == plain[0][0] && s[0][8] == plain[0][8], "the strip's border rows are whole");
            assert!(s[0].iter().flatten().filter(|&&c| c == 0).count() > 12, "a number is drawn");
        }
        assert!(half != full && full != none && half != none, "the number follows the fuel");
    }
    type Changes = std::rc::Rc<std::cell::RefCell<Vec<(u32, usize, usize)>>>;
    struct Timeline { keys: Vec<(u32, u16)>, up: Option<u16>, polls: u32, now: u32, first: Vec<u32>, last: Vec<u32>, changes: Changes }
    impl Input for Timeline {
        fn next_key(&mut self) -> Key { 27 }
        fn present(&mut self, rgb: &[u32]) {
            if self.first.is_empty() { self.first = rgb.to_vec(); }
            if self.last != rgb {
                let (top, first) = (*rgb.iter().max().unwrap(), &self.first); self.last = rgb.to_vec();
                let words = (86..114usize).flat_map(|y| (12..244usize).map(move |x| y * 320 + x)).filter(|&i| rgb[i] == top).count();
                self.changes.borrow_mut().push((self.now, rgb.iter().zip(first).filter(|(a, b)| a != b).count(), words));
            }
        }
        fn poll(&mut self, c: &mut dyn Clock) -> Event {
            self.now = c.now();
            if let Some(k) = self.up.take() { return Event::KeyUp(k) }
            if let Some(&(at, k)) = self.keys.first() { if c.now() >= at { self.keys.remove(0); self.up = Some(k); return Event::Key(k) } }
            self.polls += 1; if self.polls % 4 == 0 { c.wait_tick(); } Event::Idle
        }
    }
    fn pad(keys: &[(u32, u16)]) -> Option<(Space, Vec<(u32, usize, usize)>)> {
        let dir = oracle_env::gamefiles()?;
        let mut party = vec![0; PARTY_SIZE]; party[6] = 2; party[0xFC] = 100; for i in 0..5 { party[i * 256 + 0x127] = 9; }
        let changes = Changes::default();
        let input = Timeline { keys: keys.to_vec(), up: None, polls: 0, now: 0, first: vec![], last: vec![], changes: changes.clone() };
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), party, vec![], Box::new(input), Box::new(Counter::at(10)), Box::new(Dir(dir))).unwrap();
        sp.product = true;
        sp.space_main(b"0A");
        let seen = changes.borrow().clone(); Some((sp, seen))
    }
    #[test]
    fn the_first_flight_frame_has_its_stars_whatever_the_view_word() {
        struct First(std::rc::Rc<std::cell::RefCell<Vec<u32>>>, u32);
        impl Input for First {
            fn next_key(&mut self) -> Key { 27 }
            fn present(&mut self, rgb: &[u32]) { if self.0.borrow().is_empty() { *self.0.borrow_mut() = rgb.to_vec(); } }
            fn poll(&mut self, c: &mut dyn Clock) -> Event { self.1 += 1; c.wait_tick(); match self.1 { 40 => Event::Key(b'T' as u16), 400 => Event::Key(b'Q' as u16), 440 => Event::Key(b'Y' as u16), _ => Event::Idle } }
        }
        let Some(dir) = oracle_env::gamefiles() else { return };
        let stars = |view: i16| {
            let mut party = vec![0; PARTY_SIZE]; party[6] = 2; party[0xFC] = 100; for i in 0..5 { party[i * 256 + 0x127] = 9; }
            let first: std::rc::Rc<std::cell::RefCell<Vec<u32>>> = Default::default();
            let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), party, vec![], Box::new(First(first.clone(), 0)), Box::new(Counter::at(10)), Box::new(Dir(dir.clone()))).unwrap();
            sp.s.set_word(VIEW, view); sp.space_main(b"0A");
            let f = first.borrow(); (30..150usize).flat_map(|y| (20..110usize).map(move |x| y * 320 + x)).filter(|&i| f[i] & 0xFFFFFF != 0).count()
        };
        let (plain, preset) = (stars(-1), stars(3));
        assert!(plain > 40, "the 1990 launch draws stars: {plain}");
        assert_eq!(preset, plain, "the same field with the view word preset");
    }
    #[test]
    fn one_take_off_press_lifts_the_ship() {
        let Some((sp, seen)) = pad(&[(30, b'T' as u16), (300, b'Q' as u16), (310, b'Y' as u16)]) else { return };
        let moved: Vec<u32> = seen.iter().map(|c| c.0).filter(|&t| t > 31 && t < 300).collect();
        assert!(moved.first().is_some_and(|&t| t < 80) && moved.len() > 5, "the ship flies on one press: frame changes at {moved:?}");
        assert_eq!(sp.s.word(CURRENT), -1);
    }
    #[test]
    fn commo_always_answers() {
        let keys = [(30, b'C' as u16), (60, b'T' as u16), (90, b'T' as u16), (200, b'Q' as u16), (210, b'Y' as u16)];
        let Some((sp, seen)) = pad(&keys) else { return };
        let at = |from: u32, to: u32| seen.iter().filter(|c| c.0 >= from && c.0 < to).map(|c| c.2).collect::<Vec<_>>();
        assert!(at(30, 60).iter().any(|&n| n > 400), "C draws the two-line notice: {seen:?}");
        assert!(at(60, 64).contains(&0) && at(64, 90).is_empty(), "the first T only dismisses it; the ship stays on the pad: {seen:?}");
        assert!(at(91, 200).len() > 5 && sp.s.word(CURRENT) == -1, "the second T takes off");
        let (f5, f6) = (mt_engine::io::keys::F1 + 4, mt_engine::io::keys::F1 + 5);
        let keys = [(30, b'S' as u16), (40, b'A' as u16), (50, f5), (60, f6), (70, 27), (100, b'C' as u16), (130, 27), (160, b'T' as u16), (260, b'Q' as u16), (270, b'Y' as u16)];
        let Some((sp, seen)) = pad(&keys) else { return };
        let size = |from: u32, to: u32| seen.iter().filter(|c| c.0 >= from && c.0 < to).map(|c| c.1).max().unwrap_or(0);
        assert_eq!((sp.s.pbyte(0xA4), sp.s.pbyte(0xA5)), (0, 5), "the engineer sits at communications");
        assert!(size(72, 100) < 8000 && size(100, 130) > 20000, "flight after the Esc, then C opens the communications station: {seen:?}");
        assert_eq!((sp.s.word(STATION_ACTIVE), sp.s.word(CURRENT)), (0, -1), "Esc returns to flight, T takes off");
    }
}
