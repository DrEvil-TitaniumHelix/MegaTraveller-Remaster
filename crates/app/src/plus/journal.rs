use mt1::ui::{Scene, ScreenKind};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fs::{self, OpenOptions}, io::{self, BufRead, Write}, path::Path, sync::OnceLock};

pub struct Entry { pub id: &'static str, pub text: &'static str, pub rule: &'static str, pub source: &'static str }
pub fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| super::help::section("## J.", "\0").lines().filter_map(|l| {
        let cols: Vec<_> = l.split('|').map(str::trim).collect();
        (cols.len() >= 6 && cols[1].starts_with('J')).then(|| Entry { id: cols[1], text: cols[2], rule: cols[3], source: cols[4] })
    }).collect())
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Note { pub at: u64, pub text: String }
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Journal {
    pub unlocked: BTreeSet<String>, pub notes: Vec<Note>,
    previous: Option<(u8, u8, i8, i16)>, loaded: bool,
}
fn byte(scene: &Scene, at: usize) -> u8 { scene.party.0.get(at).copied().unwrap_or(0) }
fn has(scene: &Scene, item: std::ops::RangeInclusive<u8>) -> bool {
    (0..5).filter(|i| scene.party.mask() & (1 << i) != 0).any(|i| (0..25).any(|slot| {
        let at = 0x102 + i * 256 + 0xCC + slot * 2;
        let index = u16::from_le_bytes([byte(scene, at), byte(scene, at + 1)]) as usize;
        (1..=256).contains(&index) && item.contains(&byte(scene, 0xA82 + 5 * index.saturating_sub(1)))
    }))
}
impl Journal {
    pub fn observe(&mut self, scene: &Scene) {
        if scene.party.mask() == 0 {
            if matches!(scene.screen, ScreenKind::Startup | ScreenKind::Cg | ScreenKind::Picture) { self.unlocked.clear(); self.previous = None; }
            return;
        }
        let (system, area, flags) = (byte(scene, 6), byte(scene, 7), byte(scene, 0xC1));
        if scene.screen == ScreenKind::Ground { self.unlocked.insert("J1".into()); }
        let arik = scene.context.arik;
        let reached = self.previous.is_some_and(|(s, a, l, timer)| s == system && a == area && l == scene.level && timer > 0)
            && (scene.pos.1 >> 1) & 1 != 0
            && scene.map.cell((scene.pos.0 >> 1) as i32, (scene.pos.1 >> 1) as i32).is_some_and(|c| c[0] & 1 == 0 && c[1] == 19)
            && arik == Some(0) && flags & 64 == 0 && scene.context.pursuer.is_none();
        for (id, yes) in [
            ("J2", self.unlocked.contains("J1") && system == 2),
            ("J3", system == 0 || has(scene, 97..=106)), ("J4", flags & 3 == 2), ("J5", system == 1),
            ("J7", system == 1 && arik.is_some_and(|t| t > 0)),
            ("J8", system == 1 && self.unlocked.contains("J7") && reached),
            ("J9", flags & 64 != 0),
            ("J10", system == 5 && flags & 64 != 0 && arik == Some(0) && scene.context.pursuer == Some(0x12F)),
            ("J11", has(scene, 90..=90)), ("J12", has(scene, 90..=90) && has(scene, 87..=88)),
        ] { if yes { self.unlocked.insert(id.into()); } }
        self.previous = arik.map(|timer| (system, area, scene.level, timer));
    }
    pub fn text(&self) -> String {
        let mut text = String::new();
        for entry in entries() {
            if self.unlocked.contains(entry.id) {
                text.push_str(&format!("{}: {} [{}]\n\n", entry.id, entry.text, entry.source));
            } else { text.push_str(&format!("{}: locked\n", entry.id)); }
        }
        text.push_str("\nYour notes\n");
        for note in &self.notes { text.push_str(&note.text); text.push('\n'); }
        text
    }
    pub fn load_notes(&mut self, root: &Path, faithful: bool) -> io::Result<()> {
        if faithful || self.loaded { return Ok(()); }
        let path = root.join("acceptance/local/journal.jsonl");
        let file = match fs::File::open(path) { Ok(f) => f, Err(e) if e.kind() == io::ErrorKind::NotFound => { self.loaded = true; return Ok(()); }, Err(e) => return Err(e) };
        let mut notes = Vec::new();
        for line in io::BufReader::new(file).lines() {
            let line = line?; if line.trim().is_empty() { continue; }
            notes.push(serde_json::from_str(&line).map_err(io::Error::other)?);
        }
        self.notes = notes; self.loaded = true; Ok(())
    }
    pub fn add_note(&mut self, root: &Path, text: &str, faithful: bool) -> io::Result<()> {
        if faithful { return Ok(()); }
        self.load_notes(root, false)?;
        let text = super::one_line(text);
        if text.is_empty() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Write a note first")); }
        let note = Note { at: super::now(), text };
        let path = root.join("acceptance/local/journal.jsonl"); fs::create_dir_all(path.parent().unwrap())?;
        let mut line = serde_json::to_vec(&note)?; line.push(b'\n');
        OpenOptions::new().create(true).append(true).open(path)?.write_all(&line)?;
        self.notes.push(note); Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene { let mut s = Scene::program(ScreenKind::Ground); s.party.0 = vec![0; 0xF86]; s.party.0[0] = 1; s.party.0[6] = 2; s }
    fn item(s: &mut Scene, slot: usize, id: u8) { s.party.0[0x102 + 0xCC + slot * 2] = (slot + 1) as u8; s.party.0[0xA82 + slot * 5] = id; }
    #[test]
    fn flags_and_held_items_unlock_only_their_entries_unknowns_stay_locked() {
        let mut s = scene(); let mut j = Journal::default(); j.observe(&s);
        assert_eq!(j.unlocked.iter().map(String::as_str).collect::<Vec<_>>(), ["J1", "J2"]);
        s.party.0[0xC1] = 2; j.observe(&s); assert!(j.unlocked.contains("J4"));
        s.party.0[0xA82] = 90; j.observe(&s); assert!(!j.unlocked.contains("J11"), "unreferenced object is not inventory");
        item(&mut s, 0, 90); item(&mut s, 1, 87); item(&mut s, 2, 97); j.observe(&s);
        for id in ["J3", "J11", "J12"] { assert!(j.unlocked.contains(id)); }
        assert_eq!(entries().len(), 14);
        for id in ["J6", "J13", "J14"] { assert!(entries().iter().find(|e| e.id == id).unwrap().rule.contains("unknown")); assert!(!j.unlocked.contains(id)); }
        let before = s.clone(); j.observe(&s); assert_eq!(before, s);
    }
    #[test]
    fn story_timer_candidates_need_their_state_and_latch_after_departure() {
        let mut s = scene(); s.party.0[6] = 1; s.context.arik = Some(0);
        let mut j = Journal::default(); j.observe(&s); assert!(j.unlocked.contains("J5")); assert!(!j.unlocked.contains("J8"));
        s.context.arik = Some(297); j.observe(&s); assert!(j.unlocked.contains("J7"), "a clock already counting (first seen at 297) unlocks J7");
        j=serde_json::from_slice(&serde_json::to_vec(&j).unwrap()).unwrap();
        s.pos = (0, 2); s.map = std::sync::Arc::new(mt1::ui::scene::MapCells { rows: 1, cols: 2, stride: 2, cells: vec![0, 0, 0, 0, 19, 0], ..Default::default() });
        s.context.arik = Some(0); j.observe(&s); assert!(j.unlocked.contains("J8"));
        s.party.0[0xC1] |= 64; j.observe(&s); assert!(j.unlocked.contains("J9")); assert!(!j.text().contains("candidate"), "no research label in the player's Journal");
        s.party.0[6] = 5; j.observe(&s); assert!(!j.unlocked.contains("J10"));
        s.context.pursuer = Some(0x12F); j.observe(&s); assert!(j.unlocked.contains("J10"));
        s.party.0[6] = 0; j.observe(&s); assert!(j.unlocked.contains("J3")); assert!(j.unlocked.contains("J5"));
        j.observe(&Scene::program(ScreenKind::Startup)); assert!(j.unlocked.is_empty());
    }
    #[test]
    fn notes_round_trip_and_faithful_never_creates_files() {
        let root = std::env::temp_dir().join(format!("mt-plus-journal-{}", std::process::id()));
        let mut j = Journal::default(); j.add_note(&root, "faithful", true).unwrap(); assert!(!root.exists());
        j.add_note(&root, "First\nsecond", false).unwrap();
        let mut restored = Journal::default(); restored.load_notes(&root, false).unwrap();
        assert_eq!(restored.notes, j.notes); assert_eq!(j.notes[0].text, "First second");
        assert!(j.add_note(&root, "  ", false).is_err()); fs::remove_dir_all(root).unwrap();
    }
}
