use super::state::{Far, Ground, Mem};
use mt_engine::present::prim::Raster;
pub(crate) mod paint;
mod view;
mod cursor;
mod screens;
mod boot;
pub mod character;
pub use boot::{game_init,select_tileset,cpic_load};
pub use cursor::{cursor_on,cursor_off,cursor_track,flip,SelectionBlink};
pub use screens::{intro,intro_page,main_screen,party_bar,shop_portrait,walker_panel,status_panel,air_panel,select_panel,redraw_party,hp_bar,draw_party_order};
pub use view::redraw;
use paint::Paint;

fn video_offset(m:&Mem,slot:u16)->usize {let p=m.pointer(slot); (p.off as usize+p.seg as usize*16)&65535}
pub(crate) fn copy(g:&mut Ground,src:u16,sy:i32,sx:i32,h:i32,w:i32,dst:u16,dy:i32,dx:i32) {
    let (s,d)=(video_offset(&g.mem,src) as i64,video_offset(&g.mem,dst) as i64);
    if s%8192==0 && d%8192==0 {g.fb.copy(s as usize/8192,sy,sx,h.clamp(0,65536),w.clamp(0,65536),d as usize/8192,dy,dx);return}
    for r in 0..h.max(0).min(65536) as i64 {for c in 0..w.max(0).min(65536) as i64 {
        let a=((s+(sy as i64+r)*40+sx as i64+c)&65535) as usize;
        let b=((d+(dy as i64+r)*40+dx as i64+c)&65535) as usize;
        for p in 0..4 {g.fb.planes[p*65536+b]=g.fb.planes[p*65536+a];}
    }}
}
pub fn copy_region(g:&mut Ground,src:u16,sy:i32,sx:i32,h:i32,w:i32,dst:u16,dy:i32,dx:i32) {copy(g,src,sy,sx,h,w,dst,dy,dx)}
pub fn front(g:&mut Ground) {select_page(g,true)}
pub fn back(g:&mut Ground) {select_page(g,false)}
fn select_page(g:&mut Ground,front:bool) {
    let page=g.mem.u16(0x52F4)&1;
    g.mem.set_u16(0x7198,front as u16);
    g.mem.set_u16(0x52B3,0xA000+((if front {page} else {page^1})<<9));
    let p=g.mem.pointer(0x7168+4*(page^u16::from(front)));g.mem.set_pointer(0x503A,p);
}
pub fn present(g:&mut Ground,y:i32,xbyte:i32,h:i32,wbyte:i32) {
    let p=g.mem.u16(0x52F4)&1;copy(g,0x7168+p*4,y,xbyte,h,wbyte,0x7168+(p^1)*4,y,xbyte);
}
pub fn copy_front_to_back(g:&mut Ground,y:i32,xbyte:i32,h:i32,wbyte:i32) {
    let p=g.mem.u16(0x52F4)&1;copy(g,0x7168+(p^1)*4,y,xbyte,h,wbyte,0x7168+p*4,y,xbyte);
}
fn frame(g:&mut Ground,name:&str) {
    g.frame(name);
}

pub fn dismiss_message(g:&mut Ground,flag:i16) {
    g.enter("dismiss_message");
    if g.mem.u8(0x6C3C)==0 {
        g.mem.set_message_deadline(0);
        let mut p=Paint::new(g,Raster::Planar);p.ly("GROUND",2,166,0);p.finish();
        if flag!=0 {present(g,166,0,34,24)}
    }
    frame(g,"dismiss_message");
}
pub fn show_message(g:&mut Ground,s:Far) {
    let t=Paint::new(g,Raster::Planar).string(s);message(g,&t);
}
pub fn message(g:&mut Ground,t:&[u8]) {
    g.enter("message");
    g.boot_checkpoint("show_message");
    if g.harness {g.message_text(&t.iter().map(|&b|b as char).collect::<String>());}
    dismiss_message(g,0);let now=g.now();g.mem.set_message_deadline(now.wrapping_add(180));
    let mut p=Paint::new(g,Raster::Planar);p.text.shadow=true;p.wrapped(t,169,1,189,40,9,6,9);p.text.shadow=false;p.finish();
    present(g,169,0,31,24);frame(g,"message");
}

fn menu_paint(p:&mut Paint<'_>,menu:Far) {
    p.g.enter("location_box");p.ly("GROUND",4,141,193);p.ink(9);p.at(141,261);p.text(b"system:");p.at(155,261);p.text(b"planet:");p.ink(0);
    for (off,y) in [(0x5E1F,147),(0x5E20,161)] {p.at(y,266);let i=p.g.mem.u8(off);if i!=255 {p.pointed_text(0x15E1+i as u16*4)}}
    p.finish();p.g.frame("location_box");p.ink(0);
    for i in 0..p.g.mem.read(menu.add(32),1).min(6) as i32 {
        let ptr=Far::new(p.g.mem.read(menu.add(i*4),2) as u16,p.g.mem.read(menu.add(i*4+2),2) as u16);
        if ptr!=Far::default() {let t=p.string(ptr);let t=if p.g.scene.is_some() && t==b"ESC" {b"Back".to_vec()} else {t};p.ink(p.g.mem.read(menu.add(33+i),1) as u8);p.at(171+(i%3)*10,if i<3 {200} else {262});p.text(&t)}
    }
}
pub fn draw_menu(g:&mut Ground,menu:Far) {
    g.enter("draw_menu");cursor::hide(g);let mut p=Paint::new(g,Raster::Planar);menu_paint(&mut p,menu);p.finish();
    let h=Far::new(g.mem.read(menu.add(28),2) as u16,g.mem.read(menu.add(30),2) as u16);
    if h!=Far::default() {
        let t=Paint::new(g,Raster::Planar).string(h);
        if g.mem.u8(0x6C3C)!=0 {
            front(g);let mut p=Paint::new(g,Raster::Planar);p.ink(6);p.text.shadow=true;p.ly("GROUND",1,54,0);p.wrapped(&t,58,3,187,142,8,6,9);p.text.shadow=false;p.finish();back(g);
        } else {message(g,&t)}
    }
    frame(g,"draw_menu");
}
pub fn present_menu(g:&mut Ground) {g.enter("present_menu");cursor::hide(g);present(g,141,24,59,16);frame(g,"present_menu")}
pub fn fill_rect(g:&mut Ground,y:i32,x:i32,h:i32,w:i32,c:u8) {let mut p=Paint::new(g,Raster::Clip);p.fill(y,x,h,w,c);p.finish()}
pub fn item_icon(g:&mut Ground,index:u8,y:i32,x:i32) {let mut p=Paint::new(g,Raster::Planar);p.item_icon(index,y,x);p.finish()}
pub fn rental_picture(g:&mut Ground,index:u8)->bool {boot::load_picture(g,"C.DAT",index as usize,0x62B0,2)}
pub fn draw_line(g:&mut Ground,y1:i32,x1:i32,y2:i32,x2:i32,c:u8) {let mut p=Paint::new(g,Raster::Clip);p.line(y1,x1,y2,x2,c);p.finish()}
pub fn marker(g:&mut Ground,sy:i32,sx:i32,val:i32) {
    let mut p=Paint::new(g,Raster::Planar);p.ly("GROUND",0x30,sy,sx);
    if val!=-1 {p.at(sy+4,sx+5);p.ink(6);p.shifted(val,0x80)}
    p.finish();
}
pub fn vehicle_pic(g:&mut Ground,v:i32) {
    let mut p=Paint::new(g,Raster::Clip);p.fill(1,0x101,0x31,0x3b,6);
    let o=if v>0x18 {0} else {4};
    for (off,value) in [(0x719E,0x32),(0x71A0,7),(0x719C,o),(0x7196,o)] {p.g.mem.set_i16(off,value as i16)}
    p.picture("C.DAT",(v-1).max(0) as usize,1,0x20*8,o,o*8,0x32,7*8,None);p.finish();
}
pub fn outline(g:&mut Ground,rectangles:Far,index:usize,color:u8) {
    if index>8191 {return}
    let p=rectangles.add(index as i32*8);let a:Vec<i32>=(0..4).map(|i|g.mem.read_i(p.add(i*2),2)).collect();
    let mut p=Paint::new(g,Raster::Planar);p.rect(a[0],a[1],a[0]+a[2]-1,a[1]+a[3],color);p.pen.colour=color;p.finish();
}
