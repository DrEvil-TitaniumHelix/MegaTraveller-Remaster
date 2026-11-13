use mt_engine::io::dump::json_str;
use std::{fmt::{self, Write as _}, mem::size_of};
pub const MAX_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_JSON_BYTES: usize = 64 * 1024 * 1024;
pub type Region = (u16, u16, Vec<u8>);
fn runaway() -> ! { eprintln!("ground: runaway: TRACE byte budget exceeded ({MAX_BYTES} retained, {MAX_JSON_BYTES} JSON)"); std::process::exit(3) }
fn charge(used: &mut usize, n: usize) {
    *used = used.checked_add(n).filter(|&v| v <= MAX_BYTES).unwrap_or_else(|| runaway());
}
fn slot<T>(v: &mut Vec<T>, used: &mut usize) {
    if v.len() < v.capacity() { return }
    let target = v.capacity().saturating_mul(2).max(4);
    let extra = (target - v.capacity()).checked_mul(size_of::<T>()).unwrap_or_else(|| runaway());
    charge(used, extra);
    v.try_reserve_exact(target - v.len()).unwrap_or_else(|_| runaway());
}
fn copy<T: Copy>(v: &[T], used: &mut usize) -> Vec<T> {
    charge(used, v.len().checked_mul(size_of::<T>()).unwrap_or_else(|| runaway()));
    v.to_vec()
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Call { pub name: &'static str, pub args: Vec<u16>, pub ret: Option<u32> }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    pub name: &'static str, pub args: Vec<u16>, pub ax: Option<u16>, pub calls: Vec<Call>, pub writes: Vec<Region>,
    pub entry: Option<Vec<Region>>,
}
impl Frame {
    fn heap_bytes(&self) -> usize {
        let regions = |v: &Vec<Region>| v.capacity() * size_of::<Region>() + v.iter().map(|r| r.2.capacity()).sum::<usize>();
        self.args.capacity() * 2 + self.calls.capacity() * size_of::<Call>() + self.calls.iter().map(|c| c.args.capacity() * 2).sum::<usize>()
            + regions(&self.writes) + self.entry.as_ref().map_or(0, regions)
    }
}
#[derive(Debug, Default)]
pub struct Trace { pub on: bool, open: Vec<Frame>, pub done: Vec<Frame>, bytes: usize,
    pub lean: bool }
impl Trace {
    pub fn take_walk_jsonl(&mut self) -> String {
        self.done.retain(|f| matches!(f.name,
            "walker_set_pose" | "try_reload" | "npc_can_step" | "move_party" |
            "npc_tick" | "set_leader" | "scroll_view_to_player" | "party_order" |
            "try_move" | "vehicle_terrain" | "foot_terrain" | "set_cell_flags" | "release_cell"));
        self.take_jsonl()
    }
    pub fn enter(&mut self, name: &'static str, args: &[u16]) {
        if !self.on { return }
        self.call(name, args, None);
        slot(&mut self.open, &mut self.bytes);
        let args = copy(args, &mut self.bytes);
        self.open.push(Frame { name, args, ..Frame::default() });
    }
    pub(crate) fn capture_entry<'a>(&mut self, regions: impl Iterator<Item = (u16, u16, &'a [u8])>) {
        if !self.on || self.lean { return }
        if let Some(f) = self.open.last_mut() {
            let mut entry = Vec::new();
            for (seg, off, bytes) in regions {
                slot(&mut entry, &mut self.bytes);
                entry.push((seg, off, copy(bytes, &mut self.bytes)));
            }
            f.entry = Some(entry);
        }
    }
    pub fn exit(&mut self, ax: Option<u16>) {
        if !self.on { return }
        if let Some(mut f) = self.open.pop() {
            f.ax = ax;
            if let Some(p) = self.open.last_mut() { if let Some(c) = p.calls.last_mut() { if c.name == f.name && c.ret.is_none() { c.ret = ax.map(|a| a as u32) } } }
            slot(&mut self.done, &mut self.bytes); self.done.push(f);
        }
    }
    pub fn call(&mut self, name: &'static str, args: &[u16], ret: Option<u32>) {
        if !self.on { return }
        if let Some(p) = self.open.last_mut() {
            slot(&mut p.calls, &mut self.bytes);
            p.calls.push(Call { name, args: copy(args, &mut self.bytes), ret });
        }
    }
    pub fn write(&mut self, seg: u16, off: u16, bytes: &[u8]) {
        if !self.on || self.open.is_empty() || bytes.is_empty() || (self.lean && seg >= 0xA000) { return }
        for f in &mut self.open {
            slot(&mut f.writes, &mut self.bytes);
            f.writes.push((seg, off, copy(bytes, &mut self.bytes)));
        }
    }
    pub fn depth(&self) -> usize { self.open.len() }
    pub fn retained_bytes(&self) -> usize { self.bytes }
    pub fn take_jsonl(&mut self) -> String {
        let mut out = Json(String::new());
        for f in &self.done {
            if f.entry.is_none() { eprintln!("ground: TRACE {} has no entry memory; use Mem::trace_enter", f.name); std::process::exit(3) }
            json(&mut out, f).unwrap_or_else(|_| runaway())
        }
        self.done = Vec::new();
        if self.open.is_empty() { self.open = Vec::new() }
        self.bytes = self.open.capacity() * size_of::<Frame>() + self.open.iter().map(Frame::heap_bytes).sum::<usize>();
        out.0
    }
}
struct Json(String);
impl fmt::Write for Json {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let len = self.0.len().checked_add(s.len()).filter(|&n| n <= MAX_JSON_BYTES).ok_or(fmt::Error)?;
        if len > self.0.capacity() {
            let cap = len.max(self.0.capacity().saturating_mul(2)).min(MAX_JSON_BYTES);
            self.0.try_reserve_exact(cap - self.0.len()).map_err(|_| fmt::Error)?;
        }
        self.0.push_str(s); Ok(())
    }
}
fn values<T: fmt::Display>(out: &mut Json, v: &[T]) -> fmt::Result {
    for (i, x) in v.iter().enumerate() { if i != 0 { out.write_char(',')? } write!(out, "{x}")? }
    Ok(())
}
fn regions(out: &mut Json, v: &[Region]) -> fmt::Result {
    out.write_char('[')?;
    for (i, (s, o, b)) in v.iter().enumerate() {
        if i != 0 { out.write_char(',')? } write!(out, "[{s},{o},[")?; values(out, b)?; out.write_str("]]")?;
    }
    out.write_char(']')
}
fn json(out: &mut Json, f: &Frame) -> fmt::Result {
    write!(out, "{{\"name\":{},\"args\":[", json_str(f.name))?; values(out, &f.args)?;
    write!(out, "],\"ax\":{},\"calls\":[", f.ax.map_or("null".into(), |a| a.to_string()))?;
    for (i, c) in f.calls.iter().enumerate() {
        if i != 0 { out.write_char(',')? }
        write!(out, "{{\"name\":{},\"args\":[", json_str(c.name))?; values(out, &c.args)?;
        write!(out, "],\"ret\":{}}}", c.ret.map_or("null".into(), |r| r.to_string()))?;
    }
    out.write_str("],\"writes\":")?; regions(out, &f.writes)?;
    out.write_str(",\"entry\":")?;
    match &f.entry { Some(e) => regions(out, e)?, None => out.write_str("null")? }
    out.write_str("}\n")
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn integration_exports_contract_routines_and_retains_helper_calls() {
        let mut t=Trace::default();t.on=true;
        t.enter("npc_tick",&[1]);t.capture_entry(std::iter::empty());
        t.enter("check_party_alive",&[]);t.capture_entry(std::iter::empty());t.exit(None);
        t.enter("set_cell_flags",&[1,2,3]);t.capture_entry(std::iter::empty());t.exit(None);
        t.exit(Some(1));
        let text=t.take_walk_jsonl();
        assert_eq!(text.lines().count(),2);
        assert!(text.contains("\"calls\":[{\"name\":\"check_party_alive\""));
        assert!(text.lines().next().unwrap().starts_with("{\"name\":\"set_cell_flags\""));
        assert_eq!(t.retained_bytes(),0);
    }
    #[test]
    fn open_allocations_stay_charged_across_dumps() {
        let mut t = Trace::default(); t.on = true; t.enter("outer", &[]); t.capture_entry(std::iter::empty());
        for _ in 0..16 {
            t.call("callee", &[1, 2], None); t.write(1, 2, &[7; 1024]);
            let before = t.retained_bytes(); assert!(t.take_jsonl().is_empty()); assert_eq!(t.retained_bytes(), before);
        }
        assert!(t.retained_bytes() >= 16 * 1024);
        t.enter("nested", &[1]); t.capture_entry(std::iter::empty()); t.exit(Some(0)); t.take_jsonl();
        let retained = t.retained_bytes(); t.write(1, 2, &[0; 1024]); assert!(t.retained_bytes() >= retained + 1024);
        t.exit(None); t.take_jsonl(); assert_eq!(t.retained_bytes(), 0);
    }
    #[test]
    fn frames_in_completion_order() {
        let mut t = Trace::default();
        t.enter("move_party", &[0, 2, 1]); t.write(0x3062, 1, &[1]);
        assert_eq!((t.depth(), t.take_jsonl().as_str()), (0, ""));
        t.on = true;
        t.enter("move_party", &[0, 0xFFFE, 3]);
        t.capture_entry(std::iter::empty());
        t.call("get_ticks", &[], Some(0x12345)); t.write(0x3062, 0x6B6C, &[44, 0]);
        t.enter("try_move", &[44, 10]); t.capture_entry(std::iter::empty()); t.write(0x3062, 0xE8, &[1]); t.call("door_key", &[7], Some(0)); t.exit(Some(1));
        t.write(0x3062, 0, &[]);
        t.exit(Some(0));
        assert_eq!(t.depth(), 0); assert_eq!(t.done.len(), 2); assert_eq!(t.done[0].name, "try_move");
        assert_eq!(t.done[1].calls, [Call { name: "get_ticks", args: vec![], ret: Some(0x12345) }, Call { name: "try_move", args: vec![44, 10], ret: Some(1) }]);
        assert_eq!(t.done[1].writes, [(0x3062, 0x6B6C, vec![44, 0]), (0x3062, 0xE8, vec![1])]); assert_eq!(t.done[0].writes, [(0x3062, 0xE8, vec![1])]);
        let j = t.take_jsonl();
        assert_eq!(j.replace(",\"entry\":[]", ""), "{\"name\":\"try_move\",\"args\":[44,10],\"ax\":1,\"calls\":[{\"name\":\"door_key\",\"args\":[7],\"ret\":0}],\"writes\":[[12386,232,[1]]]}\n{\"name\":\"move_party\",\"args\":[0,65534,3],\"ax\":0,\"calls\":[{\"name\":\"get_ticks\",\"args\":[],\"ret\":74565},{\"name\":\"try_move\",\"args\":[44,10],\"ret\":1}],\"writes\":[[12386,27500,[44,0]],[12386,232,[1]]]}\n");
        assert!(t.take_jsonl().is_empty()); t.exit(None);
        t.enter("set_leader", &[1]); t.capture_entry(std::iter::empty()); t.exit(None); assert!(t.take_jsonl().contains("\"ax\":null"));
    }
}
