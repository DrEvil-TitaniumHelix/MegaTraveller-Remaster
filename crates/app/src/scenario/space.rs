use super::{num, string, Pokes, Extensions};
use crate::lockstep::{schema, symbolic};
use mt1::ground::Ground;
use mt1::space::{Space, state::ENCOUNTER};
use std::collections::BTreeMap;
use mt_engine::{data::stp, io::{Dir, Files}};
use serde_json::{json, Value};
use std::path::Path;

pub fn validate(v:&Value)->Result<(),String> {
    let ship=&v["ship"];
    if !ship.as_object().is_some_and(|o|!o.is_empty()) {return Ok(())}
    if v["meta"]["format"].as_f64()!=Some(1.2) {return Err("meta/format: ship fields require 1.2".into())}
    let screen=string(ship,"screen","flight");
    if (screen=="station")!=ship.get("station").is_some() {return Err("ship/station: required exactly for screen=station".into())}
    if screen=="jump" && ship["orbit"]!=json!(-1) {return Err("ship/orbit: jump screen requires -1 (free flight)".into())}
    if let Some(selected)=ship["encounter"].as_u64() {
        let marker=ship["ships"].get(selected as usize-1).ok_or("ship/encounter: must select a ship.ships record")?;
        if marker["direction"]==json!(-1) {return Err("ship/encounter: selected marker is inactive".into())}
    }
    if ship.get("encounter_group").is_some() && ship.get("encounter").is_none() {return Err("ship/encounter_group: requires ship.encounter".into())}
    Ok(())
}

pub fn references(v:&Value,dir:&Path)->Result<(),String> {
    let ship=&v["ship"];
    if !ship.as_object().is_some_and(|o|!o.is_empty()) {return Ok(())}
    let system=num(ship,"system",num(&v["place"],"world",0));
    let world=num(ship,"world",num(&v["place"],"area",0));
    let name=format!("{system}.STP");
    let bytes=Dir(dir.into()).read(&name).ok_or_else(||format!("ship/system: cannot read {name}"))?;
    let stp=stp::decode(&bytes).map_err(|e|format!("ship/system: {name}: {e:?}"))?;
    if stp.objects.is_empty() || stp.objects.len()>80 {return Err("ship/system: STP must have 1..80 objects".into())}
    if world!=255 && !stp.objects.iter().any(|o|i64::from(o.world)==world+1) {return Err(format!("ship/world: {world} is absent from {name}"))}
    if let Some(orbit)=ship["orbit"].as_i64() {
        if orbit>=0 && orbit as usize>=stp.objects.len() {return Err(format!("ship/orbit: {orbit} outside {name} object count {}",stp.objects.len()))}
    }
    Ok(())
}

pub fn stage(v:&Value,g:&mut Ground,p:&mut Pokes,e:&mut Extensions)->Result<(),String> {
    let ship=&v["ship"];
    if !ship.as_object().is_some_and(|o|!o.is_empty()) {return Ok(())}
    for (key,target) in [("system","party.system"),("world","party.planet")] {
        if let Some(n)=ship[key].as_u64() {p.set(g,target,n as u32)?}
    }
    if let Some(n)=ship["jump_drive"].as_u64() {
        let flags=p.read(g,"party.ship_flags")?;
        p.set(g,"party.ship_flags",(flags&!3)|n as u32)?;
    }
    if let Some(b)=ship["fuel_scoops"].as_bool() {
        let flags=p.read(g,"party.ship_flags")?;
        p.set(g,"party.ship_flags",if b {flags|0x18} else {flags&!0x18})?;
    }
    if let Some(programs)=ship["programs"].as_array() {
        for i in 0..20 {
            let byte=programs.get(i).map_or(0,|slot|num(slot,"id",0) as u32
                | if slot["loaded"]==true {0x80} else {0}
                | if slot["running"]==true {0x20} else {0});
            p.set(g,&format!("party.programs[{i}]"),byte)?;
        }
    }
    for (i,n) in super::array(&ship["damage"],"sections").enumerate() {p.set(g,&format!("party.ship_damage[{i}]"),n.as_u64().ok_or("ship/damage/sections: byte")? as u32)?}
    if let Some(n)=ship["damage"]["hull"].as_u64() {p.set(g,"party.hull_damage",n as u32)?}
    let pending=&mut e.pending_space;
    pending.insert("space_entry".into(),json!(string(ship,"screen","flight")));
    for (key,target) in [("orbit","s.current_object"),("thrust","s.thrust"),("encounter","s.encounter"),("encounter_group","space_encounter_group")] {
        if let Some(value)=ship.get(key) {pending.insert(target.into(),value.clone());}
    }
    for (key,targets) in [("position",["s.x","s.y"]),("velocity",["s.vx","s.vy"])] {
        if let Some(pair)=ship[key].as_array() {for (target,value) in targets.into_iter().zip(pair) {pending.insert(target.into(),value.clone());}}
    }
    if let Some(n)=ship["heading"].as_i64() {pending.insert("s.heading".into(),json!(90+n));pending.insert("s.direction".into(),json!(1+n));pending.insert("s.view_heading".into(),json!(n));}
    if let Some(n)=ship["thrust"].as_i64() {pending.insert("s.moving".into(),json!(i32::from(n!=0)));}
    if let Some(station)=ship["station"].as_str() {
        let id=match station {"pilot"=>10,"gunner_1"=>11,"gunner_2"=>12,"computer"=>13,"engineering"=>14,"comms"=>15,"crew"=>17,
            _=>return Err("ship/station: unknown station".into())};
        pending.insert("s.station_id".into(),json!(id));pending.insert("s.station_active".into(),json!(1));
    }
    if let Some(ships)=ship["ships"].as_array() {
        pending.insert("s.ship_count".into(),json!(ships.len()));
        let mut table:Vec<Value>=ships.iter().map(|ship| {
            let n=num(ship,"direction",-1);
            json!({"x":ship["x"],"y":ship["y"],"active":if n==-1 {-1} else {100+n},"dstx":ship["dstx"],"dsty":ship["dsty"]})
        }).collect();
        table.resize(18,json!({"x":0,"y":0,"active":-1,"dstx":0,"dsty":0}));
        pending.insert("s.ships".into(),json!(table));
    }
    Ok(())
}

pub fn apply_pending(sp:&mut Space,pending:&BTreeMap<String,Value>)->Result<(),String> {
    let sch=schema::schema();
    let mut fields=schema::fields(&sch["properties"]["party"],"party",0);
    fields.extend(schema::fields(&sch["properties"]["s"],"s",0));
    let mut encounter_group=None;
    for (target,value) in pending {
        match target.as_str() {
            "space_entry"=>sp.scenario_entry=value.as_str().map(String::from),
            "space_encounter_group"=>encounter_group=Some(value),
            "s.ships"=>{
                let rows=value.as_array().ok_or("scenario ship: ships: array")?;
                for (i,row) in rows.iter().enumerate().take(18) {
                    for (k,name) in ["x","y","active","dstx","dsty"].iter().enumerate() {
                        let v=row[*name].as_i64().ok_or_else(||format!("scenario ship: ships[{i}].{name}: integer"))?;
                        sp.s.set_enc(i as i16,2*k as u16,v as i16);
                    }
                }
            }
            _=>{
                let v=value.as_i64().ok_or_else(||format!("scenario ship: {target}: integer"))?;
                let f=fields.iter().find(|f|f.path==*target).ok_or_else(||format!("scenario ship: unmapped S target {target}"))?;
                let (section,_)=symbolic::resolve(&fields,target)?;
                for k in 0..f.width {
                    let b=(v>>(8*k)) as u8;
                    match section.as_str() {"s"=>sp.s.set_byte((f.offset+k) as u16,b),"party"=>sp.s.set_pbyte((f.offset+k) as u16,b),_=>return Err(format!("scenario ship: {target}: section {section} is not S state"))}
                }
            }
        }
    }
    if let Some(group)=encounter_group {
        let encounter=sp.s.word(ENCOUNTER);
        if encounter<1 {return Err("scenario ship: encounter_group needs encounter".into())}
        let kind=match group["kind"].as_str() {Some("light")=>0,Some("medium")=>1,Some("heavy")=>2,_=>return Err("scenario ship: encounter_group.kind: light, medium or heavy".into())};
        let count=group["count"].as_i64().filter(|n|(1..=4).contains(n)).ok_or("scenario ship: encounter_group.count: 1..4")?;
        let faction=group["faction"].as_i64().filter(|n|(1..=13).contains(n)).ok_or("scenario ship: encounter_group.faction: 1..13")?;
        let first=(encounter-1).wrapping_mul(4);
        for i in first..first+4 {for off in (0..30).step_by(2) {sp.s.set_enemy(i,off,0);}}
        sp.create_ships(first,count as i16,faction as i16,kind);
    }
    if sp.s.word(ENCOUNTER)>0 {sp.init_encounter();}
    Ok(())
}
