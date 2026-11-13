use super::*;
use mt_engine::data::{pic, Variant};

fn asset_block(g:&mut Ground,size:usize)->Option<Far> {
    if size>65536 {return None}
    let mut start=0x10000usize;
    for (&seg,data) in &g.mem.segs {
        let base=seg as usize*16;
        if base>=0x30000 {break}
        if start+size<=base {break}
        if base+data.len()>start {start=(base+data.len()+15)&!15}
    }
    if start+size>0x30000 {return None}
    let p=Far::new(0,(start/16) as u16);g.mem.alloc(p.seg,size);Some(p)
}

fn load_layout(g:&mut Ground,name:&str,dst:u16)->bool {
    use mt_engine::data::{ly,bin::Record};
    let Some(file)=g.files().read(name) else {return false};
    let Ok(entries)=ly::decode(&file) else {return false};
    let size=5+entries.len()*4+entries.iter().map(|e|e.len()*10+1).sum::<usize>();
    let Some(p)=asset_block(g,size) else {return false};let seg=p.seg;
    let mut bytes=vec![0;5+entries.len()*4];bytes[0]=entries.len() as u8;
    bytes[1..3].copy_from_slice(&5u16.to_le_bytes());bytes[3..5].copy_from_slice(&seg.to_le_bytes());
    for (i,entry) in entries.iter().enumerate() {
        if bytes.len()+entry.len()*10+1>65536 {return false}
        let offset=bytes.len() as u16;
        bytes[5+i*4..7+i*4].copy_from_slice(&offset.to_le_bytes());
        bytes[7+i*4..9+i*4].copy_from_slice(&seg.to_le_bytes());
        for record in entry {record.write(&mut bytes)}
        bytes.push(0);
    }
    g.mem.segs.insert(seg,bytes);g.mem.set_pointer(dst,Far::new(0,seg));true
}

fn load_dictionary(g:&mut Ground)->bool {
    let Some(mut data)=g.files().read("TRAV.DAT") else {return false};
    let Ok(dict)=mt_engine::data::dict::decode(&data) else {return false};
    if data.len()>65536 || dict.words.len()>2050 {return false}
    let Some(bytes)=asset_block(g,data.len()) else {return false};
    let Some(pointers)=asset_block(g,0x2008) else {return false};
    let mut table=vec![0;0x2008];let mut offset=0usize;
    for (i,word) in dict.words.iter().enumerate() {
        table[4*i..4*i+2].copy_from_slice(&(offset as u16).to_le_bytes());
        table[4*i+2..4*i+4].copy_from_slice(&bytes.seg.to_le_bytes());
        offset+=word.len();data[offset]=0;offset+=1;
    }
    g.mem.segs.insert(bytes.seg,data);g.mem.segs.insert(pointers.seg,table);
    g.mem.set_pointer(0x6D69,bytes);g.mem.set_pointer(0x6D71,pointers);true
}

pub(crate) fn load_items(g:&mut Ground)->bool {
    let Some(file)=g.files().read("TRAV.ITM") else {return false};
    let Some(header)=file.get(..2) else {return false};
    let count=u16::from_le_bytes([header[0],header[1]]) as usize;
    let Some(source)=file.get(2..2+count*46) else {return false};
    let mut items=source.to_vec();
    for record in items.chunks_exact_mut(46) {
        for byte in record.iter_mut().take_while(|b| **b!=0) {
            let flag=0x5637u16.wrapping_add(*byte as i8 as i16 as u16);
            if g.mem.u8(flag)&2!=0 {*byte=byte.wrapping_add(128)}
        }
    }
    g.mem.segs.insert(0x9100,items);g.mem.set_pointer(0x6D6D,Far::new(0,0x9100));true
}

fn load_sounds(g:&mut Ground)->bool {
    if g.mem.u8(0x4B1A)==0 {
        if g.mem.i16(0x4B18)==-1 {g.mem.set_u16(0x4B18,g.mem.u16(0x55AD).wrapping_add(2))}
        g.mem.set_u8(0x4B1A,1);
    }
    let table=g.mem.pointer(0x4B12);let mut count=0;
    for i in 0..19u16 {
        let entry=table.add(i as i32*6);
        if g.mem.read(entry.add(4),2)==0 {break}
        count=i+1;
        let name=Far::new(g.mem.read(entry,2) as u16,g.mem.read(entry.add(2),2) as u16);
        let name=g.mem.cstr(name);
        let file=match g.files().read_limited(&name,64_128) {Ok(f)=>Some(f),Err(e) if e.kind()==std::io::ErrorKind::InvalidData=>None,Err(_)=>continue};
        let Some(data)=file.as_deref().and_then(|f|f.get(128..)).filter(|b|b.len()<=64000) else {
            g.mem.set_u32(0x6F00+4*i,0);g.mem.set_pointer(0x6F4C+4*i,Far::default());continue
        };
        let Some(p)=asset_block(g,data.len()) else {return false};
        g.mem.write_bytes(p,data);g.mem.set_pointer(0x6F4C+4*i,p);g.mem.set_u32(0x6F00+4*i,data.len() as u32);
    }
    g.mem.set_u16(0x4A98,count);true
}

pub fn cpic_load(g:&mut Ground,id:u8,slot:u8)->bool {
    if id>=10 || !(2..7).contains(&slot) {return false}
    g.mem.set_u8(0x4D8A,id+4);
    let old=g.mem.u8(0x6FC6+id as u16);
    if old==slot {return true}
    let evicted=g.mem.u8(0x4CDE+slot as u16);
    if evicted<10 {g.mem.set_u8(0x6FC6+evicted as u16,0)}
    let dst=0x6298+24*slot as u16;
    if old==0 {
        g.mem.set_u8(0x4CE5,1);
        if !load_picture(g,"C.DAT",id as usize+4,dst,2) {return false}
    } else if old<7 {
        let src=0x6298+24*old as u16;let bytes=g.mem.bytes(Mem::ds(src),24);
        g.mem.write_bytes(Mem::ds(dst),&bytes);g.mem.set_u8(0x4CDE+old as u16,20);g.mem.set_pointer(src+8,Far::default());
    } else {return false}
    g.mem.set_u8(0x6FC6+id as u16,slot);g.mem.set_u8(0x4CDE+slot as u16,id);true
}

pub(super) fn load_picture(g: &mut Ground, name: &str, index: usize, dst: u16, mode: u16) -> bool {
    let Some(bytes) = g.files().read(name) else { return false };
    let picture = if name.ends_with(".DAT") {pic::decode_set(Variant::Mt1,&bytes).ok().and_then(|p|p.get(index).cloned())}
        else {pic::decode(Variant::Mt1,&bytes).ok()};
    let Some(p) = picture else { return false };
    g.mem.write_bytes(Mem::ds(0x524E),&p.pal);
    for (off,value) in [(0,p.h),(2,p.wbytes4),(4,1),(6,4)] {g.mem.set_u16(dst+off,value)}
    if name=="X.DAT" {g.boot_checkpoint("tileset_loaded")}
    for (off,value) in [(0,p.h),(2,p.wbytes4),(4,4),(6,1),(16,1),(18,1)] {g.mem.set_u16(dst+off,value)}
    if mode!=0 {g.mem.set_u16(dst+12,p.h);g.mem.set_u16(dst+14,p.wbytes4)}
    true
}

pub fn select_tileset(g:&mut Ground,name:&[u8]) {
    let n=name.iter().position(|&b|b==0).unwrap_or(name.len());let name=&name[..n];
    if n>12 {return}
    g.enter("select_tileset");
    let first=g.mem.cstr(Mem::ds(0x6F98));let second=g.mem.cstr(Mem::ds(0x6FA5));
    let slot=if first.as_bytes()==name {0} else if second.as_bytes()==name {1} else if first.is_empty() {0} else if second.is_empty() {1} else {0};
    if [first.as_bytes(),second.as_bytes()][slot]!=name {
        let index=if name.first().is_some_and(|b|b.eq_ignore_ascii_case(&b'I')) {3} else {name.get(1).copied().unwrap_or(b'1').wrapping_sub(b'1') as usize};
        if !load_picture(g,"X.DAT",index,0x63FB,0) {return}
        let target=g.mem.pointer(0x503A);g.mem.set_pointer(0x503A,g.mem.pointer(0x7150+slot as u16*4));
        let mut p=Paint::new(g,Raster::Planar);p.picture("X.DAT",index,0,0,0,0,200,320,None);p.finish();
        g.mem.write_bytes(Mem::ds(0x6F98+slot as u16*13),name);g.mem.set_u8(0x6F98+slot as u16*13+n as u16,0);
        g.mem.set_pointer(0x503A,target);
    }
    g.mem.write_bytes(Mem::ds(0x6FB3),name);g.mem.set_u8(0x6FB3+n as u16,0);
    g.mem.set_pointer(0x6B5F,g.mem.pointer(0x7150+slot as u16*4));
    g.frame("select_tileset");
}

fn graphics(g: &mut Ground) {
    for (off,value) in [(0x503E,0),(0x5042,4),(0x5040,1),(0x717C,0),(0x7164,0),
        (0x7170,200),(0x7166,40),(0x7194,0xE2AC),(0x719A,0xE274),(0x7192,0xCB5E),(0x52B3,0xA000)] {g.mem.set_u16(off,value)}
    for i in 0..8 {g.mem.set_pointer(0x7144+i*4,Far::new(0,0xA000+i*0x200))}
    g.mem.set_pointer(0x7168,Far::new(0,0xA200));g.mem.set_pointer(0x716C,Far::new(0,0xA000));
    g.mem.set_u8(0x71A2,8);g.mem.set_u16(0x52F4,0);
    for (i,v) in [0,8,16,24].into_iter().enumerate() {g.mem.set_u8(0x52C4+i as u16,v)}
}
fn device_init(g:&mut Ground) {for i in 0..16 {select_panel(g,i,g.mem.u8(0x110+i))}}
fn fill_screen(g:&mut Ground) {let mut p=Paint::new(g,Raster::Planar);p.fill(0,0,200,320,0);p.finish()}

pub fn game_init(g:&mut Ground)->bool {
    g.observe_boot("game_init",&[]);
    let exists=g.files().read("T.DAT").is_some() && g.files().read("W.DAT").is_some();
    g.mem.set_u8(0x6D2A,u8::from(exists));
    g.mem.set_u8(0x6248,b'0');g.mem.set_u8(0x6B72,2);
    g.mem.set_pointer(0x7238,Far::new(0x762F,0x1000));
    graphics(g);
    g.mem.set_pointer(0x503A,g.mem.pointer(0x7148));fill_screen(g);
    g.mem.set_pointer(0x503A,g.mem.pointer(0x7144));fill_screen(g);
    g.mem.set_npc_count(4);g.mem.set_party_u8(0x81,0);g.mem.set_i16(0x6B73,-1);g.mem.set_i16(0x6413,-1);
    for off in [0x6B63,0x6D20,0x6B76] {g.mem.set_u8(off,0)}
    device_init(g);if !load_layout(g,"FONT.LY",0x7131) {return false}g.mem.set_u8(0x713D,b'^');
    if !g.landing && !load_items(g) {return false}
    g.mem.set_sound_enabled(g.mem.read(g.mem.pointer(0x6D6D).add(0x1223),1) as u8);
    if !load_dictionary(g) {return false}
    let mut source=0x1659;
    for i in 0..7 {let n=g.mem.u8(0x1651+i);g.mem.set_u8(0x6D34+i*7,n);for j in 0..n as u16 {g.mem.set_u8(0x6D35+i*7+j,g.mem.u8(source));source+=1}}
    if g.mem.read(g.mem.pointer(0x6D6D).add(0x11F5),1)!=0 {g.mem.set_debug(1)}
    if !g.landing {g.mem.set_sound_enabled(1)}
    if !g.scenario_boot && !g.landing {g.mem.set_party_u8(6,0);g.mem.set_party_u8(7,0);}
    g.mem.set_u8(0x504C,1);front(g);fill_screen(g);back(g);device_init(g);
    g.mem.set_u16(0x7000,if g.mouse_driver {0xFFFF} else {0});g.mem.set_u16(0x7002,if g.mouse_driver {2} else {0x4CE6+4*11});
    g.mem.set_cursor_visible(0);g.mem.set_u16(0x7012,1);
    g.mem.set_pointer(0x6B6E,g.mem.pointer(0x6D65).add(0x602));
    g.mem.set_u8(0x5E1F,255);g.mem.set_u8(0x5E20,255);
    g.mem.set_pointer(0x6B5B,g.mem.pointer(0x714C));g.mem.set_pointer(0x6B5F,g.mem.pointer(0x7150));
    for (off,value) in [(0x717C,54),(0x7164,0),(0x7170,112),(0x7166,24)] {g.mem.set_u16(off,value)}
    for (name,dst) in [("LED.LY",0x6261),("GROUND.LY",0x641C),("TOP.LY",0x63F7)] {if !load_layout(g,name,dst) {return false}}
    for (index,dst) in [0x6268,0x6280,0x6230,0x6B77,0x6249].into_iter().enumerate() {
        if !load_picture(g,"OBJ.DAT",index,dst,if index==3 {2} else {0}) {return false}
    }
    g.mem.set_pointer(0x503A,g.mem.pointer(0x714C));
    let mut p=Paint::new(g,Raster::Planar);p.picture("OBJ.DAT",4,0,0,0,0,200,320,None);p.finish();
    back(g);main_screen(g,0);
    g.mem.set_pointer(0x503A,g.mem.pointer(0x7168+(g.mem.u16(0x52F4)&1)*4));
    redraw_party(g);
    if g.mem.party_u8(0x85)>0 {for (src,dst) in [(0x84,0x6FE6),(0x85,0x6FE8),(0x86,0x6FE7),(0x87,0x6FE9)] {g.mem.set_u8(dst,g.mem.party_u8(src))}}
    if !load_picture(g,"C.DAT",0,0x6298,2) || !load_picture(g,"C.DAT",4,0x62C8,2) {return false}
    g.mem.set_u8(0x4D8A,4);g.mem.set_u8(0x4CE0,0);g.mem.set_u8(0x4CE5,1);g.mem.set_u8(0x6FC6,2);
    g.mem.set_u8(0x4DD7,b'A');g.mem.set_u8(0x4DE4,b'C');
    if g.mem.party_u8(0xC1)&128!=0 {intro(g)}
    if g.sound_option!=b'A' {
        if g.sound_option==b'C' {g.mem.set_i16(0x4B18,-1)}
        message(g,b"Please wait, loading digitized sounds ...");g.mem.set_u8(0x6B68,1);
        if !load_sounds(g) {return false}
    }
    if !crate::ground::walk::boot(g) {return false}
    select_panel(g,15,6);g.mem.set_u32(0x4606,g.mem.u32(0xEA+g.mem.sound_enabled() as u16*4));
    true
}
