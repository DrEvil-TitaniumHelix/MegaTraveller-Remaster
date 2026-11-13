use super::schema::{self, Field};
use mt1::ground::{Ground, state::{Far, Mem}};

pub fn resolve(fields: &[Field], target: &str) -> Result<(String, usize), String> {
    let (p, k) = target.rsplit_once('+').map_or(Ok((target, 0)), |(p,k)|
        k.parse::<usize>().map(|k|(p,k)).map_err(|_| "invalid poke byte index".to_string()))?;
    if let Some(prog) = p.strip_prefix("rng.") {
        if k < 4 && ["STARTUP", "CG", "T", "S"].contains(&prog) { return Ok((p.into(), k)) }
    }
    if let Some(f) = fields.iter().find(|f| f.path == p) {
        if k < f.width && !p.ends_with("_ptr") { return Ok((p.split('.').next().unwrap_or(p).into(), f.offset + k)) }
    }
    Err(format!("unmapped symbolic poke {target}"))
}

pub fn ground_byte(g: &mut Ground, section: &str, at: usize, value: u8) -> Result<(), String> {
    match section {
        "party" if at < Mem::PARTY_SIZE => g.mem.set_party_u8(at as u16, value),
        "t" if at < 65536 => g.mem.set_u8(at as u16, value),
        "rng.T" if at < 4 => {
            g.rand.lcg.seed = (g.rand.lcg.seed & !(255 << (at*8))) | ((value as u32) << (at*8));
            g.mem.set_u32(0x5c20, g.rand.lcg.seed);
        }
        _ => return Err(format!("unmodeled poke {section}+{at}")),
    }
    Ok(())
}

pub fn write(g:&mut Ground, fields:&[Field], target:&str, value:u8)->Result<(),String> {
    let (path,k)=target.rsplit_once('+').map_or(Ok((target,0)),|(p,k)|k.parse::<usize>().map(|k|(p,k)).map_err(|_|"invalid poke byte index".to_string()))?;
    if let Some(p)=indirect(g,path) {
        if k!=0 {return Err(format!("{target}: byte outside indirect target"))}
        g.mem.write(p,value as u32,1);return Ok(())
    }
    let (section,at)=resolve(fields,target)?;
    ground_byte(g,&section,at,value)
}

pub fn emit_requested(t:&mut serde_json::Value,g:&Ground,fields:&[Field],target:&str) {
    use serde_json::{Value,json};
    let path=target.split('+').next().unwrap_or(target);
    let Some(tail)=path.strip_prefix("t.") else {return};
    if let Some(rest)=tail.strip_prefix("monsters[") {
        let Some((i,name))=rest.split_once("].") else {return};
        let Ok(i)=i.parse::<usize>() else {return};
        let Some(monster)=t["monsters"].get_mut(i) else {return};
        if monster.get(name).is_some() {return}
        if let Some(f)=fields.iter().find(|f|f.path==path && f.width<=4) {monster[name]=g.mem.read(Mem::ds(f.offset as u16),f.width).into()}
    } else if tail.starts_with("office_open[") {
        t["office_open"]=json!((0..20).map(|e|g.mem.u8(0x5dfb+e)).collect::<Vec<_>>());
    } else if let Some(p)=indirect(g,path) {
        if let Some(rest)=tail.strip_prefix("maps[") {
            if let Some((level,rest))=rest.split_once("].cells[") {
                if let Some(i)=rest.strip_suffix("].attr") {t["maps"][level]["cells"][i]["attr"]=g.mem.read(p,1).into()}
            }
        } else if let Some(i)=tail.strip_prefix("map_cells[").and_then(|s|s.strip_suffix("].attr")) {t["map_cells"][i]["attr"]=g.mem.read(p,1).into()}
    } else if !tail.contains('.') && t.get(tail).is_none() {
        if let Some(f)=fields.iter().find(|f|f.path==path && f.width<=4) {t[tail]=Value::from(g.mem.read(Mem::ds(f.offset as u16),f.width))}
    }
}

pub fn catalog() -> Vec<Field> {
    let s = schema::schema();
    let mut f = schema::fields(&s["properties"]["party"], "party", 0);
    f.extend(schema::fields(&s["properties"]["t"], "t", 0));
    let mut add = |path: String, offset, width| f.push(Field {path,offset,width,text:false,hex:false});
    for (name,off,width) in [("world_tech",0x5e1b,1),("vehicle_condition",0x6d23,1),
        ("vehicle_rented",0x6d31,1),("vehicle_model",0x6d30,1),("vehicle_row",0x6419,1),
        ("vehicle_col",0x63f4,1),("vehicle_picture_id",0x2e8,1),("vehicle_picture_height",0x62b0,2),("vehicle_picture_width",0x62b2,2)] { add(format!("t.{name}"),off,width) }
    for i in 0..32 {
        for (name,off,width) in [("id",0,2),("level",2,1),("home_row",3,2),("home_col",5,2),
            ("type",7,1),("move",8,1),("picture",9,1),("hits",10,1),("rank",11,1),
            ("attitude",13,1),("token",26,2),("ammo",30,1),("weapon",31,1),("x",32,2),
            ("y",34,2),("facing",36,1),("steps",37,1),("reactive",38,1),("turn",39,1),
            ("animation",40,1),("morale",41,1)] {
            add(format!("t.monsters[{i}].{name}"),0x63f4+(i+1)*46+off,width);
        }
        add(format!("t.monsters[{i}].raw"),0x63f4+(i+1)*46,46);
    }
    for e in 11..=19 { add(format!("t.office_open[{e}]"),0x5dfb+e,1) }
    add("t.starport_area".into(),0x6d20,1); add("t.squad_broken".into(),0x6d0e,2);
    for i in 0..5 {
        for (name,off,width) in [("active",0,1),("facing",1,1),("hand",2,1),("builtin",3,1),("target",5,1),("timer",6,1),("x",8,2),("y",10,2),("aim_x",12,2),("aim_y",14,2),
            ("dest_x",16,2),("dest_y",18,2),("phase",20,2),("route_len",22,1)] { add(format!("t.walkers[{i}].{name}"),0x5c54+i*86+off,width) }
        add(format!("t.walkers[{i}].raw"),0x5c54+i*86,86);
    }
    for i in 0..8 { for k in 0..3 { add(format!("party.chars[{i}].grenades[{k}]"),0x102+256*i+0xc9+k,1) } }
    for n in 0..128 { for (name,off,width) in [("item",0,2),("next",2,1),("glyph",3,1),("row",4,2),("col",6,2),("spawn",8,1)] { add(format!("party.ground[{n}].{name}"),0x602+9*n+off,width) } }
    f
}

pub fn indirect(g: &Ground, path: &str) -> Option<Far> {
    if path == "t.world_law" { let p=g.mem.pointer(0x5e02); return (p!=Far::default()).then(||p.add(6)) }
    if let Some(rest) = path.strip_prefix("t.maps[") {
        let (level, rest) = rest.split_once("].cells[")?;
        let level = level.parse::<u16>().ok()?;
        let i = rest.strip_suffix("].attr")?.parse::<usize>().ok()?;
        if level >= 8 { return None }
        let ptr = g.mem.pointer(0x606e + level * 4);
        let header = 0x6096 + 26 * level;
        let n = g.mem.u8(header) as usize * 12 * g.mem.u8(header + 1) as usize * 20;
        return (ptr != Far::default() && i < n).then(|| ptr.add((3*i+2) as i32));
    }
    let i = path.strip_prefix("t.map_cells[")?.strip_suffix("].attr")?.parse::<usize>().ok()?;
    let n = g.mem.map_stride().max(0) as usize * g.mem.map_blocks_high() as usize * 12;
    (i < n).then(|| g.mem.pointer(0xbc).add((3*i+2) as i32))
}
