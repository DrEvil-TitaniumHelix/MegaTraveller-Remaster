use super::{Space, state::*};
use mt_engine::io::{scan, Event};

pub const KEY_ASCII: u16 = 0x27A5;
pub const RAW_SCAN: u16 = 0x4840;

impl Space {
    pub fn keyboard_isr(&mut self, raw: u8) {
        self.s.set_byte(RAW_SCAN, raw);
        if raw == 0x1D { return; }
        if raw > 0x80 { if self.s.byte(KEY) & 0x7F > 0x46 { self.s.set_byte(KEY, 0); } } else { self.s.set_byte(KEY, raw); }
    }
    pub fn poll_input(&mut self) {
        if self.input.data_mode() && self.play_time_wait { self.clock.wait_tick(); return }
        if self.input.data_mode() && !self.play_command_poll
            && (self.s.byte(KEY) != 0 || self.input.wants_decision()) { return }
        if !self.input.data_mode() || !self.input.wants_decision() {
            let rgb = self.r.fb.rgb((self.s.word(ACTIVE_PAGE) & 1) as usize, &self.dac);
            self.input.present(&rgb);
            if self.product { self.input.scene(&crate::ui::Scene::space(self)); }
            if self.input.diagnostics() {
                let state = crate::play::diagnostic::space(self); self.input.scene(&state);
            }
        }
        let e = self.input.poll(self.clock.as_mut());
        self.apply(e);
        while self.input.arrived() {
            match self.input.poll(self.clock.as_mut()) { e @ (Event::Key(_) | Event::KeyUp(_) | Event::Mouse(_)) => self.apply(e), _ => break }
        }
    }
    pub(super) fn apply(&mut self, e: Event) {
        match e {
            Event::Key(k) => { if let Some(code) = scan::make_code(k) { self.keyboard_isr(code) } }
            Event::KeyUp(k) => { if let Some(code) = scan::make_code(k) {
                let unread = code > 0x46 && self.s.byte(KEY) == code && self.product && self.observer.is_none() && !self.input.data_mode();
                if !unread { self.keyboard_isr(code | 0x80) }
            } }
            Event::Mouse(m) => self.mouse_callback(m),
            _ => {}
        }
    }
    pub fn spin(&mut self) {
        if self.observer.is_none() { self.poll_input() } else { self.event(crate::lockstep::SpaceEvent::Spin) }
    }
    pub fn translate_station_key(&mut self) -> i16 {
        super::play::station(self,"4707");
        let mut key = 0i16;
        if self.s.byte(KEY) == 0 && self.s.byte(PENDING_KEY) != 0 { let p = self.s.byte(PENDING_KEY); self.s.set_byte(KEY, p); self.s.set_byte(PENDING_KEY, 0); }
        self.show_cursor();
        while key == 0 {
            while self.s.byte(KEY) == 0 { self.update_cursor(); }
            let mut code = self.s.byte(KEY);
            if code == 0x6E {
                self.station_mouse_command();
                code = self.s.byte(KEY);
                if code == 0x6E { self.s.set_byte(KEY, 0); super::play::station(self,"4707"); continue; }
            }
            key = match code {
                0x1C => 13, 0x0E => 14, 1 | 0x17 => 27,
                0x48 => 1, 0x50 => 2, 0x4B => 3, 0x4D => 4, 0x49 => 5, 0x51 => 6, 0x47 => 7, 0x4F => 8, 0x4C => 9, 0x4E => b'+' as i16, 0x4A => b'-' as i16,
                0x60..=0x67 if self.product && self.s.word(STATION_ACTIVE) != 0 => { self.switch_to = Some((code - 0x60) as i32); b'V' as i16 }
                0x3B..=0x3F if self.record_elsewhere((code - 0x3B) as i16) => 256,
                0x46..=0x51 => 0,
                _ => self.s.byte(KEY_ASCII.wrapping_add(code as u16)) as i16,
            };
            if key == 0 && self.s.byte(KEY) != 0 { self.spin(); }
        }
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0);
        self.hide_cursor();
        key
    }
}
