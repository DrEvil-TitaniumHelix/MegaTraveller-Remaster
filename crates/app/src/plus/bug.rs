use super::{journal::Journal, png};
use crate::look::Look;
use mt1::ui::{Scene, ScreenKind};
use serde_json::json;
use std::{fs::{self, OpenOptions}, io::{self, Write}, path::{Path, PathBuf}};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Bug, Qol, Question }
impl Kind {
    pub fn from_index(n: u8) -> Self { match n { 1 => Self::Qol, 2 => Self::Question, _ => Self::Bug } }
    pub fn name(self) -> &'static str { match self { Self::Bug => "bug", Self::Qol => "qol", Self::Question => "question" } }
}
mod fields;
pub use fields::{play_fields, Evidence, Image, PlayFields};
#[cfg(test)]
mod play_fields_tests;

pub struct Capture<'a> {
    pub faithful: bool, pub kind: Kind, pub text: &'a str, pub rgb: &'a [u32],
    pub scene: &'a Scene, pub journal: &'a Journal, pub look: &'a Look,
    pub play: Option<&'a PlayFields>,
}
pub const ISSUES: &str = "https://github.com/DrEvil-TitaniumHelix/MegaTraveller-Remaster/issues";
pub const SEND: &str = "To send it: zip that folder, attach it to an issue:";
#[derive(Clone, Debug)]
pub struct Report { pub id: String, pub directory: PathBuf }

fn stamp(seconds: u64) -> String {
    let mut days = seconds / 86400; let mut year = 1970u64;
    let leap = |y: u64| y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    while days >= if leap(year) { 366 } else { 365 } { days -= if leap(year) { 366 } else { 365 }; year += 1; }
    let lengths = [31, if leap(year) { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0; while days >= lengths[month] { days -= lengths[month]; month += 1; }
    format!("{year:04}{:02}{:02}-{:02}{:02}{:02}", month + 1, days + 1, seconds / 3600 % 24, seconds / 60 % 60, seconds % 60)
}
pub fn write(root: &Path, capture: Capture<'_>) -> io::Result<Option<Report>> { write_at(root, capture, super::now()) }
fn write_at(root: &Path, c: Capture<'_>, created: u64) -> io::Result<Option<Report>> {
    if c.faithful { return Ok(None); }
    let text = super::one_line(c.text);
    if text.is_empty() { return Err(io::Error::new(io::ErrorKind::InvalidInput, "Describe the report first")); }
    let png = png::rgb(320, 200, c.rgb)?;
    let base = format!("{}-mt1", stamp(created));
    let bugs = root.join("acceptance/local/bugs"); fs::create_dir_all(&bugs)?;
    let (id, directory) = (0..10000).find_map(|i| {
        let id = if i == 0 { base.clone() } else { format!("{base}-{}", i + 1) };
        let directory = bugs.join(&id);
        match fs::create_dir(&directory) { Ok(()) => Some(Ok((id, directory))), Err(e) if e.kind() == io::ErrorKind::AlreadyExists => None, Err(e) => Some(Err(e)) }
    }).unwrap_or_else(|| Err(io::Error::new(io::ErrorKind::AlreadyExists, "Too many reports in this second")))?;
    let result = (|| {
        let play = c.play;
        let snapshot = play.map(|p| p.write_snapshot(&directory)).transpose()?.flatten();
        let party = &c.scene.party.0;
        let ground = !party.is_empty() && c.scene.screen != ScreenKind::Space;
        let bundle = json!({
            "id": id, "kind": c.kind.name(), "text": text, "created": created,
            "build": env!("MT_BUILD_HASH"), "title": format!("MegaTraveller 1 Plus - M8P ({})", env!("MT_BUILD_HASH")),
            "screen": format!("{:?}", c.scene.screen),
            "where": { "world": party.get(6), "area": party.get(7), "level": ground.then_some(c.scene.level),
                "cell": ground.then_some([c.scene.pos.0 >> 1, c.scene.pos.1 >> 1]), "clock": c.scene.context.clock },
            "decision": play.as_ref().map(|p| &p.decision),
            "state": play.as_ref().map(|p| &p.state_hash), "state_hash": play.as_ref().map(|p| &p.state_hash),
            "snapshot": snapshot,
            "play_fields_status": play.map_or_else(|| "partial: decision, state_hash, snapshot".into(), PlayFields::status),
            "journal": { "unlocked": c.journal.unlocked, "notes_count": c.journal.notes.len() },
            "options": { "look": c.look, "classic_combat": c.look.classic_combat }
        });
        fs::write(directory.join("screen.png"), &png)?;
        fs::write(directory.join("bundle.json"), serde_json::to_vec_pretty(&bundle)?)?;
        let ledger = root.join("docs/BUGS.md"); fs::create_dir_all(ledger.parent().unwrap())?;
        let previous = match fs::read_to_string(&ledger) { Ok(t) => t, Err(e) if e.kind() == io::ErrorKind::NotFound => String::new(), Err(e) => return Err(e) };
        let n = previous.lines().filter_map(|l| l.strip_prefix('#')?.split_whitespace().next()?.parse::<u64>().ok()).max().unwrap_or(0) + 1;
        let line = format!("#{n} {} {} {id} {}\n", stamp(created), c.kind.name(), text.chars().take(80).collect::<String>());
        OpenOptions::new().create(true).append(true).open(ledger)?.write_all(line.as_bytes())?;
        Ok(())
    })();
    if let Err(e) = result { let _ = fs::remove_dir_all(&directory); return Err(e); }
    Ok(Some(Report { id, directory }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    #[test]
    fn complete_local_bundle_kinds_collisions_and_faithful_no_io() {
        let root = std::env::temp_dir().join(format!("mt-plus-bug-{}", std::process::id()));
        let mut s = Scene::program(ScreenKind::Ground); s.party.0 = vec![0; 0xF86]; s.party.0[0] = 1; s.party.0[6] = 2; s.pos = (44, 10);
        s.context.clock = Some(123); s.squad.encounter = true;
        let mut j = Journal::default(); j.observe(&s); let look = Look::default(); let rgb = vec![0x123456; 64000];
        let capture = |faithful, kind| Capture { faithful, kind, text: "A report\nwith details", rgb: &rgb, scene: &s, journal: &j, look: &look, play: None };
        assert!(write_at(&root, capture(true, Kind::Bug), 0).unwrap().is_none()); assert!(!root.exists());
        for (i, kind) in [Kind::Bug, Kind::Qol, Kind::Question].into_iter().enumerate() {
            let report = write_at(&root, capture(false, kind), 0).unwrap().unwrap();
            assert!(report.id.starts_with("19700101-000000-mt1"));
            let bundle: Value = serde_json::from_slice(&fs::read(report.directory.join("bundle.json")).unwrap()).unwrap();
            for name in ["id", "kind", "text", "created", "build", "title", "screen", "where", "decision", "state", "state_hash", "snapshot", "journal", "options"] { assert!(bundle.get(name).is_some(), "{name}"); }
            assert_eq!(bundle["kind"], kind.name()); assert_eq!(bundle["where"]["cell"], json!([22, 5]));
            assert_eq!(bundle["where"]["clock"], 123); assert_eq!(bundle["text"], "A report with details");
            assert_eq!(bundle["journal"]["unlocked"], json!(["J1", "J2"]));
            for name in ["decision", "state", "state_hash", "snapshot"] { assert!(bundle[name].is_null()); }
            assert!(play_fields(&s, &Evidence::default(), None).snapshot.is_none(), "encounter: no invented snapshot");
            assert_eq!(fs::read(report.directory.join("screen.png")).unwrap(), png::rgb(320, 200, &rgb).unwrap());
            assert!(fs::read_to_string(root.join("docs/BUGS.md")).unwrap().contains(&format!("#{} ", i + 1)));
        }
        assert_eq!(fs::read_dir(root.join("acceptance/local/bugs")).unwrap().count(), 3);
        assert_eq!(stamp(1709164800), "20240229-000000");
        fs::remove_dir_all(root).unwrap();
    }
}
