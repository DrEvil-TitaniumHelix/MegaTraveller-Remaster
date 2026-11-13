use super::{Space, state::*};
use crate::lockstep::SpaceEvent;

pub const DEADLINES: u16 = 0x345E;
pub const ACTIVE: u16 = 0x34BA;
pub const INTERVALS: u16 = 0x349A;
pub const SNAPSHOT: u16 = 0x0188;

pub trait Hooks {
    fn update_cursor(&mut self, sp: &mut Space) { if sp.observer.is_none() { sp.poll_input() } sp.update_software_cursor() }
    fn hide_cursor(&mut self, sp: &mut Space) { sp.hide_software_cursor() }
    fn show_cursor(&mut self, sp: &mut Space) { sp.show_software_cursor() }
    fn redraw_tactical(&mut self, _sp: &mut Space, _mode: i16) -> Option<i16> { None }
    fn redraw_active_station(&mut self, sp: &mut Space) { sp.redraw_active_station() }
}
pub struct Native;
impl Hooks for Native {}

impl Space {
    pub fn ticks(&mut self) -> u32 { let now = self.read(); self.event(SpaceEvent::Read); now }
    pub(super) fn read(&mut self) -> u32 {
        self.ev.reads = self.ev.reads.wrapping_add(1);
        let measured = match self.observer.take() { Some(mut o) => { let v = o.read(self); self.observer = Some(o); v } None => None };
        let now = measured.unwrap_or_else(|| self.clock.now());
        self.last_ticks = now;
        now
    }
    pub fn rand(&mut self) -> i16 { self.ev.rands = self.ev.rands.wrapping_add(1); let n = self.s.rand_raw(); self.event(SpaceEvent::Rand); n }
    pub fn random(&mut self, bound: i16) -> i16 { if bound == 0 { 0 } else { (self.rand() as i32 % bound as i32) as i16 } }
    pub(super) fn event(&mut self, e: SpaceEvent) {
        let Some(mut o) = self.observer.take() else { return };
        o.event(self, e);
        self.observer = Some(o);
    }
    pub fn with_hooks(&mut self, f: impl FnOnce(&mut Space, &mut dyn Hooks)) {
        let mut h = self.hooks.take().unwrap_or_else(|| Box::new(Native));
        f(self, h.as_mut());
        self.hooks = Some(h);
    }
    pub fn update_cursor(&mut self) { self.ev.cursor = self.ev.cursor.wrapping_add(1); self.event(SpaceEvent::Cursor); self.with_hooks(|sp, h| h.update_cursor(sp)) }
    pub fn hide_cursor(&mut self) { self.with_hooks(|sp, h| h.hide_cursor(sp)) }
    pub fn show_cursor(&mut self) { self.with_hooks(|sp, h| h.show_cursor(sp)) }
    pub fn snapshot_clock(&mut self) { let now = self.ticks(); self.s.set_dword(SNAPSHOT, now); }
    pub fn wait_with_input(&mut self, delay: i16) -> Option<i16> {
        if self.input.data_mode() && !self.play_time_wait { return super::play::native_wait(self,delay); }
        if self.product && self.observer.is_none() && !self.input.data_mode() && self.s.dword(SNAPSHOT) == 0xFFFF { return None }
        if self.s.dword(SNAPSHOT) != 0xFFFF { self.snapshot_clock(); }
        while self.ticks() < self.s.dword(SNAPSHOT).wrapping_add(delay as i32 as u32) {
            self.update_cursor();
            let have_key = self.s.byte(KEY) != 0;
            self.event(SpaceEvent::WaitKeyTest);
            if have_key { let k = self.s.byte(KEY); self.s.set_byte(PENDING_KEY, k); self.s.set_byte(KEY, 0); return Some(1); }
        }
        self.s.set_dword(SNAPSHOT, 0xFFFF);
        None
    }
    pub fn wait_ticks(&mut self, n: i16) { let deadline = self.ticks().wrapping_add(n as i32 as u32); while self.ticks() < deadline { if self.input.data_mode() { self.clock.wait_tick(); } } }
    pub fn run_tasks(&mut self, watched: i16) -> i16 {
        let (mut changed, mut enemy_changed) = (0i16, 0i16);
        if self.s.word(STATION_ACTIVE) != 0 && self.s.word(STATION) != 15
            && (self.s.word(TACTICAL_DRAWN) == 0 || self.ticks() > self.s.dword(DEADLINES + 32)) && self.s.word(ENCOUNTER) > 0 {
            let mut scripted = None; self.with_hooks(|sp, h| scripted = h.redraw_tactical(sp, 0));
            enemy_changed = match scripted { Some(v) => v, None => self.redraw_tactical_radar(0) };
        }
        for i in 0..10i16 {
            self.update_cursor();
            let (active, deadline) = (ACTIVE + 2 * i as u16, DEADLINES + 4 * i as u16);
            if self.s.word(active) != -1 && self.ticks() >= self.s.dword(deadline) {
                if watched == i { changed = 1; }
                match i {
                    0 => {
                        self.hide_cursor();
                        let now = self.ticks(); self.s.set_dword(deadline, now.wrapping_add(self.s.word(INTERVALS) as i32 as u32));
                        let who = (self.s.word(active) as u16).wrapping_mul(256);
                        self.s.set_pbyte(who.wrapping_add(0x127), self.s.pbyte(who.wrapping_add(0x127)).wrapping_add(1));
                        self.with_hooks(|sp, h| h.redraw_active_station(sp));
                        let who = (self.s.word(active) as u16).wrapping_mul(256);
                        if self.s.pbyte(who.wrapping_add(0x127)) == self.s.pbyte(who.wrapping_add(0x125)) { changed = 1; self.s.set_word(active, -1); }
                        self.show_cursor();
                    }
                    1..=4 => {
                        self.hide_cursor();
                        if (1..5).contains(&watched) { changed = 1; }
                        let now = self.ticks(); self.s.set_dword(deadline, now.wrapping_add(self.s.word(INTERVALS + 2 * i as u16) as i32 as u32));
                        let at = 0xC1 + i as u16; self.s.set_pbyte(at, self.s.pbyte(at).wrapping_sub(1));
                        if self.s.pbyte(at) == 0 { changed = 1; self.s.set_word(active, -1); }
                        self.show_cursor();
                    }
                    5 => {
                        self.hide_cursor();
                        let now = self.ticks(); self.s.set_dword(deadline, now.wrapping_add(self.s.word(INTERVALS + 10) as i32 as u32));
                        self.s.set_word(0x39D, self.s.word(0x39D).wrapping_sub(1));
                        if self.s.word(0x39D) == 0 {
                            changed = 1;
                            let at = (self.s.word(active) as u16).wrapping_add(0xAC); self.s.set_pbyte(at, self.s.pbyte(at).wrapping_add(64));
                            self.s.set_word(active, -1);
                            let at = (self.s.pbyte(0xA3) as u16).wrapping_sub(1).wrapping_mul(256).wrapping_add(0x188);
                            self.s.set_pbyte(at, self.s.pbyte(at).wrapping_add(1));
                        }
                        self.show_cursor();
                    }
                    6 => self.s.set_word(active, -1),
                    7 => {
                        if self.s.word(STATION_ACTIVE) == 0 && self.s.word(MOVING) != 0 && self.s.pbyte(0xFC) != 0 {
                            self.hide_cursor();
                            let fuel = self.s.pbyte(0xFC) as i16;
                            self.r.front(&mut self.s); self.r.fill(&self.s, 183, 4, 5, fuel, 0);
                            self.r.back(&mut self.s); self.r.fill(&self.s, 183, 4, 5, fuel, 0);
                            self.s.set_pbyte(0xFC, self.s.pbyte(0xFC).wrapping_sub(1));
                            let now = self.ticks(); self.s.set_dword(deadline, now.wrapping_add(72));
                            let fuel = self.s.pbyte(0xFC) as i16;
                            self.r.front(&mut self.s); self.r.fill(&self.s, 183, 4, 5, fuel, 9);
                            self.r.back(&mut self.s); self.r.fill(&self.s, 183, 4, 5, fuel, 9);
                            self.show_cursor();
                        } else { self.s.set_word(MOVING, 0); }
                    }
                    9 => {
                        for at in [0x1BF, 0x1C1] { let charge = self.s.word(at); if charge < 100 { self.s.set_word(at, charge.wrapping_add(5).min(100)); } }
                        let now = self.ticks(); self.s.set_dword(deadline, now.wrapping_add(26));
                    }
                    _ => {}
                }
            }
            if changed == 0 { changed = enemy_changed; }
        }
        changed
    }
}
