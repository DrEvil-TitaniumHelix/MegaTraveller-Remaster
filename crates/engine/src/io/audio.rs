use std::collections::VecDeque;

pub fn levels(writes: &[(u16, u8)], dac_port: u16) -> (Vec<u8>, u8) {
    let (mut out, mut level) = (Vec::new(), 0u8);
    for (i, &(port, value)) in writes.iter().enumerate() {
        level = match port {
            0x42 if value == 255 => { out.push(level); continue }
            0x61 => if value & 2 != 0 { 255 } else { 0 },
            p if p == dac_port && p != 0x42 && p != 0x43 => value,
            _ => continue,
        };
        let restore = writes.get(i + 1).is_some_and(|w| w.0 == 0x43);
        if !restore { if let Some(last) = out.last_mut() { *last = level } }
    }
    (out, level)
}
pub fn cadence_hz(rate: u32) -> u32 { if rate == 0 { return 0 } let div = (1_193_180 / rate).max(1); (1_193_180 + div / 2) / div }

struct Play { hz: u32, levels: Vec<u8>, rest: u8, tone: bool }
pub struct Speaker {
    device_hz: u32,
    queue: VecDeque<Play>,
    pos: u64,
    rest: u8,
    hp_x: f32, hp_y: f32, lp: f32,
}
impl Speaker {
    pub const MAX_QUEUE: usize = 64;
    pub fn new(device_hz: u32) -> Speaker { Speaker { device_hz: device_hz.max(1), queue: VecDeque::new(), pos: 0, rest: 0, hp_x: 0.0, hp_y: 0.0, lp: 0.0 } }
    pub fn push(&mut self, hz: u32, levels: Vec<u8>, rest: u8) {
        if hz == 0 || levels.is_empty() { return }
        if self.queue.len() >= Self::MAX_QUEUE { self.queue.pop_front(); self.pos = 0 }
        self.queue.push_back(Play { hz, levels, rest, tone: false });
    }
    pub fn push_tone(&mut self, hz: u32, samples: &[u8]) {
        if hz == 0 || samples.is_empty() { return }
        if self.queue.front().is_some_and(|p| p.tone) { self.pos = 0 }
        self.queue.retain(|p| !p.tone);
        let rest = self.queue.back().map_or(self.rest, |p| p.rest);
        let levels = samples.iter().map(|&s| if rest < 128 { rest.saturating_add(s) } else { rest.saturating_sub(s) }).collect();
        self.queue.push_back(Play { hz, levels, rest, tone: true });
    }
    pub fn idle(&self) -> bool { self.queue.is_empty() }
    fn step(&mut self) -> u8 {
        while let Some(p) = self.queue.front() {
            let i = (self.pos / self.device_hz as u64) as usize;
            if i < p.levels.len() { let v = p.levels[i]; self.pos += p.hz as u64; return v }
            self.rest = p.rest; self.queue.pop_front(); self.pos = 0;
        }
        self.rest
    }
    pub fn fill(&mut self, out: &mut [f32]) {
        for o in out.iter_mut() {
            let x = (self.step() as f32 - 128.0) / 128.0;
            let y = x - self.hp_x + 0.995 * self.hp_y; self.hp_x = x; self.hp_y = y;
            self.lp += 0.3 * (y - self.lp);
            *o = (self.lp * 0.5).clamp(-1.0, 1.0);
        }
    }
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn speaker_levels_follow_port_61_bit1_per_slot() {
        let w = [(0x43, 0x98), (0x61, 0x11), (0x42, 0), (0x42, 255), (0x61, 0x12), (0x61, 0x13), (0x42, 255), (0x42, 255), (0x61, 0x10), (0x61, 0x11), (0x61, 0x10), (0x43, 0xB6), (0x42, 0xD0), (0x42, 4)];
        assert_eq!(levels(&w, 0), (vec![255, 255, 0], 0));
        assert_eq!(levels(&[(0x42, 255), (0x22F, 7), (0x42, 255), (0x22F, 200)], 0x22F), (vec![7, 200], 200));
        assert_eq!(levels(&[(0x42, 255), (0x61, 0x12), (0x61, 0x13), (0x61, 0x10), (0x43, 0xB6), (0x42, 0xD0), (0x42, 4)], 0), (vec![255], 0));
        assert_eq!(levels(&[(0x42, 255), (0x61, 0x12), (0x61, 0x13), (0x42, 255), (0x61, 0x10), (0x43, 0xB6)], 0), (vec![255, 255], 0));
        assert_eq!(levels(&[(0x42, 255), (0x22F, 7), (0x61, 0x12), (0x43, 0xB6)], 0x22F), (vec![7], 255));
        assert_eq!(cadence_hz(5000), 5013); assert_eq!(cadence_hz(7000), 7019); assert_eq!(cadence_hz(0), 0);
    }
    #[test]
    fn player_is_one_voice_in_order_and_bounded() {
        let mut s = Speaker::new(10);
        s.push(5, vec![255, 255], 0); s.push(5, vec![0; 2], 255);
        let mut raw = Vec::new(); for _ in 0..12 { raw.push(s.step()) }
        assert_eq!(raw, [255, 255, 255, 255, 0, 0, 0, 0, 255, 255, 255, 255]); assert!(s.idle());
        for _ in 0..100 { s.push(5, vec![1], 0) } assert_eq!(s.queue.len(), Speaker::MAX_QUEUE);
        let mut out = vec![0f32; 64]; s.fill(&mut out); assert!(out.iter().all(|v| v.is_finite() && v.abs() <= 1.0));
        s.push(0, vec![1], 0); s.push(5, vec![], 0);
        let mut z = Speaker::new(0); z.push(5, vec![255], 0); z.fill(&mut out);
        let mut s = Speaker::new(10); s.rest = 100; s.push_tone(5, &[10, 10]); s.push(5, vec![7], 100); s.push_tone(5, &[20]);
        assert_eq!(s.queue.iter().map(|p| (p.levels.clone(), p.tone)).collect::<Vec<_>>(), [(vec![7], false), (vec![120], true)]);
        let mut hi = Speaker::new(10); hi.rest = 255; hi.push_tone(5, &[240]); assert_eq!(hi.queue[0].levels, [15], "from the high rail the tone goes down");
        s.push_tone(0, &[1]); s.push_tone(5, &[]); assert_eq!(s.queue.len(), 2);
    }
}
