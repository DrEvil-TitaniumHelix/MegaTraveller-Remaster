use super::weapons::{armour, armour_named, ArmourValues};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Value { Str(String), Int(i64), Bool(bool), Arr(Vec<Value>), Table(BTreeMap<String, Value>) }
impl Value {
    pub fn get(&self, key: &str) -> Option<&Value> { match self { Value::Table(t) => t.get(key), _ => None } }
    pub fn str(&self) -> Option<&str> { match self { Value::Str(s) => Some(s), _ => None } }
    pub fn int(&self) -> Option<i64> { match self { Value::Int(i) => Some(*i), _ => None } }
    pub fn arr(&self) -> Option<&[Value]> { match self { Value::Arr(a) => Some(a), _ => None } }
    pub fn table(&self) -> Option<&BTreeMap<String, Value>> { match self { Value::Table(t) => Some(t), _ => None } }
}
fn strip(line: &str) -> (String, i32) {
    let (mut out, mut depth, mut quoted) = (String::new(), 0i32, false);
    let mut chars = line.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => { quoted = !quoted; out.push(c) }
            '\\' if quoted => { out.push(c); if let Some(n) = chars.next() { out.push(n) } }
            '#' if !quoted => break,
            '[' | '{' if !quoted => { depth += 1; out.push(c) }
            ']' | '}' if !quoted => { depth -= 1; out.push(c) }
            _ => out.push(c),
        }
    }
    (out, depth)
}
struct P<'a> { s: &'a [u8], at: usize }
impl<'a> P<'a> {
    fn ws(&mut self) { while self.at < self.s.len() && (self.s[self.at] as char).is_whitespace() { self.at += 1 } }
    fn peek(&self) -> Option<u8> { self.s.get(self.at).copied() }
    fn eat(&mut self, c: u8) -> Result<(), String> { self.ws(); if self.peek() == Some(c) { self.at += 1; Ok(()) } else { Err(format!("expected '{}' at {}", c as char, self.at)) } }
    fn key(&mut self) -> Result<String, String> {
        self.ws();
        if self.peek() == Some(b'"') { return self.string() }
        let start = self.at;
        while self.at < self.s.len() && (self.s[self.at].is_ascii_alphanumeric() || self.s[self.at] == b'_' || self.s[self.at] == b'-') { self.at += 1 }
        if start == self.at { return Err(format!("expected a key at {}", self.at)) }
        Ok(String::from_utf8_lossy(&self.s[start..self.at]).into_owned())
    }
    fn string(&mut self) -> Result<String, String> {
        self.eat(b'"')?;
        let mut out = String::new();
        loop {
            match self.peek() {
                None => return Err("unterminated string".into()),
                Some(b'"') => { self.at += 1; return Ok(out) }
                Some(b'\\') => { self.at += 1; let c = self.peek().ok_or("bad escape")?; self.at += 1; out.push(match c { b'n' => '\n', b't' => '\t', other => other as char }) }
                Some(_) => { let rest = &self.s[self.at..]; let n = rest.iter().position(|&b| b == b'"' || b == b'\\').unwrap_or(rest.len()); out.push_str(&String::from_utf8_lossy(&rest[..n])); self.at += n }
            }
        }
    }
    fn value(&mut self) -> Result<Value, String> {
        self.ws();
        match self.peek().ok_or("expected a value")? {
            b'"' => Ok(Value::Str(self.string()?)),
            b'[' => {
                self.at += 1; let mut items = Vec::new();
                loop { self.ws(); if self.peek() == Some(b']') { self.at += 1; return Ok(Value::Arr(items)) } items.push(self.value()?); self.ws(); if self.peek() == Some(b',') { self.at += 1 } }
            }
            b'{' => {
                self.at += 1; let mut t = BTreeMap::new();
                loop { self.ws(); if self.peek() == Some(b'}') { self.at += 1; return Ok(Value::Table(t)) } let k = self.key()?; self.eat(b'=')?; let v = self.value()?; t.insert(k, v); self.ws(); if self.peek() == Some(b',') { self.at += 1 } }
            }
            b't' | b'f' => { let rest = &self.s[self.at..]; for (w, b) in [("true", true), ("false", false)] { if rest.starts_with(w.as_bytes()) { self.at += w.len(); return Ok(Value::Bool(b)) } } Err("bad literal".into()) }
            _ => { let start = self.at; while self.at < self.s.len() && (self.s[self.at].is_ascii_digit() || self.s[self.at] == b'-' || self.s[self.at] == b'_') { self.at += 1 }
                String::from_utf8_lossy(&self.s[start..self.at]).replace('_', "").parse().map(Value::Int).map_err(|_| format!("bad number at {start}")) }
        }
    }
}
fn node<'a>(root: &'a mut BTreeMap<String, Value>, path: &[String]) -> Result<&'a mut BTreeMap<String, Value>, String> {
    let mut t = root;
    for seg in path {
        let v = t.entry(seg.clone()).or_insert_with(|| Value::Table(BTreeMap::new()));
        t = match v { Value::Table(m) => m, Value::Arr(a) => match a.last_mut() { Some(Value::Table(m)) => m, _ => return Err(format!("{seg}: not a table")) }, _ => return Err(format!("{seg}: not a table")) };
    }
    Ok(t)
}
pub fn parse_toml(text: &str) -> Result<BTreeMap<String, Value>, String> {
    let (mut root, mut current): (BTreeMap<String, Value>, Vec<String>) = (BTreeMap::new(), Vec::new());
    let (mut pending, mut depth) = (String::new(), 0i32);
    for (n, raw) in text.lines().enumerate() {
        let (line, d) = strip(raw);
        pending.push_str(&line); pending.push(' '); depth += d;
        if depth > 0 { continue }
        let line = std::mem::take(&mut pending); depth = 0;
        let line = line.trim();
        if line.is_empty() { continue }
        let err = |e: String| format!("npcs.toml line {}: {e}", n + 1);
        if let Some(h) = line.strip_prefix("[[").and_then(|l| l.strip_suffix("]]")) {
            let path: Vec<String> = h.split('.').map(|s| s.trim().to_string()).collect();
            let (parent, last) = path.split_at(path.len() - 1);
            let t = node(&mut root, parent).map_err(err)?;
            match t.entry(last[0].clone()).or_insert_with(|| Value::Arr(Vec::new())) { Value::Arr(a) => a.push(Value::Table(BTreeMap::new())), _ => return Err(err(format!("{} is not an array", last[0]))) }
            current = path;
        } else if let Some(h) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            current = h.split('.').map(|s| s.trim().to_string()).collect();
            node(&mut root, &current).map_err(err)?;
        } else {
            let mut p = P { s: line.as_bytes(), at: 0 };
            let k = p.key().map_err(err)?; p.eat(b'=').map_err(err)?; let v = p.value().map_err(err)?;
            node(&mut root, &current).map_err(err)?.insert(k, v);
        }
    }
    Ok(root)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Morale { VeryLow, Low, #[default] Average, High }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub id: String, pub label: String, pub animal: bool, pub upp: [i32; 6], pub hits: (i32, i32), pub skills: BTreeMap<String, i32>,
    pub tactics: i32, pub leader: i32, pub recon: i32, pub morale: Morale, pub weapon: Option<u8>, pub armour: Option<ArmourValues>,
    pub from_record: bool, pub source: String, pub animal_attack: Option<(i32, i32)>,
}
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Compendium { pub version: String, pub weapon_skill: BTreeMap<u8, String>, pub roles: BTreeMap<String, Entry>, pub picture: BTreeMap<u8, String>, pub records: BTreeMap<(u8, u8, u8), String> }
fn upp(s: &str) -> [i32; 6] {
    let mut out = [7; 6];
    for (i, c) in s.chars().take(6).enumerate() { out[i] = c.to_digit(36).map_or(7, |d| d as i32) }
    out
}
impl Compendium {
    pub fn parse(text: &str) -> Result<Compendium, String> {
        let root = parse_toml(text)?;
        let mut c = Compendium { version: root.get("meta").and_then(|m| m.get("version")).and_then(Value::str).unwrap_or("").into(), ..Default::default() };
        let alias: BTreeMap<String, String> = root.get("skill_alias").and_then(Value::table).map(|t| t.iter().filter_map(|(k, v)| Some((k.clone(), v.str()?.to_string()))).collect()).unwrap_or_default();
        for (k, v) in root.get("weapon_skill").and_then(Value::table).into_iter().flatten() { if let (Ok(i), Some(s)) = (k.parse::<u8>(), v.str()) { c.weapon_skill.insert(i, s.into()); } }
        for (id, e) in root.get("role").and_then(Value::table).into_iter().flatten() {
            let animal = e.get("kind").and_then(Value::str) == Some("animal");
            let skills: BTreeMap<String, i32> = e.get("skills").and_then(Value::table).map(|t| t.iter().filter_map(|(k, v)| Some((alias.get(k).cloned().unwrap_or_else(|| k.clone()), v.int()? as i32))).collect()).unwrap_or_default();
            let hits = e.get("hits").and_then(Value::arr).map(|a| (a.first().and_then(Value::int).unwrap_or(0) as i32, a.get(1).and_then(Value::int).unwrap_or(0) as i32)).unwrap_or((0, 0));
            let eq = e.get("equipment");
            let from_record = eq.and_then(|q| q.get("from_record")).map_or(false, |v| v == &Value::Bool(true));
            let armour_values = if animal {
                e.get("armor").and_then(Value::str).map(|n| armour_named(n).map(ArmourValues::of).unwrap_or(ArmourValues::uniform(0, true)))
            } else { eq.and_then(|q| q.get("armor")).and_then(Value::int).and_then(|i| armour(i as u8)).map(ArmourValues::of) };
            let animal_attack = animal.then(|| {
                let skill = e.get("weapons").and_then(Value::str).and_then(|w| w.rsplit_once('+')).and_then(|(_, n)| n.trim().parse().ok()).unwrap_or(0);
                (skill, e.get("weapon_hits").and_then(Value::int).unwrap_or(1) as i32)
            });
            let morale = match e.get("morale").and_then(Value::str) { Some("very_low") => Morale::VeryLow, Some("low") => Morale::Low, Some("high") => Morale::High, _ => Morale::Average };
            let entry = Entry {
                id: id.clone(), label: e.get("label").and_then(Value::str).unwrap_or(id).into(), animal,
                upp: e.get("upp").and_then(Value::str).map_or([7; 6], upp), hits,
                tactics: e.get("tactics_pool_contribution").and_then(Value::int).map_or_else(|| skills.get("Tactics").copied().unwrap_or(0), |t| t as i32),
                leader: skills.get("Leader").copied().unwrap_or(0), recon: skills.get("Recon").copied().unwrap_or(0), skills, morale,
                weapon: eq.and_then(|q| q.get("weapon")).and_then(Value::int).map(|i| i as u8), armour: armour_values, from_record,
                source: e.get("source").and_then(Value::str).unwrap_or("").into(), animal_attack,
            };
            c.roles.insert(id.clone(), entry);
        }
        for (k, v) in root.get("picture").and_then(Value::table).into_iter().flatten() { if let (Ok(p), Some(r)) = (k.parse::<u8>(), v.get("role").and_then(Value::str)) { c.picture.insert(p, r.into()); } }
        for r in root.get("record").and_then(Value::arr).into_iter().flatten() {
            if let (Some(w), Some(a), Some(k), Some(role)) = (r.get("world").and_then(Value::int), r.get("area").and_then(Value::int), r.get("record").and_then(Value::int), r.get("role").and_then(Value::str)) {
                c.records.insert((w as u8, a as u8, k as u8), role.into());
            }
        }
        Ok(c)
    }
    pub fn role_of(&self, world: u8, area: u8, record: u8, picture: u8) -> Option<&str> {
        self.records.get(&(world, area, record)).or_else(|| self.picture.get(&picture)).map(String::as_str)
    }
    pub fn entry_of(&self, world: u8, area: u8, record: u8, picture: u8) -> Option<&Entry> {
        self.roles.get(self.role_of(world, area, record, picture)?)
    }
    pub fn skill_name(&self, item_skill: u8) -> Option<&str> { self.weapon_skill.get(&item_skill).map(String::as_str) }
}
pub const GUN_COMBAT: [&str; 8] = ["Handgun", "Rifleman", "Combat Rifleman", "Submachinegun", "Laser Weapons", "Energy Weapons", "Heavy Weapons", "Neural Weapons"];

#[cfg(test)]
pub const NPCS_TOML: &str = include_str!("../../../../../data/npcs.toml");
#[cfg(test)]
mod t {
mod oracle_env { include!("../../../tests/oracle_env/mod.rs"); }
    use super::*;
    #[test]
    fn the_subset_parser_reads_what_the_tool_writes() {
        let t = parse_toml("# c\n[a]\nx = 1 # c\n\"q k\" = \"s\\\"t # not\"\n[a.b]\narr = [1, [2, 3],\n  \"z\"]\nt = { k = -4, \"s\" = true, n = { } }\n[[r]]\nv = 1\n[[r]]\nv = 2\n[r.sub]\nw = false\n").unwrap();
        let a = &t["a"];
        assert_eq!((a.get("x"), a.get("q k").and_then(Value::str)), (Some(&Value::Int(1)), Some("s\"t # not")));
        assert_eq!(a.get("b").unwrap().get("arr"), Some(&Value::Arr(vec![Value::Int(1), Value::Arr(vec![Value::Int(2), Value::Int(3)]), Value::Str("z".into())])));
        let it = a.get("b").unwrap().get("t").unwrap(); assert_eq!((it.get("k"), it.get("s"), it.get("n").map(|n| n.table().unwrap().len())), (Some(&Value::Int(-4)), Some(&Value::Bool(true)), Some(0)));
        let r = t["r"].arr().unwrap(); assert_eq!((r.len(), r[1].get("v"), r[1].get("sub").unwrap().get("w")), (2, Some(&Value::Int(2)), Some(&Value::Bool(false))));
        assert!(parse_toml("x = ").is_err()); assert!(parse_toml("[a\n").is_err() || parse_toml("[a\n").is_ok()); assert!(parse_toml("x = \"open").is_err());
    }
    #[test]
    fn the_shipped_compendium_resolves_as_npcs_md_says() {
        let c = Compendium::parse(NPCS_TOML).unwrap();
        assert_eq!((c.roles.len(), c.picture.len(), c.records.len(), c.weapon_skill.len()), (25, 56, 11, 13));
        let z = &c.roles["zhodani_trooper"];
        assert_eq!((z.upp, z.hits, z.tactics, z.leader, z.recon, z.skills["Combat Rifleman"], z.weapon, z.armour.map(|a| a.bullets), z.morale, z.animal), ([7, 7, 3, 6, 7, 7], (3, 3), 1, 0, 0, 3, Some(23), Some(10), Morale::High, false));
        let a = &c.roles["assassin_amrhein_alazar"];
        assert_eq!((a.from_record, a.skills.get("Combat Rifleman"), a.skills.get("Assault Rifle"), a.armour), (true, Some(&4), None, None), "printed 'Assault Rifle' aliases to Combat Rifleman; equipment from the record");
        let h = &c.roles["animal_horned"];
        assert_eq!((h.animal, h.hits, h.animal_attack, h.armour, h.tactics), (true, (4, 3), Some((1, 2)), Some(ArmourValues::uniform(0, true)), 0));
        assert_eq!((c.skill_name(28), c.skill_name(4), c.skill_name(99)), (Some("Handgun"), Some("Combat Rifleman"), None));
        assert_eq!((c.role_of(2, 0, 14, 41), c.role_of(2, 0, 31, 0), c.role_of(2, 0, 32, 25), c.role_of(1, 1, 5, 25), c.role_of(1, 1, 5, 200)), (Some("thug"), Some("zhodani_officer"), Some("assassin_szen_sanai"), Some("assassin"), None));
        assert!(c.entry_of(2, 0, 14, 41).is_some() && c.entry_of(1, 1, 5, 25).is_none() && c.entry_of(1, 1, 5, 200).is_none());
        assert_eq!(c.roles["civilian"].skills.len(), 0); assert_eq!(c.roles["police"].upp[1], 11, "Traveller hex B = 11");
    }
    #[test]
    fn the_32_efate_records_map() {
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes = std::fs::read(dir.join("W.DAT")).expect("W.DAT");
        let chunks = mt_engine::data::world::decode(mt_engine::data::Variant::Mt1, &bytes).expect("W.DAT decodes");
        let Some(mt_engine::data::world::Chunk::Area(efate)) = chunks.get(7) else { panic!("chunk 7 is not Efate area 0") };
        let c = Compendium::parse(NPCS_TOML).unwrap();
        assert_eq!(efate.objects.len(), 32);
        let (mut entries, mut invulnerable, mut derived) = (0, 0, Vec::new());
        for (i, o) in efate.objects.iter().enumerate() {
            let k = i as u8 + 1;
            if o.fa == 0xFF { invulnerable += 1; continue }
            match c.entry_of(2, 0, k, o.f9) { Some(_) => entries += 1, None => derived.push((k, o.f9)) }
        }
        assert_eq!((entries + invulnerable, derived.as_slice()), (32, &[][..]), "every Efate record has an entry (npc_generate.py --check)");
        assert_eq!([14, 15, 16].map(|k| c.role_of(2, 0, k, efate.objects[k as usize - 1].f9)), [Some("thug"); 3], "the quick-start ambush: picture 41 thugs");
        assert_eq!((c.role_of(2, 0, 31, efate.objects[30].f9), c.role_of(2, 0, 32, efate.objects[31].f9)), (Some("zhodani_officer"), Some("assassin_szen_sanai")));
        assert_eq!((efate.objects[13].f1c[3], efate.objects[14].f1c[3], efate.objects[15].f1c[3]), (14, 4, 20), "ACR, auto pistol, TL9 laser rifle (ASBUILT 5)");
    }
}
