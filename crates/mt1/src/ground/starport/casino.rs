use super::shop_menu as shop;
use crate::ground::{input::{self,ui},render,state::Ground,walk};

pub fn reel_symbol(g: &Ground, value: i16) -> i16 {
    for i in 0..12u16 {
        if g.mem.i16(0x209E+4*i)<=value && value<=g.mem.i16(0x20A0+4*i) {return i as i16+1}
    }
    value.wrapping_add(75)
}
pub fn award(reels: [i16;3], symbols: [i16;3]) -> u32 {
    let [a,b,c]=reels;let [x,y,z]=symbols;
    let (base,value)=if a==b && b==c {(3000,a)} else if a==b {(250,b)}
        else if x==y && y==z {(250,x)} else if x==y {(50,x)}
        else if y==z {(40,x)} else if x==z {(10,x)} else {(0,0)};
    (base as u32).wrapping_mul((value/10+1) as i32 as u32)
}
pub fn slot_spin(g: &mut Ground, reels: &mut [i16;3], mode: i16, skilled: bool) {
    let skill=g.mem.party_u8((g.mem.selected_char() as u16*256).wrapping_add(0x142)).min(9) as i16;
    if mode==0 {reels[0]=walk::random(g,78);render::item_icon(g,reels[0] as u8,70,48)}
    let (mut lo,mut width)=(0,78);
    if skilled && skill>0 {lo=(reels[0].wrapping_sub((10-skill)*2)).max(0);width=(10-skill)*4;if lo.wrapping_add(width)>78 {width=78i16.wrapping_sub(lo)}}
    if mode<2 {reels[1]=walk::random(g,width).wrapping_add(lo);render::item_icon(g,reels[1] as u8,70,72)}
    reels[2]=walk::random(g,width).wrapping_add(lo);render::item_icon(g,reels[2] as u8,70,96);
}
fn text(g: &mut Ground, text: &[u8], y: i32, x: i32) {
    ui::ink(g,6);g.mem.set_u8(0x713F,g.mem.u8(0x713F)|1);ui::text(g,text,y,x);g.mem.set_u8(0x713F,g.mem.u8(0x713F)&!1);
}
fn wait(g: &mut Ground, line: u8, again: bool) -> bool {
    if g.scene.is_none() {
        crate::play_prompts::named(g,"1AC9C",&[("spin",if again {"Spin again"} else {"Play slots"},32),("back","Leave",27)],&[]);
        return input::menu_getkey(g)==27
    }
    let spin=format!("Spin ({})",super::money(g,100));let hot=shop::choice_words(g,line,spin.as_bytes(),b"Leave");
    g.frame("casino_choices");
    crate::play_prompts::named(g,"1AC9C",&[("spin",&spin,83),("back","Leave",76)],&hot);
    matches!(input::menu_getkey(g),27|76|108)
}
pub fn run(g: &mut Ground) {
    let who=g.mem.selected_char() as i16;shop::set_box(g,2);
    if (g.mem.u32(0x6D13) as i32)<=0 {
        shop::say(g,b"Sorry, the Naasirka Casino is temporarily closed pending additional funding.",true,6);
        input::menu_getkey(g);shop::set_box(g,0);return;
    }
    if shop::cash(g,who)<100 {shop::say(g,b"You don't have enough money to play.",true,6);input::menu_getkey(g);shop::set_box(g,0);return}
    shop::say(g,format!("Welcome to the Naasirka casino. Press the spacebar to play space slots ({} a spin), or the ESC key to exit",super::money(g,100)).as_bytes(),true,6);
    let under=g.mem.u8(0x6E2A)+1;
    if wait(g,under,false) {return}
    shop::clear(g);shop::layout(g,7);text(g,b"N A A S I R K A",108,44);
    loop {
        let mut leave_after_spin=false;
        let mut shown_tick=None;
        render::fill_rect(g,189,0,10,160,9);
        text(g,format!("{} has {}",shop::name(g,who),super::money(g,shop::cash(g,who))).as_bytes(),190,2);
        for (y,x,h,w,c) in [(126,36,39,95,9),(69,48,1,73,8),(70,120,16,1,8),(85,48,1,73,8)] {render::fill_rect(g,y,x,h,w,c)}
        let mut reels=[0;3];
        for (mode,duration) in [20,13,13].into_iter().enumerate() {
            let end=g.now().wrapping_add(duration);
            while g.now()<end {
                slot_spin(g,&mut reels,mode as i16,false);
                if g.scene.is_some() {
                    let tick=g.now();
                    if shown_tick!=Some(tick) { g.frame("casino_spin"); shown_tick=Some(tick); }
                }
                g.poll();
                if g.scene.is_some() && g.mem.key()==1 { leave_after_spin=true; }
            }
            slot_spin(g,&mut reels,mode as i16,true);
        }
        let win=award(reels,reels.map(|v|reel_symbol(g,v)));
        shop::set_cash(g,who,shop::cash(g,who).saturating_sub(100));g.mem.set_u8(0x6E2A,10);
        let product=g.scene.is_some();
        if win==0 {shop::say(g,if product {b"         No win this time"} else {b"         You pay the money"},false,6);
            if product {shop::say(g,format!("         You lose {}",super::money(g,100)).as_bytes(),false,6)} else {shop::say(g,b"         Take the chances",false,6)}}
        else {
            shop::say(g,b"         We have a winner!!",false,6);shop::say(g,if product {format!("         You win {}",super::money(g,win))} else {format!("                 {}",super::money(g,win))}.as_bytes(),false,6);
            shop::set_cash(g,who,shop::cash(g,who).wrapping_add(win));g.mem.set_u32(0x6D13,g.mem.u32(0x6D13).wrapping_sub(win));
            if (g.mem.u32(0x6D13) as i32)<=0 {input::menu_getkey(g);shop::clear(g);shop::say(g,b"You have broken the bank! Sorry you can't play anymore, but we will have to close the Casino for now.",true,6);input::menu_getkey(g);break}
        }
        if g.scene.is_some() { g.frame("casino_result"); }
        if leave_after_spin { break; }
        let under=g.mem.u8(0x6E2A)+1;
        if wait(g,under,true) {break}
        if shop::cash(g,who)==0 {shop::clear(g);shop::say(g,b"You have no more money. You'll have to Leave.",true,6);input::menu_getkey(g);break}
    }
    shop::set_box(g,0);
}
