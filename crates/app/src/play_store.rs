use mt_engine::play::{Choice, Entry, Grant, Journal, Pick, Root, RootKind, Snapshot};
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};

pub const MAX_FILE_BYTES: usize = 16 << 20;
pub const MAX_METADATA_BYTES: usize = 4096;

#[derive(Serialize, Deserialize)]
struct Document { format: u32, id: String, name: String, created: String, build: String, root: RootFile, journal: JournalFile, sequence: u64, hash: String }
#[derive(Serialize, Deserialize)]
struct RootFile { kind: String, image: Option<String>, start_tick: u32, scenario: Option<String>, gamefiles: String }
#[derive(Serialize, Deserialize)]
struct JournalFile { hash_version: u32, entries: Vec<EntryFile> }
#[derive(Serialize, Deserialize)]
struct EntryFile { decision: String, pick: PickFile, before: String, after: Option<String>, label_hash: Option<u32>, grant: GrantFile }
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
enum PickFile { Id(String), Row { row: String, qty: u32 }, Text(String), Number(i64) }
#[derive(Serialize, Deserialize)]
struct GrantFile { requested: u32, used: u32, remaining: u32 }

fn id(id: &str) -> Result<(), String> {
    if id.is_empty() || id.len() > 96 || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_') { return Err("play: snapshot id must contain 1..96 letters, digits, '-' or '_'".into()) } Ok(())
}
fn error(e: impl std::fmt::Display) -> String { format!("play snapshot: {e}") }
fn metadata(s: &Snapshot) -> Result<(), String> {
    for (name, value) in [("name", s.name.as_str()), ("created", &s.created), ("build", &s.build),
        ("gamefiles", &s.root.gamefiles), ("scenario", s.root.scenario.as_deref().unwrap_or(""))] {
        if value.len() > MAX_METADATA_BYTES { return Err(format!("play: {name} exceeds {MAX_METADATA_BYTES} metadata bytes")) }
    }
    Ok(())
}
fn size(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() > MAX_FILE_BYTES { return Err("play: snapshot file exceeds size limit".into()) } Ok(())
}

pub fn compatible_build(snapshot: &Snapshot, current: &str) -> Result<(), String> {
    compatible_id(&snapshot.build,current)
}
fn compatible_id(saved:&str,current:&str)->Result<(),String> {
    if saved == "source archive" || current == "source archive" || saved != current {
        return Err(format!("snapshot build '{saved}' differs from executable build '{current}' (source archive is not a verifiable build)"));
    }
    Ok(())
}

pub fn save(dir: &Path, snapshot: &Snapshot) -> Result<(), String> {
    id(&snapshot.id)?; snapshot.validate()?; metadata(snapshot)?;
    if let Some(image) = &snapshot.root.image { size(image)?; }
    let path = dir.join(format!("{}.json", snapshot.id));
    if path.exists() { return Err("play: snapshot id already exists".into()) }
    let mut root = snapshot.root.clone();
    if root.kind == RootKind::Anchor {
        root.image = Some(mt1::play::anchors::seal(root.image.as_deref().ok_or("play: missing anchor image")?, snapshot.root_sequence()?, root.start_tick)?);
    }
    if let Some(image) = &root.image { size(image)?; }
    let image = root.image.as_ref().map(|_| format!("{}.mtsav", snapshot.id));
    let document = Document { format: 1, id: snapshot.id.clone(), name: snapshot.name.clone(), created: snapshot.created.clone(), build: snapshot.build.clone(),
        root: RootFile { kind: match root.kind { RootKind::NewGame => "newgame", RootKind::Save => "save", RootKind::Anchor => "anchor" }.into(), image: image.clone(), start_tick: root.start_tick, scenario: root.scenario, gamefiles: root.gamefiles },
        journal: JournalFile { hash_version: snapshot.journal.hash_version, entries: snapshot.journal.entries.iter().map(|e| EntryFile {
            decision: e.choice.decision.clone(), pick: match &e.choice.pick { Pick::Id(v) => PickFile::Id(v.clone()), Pick::Row { row, qty } => PickFile::Row { row: row.clone(), qty: *qty }, Pick::Text(v) => PickFile::Text(v.clone()), Pick::Number(v) => PickFile::Number(*v) },
            before: e.before.clone(), after: e.after.clone(), label_hash: e.label_hash,
            grant: GrantFile { requested: e.grant.requested, used: e.grant.used, remaining: e.grant.remaining },
        }).collect() }, sequence: snapshot.sequence, hash: snapshot.hash.clone() };
    let json = serde_json::to_vec_pretty(&document).map_err(error)?;
    size(&json)?;
    fs::create_dir_all(dir).map_err(error)?;
    if let (Some(name), Some(bytes)) = (image, root.image) {
        let tmp = dir.join(format!("{}.mtsav.tmp", snapshot.id)); fs::write(&tmp, bytes).map_err(error)?;
        fs::rename(tmp, dir.join(name)).map_err(error)?;
    }
    let tmp = path.with_extension("json.tmp"); fs::write(&tmp, json).map_err(error)?; fs::rename(tmp, path).map_err(error)
}
fn read_bounded(path: &Path, max: u64) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let mut bytes = vec![]; fs::File::open(path).map_err(error)?.take(max + 1).read_to_end(&mut bytes).map_err(error)?;
    if bytes.len() as u64 > max { return Err("play: snapshot file exceeds size limit".into()) } Ok(bytes)
}
pub fn load(dir: &Path, name: &str) -> Result<Snapshot, String> {
    load_checked(dir,name,None)
}
pub fn load_for_build(dir:&Path,name:&str,current:&str)->Result<Snapshot,String> {
    load_checked(dir,name,Some(current))
}
fn load_checked(dir:&Path,name:&str,current:Option<&str>)->Result<Snapshot,String> {
    id(name)?;
    let d: Document = serde_json::from_slice(&read_bounded(&dir.join(format!("{name}.json")), MAX_FILE_BYTES as u64)?).map_err(error)?;
    if d.format != 1 || d.id != name { return Err("play: unsupported or misnamed snapshot".into()) }
    if let Some(current)=current {compatible_id(&d.build,current)?;}
    let kind = match d.root.kind.as_str() { "newgame" => RootKind::NewGame, "save" => RootKind::Save, "anchor" => RootKind::Anchor, _ => return Err("play: unknown root kind".into()) };
    let image = match d.root.image {
        Some(file) => { if file != format!("{name}.mtsav") { return Err("play: root image must be the adjacent snapshot file".into()) } Some(read_bounded(&dir.join(file), MAX_FILE_BYTES as u64)?) }, None => None,
    };
    let s = Snapshot { id: d.id, name: d.name, created: d.created, build: d.build,
        root: Root { kind, image, start_tick: d.root.start_tick, scenario: d.root.scenario, gamefiles: d.root.gamefiles },
        journal: Journal { hash_version: d.journal.hash_version, entries: d.journal.entries.into_iter().map(|e| Entry {
            choice: Choice::new(e.decision, match e.pick { PickFile::Id(v) => Pick::Id(v), PickFile::Row { row, qty } => Pick::Row { row, qty }, PickFile::Text(v) => Pick::Text(v), PickFile::Number(v) => Pick::Number(v) }),
            before: e.before, after: e.after, label_hash: e.label_hash, grant: Grant { requested: e.grant.requested, used: e.grant.used, remaining: e.grant.remaining },
        }).collect() }, sequence: d.sequence, hash: d.hash };
    s.validate()?; metadata(&s)?;
    if s.root.kind == RootKind::Anchor {
        let saved = mt1::save::Saved::decode(s.root.image.as_deref().ok_or("play: missing anchor image")?)?;
        let meta = saved.play.ok_or("play: missing PLAY section")?;
        if meta.sequence != s.root_sequence()? || meta.start_tick != s.root.start_tick || saved.squad.is_none() { return Err("play: anchor metadata does not match snapshot".into()) }
    }
    Ok(s)
}
pub fn list(dir: &Path) -> Result<Vec<Snapshot>, String> {
    if !dir.exists() { return Ok(vec![]) }
    let mut snapshots = vec![];
    for entry in fs::read_dir(dir).map_err(error)? {
        let p = entry.map_err(error)?.path();
        if p.extension().is_some_and(|e| e == "json") { snapshots.push(load(dir, p.file_stem().and_then(|n| n.to_str()).ok_or("play: invalid snapshot name")?)?); }
    }
    snapshots.sort_by(|a, b| a.id.cmp(&b.id)); Ok(snapshots)
}

#[cfg(test)]
mod tests {
    use super::*;
    const LIMIT: usize = 16 << 20;
    fn snapshot() -> Snapshot {
        Snapshot { id: "boundary".into(), name: "Test".into(), created: "now".into(), build: "test".into(),
            root: Root { kind: RootKind::Save, image: Some(vec![1]), start_tick: 0, scenario: None, gamefiles: "test".into() },
            journal: Journal::default(), sequence: 1, hash: "s:final".into() }
    }
    fn directory(case: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/m8p-core/fix1-store").join(format!("{case}-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()))
    }
    #[test]
    fn oversized_metadata_is_rejected_before_creating_files() {
        for field in 0..5 {
            let mut s = snapshot(); let value = "x".repeat(4097);
            match field { 0 => s.name = value, 1 => s.created = value, 2 => s.build = value, 3 => s.root.gamefiles = value, _ => s.root.scenario = Some(value) }
            let dir = directory("metadata"); assert!(save(&dir, &s).is_err(), "metadata field {field}"); assert!(!dir.exists());
            match field { 0 => s.name.pop(), 1 => s.created.pop(), 2 => s.build.pop(), 3 => s.root.gamefiles.pop(), _ => s.root.scenario.as_mut().unwrap().pop() };
            save(&dir, &s).unwrap(); assert_eq!(load(&dir, &s.id).unwrap(), s);
            let path = dir.join("boundary.json"); let mut document: serde_json::Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
            let value = serde_json::Value::String("x".repeat(4097));
            match field { 0 => document["name"] = value, 1 => document["created"] = value, 2 => document["build"] = value,
                3 => document["root"]["gamefiles"] = value, _ => document["root"]["scenario"] = value }
            fs::write(&path, serde_json::to_vec(&document).unwrap()).unwrap(); assert!(load(&dir, &s.id).is_err());
            fs::remove_dir_all(dir).unwrap();
        }
    }
    #[test]
    fn image_size_boundary_round_trips_and_overflow_writes_nothing() {
        let mut s = snapshot(); s.root.image = Some(vec![1; LIMIT + 1]);
        let rejected = directory("image-overflow"); assert!(save(&rejected, &s).is_err()); assert!(!rejected.exists());
        s.root.image.as_mut().unwrap().pop(); let accepted = directory("image-boundary");
        save(&accepted, &s).unwrap(); assert_eq!(load(&accepted, &s.id).unwrap(), s);
        fs::remove_dir_all(accepted).unwrap();
    }
    #[test]
    fn json_size_boundary_round_trips_and_overflow_writes_nothing() {
        let mut s = snapshot(); s.sequence = 2;
        s.journal.entries.push(Entry { choice: Choice::new("d:1", Pick::Text(String::new())), before: "s:first".into(),
            after: Some(s.hash.clone()), label_hash: None, grant: Grant::default() });
        let probe = directory("json-probe"); save(&probe, &s).unwrap();
        let overhead = fs::metadata(probe.join("boundary.json")).unwrap().len() as usize;
        fs::remove_dir_all(probe).unwrap();
        s.journal.entries[0].choice.pick = Pick::Text("x".repeat(LIMIT - overhead + 1));
        let rejected = directory("json-overflow"); assert!(save(&rejected, &s).is_err()); assert!(!rejected.exists());
        if let Pick::Text(text) = &mut s.journal.entries[0].choice.pick { text.pop(); }
        let accepted = directory("json-boundary"); save(&accepted, &s).unwrap();
        assert_eq!(fs::metadata(accepted.join("boundary.json")).unwrap().len(), LIMIT as u64);
        assert_eq!(load(&accepted, &s.id).unwrap(), s); fs::remove_dir_all(accepted).unwrap();
    }
}
