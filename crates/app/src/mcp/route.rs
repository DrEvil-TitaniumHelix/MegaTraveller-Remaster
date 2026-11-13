use super::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Route {
    pub start: String,
    pub start_tick: u32,
    #[serde(default = "timeout")]
    pub timeout_ms: u64,
    pub steps: Vec<Step>,
}
fn timeout() -> u64 { 30_000 }
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Step {
    pub pick: Option<Value>,
    pub snapshot: Option<String>,
    pub restore: Option<String>,
    #[serde(rename = "assert")]
    pub assertion: Option<Value>,
    pub replay: Option<bool>,
    pub expect_kind: Option<String>,
    pub expect_label: Option<String>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub frame: bool,
    pub after: Option<After>,
    #[serde(default)]
    pub unfilled: bool,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct After { pub kind: String, pub state: Option<Value> }
pub(super) fn name(s: &str) -> bool {
    !s.is_empty() && s.len() <= 80 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
fn valid_pick(v: &Value) -> bool {
    v.is_string() || v.as_i64().is_some() || v.as_object().is_some_and(|o| o.len() == 2 &&
        o.get("row").is_some_and(Value::is_string) && o.get("qty").and_then(Value::as_u64).is_some_and(|n| u32::try_from(n).is_ok()))
}
impl Route {
    pub fn read(path: &Path) -> Result<Self, String> {
        let bytes = read(path)?;
        let route: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        route.validate()?; Ok(route)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.start.is_empty() || self.timeout_ms == 0 || self.timeout_ms > 3_600_000 || self.steps.is_empty() {
            return Err("route needs start, start_tick, 1..3600000 timeout_ms, and nonempty steps".into())
        }
        let mut saved = std::collections::HashSet::new();
        for (i, s) in self.steps.iter().enumerate() {
            let mut check = || -> Result<(), String> {
                if [s.pick.is_some(), s.snapshot.is_some(), s.restore.is_some(), s.assertion.is_some(), s.replay.is_some()]
                    .iter().filter(|&&v| v).count() != 1 { return Err("exactly one of pick/snapshot/restore/assert/replay is required".into()) }
                if s.pick.as_ref().is_some_and(|v| !valid_pick(v)) { return Err("invalid pick".into()) }
                if s.replay == Some(false) { return Err("replay must be true".into()) }
                if let Some(n) = &s.snapshot { if !name(n) || !saved.insert(n.clone()) { return Err("invalid or repeated snapshot name".into()) } }
                if let Some(n) = &s.restore { if !saved.contains(n) { return Err("restore must name an earlier snapshot".into()) } }
                if let Some(k) = &s.expect_kind { if !["menu","yesno","list","text","number","ack","ground","space","combat.turn"].contains(&k.as_str()) { return Err("unknown expect_kind".into()) } }
                if let Some(a) = &s.assertion { validate_assert(a)?; if a["cell"].is_object() { return Err("cell.changed is only valid in a pick's after.state".into()) } }
                if let Some(a) = &s.after {
                    if !["menu","yesno","list","text","number","ack","ground","space","combat.turn"].contains(&a.kind.as_str()) { return Err("unknown after.kind".into()) }
                    if let Some(state) = &a.state { validate_assert(state)?; if state["cell"].is_object() && s.pick.is_none() { return Err("cell.changed requires a pick".into()) } }
                }
                if i + 1 == self.steps.len() && s.pick.is_some() && s.after.is_none() { return Err("last pick requires after.kind and optional after.state".into()) }
                Ok(())
            };
            check().map_err(|e| format!("step {}: {e}", i + 1))?;
        }
        Ok(())
    }
}
pub(super) fn read(path: &Path) -> Result<Vec<u8>, String> {
    let mut bytes = vec![];
    std::fs::File::open(path).map_err(|e| e.to_string())?.take((16 << 20) + 1).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.len() > 16 << 20 { return Err("route/trace exceeds 16 MiB".into()) } Ok(bytes)
}
fn validate_assert(a: &Value) -> Result<(), String> {
    let fields = a.as_object().filter(|o| !o.is_empty()).ok_or("assert must be a nonempty object")?;
    for (key, value) in fields {
        let valid = match key.as_str() {
            "system" | "planet" | "fuel" => value.as_u64().is_some_and(|n| n <= 255),
            "alive_mask" => value.as_u64().is_some_and(|n| n <= 31) || value.as_object().is_some_and(|o| o.len() == 1 && o.get("ne") == Some(&json!(0))),
            "cell" => value.as_array().is_some_and(|a| a.len() == 2 && a.iter().all(|v| v.as_i64().is_some())) || value == &json!({"changed":true}),
            "conscious" => value.as_u64().is_some_and(|n| n <= 5) || value.as_object().is_some_and(|o| o.len() == 1 && o.get("min").and_then(Value::as_u64).is_some_and(|n| n <= 5)),
            "alerted" => value.as_u64().is_some_and(|n| n <= 255),
            "ship_name" | "panel" => value.as_str().is_some_and(|s| !s.is_empty()),
            "clock_days" => value.as_u64().is_some_and(|n| n <= 65535) || value.as_object().is_some_and(|o| o.len() == 2 && o.get("since").and_then(Value::as_str).is_some_and(name) && o.get("delta").and_then(Value::as_u64).is_some_and(|n| n <= 65535)),
            "programs_loaded" | "programs_running" => value.as_array().is_some_and(|a| !a.is_empty() && a.iter().all(|v| v.as_u64().is_some_and(|n| n <= 15))),
            "in_space" => value.is_boolean(),
            "encounter" => value.is_null() || value.is_string(),
            _ => false,
        };
        if !valid { return Err(format!("unsupported assertion {key}: {value}")) }
    }
    Ok(())
}
#[cfg(test)]
fn assert_state(a: &Value, observed: &Observed) -> Result<(), String> {
    check_state(a, &state(observed, None)?, None, &std::collections::HashMap::new())
}
pub(super) fn state(observed: &Observed, decision: Option<&Decision>) -> Result<Value, String> {
    let o = observed.state.as_ref().ok_or("expected game state; seen no observation")?;
    if o.scene.party.0.len() <= 0xFC { return Err("expected game state; seen no party observation".into()) }
    let b = |at: usize| o.scene.party.0.get(at).copied().map(Value::from).unwrap_or(Value::Null);
    let party = &o.scene.party.0;
    let conscious = (0..5).filter(|i| party[0] & (1 << i) != 0 && party.get(0x127 + 256*i).copied().unwrap_or(0) > party.get(0x126 + 256*i).copied().unwrap_or(0)).count();
    let ship = party.get(0x8C..0xA0).map(|s| String::from_utf8_lossy(s.split(|b| *b == 0).next().unwrap_or_default()).trim_end().to_owned());
    let days = party.get(0xF84..0xF86).map(|s| u16::from_le_bytes([s[0],s[1]]));
    let programs = |flag| party.get(0xAC..0xC0).unwrap_or_default().iter().filter(|p| **p & flag != 0).map(|p| p & 15).collect::<Vec<_>>();
    Ok(json!({"system":b(6),"planet":b(7),"alive_mask":b(0),"fuel":b(0xFC),
        "cell":if o.flight.is_none() { json!([o.scene.pos.0 >> 1, o.scene.pos.1 >> 1]) } else { Value::Null },
        "in_space":o.flight.is_some(),
        "encounter":if let Some(id) = &o.encounter { json!(id) } else if o.scene.squad.encounter { json!("active") }
            else if let Some(f) = &o.flight { if f.encounter >= 0 { json!(f.encounter) } else { Value::Null } } else { Value::Null },
        "conscious":conscious,"alerted":o.scene.squad.alerted,"ship_name":ship,"clock_days":days,
        "panel":decision.map(|d| format!("{}:{}",d.context.program,d.context.routine)),
        "programs_loaded":programs(0x80),"programs_running":programs(0x20)}))
}
pub(super) fn check_state(a: &Value, state: &Value, before: Option<&Value>, named: &std::collections::HashMap<String, Value>) -> Result<(), String> {
    for (key, expected) in a.as_object().unwrap() {
        let seen = &state[key];
        let equal = match key.as_str() {
            "alive_mask" if expected.is_object() => seen.as_u64().is_some_and(|n| n != 0),
            "conscious" if expected.is_object() => seen.as_u64().is_some_and(|n| n >= expected["min"].as_u64().unwrap()),
            "cell" if expected.is_object() => before.is_some_and(|b| !b[key].is_null() && !seen.is_null() && b[key] != *seen),
            "clock_days" if expected.is_object() => named.get(expected["since"].as_str().unwrap()).and_then(|s| s[key].as_u64()).zip(seen.as_u64()).is_some_and(|(a,b)| (b as u16).wrapping_sub(a as u16) == expected["delta"].as_u64().unwrap() as u16),
            "programs_loaded" | "programs_running" => seen.as_array().is_some_and(|v| expected.as_array().unwrap().iter().all(|e| v.contains(e))),
            _ => seen == expected,
        };
        if !equal { return Err(format!("{key}: expected {expected}, seen {seen}")) }
    }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Record {
    pub decision: String, pub kind: String, pub pick: Value,
    pub before: String, pub after: Option<String>, pub label_hash: Option<u32>,
    pub requested: u32, pub used: u32, pub remaining: u32,
}
impl Record {
    pub fn from_entry(e: &mt_engine::play::Entry, kind: String) -> Self {
        Self { decision: e.choice.decision.clone(), kind, pick: pick_value(&e.choice.pick), before: e.before.clone(), after: e.after.clone(),
            label_hash: e.label_hash, requested: e.grant.requested, used: e.grant.used, remaining: e.grant.remaining }
    }
    pub fn entry(&self) -> Result<mt_engine::play::Entry, String> {
        if !valid_pick(&self.pick) { return Err("invalid trace pick".into()) }
        let pick = if self.kind == "text" && self.label_hash.is_none() {
            Pick::Text(self.pick.as_str().ok_or("trace text pick must be a string")?.into())
        } else if let Some(s) = self.pick.as_str() { Pick::Id(s.into()) }
        else if let Some(n) = self.pick.as_i64() { Pick::Number(n) }
        else { Pick::Row { row: self.pick["row"].as_str().unwrap().into(), qty: self.pick["qty"].as_u64().unwrap() as u32 } };
        Ok(mt_engine::play::Entry { choice: Choice::new(&self.decision, pick), before: self.before.clone(), after: self.after.clone(), label_hash: self.label_hash,
            grant: mt_engine::play::Grant { requested: self.requested, used: self.used, remaining: self.remaining } })
    }
}
pub(super) fn root_snapshot(root: &Root, records: &[Record], decision: &str, hash: &str) -> Result<Snapshot, String> {
    let snapshot = Snapshot { id: "replay".into(), name: "Replay".into(), created: String::new(), build: env!("MT_BUILD_HASH").into(),
        root: root.clone(), journal: Journal { hash_version: mt_engine::play::HASH_VERSION, entries: records.iter().map(Record::entry).collect::<Result<_, _>>()? },
        sequence: decision.strip_prefix("d:").ok_or("invalid trace decision")?.parse().map_err(|_| "invalid trace sequence")?, hash: hash.into() };
    snapshot.validate()?; Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn assertions_distinguish_conscious_alerted_panel_clock_and_program_phases() {
        let mut o = Observation::program(mt1::ui::ScreenKind::Ground,1234);
        o.scene.party.0=vec![0;0xF86];
        let b=&mut o.scene.party.0; b[0]=31;
        b[0x127]=10;b[0x126]=10;b[0x227]=11;b[0x226]=10;b[0x327]=0;
        b[0x8C..0x96].copy_from_slice(b"Interloper");
        b[0xF84..0xF86].copy_from_slice(&4u16.to_le_bytes());
        b[0xAC]=0x43; b[0xAD]=0x84;
        o.scene.squad.alerted=2;
        let mut observed=Observed {state:Some(o),..Default::default()};
        let decision=Decision::new(Kind::Menu,"t","13FF2");
        let base=state(&observed,Some(&decision)).unwrap();
        let named=std::collections::HashMap::from([("BeforeJump".into(),json!({"clock_days":65533}))]);
        let a=json!({"conscious":1,"alerted":2,"ship_name":"Interloper","panel":"t:13FF2","clock_days":{"since":"BeforeJump","delta":7},"programs_loaded":[4]});
        validate_assert(&a).unwrap();check_state(&a,&base,None,&named).unwrap();
        for (key,wrong) in [("conscious",json!(2)),("alerted",json!(0)),("ship_name",json!("wrong")),("panel",json!("t:other")),
            ("clock_days",json!({"since":"BeforeJump","delta":6})),("programs_loaded",json!([3])),("programs_running",json!([4]))] {
            assert!(check_state(&json!({key:wrong}),&base,None,&named).unwrap_err().contains(key));
        }
        let b=&mut observed.state.as_mut().unwrap().scene.party.0;b[0xAC]=0xA3;b[0xAD]=0xA4;
        let next=state(&observed,Some(&decision)).unwrap();
        check_state(&json!({"programs_loaded":[3,4],"programs_running":[3,4],"conscious":{"min":1}}),&next,None,&named).unwrap();
        let mut next=next;next["cell"]=json!([1,0]);
        check_state(&json!({"cell":{"changed":true}}),&next,Some(&base),&named).unwrap();
        assert!(check_state(&json!({"cell":{"changed":true}}),&base,Some(&base),&named).is_err());
    }
    #[test]
    fn assertions_cover_ground_space_and_reject_each_wrong_field() {
        let mut o = Observation::program(mt1::ui::ScreenKind::Ground, 1234);
        o.scene.party.0 = vec![0; 0xF86]; o.scene.party.0[0] = 31; o.scene.party.0[6] = 2; o.scene.party.0[0xFC] = 125;
        o.scene.pos = (66, 6);
        let mut observed = Observed { state: Some(o), ..Default::default() };
        let expected = json!({"system":2,"planet":0,"alive_mask":31,"fuel":125,"cell":[33,3],"in_space":false,"encounter":null});
        validate_assert(&expected).unwrap(); assert_state(&expected, &observed).unwrap();
        for (key, wrong) in [("system",json!(3)),("planet",json!(255)),("alive_mask",json!(0)),("fuel",json!(61)),
            ("cell",json!([3,33])),("in_space",json!(true)),("encounter",json!("ambush"))] {
            assert!(assert_state(&json!({key:wrong}), &observed).unwrap_err().contains(key));
        }
        assert_state(&json!({"alive_mask":{"ne":0}}), &observed).unwrap();
        let o = observed.state.as_mut().unwrap(); o.scene.party.0[0] = 0;
        o.scene.party.0[6] = 3; o.scene.party.0[7] = 255; o.scene.party.0[0xFC] = 61;
        o.flight = Some(mt1::play::observation::Flight { position:(0,0),velocity:(0,0),heading:0,thrust:0,station:0,encounter:-1 });
        assert_state(&json!({"system":3,"planet":255,"fuel":61,"in_space":true,"encounter":null}), &observed).unwrap();
        assert!(assert_state(&json!({"alive_mask":{"ne":0}}), &observed).is_err());
        observed.state.as_mut().unwrap().flight.as_mut().unwrap().encounter = 1;
        assert!(assert_state(&json!({"encounter":null}), &observed).is_err());
        assert!(assert_state(&json!({"in_space":false}), &Observed::default()).is_err());
    }
    #[test]
    fn route_rejects_ambiguous_actions_typos_and_forward_restores() {
        for step in [json!({"pick":"continue","snapshot":"S1"}),json!({"restore":"later"}),json!({"snapshot":"../escape"}),
            json!({"assert":{"typo":2}}),json!({"assert":{}}),json!({"replay":false}),json!({"pick":{"row":"x","qty":-1}})] {
            let r: Route = serde_json::from_value(json!({"start":"quickstart","start_tick":1234,"steps":[step]})).unwrap();
            assert!(r.validate().is_err());
        }
        assert!(serde_json::from_value::<Route>(json!({"start":"quickstart","start_tick":1234,"steps":[{"pick":"continue","expect_knd":"ack"}]})).is_err());
    }
}
