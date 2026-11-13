use super::*;
fn status(g:&mut Ground)->(i32,i32) {
    let m=g.input.mouse();let (mut y,mut x)=(m.row as i32,m.col as i32);
    if g.mem.u8(0x7012)!=0 && y>46 && x<=192 {if g.mem.i16(0x6FFE)<=46 {y=46} else {x=193}}
    g.mem.set_u16(0x6FF8,m.buttons);g.mem.set_i16(0x6FFE,y as i16);g.mem.set_i16(0x6FFC,x as i16);(y,x)
}
fn pixels(g:&Ground)->bool {g.scene.is_none()}
fn save(g:&mut Ground,y:i32,x:i32,second:bool) {if pixels(g) {copy(g,0x503A,y,x>>3,8.min(200-y),2,0x7158,120,if second {2} else {0})}}
fn restore(g:&mut Ground,y:i32,x:i32) {if pixels(g) {copy(g,0x7158,120,0,8.min(200-y),2,0x503A,y,x>>3)}}
fn glyph(g:&mut Ground,y:i32,x:i32) {let draw=pixels(g);let mut p=Paint::new(g,Raster::Clip);if draw {p.ly("FONT",10,y,x)} else {(p.pen.colour,p.pen.op)=crate::ui::CURSOR_PEN}p.finish();}
fn begin(g:&mut Ground)->bool {let change=g.mem.u16(0x7198)==0;g.mem.set_u8(0x7010,change as u8);if change {front(g)}change}
pub fn cursor_on(g:&mut Ground) {
    if g.mem.u16(0x7000)!=0 {g.enter("cursor_show");}
    if g.mem.u16(0x7000)!=0 && g.mem.cursor_visible()==0 {
        g.mem.set_cursor_visible(1);let (y,x)=status(g);g.mem.set_i16(0x700C,y as i16);g.mem.set_i16(0x700E,x as i16);
        let changed=begin(g);save(g,y,x,false);glyph(g,y,x);if changed {back(g)}
    }
    if g.mem.u16(0x7000)!=0 {frame(g,"cursor_show");}
}
pub fn cursor_off(g:&mut Ground) {
    if g.mem.u16(0x7000)!=0 {g.enter("cursor_hide");}
    hide(g);
    if g.mem.u16(0x7000)!=0 {frame(g,"cursor_hide");}
}
pub(super) fn hide(g:&mut Ground) {
    if g.mem.cursor_visible()!=0 {
        let changed=begin(g);restore(g,g.mem.i16(0x700C) as i32,g.mem.i16(0x700E) as i32);
        g.mem.set_cursor_visible(0);g.mem.set_i16(0x700C,-1);g.mem.set_i16(0x700E,-1);if changed {back(g)}
    }
}
pub fn cursor_track(g:&mut Ground) {
    if g.mem.cursor_visible()==0 || g.mem.u16(0x7000)==0 {return}
    let (y,x)=status(g);if (y,x)==(g.mem.i16(0x700C) as i32,g.mem.i16(0x700E) as i32) {return}
    let changed=begin(g);restore(g,g.mem.i16(0x700C) as i32,g.mem.i16(0x700E) as i32);
    g.mem.set_i16(0x700C,y as i16);g.mem.set_i16(0x700E,x as i16);save(g,y,x,false);glyph(g,y,x);if changed {back(g)}
}
pub fn flip(g:&mut Ground) {
    let (y,x)=status(g);let visible=g.mem.cursor_visible()!=0;
    if visible {back(g);save(g,y,x,true);glyph(g,y,x)}
    let page=(g.mem.u16(0x52F4)^1)&1;g.mem.set_u16(0x52F4,page);back(g);
    if visible {restore(g,g.mem.i16(0x700C) as i32,g.mem.i16(0x700E) as i32);if pixels(g) {copy(g,0x7158,120,2,8,2,0x7158,120,0)}g.mem.set_i16(0x700C,y as i16);g.mem.set_i16(0x700E,x as i16)}
}
pub struct SelectionBlink {pub rectangles:Far,pub count:usize,pub selected:usize,pub color:u8,pub highlight:u8,pub phase:bool,pub deadline:u32}
impl SelectionBlink {
    pub fn new(g:&mut Ground,rectangles:Far,count:usize,selected:usize,color:u8,highlight:u8)->Self {
        let selected=if selected<count {selected} else {0};let deadline=g.now().wrapping_add(9);
        cursor_off(g);front(g);if count>0 {outline(g,rectangles,selected,color)}cursor_on(g);
        Self {rectangles,count,selected,color,highlight,phase:true,deadline}
    }
    pub fn poll(&mut self,g:&mut Ground) {cursor_track(g);let now=g.now();if self.count>0 && now>self.deadline {cursor_off(g);front(g);self.deadline=now.wrapping_add(9);outline(g,self.rectangles,self.selected,if self.phase {self.highlight} else {self.color});self.phase=!self.phase;cursor_on(g)}}
    pub fn choose(&mut self,g:&mut Ground,i:usize) {if self.count==0 {return}cursor_off(g);front(g);if self.phase {outline(g,self.rectangles,self.selected,self.highlight)}self.selected=i%self.count;outline(g,self.rectangles,self.selected,self.color);self.phase=true;self.deadline=g.now().wrapping_add(9);cursor_on(g)}
}
#[cfg(test)]
mod t {
mod oracle_env { include!("../../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir,Files,Input,MouseState},present::textlog,sim::tick::Counter};
    use std::{cell::Cell,rc::Rc};
    struct Mouse(Rc<Cell<MouseState>>);
    impl Input for Mouse {fn next_key(&mut self)->u16 {13}fn mouse(&mut self)->MouseState {self.0.get()}}
    #[test]
    fn product_cursor_leaves_pixels_and_text_cells_alone() {
        let Some(dir)=oracle_env::gamefiles() else {return};let dir=Dir(dir);let m=Rc::new(Cell::new(MouseState {col:100,row:100,buttons:0}));
        let mut g=Ground::new(&dir.read("T.DAT").unwrap(),dir.read("DEFAULT.SAV").unwrap(),Box::new(Mouse(m.clone())),Box::new(Counter::default()),Box::new(dir)).unwrap();
        g.mem.set_pointer(0x7168,Far::new(0,0xA200));g.mem.set_pointer(0x716C,Far::new(0,0xA000));g.mem.set_u16(0x52F4,0);g.mem.set_u16(0x7000,0xFFFF);
        for (i,v) in [0,8,16,24].into_iter().enumerate() {g.mem.set_u8(0x52C4+i as u16,v)}
        front(&mut g);g.scene=Some(crate::ui::SceneHook::default());textlog::set_enabled(true);
        {let mut p=Paint::new(&mut g,Raster::Clip);p.ink(15);p.at(100,96);p.text(b"Vehicle");p.finish();}
        let page=(g.mem.u16(0x52F4)&1) as usize;
        let (planes,cells)=(g.fb.planes.clone(),textlog::frame(&g.fb,page).visible);
        assert!(!cells.is_empty());
        cursor_on(&mut g);
        assert_eq!((g.mem.cursor_visible(),g.mem.i16(0x700C),g.mem.i16(0x700E)),(1,100,100));
        m.set(MouseState {col:104,row:103,buttons:0});cursor_track(&mut g);
        assert_eq!((g.mem.i16(0x700C),g.mem.i16(0x700E)),(103,104));
        flip(&mut g);cursor_off(&mut g);
        assert_eq!((g.mem.cursor_visible(),g.mem.i16(0x700C),g.mem.i16(0x700E)),(0,-1,-1));
        assert_eq!(g.fb.planes,planes,"no pixel of the 1990 cursor in product mode");
        assert_eq!(textlog::frame(&g.fb,page).visible,cells,"the text cells under the pointer survive");
        assert_eq!((g.mem.u8(0x52C8),g.mem.u8(0x52B2)),crate::ui::CURSOR_PEN);
        g.scene=None;g.mem.set_u8(0x52C8,7);cursor_on(&mut g);
        assert_ne!(g.fb.planes,planes);assert_eq!((g.mem.u8(0x52C8),g.mem.u8(0x52B2)),crate::ui::CURSOR_PEN);
        textlog::set_enabled(false);
    }
}
