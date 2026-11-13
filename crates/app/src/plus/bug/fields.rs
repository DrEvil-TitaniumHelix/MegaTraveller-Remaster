use mt_engine::play::{Play, Root, Snapshot};
use mt1::{play::diagnostic, ui::{Scene, ScreenKind}};
use serde_json::Value;
use std::{fs, io, path::Path};

#[derive(Clone, Debug, Default)]
pub struct Evidence {
    pub state: Option<diagnostic::State>,
    pub ground_anchor: Option<Vec<u8>>,
}
impl Evidence {
    pub fn scene(&mut self, scene: &Scene) {
        if matches!(scene.screen, ScreenKind::Startup | ScreenKind::Cg) {
            self.state = None; self.ground_anchor = None;
        }
    }
}
#[derive(Clone, Debug)]
pub enum Image { Window(Vec<u8>), Data(Snapshot) }
#[derive(Clone, Debug)]
pub struct PlayFields { pub decision: Value, pub state_hash: Option<String>, pub snapshot: Option<Image> }
pub fn play_fields(scene: &Scene, evidence: &Evidence, session: Option<(&Play, &Root)>) -> PlayFields {
    let decision = session.and_then(|(p, _)| p.pending()).map(crate::mcp::public_decision).unwrap_or(Value::Null);
    let encounter = scene.squad.encounter || evidence.state.as_ref().is_some_and(|s| s.encounter);
    let snapshot = if encounter { None } else if let Some((play, root)) = session {
        play.snapshot(root, "snapshot", "Bug report", &super::super::now().to_string(), env!("MT_BUILD_HASH")).ok().map(Image::Data)
    } else { evidence.ground_anchor.clone().map(Image::Window) };
    PlayFields { decision, state_hash: evidence.state.as_ref().map(|s| s.hash.clone()), snapshot }
}
impl PlayFields {
    pub fn status(&self) -> String {
        let mut missing = vec![];
        if self.decision.is_null() { missing.push("decision"); }
        if self.state_hash.is_none() { missing.push("state_hash"); }
        if self.snapshot.is_none() { missing.push("snapshot"); }
        if missing.is_empty() { "available".into() } else { format!("partial: {}", missing.join(", ")) }
    }
    pub(super) fn write_snapshot(&self, dir: &Path) -> io::Result<Option<&'static str>> {
        match &self.snapshot {
            Some(Image::Window(bytes)) => { fs::write(dir.join("snapshot.mtsav"), bytes)?; Ok(Some("snapshot.mtsav")) }
            Some(Image::Data(snapshot)) => {
                crate::play_store::save(dir, snapshot).map_err(io::Error::other)?; Ok(Some("snapshot.json"))
            }
            None => Ok(None),
        }
    }
}
