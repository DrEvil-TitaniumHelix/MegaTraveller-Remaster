use super::{array, num, string, assets::Assets, pokes::Pokes, Extensions};
use mt1::ground::{Ground, commands_party, round::{self, Boundary, Def, Encounter, End, NpcSt, Rect, Side, Start, Surprise, Turn}, rules::compendium::Compendium, squad, walk};
use serde_json::{Value, json};
use std::collections::BTreeMap;

fn rect(v: &Value) -> Option<Rect> { Some(Rect { r0: v[0][0].as_i64()? as i16, c0: v[0][1].as_i64()? as i16, r1: v[1][0].as_i64()? as i16, c1: v[1][1].as_i64()? as i16 }) }
pub fn defs_of(defs: &[Value], sheets: &BTreeMap<usize, Value>) -> Vec<Def> {
    defs.iter().map(|e| {
        let start = if e["start"] == "scripted" {
            match (string(&e["trigger"], "kind", ""), rect(&e["trigger"]["rect"])) { ("area_entry", Some(r)) => Start::Area(r), (kind, _) => Start::Unsupported(kind.into()) }
        } else { Start::Sight };
        let end = if e["end"] == "locked" { End::Locked { escape: e.get("escape_zone").and_then(rect) } } else { End::Open { rounds: num(e, "n_rounds", round::ENCOUNTER_END_ROUNDS as i64) as u16 } };
        let boundary = match string(e, "boundary", "radius") { "level" => Boundary::Level, "area" => rect(&e["rect"]).map_or(Boundary::Level, Boundary::Area), "screen" => Boundary::Screen, _ => Boundary::Radius(num(e, "radius", round::ENCOUNTER_RADIUS as i64) as i32) };
        let participants = e["participants"].as_array().map(|a| a.iter().filter_map(|x| x.as_u64().map(|n| n as u8)
            .or_else(|| sheets.iter().find(|(_, s)| s["id"] == *x).map(|(&k, _)| k as u8))).collect());
        let surprise = match string(e, "surprise", "roll") { "attackers" => Surprise::Attackers, "defenders" => Surprise::Defenders, "none" => Surprise::None, _ => Surprise::Roll };
        Def { id: string(e, "id", "").into(), start, end, boundary, participants, auto_break: e["auto_break"].as_bool().unwrap_or(true), allow_save: e["allow_save"].as_bool().unwrap_or(false),
            surprise, attacker: if e["attacker"] == "party" { Side::Party } else { Side::Npcs }, alerted_rounds: num(e, "alerted_rounds", round::ALERT_ROUNDS as i64) as u16 }
    }).collect()
}
pub fn install_defs(g: &mut Ground, defs: &[Value], sheets: &BTreeMap<usize, Value>) {
    if let Some(s) = g.squad.as_mut() { s.p1.defs = defs_of(defs, sheets) }
}
fn direction(g: &Ground, dr: i64, dc: i64) -> Option<u8> { (0..9u16).find(|&d| g.mem.i8(0x2f5 + d * 2) as i64 == dr && g.mem.i8(0x2f6 + d * 2) as i64 == dc).map(|d| d as u8) }

pub fn apply(v: &Value, a: &Assets, g: &mut Ground, p: &mut Pokes, ext: &Extensions, proof: &mut Vec<(String, Value)>) -> Result<(), String> {
    if let Some(inside) = v["place"]["starport_area"].as_bool() { p.set(g, "t.starport_area", (!inside) as u32)? }
    for m in array(&v["party"], "members") {
        let slot = num(m, "slot", 0);
        for (i, key) in ["str", "dex", "end", "int", "edu", "soc"].iter().enumerate() { if let Some(n) = m["attr"][key].as_u64() { p.set(g, &format!("party.chars[{slot}].attr[{i}]"), n as u32)? } }
        for (i, key) in ["explosive", "chemical", "radiation"].iter().enumerate() { if let Some(n) = m["grenades"][key].as_u64() { p.set(g, &format!("party.chars[{slot}].grenades[{i}]"), n as u32)? } }
    }
    for (k, o) in array(v, "ground").enumerate() {
        let w = format!("ground[{k}]");
        let id = num(o, "id", 0); a.item(id as usize, o["name"].as_str(), &w)?;
        let (row, col) = (o["cell"][0].as_i64().ok_or_else(|| format!("{w}/cell: row"))?, o["cell"][1].as_i64().ok_or_else(|| format!("{w}/cell: col"))?);
        let obj = walk::new_object(g, id as i32); if obj <= 0 { return Err(format!("{w}: no free object record")) }
        let slot = walk::place_object(g, row as i32, col as i32, obj); if slot <= 0 { return Err(format!("{w}: the cell takes no object (no free ground slot, or it swallows items)")) }
        let objects = format!("party.objects[{}]", obj - 1);
        p.set(g, &format!("{objects}.id"), id as u32)?;
        for field in ["qty", "charge"] { let value = o[field].as_u64().map_or_else(|| p.read(g, &format!("{objects}.{field}")), |n| Ok(n as u32))?; p.set(g, &format!("{objects}.{field}"), value)? }
        for (field, value) in [("item", obj as u32), ("row", row as u32), ("col", col as u32)] { p.set(g, &format!("party.ground[{slot}].{field}"), value)? }
    }
    let Some(sq) = v.get("squad") else { return Ok(()) };
    let compendium = Compendium::parse(&super::assets::packaged("npcs.toml", include_str!("../../../../data/npcs.toml"))?).map_err(|e| format!("data/npcs.toml: {e}"))?;
    let mut state = squad::State::with_compendium(compendium);
    state.p1.defs = defs_of(&ext.encounter_defs, &ext.npc_sheets);
    g.squad = Some(state);
    let selected = num(sq, "selected", 0) as u32;
    let broken = string(sq, "mode", "grouped") == "broken";
    p.set(g, "t.leader", selected)?;
    if broken {
        if g.mem.combat() == 0 && !commands_party::split(g) { return Err("squad/mode: not enough room to break the party here (the 1990 rule)".into()) }
        p.set(g, "t.squad_broken", 1)?;
        for t in array(sq, "travellers") {
            let slot = num(t, "slot", 0); let w = format!("t.walkers[{slot}]");
            if let Some(c) = t.get("cell") {
                let (r, c) = (c[0].as_i64().ok_or("squad/travellers/cell: row")? as u32 * 2, c[1].as_i64().ok_or("squad/travellers/cell: col")? as u32 * 2);
                for (field, value) in [("active", 1), ("x", r), ("y", c), ("aim_x", r), ("aim_y", c)] { p.set(g, &format!("{w}.{field}"), value)? }
            }
            if let Some(f) = t["facing"].as_u64() { p.set(g, &format!("{w}.facing"), f as u32)? }
        }
        walk::set_leader(g, selected as i16);
        p.set(g, "t.walker", selected)?;
        for (target, value) in [("t.row", g.mem.x()), ("t.col", g.mem.y()), ("t.facing", g.mem.heading())] { p.set(g, target, value as u16 as u32)? }
    }
    proof.push(("squad.mode".into(), json!(if broken { "broken" } else { "grouped" }))); proof.push(("squad.selected".into(), json!(selected)));
    for t in array(sq, "travellers") {
        let slot = num(t, "slot", 0); let w = format!("t.walkers[{slot}]");
        if let Some(route) = t["route"].as_array() {
            if !broken { return Err("squad/travellers/route: routes need mode = broken".into()) }
            let mut prev = round::traveller_cell(g, slot as u8);
            let mut bytes = Vec::new();
            for (i, c) in route.iter().enumerate() {
                let (r, cc) = (c[0].as_i64().unwrap_or(0), c[1].as_i64().unwrap_or(0));
                bytes.push(direction(g, r - prev.0 as i64, cc - prev.1 as i64).ok_or_else(|| format!("squad/travellers slot {slot}/route[{i}]: not one step from the cell before"))?);
                prev = (r as i16, cc as i16);
            }
            p.set(g, &format!("{w}.route_len"), bytes.len() as u32)?;
            for (i, b) in bytes.iter().enumerate() { p.set(g, &format!("{w}.raw+{}", 23 + i), *b as u32)? }
        }
        if let Some(target) = t["target"].as_u64() { if !broken { return Err("squad/travellers/target: targets need mode = broken".into()) } p.set(g, &format!("{w}.target"), target as u32)? }
        let (acted, attacked) = (t["acted"] == true, t["attacked"] == true);
        let s = g.squad.as_mut().unwrap();
        s.p1.turns[(slot as usize).min(4)] = Turn { moved: acted, attacked: acted || attacked, done: acted, mp: if acted { 0 } else { round::MOVE_WALK }, ..Turn::default() };
        proof.push((format!("squad.travellers[{slot}].cell"), json!(round::traveller_cell(g, slot as u8))));
        if acted || attacked { proof.push((format!("squad.travellers[{slot}].pips"), json!({"acted": acted, "attacked": acted || attacked}))) }
    }
    for x in array(sq, "npc") {
        let k = num(x, "record", 0) as u8;
        let defs_alert = g.squad.as_ref().unwrap().p1.defs.first().map_or(round::ALERT_ROUNDS, |d| d.alerted_rounds);
        let st = NpcSt { role: x["role"].as_str().map(String::from), skills: x["skills"].as_object().into_iter().flatten().filter_map(|(n, l)| Some((n.clone(), l.as_i64()? as i32))).collect(),
            engaged: x["engaged"] == true, alerted: x["alerted_rounds"].as_u64().map(|n| n as u16), alert_full: x["alerted_rounds"].as_u64().map_or(defs_alert, |n| n as u16), last_seen: None, speed: num(x, "speed", 0) as u8, acted: false, ..Default::default() };
        if let Some(hits) = x["hits"].as_u64() {
            g.squad.as_mut().unwrap().pools.insert(k, hits as i32);
            if hits == 0 { p.set(g, &format!("t.monsters[{}].level", k - 1), 20)? }
            proof.push((format!("squad.npc[{k}].hits"), json!(hits)));
        }
        proof.push((format!("squad.npc[{k}]"), json!({"role": st.role, "skills": st.skills, "engaged": st.engaged, "alerted_rounds": st.alerted, "speed": st.speed})));
        g.squad.as_mut().unwrap().p1.npcs.insert(k, st);
    }
    if let Some(en) = sq.get("encounter") {
        let id = string(en, "id", "");
        let def = g.squad.as_ref().unwrap().p1.def(id).cloned().ok_or_else(|| format!("squad/encounter: {id:?} is no encounter of this level or of this file"))?;
        let round_n = num(en, "round", 1) as u16;
        let side = if string(en, "to_act", "party") == "npcs" { Side::Npcs } else { Side::Party };
        let surprised = match string(en, "surprised", "none") { "party" => Some(Side::Party), "npcs" => Some(Side::Npcs), _ => None };
        let pools = round::pools(g);
        let s = g.squad.as_mut().unwrap();
        if matches!(def.start, Start::Area(_)) { s.p1.done.push(def.id.clone()) }
        s.p1.encounter = Some(Encounter { def: def.id.clone(), round: round_n, side, first: side, choosing: false, surprised, quiet: 0, grouped: !broken, group_moved: false, pools, npc_start: Vec::new(), withdrawing: false, morale_round: 0, interrupt: None });
        s.encounter = true; s.round = round_n;
        let roster = round::npc_side(g); if let Some(e) = g.squad.as_mut().and_then(|s| s.p1.encounter.as_mut()) { e.npc_start = roster }
        let s = g.squad.as_mut().unwrap();
        if surprised == Some(Side::Party) { for t in &mut s.p1.turns { t.attacked = true } }
        s.p1.strip = format!("Round {round_n} - {} to act", side.name());
        proof.push(("squad.encounter".into(), json!({"id": id, "round": round_n, "to_act": side.name(), "surprised": surprised.map(Side::name)})));
    }
    Ok(())
}
pub fn record(g: &Ground) -> Value {
    let Some(s) = g.squad.as_ref() else { return Value::Null };
    let p = &s.p1;
    let travellers: Vec<Value> = (0..5u8).filter(|&i| round::alive(g, i)).map(|i| { let w = mt1::ground::state::Mem::walker(i as i16); let t = p.turns[i as usize];
        json!({"slot": i, "cell": round::traveller_cell(g, i), "facing": g.mem.read(w.add(1), 1), "acted": t.done, "attacked": t.attacked, "route_len": g.mem.read(w.add(22), 1), "target": g.mem.read(w.add(5), 1)}) }).collect();
    let npcs: Vec<Value> = p.npcs.iter().map(|(k, n)| json!({"record": k, "role": n.role, "skills": n.skills, "hits": s.pools.get(k), "engaged": n.engaged, "alerted_rounds": n.alerted, "speed": n.speed})).collect();
    json!({"mode": if g.mem.combat() != 0 { "broken" } else { "grouped" }, "selected": round::selected(g), "travellers": travellers, "npc": npcs,
        "encounter": p.encounter.as_ref().map(|e| json!({"id": e.def, "round": e.round, "to_act": e.side.name(), "surprised": e.surprised.map(Side::name), "pools": e.pools, "grouped": e.grouped})),
        "alerted": round::alerted_count(g), "defs": p.defs.iter().map(|d| json!({"id": d.id, "locked": d.locked(), "allow_save": d.allow_save})).collect::<Vec<_>>(), "strip": p.strip})
}
