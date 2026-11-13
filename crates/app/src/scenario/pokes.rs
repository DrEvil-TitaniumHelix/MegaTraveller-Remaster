use crate::lockstep::{schema, symbolic};
use mt1::ground::{Ground,state::Mem};
use serde_json::{Value,json};
use std::collections::BTreeMap;

pub struct Pokes {
    fields:Vec<schema::Field>,
    pub expected:BTreeMap<String,u32>,
}
impl Pokes {
    pub fn new()->Self {Self {fields:symbolic::catalog(),expected:BTreeMap::new()}}
    pub fn read(&self,g:&Ground,path:&str)->Result<u32,String> {
        if let Some(p)=symbolic::indirect(g,path) {return Ok(g.mem.read(p,1))}
        if path=="rng.T" {return Ok(g.rand.lcg.seed)}
        let (section,at)=symbolic::resolve(&self.fields,path)?;
        let width=if path.contains('+') {1} else {self.fields.iter().find(|f|f.path==path).map_or(0,|f|f.width)};
        if width==0 || width>4 {return Err(format!("{path}: use byte targets for a byte span"))}
        Ok(g.mem.read(if section=="party" {g.mem.pointer(0x6d65).add(at as i32)} else {Mem::ds(at as u16)},width))
    }
    pub fn set(&mut self,g:&mut Ground,path:&str,value:u32)->Result<(),String> {
        if symbolic::indirect(g,path).is_some() {
            if value>255 {return Err(format!("{path}: byte overflow"))} symbolic::write(g,&self.fields,path,value as u8)?;
        } else {
            let width=if path=="rng.T" {4} else if path.contains('+') {1}
                else {self.fields.iter().find(|f|f.path==path).ok_or_else(||format!("{path}: unknown symbolic target"))?.width};
            if width>4 || (width<4 && value>=1u32<<(width*8)) {return Err(format!("{path}: value does not fit target"))}
            for k in 0..width {
                let target=if path.contains('+') {path.to_string()} else {format!("{path}+{k}")};
                symbolic::write(g,&self.fields,&target,(value>>(8*k)) as u8)?;
            }
        }
        if self.read(g,path)?!=value {return Err(format!("{path}: write read-back mismatch"))}
        self.expected.insert(path.into(),value);Ok(())
    }
    pub fn bytes(&mut self,g:&mut Ground,path:&str,bytes:&[u8])->Result<(),String> {
        for (i,&b) in bytes.iter().enumerate() {self.set(g,&format!("{path}+{i}"),b as u32)?} Ok(())
    }
    pub fn proof(&self,g:&Ground)->Result<Vec<(String,Value)>,String> {
        let mut out=vec![];
        for (p,&expected) in &self.expected {
            let got=self.read(g,p)?;
            if got!=expected {return Err(format!("{p}: final read-back mismatch (expected {expected}, got {got})"))}
            out.push((p.clone(),got.into()));
        }
        Ok(out)
    }

}

pub fn record(g:&Ground)->Value {
    let s=schema::schema();
    let mut t=schema::decode(&s["properties"]["t"],&g.mem.ds);
    t["world_law"]=g.mem.read(g.mem.pointer(0x5e02).add(6),1).into();
    t["world_tech"]=g.mem.u8(0x5e1b).into();
    t["vehicle_condition"]=g.mem.vehicle_condition().into();
    t["vehicle_rented"]=g.mem.u8(0x6d31).into();
    t["vehicle_model"]=g.mem.u8(0x6d30).into();
    t["office_open"]=json!((0..20).map(|i|g.mem.u8(0x5dfb+i)).collect::<Vec<_>>());
    let n=g.mem.map_stride().max(0) as usize*g.mem.map_blocks_high() as usize*12*3;
    t["map_sha1"]=schema::sha1(&g.mem.bytes(g.mem.pointer(0xbc),n)).into();
    t["monsters"]=json!((1..=32).map(|i| {
        let p=Mem::npc(i);
        json!({"level":g.mem.read(p.add(2),1),"x":g.mem.read_i(p.add(32),2),"y":g.mem.read_i(p.add(34),2),
            "attitude":g.mem.read(p.add(13),1),"hits":g.mem.read(p.add(10),1),"weapon":g.mem.read(p.add(31),1),
            "ammo":g.mem.read(p.add(30),1),"raw":schema::hex(&g.mem.bytes(p,46))})
    }).collect::<Vec<_>>());
    let fields=symbolic::catalog();
    for field in &fields {symbolic::emit_requested(&mut t,g,&fields,&field.path)}
    json!({"v":1,"step":0,"key":null,"prog":"T","rng":{"STARTUP":null,"CG":null,"T":g.rand.lcg.seed,"S":null},
        "party":schema::decode(&s["properties"]["party"],g.mem.party()),"t":t,"bios":{"ticks":g.mem.read(Mem::BIOS_TICKS,4)}})
}
