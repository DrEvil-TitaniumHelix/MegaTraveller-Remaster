use super::*;

pub(super) struct Replay { pub snapshot: Snapshot, pub next: usize }
impl Snapshot {
    pub fn root_sequence(&self) -> Result<u64, String> {
        let first = if let Some(e) = self.journal.entries.first() {
            e.choice.decision.strip_prefix("d:").ok_or("play: malformed decision id")?.parse::<u64>().map_err(|_| "play: malformed decision id")?
        } else { self.sequence };
        first.checked_sub(1).ok_or_else(|| "play: zero decision sequence".into())
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.sequence == u64::MAX { return Err("play: decision sequence exhausted".into()) }
        if self.journal.hash_version != HASH_VERSION { return Err(format!("play: hash version {} is unsupported (expected {HASH_VERSION})", self.journal.hash_version)) }
        if self.root.kind != RootKind::NewGame && self.root.image.is_none() { return Err("play: root image missing".into()) }
        let first = self.root_sequence()?;
        if self.root.kind != RootKind::Anchor && first != 0 { return Err("play: non-anchor journal does not start at d:1".into()) }
        if first.checked_add(self.journal.entries.len() as u64).and_then(|n| n.checked_add(1)) != Some(self.sequence) { return Err("play: journal does not reach snapshot decision".into()) }
        for (i, e) in self.journal.entries.iter().enumerate() {
            if e.choice.decision != format!("d:{}", first + i as u64 + 1) { return Err("play: discontinuous journal".into()) }
            let after = self.journal.entries.get(i + 1).map_or(&self.hash, |next| &next.before);
            if e.after.as_ref() != Some(after) { return Err(format!("play: incomplete or inconsistent hash chain at {}", e.choice.decision)) }
        }
        Ok(())
    }
}
impl Play {
    pub fn snapshot(&self, root: &Root, id: &str, name: &str, created: &str, build: &str) -> Result<Snapshot, String> {
        let d = self.pending().ok_or("play: snapshot requires a pending decision")?;
        let (root, from) = match &self.anchor {
            Some(a) => (Root { kind: RootKind::Anchor, image: Some(a.image.clone()), ..root.clone() }, a.journal_at),
            None => (root.clone(), 0),
        };
        let s = Snapshot { id: id.into(), name: name.into(), created: created.into(), build: build.into(), root,
            journal: Journal { hash_version: HASH_VERSION, entries: self.journal.entries[from..].to_vec() }, sequence: self.sequence, hash: d.state.clone() };
        s.validate()?; Ok(s)
    }
    pub fn restore(&mut self, snapshot: Snapshot) -> Result<(), String> {
        snapshot.validate()?;
        self.start_sequence(snapshot.root_sequence()?)?;
        self.replay = Some(Replay { snapshot, next: 0 }); Ok(())
    }
    pub fn replaying(&self) -> bool { self.replay.is_some() }
    pub(super) fn replay_choice(&self) -> Maybe<Choice> {
        self.replay.as_ref()?.snapshot.journal.entries.get(self.replay.as_ref()?.next).map(|e| e.choice.clone())
    }
    pub(super) fn check_replay_choice(&self, choice: &Choice) -> Result<(), String> {
        if let Some(expected) = self.replay_choice() { if &expected != choice { return Err("play: choice differs from replay journal".into()) } }
        Ok(())
    }
    pub(super) fn check_replay(&mut self) -> Result<(), String> {
        let Some(r) = &self.replay else { return Ok(()) };
        let d = self.pending().ok_or("play: replay registered no decision")?;
        let (sequence, hash) = match r.snapshot.journal.entries.get(r.next) {
            Some(e) => (e.choice.decision.clone(), &e.before),
            None => (format!("d:{}", r.snapshot.sequence), &r.snapshot.hash),
        };
        if d.id != sequence || &d.state != hash { return Err(format!("play: replay diverged at {}: expected {} {}, got {} {}", r.next, sequence, hash, d.id, d.state)) }
        if r.next != 0 {
            let expected = &r.snapshot.journal.entries[r.next - 1];
            let got = self.journal.entries.last().ok_or("play: replay journal missing entry")?;
            if got != expected { return Err(format!("play: replay entry differs at {}", expected.choice.decision)) }
        }
        if r.next == r.snapshot.journal.entries.len() { self.replay = None; }
        Ok(())
    }
}
