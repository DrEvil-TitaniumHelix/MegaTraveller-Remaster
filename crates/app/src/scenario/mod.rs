mod assets;
mod pokes;
mod validate;
mod space;
mod squad;
use assets::Assets;
use pokes::Pokes;
use mt1::ground::{self, Ground, render, walk, state::Mem};
use mt_engine::{io::{Dir,Files,Scripted},sim::tick::Counter};
#[cfg(test)]
use mt_engine::sim::tick::Clock;
use serde_json::{Value,json};
use std::{cell::RefCell,collections::BTreeMap,fs,io::Read,path::{Path,PathBuf},rc::Rc};

pub type Request=Rc<RefCell<Option<PathBuf>>>;
pub fn num(v:&Value,key:&str,default:i64)->i64 {v[key].as_i64().unwrap_or(default)}
pub fn string<'a>(v:&'a Value,key:&str,default:&'a str)->&'a str {v[key].as_str().unwrap_or(default)}
pub fn array<'a>(v:&'a Value,key:&str)->impl Iterator<Item=&'a Value> {v[key].as_array().into_iter().flatten()}

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Extensions {
    pub pending_srand:BTreeMap<String,u32>,
    #[serde(deserialize_with="npc_sheets_from_json")]
    pub npc_sheets:BTreeMap<usize,Value>,
    pub encounter_defs:Vec<Value>,
    pub pending_space:BTreeMap<String,Value>,
}
fn npc_sheets_from_json<'de,D:serde::Deserializer<'de>>(d:D)->Result<BTreeMap<usize,Value>,D::Error> {
    let sheets=<BTreeMap<String,Value> as serde::Deserialize>::deserialize(d)?;
    sheets.into_iter().map(|(key,sheet)|key.parse::<usize>().map(|key|(key,sheet)).map_err(serde::de::Error::custom)).collect()
}
impl Extensions {
    fn record(&self)->Value {json!({"pending_srand":self.pending_srand,"npc_sheets":self.npc_sheets,"encounter_defs":self.encounter_defs,"pending_space":self.pending_space})}
}
pub struct Prepared {
    pub game:Ground,
    pub extensions:Extensions,
    pub proof:Vec<(String,Value)>,
    pub path:PathBuf,
    pub label:String,
}
pub fn label(v:&Value,path:&Path)->String {
    let stem=path.file_stem().map(|s|s.to_string_lossy().into_owned()).unwrap_or_default();
    string(&v["meta"],"menu",string(&v["meta"],"name",&stem)).to_string()
}
impl Prepared {
    pub fn report(&self) {
        for (target,value) in &self.proof {println!("scenario {}: {target} = {value} (read back)",self.path.display())}
        if self.extensions.pending_space.is_empty() {println!("scenario applied: {} targets, 0 mismatches",self.proof.len());}
        else {
            println!("scenario block prepared: {} targets, 0 mismatches; S targets apply at launch",self.proof.len());
            for (target,value) in &self.extensions.pending_space {println!("scenario pending (applied at launch): {target} = {value}");}
        }
    }
    pub fn require_playable(&self)->Result<(),String> {Ok(())}
    pub fn record(&self)->Value {let mut r=pokes::record(&self.game);r["scenario"]=self.extensions.record();r["scenario"]["squad"]=squad::record(&self.game);r}
}
pub fn load(path:&Path,dir:&Path)->Result<Prepared,String> {
    let result=(|| {
        let mut text=String::new();
        fs::File::open(path).map_err(|e|format!("file: {e}"))?.take(1024*1024+1).read_to_string(&mut text).map_err(|e|format!("file: {e}"))?;
        if text.len()>1024*1024 {return Err("file: exceeds 1 MiB".into())}
        prepare(&text,path,dir)
    })();
    result.map_err(|e:String|format!("scenario {}: {}",path.display(),e.replace(['\r','\n']," ")))
}
fn prepare(text:&str,path:&Path,dir:&Path)->Result<Prepared,String> {
    let v=validate::document(text)?;
    if let Some(file)=v["save"]["file"].as_str() {let mut p=prepare_save(file,path,dir)?;p.label=label(&v,path);return Ok(p)}
    let assets=Assets::load(dir)?;
    assets.validate(&v)?;
    space::references(&v,dir)?;
    let files=Dir(dir.into());
    let block=files.read("DEFAULT.SAV").ok_or("base_block: cannot read DEFAULT.SAV")?;
    let t=files.read("T.DAT").ok_or("base_block: cannot read T.DAT")?;
    let mut g=Ground::new(&t,block,Box::new(Scripted::new([])),Box::new(Counter::default()),Box::new(files)).ok_or("base_block: invalid T.DAT or DEFAULT.SAV")?;
    g.scenario_boot=true;
    g.mouse_driver=true;
    ground::start(&mut g);
    let mut p=Pokes::new();let mut extensions=Extensions::default();
    let flags=p.read(&g,"party.ship_flags")? & 127;
    p.set(&mut g,"party.ship_flags",flags)?;
    for (key,target) in [("world","party.system"),("area","party.planet")] {p.set(&mut g,target,num(&v["place"],key,0) as u32)?}
    block_phase(&v,&assets,&mut g,&mut p)?;
    let block_proof: Vec<_> = p.proof(&g)?.into_iter().filter(|(target,value)|
        target.starts_with("party.objects[") && target.ends_with(".id") && *value==255).collect();
    for (target,_) in &block_proof { p.expected.remove(target); }
    if !render::game_init(&mut g) {return Err("place: game_init could not load the world".into())}
    g.mem.set_phase(0);render::redraw(&mut g);
    g.boot_checkpoint("main_loop_ready");
    let pl=&v["place"];
    let level=num(pl,"level",0) as i16;
    if level!=g.mem.level() as i16 && !walk::event_at(&mut g,num(pl,"world",0) as u16,num(pl,"area",0) as u16,level) {
        return Err("place/level: enter_area failed".into())
    }
    let row=pl["cell"][0].as_i64().ok_or("place/cell: row")? as i16;
    let col=pl["cell"][1].as_i64().ok_or("place/cell: col")? as i16;
    walk::set_player_pos(&mut g,row,col);
    p.set(&mut g,"t.level",level as u32)?;
    p.set(&mut g,"t.row",row as u32*2)?;p.set(&mut g,"t.col",col as u32*2)?;
    if let Some(f)=pl["facing"].as_u64() {p.set(&mut g,"t.facing",f as u32)?}
    npc_changes(&v,&assets,&mut g,&mut p)?;
    spawn(&v,&assets,&mut g,&mut p,&mut extensions)?;
    encounters(&v,&assets,&g,&mut extensions)?;
    if let Some(vehicle)=v.get("vehicle") {vehicle_apply(vehicle,&mut g,&mut p)?}
    for (field,target) in [("law","t.world_law"),("tech","t.world_tech"),("offences","t.offences")] {
        if let Some(n)=v["starport"][field].as_u64() {p.set(&mut g,target,n as u32)?}
    }
    for (i,name) in ["zenaca","casino","library","tas","hospital","weapons","armor","general","imperial"].iter().enumerate() {
        if let Some(b)=v["starport"]["offices"][name].as_bool() {p.set(&mut g,&format!("t.office_open[{}]",11+i),b as u32)?}
    }
    let mut squad_proof=Vec::new();
    squad::apply(&v,&assets,&mut g,&mut p,&extensions,&mut squad_proof)?;
    for prog in ["T","S","CG","STARTUP"] {
        if let Some(seed)=v["seeds"][prog].as_u64() {
            if prog=="T" {p.set(&mut g,"rng.T",seed as u32)?}
            else {extensions.pending_srand.insert(prog.into(),seed as u32);}
        }
    }
    g.mem.set_i16(0x6413,-1);
    render::dismiss_message(&mut g,1);render::redraw_party(&mut g);render::redraw(&mut g);
    space::stage(&v,&mut g,&mut p,&mut extensions)?;
    let mut proof=block_proof;
    proof.extend(p.proof(&g)?);
    for (prog,&seed) in &extensions.pending_srand {proof.push((format!("pending_srand.{prog}"),seed.into()))}
    for (record,sheet) in &extensions.npc_sheets {proof.push((format!("npc_spawn[{record}].sheet"),sheet.clone()))}
    proof.push(("encounter_defs".into(),json!(extensions.encounter_defs)));
    proof.extend(squad_proof);
    let snap=pokes::record(&g);
    let schema=crate::lockstep::schema::schema();
    let block=crate::lockstep::schema::encode_party(&schema["properties"]["party"],&snap["party"])?;
    if block!=g.mem.party() {return Err("base_block: lossless read-back failed".into())}
    Ok(Prepared {game:g,extensions,proof,path:path.into(),label:label(&v,path)})
}

fn prepare_save(file:&str,path:&Path,dir:&Path)->Result<Prepared,String> {
    let file=path.parent().unwrap_or(Path::new("")).join(file);
    let bytes=fs::read(&file).map_err(|e|format!("save/file: {}: {e}{}",file.display(),
        if e.kind()==std::io::ErrorKind::NotFound {"; make it first: load 'Starport before save' and save (or run the --scenario-check --save command)"} else {""}))?;
    let saved=mt1::save::Saved::decode(&bytes).map_err(|e|format!("save/file: {}: {e}",file.display()))?;
    let (mode,build)=(saved.mode,saved.build.clone());
    let mut g=mt1::save::ground(saved.clone(),Box::new(Scripted::new([])),Box::new(Counter::at(saved.ticks)),Box::new(Dir(dir.into())));
    if mt1::save::Saved::capture(&mut g,mode,&build)!=saved {return Err(format!("save/file: {}: the installed state does not read back as the file",file.display()))}
    g.mouse_driver=true;
    let mut extensions=Extensions::default();
    extensions.encounter_defs=Assets::load(dir)?.game_encounters(&json!({"world":g.mem.party_u8(6),"area":g.mem.party_u8(7),"level":g.mem.level()}))?;
    let proof=vec![("save.file".into(),json!(file.display().to_string())),("save.mode".into(),json!(format!("{mode:?}").to_lowercase())),("save.image_bytes".into(),json!(bytes.len()))];
    Ok(Prepared {game:g,extensions,proof,path:path.into(),label:String::new()})
}

fn block_phase(v:&Value,a:&Assets,g:&mut Ground,p:&mut Pokes)->Result<(),String> {
    for key in ["credits","fuel"] {if let Some(n)=v["party"][key].as_u64() {p.set(g,&format!("party.{key}"),n as u32)?}}
    if v["party"].get("cargo").is_some() {
        for i in 0..20 {p.set(g,&format!("party.cargo[{i}]"),v["party"]["cargo"][i].as_u64().unwrap_or(0) as u32)?}
    }
    if let Some(date)=v["time"]["date"].as_u64() {p.set(g,"party.clock",date as u32)?}
    for (i,n) in array(&v["party"]["damage"],"sections").enumerate() {p.set(g,&format!("party.ship_damage[{i}]"),n.as_u64().ok_or("party/damage/sections: byte")? as u32)?}
    if let Some(n)=v["party"]["damage"]["hull"].as_u64() {p.set(g,"party.hull_damage",n as u32)?}
    for member in array(&v["party"],"members") {
        let slot=num(member,"slot",0);let base=format!("party.chars[{slot}]");
        for key in ["hits","hits_max","credits","oxygen"] {if let Some(n)=member[key].as_u64() {p.set(g,&format!("{base}.{key}"),n as u32)?}}
        for (k,x) in member["skills"].as_object().into_iter().flatten() {p.set(g,&format!("{base}.skills[{k}]"),x.as_u64().ok_or("party/members/skills: integer")? as u32)?}
        if let Some(present)=member["present"].as_bool() {
            let mask=p.read(g,"party.alive_mask")?;
            p.set(g,"party.alive_mask",if present {mask|(1<<slot)} else {mask&!(1<<slot)})?;
            let flags=p.read(g,&format!("{base}.flags"))?;
            p.set(g,&format!("{base}.flags"),if present {flags&!2} else {flags|2})?;
            if !present {
                p.set(g,&format!("{base}.hits"),0)?;
                for i in 0..12 {let target=format!("party.stations[{i}]");if p.read(g,&target)?==slot as u32+1 {p.set(g,&target,0)?}}
            }
        }
        if p.read(g,&format!("{base}.hits"))?>p.read(g,&format!("{base}.hits_max"))? {return Err(format!("party/members/{slot}/hits: exceeds hits_max"))}
    }
    let mut lights=0u16;
    for gi in array(&v["party"],"items") {
        let slot=num(gi,"slot",0) as usize;
        if gi["replace"].as_bool().unwrap_or(true) {
            for c in 0..23 {
                let target=inventory(slot,c);let obj=p.read(g,&target)?;
                if (1..=256).contains(&obj) {p.set(g,&format!("party.objects[{}].id",obj-1),255)?}
                p.set(g,&target,0)?;
            }
        }
        for (name,range) in [("hands",21..23),("armor",20..21),("pack",0..19)] {
            let entries:Vec<_>=if name=="armor" {gi.get(name).into_iter().collect()} else {array(gi,name).collect()};
            for item in entries {
                let cell=match item["cell"].as_u64() {
                    Some(c) if name=="pack" => {let c=c as usize; if c>18 {return Err(format!("party/items/{slot}/pack/cell: 0..18"))} if p.read(g,&inventory(slot,c))?!=0 {return Err(format!("party/items/{slot}/pack/cell: cell {c} is taken"))} c}
                    Some(_) => return Err(format!("party/items/{slot}/{name}/cell: only pack items take a cell")),
                    None => range.clone().find(|&c|p.read(g,&inventory(slot,c)).is_ok_and(|n|n==0)).ok_or_else(||format!("party/items/{slot}/{name}: no free inventory cell"))?,
                };
                let obj=(0..256).find(|i|p.read(g,&format!("party.objects[{i}].id")).is_ok_and(|n|n==255))
                    .ok_or("party/items: no free object record")?;
                let id=num(item,"id",0);let def=a.item(id as usize,None,"party/items")?;
                if u16::from_le_bytes([def[0x1D],def[0x1E]])==11 && num(item,"charge",def[0x28] as i64)!=0 {lights+=1}
                for (field,value) in [("id",id),("qty",num(item,"qty",0)),("charge",num(item,"charge",def[0x28] as i64))] {
                    p.set(g,&format!("party.objects[{obj}].{field}"),value as u32)?;
                }
                p.set(g,&inventory(slot,cell),obj as u32+1)?;
            }
        }
    }
    if v["party"]["items"].as_array().is_some_and(|a|!a.is_empty()) {g.mem.set_party_u16(0xF82,lights)}
    Ok(())
}
fn inventory(slot:usize,c:usize)->String {
    let field=match c {0..=18=>format!("inventory[{c}]"),19=>"raw.c_f2".into(),20=>"actor".into(),21=>"armor".into(),_=>"weapon".into()};
    format!("party.chars[{slot}].{field}")
}
fn monster(record:usize,field:&str)->String {format!("t.monsters[{}].{field}",record-1)}
fn occupancy(g:&Ground,row:i16,col:i16)->Result<String,String> {
    if row<0 || col<0 || row as usize>=g.mem.map_blocks_high() as usize*12 || col>=g.mem.map_stride() {return Err("npc_move: cell outside loaded map".into())}
    Ok(format!("t.maps[{}].cells[{}].attr",g.mem.level(),row as usize*g.mem.map_stride() as usize+col as usize))
}
fn cached_occupancy(g:&Ground,level:u32,row:i16,col:i16)->Result<Option<String>,String> {
    if level>=8 {return Ok(None)}
    let header=0x6096+26*level as u16;
    if g.mem.pointer(0x606e+4*level as u16)==ground::state::Far::default() {return Ok(None)}
    let width=g.mem.u8(header+1) as usize*20;let height=g.mem.u8(header) as usize*12;
    if row<0 || col<0 || row as usize>=height || col as usize>=width {return Err(format!("npc_move: cell outside cached level {level}"))}
    Ok(Some(format!("t.maps[{level}].cells[{}].attr",row as usize*width+col as usize)))
}
fn clear_npc(g:&mut Ground,p:&mut Pokes,k:usize)->Result<(),String> {
    let level=p.read(g,&monster(k,"level"))?;
    let row=p.read(g,&monster(k,"x"))? as i16/2;let col=p.read(g,&monster(k,"y"))? as i16/2;
    if let Some(target)=cached_occupancy(g,level,row,col)? {
        let flags=p.read(g,&target)?;
        if flags&63==k as u32 {p.set(g,&target,flags&192)?}
    }
    Ok(())
}
fn move_npc(g:&mut Ground,p:&mut Pokes,k:usize,row:u32,col:u32,level:u32)->Result<(),String> {
    for (name,n) in [("level",level),("home_row",row*2),("home_col",col*2),("x",row*2),("y",col*2)] {p.set(g,&monster(k,name),n)?}
    if let Some(target)=cached_occupancy(g,level,row as i16,col as i16)? {
        let flags=p.read(g,&target)?;
        if flags&63!=0 && flags&63!=k as u32 {return Err(format!("npc_move/{k}: destination occupied by record {}",flags&63))}
        p.set(g,&target,(flags&192)|k as u32)?;
    }
    Ok(())
}
fn npc_changes(v:&Value,_a:&Assets,g:&mut Ground,p:&mut Pokes)->Result<(),String> {
    if let Some(attitude)=match string(&v["npcs"],"hostility","as_map") {"none"=>Some(2),"all_hostile"=>Some(0),_=>None} {
        for k in 1..=g.mem.npc_count() as usize {
            if p.read(g,&monster(k,"level"))?==g.mem.level() as u32 {let target=monster(k,"attitude");let flags=p.read(g,&target)?;p.set(g,&target,(flags&!3)|attitude)?}
        }
    }
    for n in array(&v["npcs"],"set") {if n.get("cell").is_some() || n.get("level").is_some() || n["present"]==false {clear_npc(g,p,num(n,"record",0) as usize)?}}
    for n in array(&v["npcs"],"set") {
        let k=num(n,"record",0) as usize;
        if n["present"]==false {
            let id=p.read(g,&monster(k,"id"))?;
            if id>=400 {return Err(format!("npc_remove/{k}: id outside saved kill-bit span"))}
            let target=format!("party.object_bits+{}",id/8);let flags=p.read(g,&target)?;
            p.set(g,&target,flags|(1<<(id&7)))?;p.set(g,&monster(k,"level"),20)?;continue
        }
        let level=num(n,"level",p.read(g,&monster(k,"level"))? as i64) as u32;
        if n["present"]==true && level==20 {return Err(format!("npcs/set/{k}/present: removed record needs a level"))}
        if n.get("cell").is_some() || n.get("level").is_some() {
            let row=n["cell"][0].as_u64().unwrap_or(p.read(g,&monster(k,"x"))? as u64/2) as u32;
            let col=n["cell"][1].as_u64().unwrap_or(p.read(g,&monster(k,"y"))? as u64/2) as u32;
            move_npc(g,p,k,row,col,level)?;
        }
        if let Some(hostile)=n["hostile"].as_bool() {let target=monster(k,"attitude");let flags=p.read(g,&target)?;p.set(g,&target,(flags&!3)|if hostile {0} else {2})?}
        for field in ["hits","weapon","ammo"] {if let Some(x)=n[field].as_u64() {p.set(g,&monster(k,field),x as u32)?}}
    }
    Ok(())
}

fn spawn(v:&Value,a:&Assets,g:&mut Ground,p:&mut Pokes,ext:&mut Extensions)->Result<(),String> {
    for n in array(v,"npc") {
        let role=string(n,"role","");let sheet=a.roles["role"].get(role).ok_or("npc/role: unknown")?;
        let k=num(n,"record",g.mem.npc_count() as i64+1) as usize;
        clear_npc(g,p,k)?;
        let old_id=if k<=g.mem.npc_count() as usize {p.read(g,&monster(k,"id"))? as u16}
            else {(0..400u16).find(|id|(1..=g.mem.npc_count()).all(|j|g.mem.read(Mem::npc(j as i16),2)!=*id as u32)).ok_or("npc_spawn: no free saved NPC id")?};
        let weapon=num(n,"weapon",num(&sheet["equipment"],"weapon",255)) as usize;
        let item=a.item(weapon,None,"npc/weapon")?;
        let skill_name=a.roles["weapon_skill"][item[0x25].to_string()].as_str().unwrap_or("");
        let skill=sheet["skills"].as_object().into_iter().flatten().filter(|(name,_)|
            name.as_str()==skill_name || a.roles["skill_alias"][name.as_str()].as_str()==Some(skill_name)).filter_map(|(_,v)|v.as_u64()).max().unwrap_or(0);
        let level=num(n,"level",num(&v["place"],"level",0)) as u8;
        let row=n["at"][0].as_u64().ok_or("npc/at: row")? as u16;
        let col=n["at"][1].as_u64().ok_or("npc/at: col")? as u16;
        let attitude=match string(n,"attitude",string(sheet,"attitude","neutral")) {"hostile"=>0,"police"=>1,_=>2};
        let mut bytes=[0u8;46];
        bytes[0..2].copy_from_slice(&old_id.to_le_bytes());bytes[2]=level;
        for offset in [3,32] {bytes[offset..offset+2].copy_from_slice(&(row*2).to_le_bytes())}
        for offset in [5,34] {bytes[offset..offset+2].copy_from_slice(&(col*2).to_le_bytes())}
        bytes[7]=(num(n,"leash",0) as u8)<<3;bytes[8]=2;bytes[9]=num(sheet,"picture",0) as u8;
        bytes[10]=num(n,"hits",(skill*8).clamp(8,254) as i64) as u8;
        bytes[13]=attitude;bytes[26..28].copy_from_slice(&u16::MAX.to_le_bytes());
        bytes[30]=num(n,"ammo",((num(&sheet["equipment"],"clips",0)+1)*item[0x28] as i64).min(255)) as u8;
        bytes[31]=weapon as u8;bytes[36]=n["facing"].as_u64().map(|v|v as u8).unwrap_or_else(||g.rand.random(8) as u8);
        bytes[37]=(g.rand.random(5)+5) as u8;bytes[39]=1;bytes[40]=1;bytes[41]=bytes[11].wrapping_mul(10);
        p.bytes(g,&monster(k,"raw"),&bytes)?;
        if k>g.mem.npc_count() as usize {p.set(g,"t.monster_count",k as u32)?}
        if let Some(target)=cached_occupancy(g,level as u32,row as i16,col as i16)? {
            let flags=p.read(g,&target)?;
            if flags&63!=0 {return Err(format!("npc_spawn/{k}: destination occupied by record {}",flags&63))}
            p.set(g,&target,(flags&192)|k as u32)?;
        }
        if p.expected.contains_key(&monster(k,"attitude")) {p.set(g,&monster(k,"attitude"),attitude as u32)?}
        ext.npc_sheets.insert(k,json!({"role":role,"id":n.get("id"),"sheet":sheet}));
    }
    Ok(())
}
fn encounters(v:&Value,a:&Assets,g:&Ground,ext:&mut Extensions)->Result<(),String> {
    ext.encounter_defs=a.game_encounters(&v["place"])?;
    for e in array(v,"encounter") {
        if let Some(i)=ext.encounter_defs.iter().position(|old|old["id"]==e["id"]) {ext.encounter_defs[i]=e.clone()}
        else {ext.encounter_defs.push(e.clone())}
    }
    for e in &ext.encounter_defs {
        if e["end"]!="locked" {continue}
        let records:Vec<usize>=if let Some(ps)=e["participants"].as_array() {
            ps.iter().filter_map(|v|v.as_u64().map(|n|n as usize).or_else(||ext.npc_sheets.iter().find(|(_,s)|s["id"]==*v).map(|(&k,_)|k))).collect()
        } else {(1..=g.mem.npc_count() as usize).collect()};
        if !records.iter().any(|&k|{let n=Mem::npc(k as i16);g.mem.read(n.add(2),1)==g.mem.level() as u32 && g.mem.read(n.add(13),1)&3==0}) {
            if array(v,"encounter").any(|n|n["id"]==e["id"]) {return Err(format!("encounter {}: locked needs a hostile participant",string(e,"id","")))}
        }
    }
    Ok(())
}
fn vehicle_apply(v:&Value,g:&mut Ground,p:&mut Pokes)->Result<(),String> {
    let model=num(v,"type",0) as u32;
    let bytes=g.files().read("C.DAT").ok_or("vehicle: cannot read C.DAT")?;
    let pictures=mt_engine::data::pic::decode_set(mt_engine::data::Variant::Mt1,&bytes).map_err(|e|format!("vehicle: C.DAT {e:?}"))?;
    let picture=pictures.get(model as usize).ok_or("vehicle: missing rental picture")?;
    if !render::rental_picture(g,model as u8) {return Err("vehicle: cannot load C.DAT rental picture".into())}
    for (name,value) in [("height",picture.h),("width",picture.wbytes4)] {
        let target=format!("t.vehicle_picture_{name}");
        if p.read(g,&target)?!=value as u32 {return Err(format!("{target}: picture read-back mismatch"))}
        p.expected.insert(target,value as u32);
    }
    p.set(g,"t.vehicle_picture_id",model)?;
    p.set(g,"t.vehicle_model",model)?;p.set(g,"t.vehicle_rented",1)?;
    p.set(g,"t.vehicle_condition",num(v,"condition",100) as u32)?;
    let boarded=v["boarded"].as_bool().unwrap_or(true);
    let row=g.mem.x()/2+if boarded {0} else {2};let col=g.mem.y()/2;let target=occupancy(g,row,col)?;
    let flags=p.read(g,&target)?;
    if flags&63!=0 {return Err("vehicle: party cell occupied by an NPC".into())}
    p.set(g,"t.vehicle_row",row as u32)?;p.set(g,"t.vehicle_col",col as u32)?;
    p.set(g,&target,flags|64)?;
    p.set(g,"t.vehicle",if boarded {model} else {0})?;
    Ok(())
}

pub fn attach_scene(g:&mut Ground) {
    g.scene=Some(mt1::ui::SceneHook::default());
    g.input.scene(&mt1::ui::Scene::program(mt1::ui::ScreenKind::Picture));
}

pub struct Loader {
    pub request:Request,
    pub dir:PathBuf,
    pub extensions:Extensions,
    location:Option<(u8,u8,i8)>,
    live_clock:bool,
    pub last_error:Option<String>,
}
fn log(g:&mut Ground,line:String) {if let Some(s)=g.squad.as_mut() {s.p1.log(line)}}
impl Loader {
    pub fn new(request:Request,dir:PathBuf,extensions:Extensions)->Self {Self {request,dir,extensions,location:None,live_clock:false,last_error:None}}
    pub fn desktop(request:Request,dir:PathBuf,extensions:Extensions)->Self {Self {live_clock:true,..Self::new(request,dir,extensions)}}
    #[allow(dead_code)]
    pub fn desktop_request(&mut self,request:Request) {self.request=request;self.live_clock=true;}
}
impl Loader {
    #[allow(dead_code)]
    pub fn resume_at(dir:PathBuf,location:(u8,u8,i8))->Self {
        Self {request:Request::default(),dir,extensions:Extensions::default(),location:Some(location),live_clock:false,last_error:None}
    }
}
impl ground::Tooling for Loader {
    fn space_launched(&mut self,sp:&mut mt1::space::Space)->Result<(),String> {
        let pending=std::mem::take(&mut self.extensions.pending_space);
        if pending.is_empty() {return Ok(())}
        space::apply_pending(sp,&pending).map_err(|e|format!("scenario launch: {e}"))
    }
    fn space_seed(&mut self,prog:&str)->Option<u16> {self.extensions.pending_srand.remove(prog).map(|s|s as u16)}
    fn pending(&self)->bool {self.request.borrow().is_some()}
    fn last_error(&self)->Option<String> {self.last_error.clone()}
    fn poll(&mut self,g:&mut Ground)->bool {
        let request=self.request.borrow_mut().take();
        if let Some(path)=request {
            let stem=path.file_stem().map(|s|s.to_string_lossy().into_owned()).unwrap_or_default();
            log(g,format!("Loading scenario {stem}..."));
            match load(&path,&self.dir) {
                Err(e)=>{eprintln!("{e}");log(g,format!("Scenario load failed: {e}"));self.last_error=Some(e)},
                Ok(mut prepared)=>{
                    if let Err(e)=prepared.require_playable() {eprintln!("{e}");log(g,format!("Scenario load failed: {e}"));self.last_error=Some(e);return false}
                    prepared.report();
                    std::mem::swap(&mut prepared.game.input,&mut g.input);
                    if self.live_clock {
                        let now = prepared.game.clock.now();
                        std::mem::swap(&mut prepared.game.clock, &mut g.clock);
                        prepared.game.clock.set(now);
                    }
                    prepared.game.mouse_driver=g.mouse_driver;
                    prepared.game.balance=g.balance;
                    prepared.game.save_slot=g.save_slot.take();
                    prepared.game.input.flush();
                    if g.scene.is_some() {attach_scene(&mut prepared.game)}
                    let loaded=prepared.game.squad.take();
                    prepared.game.squad=g.squad.take().map(|old|loaded.unwrap_or_else(||mt1::ground::squad::State::with_compendium(old.compendium)));
                    self.extensions=prepared.extensions;
                    *g=prepared.game;
                    self.location=Some((g.mem.party_u8(6),g.mem.party_u8(7),g.mem.level()));
                    squad::install_defs(g,&self.extensions.encounter_defs,&self.extensions.npc_sheets);
                    self.last_error=None;log(g,format!("{} loaded",prepared.label));
                    return true
                }
            }
        }
        let location=(g.mem.party_u8(6),g.mem.party_u8(7),g.mem.level());
        if self.location.is_none_or(|old|old!=location) {
            if self.location.is_some() || self.extensions.encounter_defs.is_empty() {
                if self.location.is_some() {self.extensions.encounter_defs.clear();self.extensions.npc_sheets.clear();}
                match Assets::load(&self.dir).and_then(|a|a.game_encounters(&json!({"world":location.0,"area":location.1,"level":location.2}))) {
                    Ok(defs)=>self.extensions.encounter_defs=defs,Err(e)=>eprintln!("scenario encounter_defs: {e}")
                }
            }
            squad::install_defs(g,&self.extensions.encounter_defs,&self.extensions.npc_sheets);
        }
        self.location=Some(location);false
    }
}

#[cfg(test)]
mod tests;
#[cfg(test)]
mod panel_tests;
