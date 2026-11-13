use crate::ground::{input::ui, render, state::Ground, walk};
use mt_engine::{data::ly as data_ly, present::{ly::{self, Pen}, prim::Raster}};

pub use crate::ground::input::dialogue::set_text_box as set_box;
pub use super::{cash, put_cash as set_cash};
use super::ui::font_bytes;
pub fn layout(g: &mut Ground, index: usize) {
    let Some(data) = g.files().read("STPT.LY") else { return };
    let Ok(entries) = data_ly::decode(&data) else { return };
    let mut pen = Pen::new(g.mem.u8(0x52C8)); let page = ui::page(g);
    ly::layout(&mut g.fb, Raster::Wrap, page, &entries, index, 0, 0, &mut pen);
    g.mem.set_u8(0x52C8, pen.colour); g.mem.set_u8(0x52B2, pen.op);
}
fn at(g: &mut Ground, y: i32, x: i32) { g.mem.set_i16(0x7139,y as i16); g.mem.set_i16(0x713B,x as i16) }
fn puts(g: &mut Ground, s: &[u8]) { ui::text(g,s,g.mem.i16(0x7139) as i32,g.mem.i16(0x713B) as i32) }
fn shadow(g: &mut Ground, on: bool) { let f=g.mem.u8(0x713F);g.mem.set_u8(0x713F, if on {f|1} else {f&!1}) }
pub fn limits(g: &mut Ground, a: i16, b: i16) { g.mem.set_i16(0x5228,a);g.mem.set_i16(0x522A,b) }
pub fn clear(g: &mut Ground) {
    at(g,g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32);
    if g.mem.i16(0x6E0A)==1 {
        render::back(g);layout(g,7);render::present(g,0,0,200,40);render::front(g);
    } else {render::fill_rect(g,g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32,g.mem.i16(0x6E06) as i32,g.mem.i16(0x6E00) as i32,9)}
}
pub fn say(g: &mut Ground, mut s: &[u8], reset: bool, color: u8) {
    if reset {g.mem.set_u8(0x6E2A,0)}
    let width=(g.mem.i16(0x6E00)/6) as u8 as usize;
    if width==0 {return}
    shadow(g,true);
    loop {
        let n=s.len().min(width);
        let end=if n==width {(1..width).rev().find(|&i|s[i]==b' ').unwrap_or(0)} else {n};
        at(g,g.mem.u8(0x6E2A) as i32*8+g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32);
        ui::ink(g,color);puts(g,&s[..end]);g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(1));
        if n!=width {break} s=&s[(end+1).min(s.len())..];
    }
    shadow(g,false);
}
pub fn drawn_words(g: &mut Ground, line: &[u8], words: &[&[u8]]) -> Vec<[i32; 4]> {
    if g.lockstep.is_some() || !g.input.wants_decision() { return vec![] }
    let Some(font) = ui::font(g) else { return vec![] };
    let (y, x) = ((g.mem.u8(0x6E2A) as i64 - 1) * 8 + g.mem.i16(0x6E0E) as i64, g.mem.i16(0x6E08) as i64);
    let mut fb = mt_engine::present::fb::Fb::new();
    words.iter().map(|w| {
        let Some(at) = line.windows(w.len().max(1)).position(|s| s == *w) else { return [0; 4] };
        let mut c = ui::cursor(g); c.at(y, x); c.text(&mut fb, &font, &line[..at]); let x0 = c.x; c.text(&mut fb, &font, w);
        [x0 as i32, y as i32, (c.x - x0) as i32, 8]
    }).collect()
}
pub fn choice_words(g: &mut Ground, line: u8, a: &[u8], b: &[u8]) -> Vec<[i32; 4]> {
    let x=g.mem.i16(0x6E08);let old=(g.mem.i16(0x5228),g.mem.i16(0x522A));limits(g,x+110,x+110);g.mem.set_u8(0x6E2A,line);
    let words=[b"  ",a,b"^",b].concat();
    say(g,&words,false,10);let hot=drawn_words(g,&words,&[a,b]);limits(g,old.0,old.1);hot
}
pub fn name(g: &Ground, who: i16) -> String {g.mem.cstr(super::party(g,who as i32*256+0x102))}
pub fn skill_bonus(g: &mut Ground) -> i16 {
    super::brokerage(g).wrapping_add((g.mem.party_u8((g.mem.party_u8(1) as u16*256).wrapping_add(0x11B))/5) as i16)
}
pub fn price(base: u32, percent: i16) -> u32 {(base.wrapping_mul(percent as i32 as u32) as i32 / 100) as u32}

pub struct Menu<'a> {
    pub title: &'a str, pub names: &'a [String], pub prices: &'a [u32],
    pub quantities: &'a mut [u8], pub limits: &'a [u8], pub rows: usize,
    pub select_only: u8, pub money: u32, pub max_load: u8, pub weights: Option<&'a [u8]>,
    pub kind: u8, pub sign: i8, pub colors: Option<&'a [u8]>,
}
fn hotkeys(g: &mut Ground, names: &[String], first: usize, end: usize) {
    for i in 0..26 {g.mem.set_u8(0x6E10+i,0)}
    for (i,name) in names.iter().enumerate().take(end).skip(first) {
        let Some(c)=name.bytes().take_while(|&c|c<128).find(u8::is_ascii_alphabetic) else {continue};
        let off=0x6E10+(c.to_ascii_uppercase()-b'A') as u16;
        if g.mem.u8(off)==0 {g.mem.set_u8(off,if c.is_ascii_lowercase() {(i-first+1) as u8} else {(i+1) as u8})}
    }
}
fn line_clear(g: &mut Ground) {
    let x=g.mem.i16(0x713B) as i32;
    render::fill_rect(g,g.mem.i16(0x7139) as i32,x,8,g.mem.i16(0x6E00) as i32-(x-g.mem.i16(0x6E08) as i32),9);
}
fn draw(g: &mut Ground, m: &Menu<'_>, first: usize, end: usize, more: bool, money: u32, color: u8) {
    render::cursor_off(g);if g.mem.i16(0x6E0A)==1 {render::back(g)}
    let (x,y,lines)=(g.mem.i16(0x6E08) as i32,g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E04) as i32);
    shadow(g,true);at(g,y,x);say(g,&font_bytes(m.title),true,10);ui::ink(g,color);
    if m.money!=0 {at(g,y+8,x);line_clear(g);shadow(g,true);puts(g,&font_bytes(format!("credits: {money}$     ")));shadow(g,false)}
    shadow(g,false);
    if more {at(g,(lines+1)*8+y,x);ui::ink(g,10);puts(g,b"<<more>>")}
    at(g,y+32,x);hotkeys(g,m.names,first,end);
    for i in first..end {
        if m.prices[i]!=0 {
            line_clear(g);
            if m.quantities[i]==0 {
                ui::ink(g,m.colors.and_then(|c|c.get(i)).copied().unwrap_or(6));
                shadow(g,m.colors.is_some());puts(g,&font_bytes(format!("|{}",m.names[i])));shadow(g,false);
            } else {
                puts(g,&font_bytes(format!("   \u{9}|{}",m.names[i])));
                let old=(g.mem.i16(0x5228),g.mem.i16(0x522A));
                if m.kind<97 {limits(g,210,260);puts(g,&font_bytes(format!("^/ {}$",m.prices[i].wrapping_mul(m.quantities[i] as u32) as i32)))}
                else {limits(g,(x+165) as i16,(x+165) as i16)}
                if m.quantities[i]>1 {puts(g,&font_bytes(format!("^{:2}",m.quantities[i])))} else {line_clear(g)}
                limits(g,old.0,old.1);
            }
        } else {puts(g,&font_bytes(format!(" {}",m.names[i])))}
        at(g,g.mem.i16(0x7139) as i32+8,x);
    }
    if g.mem.i16(0x6E0A)==1 {render::flip(g)}
}
fn select(g: &mut Ground, m: &Menu<'_>, first: usize, end: usize, mut current: usize, more: bool) -> i32 {
    g.mem.set_u8(0x6E2D,1);
    loop {
        let (x,y)=(g.mem.i16(0x6E08) as i32,g.mem.i16(0x6E0E) as i32);
        at(g,(current-first+4) as i32*8+y,x);ui::ink(g,10);puts(g,&[5]);
        let mut key=super::getkey(g);
        if key==5000 {return 5000}
        key=match key {1|3=>b'8' as u16,2|4=>b'2' as u16,_=>key};
        at(g,g.mem.i16(0x6E04) as i32*8+y,x);line_clear(g);
        match key {13=>return current as i32+1000,43=>return current as i32+1,45=>return -(current as i32+1),27=>return 0,_=>{}}
        if (5..=8).contains(&key) && m.rows!=m.names.len() {return current as i32*100+key as i32+1996}
        render::fill_rect(g,(current-first+4) as i32*8+y,x,8,6,9);
        let hot=if key<=255 && (key as u8).is_ascii_alphabetic() {g.mem.u8(0x6E10+((key as u8).to_ascii_uppercase()-b'A') as u16)} else {0};
        let mut next=current as i32;
        if hot!=0 {next=hot as i32-1}
        else if key==b'8' as u16 {next-=1;while next>=first as i32 && m.prices[next as usize]==0 {next-=1}}
        else if key==b'2' as u16 {next+=1;while next<end as i32 && m.prices[next as usize]==0 {next+=1}}
        if next<first as i32 && first!=0 {return 997}
        if next<0 {next=end as i32-1;while next>=first as i32 && m.prices[next as usize]==0 {next-=1}}
        else if next>=end as i32 {
            if more {return 998} next=first as i32;while next<end as i32 && m.prices[next as usize]==0 {next+=1}
        }
        if next>=first as i32 && next<end as i32 {current=next as usize}
    }
}
pub fn run(g: &mut Ground, m: Menu<'_>) -> usize {
    let n=m.names.len();
    if n==0 || n>255 || m.rows==0 || m.prices.len()<n || m.quantities.len()<n || m.limits.len()<n {return 0}
    let Some(mut current)=m.prices[..n].iter().position(|&p|p!=0) else {return 0};
    render::cursor_off(g);let color=g.mem.u8(0x713E);ui::ink(g,6);clear(g);m.quantities[..n].fill(0);
    let mut first=0;let mut end=m.rows.min(n);let mut more=end<n;
    hotkeys(g,m.names,first,end);
    if g.mem.u8(0x6E2B)!=0 {
        current=(g.mem.u8(0x6E2B) as usize-1).min(n-1);first=current/m.rows*m.rows;end=(first+m.rows).min(n);
        if first+m.rows>n {more=false}
    }
    let mut enabled=true;let mut money=m.money;let mut load=0u8;
    if m.select_only==0 && g.mem.i16(0x6E0A)!=0 {
        if g.mem.i16(0x6E0A)==1 {render::back(g)}
        ui::ink(g,6);at(g,g.mem.i16(0x6E04) as i32*8+g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32);
        puts(g,b"+/- amount  ENTER buys  ESC exits");enabled=false;
    }
    render::cursor_on(g);
    loop {
        draw(g,&m,first,end,more,money,color);
        if g.mem.i16(0x6E0A)==1 {render::front(g)}
        crate::play_lists::shop(g,&m,current,first,more,money,load);
        render::cursor_on(g);let mut action=select(g,&m,first,end,current,more);g.mem.set_u8(0x6E2D,0);render::cursor_off(g);
        if action==5000 {
            let row=((g.mem.i16(0x6E0C)-g.mem.i16(0x6E0E))>>3)-4;
            g.mem.set_i16(0x6E0C,row);
            if row<0 {action=current as i32*100+2001;g.mem.set_i16(0x6E0C,0)}
            else if row as usize>=end-first {action=current as i32*100+2002;g.mem.set_i16(0x6E0C,0)}
            else if m.prices[first+row as usize]!=0 {current=first+row as usize;action=9999}
            else {g.mem.set_i16(0x6E0C,0);continue}
        }
        if g.mem.i16(0x6E0A)==1 {render::back(g)}
        let mut home=false;
        if action>2000 && action<9999 {
            current=((action-2000)/100) as usize;let code=action-(current as i32*100+2000);
            if code%2!=0 && first==0 {continue}
            action=match code {
                1=>{home=true;997}, 2=>{if end==n || end-first<m.rows {continue}998},
                3=>{first=1;997},4=>{if end==n || end-first<m.rows {continue}first=(n/m.rows-1)*m.rows;end=first+m.rows;998},_=>continue,
            };
        }
        if action==9999 {action=999} else if action>=1000 {current=(action-1000) as usize;action=999}
        match action {
            0=>{render::cursor_on(g);return 0},
            997|998=>{
                if g.mem.i16(0x6E0A)!=1 {clear(g)} else {layout(g,7)}
                if action==997 {first=first.saturating_sub(m.rows);end=(first+m.rows).min(n);current=end-1;more=true;
                    while current>first && m.prices[current]==0 {current-=1} if home {current=first}
                } else {first+=m.rows;end=(end+m.rows).min(n);current=first;more=end<n;while current+1<end && m.prices[current]==0 {current+=1}}
            }
            999=>{
                if !enabled || current>=n || (m.select_only==1 && money!=0 && m.prices[current]>money) {continue}
                if !(money!=0 && m.select_only==0) && g.mem.i16(0x503E)!=1 {
                    render::front(g);
                    ui::ink(g,m.colors.and_then(|c|c.get(current-first)).copied().unwrap_or(10));
                    let y=(current-first+4) as i32*8+g.mem.i16(0x6E0E) as i32;
                    let x=g.mem.i16(0x6E08) as i32;let width=g.mem.i16(0x6E00) as i32;
                    at(g,y,x+12);puts(g,&font_bytes(format!("|{}",m.names[current])));
                    render::copy_region(g,0x503A,y-10,x,18,width,0x7158,0,0);
                    for _ in 0..5 {
                        render::fill_rect(g,y,x,8,width,9);walk::wait_ticks(g,1);
                        render::copy_region(g,0x7158,0,0,18,width,0x503A,y-10,x);walk::wait_ticks(g,1);
                    }
                }
                if g.mem.u8(0x6E2B)!=0 {g.mem.set_u8(0x6E2B,(current+1) as u8)}
                render::cursor_on(g);return current+1;
            }
            _=>{
                let i=action.unsigned_abs() as usize-1;if i>=n {continue}current=i;
                let can_add=m.select_only==0 && m.quantities[i]<m.limits[i];
                if m.select_only==0 && !can_add {
                    let message=match m.kind {98=>b"Unable to carry more.".as_slice(),97=>b"No more of those.",99=>b"This weapon is full.",_=>b""};
                    if !message.is_empty() {at(g,g.mem.i16(0x6E04) as i32*8+g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32);if m.kind==97 {ui::ink(g,6)}puts(g,message)}
                }
                let weight=m.weights.and_then(|w|w.get(i)).copied().unwrap_or(1);
                let signed_price=m.prices[i].wrapping_mul(m.sign as i32 as u32);
                if action>0 {
                    if (can_add || (m.quantities[i]==0 && m.limits[i]!=0)) && money.wrapping_sub(signed_price) as i32>=0 {
                        if (load as u16+weight as u16)<m.max_load as u16 || (m.max_load==0 && (load as u16+weight as u16)<1000) {
                            enabled=true;load=load.wrapping_add(weight);m.quantities[i]=m.quantities[i].wrapping_add(1);money=money.wrapping_sub(signed_price);
                            if g.mem.i16(0x6E0A)==1 {layout(g,7)}
                        } else {
                            let message=match m.kind {0=>"The Interloper's computer program storage is full.",1=>"The Interloper is only equipped for a single computer.",5=>"There is no space in the Interloper's cargo bay.",6=>"There is no cargo in the Interloper's cargo bay.",3=>"The Interloper's missile storage is full.",98=>"Unable to carry more.",_=>""};
                            if !message.is_empty() {at(g,g.mem.i16(0x6E04) as i32*8+g.mem.i16(0x6E0E) as i32,g.mem.i16(0x6E08) as i32);puts(g,message.as_bytes())}
                        }
                    }
                } else if m.quantities[i]!=0 {
                    load=load.wrapping_sub(weight);money=money.wrapping_add(signed_price);m.quantities[i]-=1;
                    if g.mem.i16(0x6E0A)==1 {layout(g,7)}
                }
            }
        }
    }
}
pub fn choose_payer(g: &mut Ground, title: &str, mode: u8, need: i16) -> i16 {
    super::ui::payer(g,&font_bytes(title),mode,need)
}

pub fn choices(g: &mut Ground, title: &str, names: &[String]) -> usize {
    let n=names.len(); let mut quantities=vec![0;n];
    run(g,Menu {title,names,prices:&vec![1;n],quantities:&mut quantities,limits:&vec![0;n],rows:n,
        select_only:1,money:0,max_load:0,weights:None,kind:0,sign:1,colors:None})
}
