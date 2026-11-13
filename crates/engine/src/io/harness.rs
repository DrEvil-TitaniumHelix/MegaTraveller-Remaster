use super::{dump::{self, Step}, mouse::{MouseEvent, MouseState}, script::Line, Event, Input, Key};
use crate::sim::tick::Clock;
use std::{collections::{VecDeque, BTreeSet}, path::PathBuf, io::Write};

pub struct Harness {
    pub lines: VecDeque<Line>, pub dir: PathBuf, pub n: usize, pub last: Option<Line>, pub files: Vec<(String, Vec<u8>)>, pub exit: bool,
    pub pending_ticks: u32, boundary: bool, pub mouse: MouseState, pub event: Option<MouseEvent>,
    pub scan_gap: u32, scans: VecDeque<Event>, scan_wait: u32, pub max_polls: u64, polls: u64,
    streamed: BTreeSet<String>,
    checkpoints: VecDeque<(super::script::Checkpoint,u32)>, calibrated: bool,
}
impl Harness {
    pub fn new(keys: impl IntoIterator<Item = Key>, dir: PathBuf, exit: bool) -> Harness { Harness::lines(keys.into_iter().map(Line::Key), dir, exit) }
    pub fn lines(lines: impl IntoIterator<Item = Line>, dir: PathBuf, exit: bool) -> Harness {
        let mut lines:VecDeque<_>=lines.into_iter().collect();let mut checkpoints=VecDeque::new();
        while let Some(Line::At{name,tick})=lines.front().copied() {checkpoints.push_back((name,tick));lines.pop_front();}
        let calibrated=!checkpoints.is_empty();
        Harness { lines, checkpoints, calibrated, dir, n: 0, last: None, files: vec![], exit, pending_ticks: 0, boundary: false, mouse: MouseState::default(), event: None, scan_gap: 4, scans: VecDeque::new(), scan_wait: 0, max_polls: 50_000_000, polls: 0, streamed: BTreeSet::new() }
    }
    pub fn write_end(&self) -> std::io::Result<()> { let f: Vec<(&str, &[u8])> = self.files.iter().map(|(n, b)| (n.as_str(), b.as_slice())).collect(); dump::write_end(&self.dir, &f) }
    pub fn exhausted(&self) -> bool { self.n == usize::MAX }
    fn finish(&mut self) {
        if let Err(e) = self.write_end() { eprintln!("harness: {}: {e}", self.dir.display()); if self.exit { std::process::exit(3) } }
        if let Some((name, tick)) = self.checkpoints.front() {
            eprintln!("harness: {} clock checkpoints unconsumed at the end of the script (next: {} {tick})", self.checkpoints.len(), name.name());
            if self.exit { std::process::exit(3) }
        }
        if self.exit { std::process::exit(0) }
        self.n = usize::MAX;
    }
    fn take(&mut self) -> Option<Line> { let l = self.lines.pop_front(); if l.is_some() { self.last = l; self.boundary = false; self.polls = 0 } l }
    fn tick(&mut self, clock: &mut dyn Clock) -> Event {
        clock.wait_tick();
        if !self.scans.is_empty() {
            self.scan_wait = self.scan_wait.saturating_add(1);
            if self.scan_wait >= self.scan_gap { self.scan_wait = 0; return self.scans.pop_front().unwrap_or(Event::Idle) }
        }
        Event::Idle
    }
    fn mouse_line(&mut self, m: MouseEvent) { self.mouse = MouseState { buttons: 0, col: m.col, row: m.row }; self.event = Some(m) }
}
impl Input for Harness {
    fn clock_checkpoint(&mut self,name:&str)->Result<Option<u32>,String> {
        if !self.calibrated {return Ok(None)}
        let Some((expected,tick))=self.checkpoints.pop_front() else {
            if self.exhausted() || (self.lines.is_empty() && self.pending_ticks==0 && self.boundary) {return Ok(None)}
            return Err(format!("clock checkpoint missing: {name}"))
        };
        if expected.name()!=name {return Err(format!("clock checkpoint order: expected {}, reached {name}",expected.name()))}
        Ok(Some(tick))
    }
    fn take_checkpoint(&mut self,name:&str)->Option<u32> {
        match self.checkpoints.front() { Some((expected,tick)) if expected.name()==name => { let tick=*tick; self.checkpoints.pop_front(); Some(tick) } _ => None }
    }
    fn boundary_due(&self) -> bool { !self.exhausted() && self.pending_ticks==0 && (!self.boundary || matches!(self.lines.front(), Some(Line::Key(_) | Line::Mouse(_)))) }
    fn elapse_poll(&mut self,clock:&mut dyn Clock)->Result<Event,String> {
        if self.exhausted() || (self.pending_ticks==0 && self.boundary && self.lines.is_empty()) {return Err("boot clock script ran out before checkpoint".into())}
        Ok(self.poll(clock))
    }
    fn streams_steps(&self) -> bool { self.exit }
    fn stream_step_file(&mut self, name: &str, bytes: &[u8], append: bool) {
        if self.exhausted() || (self.lines.is_empty() && self.pending_ticks == 0 && self.boundary) { return }
        let write = || -> std::io::Result<()> {
            if name.is_empty() || name.contains(['/', '\\', ':']) || name == "." || name == ".." {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "step file must be a basename"))
            }
            let dir = self.dir.join(format!("step_{:04}", self.n));
            std::fs::create_dir_all(&dir)?;
            std::fs::OpenOptions::new().create(true).write(true).append(append).truncate(!append)
                .open(dir.join(name))?.write_all(bytes)
        };
        if let Err(e) = write() { eprintln!("harness: streaming step {} {name}: {e}", self.n); std::process::exit(3) }
        if append {
            if self.streamed.len() >= 64 && !self.streamed.contains(name) { eprintln!("harness: too many append streams"); std::process::exit(3) }
            self.streamed.insert(name.to_string());
        }
    }
    fn next_key(&mut self) -> Key {
        loop {
            match self.take() {
                Some(Line::Key(k)) => return k,
                Some(Line::Ticks(_)) => {}
                Some(Line::Mouse(m)) => self.mouse_line(m),
                Some(Line::Patch { .. } | Line::At { .. }) => { eprintln!("harness: metadata requires polling input"); std::process::exit(3) }
                None => { self.finish(); return super::keys::ESC }
            }
        }
    }
    fn key_pending(&mut self) -> bool { true }
    fn step(&mut self, s: &Step) {
        if self.exhausted() { return }
        let (key, ticks, mouse) = match self.last { Some(Line::Key(k)) => (Some(k), None, None), Some(Line::Ticks(t)) => (None, Some(t), None), Some(Line::Mouse(m)) => (None, None, Some(m)), None | Some(Line::Patch { .. } | Line::At { .. }) => (None, None, None) };
        for (name, bytes) in s.files {
            if self.streamed.contains(*name) && !bytes.is_empty() { self.stream_step_file(name, bytes, true) }
        }
        let files: Vec<_> = s.files.iter().copied().filter(|(name, _)| !self.streamed.contains(*name)).collect();
        let s = Step { key, ticks, mouse, files: &files, ..*s };
        if let Err(e) = dump::write_step(&self.dir, self.n, &s) { eprintln!("harness: step {}: {e}", self.n); if self.exit { std::process::exit(3) } }
        self.n += 1;
        self.streamed.clear();
    }
    fn end_files(&mut self, files: &[(&str, &[u8])]) {
        self.files = files.iter().map(|(n, b)| (n.to_string(), b.to_vec())).collect();
        if self.exhausted() || self.files.is_empty() { return }
        if let Err(e) = self.write_end() { eprintln!("harness: {}: {e}", self.dir.display()); if self.exit { std::process::exit(3) } }
    }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        if self.exhausted() { return Event::Key(super::keys::ESC) }
        self.polls = self.polls.saturating_add(1);
        if self.polls > self.max_polls {
            eprintln!("harness: runaway: {} polls without a step boundary (line {:?})", self.polls, self.last);
            if self.exit { std::process::exit(3) }
            self.n = usize::MAX; self.pending_ticks = 0; self.scans.clear(); return Event::Key(super::keys::ESC)
        }
        if self.pending_ticks > 0 { self.pending_ticks -= 1; return self.tick(clock) }
        if !self.boundary { self.boundary = true; return Event::Boundary }
        match self.take() {
            Some(Line::Key(k)) => { self.scans.extend([Event::Key(k), Event::KeyUp(k)]); Event::Idle }
            Some(Line::Mouse(m)) => { self.mouse_line(m); Event::Mouse(m) }
            Some(Line::Patch { .. } | Line::At { .. }) => { eprintln!("harness: metadata must precede input lines"); std::process::exit(3) }
            Some(Line::Ticks(0)) => { self.boundary = true; Event::Boundary }
            Some(Line::Ticks(n)) => { self.pending_ticks = n - 1; self.tick(clock) }
            None => { self.finish(); Event::Key(super::keys::ESC) }
        }
    }
    fn mouse(&mut self) -> MouseState { self.mouse }
    fn set_mouse_position(&mut self, col: i16, row: i16) { self.mouse.col = col; self.mouse.row = row; }
}
#[cfg(test)]
mod t {
    use super::*;
    use crate::sim::tick::Counter;
    #[test]
    fn streamed_evidence_survives_step_finalization() {
        let dir = std::env::temp_dir().join(format!("mt-stream-step-{}", std::process::id()));
        let mut h = Harness::new([], dir.clone(), false);
        h.stream_step_file("trace.jsonl", b"first\n", true);
        h.stream_step_file("trace.jsonl", b"second\n", true);
        h.stream_step_file("frame_0000_draw.bin", &[1, 2, 3], false);
        let files = [("trace.jsonl", b"last\n".as_slice())];
        h.step(&Step { files: &files, ..Step::cg(None, &[], 0, &[], "test", &[], &[], &[]) });
        assert_eq!(std::fs::read(dir.join("step_0000/trace.jsonl")).unwrap(), b"first\nsecond\nlast\n");
        assert_eq!(std::fs::read(dir.join("step_0000/frame_0000_draw.bin")).unwrap(), [1, 2, 3]);
        h.step(&Step { files: &[("trace.jsonl", &[])], ..Step::cg(None, &[], 0, &[], "test", &[], &[], &[]) });
        assert!(std::fs::read(dir.join("step_0001/trace.jsonl")).unwrap().is_empty());
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn steps_then_end() {
        let d = std::env::temp_dir().join(format!("mt-harness-{}", std::process::id()));
        let mut h = Harness::new([13, 0x150], d.clone(), false);
        let s = Step::cg(None, &[1], 2, &[], "cg", &[0; 4], &[0; 16], &[7]);
        h.step(&s); assert_eq!(h.next_key(), 13); h.step(&s); assert_eq!(h.next_key(), 0x150); h.step(&s);
        h.end_files(&[("UCHARS.DAT", &[1, 2][..]), ("pool_record.bin", &[3][..])]);
        assert_eq!(h.next_key(), 27); assert!(h.exhausted()); h.step(&s);
        let j = |n: usize| std::fs::read_to_string(d.join(format!("step_{n:04}/step.json"))).unwrap();
        assert!(j(0).starts_with("{\"key\":null,") && j(1).starts_with("{\"key\":13,") && j(2).starts_with("{\"key\":336,") && !d.join("step_0003").exists());
        assert_eq!((std::fs::read(d.join("UCHARS.DAT")).unwrap(), std::fs::read(d.join("pool_record.bin")).unwrap()), (vec![1, 2], vec![3]));
        assert!(h.key_pending());
        std::fs::remove_dir_all(&d).unwrap();
    }
    #[test]
    fn t_mode_lines() {
        let d = std::env::temp_dir().join(format!("mt-harness-t-{}", std::process::id()));
        let m = MouseEvent { event: 4, buttons: 0, col: 210, row: 175 };
        let mut h = Harness::lines([Line::Ticks(400), Line::Key(13), Line::Ticks(120), Line::Mouse(m), Line::Ticks(0)], d.clone(), false);
        let (mut c, s) = (Counter::default(), Step::cg(None, &[], 0, &[], "t", &[], &[], &[]));
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        for _ in 0..400 { assert_eq!(h.poll(&mut c), Event::Idle) }
        assert_eq!(c.now, 400);
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        assert_eq!(h.poll(&mut c), Event::Idle);
        assert_eq!(c.now, 400);
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        for i in 1..=120 {
            let want = match i { 4 => Event::Key(13), 8 => Event::KeyUp(13), _ => Event::Idle };
            assert_eq!(h.poll(&mut c), want); assert_eq!(c.now, 400 + i);
        }
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        assert_eq!(h.poll(&mut c), Event::Mouse(m)); assert_eq!(h.mouse().col, 210);
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        assert_eq!(h.poll(&mut c), Event::Boundary); h.step(&s);
        assert_eq!(c.now, 520);
        assert_eq!(h.poll(&mut c), Event::Key(super::super::keys::ESC)); assert!(h.exhausted());
        let j = |n: usize| std::fs::read_to_string(d.join(format!("step_{n:04}/step.json"))).unwrap();
        assert!(j(0).contains("\"ticks\":null,\"mouse\":null")); assert!(j(1).contains("\"ticks\":400,"));
        assert!(j(2).starts_with("{\"key\":13,")); assert!(j(4).contains("\"mouse\":[210,175,4]")); assert!(j(5).contains("\"ticks\":0,"));
        std::fs::remove_dir_all(&d).unwrap();
    }
    #[test]
    fn checkpoints_in_capture_order_and_boundary_due() {
        let at = |n: &str, t: u32| Line::At { name: super::super::script::Checkpoint::parse(n).unwrap(), tick: t };
        let mut h = Harness::lines([at("show_message", 3), at("draw_gauge", 5), at("redraw_view", 9), Line::Ticks(4), Line::Key(13), Line::Ticks(6)], PathBuf::new(), false);
        let mut c = Counter::default();
        assert_eq!(h.clock_checkpoint("show_message"), Ok(Some(3)));
        assert_eq!(h.take_checkpoint("redraw_view"), None);
        assert!(h.clock_checkpoint("redraw_view").unwrap_err().contains("order: expected draw_gauge"));
        assert!(h.boundary_due());
        assert_eq!(h.poll(&mut c), Event::Boundary); assert!(!h.boundary_due());
        for i in 1..=4 { assert_eq!(h.poll(&mut c), Event::Idle); assert_eq!(h.boundary_due(), i == 4) }
        assert_eq!(h.poll(&mut c), Event::Boundary); assert!(h.boundary_due());
        assert_eq!(h.poll(&mut c), Event::Idle); assert!(h.boundary_due());
        assert_eq!(h.poll(&mut c), Event::Boundary); assert!(!h.boundary_due());
        for _ in 0..6 { h.poll(&mut c); }
        assert!(h.clock_checkpoint("missing").unwrap_err().contains("missing"));
        assert_eq!(h.poll(&mut c), Event::Boundary);
        assert_eq!(h.clock_checkpoint("after"), Ok(None));
        assert_eq!(h.poll(&mut c), Event::Key(super::super::keys::ESC)); assert!(h.exhausted());
        assert!(!h.boundary_due());
        let mut plain = Harness::lines([Line::Ticks(1)], PathBuf::new(), false);
        assert_eq!(plain.clock_checkpoint("anything"), Ok(None));
    }
    #[test]
    fn queued_scans_wrap_and_poll_budget() {
        let mut h = Harness::lines([Line::Key(0x150), Line::Key(13), Line::Ticks(16)], PathBuf::new(), false);
        let mut c = Counter::at(u32::MAX - 7);
        for _ in 0..2 { assert_eq!(h.poll(&mut c), Event::Boundary); assert_eq!(h.poll(&mut c), Event::Idle) }
        assert_eq!(h.poll(&mut c), Event::Boundary);
        for i in 1..=16 {
            let want = match i { 4 => Event::Key(0x150), 8 => Event::KeyUp(0x150), 12 => Event::Key(13), 16 => Event::KeyUp(13), _ => Event::Idle };
            assert_eq!(h.poll(&mut c), want);
        }
        assert_eq!(c.now, 8);
        let mut h = Harness::lines([Line::Ticks(100)], PathBuf::new(), false);
        h.max_polls = 3;
        assert_eq!(h.poll(&mut c), Event::Boundary);
        for _ in 0..4 { assert_eq!(h.poll(&mut c), Event::Idle) }
        assert_eq!(h.poll(&mut c), Event::Key(super::super::keys::ESC)); assert!(h.exhausted());
        let now = c.now; for _ in 0..10 { h.poll(&mut c); } assert_eq!(c.now, now);
    }
}
