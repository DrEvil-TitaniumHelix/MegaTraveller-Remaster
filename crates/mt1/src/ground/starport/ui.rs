use super::*;

pub fn font_bytes(text:impl AsRef<str>)->Vec<u8> {text.as_ref().chars().map(|c|c as u8).collect()}

pub fn at(g:&mut Ground,y:i16,x:i16) {g.mem.set_i16(0x7139,y);g.mem.set_i16(0x713B,x)}
pub fn text(g:&mut Ground,s:&[u8]) {drawing::text(g,s,g.mem.i16(0x7139) as i32,g.mem.i16(0x713B) as i32)}
pub fn ink(g:&mut Ground,c:u8) {drawing::ink(g,c)}
pub use super::shop_menu::{limits, layout, clear, say, drawn_words};
pub fn fill(g:&mut Ground,y:i16,x:i16,h:i16,w:i16,c:u8) {let mut p=Paint::new(g,Raster::Wrap);p.fill(y as i32,x as i32,h as i32,w as i32,c);p.finish()}
pub fn say_at(g:&mut Ground,off:u16,cont:bool) {say(g,&string(g,off),cont,6)}

#[derive(Clone)]
pub struct Row {pub name:Vec<u8>,pub price:u32,pub limit:u8,pub quantity:u8,pub weight:u8,pub color:Option<u8>}
impl Row {
    pub fn new(name:Vec<u8>,price:u32,limit:u8)->Self {Self {name,price,limit,quantity:0,weight:1,color:None}}
}
pub struct Shop {pub rows:usize,pub select:bool,pub money:u32,pub room:u16,pub kind:u8,pub sign:i8}
impl Shop {pub fn select(rows:usize)->Self {Self {rows,select:true,money:0,room:0,kind:0,sign:1}}}

pub fn shop(g:&mut Ground,title:&[u8],rows:&mut [Row],opt:Shop)->usize {
    let mode=u8::from(opt.select);shop_mode(g,title,rows,opt,mode)
}
pub fn shop_mode(g:&mut Ground,title:&[u8],rows:&mut [Row],opt:Shop,mode:u8)->usize {
    let names:Vec<_>=rows.iter().map(|r|drawing::string(&r.name)).collect();
    let prices:Vec<_>=rows.iter().map(|r|r.price).collect();
    let limits:Vec<_>=rows.iter().map(|r|r.limit).collect();
    let weights:Vec<_>=rows.iter().map(|r|r.weight).collect();
    let colors:Option<Vec<_>>=rows.iter().any(|r|r.color.is_some()).then(||rows.iter().map(|r|r.color.unwrap_or(6)).collect());
    let mut quantities=vec![0;rows.len()];
    let result=super::shop_menu::run(g,super::shop_menu::Menu {
        title:&drawing::string(title),names:&names,prices:&prices,quantities:&mut quantities,
        limits:&limits,rows:opt.rows,select_only:mode,money:opt.money,max_load:opt.room as u8,
        weights:Some(&weights),kind:opt.kind,sign:opt.sign,colors:colors.as_deref(),
    });
    for (row,quantity) in rows.iter_mut().zip(quantities) {row.quantity=quantity}
    result
}

pub fn payer(g:&mut Ground,title:&[u8],mode:u8,need:i16)->i16 {
    let minimum=if need==-1 {0} else {need as i32 as u32};let mut mapping=Vec::new();let mut rows=Vec::new();
    limits(g,g.mem.i16(0x6E08)+18,g.mem.i16(0x6E08)+98);
    for who in 0..5 {let money=cash(g,who);
        if g.mem.party_u8(0x127+who as u16*256)==0||!(money>minimum||need==-1||(mode>0&&mode<255)) {continue}
        let name=drawing::bytes(g,party(g,0x102+who as i32*256));let label=format!("{}^{money}$",drawing::string(&name[..name.len().min(13)]));
        let mut r=Row::new(font_bytes(label),1,0);r.color=Some(who as u8+1);rows.push(r);mapping.push(who);
    }
    let ship=cash(g,-2);
    if (ship>=minimum&&mode==0)||mode==1 {let mut r=Row::new(ui::font_bytes(format!("{:>13}^{ship}$",drawing::string(&string(g,0x242A)))),1,0);r.color=Some(6);rows.push(r)}
    crate::play_lists::payer(g,title,&rows);
    let n=rows.len();let choice=if n==0 {0} else {shop(g,title,&mut rows,Shop::select(n))};if n!=0&&choice==0 {return -1}
    let result=if mode==0&&choice==n&&ship>0 {-2} else if choice>0 {mapping.get(choice-1).copied().unwrap_or(-1)} else {-1};
    if result!=-1 {ink(g,if result==-2 {6} else {result as u8+1})}result
}
