use super::*;
use mt_engine::data::{ly as data_ly, pic as data_pic, Variant};
use mt_engine::present::{ly::{self, Pen}, pic, prim::Raster, text::{Cursor, Font}};
use std::collections::BTreeMap;

pub(crate) struct Paint<'a> {
    pub g: &'a mut Ground,
    pub raster: Raster,
    pub page: usize,
    pub pen: Pen,
    pub text: Cursor,
    font: Font,
    layouts: BTreeMap<&'static str, Vec<data_ly::Entry>>,
    sheets: BTreeMap<&'static str, Vec<data_pic::Pic>>,
    pub overlays: Vec<[i32; 7]>,
}
impl<'a> Paint<'a> {
    pub fn new(g: &'a mut Ground, raster: Raster) -> Self {
        let glyphs = g.files().read("FONT.LY").and_then(|b| data_ly::decode(&b).ok()).unwrap_or_default();
        let font = Font::new(glyphs, std::array::from_fn(|i| g.mem.u8(0x5128+i as u16)), &std::array::from_fn(|i| g.mem.u8(0x5637+i as u16)), 8);
        let m = &g.mem;
        let page = video_offset(m, 0x503A) / 8192;
        let pen = Pen { colour: m.u8(0x52C8), op: m.u8(0x52B2), ops: std::array::from_fn(|i| m.u8(0x52C4+i as u16)) };
        let text = Cursor { r: raster, page, y: m.i16(0x7139) as i64, x: m.i16(0x713B) as i64, pen: pen.colour, colour: m.u8(0x713E), shadow: m.u8(0x713F)&1!=0, bg: m.u8(0x707A), tab: m.u8(0x713D), tabs: [m.i16(0x5228) as i64,m.i16(0x522A) as i64], window_x: m.i16(0x6E08) as i64, ops: pen.ops };
        Self { g, raster, page, pen, text, font, layouts: BTreeMap::new(), sheets: BTreeMap::new(), overlays: vec![] }
    }
    pub fn finish(&mut self) {
        let m = &mut self.g.mem;
        m.set_u8(0x52C8,self.pen.colour); m.set_u8(0x52B2,self.pen.op);
        m.set_i16(0x7139,self.text.y as i16); m.set_i16(0x713B,self.text.x as i16);
        m.set_u8(0x713E,self.text.colour); m.set_u8(0x713F,(m.u8(0x713F)&!1)|self.text.shadow as u8); m.set_u8(0x707A,self.text.bg);
    }
    pub fn ly(&mut self, name: &'static str, i: usize, y: i32, x: i32) {
        if name=="FONT" { ly::layout(&mut self.g.fb,self.raster,self.page,&self.font.glyphs,i,y,x,&mut self.pen); return }
        if !self.layouts.contains_key(name) {
            let e=self.g.files().read(&format!("{name}.LY")).and_then(|b| data_ly::decode(&b).ok()).unwrap_or_default(); self.layouts.insert(name,e);
        }
        ly::layout(&mut self.g.fb,self.raster,self.page,&self.layouts[name],i,y,x,&mut self.pen);
    }
    pub fn fill(&mut self,y:i32,x:i32,h:i32,w:i32,c:u8) { self.g.fb.fill(self.raster,self.page,y,x,h,w,c,self.pen.op) }
    pub fn line(&mut self,y:i32,x:i32,yy:i32,xx:i32,c:u8) { self.g.fb.line(self.raster,self.page,y,x,yy,xx,c,self.pen.op) }
    pub fn rect(&mut self,y:i32,x:i32,yy:i32,xx:i32,c:u8) { self.g.fb.rect(self.raster,self.page,y,x,yy,xx,c,self.pen.op) }
    pub fn ink(&mut self,c:u8) { self.pen.colour=c; self.text.ink(c);self.g.mem.set_u16(0x707B,c as u16) }
    pub fn at(&mut self,y:i32,x:i32) { self.text.at(y as i64,x as i64) }
    pub fn glyph(&mut self,c:u8) {
        self.text.pen=self.pen.colour; self.text.page=self.page;
        self.text.glyph(&mut self.g.fb,&self.font,c); self.pen.colour=self.text.pen; self.pen.op=0;
    }
    pub fn text(&mut self,t:&[u8]) {
        self.text.pen=self.pen.colour; self.text.page=self.page;
        self.text.text(&mut self.g.fb,&self.font,t); self.pen.colour=self.text.pen; self.pen.op=0;
    }
    pub fn string(&self,p:Far)->Vec<u8> { (0..65536).map(|i|self.g.mem.read(p.add(i),1) as u8).take_while(|&b|b!=0).collect() }
    pub fn pointed_text(&mut self,off:u16) { let t=self.string(self.g.mem.pointer(off)); self.text(&t) }
    pub fn shifted(&mut self,n:i32,shift:u8) { self.text(&n.to_string().bytes().map(|c|c.wrapping_add(shift)).collect::<Vec<_>>()) }
    pub fn counted(&mut self,c:u8,glyph:u8,n:u8,y:i32,x:i32) {
        self.ink(0); self.at(y,x+self.g.mem.u8(0x522C+glyph as u16) as i32); self.glyph(glyph);
        if glyph!=3 && n>0 { self.ink(c.wrapping_add(1)); self.at(y+1,x+self.g.mem.u8(0x5231+glyph as u16) as i32+i32::from(n==1)); if let Some(ch)=n.checked_add(0xB0) { self.glyph(ch) } self.ink(0) }
    }
    pub fn wrapped(&mut self,t:&[u8],y:i32,x:i32,w:i32,h:i32,lh:i32,c:u8,bg:u8) {
        self.ink(c);self.at(y,x); let (mut row,mut col,mut start)=(y as i64,x as i64,0);
        while start<t.len() {
            let stop=t[start+1..].iter().position(|&b|b==b' ').map_or(t.len(),|i|start+1+i);
            let mut word=&t[start..stop]; let mut width=word.iter().fold(0i64,|a,&b|a.wrapping_add(self.font.widths[b as usize] as i64));
            if col.wrapping_add(width)>x as i64+w as i64 {
                row=row.wrapping_add(lh as i64);col=x as i64;
                if word.first()==Some(&b' ') {word=&word[1..];width=width.wrapping_sub(self.font.widths[32] as i64);}
                if row.wrapping_add(lh as i64*2)>y as i64+h as i64 {self.text.at(row,col);self.text(b"(more)");self.fill(y,x,h,w,bg);row=y as i64;col=x as i64;}
                self.text.at(row,col);
            }
            self.text(word);col=col.wrapping_add(width);start=stop;
        }
    }
    pub fn picture(&mut self,name:&'static str,index:usize,y:i32,x:i32,sy:i32,sx:i32,h:i32,w:i32,transparent:Option<u8>) {
        if !self.sheets.contains_key(name) {
            let pics=self.g.files().read(name).and_then(|b|if name.ends_with(".DAT") {data_pic::decode_set(Variant::Mt1,&b).ok()} else {data_pic::decode(Variant::Mt1,&b).ok().map(|p|vec![p])}).unwrap_or_default();self.sheets.insert(name,pics);
        }
        if let Some(p)=self.sheets[name].get(index) {pic::draw_rect(&mut self.g.fb,self.page,p,y,x,sy,sx,h,w,transparent)}
    }
    pub fn cpic(&mut self,slot:usize,y:i32,x:i32,sy:i32,sx:i32,h:i32,w:i32) {
        self.source_rect(sy,sx,h,w);
        let i=if slot<2 {slot} else {self.g.mem.u8(0x4CDEu16.wrapping_add(slot as u16)) as usize+4};self.picture("C.DAT",i,y,x,sy,sx,h,w,Some(1));
    }
    pub fn item_icon(&mut self,i:u8,y:i32,x:i32) {
        let (i,n,w,big)=if i>=100 {(i as i32-100,6,48,1)} else {(i as i32,12,24,0)};
        self.source_rect(i/n*15,i%n*w,15,w);
        self.picture("OBJ.DAT",big,y,x,i/n*15,i%n*w,15,w,None);
    }
    fn source_rect(&mut self,sy:i32,sx:i32,h:i32,w:i32) {
        for (off,v) in [(0x719C,sy),(0x7196,sx/8),(0x719E,h),(0x71A0,w/8)] {self.g.mem.set_i16(off,v as i16)}
    }
    pub fn present(&mut self,y:i32,x:i32,h:i32,w:i32) {present(self.g,y,x,h,w)}
}
