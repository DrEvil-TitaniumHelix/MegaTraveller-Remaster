use super::*;
use mt_engine::data::Variant;
impl Paint<'_> {
    fn gauge(&mut self,i:u16) {
        self.g.enter("draw_gauge");
        let b=i*256+0x102;let (cur,max)=(self.g.mem.party_u8(b+0x25) as i32,self.g.mem.party_u8(b+0x23) as i32);
        if cur!=0 {let len=if cur>max {45} else if max!=0 {cur*45/max} else {0};self.fill(3,i as i32*40+39,45-len,4,0);self.fill(48-len,i as i32*40+39,len,4,i as u8+1);
        }
        self.finish();self.g.frame("draw_gauge");
        if cur!=0 {let split=self.g.mem.party_u8(b+0x24) as i32;let marker=if split>max {45} else if max!=0 {split*45/max} else {0};self.fill(48-marker,i as i32*40+39,2,4,6);}
    }
    fn bars(&mut self,clear:bool) {
        for i in 0..5u16 {self.gauge(i)}
        for i in 0..5u16 {if self.g.mem.party_u8(i*256+0x127)!=0 {let o=self.g.mem.party_u8(i*256+0x128) as i32;let c=if self.g.mem.party_u16(i*256+0x1F4)==0 {0x78} else {6};self.fill(3,i as i32*40+34,45-o,4,0);self.fill(48-o,i as i32*40+34,o,4,c)}}
        if clear {self.present(0,0,53,40)}
    }
    fn item(&self,who:u16,slot:u16)->Option<(Far,Far)> {
        let i=self.g.mem.party_u16(who.wrapping_mul(256).wrapping_add(0x1CE).wrapping_add(slot*2)) as i16;if i<=0 {return None}
        let inst=self.g.mem.pointer(0x6D65).add(0xA82+(i as i32-1)*5);let d=self.g.mem.pointer(0x6D6D).add(self.g.mem.read(inst,1) as i32*46);Some((inst,d))
    }
    pub(super) fn hands(&mut self,i:u16) {
        if self.g.mem.party_u8(i*256+0x127)==0 {return}
        for (slot,y,entry) in [(19,17,20),(20,33,30)] {
            if let Some((_,d))=self.item(i,slot) {self.item_icon(self.g.mem.read(d.add(38),1) as u8,y,(i as i32*5+1)*8);self.line(y,i as i32*40+8,y+14,i as i32*40+8,0)} else {self.ly("TOP",i as usize+entry,y,i as i32*40+9)}
        }
    }
    fn order(&mut self) {
        let mut order:[u16;5]=std::array::from_fn(|i|self.g.mem.party_u8(i as u16+1) as u16);
        let alive=|p:&Paint<'_>,i:u16|p.g.mem.party_u8(i.wrapping_mul(256).wrapping_add(0x127))>p.g.mem.party_u8(i.wrapping_mul(256).wrapping_add(0x126));
        if !alive(self,order[0]) {for i in 1..5 {if alive(self,order[i]) {order.swap(0,i);self.g.fb.set_reg(10,self.g.mem.u8(0x111+order[0]));}}}
        if self.g.mem.combat()==0 {
            for (i,&member) in order.iter().enumerate() {let (y,x)=(self.g.mem.i16(0x29B+i as u16*4) as i32,self.g.mem.i16(0x29D+i as u16*4) as i32);
                if self.g.mem.party_u8(member.wrapping_mul(256).wrapping_add(0x127))!=0 {self.ly("TOP",member as usize+10,y+1,x+1)} else {self.fill(y+1,x+1,13,25,0)}}
            for (i,&member) in order.iter().enumerate() {let b=member.wrapping_mul(256);if self.g.mem.party_u8(b.wrapping_add(0x127))!=0 {self.counted(member as u8,self.g.mem.party_u8(b.wrapping_add(0x122)),self.g.mem.party_u8(b.wrapping_add(0x123)),self.g.mem.i16(0x29B+i as u16*4) as i32+2,self.g.mem.i16(0x29D+i as u16*4) as i32+2)}}
        } else {self.fill(1,256,50,60,6)}
    }
    pub(super) fn side_panel(&mut self,quiet:bool) {
        if self.g.mem.combat()==0 {if !quiet {self.ly("GROUND",3,54,193)}if self.g.mem.countdown()==100 {self.picture("OBJ.DAT",2,68,200,0,0,200,320,None)}return}
        let who=self.g.mem.leader() as u16;self.ly("GROUND",3,54,193);self.ly("GROUND",5,62,195);
        let secondary=self.g.mem.read(Mem::walker(who as i16).add(2),1) as u16;let item=self.item(who,secondary+21);
        if let Some((inst,d))=item {if self.g.mem.read(d.add(29),2)==2 {let kind=self.g.mem.read(d.add(39),1)&192;let glyph=if kind==64 {7} else if kind!=0 {8} else {0};if glyph!=0 {let n=self.g.mem.read(inst.add(2),1);let cap=self.g.mem.read(d.add(40),1);self.at(104,200);self.ink(10);for _ in 0..n {self.glyph(glyph)}self.ink(8);for _ in n..cap {self.glyph(glyph)}}}}
        self.ink(10);
        if let Some((inst,d))=item {self.item_icon(self.g.mem.read(d.add(38),1) as u8,79,200);if self.g.mem.read(d.add(29),2)==2 {self.at(81,284);self.shifted(self.g.mem.read(inst.add(1),1) as i32,160)}}
        for i in 0..3 {self.at(124,i*22+210);self.shifted(self.g.mem.party_u8(who.wrapping_mul(256).wrapping_add(0x1CB+i as u16)) as i32,160)}
        for i in 0..256u16 {let a=0x1FD+i*9;let color=self.g.mem.u8(a+8);if color==255 {break}let ptr=self.g.mem.pointer(a+4);
            let t=match i {0=>Some(self.string(self.g.mem.pointer(0x6D65).add(who as i32*256+0x102)).iter().map(|b|b.to_ascii_lowercase()).collect::<Vec<_>>()),1=>item.map(|(_,d)|self.string(d)),2=>Some(if secondary!=0 {b"secondary weapon".to_vec()} else {b"primary weapon".to_vec()}),_=>if ptr==Far::default() {None} else {Some(self.string(ptr))}};
            if let Some(t)=t {self.at(self.g.mem.i16(a) as i32,self.g.mem.i16(a+2) as i32);self.ink(color);self.text(&t)}
        }
    }
}
pub fn party_bar(g:&mut Ground,clear:bool) {let mut p=Paint::new(g,Raster::Clip);p.bars(clear);p.finish()}
pub fn hp_bar(g:&mut Ground,i:u16) {let mut p=Paint::new(g,Raster::Clip);p.gauge(i.min(4));p.finish()}
pub fn draw_party_order(g:&mut Ground) {let mut p=Paint::new(g,Raster::Clip);p.order();p.finish();let who=g.mem.party_u8(1);g.mem.set_selected_char(who)}
pub fn walker_panel(g:&mut Ground,quiet:bool,redraw:bool) {
    let mut p=Paint::new(g,Raster::Planar);p.side_panel(quiet);p.finish();
    if redraw {present(g,54,24,130,16)}
}
pub fn status_panel(g:&mut Ground) {cursor_off(g);present(g,0,0,53,40);cursor_on(g)}
pub fn air_panel(g:&mut Ground,clear:bool) {
    let mut any=false;
    let mut p=Paint::new(g,Raster::Planar);
    for i in 0..5u16 {if p.g.mem.party_u8(i*256+0x127)!=0 {
        any=true;let v=p.g.mem.party_u8(i*256+0x128) as i32;
        let c=if p.g.mem.party_u16(i*256+0x1F4)==0 {0x78} else {6};
        p.fill(3,i as i32*40+34,45-v,4,0);p.fill(48-v,i as i32*40+34,v,4,c);
    }}
    p.finish();if clear && any {status_panel(g)}
}
pub fn select_panel(g:&mut Ground,index:u16,value:u8) {
    if g.mem.u16(0x503E)!=2 && index<16 {g.fb.set_reg(index as usize,value);g.mem.set_u8(0x110+index,value)}
}
pub fn redraw_party(g:&mut Ground) {
    let who=if g.mem.combat()!=0 {g.mem.leader() as u16} else {g.mem.party_u8(1) as u16};
    select_panel(g,10,g.mem.u8(0x111u16.wrapping_add(who)));
}
pub fn main_screen(g:&mut Ground,arg:u16) {
    g.screen_kind(crate::ui::ScreenKind::Ground);g.enter("main_screen");
    let mut p=Paint::new(g,Raster::Clip);p.ly("TOP",9,0,0);p.ly("TOP",5,2,208);p.bars(arg!=0);p.pen.colour=0;
    for i in 0..5u16 {let b=i*256+0x102;if p.g.mem.party_u8(b+0x25)!=0 {p.counted(i as u8,p.g.mem.party_u8(b+0x20),p.g.mem.party_u8(b+0x21),4,i as i32*40+8)} else {p.fill(2,i as i32*40+7,46,36,0)}}
    p.finish();dismiss_message(p.g,0);p.side_panel(false);menu_paint(&mut p,Mem::ds(0x435B));
    let help=p.g.mem.pointer(0x435B+28);if help!=Far::default() {let t=p.string(help);if p.g.mem.u8(0x6C3C)==0 {p.ly("GROUND",2,166,0)}p.text.shadow=true;p.wrapped(&t,169,1,189,40,9,6,9);p.present(169,0,31,24);p.text.shadow=false;}
    p.order();for i in 0..5 {p.hands(i)}p.finish();
    let page=g.mem.u16(0x52F4)&1;copy(g,0x7168+(page^1)*4,54,0,112,24,0x7168+page*4,54,0);present(g,0,0,200,40);
    for (off,value) in [(0x717C,54),(0x7164,0),(0x7170,112),(0x7166,24)] {g.mem.set_u16(off,value)}
    g.mem.set_u8(0x6265,1);frame(g,"main_screen");
}
pub fn shop_portrait(g:&mut Ground,person:i16,room:i16) {
    g.enter("shop_portrait");
    let mut p=Paint::new(g,Raster::Clip);p.picture("Y.DAT",if (0..8).contains(&room) {room as usize} else {6},68,200,0,0,200,320,None);
    if person!=0 && (person<=24 || person>=54) {p.picture("Z.DAT",person.wrapping_sub(1) as usize,68,200,0,0,200,320,Some(14))}p.finish();frame(g,"shop_portrait");
}

pub fn intro_page(g:&mut Ground,index:usize) {
    if index>4 {return}
    g.screen_kind(crate::ui::ScreenKind::Intro);
    if index==0 {
        g.mem.set_party_u8(6,2);g.mem.set_party_u8(7,0);
        crate::ground::input::dialogue::set_text_box(g,2);back(g);
        let mut p=Paint::new(g,Raster::Planar);p.ly("GROUND",1,54,0);p.finish();
    }
    if index==0 || index==4 {
        let pic=g.files().read("START.PIC").and_then(|b|mt_engine::data::pic::decode(Variant::Mt1,&b).ok());
        if let Some(pic)=pic {
            for (off,value) in [(0x7196,if index==4 {14} else {0}),(0x719C,0),(0x719E,pic.h),(0x71A0,pic.wbytes4/2)] {g.mem.set_u16(off,value)}
            let mut p=Paint::new(g,Raster::Planar);p.fill(55,192,146,128,7);
            p.picture("START.PIC",0,60,200,0,if index==4 {112} else {0},pic.h as i32,pic.wbytes4 as i32/2*8,None);p.ink(6);p.finish();
            present(g,54,0,146,40);
        }
    }
    if let Err(e)=crate::ground::input::dialogue::say_message(g,index as i16,false) {eprintln!("ground: {e}");std::process::exit(3)}
}
pub fn intro(g:&mut Ground) {
    g.observe_boot("intro", &[]);
    if !super::boot::load_picture(g,"START.PIC",0,0x7240,1) {eprintln!("ground: START.PIC missing or invalid");std::process::exit(3)}
    for i in 0..5 {intro_page(g,i);g.get_key();}
    back(g);main_screen(g,0);g.mem.set_outdoors(1);
    let p=g.mem.pointer(0x6D65).add(0x88);g.mem.write(p,30000,4);
}
