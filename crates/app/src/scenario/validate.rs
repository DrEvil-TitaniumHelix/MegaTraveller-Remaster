use serde_json::Value;

pub fn document(text: &str) -> Result<Value, String> {
    let t: toml::Value = toml::from_str(text).map_err(|e| format!("TOML: {}", e.message()))?;
    let v = serde_json::to_value(t).map_err(|e| e.to_string())?;
    save_rule(&v)?;
    let s: Value = serde_json::from_str(include_str!("../../../../schema/scenario.schema.json")).map_err(|e| e.to_string())?;
    check(&s, &s, &v, "")?;
    if (v.get("npc").is_some() || v.get("encounter").is_some()) && v["meta"]["format"].as_f64().is_none_or(|n|n<1.1) {
        return Err("meta/format: npc and encounter require 1.1 or later".into())
    }
    if v["meta"]["format"].as_f64().is_none_or(|n|n<1.4) {
        let mut needs: Vec<String>=["squad","ground"].iter().filter(|k|v.get(**k).is_some()).map(|k|k.to_string()).collect();
        if v["place"].get("starport_area").is_some() {needs.push("place.starport_area".into())}
        for m in super::array(&v["party"],"members") {for k in ["attr","grenades"] {if m.get(k).is_some() {needs.push(format!("party.members.{k}"))}}}
        for gi in super::array(&v["party"],"items") {if super::array(gi,"pack").any(|it|it.get("cell").is_some()) {needs.push("party.items.pack.cell".into())}}
        for e in super::array(&v,"encounter") {if e["allow_save"]==true {needs.push(format!("encounter {} allow_save = true",super::string(e,"id","")))}}
        if let Some(n)=needs.first() {return Err(format!("meta/format: {n} requires 1.4"))}
    }
    if v["party"].get("damage").is_some() && v["meta"]["format"].as_f64().is_none_or(|n|n<1.5) { return Err("meta/format: party.damage requires 1.5".into()) }
    super::space::validate(&v)?;
    Ok(v)
}

pub fn save_rule(v: &Value) -> Result<(), String> {
    let Some(o) = v.as_object() else { return Ok(()) };
    if !o.contains_key("save") { return if o.contains_key("place") { Ok(()) } else { Err("place: required field (or a [save] section)".into()) } }
    if v["meta"]["format"].as_f64().is_none_or(|n| n < 1.3) { return Err("meta/format: save requires 1.3".into()) }
    if let Some(extra) = o.keys().find(|k| !matches!(k.as_str(), "meta" | "save")) { return Err(format!("{extra}: a [save] scenario is the save; it takes only [meta] and [save]")) }
    if v["meta"].get("base").is_some() { return Err("meta/base: a [save] scenario starts from the save; base is not allowed".into()) }
    if v["save"]["file"].as_str().is_some_and(|f| !confined(f)) { return Err("save/file: must be a relative path inside the scenario folder (no absolute path, no '..', a .mtsav)".into()) }
    Ok(())
}

fn confined(t: &str) -> bool {
    let b = t.as_bytes();
    !t.starts_with(['/', '\\']) && !(b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':')
        && !t.split(['/', '\\']).any(|s| s == "..") && t.len() > 6 && t.ends_with(".mtsav")
}

fn pattern(p: &str, text: &str) -> bool {
    match p {
        "^[0-9]+(\\.[0-9]+)*$" => text.split('.').all(|s| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit())),
        "^[a-z][a-z0-9_]*$" => text.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
            && text.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'_'),
        "^([0-9]|[1-6][0-9]|70)$" => text.parse::<usize>().is_ok_and(|i| i <= 70 && i.to_string() == text),
        r"^(?![\\/])(?![A-Za-z]:)(?!.*(^|[\\/])\.\.([\\/]|$)).+\.mtsav$" => confined(text),
        "^[0-9]{1,2}$" => (1..=2).contains(&text.len()) && text.bytes().all(|c|c.is_ascii_digit()),
        "^[0-9]{1,3}$" => (1..=3).contains(&text.len()) && text.bytes().all(|c|c.is_ascii_digit()),
        "^[0-9A-HJ]{6}$" => text.len()==6 && text.bytes().all(|c|c.is_ascii_digit() || (b'A'..=b'H').contains(&c) || c==b'J'),
        "^(Ap|As|A[0-9]+) (Fs|Am|F[0-9]+)$" => text.split_once(' ').is_some_and(|(a,b)| {
            let numbered=|s:&str,prefix:char|s.strip_prefix(prefix).is_some_and(|n|!n.is_empty() && n.bytes().all(|c|c.is_ascii_digit()));
            (a=="Ap" || a=="As" || numbered(a,'A')) && (b=="Fs" || b=="Am" || numbered(b,'F'))
        }),
        _ => false,
    }
}

pub fn check(root: &Value, s: &Value, v: &Value, path: &str) -> Result<(), String> {
    let fail = |why: &str| Err(format!("{}: {why}", if path.is_empty() {"(top)"} else {path}));
    if let Some(r) = s["$ref"].as_str() {
        let target = root.pointer(r.strip_prefix('#').ok_or_else(|| format!("{path}: external schema reference"))?)
            .ok_or_else(|| format!("{path}: missing schema reference"))?;
        check(root,target,v,path)?;
    }
    for rule in s["allOf"].as_array().into_iter().flatten() { check(root,rule,v,path)?; }
    if let Some(a) = s["oneOf"].as_array() {
        if a.iter().filter(|s| check(root,s,v,path).is_ok()).count() != 1 { return fail("value does not match an allowed type") }
    }
    if let Some(c) = s.get("const") { if c != v { return fail(&format!("expected {c}")) } }
    if let Some(a) = s["enum"].as_array() { if !a.contains(v) { return fail(&format!("expected one of {}",s["enum"])) } }
    if let Some(t) = s["type"].as_str() {
        if !match t {"object"=>v.is_object(),"array"=>v.is_array(),"string"=>v.is_string(),
            "integer"=>v.as_i64().is_some(),"boolean"=>v.is_boolean(),_=>false} { return fail(&format!("expected {t}")) }
    }
    if let Some(n) = v.as_i64() {
        if s["minimum"].as_i64().is_some_and(|min|n<min) || s["maximum"].as_i64().is_some_and(|max|n>max) {
            return fail("integer outside permitted range")
        }
    }
    if let Some(t) = v.as_str() {
        if s["minLength"].as_u64().is_some_and(|n|t.chars().count() < n as usize)
            || s["maxLength"].as_u64().is_some_and(|n|t.chars().count() > n as usize) { return fail("string length outside permitted range") }
        if s["pattern"].as_str().is_some_and(|p| !pattern(p,t)) { return fail("string does not match field pattern") }
    }
    if let Some(a) = v.as_array() {
        if s["minItems"].as_u64().is_some_and(|n|a.len()<n as usize) || s["maxItems"].as_u64().is_some_and(|n|a.len()>n as usize) { return fail("array length outside permitted range") }
        for (i,x) in a.iter().enumerate() {
            if let Some(item) = s["prefixItems"].get(i).or_else(|| s.get("items")) { check(root,item,x,&format!("{path}/{i}"))? }
        }
    }
    if let Some(o) = v.as_object() {
        if s["maxProperties"].as_u64().is_some_and(|n|o.len()>n as usize) { return fail("section must be empty") }
        for name in s["required"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            if !o.contains_key(name) { return Err(format!("{path}/{name}: required field")) }
        }
        for (name,x) in o {
            let p = if path.is_empty() {name.clone()} else {format!("{path}/{name}")};
            if let Some(rule) = s.get("propertyNames") { check(root,rule,&Value::String(name.clone()),&p)? }
            if let Some(rule) = s["properties"].get(name) { check(root,rule,x,&p)? }
            else if s["additionalProperties"] == false { return Err(format!("{p}: unknown field")) }
            else if s["additionalProperties"].is_object() { check(root,&s["additionalProperties"],x,&p)? }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_rejects_unknown_fields_and_unsafe_types() {
        let good=include_str!("../../../../scenarios/m4b_medkit_in_pack.toml");
        assert!(document(good).is_ok());
        assert!(document(&good.replace("[meta]", "[meta]\ncolour = 'red'")).unwrap_err().contains("meta/colour"));
        for (from,to) in [("world = 2","world = -1"),("hits = 8","hits = 300"),("format = 1","format = 2")] {
            assert!(document(&good.replace(from,to)).is_err(),"{from}");
        }
        assert!(document(&format!("{good}\n[ship]\nx = 1")).is_err());
    }
    #[test]
    fn save_section_is_the_whole_scenario() {
        let save = "[meta]\nformat = 1.3\nname = \"s\"\nversion = \"1\"\nauthor = \"a\"\npurpose = \"test\"\n[save]\nfile = \"saves/x.mtsav\"\n";
        assert_eq!(document(save).unwrap()["save"]["file"], "saves/x.mtsav");
        assert!(document(&save.replace("1.3", "1.2")).unwrap_err().contains("meta/format"));
        assert!(document(&format!("{save}[place]\nworld = 2\narea = 0\nlevel = 0\ncell = [1, 1]\n")).unwrap_err().starts_with("place: a [save]"));
        assert!(document(&format!("{save}slot = 1\n")).unwrap_err().contains("save/slot"));
        assert!(document(&save.replace("[save]\nfile = \"saves/x.mtsav\"\n", "")).unwrap_err().starts_with("place: required"));
        assert!(document(&save.replace("file = \"saves/x.mtsav\"", "")).unwrap_err().contains("save/file"));
        for bad in ["C:/x.mtsav", "/x.mtsav", "\\\\x.mtsav", "../x.mtsav", "saves/../../x.mtsav", "..", "x.sav", ".mtsav"] {
            assert!(document(&save.replace("saves/x.mtsav", bad)).unwrap_err().starts_with("save/file: must be"), "{bad}");
        }
        let schema: Value = serde_json::from_str(include_str!("../../../../schema/scenario.schema.json")).unwrap();
        let p = schema["properties"]["save"]["properties"]["file"]["pattern"].as_str().unwrap();
        assert!(pattern(p, "saves/x.mtsav") && pattern(p, "a\\b.mtsav") && !pattern(p, "../x.mtsav") && !pattern(p, "d:\\x.mtsav"));
        assert!(document(&save.replace("purpose = \"test\"", "purpose = \"test\"\nbase = \"quickstart\"")).unwrap_err().starts_with("meta/base"));
    }
    #[test]
    fn encounter_file_checks_both_definition_and_required_location() {
        let schema:Value=serde_json::from_str(include_str!("../../../../schema/scenario.schema.json")).unwrap();
        let text=include_str!("../../../../data/encounters.toml");
        let doc:toml::Value=toml::from_str(text).unwrap();
        let good=serde_json::to_value(doc).unwrap();
        let check_file=|v:&Value|check(&schema,&schema["$defs"]["encounter_file"],v,"data/encounters.toml");
        assert!(check_file(&good).is_ok());
        let mut bad=good.clone();bad["encounter"][0]["colour"]="red".into();
        assert!(check_file(&bad).unwrap_err().contains("encounter/0/colour"));
        let mut bad=good.clone();bad["encounter"][0].as_object_mut().unwrap().remove("where");
        assert!(check_file(&bad).unwrap_err().contains("encounter/0/where"));
        let mut bad=good;bad["encounter"][0]["where"]["world"]="invalid".into();
        assert!(check_file(&bad).unwrap_err().contains("encounter/0/where/world"));
    }
}
