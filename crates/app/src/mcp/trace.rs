use super::*;
use super::route::Record;
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Boundary { pub decision: String, pub state: String, pub kind: String }
impl Boundary {
    fn read(v: &Value) -> Result<Self,String> {
        Ok(Self { decision:string(&v["decision"])?, state:string(&v["state"])?, kind:string(&v["kind"])? })
    }
}
fn string(v: &Value) -> Result<String,String> { v.as_str().map(str::to_owned).ok_or_else(|| "missing trace string".into()) }
#[derive(Clone)]
pub(super) struct Target { pub records: Vec<Record>, pub at: Boundary }
pub(super) struct ReplayFile {
    pub header: Value, pub events: Vec<Value>, pub targets: std::collections::VecDeque<Target>, pub complete: bool,
}
pub(super) fn read(path: &Path) -> Result<ReplayFile,String> {
    let bytes = route::read(path)?;
    let mut rows = bytes.split(|b| *b == b'\n').filter(|b| !b.iter().all(u8::is_ascii_whitespace));
    let header: Value = serde_json::from_slice(rows.next().ok_or("empty trace")?).map_err(|e|e.to_string())?;
    if header["type"] != "header" || !matches!(header["format"].as_u64(),Some(1|2)) || header["hash_version"] != mt_engine::play::HASH_VERSION { return Err("unsupported trace/header/hash version".into()) }
    string(&header["start"])?; string(&header["gamefiles"])?;
    header["start_tick"].as_u64().and_then(|n| u32::try_from(n).ok()).ok_or("invalid trace tick")?;
    header["timeout_ms"].as_u64().filter(|n| (1..=3_600_000).contains(n)).ok_or("invalid trace timeout")?;
    let modern = header["format"] == 2;
    if modern && header["route_steps"].as_u64().is_none_or(|n| n == 0) { return Err("missing route step count".into()) }
    let mut result = ReplayFile { header, events:vec![],targets:Default::default(),complete:false };
    let mut records: Vec<Record> = vec![];
    let mut at: Option<Boundary> = None;
    let mut pending: Option<Value> = None;
    let mut next: Option<(String,String)> = None;
    let mut history: Vec<Boundary> = vec![];
    let mut steps = 0u64;
    let mut terminal = false;
    for line in rows {
        if terminal { return Err("events after terminal record".into()) }
        let v: Value = serde_json::from_slice(line).map_err(|e|e.to_string())?;
        match v["type"].as_str() {
            Some("at") => {
                if pending.is_some() { return Err("at before accepted pick completed".into()) }
                let seen = Boundary::read(&v)?;
                if let Some((id,hash)) = next.take() {
                    if seen.decision != id || seen.state != hash { return Err("at differs from entry/restore result".into()) }
                } else if let Some(old) = &at {
                    if old != &seen { return Err("at differs at unchanged boundary".into()) }
                } else if seen.decision != "d:1" { return Err("trace must begin at d:1".into()) }
                at = Some(seen.clone()); history.push(seen);
            }
            Some("pick") => {
                let a = at.as_ref().ok_or("pick without at")?;
                if pending.is_some() || next.is_some() || v["decision"] != a.decision || v["kind"] != a.kind || v["before"] != a.state { return Err("pick differs from decision boundary".into()) }
                pending = Some(v.clone());
            }
            Some("entry") => {
                let a = at.as_ref().ok_or("entry without at")?;
                let p = pending.take().ok_or("entry without accepted pick")?;
                let e: Record = serde_json::from_value(v["entry"].clone()).map_err(|e|e.to_string())?;
                if e.decision != a.decision || e.kind != a.kind || e.before != a.state || p["pick"] != e.pick { return Err("entry differs from at/pick".into()) }
                let n: u64 = e.decision.strip_prefix("d:").ok_or("bad decision")?.parse().map_err(|_|"bad sequence")?;
                next = Some((format!("d:{}",n.checked_add(1).ok_or("sequence overflow")?),e.after.clone().ok_or("incomplete entry")?));
                records.push(e);
            }
            Some("restore") => {
                if pending.is_some() || next.is_some() { return Err("restore before completed boundary".into()) }
                let current = at.as_ref().ok_or("restore without at")?;
                let restored: Vec<Record> = serde_json::from_value(v["entries"].clone()).map_err(|e|e.to_string())?;
                let prefix_seen = records.starts_with(&restored) || result.targets.iter().any(|t| t.records.starts_with(&restored));
                let state = restored.last().and_then(|e|e.after.as_ref()).or_else(||history.first().map(|b|&b.state)).ok_or("restore without root boundary")?;
                if !prefix_seen || v["decision"] != format!("d:{}",restored.len()+1) || v["state"] != *state { return Err("restore target differs from recorded chain".into()) }
                let target = history.iter().find(|b| v["decision"] == b.decision && v["state"] == b.state).ok_or("restore target was never reached")?;
                result.targets.push_back(Target {records:records.clone(),at:current.clone()});
                records = restored; next = Some((target.decision.clone(),target.state.clone()));
            }
            Some("step") if modern => {
                let a = at.as_ref().ok_or("step without at")?;
                steps += 1;
                if steps > result.header["route_steps"].as_u64().unwrap() || pending.is_some() || next.is_some() || v["index"] != steps || v["decision"] != a.decision || v["state"] != a.state || v["kind"] != a.kind { return Err("step differs from verified boundary/count".into()) }
            }
            Some("complete") => {
                let a = at.as_ref().ok_or("complete without at")?;
                if pending.is_some() || next.is_some() || v["state"] != a.state || v["steps"].as_u64().is_none_or(|n| n == 0) { return Err("incomplete terminal record".into()) }
                if modern && (v["steps"] != steps || result.header["route_steps"] != steps) { return Err("completion does not cover every route step".into()) }
                result.complete = modern; terminal = true;
            }
            Some("prefix_verified") => {
                if let Some((id,state)) = &next {
                    if v["decision"] != *id || v["state"] != *state { return Err("prefix report differs from entry result".into()) }
                } else if let Some(a) = &at {
                    if v["decision"] != a.decision || v["state"] != a.state { return Err("prefix report differs from boundary".into()) }
                }
                terminal = true;
            }
            _ => return Err("invalid trace event".into()),
        }
        result.events.push(v);
    }
    if let Some((decision,state)) = next { at = Some(Boundary {decision,state,kind:String::new()}); }
    if let Some(at) = at { result.targets.push_back(Target {records,at}); }
    Ok(result)
}
