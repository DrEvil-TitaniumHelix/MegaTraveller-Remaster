use super::{
    render,
    state::{Far, Ground, Mem},
};
use mt_engine::io::audio;
#[path = "ending.rs"]
mod ending;
pub trait Services {
    fn call(&mut self, g: &mut Ground, name: &'static str, args: &[u16]) -> Option<u32>;
}
struct Live;
impl Services for Live {
    fn call(&mut self, _: &mut Ground, _: &'static str, _: &[u16]) -> Option<u32> {
        None
    }
}
struct Context<'a> {
    g: &'a mut Ground,
    services: &'a mut dyn Services,
    fuel: usize,
    initial_second: i32,
}
fn i16(v: i32) -> i32 {
    v as i16 as i32
}
fn far(v: u32) -> Far {
    Far::new(v as u16, (v >> 16) as u16)
}
fn packed(p: Far) -> u32 {
    p.off as u32 | ((p.seg as u32) << 16)
}
impl Context<'_> {
    fn r(&self, p: Far, o: i32, n: usize, s: bool) -> i32 {
        if s {
            self.g.mem.read_i(p.add(o), n)
        } else {
            self.g.mem.read(p.add(o), n) as i32
        }
    }
    fn b(&self, p: Far, o: i32) -> i32 {
        self.r(p, o, 1, false)
    }
    fn sb(&self, p: Far, o: i32) -> i32 {
        self.r(p, o, 1, true)
    }
    fn w(&self, p: Far, o: i32) -> i32 {
        self.r(p, o, 2, true)
    }
    fn wb(&mut self, p: Far, o: i32, v: i32) {
        self.g.mem.write(p.add(o), v as u32, 1)
    }
    fn ww(&mut self, p: Far, o: i32, v: i32) {
        self.g.mem.write(p.add(o), v as u32, 2)
    }
    fn party(&self, o: i32) -> i32 {
        self.g.mem.party_u8(o as u16) as i32
    }
    fn sparty(&mut self, o: i32, v: i32) {
        self.g.mem.set_party_u8(o as u16, v as u8)
    }
    fn cell(&self, x: i32, y: i32) -> Far {
        self.g.mem.pointer(0xbc).add(x.wrapping_mul(self.g.mem.map_stride() as i32).wrapping_add(y).wrapping_mul(3))
    }
    fn call(&mut self, name: &'static str, args: &[i32]) -> u32 {
        let words: Vec<u16> = args.iter().map(|&a| a as u16).collect();
        if let Some(v) = self.services.call(self.g, name, &words) {
            self.g.mem.trace.call(name, &words, Some(v));
            return v;
        }
        if let Some(routine) = routine_name(name) {
            return self.invoke(routine, &words);
        }
        let v = self.leaf(name, &words);
        self.g.mem.trace.call(name, &words, Some(v));
        v
    }
    fn invoke(&mut self, name: &'static str, args: &[u16]) -> u32 {
        if matches!(name,"npc_tick"|"move_party") && self.g.boot_diagnostics {
            self.g.observe_boot(name,&args.iter().map(|&a|a as i16).collect::<Vec<_>>());
        }
        if self.fuel == 0 {
            return 0;
        }
        self.fuel -= 1;
        if self.fuel == 0 { eprintln!("ground: walk call budget spent at {name}: every further 1990 routine in this call returns 0 (a runaway loop or a desync)") }
        let party_len = self.g.mem.party().len();
        self.g.mem.trace_enter(name, args);
        let a = |i: usize| args.get(i).copied().unwrap_or(0) as i16 as i32;
        let p = |i: usize| Far::new(args.get(i).copied().unwrap_or(0), args.get(i + 1).copied().unwrap_or(0));
        let result = match name {
            "walker_set_pose" => {
                self.walker_set_pose(p(0));
                0
            }
            "try_reload" => self.try_reload(a(0)),
            "npc_can_step" => self.npc_can_step(a(0), a(1), p(2)),
            "move_party" => self.move_party(a(0), a(1), a(2)),
            "npc_tick" => self.npc_tick(a(0)),
            "set_leader" => {
                self.set_leader(a(0));
                0
            }
            "scroll_view_to_player" => {
                self.scroll_view_to_player(a(0));
                0
            }
            "party_order" => {
                self.party_order(a(0));
                0
            }
            "try_move" => self.try_move(a(0), a(1)),
            "vehicle_terrain" => self.vehicle_terrain(p(0), a(2)),
            "foot_terrain" => self.foot_terrain(p(0), a(2)),
            "set_cell_flags" => {
                self.set_cell_flags(a(0), a(1), a(2));
                0
            }
            "release_cell" => {
                self.release_cell(a(0), a(1));
                0
            }
            "atmosphere_hostile" => self.atmosphere_hostile(),
            "atmosphere_tainted" => self.atmosphere_tainted(),
            "atmosphere_tick" => {
                self.atmosphere_tick();
                0
            }
            "air_tick" => {
                self.air_tick();
                0
            }
            "self_destruct_tick" => {
                self.self_destruct_tick();
                0
            }
            "alarm_tick" => {
                self.alarm_tick();
                0
            }
            "check_party_alive" => {
                self.check_party_alive();
                0
            }
            "resolve_event" => {
                self.resolve_event();
                0
            }
            "sync_leader_pos" => {
                self.sync_leader_pos();
                0
            }
            "play_pending_sounds" => {
                self.play_pending_sounds();
                0
            }
            _ => 0,
        };
        if name == "try_move" && party_len == Mem::PARTY_SIZE {
            if let Some(p) = self.g.mem.segs.get_mut(&Mem::PARTY_SEG) {
                p.truncate(party_len)
            }
        }
        let ax = (!is_void(name)).then_some(result as u16);
        self.g.mem.trace.exit(ax);
        if self.g.mem.trace.on {
            if let Some(frame) = self.g.mem.trace.done.last_mut() {
                for call in &mut frame.calls {
                    call.name = match call.name {
                        "try_move" => "can_enter",
                        "scroll_view_to_player" => "update_position",
                        "set_leader" => "select_leader",
                        other => other,
                    };
                }
            }
        }
        self.g.stream_trace();
        result as u16 as u32
    }
    fn walker_set_pose(&mut self, w: Far) {
        let d = w.off as i32 - 0x5c54;
        let index = self.call("long_divide", &[d, if d < 0 { -1 } else { 0 }, 86, 0]) as i16 as i32;
        let slot = if self.b(w, 3) != 0 {
            16
        } else {
            let df = self.call("item_def", &[index, self.b(w, 2) + 21]);
            if df == 0 {
                self.wb(w, 5, 0);
                return;
            }
            self.b(far(df), 0x25)
        };
        let skill = self.party((index << 8) + slot + 0x12b);
        self.wb(
            w,
            6,
            if skill == 0 {
                5
            } else if skill == 1 {
                4
            } else if skill <= 3 {
                3
            } else if skill <= 6 {
                2
            } else {
                1
            },
        );
    }
    fn try_reload(&mut self, index: i32) -> i32 {
        let w = Mem::walker(index as i16);
        let item = self.call("item_ptr", &[index, self.b(w, 2) + 21]);
        let df = far(self.call("item_def", &[index, self.b(w, 2) + 21]));
        let it = far(item);
        if item != 0 && self.r(df, 0x1d, 2, false) == 2 && self.b(df, 0x27) & 32 != 0 && self.b(it, 1) != 0 && self.b(it, 2) == 0 {
            self.wb(it, 1, self.b(it, 1) - 1);
            self.wb(it, 2, self.b(df, 0x28));
            if index == self.g.mem.leader() as i32 {
                self.call("walker_panel", &[0, 1]);
            }
            return 1;
        }
        0
    }
    fn npc_can_step(&mut self, x: i32, y: i32, npc: Far) -> i32 {
        let cell = self.cell(x, y);
        let (flags, terrain, occupied) = (self.b(cell, 0), self.b(cell, 1), self.b(cell, 2));
        let (wall, leash, mode) = (terrain / 20, self.b(npc, 7) >> 3, self.b(npc, 7) & 7);
        if mode == 4 {
            return 0;
        }
        let (tx, ty) = (self.w(npc, 32), self.w(npc, 34));
        if leash != 0 && (i16(i16(x - (self.w(npc, 3) >> 1)).abs()) >= leash || i16(i16(y - (self.w(npc, 5) >> 1)).abs()) >= leash) {
            return 0;
        }
        if (self.g.mem.x() as i32 >> 1) == x
            && (self.g.mem.y() as i32 >> 1) == y
            && self.g.mem.encounter_mode() != 2
            && self.g.mem.individual() == 0
        {
            return 0;
        }
        if occupied & 0x7f != 0 {
            return 0;
        }
        if self.g.mem.combat() != 0 {
            for i in 0..5 {
                let w = Mem::walker(i);
                if self.b(w, 0) != 0 && i16(x * 2) == self.w(w, 8) && i16(y * 2) == self.w(w, 10) {
                    return 0;
                }
            }
            if self.b(npc, 31) != 0
                && (i16(x * 2) < self.g.mem.view_x() as i32
                    || i16(y * 2) < self.g.mem.view_y() as i32
                    || (self.g.mem.view_x() as i32 >> 1) + 7 <= x
                    || (self.g.mem.view_y() as i32 >> 1) + 12 <= y)
            {
                return 0;
            }
        }
        if flags & 1 == 0 && matches!(terrain, 38 | 39) {
            return 0;
        }
        if mode & 3 != 0 {
            let water = flags & 1 != 0 && (0x41..=0x5e).contains(&terrain);
            let land = flags & 1 == 0 || (0x28..=0x40).contains(&terrain);
            if (mode == 1 && !water) || (mode == 2 && !land) || !(water || land) {
                return 0;
            }
        }
        if flags & 1 != 0 {
            return (terrain >= 5 && wall < 5) as i32;
        }
        if matches!(terrain, 15 | 16) {
            self.wb(cell, 1, terrain + 20);
            if self.call("hit_npc", &[tx >> 1, ty >> 1, 1, 25]) != 0 {
                self.call("set_cell_flags", &[x * 2, y * 2, 0x80]);
                self.call("npc_dies", &[tx >> 1, ty >> 1]);
                self.call("set_cell_flags", &[tx, ty, 0]);
                self.g.mem.set_event_id(-1);
            }
            return 1;
        }
        (wall == 1) as i32
    }
    fn append(&mut self, w: Far, d: i32) {
        self.wb(w, 23 + self.sb(w, 22), d);
        self.wb(w, 22, self.sb(w, 22) + 1)
    }
    fn move_party(&mut self, dx: i32, dy: i32, direction: i32) -> i32 {
        if dx == 0 && dy == 0 {
            return 1;
        }
        if self.g.mem.phase() == 0 && (self.g.mem.npc_mode() != 0 || self.g.mem.individual() == 0) {
            self.g.mem.set_phase(1)
        }
        let w = Mem::walker(self.g.mem.leader());
        if self.g.mem.npc_mode() == 0 {
            if self.g.mem.individual() != 0 {
                self.wb(w, 5, 0)
            } else if self.g.mem.orders() == 0 {
                self.ww(w, 16, 0);
                self.ww(w, 18, 0);
                self.ww(w, 20, 0);
                self.wb(w, 22, 0)
            }
        }
        if direction != self.g.mem.heading() as i32 && self.g.mem.individual() == 0 {
            if self.g.mem.orders() != 0 {
                if self.sb(w, 22) == 62 {
                    self.call("beep", &[]);
                    return 1;
                }
                self.append(w, direction)
            }
            let h = self.g.mem.heading() as i32;
            let step = if (direction < h && i16(h - direction) <= 4) || (direction > h && i16(direction - h) > 4) { -1 } else { 1 };
            if self.g.mem.npc_mode() == 0 {
                for _ in 0..8 {
                    if direction == self.g.mem.heading() as i32 {
                        break;
                    }
                    self.g.mem.set_turning(1);
                    self.call(
                        "invalidate_rect",
                        &[
                            i16((self.g.mem.x() as i32 - self.g.mem.view_x() as i32) * 8 + 54),
                            i16(self.g.mem.y() as i32 - self.g.mem.view_y() as i32),
                            16,
                            2,
                        ],
                    );
                    self.g.mem.set_heading(((self.g.mem.heading() as i32 + step) & 7) as i16);
                    self.call("mark_time", &[]);
                    self.call("redraw", &[]);
                    self.call("wait_ticks", &[1]);
                }
            } else {
                self.g.mem.set_heading(direction as i16)
            }
            self.call("invalidate_rect", &[54, 0, 112, 24]);
            self.g.mem.set_turning(0);
            if self.g.mem.orders() == 0 {
                self.g.mem.set_phase(self.g.mem.phase() ^ 3)
            }
            return 0;
        }
        let (nx, ny) = (i16(self.g.mem.x() as i32 + dx), i16(self.g.mem.y() as i32 + dy));
        if (self.g.mem.x() | self.g.mem.y()) & 1 != 0
            || self.call("can_enter", &[(self.g.mem.x() as i32 >> 1) + dx, (self.g.mem.y() as i32 >> 1) + dy]) != 0
        {
            if self.g.mem.orders() != 0 && self.g.mem.individual() == 0 {
                if self.sb(w, 22) == 62 {
                    self.call("beep", &[]);
                    return 1;
                }
                self.append(w, direction)
            }
            self.g.mem.set_previous_x(self.g.mem.x());
            self.g.mem.set_previous_y(self.g.mem.y());
            self.g.mem.set_x(nx as i16);
            self.g.mem.set_y(ny as i16);
            if self.g.mem.orders() == 0 {
                self.g.mem.set_heading(direction as i16)
            }
            if self.g.mem.npc_mode() == 0 {
                self.call("update_position", &[1]);
            }
            if self.g.mem.orders() == 0 {
                if self.g.mem.encounter() != 0 {
                    if self.g.mem.encounter_mode() == 0 {
                        self.call("tick_party", &[25, 4]);
                    } else {
                        self.call("damage", &[0, 25, 4]);
                    }
                    self.g.mem.set_encounter(0);
                    self.call("sound", &[6]);
                }
                if self.g.mem.pending_event() != 0 {
                    self.g.mem.set_pending_event(0);
                    self.g.mem.set_encounter_mode(self.g.mem.saved_mode());
                    if self.g.scene.is_some() { super::input::ui::message(self.g, b"You board the vehicle.") }
                    self.call("release_cell", &[self.g.mem.x() as i32 + dx, self.g.mem.y() as i32 + dy]);
                    self.g.mem.set_event_id(-1);
                }
                if self.g.mem.move_back() != 0 {
                    self.call("sound", &[5]);
                }
            } else {
                self.g.mem.set_move_back(0);
                self.g.mem.set_pending_event(0);
                self.g.mem.set_encounter(0)
            }
            if self.g.mem.event() == -1 {
                if self.g.mem.update_event() != 0 {
                    let event = self.g.mem.pointer(0x6b97);
                    self.wb(event, 1, self.g.mem.event_value() as i32);
                    self.wb(event, 0, self.b(event, 0) | 1);
                    self.g.mem.set_update_event(0);
                    self.g.mem.set_event_value(0);
                    self.g.mem.set_event_id(-1)
                }
                if self.g.mem.npc_mode() != 0 {
                    if self.g.mem.phase() != 0 {
                        self.g.mem.set_phase(self.g.mem.phase() ^ 3)
                    }
                } else {
                    if self.g.mem.move_back() != 0 {
                        self.g.mem.set_move_back(0);
                        self.g.mem.set_x((self.g.mem.x() as i32 - dx) as i16);
                        self.g.mem.set_y((self.g.mem.y() as i32 - dy) as i16)
                    } else {
                        self.g.mem.set_x((self.g.mem.x() as i32 + dx) as i16);
                        self.g.mem.set_y((self.g.mem.y() as i32 + dy) as i16)
                    }
                    self.call("update_position", &[1]);
                }
            }
        } else if self.g.mem.npc_mode() == 0 {
            self.g.mem.set_move_delay(4);
            self.g.mem.set_phase(0);
            self.call("update_position", &[0]);
        }
        0
    }
}
impl Context<'_> {
    fn npc_tick(&mut self, redraw: i32) -> i32 {
        if self.g.mem.orders() != 0 {
            return 0;
        }
        self.g.mem.set_npc_attacking(1);
        self.g.mem.set_npc_mode(1);
        let phases = super::round::phases(self.g);
        for i in 0..5 {
            if phases & super::round::PHASE_AUTOFIRE == 0 { break }
            let w = Mem::walker(i);
            if i == self.g.mem.leader() || self.sb(w, 5) == 0 {
                continue;
            }
            self.wb(w, 6, self.b(w, 6) - 1);
            if self.b(w, 6) != 0 {
                continue;
            }
            let p = Mem::npc(self.sb(w, 5) as i16);
            if self.b(p, 2) != self.g.mem.level() as u8 as i32 {
                continue;
            }
            self.ww(w, 12, self.w(p, 32) & 0xfffe);
            self.ww(w, 14, self.w(p, 34) & 0xfffe);
            if self.call("party_fire", &[i as i32]) == 0 {
                if self.g.mem.npc_mode() == 2 {
                    self.g.mem.set_npc_mode(1);
                    self.wb(w, 6, 1)
                } else if self.b(w, 3) != 0 {
                    self.wb(w, 5, 0);
                    self.wb(w, 3, 0);
                    continue;
                } else if self.call("try_reload", &[i as i32]) == 0 {
                    if self.b(w, 2) == 0 {
                        self.wb(w, 2, self.b(w, 2) + 1)
                    } else {
                        self.wb(w, 5, 0);
                        continue;
                    }
                }
            }
            self.call("walker_set_pose", &[w.off as i32, w.seg as i32]);
        }
        self.g.mem.set_npc_mode(0);
        let (x0, y0) = (self.g.mem.view_x() as i32, self.g.mem.view_y() as i32);
        let (x1, y1) = (i16(x0 + 12), i16(y0 + 22));
        for i in 0..self.g.mem.npc_count() {
            if phases & super::round::PHASE_ATTACKS == 0 { break }
            let p = Mem::npc(i as i16 + 1);
            if self.b(p, 2) != self.g.mem.level() as u8 as i32 || self.b(p, 31) == 0 {
                continue;
            }
            let attitude = self.b(p, 13) & 3;
            if !(x0 <= self.w(p, 32) && self.w(p, 32) <= x1 && y0 <= self.w(p, 34) && self.w(p, 34) <= y1) {
                continue;
            }
            if (self.b(p, 38) != 0 && self.g.mem.party_fired() != 0) || attitude == 0 || (attitude == 1 && self.g.mem.provoked() != 0) {
                if (attitude == 0 && self.call("random", &[10]) as i32 <= self.b(p, 10) / 10)
                    || self.call("random", &[10]) < 3
                    || (self.g.mem.combat() != 0 && self.w(p, 42) != 0 && self.call("random", &[10]) < 5)
                {
                    self.call("monster_attack", &[p.off as i32, p.seg as i32, attitude]);
                }
            }
        }
        self.g.mem.set_npc_attacking(0);
        self.call("play_pending_sounds", &[]);
        let mut changed = 0;
        self.g.mem.set_npc_mode(1);
        let saved =
            (self.g.mem.heading(), self.g.mem.y(), self.g.mem.x(), self.g.mem.leader(), self.g.mem.phase(), self.g.mem.individual());
        self.g.mem.set_individual(0);
        self.g.mem.set_leader(0);
        while phases & super::round::PHASE_ROUTES != 0 && self.g.mem.leader() < 5 {
            let w = Mem::walker(self.g.mem.leader());
            if self.g.mem.leader() != saved.3 && self.sb(w, 22) != 0 {
                self.g.mem.set_heading(self.b(w, 1) as i16);
                self.g.mem.set_x(self.w(w, 8) as i16);
                self.g.mem.set_y(self.w(w, 10) as i16);
                self.g.mem.set_phase(self.w(w, 20) as i16);
                let d = self.sb(w, 23);
                let dx = self.g.mem.i8((0x2f5 + d * 2) as u16) as i32;
                let dy = self.g.mem.i8((0x2f6 + d * 2) as u16) as i32;
                if self.call("move_party", &[dx, dy, d]) == 0 {
                    if (self.g.mem.x() | self.g.mem.y()) & 1 == 0 {
                        self.wb(w, 22, self.sb(w, 22) - 1);
                        if self.sb(w, 22) != 0 {
                            self.call("memmove", &[w.add(23).off as i32, w.seg as i32, w.add(24).off as i32, w.seg as i32, self.sb(w, 22)]);
                        } else {
                            self.ww(w, 16, 0);
                            self.ww(w, 18, 0);
                            self.g.mem.set_phase(0)
                        }
                    }
                    changed = 1;
                }
                self.wb(w, 1, self.g.mem.heading() as i32);
                self.ww(w, 8, self.g.mem.x() as i32);
                self.ww(w, 10, self.g.mem.y() as i32);
                self.ww(w, 20, self.g.mem.phase() as i32);
            }
            self.g.mem.set_leader(self.g.mem.leader().wrapping_add(1));
        }
        self.g.mem.set_npc_mode(0);
        self.g.mem.set_heading(saved.0);
        self.g.mem.set_y(saved.1);
        self.g.mem.set_x(saved.2);
        self.g.mem.set_leader(saved.3);
        self.g.mem.set_phase(saved.4);
        self.g.mem.set_individual(saved.5);
        let (x0, y0) = (i16(self.g.mem.view_x() as i32 - 2), i16(self.g.mem.view_y() as i32 - 2));
        let (x1, y1) = (i16(x0 + 18), i16(y0 + 28));
        for i in 0..self.g.mem.npc_count() {
            if phases & super::round::PHASE_WANDER == 0 { break }
            let p = Mem::npc(i as i16 + 1);
            if self.b(p, 2) != self.g.mem.level() as u8 as i32 {
                continue;
            }
            if self.b(p, 7) == 4 {
                if self.b(p, 37) == 0 {
                    self.wb(p, 36, (self.b(p, 36) + self.sb(p, 39)) & 7);
                    let n = self.call("random", &[20]);
                    self.wb(p, 37, n as i32 + 20);
                    self.wb(p, 39, 1);
                    if self.call("random", &[10]) < 5 {
                        self.wb(p, 39, -1)
                    }
                } else {
                    self.wb(p, 37, self.b(p, 37) - 1)
                }
            } else {
                let d = if self.sb(p, 40) >= 0 { self.g.mem.u8((0xd8 + self.b(p, 36)) as u16) as u16 } else { 5 };
                let (dx, dy, _) = self.g.mem.direction(d);
                let (dx, dy) = (dx as i32, dy as i32);
                let (x, y) = (i16((self.w(p, 32) >> 1) + dx), i16((self.w(p, 34) >> 1) + dy));
                if self.w(p, 32) & 1 != 0
                    || self.w(p, 34) & 1 != 0
                    || (self.b(p, 37) > 0 && self.call("npc_can_step", &[x, y, p.off as i32, p.seg as i32]) != 0)
                {
                    if self.b(p, 2) != self.g.mem.level() as u8 as i32 {
                        continue;
                    }
                    self.call("set_cell_flags", &[self.w(p, 32), self.w(p, 34), 0]);
                    self.ww(p, 32, self.w(p, 32) + dx);
                    self.ww(p, 34, self.w(p, 34) + dy);
                    self.call("set_cell_flags", &[self.w(p, 32), self.w(p, 34), i as i32 + 1]);
                    self.wb(p, 37, self.b(p, 37) - 1);
                    if self.b(p, 7) & 3 == 0 || self.call("random", &[10]) < 3 {
                        self.wb(p, 39, 0)
                    }
                } else {
                    if self.sb(p, 39) == 0 {
                        if self.call("random", &[10]) < 5 {
                            self.wb(p, 39, 0);
                            let n = self.call("random", &[10]);
                            self.wb(p, 37, n as i32 + 5);
                            self.wb(p, 40, -1)
                        } else {
                            let n = self.call("random", &[30]);
                            self.wb(p, 39, n as i32 / 10 - 1);
                            let n = self.call("random", &[10]);
                            self.wb(p, 37, n as i32 + 2);
                            self.wb(p, 40, 1)
                        }
                    }
                    self.wb(p, 36, self.b(p, 36) + self.sb(p, 39));
                    if self.b(p, 36) == 8 {
                        self.wb(p, 36, 0)
                    } else if self.b(p, 36) == 255 {
                        self.wb(p, 36, 7)
                    }
                    if self.b(p, 37) == 0 {
                        self.wb(p, 37, 1)
                    }
                }
            }
            if x0 <= self.w(p, 32) && self.w(p, 32) <= x1 && y0 <= self.w(p, 34) && self.w(p, 34) <= y1 {
                changed = 1
            }
            if self.sb(p, 40) != -1 {
                self.wb(p, 40, self.sb(p, 40) ^ 1)
            }
        }
        if changed != 0 || self.g.mem.event_id() == -1 {
            self.g.mem.set_event_id(-1);
            if redraw != 0 {
                self.call("redraw", &[]);
            }
        }
        changed
    }
    fn atmosphere_hostile(&self) -> i32 {
        (self.g.mem.encounter_mode() == 0 && self.g.mem.outdoors() != 0 && self.g.mem.level() <= 1 && self.g.mem.atmosphere() <= 3) as i32
    }
    fn atmosphere_tainted(&self) -> i32 {
        (self.g.mem.encounter_mode() == 0
            && self.g.mem.outdoors() != 0
            && self.g.mem.level() <= 1
            && self.g.mem.u8(0x2d7 + self.g.mem.atmosphere() as u16) != 0) as i32
    }
    fn self_destruct_tick(&mut self) {
        if self.g.mem.countdown() == 0 {
            self.call("shake_screen", &[4, 50]);
            self.call("game_over", &[0xe19, Mem::DS as i32]);
        } else {
            self.g.mem.set_countdown(self.g.mem.countdown().wrapping_sub(1))
        }
        self.call("redraw", &[]);
    }
    fn play_pending_sounds(&mut self) {
        if self.g.mem.shot_sound() != 0 {
            self.call("sound", &[self.g.mem.shot_sound() as i32 - 1]);
            self.call("wait_ticks", &[3]);
            if self.g.mem.impact_sound() != 0 {
                self.call("sound", &[4]);
            }
            if self.g.mem.extra_sound() != 0 {
                self.call("sound", &[self.g.mem.extra_sound() as i32 - 1]);
            }
            self.g.mem.set_shot_sound(0);
            self.g.mem.set_extra_sound(0);
            self.g.mem.set_impact_sound(0);
            self.call("redraw", &[]);
        }
    }
    fn resolve_event(&mut self) {
        if self.g.mem.event() < 8 && self.g.mem.countdown() != 100 {
            self.g.mem.set_countdown(100);
            self.call("walker_panel", &[0, 0]);
        }
        if self.g.mem.event() < 5 {
            self.call("event_at", &[self.party(6), self.party(7), self.g.mem.event() as i32]);
        } else if self.g.mem.event() < 8 {
            self.call("event_at", &[9, 9, self.g.mem.event() as i32]);
        } else {
            self.call("world_event", &[]);
        }
        self.g.mem.set_event(-1)
    }
    fn sync_leader_pos(&mut self) {
        let w = Mem::walker(self.g.mem.leader());
        let (x, y) =
            if self.g.mem.orders() == 0 || self.w(w, 16) == 0 { (self.w(w, 8), self.w(w, 10)) } else { (self.w(w, 16), self.w(w, 18)) };
        self.g.mem.set_x(x as i16);
        self.g.mem.set_y(y as i16);
        if self.sb(w, 5) != 0 {
            let p = Mem::npc(self.sb(w, 5) as i16);
            self.ww(w, 12, self.w(p, 32));
            self.ww(w, 14, self.w(p, 34));
        }
        self.g.mem.set_individual(0);
        self.call("redraw", &[]);
    }
    fn check_party_alive(&mut self) {
        for i in 0..5 {
            if self.party((i << 8) + 0x127) > self.party((i << 8) + 0x126) {
                return;
            }
        }
        self.call("game_over", &[0x4c7b, Mem::DS as i32]);
    }
    fn air_tick(&mut self) {
        self.sparty(0x82, self.party(0x82) - 1);
        if self.party(0x82) == 0 {
            self.sparty(0x82, 135)
        }
        let mut result = 0;
        for i in 0..5 {
            if self.party((i << 8) + 0x127) == 0 {
                continue;
            }
            if self.call("both_hands_air", &[i]) == 0 {
                self.call("char_dies", &[i]);
                continue;
            }
            let rate = self.party((i << 8) + 0x118) / 5 + self.party((i << 8) + 0x170);
            let n = self.party(0x82);
            if rate == 0 || (rate == 1 && n & 1 != 0) || ((2..=4).contains(&rate) && n & 3 == 3) || (rate >= 5 && n & 7 == 7) {
                result = self.call("use_air", &[i]);
                if result == 2 {
                    break;
                }
            }
        }
        if result != 0 {
            self.call("air_panel", &[1]);
        }
    }
    fn atmosphere_tick(&mut self) {
        let tainted = self.call("atmosphere_tainted", &[]);
        if self.g.mem.outdoors() == 0 {
            return;
        }
        self.sparty(0x83, self.party(0x83) - 1);
        if self.party(0x83) != 0 {
            return;
        }
        self.sparty(0x83, 30);
        let (mut dead, mut hurt) = (false, false);
        for i in 0..5 {
            if self.party((i << 8) + 0x127) == 0 {
                continue;
            }
            let unconscious = self.party((i << 8) + 0x127) <= self.party((i << 8) + 0x126);
            if unconscious || (tainted != 0 && self.g.mem.party_u16(((i << 8) + 0x1f4) as u16) == 0) {
                if self.call("damage", &[i, 100, 1]) == 3 {
                    dead = true
                }
                hurt = true;
                if !unconscious && self.g.mem.air_warning() == 0 {
                    self.g.mem.set_air_warning(1);
                    self.call("show_message", &[0x4c48, Mem::DS as i32]);
                }
            }
        }
        if dead {
            self.call("check_deaths", &[]);
        }
        if hurt {
            self.call("redraw", &[]);
            self.call("status_panel", &[]);
        }
    }
    fn alarm_tick(&mut self) {
        self.g.mem.set_arik(self.g.mem.arik().wrapping_sub(1));
        if self.g.mem.arik() != 0 {
            return;
        }
        let mut who = 0;
        if self.party(0xc1) & 64 == 0 && self.party(6) == 1 {
            who = 0xf8
        }
        if self.party(0xc1) & 64 != 0 && self.party(6) == 5 {
            who = 0x12f
        }
        if who != 0 {
            for i in 0..self.g.mem.npc_count() {
                if self.r(Mem::npc(i as i16 + 1), 0, 2, false) == who {
                    self.g.mem.set_pursuer_active(1);
                    self.g.mem.set_pursuer_index(i);
                    if who == 0xf8 {
                        self.sparty(0xc1, self.party(0xc1) | 64)
                    }
                    break;
                }
            }
        }
    }
    fn set_leader(&mut self, index: i32) {
        if self.g.mem.combat() == 0 {
            return;
        }
        let old = Mem::walker(self.g.mem.leader());
        if self.g.mem.orders() != 0 && self.g.mem.individual() == 0 {
            self.ww(old, 16, self.g.mem.x() as i32);
            self.ww(old, 18, self.g.mem.y() as i32)
        }
        self.ww(old, 20, 0);
        let new = Mem::walker(index as i16);
        self.wb(new, 3, 0);
        self.wb(new, 22, 0);
        self.g.mem.set_individual(0);
        self.ww(new, 8, self.w(new, 8) & 0xfffe);
        self.ww(new, 10, self.w(new, 10) & 0xfffe);
        self.g.mem.set_x(self.w(new, 8) as i16);
        self.g.mem.set_y(self.w(new, 10) as i16);
        self.g.mem.set_heading(self.b(new, 1) as i16);
        self.g.mem.set_leader(index as i16)
    }
    fn scroll_view_to_player(&mut self, show: i32) {
        if self.g.mem.combat() == 0 {
            let dx = i16(self.g.mem.x() as i32 - self.g.mem.view_x() as i32) >> 1;
            if dx == 0 {
                self.g.mem.set_view_x(self.g.mem.view_x().wrapping_sub(1))
            } else if dx == 6 {
                self.g.mem.set_view_x(self.g.mem.view_x().wrapping_add(1))
            }
            let dy = i16(self.g.mem.y() as i32 - self.g.mem.view_y() as i32) >> 1;
            if dy == 0 {
                self.g.mem.set_view_y(self.g.mem.view_y().wrapping_sub(1))
            } else if dy == 11 {
                self.g.mem.set_view_y(self.g.mem.view_y().wrapping_add(1))
            }
        } else if self.g.mem.individual() != 0 {
            self.g.mem.set_individual(1);
            let t = self.call("get_ticks", &[]);
            self.g.mem.set_target_deadline(t.wrapping_add(9))
        }
        self.call("mark_time", &[]);
        self.call("redraw", &[]);
        if self.g.mem.encounter_mode() == 0 && show != 0 && self.g.mem.individual() == 0 {
            self.call("sound", &[0]);
            let n = self.call("sound_busy", &[]) as i16 as i32;
            self.call("add_ticks", &[n / 10]);
        }
        self.call("wait_move_delay", &[]);
    }
    fn party_order(&mut self, redraw: i32) {
        if self.g.mem.combat() != 0 {
            if self.g.mem.orders() != 0 && self.g.mem.individual() == 0 {
                let w = Mem::walker(self.g.mem.leader());
                if self.w(w, 8) != self.g.mem.x() as i32 || self.w(w, 10) != self.g.mem.y() as i32 {
                    self.ww(w, 16, self.g.mem.x() as i32 & 0xfffe);
                    self.ww(w, 18, self.g.mem.y() as i32 & 0xfffe);
                    self.g.mem.set_x((self.w(w, 8) & 0xfffe) as i16);
                    self.g.mem.set_y((self.w(w, 10) & 0xfffe) as i16);
                    self.ww(w, 20, 0)
                }
            }
            self.g.mem.set_orders(self.g.mem.orders() ^ 1);
            self.call("show_message", &[if self.g.mem.orders() != 0 { 0xac8 } else { 0xb00 }, Mem::DS as i32]);
            return;
        }
        let mut a = self.call("weapon_menu", &[0x2af, Mem::DS as i32, 5, 0, 6, 8, 0xb11, Mem::DS as i32]) as i16 as i32;
        if self.party((self.party(a + 1) << 8) + 0x127) == 0 {
            a = -1
        }
        let mut b = self.initial_second;
        if a != -1 {
            b = self.call("weapon_menu", &[0x2af, Mem::DS as i32, 5, (a == 0) as i32, 4, 8, 0xb11, Mem::DS as i32]) as i16 as i32
        }
        if self.party((self.party(b + 1) << 8) + 0x127) == 0 {
            b = -1
        }
        if a != -1 && b != -1 && a != b {
            let old = self.party(a + 1);
            self.sparty(a + 1, self.party(b + 1));
            self.sparty(b + 1, old);
            self.call("check_deaths", &[]);
            if redraw != 0 && self.g.mem.combat() == 0 {
                self.call("redraw_party", &[]);
            }
            self.call("status_panel", &[]);
        }
    }
}
impl Context<'_> {
    fn try_move(&mut self, mut x: i32, mut y: i32) -> i32 {
        let cell = self.cell(x, y);
        if self.b(cell, 2) & 64 != 0 {
            if self.g.mem.combat() != 0 {
                return 0;
            }
            self.g.mem.set_pending_event(1);
            return 1;
        }
        let lit = self.b(cell, 0) & 1;
        let mut newtile = 0;
        if self.g.mem.combat() != 0 {
            let (x2, y2) = (i16(x * 2), i16(y * 2));
            if x2 < self.g.mem.view_x() as i32
                || y2 < self.g.mem.view_y() as i32
                || i16(self.g.mem.view_x() as i32 + 12) < x2
                || i16(self.g.mem.view_y() as i32 + 22) < y2
            {
                return 0;
            }
            if self.g.mem.individual() != 0 {
                self.g.mem.set_move_delay(1);
                return 1;
            }
            if lit == 0 && (self.b(cell, 1) <= 14 || matches!(self.b(cell, 1), 38 | 39)) {
                return 0;
            }
            if lit != 0 && self.b(cell, 1) <= 4 {
                return 0;
            }
            for i in 0..5 {
                let w = Mem::walker(i);
                if i != self.g.mem.leader() && self.b(w, 0) != 0 && self.w(w, 8) == x2 && self.w(w, 10) == y2 {
                    return 0;
                }
            }
            x = x2 >> 1;
            y = y2 >> 1;
        }
        if self.g.mem.encounter_mode() == 0 {
            if lit != 0 {
                if self.b(cell, 1) < 5 {
                    self.g.mem.set_event(self.b(cell, 1) as i8)
                } else if self.b(cell, 1) > 235 && self.g.mem.interior_kind() == b'I' {
                    let key = self.call("door_key", &[y]);
                    if key != 0 {
                        let scratch = Far::new(0xfec0, 0x9000);
                        self.call("strcpy", &[scratch.off as i32, scratch.seg as i32, key as u16 as i32, (key >> 16) as i32]);
                        self.call("strcat", &[scratch.off as i32, scratch.seg as i32, 0xa36, Mem::DS as i32]);
                        newtile = self.b(cell, 1);
                        self.g.mem.set_pointer(0x6b97, cell);
                        self.wb(cell, 1, 34);
                        self.wb(cell, 0, self.b(cell, 0) & !1);
                        self.g.mem.set_event_id(-1);
                        if self.b(scratch, 0) != b'X' as i32 {
                            self.call("show_message", &[scratch.off as i32, scratch.seg as i32]);
                        }
                        self.call("sound", &[17]);
                    } else {
                        self.call("show_message", &[0xa3d, Mem::DS as i32]);
                    }
                }
            } else {
                let tile = self.b(cell, 1);
                if tile < 15 {
                    self.g.mem.set_event((tile + 5) as i8)
                } else if tile < 17 {
                    self.wb(cell, 1, tile + 20);
                    self.g.mem.set_event_id(-1);
                    self.g.mem.set_encounter(1)
                } else if tile == 17 && self.g.mem.countdown() == 100 {
                    self.g.mem.set_countdown(99);
                    self.call("show_message", &[0xa52, Mem::DS as i32]);
                    self.call("self_destruct", &[]);
                } else if tile == 38 {
                    if self.call("police_check", &[]) == 0 {
                        return 0;
                    }
                } else if tile == 39 {
                    self.call("police_encounter", &[]);
                } else if tile == 19 {
                    if y & 1 != 0 {
                        let p = Mem::npc(self.g.mem.pursuer_index() as i16 + 1);
                        self.g.mem.set_arik(0);
                        if self.g.mem.pursuer_active() != 0 && self.b(p, 2) == self.g.mem.level() as u8 as i32 {
                            self.call("npc_dies", &[self.w(p, 32) >> 1, self.w(p, 34) >> 1]);
                            self.call("set_cell_flags", &[self.w(p, 32), self.w(p, 34), 0]);
                        }
                    } else if self.g.mem.arik() == 0 {
                        self.g.mem.set_arik(300)
                    }
                }
            }
        } else if self.g.mem.encounter_mode() == 1 && lit == 0 && (15..17).contains(&self.b(cell, 1)) {
            self.wb(cell, 1, self.b(cell, 1) + 20);
            self.g.mem.set_event_id(-1);
            self.g.mem.set_encounter(1)
        }
        let terrain_class = self.b(cell, 1) / 20;
        let mut foot = 0;
        let allowed;
        if self.b(cell, 2) & 63 != 0 {
            foot = 1;
            let mut a = 0;
            if self.g.mem.encounter_mode() == 2 {
                a = 1
            } else if self.g.mem.encounter_mode() == 1 {
                a = 1;
                if self.call("hit_npc", &[x, y, 1, 5]) != 0 {
                    self.call("npc_dies", &[x, y]);
                    self.call("set_cell_flags", &[x * 2, y * 2, 128]);
                    self.g.mem.set_event_id(-1);
                    self.call("sound", &[4]);
                    self.sparty(0x81, 1);
                } else {
                    self.g.mem.set_move_back(1)
                }
                self.g.mem.set_party_fired(1);
            } else if self.g.scene.is_some() && !super::round::in_encounter(self.g) && super::round::neutral(self.g, (self.b(cell, 2) & 63) as u8) {
                self.g.blocked_by = Some((self.b(cell, 2) & 63) as u8);
            }
            allowed = a;
        } else {
            allowed = match self.g.mem.encounter_mode() {
                0 => (self.call("foot_terrain", &[cell.off as i32, cell.seg as i32, terrain_class]) & 255) as i32,
                1 => {
                    let a = (self.call("vehicle_terrain", &[cell.off as i32, cell.seg as i32, terrain_class]) & 255) as i32;
                    foot = (self.call("foot_terrain", &[cell.off as i32, cell.seg as i32, terrain_class]) & 255) as i32;
                    a
                }
                2 => {
                    if lit != 0 {
                        (!matches!(terrain_class, 5 | 6)) as i32
                    } else {
                        (terrain_class != 2 && !matches!(self.b(cell, 1), 38 | 39)) as i32
                    }
                }
                3 => (lit != 0 && terrain_class == 11) as i32,
                _ => 0,
            };
        }
        if allowed != 0 {
            let rec = ((self.g.mem.selected_char() as i32) << 8) + 0x102;
            let condition = (100 - self.g.mem.vehicle_condition() as i32) / 20;
            let delay = match self.g.mem.encounter_mode() {
                2 => 4 - self.party(rec + 0x43),
                3 => 6 - self.party(rec + 0x62),
                1 => ((foot ^ 1) << 1) + 5 - self.party(rec + 0x2e) + condition,
                _ => {
                    if self.g.mem.level() != 0 || lit == 0 {
                        3
                    } else {
                        self.g.mem.u8((0x28d + terrain_class) as u16) as i32
                    }
                }
            };
            self.g.mem.set_move_delay(delay as i16);
            if self.g.mem.encounter_mode() != 0 {
                self.g.mem.set_move_delay((self.g.mem.move_delay() as i32 + condition) as i16)
            }
            if self.g.mem.combat() != 0 && self.party(((self.g.mem.leader() as i32) << 8) + 0x12a) & 4 != 0 {
                self.g.mem.set_move_delay(self.g.mem.move_delay().wrapping_sub(2));
                if self.call("random", &[20]) == 10 {
                    self.call("damage", &[self.g.mem.leader() as i32, 100, 1]);
                    self.call("status_panel", &[]);
                }
            }
            if self.g.mem.move_delay() <= 0 {
                self.g.mem.set_move_delay(1)
            }
            if self.g.mem.event_value() != 0 {
                self.g.mem.set_update_event(1)
            }
            self.g.mem.set_pointer(0x6b64, cell);
        }
        if newtile != 0 {
            self.g.mem.set_event_value(newtile as u8)
        }
        allowed
    }
    fn vehicle_terrain(&self, p: Far, class: i32) -> i32 {
        let tile = self.b(p, 1);
        if self.b(p, 0) & 1 != 0 {
            (tile > 4 && (class < 5 || (7..=8).contains(&class))) as i32
        } else {
            (tile > 14 && class < 2 && !matches!(tile, 38 | 39 | 17)) as i32
        }
    }
    fn foot_terrain(&self, p: Far, class: i32) -> i32 {
        ((self.b(p, 0) & 1 != 0 && class < 5) || class < 2) as i32
    }
    fn set_cell_flags(&mut self, x: i32, y: i32, value: i32) {
        if y >> 1 < self.g.mem.map_stride() as i32 && x >> 1 < (self.g.mem.map_blocks_high() as i32) * 12 {
            let p = self.cell(x >> 1, y >> 1).add(2);
            self.wb(p, 0, (self.b(p, 0) & 0xc0) | value)
        }
    }
    fn release_cell(&mut self, x: i32, y: i32) {
        let p = self.cell(x >> 1, y >> 1).add(2);
        self.wb(p, 0, self.b(p, 0) & 0xbf)
    }
}
fn routine_name(name: &str) -> Option<&'static str> {
    Some(match name {
        "can_enter" | "try_move" => "try_move",
        "update_position" | "scroll_view_to_player" => "scroll_view_to_player",
        "select_leader" | "set_leader" => "set_leader",
        "walker_set_pose" => "walker_set_pose",
        "try_reload" => "try_reload",
        "npc_can_step" => "npc_can_step",
        "move_party" => "move_party",
        "npc_tick" => "npc_tick",
        "party_order" => "party_order",
        "vehicle_terrain" => "vehicle_terrain",
        "foot_terrain" => "foot_terrain",
        "set_cell_flags" => "set_cell_flags",
        "release_cell" => "release_cell",
        "atmosphere_hostile" => "atmosphere_hostile",
        "atmosphere_tainted" => "atmosphere_tainted",
        "atmosphere_tick" => "atmosphere_tick",
        "air_tick" => "air_tick",
        "self_destruct_tick" => "self_destruct_tick",
        "alarm_tick" => "alarm_tick",
        "check_party_alive" => "check_party_alive",
        "resolve_event" => "resolve_event",
        "sync_leader_pos" => "sync_leader_pos",
        "play_pending_sounds" => "play_pending_sounds",
        _ => return None,
    })
}
fn is_void(n: &str) -> bool {
    !matches!(
        n,
        "try_reload"
            | "npc_can_step"
            | "move_party"
            | "npc_tick"
            | "try_move"
            | "vehicle_terrain"
            | "foot_terrain"
            | "atmosphere_hostile"
            | "atmosphere_tainted"
    )
}
pub fn replay(g: &mut Ground, name: &str, args: &[u16], initial_second: i16, services: &mut dyn Services) -> u32 {
    let Some(name) = routine_name(name) else { return 0 };
    Context { g, services, fuel: 65536, initial_second: initial_second as i32 }.invoke(name, args)
}
fn live(g: &mut Ground, name: &str, args: &[u16]) -> i16 {
    replay(g, name, args, 0, &mut Live) as i16
}
macro_rules! noargs {($($n:ident),*)=>{$(pub fn $n(g:&mut Ground) {live(g,stringify!($n),&[]);})*}}
noargs!(atmosphere_tick, air_tick, self_destruct_tick, alarm_tick, check_party_alive, resolve_event, sync_leader_pos, play_pending_sounds);
pub fn atmosphere_hostile(g: &mut Ground) -> i16 {
    live(g, "atmosphere_hostile", &[])
}
pub fn move_party(g: &mut Ground, dx: i16, dy: i16, d: i16) -> i16 {
    live(g, "move_party", &[dx as u16, dy as u16, d as u16])
}
pub fn npc_tick(g: &mut Ground, redraw: i16) -> i16 {
    live(g, "npc_tick", &[redraw as u16])
}
pub fn set_leader(g: &mut Ground, index: i16) {
    live(g, "set_leader", &[index as u16]);
}
pub fn party_order(g: &mut Ground, redraw: i16) {
    live(g, "party_order", &[redraw as u16]);
}
pub fn try_reload(g: &mut Ground, index: i16) -> i16 {
    live(g, "try_reload", &[index as u16])
}
pub fn walker_set_pose(g: &mut Ground, w: Far) {
    live(g, "walker_set_pose", &[w.off, w.seg]);
}
pub fn npc_can_step(g: &mut Ground, x: i16, y: i16, p: Far) -> i16 {
    live(g, "npc_can_step", &[x as u16, y as u16, p.off, p.seg])
}
pub fn try_move(g: &mut Ground, x: i16, y: i16) -> i16 {
    live(g, "try_move", &[x as u16, y as u16])
}
pub fn vehicle_terrain(g: &mut Ground, p: Far, class: i16) -> i16 {
    live(g, "vehicle_terrain", &[p.off, p.seg, class as u16])
}
pub fn foot_terrain(g: &mut Ground, p: Far, class: i16) -> i16 {
    live(g, "foot_terrain", &[p.off, p.seg, class as u16])
}
pub fn set_cell_flags(g: &mut Ground, x: i16, y: i16, value: i16) {
    live(g, "set_cell_flags", &[x as u16, y as u16, value as u16]);
}
pub fn release_cell(g: &mut Ground, x: i16, y: i16) {
    live(g, "release_cell", &[x as u16, y as u16]);
}
pub fn claim_cell(g: &mut Ground, x: i16, y: i16) -> i16 {
    let (row, col) = ((x >> 1) as u8, (y >> 1) as u8);
    g.mem.set_u8(0x6419, row);
    g.mem.set_u8(0x63F4, col);
    let cell = super::commands::cell(g, row as i16, col as i16).add(2);
    let flags = g.mem.read(cell, 1) as u8;
    if flags & 0x3F != 0 { return 0 }
    g.mem.write(cell, (flags | 0x40) as u32, 1);
    1
}
impl Context<'_> {
    fn leaf(&mut self, name: &str, a: &[u16]) -> u32 {
        let v = |i: usize| a.get(i).copied().unwrap_or(0);
        let s = |i: usize| v(i) as i16 as i32;
        let p = |i: usize| Far::new(v(i), v(i + 1));
        match name {
            "random" => random(self.g, s(0) as i16) as u16 as u32,
            "long_divide" => {
                let n = (v(0) as u32 | ((v(1) as u32) << 16)) as i32;
                let d = (v(2) as u32 | ((v(3) as u32) << 16)) as i32;
                n.checked_div(d).unwrap_or(0) as u32
            }
            "get_ticks" => self.g.now(),
            "redraw" => {
                render::redraw(self.g);
                0
            }
            "show_message" => {
                render::show_message(self.g, p(0));
                0
            }
            "dismiss_message" => {
                render::dismiss_message(self.g, s(0) as i16);
                0
            }
            "memmove" => {
                let b = self.g.mem.bytes(p(2), v(4) as usize);
                self.g.mem.write_bytes(p(0), &b);
                packed(p(0))
            }
            "strcpy" | "strcat" => {
                let dst = p(0);
                let src = p(2);
                let mut b = Vec::new();
                for i in 0..1024 {
                    let n = self.b(src, i) as u8;
                    b.push(n);
                    if n == 0 {
                        break;
                    }
                }
                let mut off = 0;
                if name == "strcat" {
                    while off < 1024 && self.b(dst, off) != 0 {
                        off += 1
                    }
                }
                if dst.seg == Mem::PARTY_SEG && dst.off >= 0xf000 {
                    if let Some(b) = self.g.mem.segs.get_mut(&dst.seg) {
                        b.resize(0x10000, 0)
                    }
                }
                self.g.mem.write_bytes(dst.add(off), &b);
                packed(dst)
            }
            "item_ptr" => packed(self.item_ptr(s(0), s(1))),
            "item_def" => packed(self.item_def(s(0), s(1))),
            "door_key" => packed(self.door_key(s(0))),
            "party_fire" => super::combat::party_fire(self.g, s(0)) as u16 as u32,
            "monster_attack" => super::combat::monster_attack(self.g, p(0), s(2)) as u16 as u32,
            "hit_npc" => super::combat::hit_npc(self.g, s(0), s(1), s(2), s(3)) as u16 as u32,
            "npc_dies" => { super::combat::npc_dies(self.g, s(0), s(1)); 0 }
            "char_dies" => { super::combat::char_dies(self.g, s(0)); 0 }
            "damage" => super::combat::take_hit(self.g, s(0), s(1), s(2)) as u16 as u32,
            "tick_party" => { super::combat::tick_party(self.g, s(0), s(1)); 0 }
            "event_at" => {
                if !event_at(self.g, v(0), v(1), s(2) as i16) {
                    world_load_failed(self.g);
                }
                0
            }
            "world_event" => {
                world_event(self.g);
                0
            }
            "enter_world" => {
                enter_world(self.g, v(0), v(1), v(2), v(3));
                0
            }
            "both_hands_air" => {
                let d1 = self.item_def(s(0), 20);
                let d2 = self.item_def(s(0), 19);
                (packed(d1) != 0 && packed(d2) != 0 && self.b(d1, 0x29) != 0 && self.b(d2, 0x29) != 0) as u32
            }
            "use_air" => {
                let off = (s(0) << 8) + 0x128;
                self.sparty(off, self.party(off) - 1);
                if self.party(off) == 0 {
                    self.call("char_dies", &[s(0)]);
                    2
                } else {
                    1
                }
            }
            "weapon_menu" => super::input::menu::weapon_menu(self.g, p(0), v(2) as usize, s(3) as i16, v(4) as u8, v(5) as u8, Some(p(6))) as u16 as u32,
            "sound" => {play_sound(self.g,v(0));0}
            "sound_busy" => sound_busy(self.g) as u32,
            "add_ticks" => {let now=self.g.now();if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("add_ticks({}) now={now} ticks={}", s(0), self.g.clock.ticks()) }self.g.set_clock(now.wrapping_add(s(0) as u32));0}
            "beep" => 0,
            "mark_time" => {
                let t = self.g.now();
                self.g.mem.set_u32(0x4bde, t);
                0
            }
            "wait_ticks" => {
                wait_ticks(self.g, s(0) as i16);
                0
            }
            "wait_move_delay" => {
                wait_ticks(self.g, self.g.mem.move_delay());
                0
            }
            "invalidate_rect" => {
                for (off, value) in [(0x717c, v(0)), (0x7164, v(1)), (0x7170, v(2)), (0x7166, v(3))] {
                    self.g.mem.set_u16(off, value)
                }
                0
            }
            "walker_panel" => { render::walker_panel(self.g, v(0) != 0, v(1) != 0); 0 }
            "air_panel" => { render::air_panel(self.g, v(0) != 0); 0 }
            "status_panel" => { render::status_panel(self.g); 0 }
            "redraw_party" => { render::redraw_party(self.g); 0 }
            "select_panel" => { render::select_panel(self.g, v(0), v(1) as u8); 0 }
            "shake_screen" => { shake_screen(self.g, s(0), s(1)); 0 }
            "police_check" => super::starport::police::weapons_search(self.g) as u32,
            "police_encounter" => {
                super::starport::police::encounter(self.g);
                0
            }
            "self_destruct" => 0,
            "game_over" => {
                game_over(self.g, p(0));
            }
            "check_deaths" => {
                check_deaths(self.g);
                0
            }
            _ => {
                eprintln!("ground: unavailable walk service {name}");
                0
            }
        }
    }
    fn item_ptr(&self, who: i32, slot: i32) -> Far { item_ptr(self.g, who, slot) }
    fn item_def(&self, who: i32, slot: i32) -> Far { item_def(self.g, who, slot) }
    fn door_key(&self, lock: i32) -> Far {
        if self.g.mem.cheat() != 0 {
            return ds_string(self.g, b"Master key");
        }
        if self.g.mem.u8(0x5e1d) as u32 & 1u32.wrapping_shl(self.g.mem.level() as u32) == 0 {
            return ds_string(self.g, b"X");
        }
        let lo = if self.g.mem.combat() == 0 { 0 } else { self.g.mem.leader() as i32 };
        let hi = if self.g.mem.combat() == 0 { 5 } else { lo + 1 };
        for who in lo..hi {
            if self.party((who << 8) + 0x127) != 0 {
                for slot in 21..23 {
                    let it = self.item_def(who, slot);
                    if packed(it) != 0
                        && self.b(it, 0x27) == self.party(6)
                        && self.b(it, 0x28) == self.party(7)
                        && self.b(it, 0x29) == self.g.mem.level() as i32
                        && self.b(it, 0x2b) == lock & 3
                    {
                        return it;
                    }
                }
            }
        }
        Far::default()
    }
}
pub fn item_ptr(g: &Ground, who: i32, slot: i32) -> Far {
    let party = g.mem.pointer(0x6d65);
    let index = g.mem.read_i(party.add((who << 8) + 2 * slot + 0x1ce), 2);
    if index <= 0 { Far::default() } else { party.add(0xa82 + 5 * (index - 1)) }
}
pub fn item_def(g: &Ground, who: i32, slot: i32) -> Far {
    let obj = item_ptr(g, who, slot);
    if packed(obj) == 0 { Far::default() } else { g.mem.pointer(0x6d6d).add(46 * g.mem.read(obj, 1) as i32) }
}
pub fn new_object(g: &mut Ground, id: i32) -> i32 {
    Context { g, services: &mut Live, fuel: 65536, initial_second: 0 }.new_object(id)
}
pub fn place_object(g: &mut Ground, x: i32, y: i32, item: i32) -> i32 {
    Context { g, services: &mut Live, fuel: 65536, initial_second: 0 }.place_object(x, y, item)
}
pub fn party_fire(g: &mut Ground, who: i32) -> i16 { super::combat::party_fire(g, who) as i16 }
pub(crate) fn ds_string(g: &Ground, text: &[u8]) -> Far {
    g.mem
        .ds
        .windows(text.len() + 1)
        .position(|w| w[..text.len()] == *text && w[text.len()] == 0)
        .map(|off| Mem::ds(off as u16))
        .unwrap_or_default()
}
const MAX_NPCS: usize = (0x6b52 - 0x6422) / 46;
const MAX_SPAWNS: usize = (0x63f4 - 0x6340) / 6;
const MAX_MAP_BYTES: usize = 65536;
const MAX_WORLD_MAP_BYTES: usize = 8 * 1024 * 1024;
const WORLD_BASES: [usize; 9] = [0, 4, 7, 13, 16, 18, 21, 24, 28];
fn area_index(world: u16, area: u16) -> Option<usize> {
    let base = *WORLD_BASES.get(world as usize)?;
    let end = *WORLD_BASES.get(world as usize + 1)?;
    ((area as usize) < end - base).then_some(base + area as usize)
}
fn map_size(rows: u8, cols: u8) -> Option<usize> {
    let size = rows as usize * cols as usize * 720;
    (rows != 0 && cols != 0 && size <= MAX_MAP_BYTES).then_some(size)
}
fn world_data(bytes: &[u8]) -> Option<Vec<mt_engine::data::world::Chunk>> {
    use mt_engine::data::{
        bin::{Rd, Record},
        container, world as data, Variant,
    };
    let mut count = Rd::new(bytes);
    if count.u32().ok()? > 31 {
        return None;
    }
    let chunks = container::decode(bytes).ok()?;
    let mut remaining = MAX_WORLD_MAP_BYTES;
    for chunk in chunks {
        let mut r = Rd::new(chunk);
        let inside = chunk.starts_with(b"\x01\x01INSIDE");
        let end = if inside {
            chunk.len()
        } else {
            r.take(data::AreaHdr::SIZE).ok()?;
            r.take(10).ok()?;
            let end = r.u16().ok()? as usize;
            if end < r.p || end > chunk.len() {
                return None;
            }
            end
        };
        while r.p < end {
            let hdr = data::MapHdr::take(&mut r).ok()?;
            let size = map_size(hdr.rows, hdr.cols)?;
            remaining = remaining.checked_sub(size)?;
            let mut cells = size / 3;
            while cells != 0 {
                let n = r.i16().ok()? as i32;
                let len = n.unsigned_abs() as usize;
                cells = cells.checked_sub(len)?;
                r.take(if n < 0 { 2 } else { len * 2 }).ok()?;
            }
            if inside {
                break;
            }
        }
        if r.p != end {
            return None;
        }
        if !inside {
            let n = r.u8().ok()? as usize;
            if n > MAX_NPCS {
                return None;
            }
            r.take(n * data::WObj::SIZE).ok()?;
            let n = r.u8().ok()? as usize;
            if n > MAX_SPAWNS {
                return None;
            }
            r.take(n * data::Spawn::SIZE).ok()?;
        }
        r.done().ok()?;
    }
    data::decode(Variant::Mt1, bytes).ok()
}
pub(crate) fn game_over(g: &mut Ground, why: Far) -> ! {
    if g.scene.is_some() && !g.harness && g.lockstep.is_none() {
        let why = render::paint::Paint::new(g, mt_engine::present::prim::Raster::Planar).string(why);
        let text = format!("{} You have failed.", String::from_utf8_lossy(&why).trim());
        let whole = format!("{text} Press a key or click."); super::orders::log(g, whole.clone());
        if let Some(s) = g.squad.as_mut() { s.p1.strip = whole; s.orders.hover.clear(); s.p1.acting = None }
        render::redraw(g); render::message(g, text.as_bytes());
        g.get_key();
        g.game_over=true;
        std::panic::resume_unwind(Box::new(super::main_loop::GameOver));
    }
    render::show_message(g, why);
    g.mem.set_u32(0x4bde, u32::MAX);
    wait_ticks(g, 50);
    let failed = ds_string(g, b"You have failed.");
    render::show_message(g, failed);
    g.mem.set_u32(0x4bde, u32::MAX);
    wait_ticks(g, 50);
    g.dump_step();
    std::process::exit(1)
}
fn world_load_failed(g: &mut Ground) -> ! {
    eprintln!("ground: unable to load W.DAT world/level");
    g.dump_step();
    std::process::exit(3)
}
pub fn init_world(g: &mut Ground, world: u16, area: u16) -> bool {
    g.observe_boot("init_world", &[world as i16, area as i16]);
    if let Some(s) = g.squad.as_mut() { s.reset_level() }
    g.lockstep_checkpoint("init_world");
    use mt_engine::data::{bin::Record, world as data};
    let Some(index) = area_index(world, area) else { return false };
    let Some(bytes) = g.files().read("W.DAT") else { return false };
    let Some(chunks) = world_data(&bytes) else { return false };
    let Some(data::Chunk::Area(a)) = chunks.get(index) else { return false };
    g.mem.write_bytes(Mem::ds(0x5e06), &a.hdr.to_bytes());
    for (i, &off) in a.level_offs.iter().enumerate() {
        g.mem.set_u16(0x6c2f + 2 * i as u16, off)
    }
    g.mem.set_pointer(0x5e02, Mem::ds(0x5e14));
    g.mem.set_u8(0xfa, a.hdr.start_x as u8);
    g.mem.set_u8(0xfb, a.hdr.start_y as u8);
    g.mem.set_x(a.hdr.start_x.wrapping_mul(2) as i16);
    g.mem.set_previous_x(g.mem.x());
    g.mem.set_y(a.hdr.start_y.wrapping_mul(2) as i16);
    g.mem.set_previous_y(g.mem.y());
    g.mem.set_party_u8(6, world as u8);
    g.mem.set_party_u8(7, area as u8);
    g.mem.set_u8(0x4ce5, 0);
    let at=4+index*4;
    let Some(offset)=bytes.get(at..at+4) else {return false};
    g.mem.set_u32(0x6B52,u32::from_le_bytes(offset.try_into().unwrap()));
    for (i,&id) in a.hdr.pic_ids.iter().enumerate() {
        if id==10 {break}
        if !render::cpic_load(g,id,i as u8+3) {return false}
    }
    g.mem.set_npc_count(a.objects.len() as u8);
    for (i, obj) in a.objects.iter().enumerate() {
        let p = Mem::npc(i as i16 + 1);
        g.mem.write_bytes(p, &obj.to_bytes());
        if g.mem.party_u8((obj.id >> 3).wrapping_add(8)) & (1u8 << (obj.id & 7)) != 0 {
            g.mem.write(p.add(2), 20, 1);
            continue;
        }
        g.mem.write(p.add(3), obj.x.wrapping_mul(2) as u32, 2);
        g.mem.write(p.add(5), obj.y.wrapping_mul(2) as u32, 2);
        g.mem.write(p.add(32), 0, 2);
        g.mem.write(p.add(34), 0, 2);
        let facing = random(g, 8);
        g.mem.write(p.add(36), facing as u32, 1);
        let steps = random(g, 5) + 5;
        g.mem.write(p.add(37), steps as u32, 1);
        g.mem.write(p.add(39), 1, 1);
        g.mem.write(p.add(40), 1, 1);
        g.mem.write(p.add(42), 0, 2);
        g.mem.write(p.add(44), 0, 2);
        g.mem.write(p.add(41), obj.fb.wrapping_mul(10) as u32, 1);
        g.mem.write(p.add(8), obj.f8.wrapping_add(2) as u32, 1);
        g.mem.write(p.add(38), 0, 1);
        if obj.r#type == 4 {
            g.mem.write(p.add(40), 255, 1)
        }
        if g.mem.party_u8((obj.id >> 3).wrapping_add(0x3a)) & (1u8 << (obj.id & 7)) != 0 {
            g.mem.write(p.add(26), 0x1d2, 2)
        }
    }
    g.mem.set_u8(0x6b69, a.spawns.len() as u8);
    for (i, s) in a.spawns.iter().enumerate() {
        g.mem.write_bytes(Mem::ds(0x6340u16.wrapping_add(6 * i as u16)), &s.to_bytes())
    }
    g.mem.set_air_warning(0);
    let n = random(g, 25) + 5;
    g.mem.set_u8(0x6d12, n as u8);
    g.mem.set_u32(0x6d13, ((15 - a.hdr.uwp[0] as i32) * 5000 + 50000) as u32);
    for level in 0..8 {
        let old = g.mem.pointer(0x606e + 4 * level);
        if old.off == 0 && (0x5000..0x8000).contains(&old.seg) && old.seg != g.mem.pointer(0x6d65).seg {
            g.mem.segs.remove(&old.seg);
        }
        g.mem.set_pointer(0x606e + 4 * level, Far::default())
    }
    true
}
pub fn set_player_pos(g: &mut Ground, x: i16, y: i16) {
    g.observe_boot("set_position", &[x, y]);
    g.mem.set_x(x);
    g.mem.set_y(y);
    g.mem.set_view_x(x.wrapping_sub(3));
    g.mem.set_view_y(y.wrapping_sub(6));
    if g.mem.view_x() <= 0 {
        g.mem.set_view_x(0)
    }
    if g.mem.view_y() <= 0 {
        g.mem.set_view_y(0)
    }
    if g.mem.map_blocks_high() as i16 * 12 <= g.mem.view_x().wrapping_add(7) {
        g.mem.set_view_x((g.mem.map_blocks_high() as i32 * 12 - 7) as i16)
    }
    if g.mem.view_y().wrapping_add(12) >= g.mem.map_stride() {
        g.mem.set_view_y(g.mem.map_stride().wrapping_sub(12))
    }
    g.mem.set_x(g.mem.x().wrapping_mul(2));
    g.mem.set_previous_x(g.mem.x());
    g.mem.set_y(g.mem.y().wrapping_mul(2));
    g.mem.set_previous_y(g.mem.y());
    g.mem.set_view_x(g.mem.view_x().wrapping_mul(2));
    g.mem.set_view_y(g.mem.view_y().wrapping_mul(2));
}
pub fn event_at(g: &mut Ground, world: u16, area: u16, level: i16) -> bool {
    g.observe_boot("enter_area", &[world as i16, area as i16, level]);
    use mt_engine::data::{bin::Record, world as data};
    if !(0..8).contains(&level)
        || !(-1..8).contains(&g.mem.level())
        || g.mem.npc_count() as usize > MAX_NPCS
        || g.mem.u8(0x6b69) as usize > MAX_SPAWNS
    {
        return false;
    }
    let target_world = if world == 9 { g.mem.party_u8(6) as u16 } else { world };
    let target_area = if area == 9 { g.mem.party_u8(7) as u16 } else { area };
    let Some(index) = area_index(target_world, target_area) else { return false };
    let cache = 0x606e + 4 * level as u16;
    let mut ptr = g.mem.pointer(cache);
    let loaded = if packed(ptr) == 0 {
        let Some(bytes) = g.files().read("W.DAT") else { return false };
        let Some(chunks) = world_data(&bytes) else { return false };
        let map = if world == 9 {
            match chunks.get((level + 23) as usize) {
                Some(data::Chunk::Inside(m)) => m,
                _ => return false,
            }
        } else {
            let Some(data::Chunk::Area(a)) = chunks.get(index) else { return false };
            let Some(off) = a.level_offs.get(level as usize) else { return false };
            let Some(index) = a.map_offs.iter().position(|&v| v == *off as usize) else { return false };
            let Some(m) = a.maps.get(index) else { return false };
            m
        };
        let Some(size) = map_size(map.hdr.rows, map.hdr.cols) else { return false };
        let Ok(raw_chunks)=mt_engine::data::container::decode(&bytes) else {return false};
        let raw=if world==9 {raw_chunks.get((level+23) as usize).copied()}
            else {raw_chunks.get(index).and_then(|b|b.get(g.mem.u16(0x6C2F+2*level as u16) as usize..))};
        let Some(raw)=raw.and_then(|b|b.get(26..)) else {return false};
        let mut r=mt_engine::data::bin::Rd::new(raw);let mut left=size/3;
        let (mut mode,mut word)=(0,g.mem.u16(0x6FC2));
        while left!=0 {
            let Ok(n)=r.i16() else {return false};let count=(n as i32).unsigned_abs() as usize;
            let Some(rest)=left.checked_sub(count) else {return false};left=rest;
            if n<0 {mode=1;let Ok(value)=r.u16() else {return false};word=value}
            else {mode=2;if r.take(count*2).is_err() {return false}}
        }
        let mut start = 0x50000usize;
        for (&seg, bytes) in &g.mem.segs {
            let base = seg as usize * 16;
            let end = base + bytes.len();
            if end <= start {
                continue;
            }
            if start + size <= base {
                break;
            }
            start = (end + 15) & !15;
        }
        if start + size > 0x80000 {
            return false;
        }
        ptr = Far::new(0, (start / 16) as u16);
        Some((map.clone(), size, mode, word))
    } else {
        let header = 0x6096 + 26 * level as u16;
        let Some(size) = map_size(g.mem.u8(header), g.mem.u8(header + 1)) else { return false };
        if ptr.off != 0
            || !(0x5000..0x8000).contains(&ptr.seg)
            || g.mem.segs.get(&ptr.seg).map(Vec::len) != Some(size)
            || target_world != g.mem.party_u8(6) as u16
            || target_area != g.mem.party_u8(7) as u16
        {
            return false;
        }
        None
    };
    if loaded.is_some() { g.lockstep_checkpoint("load_map") }
    if g.mem.level() != -1 {
        let off = 0xfa + g.mem.level() as u16 * 2;
        g.mem.set_u8(off, (g.mem.previous_x() >> 1) as u8);
        g.mem.set_u8(off + 1, (g.mem.previous_y() >> 1) as u8)
    }
    if world != 9 {
        g.mem.set_party_u8(6, world as u8)
    }
    if area != 9 {
        g.mem.set_party_u8(7, area as u8)
    }
    g.mem.set_level(level as i8);
    if let Some((map, size, mode, word)) = loaded {
        g.mem.set_u16(0x6FC0,mode);g.mem.set_u16(0x6FC2,word);g.mem.set_u16(0x6FC4,0);
        let cols = map.hdr.cols as usize;
        g.mem.alloc(ptr.seg, size);
        g.mem.set_pointer(0xbc, ptr);
        let header = map.hdr.to_bytes();
        g.mem.write_bytes(Mem::ds(0x5c3a), &header);
        g.mem.set_map_stride((cols * 20) as i16);
        for (k, [lo, hi]) in map.cells.iter().copied().enumerate() {
            let block = k / 240;
            let within = k % 240;
            let row = (block / cols) * 12 + within / 20;
            let col = (block % cols) * 20 + within % 20;
            g.mem.write_bytes(ptr.add(((row * cols * 20 + col) * 3) as i32), &[lo.wrapping_sub(1), hi, 0]);
        }
        let mut name = map.hdr.name.to_vec();
        let n = name.iter().position(|&b| b == 0).unwrap_or(name.len());
        name.truncate(n);
        name.extend_from_slice(b".PIC\0");
        render::select_tileset(g,&name);
        g.mem.set_pointer(cache, ptr);
        g.mem.write_bytes(Mem::ds(0x6096 + 26 * level as u16), &header);
        g.mem.write_bytes(Mem::ds(0x6fb3), &name);
        g.mem.write_bytes(Mem::ds(0x619a + 13 * level as u16), &name);
        for i in 0..g.mem.npc_count() {
            let p = Mem::npc(i as i16 + 1);
            if g.mem.read(p.add(2), 1) == level as u32 {
                let x = g.mem.read_i(p.add(3), 2) as i16;
                let y = g.mem.read_i(p.add(5), 2) as i16;
                g.mem.write(p.add(32), x as u16 as u32, 2);
                g.mem.write(p.add(34), y as u16 as u32, 2);
                set_cell_flags(g, x, y, i as i16 + 1)
            }
        }
        Context { g, services: &mut Live, fuel: 65536, initial_second: 0 }.spawn_area_objects();
    } else {
        g.mem.set_pointer(0xbc, ptr);
        let header = g.mem.bytes(Mem::ds(0x6096 + 26 * level as u16), 26);
        g.mem.write_bytes(Mem::ds(0x5c3a), &header);
        g.mem.set_map_stride(g.mem.u8(0x5c3b) as i16 * 20);
        let name = g.mem.bytes(Mem::ds(0x619a + 13 * level as u16), 13);
        render::select_tileset(g,&name);
    }
    if level != 0 && g.mem.u8(0x5f56 + 40 * (level as u16 - 1)) != 0 {
        let place = g.mem.cstr(Mem::ds(0x5f56 + 40 * (level as u16 - 1)));
        let text = format!("Welcome to {place}\0");
        g.mem.alloc(0x8800, 1024);
        g.mem.write_bytes(Far::new(0, 0x8800), text.as_bytes());
        render::show_message(g, Far::new(0, 0x8800))
    } else {
        render::dismiss_message(g, 1)
    }
    for i in 0..3 {render::select_panel(g,12+i,g.mem.u8(0x5C44+i))}
    g.mem.set_event_id(-1);
    g.mem.set_i16(0x6413, -1);
    set_player_pos(g, g.mem.u8(0xfa + 2 * level as u16) as i16, g.mem.u8(0xfb + 2 * level as u16) as i16);
    true
}
pub fn setup_world(g: &mut Ground) -> bool {
    let (world, area) = (g.mem.party_u8(6) as u16, g.mem.party_u8(7) as u16);
    g.mem.set_combat(0);
    g.mem.set_i16(0x6d2e, 0);
    if !init_world(g, world, area) {
        return false;
    }
    g.mem.set_level(-1);
    if !event_at(g, world, area, 0) {
        return false;
    }
    g.mem.set_outdoors(0);
    g.mem.set_provoked(0);
    g.mem.set_ship_count(0);
    g.mem.set_heading(4);
    g.mem.set_countdown(100);
    g.mem.set_encounter_mode(0);
    g.mem.set_u8(0x6d31, 0);
    g.mem.set_event_id(-1);
    g.mem.set_event(-1);
    let b = g.mem.bytes(Mem::ds(0x376), 14);
    g.mem.write_bytes(Mem::ds(0xfc), &b);
    true
}
pub fn enter_world(g: &mut Ground, world: u16, area: u16, arrival: u16, extra: u16) {
    if g.mem.world_pending() == 0 && arrival != 0 {
        g.mem.set_world_pending(1);
        return;
    }
    if world == 2 && arrival != 0 {
        ending::run(g, [area, arrival, extra]);
    }
    g.mem.set_world_pending(0);
    if !init_world(g, world, area) {
        world_load_failed(g);
    }
    g.mem.set_level(-1);
    if !event_at(g, world, area, 0) {
        world_load_failed(g);
    }
    render::main_screen(g, 0)
}
pub fn world_event(g: &mut Ground) {
    let level = g.mem.level();
    let off = 0xfau16.wrapping_add((level as u16).wrapping_mul(2));
    g.mem.set_u8(off, (g.mem.previous_x() >> 1) as u8);
    g.mem.set_u8(off.wrapping_add(1), (g.mem.previous_y() >> 1) as u8);
    match g.mem.event() {
        8 => { g.launch_pending = true; return; }
        9 | 10 => {
            g.mem.set_i16(0x7012,0);
            super::starport::enter(g);
            leave_starport(g);
        }
        11..=19 => {
            if g.mem.u8(0x5dfb + g.mem.event() as u16) == 0 {
                let p = ds_string(g, b"Nobody seems to be in.");
                if packed(p) != 0 {
                    render::show_message(g, p)
                }
            } else {
                super::starport::office(g);
            }
        }
        20 => super::starport::restore_stop(g),
        _ => {}
    }
    event_exit(g, off);
}
fn leave_starport(g: &mut Ground) {
    g.mem.set_event_id(-1);
    render::back(g);
    render::main_screen(g, 0);
    render::present(g,54,0,112,24);
    g.mem.set_i16(0x7012,1);
}
fn event_exit(g: &mut Ground, off: u16) {
    set_player_pos(g, g.mem.u8(off) as i16, g.mem.u8(off.wrapping_add(1)) as i16);
    let heading = g.mem.heading().wrapping_add(4);
    g.mem.set_heading(if heading > 7 { heading.wrapping_sub(8) } else { heading });
}
pub fn resume_starport(g: &mut Ground) {
    g.mem.set_i16(0x7012,0);
    super::starport::resume(g);
    leave_starport(g);
    event_exit(g, 0xfau16.wrapping_add((g.mem.level() as u16).wrapping_mul(2)));
    g.mem.set_event(-1);
}
pub fn wait_turn(g: &mut Ground) { let now = g.now(); g.mem.set_u32(0x4bde, now); wait_ticks(g, mt_engine::sim::sched::SECOND as i16) }
pub fn wait_ticks(g: &mut Ground, ticks: i16) {
    let raw = g.mem.u32(0x4bde);
    if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("wait_ticks({ticks}) mark={} now={} ticks={}", raw as i32, g.clock.now(), g.clock.ticks()) }
    let mark = raw;
    let mark = if mark == u32::MAX {
        let now = g.now();
        g.mem.set_u32(0x4bde, now);
        now
    } else {
        mark
    };
    let end = mark.wrapping_add(ticks as u32);
    for _ in 0..32768 {
        if g.now() >= end {
            break;
        }
        let before = g.clock.ticks();
        g.poll();
        if g.clock.ticks() == before && g.lockstep.is_none() {
            g.clock.wait_tick(); g.sync_bios_clock()
        }
    }
    if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("wait_ticks end={end} now={} ticks={}", g.clock.now(), g.clock.ticks()) }
    g.set_clock(end);
    g.mem.set_u32(0x4bde, u32::MAX);
}
pub fn covox_state(mem:&mut Mem,data:&[u8],rate:u16,device:u16,differential:bool,keyboard_pending:bool,speaker_refresh:&mut u8,ports:&mut Vec<(u16,u8)>)->u16 {
    let mut remaining=(data.len() as u32).wrapping_sub(1);
    mem.set_u32(0x55A0,remaining);
    if data.is_empty() || rate>255 {return 1}
    mem.set_u8(0x5588,u8::from(differential));
    mem.set_u16(0x5590,0);mem.set_u16(0x558E,0);mem.set_u16(0x559A,device);
    if device<4 {let p=mem.u16(0x5592+2*device);mem.set_u16(0x559A,p);ports.push((p.wrapping_sub(2),p as u8))}
    else if device<7 {
        let p=mem.read(Far::new(0x408+2*(device-4),0),2) as u16;mem.set_u16(0x559A,p);
        mem.set_u16(0x5590,1);ports.push((p.wrapping_add(2),4))
    } else if device==8 {mem.set_u16(0x558E,1);mem.set_u8(0x5588,1)}
    let byte=|i:usize|data.get(i).copied().unwrap_or(0);
    let header=(0..7).all(|i|byte(i)==mem.u8(0x536E+i as u16));
    let mut start=0;
    if header {
        mem.set_u8(0x558B,if rate!=0 {rate as u8} else if byte(9)!=0 {byte(9)} else {0x84});
        if byte(8)!=8 {return 1}
        mem.set_u32(0x55A0,remaining.wrapping_sub(16));
        if remaining<16 {return 1}
        remaining-=16;start=16;
    } else {mem.set_u8(0x558B,if rate!=0 {rate as u8} else {0x84})}
    *speaker_refresh^=0x10;mem.set_u8(0x5586,*speaker_refresh);
    *speaker_refresh^=0x10;
    ports.extend([(0x43,0x98),(0x61,*speaker_refresh|1),(0x42,0)]);
    let (speaker,dac)=(mem.u16(0x558E)!=0,mem.u16(0x559A));
    if keyboard_pending && remaining!=0 {mem.set_u32(0x55A0,remaining-1)}
    else {
        let mut previous=mem.u8(0x558A);
        for i in 0..remaining as usize {
            let mut sample=byte(start+i);
            ports.push((0x42,255));
            if mem.u8(0x5588)==1 {
                let value=previous.wrapping_sub(sample).wrapping_add(128);
                previous=sample;
                sample=if value<=64 {0} else if value>=192 {255} else {value.wrapping_mul(2)^128};
            }
            if speaker {
                if !(125..132).contains(&sample) {
                    *speaker_refresh^=0x10;
                    let v=if sample>=132 {*speaker_refresh&252} else {(*speaker_refresh|2)&254};
                    ports.extend([(0x61,v),(0x61,v|1)]);
                }
            } else {ports.push((dac,sample))}
        }
        if remaining!=0 && mem.u8(0x5588)==1 {
            mem.set_u8(0x5589,previous);mem.set_u8(0x558A,previous);
        }
        mem.set_u32(0x55A0,u32::MAX);
    }
    ports.extend([(0x61,mem.u8(0x5586)),(0x43,0xB6),(0x42,0xD0),(0x42,4)]);
    if mem.u16(0x5590)!=0 {ports.push((dac.wrapping_add(2),12))}
    0
}
pub fn play_sound(g:&mut Ground,n:u16) {
    if g.mem.u8(0x6B68)==0 || g.mem.sound_enabled()==0 {return}
    let len=if n<19 {g.mem.u32(0x6F00+4*n)} else {0};
    if n>=g.mem.u16(0x4A98) || len<16 || n>=19 || g.mem.pointer(0x6F4C+4*n)==Far::default() {
        g.mem.set_u16(0x4B16,0);return
    }
    let rate=g.mem.u16(0x4A9A+6*n+4) as u32;
    if rate==0 {g.mem.set_u16(0x4B16,0);return}
    g.mem.set_u16(0x4B16,((len.wrapping_mul(182).wrapping_add(rate-1))/rate) as u16);
    let data=g.mem.sound_data(n);
    let divisor=(256i32-1_193_180/rate as i32) as u16;
    let device=g.mem.u16(0x4B18);
    let mut ports=Vec::with_capacity(3*data.len()+8);
    covox_state(&mut g.mem,&data,divisor,device,false,false,&mut g.speaker_refresh,&mut ports);
    let (levels,rest)=audio::levels(&ports,g.mem.u16(0x559A));
    g.input.audio(n,audio::cadence_hz(rate),&levels,rest);
}
pub fn sound_busy(g:&Ground)->u16 {if g.mem.sound_enabled()!=0 {g.mem.u16(0x4B16)} else {0}}
pub fn check_deaths(g: &mut Ground) {
    let first = g.mem.party_u8(1) as u16;
    if g.mem.party_u8(first.wrapping_mul(256).wrapping_add(0x127)) <= g.mem.party_u8(first.wrapping_mul(256).wrapping_add(0x126)) {
        for i in 1..5 {
            let member = g.mem.party_u8(i + 1) as u16;
            if g.mem.party_u8(member.wrapping_mul(256).wrapping_add(0x127)) > g.mem.party_u8(member.wrapping_mul(256).wrapping_add(0x126)) {
                let old = g.mem.party_u8(1);
                g.mem.set_party_u8(i + 1, old);
                g.mem.set_party_u8(1, member as u8)
            }
        }
    }
    g.mem.set_selected_char(g.mem.party_u8(1));
}
pub fn boot(g: &mut Ground) -> bool {
    let (world, area) = (g.mem.party_u8(6) as u16, g.mem.party_u8(7) as u16);
    g.mem.set_pointer(0x6b6e, g.mem.pointer(0x6d65).add(0x602));
    if !init_world(g, world, area) {
        return false;
    }
    render::draw_menu(g,Mem::ds(0x435B));
    render::present_menu(g);
    g.mem.set_level(-1);
    if !event_at(g, world, area, 0) {
        return false;
    }
    if g.mem.party_u8(0xc1) & 128 != 0 {
        g.mem.set_party_u8(0xc1, g.mem.party_u8(0xc1) & 127);
        let p = g.mem.pointer(0xb8);
        render::show_message(g, p);
        set_player_pos(g, 22, 6)
    } else {
        let p = ds_string(g, b"A sign nearby says: \"No littering in the Starport area! Oxygen supplied by SuSAG, LIC MegaCorp.\"");
        if packed(p) != 0 {
            render::show_message(g, p)
        }
    }
    g.mem.set_event(-1);
    true
}
impl Context<'_> {
    fn new_object(&mut self, id: i32) -> i32 {
        for i in 0..256 {
            let off = i * 5 + 0xa82;
            if self.party(off) == 255 {
                self.sparty(off, id);
                let def = self.g.mem.pointer(0x6d6d).add(id * 46);
                match self.w(def, 0x1d) {
                    2 => {
                        let n = self.call("random", &[3]);
                        self.sparty(off + 1, n as i32);
                        let n = self.call("random", &[self.b(def, 0x28)]);
                        self.sparty(off + 2, n as i32)
                    }
                    3 => {
                        self.sparty(off + 1, self.b(def, 0x27) & 15);
                        self.sparty(off + 2, self.b(def, 0x28))
                    }
                    _ => {
                        self.sparty(off + 1, self.b(def, 0x27));
                        self.sparty(off + 2, 0)
                    }
                }
                return i + 1;
            }
        }
        -1
    }
    fn place_object(&mut self, x: i32, y: i32, item: i32) -> i32 {
        if item == -1 {
            return -1;
        }
        let list = self.g.mem.pointer(0x6b6e);
        let Some(slot) = (1..128).find(|&i| self.w(list, i * 9) == 0) else { return -1 };
        let def = self.g.mem.pointer(0x6d6d).add(self.party((item - 1) * 5 + 0xa82) * 46);
        if self.w(def, 0x1d) == 11 && self.party((item - 1) * 5 + 0xa84) != 0 {
            self.sparty((item - 1) * 5 + 0xa84, 0);
            let n = self.g.mem.party_u16(0xf82);
            self.g.mem.set_party_u16(0xf82, n.wrapping_sub(1))
        }
        let cell = self.cell(x, y);
        if self.g.mem.u8(0x6fb4) == b'1'
            && self.b(cell, 0) & 1 != 0
            && (self.b(cell, 1) == 0x6d || (0xdc..=0xe1).contains(&self.b(cell, 1)))
        {
            let p = ds_string(self.g, b"The item disappears into the depths.");
            self.call("show_message", &[p.off as i32, p.seg as i32]);
            return 0;
        }
        let p = list.add(slot * 9);
        self.ww(p, 0, item);
        self.wb(p, 2, 0);
        self.wb(p, 3, self.b(def, 0x2d));
        self.ww(p, 4, x);
        self.ww(p, 6, y);
        self.wb(p, 8, 0);
        let old = self.b(cell, 0) >> 1;
        self.wb(cell, 0, (self.b(cell, 0) & 1) | (slot << 1));
        if old != 0 {
            self.wb(p, 2, old)
        }
        self.g.mem.set_event_id(-1);
        slot
    }
    fn spawn_area_objects(&mut self) {
        for i in 0..self.g.mem.u8(0x6b69) as i32 {
            let p = Mem::ds(0x6340).add(i * 6);
            if self.b(p, 0) != self.g.mem.level() as u8 as i32 {
                continue;
            }
            let bit = self.g.mem.u8(0x5e21) as i32 + i;
            if self.party((bit >> 3) + 0x6c) & (1 << (bit & 7)) == 0 {
                let item = self.new_object(self.b(p, 1));
                let slot = self.place_object(self.w(p, 2), self.w(p, 4), item);
                if slot != -1 {
                    let list = self.g.mem.pointer(0x6b6e);
                    self.wb(list, slot * 9 + 8, bit + 1)
                }
            }
        }
    }
}
pub fn shake_screen(g: &mut Ground, delay: i32, n: i32) {
    let timed = !g.harness && g.lockstep.is_none();
    for _ in 0..delay.max(0) { play_sound(g, 6); if timed { let now = g.now(); g.mem.set_u32(0x4bde, now); wait_ticks(g, sound_busy(g) as i16 / 10) } }
    for _ in 0..n.max(0) { for bob in [1i16, -1] {
        g.mem.set_view_y(g.mem.view_y().wrapping_add(bob));
        if let Some(h) = &mut g.scene { h.shake = bob.max(0) as i8 }
        render::redraw(g);
        if timed { let now = g.now(); g.mem.set_u32(0x4bde, now); wait_ticks(g, 1) }
    } }
}
pub fn random(g: &mut Ground, bound: i16) -> i16 {
    g.observe_boot("random", &[bound]);
    if bound == 0 {
        return 0;
    }
    g.rand.recording = g.harness;
    let n = g.rand.random(bound as u16);
    g.mem.set_u32(0x5c20, g.rand.lcg.seed);
    g.observe_boot("random_return", &[n as i16]);
    n as i16
}
#[cfg(test)]
mod tests {
    use super::*;
    mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use mt_engine::{io::{Dir, Files, Input, Scripted}, sim::tick::Counter};
    use std::{cell::RefCell, rc::Rc};
    struct Seen(Rc<RefCell<Vec<(i8, i16)>>>);
    impl Input for Seen {
        fn next_key(&mut self) -> u16 { 13 }
        fn scene(&mut self, s: &dyn std::any::Any) { if let Some(s) = s.downcast_ref::<crate::ui::Scene>() { self.0.borrow_mut().push((s.shake, s.view.1)) } }
    }
    #[test]
    fn the_shake_bobs_the_view_a_frame_at_a_time() {
        let Some(dir) = oracle_env::gamefiles() else { return }; let dir = Dir(dir);
        let mut g = Ground::new(&dir.read("T.DAT").unwrap(), dir.read("DEFAULT.SAV").unwrap(), Box::new(Scripted::new([])), Box::new(Counter::at(0x1234)), Box::new(dir)).unwrap();
        crate::ground::start(&mut g);
        g.scenario_boot = true; g.sound_option = b'A';
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0); g.mem.set_party_u8(0xC1, g.mem.party_u8(0xC1) & !128);
        assert!(render::game_init(&mut g));
        let seen = Rc::new(RefCell::new(Vec::new()));
        g.scene = Some(crate::ui::SceneHook::default()); g.input = Box::new(Seen(seen.clone()));
        let (view, t0) = (g.mem.view_y(), g.now());
        shake_screen(&mut g, 0, 3);
        let mut frames: Vec<_> = seen.borrow().iter().map(|&(s, v)| (s, v - view)).collect(); frames.dedup();
        assert_eq!(frames, [(1, 1), (0, 0), (1, 1), (0, 0), (1, 1), (0, 0)], "out and back, three times");
        assert_eq!(g.mem.view_y(), view); assert!(g.now().wrapping_sub(t0) >= 6, "each frame stays up a tick in a played window");
    }
}
