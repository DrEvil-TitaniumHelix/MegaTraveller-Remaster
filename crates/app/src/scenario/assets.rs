use super::{array, num, string, validate};
use mt_engine::{data::{world, Variant}, io::{Dir, Files}};
use serde_json::Value;
use std::{collections::{BTreeMap, BTreeSet}, path::Path,io::Read};

pub struct Assets {
    pub areas: Vec<world::Chunk>,
    pub items: Vec<Vec<u8>>,
    pub roles: Value,
    pub encounters: Value,
}
impl Assets {
    pub fn load(dir: &Path) -> Result<Self,String> {
        let files=Dir(dir.into());
        let w=files.read("W.DAT").ok_or("place: cannot read W.DAT")?;
        let areas=world::decode(Variant::Mt1,&w).map_err(|e|format!("place: W.DAT {e:?}"))?;
        let i=files.read("TRAV.ITM").ok_or("party/items: cannot read TRAV.ITM")?;
        let count=i.get(..2).map(|b|u16::from_le_bytes([b[0],b[1]]) as usize).ok_or("TRAV.ITM: short header")?;
        let items=i.get(2..2+46*count).ok_or("TRAV.ITM: short table")?.chunks_exact(46).map(<[u8]>::to_vec).collect();
        let parse=|text:&str| -> Result<Value,String> { let t:toml::Value=toml::from_str(text).map_err(|e|e.to_string())?; serde_json::to_value(t).map_err(|e|e.to_string()) };
        let roles=parse(&packaged("npcs.toml",include_str!("../../../../data/npcs.toml"))?)?;
        let schema:Value=serde_json::from_str(include_str!("../../../../schema/npcs.schema.json")).map_err(|e|e.to_string())?;
        validate::check(&schema,&schema,&roles,"data/npcs.toml")?;
        let encounters=parse(&packaged("encounters.toml",include_str!("../../../../data/encounters.toml"))?)?;
        Ok(Self {areas,items,roles,encounters})
    }
    pub fn area(&self,w:usize,a:usize)->Result<&world::Area,String> {
        let bases=[0,4,7,13,16,18,21,24,28];
        if w>=8 || bases[w]+a>=bases[w+1] {return Err(format!("place/area: world {w} has no area {a}"))}
        match self.areas.get(bases[w]+a) {Some(world::Chunk::Area(a))=>Ok(a),_=>Err("place/area: missing W.DAT area".into())}
    }
    pub fn map<'a>(&self,a:&'a world::Area,level:usize)->Result<&'a world::Map,String> {
        let offset=a.level_offs.get(level).ok_or("place/level: invalid level")?;
        a.map_offs.iter().position(|&o|o==*offset as usize).and_then(|i|a.maps.get(i)).ok_or_else(||format!("place/level: no map for level {level}"))
    }
    pub fn item(&self, id:usize, name:Option<&str>, path:&str)->Result<&[u8],String> {
        let it=self.items.get(id).ok_or_else(||format!("{path}: item {id} not in TRAV.ITM"))?;
        if let Some(name)=name {
            let actual:String=it[..29].iter().take_while(|&&b|b!=0).map(|&b|b as char).collect();
            if name!=actual {return Err(format!("{path}: item {id} is '{actual}', not '{name}'"))}
        }
        Ok(it)
    }
    pub fn validate(&self,v:&Value)->Result<(),String> {
        let pl=&v["place"];let level=num(pl,"level",0) as usize;
        let a=self.area(num(pl,"world",0) as usize,num(pl,"area",0) as usize)?;
        let m=self.map(a,level)?;
        cell(m,&pl["cell"],"place/cell")?;
        let outdoor=!m.hdr.name.starts_with(b"INSIDE") && !m.hdr.name.starts_with(b"inside");
        if pl["outdoors"].as_bool().is_some_and(|b|b!=outdoor) {return Err("place/outdoors: disagrees with level map".into())}
        for group in ["members","items"] {
            let mut seen=BTreeSet::new();
            for member in array(&v["party"],group) {
                let slot=num(member,"slot",0);
                if !seen.insert(slot) {return Err(format!("party/{group}: duplicate slot {slot}"))}
                if member["present"]==false && member["hits"].as_u64().is_some_and(|n|n>0) {return Err(format!("party/members/{slot}/hits: a removed member has zero hits"))}
                if member.get("present").is_some() && slot>4 {return Err(format!("party/members/{slot}/present: only slots 0..4"))}
            }
        }
        for gi in array(&v["party"],"items") {
            for part in ["hands","armor","pack"] {
                let entries:Vec<_>=if part=="armor" {gi.get(part).into_iter().collect()} else {array(gi,part).collect()};
                for it in entries {self.item(num(it,"id",0) as usize,it["name"].as_str(),&format!("party/items/{}/{part}",num(gi,"slot",0)))?;}
            }
        }
        let lit=array(&v["party"],"items").any(|gi|["hands","pack"].iter().any(|part|array(gi,part).any(|it|self.items.get(num(it,"id",0) as usize).is_some_and(|d|u16::from_le_bytes([d[0x1D],d[0x1E]])==11) && num(it,"charge",0)!=0)));
        if matches!(level,1|2) && !lit && pl["dark"]!=true {return Err(format!("place/level: level {level} is a dark level: the 1990 draws a black room without a light turned on; give the party an electric torch (id 52) or electric lantern (53) with charge = 1"))}
        let mut used=BTreeSet::new();
        for n in array(&v["npcs"],"set") {
            let record=num(n,"record",0) as usize;
            if n["present"]==false && ["cell","level","hostile","hits","weapon","ammo"].iter().any(|key|n.get(key).is_some()) {return Err(format!("npcs/set/{record}/present: removal conflicts with other edits"))}
            if record>a.objects.len() {return Err(format!("npcs/set/{record}: area has {} records",a.objects.len()))}
            if !used.insert(record) {return Err(format!("npcs/set/{record}: record already used"))}
            if n.get("weapon").is_some() {self.item(num(n,"weapon",0) as usize,None,&format!("npcs/set/{record}/weapon"))?;}
            let l=num(n,"level",a.objects[record-1].state as i64) as usize;
            if n.get("cell").is_some() {cell(self.map(a,l)?,&n["cell"],&format!("npcs/set/{record}/cell"))?;}
            else if n.get("level").is_some() {let old=&a.objects[record-1];cell(self.map(a,l)?,&serde_json::json!([old.x,old.y]),&format!("npcs/set/{record}/level"))?;}
        }
        if let Some(vehicle)=v.get("vehicle") {
            if !outdoor {return Err("vehicle: outdoor map required".into())}
            let t=num(vehicle,"type",0);
            if a.hdr.f16 & (1<<(t-1))==0 {return Err(format!("vehicle/type: world does not rent type {t}"))}
        }
        let mut ids=BTreeMap::new();let mut appended=a.objects.len();
        for n in array(v,"npc") {
            let role=string(n,"role","");let path=format!("npc {role}");
            let r=self.roles["role"].get(role).ok_or_else(||format!("{path}: unknown role"))?;
            if let Some(id)=n["id"].as_str() {if ids.insert(id,n).is_some() {return Err(format!("{path}/id: duplicate {id}"))}}
            cell(self.map(a,num(n,"level",level as i64) as usize)?,&n["at"],&path)?;
            let record=if n.get("record").is_some() {let k=num(n,"record",0) as usize;
                if k>a.objects.len() {return Err(format!("{path}/record: omit record to append"))} k
            } else {appended+=1;appended};
            if record>32 {return Err(format!("{path}/record: no free slot (maximum 32)"))}
            if !used.insert(record) {return Err(format!("{path}/record: record already used"))}
            self.item(num(n,"weapon",num(&r["equipment"],"weapon",255)) as usize,None,&format!("{path}/weapon"))?;
        }
        let mut seen=BTreeSet::new();
        for e in array(v,"encounter") {
            let path=format!("encounter {}",string(e,"id",""));
            if !seen.insert(string(e,"id","")) {return Err(format!("{path}: duplicate id"))}
            self.encounter(e,m,a.objects.len(),&ids,&path)?;
        }
        if v["place"].get("starport_area").is_some() && !outdoor {return Err("place/starport_area: the starport area is outdoors; this level map is INSIDE".into())}
        for (k,o) in array(v,"ground").enumerate() {cell(m,&o["cell"],&format!("ground[{k}]"))?;self.item(num(o,"id",0) as usize,o["name"].as_str(),&format!("ground[{k}]"))?;}
        if let Some(sq)=v.get("squad") {
            let (broken,active)=(sq["mode"]=="broken",sq.get("encounter").is_some());
            let (mut slots,mut cells)=(BTreeSet::new(),BTreeSet::new());
            for t in array(sq,"travellers") {
                let slot=num(t,"slot",0);let w=format!("squad/travellers slot {slot}");
                if !slots.insert(slot) {return Err("squad/travellers: duplicate slot".into())}
                if let Some(c)=t.get("cell") {
                    if !broken {return Err("squad/travellers: cells need mode = broken (a grouped party stands on place.cell)".into())}
                    cell(m,c,&w)?;
                    if !cells.insert((c[0].as_i64(),c[1].as_i64())) {return Err("squad/travellers: two Travellers on one cell".into())}
                }
                let mut prev=t.get("cell").unwrap_or(&v["place"]["cell"]).clone();
                for (i,c) in array(t,"route").enumerate() {
                    cell(m,c,&format!("{w}/route[{i}]"))?;
                    let step=|j:usize|(c[j].as_i64().unwrap_or(0)-prev[j].as_i64().unwrap_or(0)).abs();
                    if step(0)>1 || step(1)>1 || step(0)+step(1)==0 {return Err(format!("{w}/route[{i}]: each cell next to the one before"))}
                    prev=c.clone();
                }
                if t.get("target").is_some_and(|x|x.as_u64().is_none_or(|k|k==0 || k>a.objects.len() as u64)) {return Err(format!("{w}/target: record outside area"))}
                if (t["acted"]==true || t["attacked"]==true) && !active {return Err("squad/travellers: acted / attacked need an active squad.encounter".into())}
            }
            if let Some(en)=sq.get("encounter") {
                let id=string(en,"id","");
                if !self.game_encounters(&v["place"])?.iter().chain(array(v,"encounter")).any(|e|e["id"]==id) {return Err(format!("squad/encounter: {id:?} is no encounter of this level (data/encounters.toml) or of this file"))}
                if string(en,"surprised","none")!="none" && num(en,"round",1)!=1 {return Err("squad/encounter: surprise belongs to round 1".into())}
            }
            for x in array(sq,"npc") {
                let k=num(x,"record",0);let w=format!("squad/npc record {k}");
                if k<1 || k>a.objects.len() as i64 {return Err(format!("{w}: record outside area"))}
                if let Some(r)=x["role"].as_str() {if r!="none" && self.roles["role"].get(r).is_none() {return Err(format!("{w}/role: unknown role"))}}
                if x["engaged"]==true && !active {return Err(format!("{w}: engaged needs an active squad.encounter"))}
            }
        }
        Ok(())
    }
    pub fn encounter(&self,e:&Value,m:&world::Map,count:usize,ids:&BTreeMap<&str,&Value>,p:&str)->Result<(),String> {
        let err=|s:&str|Err(format!("{p}: {s}"));
        if (e["start"]=="scripted")!=e.get("trigger").is_some() {return err("scripted needs trigger; sight takes none")}
        if (e["boundary"]=="area")!=e.get("rect").is_some() {return err("boundary area needs rect, only area takes rect")}
        if e.get("radius").is_some() && string(e,"boundary","radius")!="radius" {return err("radius requires boundary radius")}
        if e["end"]=="open" && e.get("escape_zone").is_some() {return err("escape_zone requires locked")}
        if e["end"]=="locked" && e.get("n_rounds").is_some() {return err("n_rounds requires open")}
        for key in ["rect","escape_zone"] {if let Some(r)=e.get(key) {rect(m,r,p)?}}
        if let Some(t)=e.get("trigger") {
            let required=match string(t,"kind","") {"area_entry"=>vec!["kind","rect"],"dialogue"=>vec!["kind","record","state"],
                "object_taken"=>vec!["kind","item"],"story_flag"=>vec!["kind","flag",if t["flag"]=="spawn_taken" {"spawn"} else {"npc_id"}],_=>return err("unknown trigger")};
            for k in &required {if t.get(k).is_none() {return Err(format!("{p}/trigger/{k}: required field"))}}
            if t.as_object().is_some_and(|o|o.keys().any(|k|!required.contains(&k.as_str()))) {return err("trigger has fields for another kind")}
            if let Some(r)=t.get("rect") {rect(m,r,p)?}
            if t.get("item").is_some() {self.item(num(t,"item",0) as usize,None,p)?;}
            if num(t,"record",0)>count as i64 {return err("trigger record outside area")}
        }
        for participant in array(e,"participants") {
            if let Some(id)=participant.as_str() {if !ids.contains_key(id) {return Err(format!("{p}: participant {id:?} has no npc id"))}}
            else if participant.as_u64().is_none_or(|k|k==0 || k>count as u64) {return err("participant record outside area")}
        }
        Ok(())
    }
    pub fn game_encounters(&self,place:&Value)->Result<Vec<Value>,String> {
        let schema:Value=serde_json::from_str(include_str!("../../../../schema/scenario.schema.json")).map_err(|e|e.to_string())?;
        validate::check(&schema,&schema["$defs"]["encounter_file"],&self.encounters,"data/encounters.toml")?;
        let mut seen=BTreeSet::new();
        for e in array(&self.encounters,"encounter") {
            let w=&e["where"];let a=self.area(num(w,"world",0) as usize,num(w,"area",0) as usize)?;
            let m=self.map(a,num(w,"level",0) as usize)?;
            let path=format!("data/encounters.toml encounter {}",string(e,"id",""));
            if !seen.insert(format!("{w}:{}",string(e,"id",""))) {return Err(format!("{path}: duplicate id on level"))}
            self.encounter(e,m,a.objects.len(),&BTreeMap::new(),&path)?;
        }
        Ok(array(&self.encounters,"encounter").filter(|e|["world","area","level"].iter().all(|k|e["where"][k]==place[k])).cloned().collect())
    }
}

pub fn cell(m:&world::Map,rc:&Value,p:&str)->Result<(),String> {
    let r=rc[0].as_u64().ok_or_else(||format!("{p}: invalid row"))? as usize;
    let c=rc[1].as_u64().ok_or_else(||format!("{p}: invalid col"))? as usize;
    let (h,w)=(m.hdr.rows as usize*12,m.hdr.cols as usize*20);
    if r>=h || c>=w {return Err(format!("{p}: cell [{r},{c}] outside map {h}x{w}"))}
    let k=(r/12*(w/20)+c/20)*240+(r%12)*20+c%20;
    let [f,t]=*m.cells.get(k).ok_or_else(||format!("{p}: missing map cell"))?;let f=f.wrapping_sub(1);
    if (f&1==0 && (t<15 || t==0x11 || t==0x13)) || (f&1!=0 && t<5) {return Err(format!("{p}: event cell; stand next to it"))}
    Ok(())
}
pub fn rect(m:&world::Map,r:&Value,p:&str)->Result<(),String> {
    let n=|i:usize,j:usize|r[i][j].as_u64().unwrap_or(u64::MAX);
    if n(0,0)>n(1,0) || n(0,1)>n(1,1) || n(1,0)>=m.hdr.rows as u64*12 || n(1,1)>=m.hdr.cols as u64*20 {
        return Err(format!("{p}: rect out of order or outside map"))
    }
    Ok(())
}

pub(super) fn packaged(name:&str,fallback:&str)->Result<String,String> {
    let path=std::env::current_exe().map_err(|e|e.to_string())?.with_file_name("data").join(name);
    match std::fs::File::open(&path) {
        Ok(file)=>{let mut text=String::new();file.take(1024*1024+1).read_to_string(&mut text).map_err(|e|format!("data/{name}: {e}"))?;
            if text.len()>1024*1024 {return Err(format!("data/{name}: exceeds 1 MiB"))} Ok(text)},
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>Ok(fallback.into()),
        Err(e)=>Err(format!("data/{name}: {e}")),
    }
}
