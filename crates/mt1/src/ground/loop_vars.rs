use mt_engine::sim::sched::Job;
#[cfg(test)] mod tests;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoopVars {
    pub initialized: bool,
    pub second: Job, pub air: Job, pub npc: Job,
    pub mouse_state: u8, pub now: u32, pub pass_tick: u32, pub last: u32,
    pub changed: i16, pub short: u32, pub first_idle: bool,
}
impl LoopVars {
    pub fn encode(&self) -> Vec<u8> {
        let mut b = vec![self.initialized as u8, self.mouse_state, self.first_idle as u8];
        for v in [self.second.deadline, self.air.deadline, self.npc.deadline, self.now, self.pass_tick, self.last, self.short] { b.extend_from_slice(&v.to_le_bytes()); }
        b.extend_from_slice(&self.changed.to_le_bytes()); b
    }
    pub fn decode(b: &[u8]) -> Result<Self, String> {
        if b.len() != 33 || b[0] > 1 || b[2] > 1 { return Err("save: LOOP: bad length or boolean".into()) }
        let n = |at: usize| u32::from_le_bytes(b[at..at + 4].try_into().unwrap());
        Ok(Self { initialized: b[0] != 0, mouse_state: b[1], first_idle: b[2] != 0,
            second: Job { deadline: n(3) }, air: Job { deadline: n(7) }, npc: Job { deadline: n(11) },
            now: n(15), pass_tick: n(19), last: n(23), short: n(27), changed: i16::from_le_bytes([b[31], b[32]]) })
    }
}
