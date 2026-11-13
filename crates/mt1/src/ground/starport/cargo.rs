use super::*;
use ui::{Row,Shop};

pub fn buy(g:&mut Ground) {
    let mut used=(0..20).filter(|&i|g.mem.party_u8(0xCA+i)!=0).count();
    let percent=(100-brokerage(g)) as u8;let percent=percent.max(90) as i32;
    if used==20 {ui::say_at(g,0x2C17,true);return}ui::clear(g);
    loop {
        let payer=ui::payer(g,&string(g,0x2C61),0,1);if payer==-1 {ui::clear(g);return}
        let mut money=cash(g,payer);let mut types=Vec::new();let mut rows=Vec::new();
        let planet=0x1BBB_u16.wrapping_add(10u16.wrapping_mul(g.mem.u16(0x6DEF)));
        for good in 0..30u16 {
            let special=g.mem.u8(planet+7)==good as u8;
            if !special && (g.mem.u8(planet+9)&(1<<(good/5))==0||g.mem.u8(planet+8)==good as u8) {continue}
            let mut price=g.mem.i16(0x1B7F+2*good) as i32*percent/100+g.mem.i8(planet+good/5) as i32*50;
            if special {price+=g.mem.i8(planet+6) as i32*50}let price=price.max(100) as u32;
            let label=format!("^{}^{}",drawing::string(&pointed(g,0x1B07+4*good)),super::money(g,price));
            types.push(good as u8);rows.push(Row::new(ui::font_bytes(label),price,255));
        }
        ui::limits(g,50,170);set_text_box(g,1);
        let result=ui::shop(g,&string(g,0x2C85),&mut rows,Shop {rows:15,select:false,money,room:(21-used) as u16,kind:5,sign:1});redraw(g);
        if result==0 {continue}
        for (r,good) in rows.iter().zip(types) {for _ in 0..r.quantity {g.mem.set_party_u8(0xCA+used as u16,good+1);used+=1;money=money.wrapping_sub(r.price)}}
        put_cash(g,payer,money);ui::clear(g);return
    }
}
pub fn sell(g:&mut Ground) {
    for slot in 0..19u16 {if g.mem.party_u8(0xCA+slot)==0 {for later in slot+1..20 {g.mem.set_party_u8(0xC9+later,g.mem.party_u8(0xCA+later))}g.mem.set_party_u8(0xDD,0)}}
    let count=(0..20).filter(|&i|g.mem.party_u8(0xCA+i)!=0).count();
    let percent=(brokerage(g)+80) as u8;let percent=percent.min(90) as i32;ui::clear(g);
    if count==0 {ui::say_at(g,0x2C96,true);return}ui::clear(g);
    loop {
        let payer=ui::payer(g,&string(g,0x2CD6),0,-1);if payer==-1 {ui::clear(g);return}
        let mut money=cash(g,payer);let mut goods=Vec::<u8>::new();let mut rows=Vec::<Row>::new();
        let planet=0x1BBB_u16.wrapping_add(10u16.wrapping_mul(g.mem.u16(0x6DEF)));let mult=g.mem.u8(0x1E73+g.mem.u8(0x6DEE) as u16) as i32;
        for cell in 0..count {let good=g.mem.party_u8(0xCA+cell as u16).wrapping_sub(1);
            if let Some(i)=goods.iter().position(|&v|v==good) {rows[i].limit=rows[i].limit.wrapping_add(1);continue}
            let t=good as u16;let mut price=g.mem.i16(0x1B7F+2*t) as i32*percent/100;let demand=g.mem.i8(planet+t/5) as i32;
            if g.mem.u8(planet+7)==good {price+=(demand*mult*50) as i16 as i32/100+g.mem.i8(planet+6) as i32*50}
            else {price=(price+demand*50)*mult/100}let price=price.max(50) as u32;
            rows.push(Row::new(ui::font_bytes(format!("^{}          ^{}",drawing::string(&pointed(g,0x1B07+4*t)),super::money(g,price))),price,1));goods.push(good);
        }
        ui::limits(g,50,170);set_text_box(g,1);let height=rows.len().min(15);
        let result=ui::shop(g,&string(g,0x2C85),&mut rows,Shop {rows:height,select:false,money,room:(count+1) as u16,kind:5,sign:-1});redraw(g);if result==0 {continue}
        for (r,good) in rows.iter().zip(goods) {let mut remaining=r.quantity;let mut slot=0;
            while remaining>0&&slot<20 {if g.mem.party_u8(0xCA+slot)==good+1 {
                for later in slot..19 {g.mem.set_party_u8(0xCA+later,g.mem.party_u8(0xCB+later))}g.mem.set_party_u8(0xDD,0);money=money.wrapping_add(r.price);remaining-=1;
            } else {slot+=1}}
        }
        put_cash(g,payer,money);ui::clear(g);return
    }
}
