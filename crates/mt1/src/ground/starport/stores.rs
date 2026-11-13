use super::{objects::{self,Allocation},shop_menu as shop};
use crate::ground::{commands_items as items,input::{self,ui},render,state::Ground,walk};

fn item_name(g: &Ground, id: i16) -> String {g.mem.cstr(items::definition(g,id))}
fn item_price(g: &Ground, id: i16, percent: i16) -> u32 {shop::price(g.mem.read(items::definition(g,id).add(0x21),4),percent)}
fn labels(g: &Ground, table: u16, count: u16) -> Vec<String> {(0..count).map(|i|g.mem.cstr(g.mem.pointer(table+4*i))).collect()}
fn quantity_menu(g: &mut Ground, title: &str, names: &[String], prices: &[u32], quantity: &mut [u8], limits: &[u8], money: u32, room: u8, kind: u8, sign: i8) -> usize {
    shop::run(g,shop::Menu {title,names,prices,quantities:quantity,limits,rows:names.len().min(10),select_only:0,money,max_load:room,weights:None,kind,sign,colors:None})
}
pub fn general_store(g: &mut Ground) {
    g.mem.set_u16(0x7012,0);shop::set_box(g,2);shop::clear(g);
    let names=labels(g,0x2096,2);
    loop {match shop::choices(g,"Welcome to the general store. What is your business?",&names) {1=>store_buy(g),2=>store_sell(g),_=>break}}
    shop::set_box(g,0);g.mem.set_u16(0x7012,1);
}
pub fn store_buy(g: &mut Ground) {
    shop::set_box(g,2);shop::clear(g);let percent=100i16.wrapping_sub(shop::skill_bonus(g));let mut payer=-1;
    loop {
        if payer==-1 {payer=shop::choose_payer(g,"Who will buy these supplies?",0,0);if payer==-1 {shop::clear(g);return}}
        let who=shop::choose_payer(g,"Who will recieve the supplies?",255,-1);if who==-1 {payer=-1;continue}
        let prices:Vec<_>=(45..55).map(|id|item_price(g,id,percent)).collect();
        let names:Vec<_>=(45..55).enumerate().map(|(i,id)|format!("{}^^{}$",item_name(g,id),prices[i] as i32)).collect();
        let mut money=shop::cash(g,payer);let x=g.mem.i16(0x6E08);shop::limits(g,x+10,x+125);
        let room=(0..19).filter(|&slot|g.mem.party_u16(items::inv(who,slot))==0).count()
            .min((1..=256).filter(|&obj|g.mem.read(items::object(g,obj),1)==255).count());
        if room==0 {shop::clear(g);shop::say(g,format!("{} has no room to carry any more.",shop::name(g,who)).as_bytes(),false,who as u8+1);input::menu_getkey(g);continue}
        let mut qty=[0;10];
        if quantity_menu(g,"Here are supplies you can buy:",&names,&prices,&mut qty,&[98;10],money,(room+1) as u8,98,1)==0 {continue}
        for (i,&amount) in qty.iter().enumerate() {for _ in 0..amount {
            if objects::give(g,who,i as i16+45,Allocation::Store)!=-1 {money=money.wrapping_sub(prices[i])}
        }}
        shop::clear(g);shop::set_cash(g,payer,money);return;
    }
}
pub fn store_sell(g: &mut Ground) {
    shop::set_box(g,2);shop::clear(g);let percent=shop::skill_bonus(g).wrapping_add(40).min(90);let mut payee=-1;
    loop {
        if payee==-1 {payee=shop::choose_payer(g,"Who will recieve the money?",0,-1);if payee==-1 {shop::clear(g);return}}
        let who=shop::choose_payer(g,"Who will sell some items?",255,-1);if who==-1 {payee=-1;continue}
        let mut ids=Vec::new();let mut counts=Vec::<u8>::new();let mut prices=Vec::new();let mut names=Vec::new();
        for slot in 0..19 {
            let obj=g.mem.party_u16(items::inv(who,slot)) as i16;if obj<=0 {continue}
            let id=g.mem.read(items::object(g,obj),1) as i16;
            if let Some(i)=ids.iter().position(|&n|n==id) {counts[i]=counts[i].wrapping_add(1);continue}
            let price=(item_price(g,id,percent) as i32).max(2) as u32;
            ids.push(id);counts.push(1);prices.push(price);names.push(format!("{}^^{}$",item_name(g,id),price as i32));
        }
        let mut money=shop::cash(g,payee);let x=g.mem.i16(0x6E08);shop::limits(g,x+10,x+125);
        if ids.is_empty() {shop::clear(g);shop::say(g,format!("{} has nothing to sell.",shop::name(g,who)).as_bytes(),false,who as u8+1);input::menu_getkey(g);continue}
        let mut qty=vec![0;ids.len()];let title=format!("{} has the following items to sell: ",shop::name(g,who));
        if quantity_menu(g,&title,&names,&prices,&mut qty,&counts,money,0,97,-1)==0 {continue}
        for (i,&id) in ids.iter().enumerate() {
            if qty[i]==0 {continue}
            for slot in 0..19 {
                let obj=g.mem.party_u16(items::inv(who,slot)) as i16;
                if g.mem.read(items::object(g,obj),1)==id as u32 {
                    items::remove(g,who,slot);qty[i]-=1;money=money.wrapping_add(prices[i]);if qty[i]==0 {break}
                }
            }
        }
        shop::clear(g);shop::set_cash(g,payee,money);return;
    }
}
pub fn armor_shop(g: &mut Ground, level: u8) {
    g.mem.set_u16(0x7012,0);let pct=100i16.wrapping_sub(shop::skill_bonus(g));
    let mut ids=Vec::new();let mut names=Vec::new();let mut prices=Vec::new();
    for id in 0..41 {
        let d=items::definition(g,id);if g.mem.read(d,1)==0 {break}
        if g.mem.read(d.add(0x2C),1)<=level as u32 && g.mem.read(d.add(0x1D),2)==3 {
            let p=item_price(g,id,pct);ids.push(id);prices.push(p);names.push(format!("{}^^{}$",item_name(g,id),p as i32));
        }
    }
    shop::set_box(g,2);shop::clear(g);ui::ink(g,10);let mut who=0;let mut title="This is the armor I have.";
    loop {
        shop::clear(g);
        if who==0 {who=shop::choose_payer(g,"Who is interested in getting armor?",5,0);if who==-1 {break}}
        let payer=shop::choose_payer(g,"Who will buy the armor?",0,0);if payer==-1 {who=0;continue}
        let mut money=shop::cash(g,payer);g.mem.set_u8(0x6E2B,1);
        loop {
            ui::ink(g,if payer==-2 {6} else {payer as u8+1});let x=g.mem.i16(0x6E08);shop::limits(g,x+100,x+140);
            let mut quantities=vec![1;ids.len()];
            let choice=shop::run(g,shop::Menu {title,names:&names,prices:&prices,quantities:&mut quantities,limits:&vec![0;ids.len()],rows:ids.len().min(10),
                select_only:1,money,max_load:0,weights:None,kind:0,sign:1,colors:None});render::front(g);
            if choice==0 {g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,b"Stop back!",false,10);title="Good to see you again.";walk::wait_ticks(g,36);g.mem.set_u8(0x6E2B,0);who=0;break}
            let i=choice-1;
            if prices[i] as i32>money as i32 {g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,b"Hey you can't afford that!",false,payer.wrapping_add(1) as u8);walk::wait_ticks(g,18);ui::ink(g,10)}
            else if objects::give_gun_with_ammo(g,who,ids[i])==-1 {g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,b"There isn't enough room.",false,who as u8+1);walk::wait_ticks(g,36)}
            else {money=shop::cash(g,payer).wrapping_sub(prices[i]);shop::set_cash(g,payer,money);title="Done. Anything else?"}
        }
    }
    ui::ink(g,10);shop::set_box(g,0);g.mem.set_u16(0x7012,1);
}

pub fn buy_consumables(g: &mut Ground, payer: i16, who: i16, mut money: u32) {
    let pct=100i16.wrapping_sub(shop::skill_bonus(g));let mut names=Vec::new();let mut prices=Vec::new();let mut limits=Vec::new();
    for i in 0..3u16 {
        limits.push(9u8.wrapping_sub(g.mem.party_u8(who as u16*256+0x1CB+i)));
        let price=g.mem.u32(0x2004+4*i).wrapping_mul(pct as i32 as u32)/100;
        prices.push(price);names.push(format!("{}^^{}$",g.mem.cstr(g.mem.pointer(0x1FF8+4*i)),price as i32));
    }
    let x=g.mem.i16(0x6E08);shop::limits(g,x+70,x+90);let mut qty=[0;3];
    if quantity_menu(g,"I have have these:",&names,&prices,&mut qty,&limits,money,0,98,1)==0 {return}
    for (i,&amount) in qty.iter().enumerate() {money=money.wrapping_sub(prices[i].wrapping_mul(amount as u32));let off=who as u16*256+0x1CB+i as u16;g.mem.set_party_u8(off,g.mem.party_u8(off).wrapping_add(amount))}
    shop::set_cash(g,payer,money);
}
pub fn buy_ammo(g: &mut Ground, payer: i16, who: i16, mut money: u32, law: u8) -> i16 {
    let pct=100i16.wrapping_sub(shop::skill_bonus(g));let mut objs=Vec::new();let mut names=Vec::new();let mut prices=Vec::new();let mut limits=Vec::new();let mut any=false;
    for slot in 0..23 {
        let obj=g.mem.party_u16(items::inv(who,slot)) as i16;if obj<=0 {continue}
        let id=g.mem.read(items::object(g,obj),1) as i16;let d=items::definition(g,id);
        if id>=254 || g.mem.read(items::definition(g,slot).add(0x2B),1)<=law as u32
            || g.mem.read(d.add(0x1D),2)!=2 || g.mem.read(d.add(0x27),1)&32==0 {continue}
        let price=shop::price(g.mem.u16(0x2010+id as u16*2) as u32,pct);
        if price<money {objs.push(obj);prices.push(price);let name=item_name(g,id).chars().take(4).collect::<String>();names.push(format!("{name:>4} ammo^^{}$",price as i32));limits.push(99u8.wrapping_sub(g.mem.read(items::object(g,obj).add(1),1) as u8))}
        any=true;
    }
    if objs.is_empty() {return -i16::from(any)}
    ui::ink(g,who as u8+1);let x=g.mem.i16(0x6E08);shop::limits(g,x+70,x+90);let mut qty=vec![0;objs.len()];
    if quantity_menu(g,"What ammo would you like?",&names,&prices,&mut qty,&limits,money,0,99,1)!=0 {
        for (i,&obj) in objs.iter().enumerate() {money=money.wrapping_sub(prices[i].wrapping_mul(qty[i] as u32));let p=items::object(g,obj).add(1);g.mem.write(p,(g.mem.read(p,1) as u8).wrapping_add(qty[i]) as u32,1)}
        shop::set_cash(g,payer,money);
    }
    1
}
pub fn sell_weapons(g: &mut Ground, tech: u8, who: i16, payer: i16, mut money: u32, law: u8) -> bool {
    let base=shop::skill_bonus(g).wrapping_add(70);let x=g.mem.i16(0x6E08);shop::limits(g,x+100,x+140);let mut sold=false;let mut title="Which weapons are you selling";
    loop {
        let mut slots=Vec::new();let mut names=Vec::new();let mut prices=Vec::new();
        for slot in 0..23 {
            let Some((obj,d))=items::item(g,who,slot) else {continue};if g.mem.read(d.add(0x1D),2)!=2 {continue}
            let mut pct=base.wrapping_add((g.mem.read(obj.add(1),1)/2) as i16);
            let item_tech=g.mem.read(d.add(0x2C),1) as u8;let item_law=g.mem.read(d.add(0x2B),1) as u8;
            if item_tech>tech {pct=pct.wrapping_add((item_tech-tech) as i16*10)}
            if item_law<law {pct=pct.wrapping_add((law-item_law) as i16*10)}
            let price=shop::price(g.mem.read(d.add(0x21),4),pct);slots.push(slot);prices.push(price);names.push(format!("{}^^{}$",g.mem.cstr(d),price as i32));
        }
        if slots.is_empty() {shop::set_cash(g,payer,money);return sold}
        ui::ink(g,who as u8+1);let mut quantities=vec![255;slots.len()];
        let choice=shop::run(g,shop::Menu {title,names:&names,prices:&prices,quantities:&mut quantities,limits:&vec![0;slots.len()],rows:slots.len().min(10),select_only:2,money,max_load:0,weights:None,kind:0,sign:1,colors:None});
        if choice==0 {shop::set_cash(g,payer,money);return true}
        sold=true;objects::discard_slot_item(g,who,slots[choice-1]);money=money.wrapping_add(prices[choice-1]);title="Deal. Anything else?";
    }
}
pub fn weapons_store(g: &mut Ground, tech: u8, law: u8) {
    g.mem.set_u16(0x7012,0);let pct=100i16.wrapping_sub(shop::skill_bonus(g));let mut ids=Vec::new();let mut names=Vec::new();let mut prices=Vec::new();
    for id in 0..30 {
        let d=items::definition(g,id);if g.mem.read(d,1)==0 {break}
        if g.mem.read(d.add(0x1D),2)==2 && g.mem.read(d.add(0x2C),1)<=tech as u32 && g.mem.read(d.add(0x2B),1)>law as u32 {
            let price=item_price(g,id,pct);ids.push(id);prices.push(price);names.push(format!("{}^^{}$",item_name(g,id),price as i32));
        }
    }
    let mut title="What are you looking for?";let mut offer="What can I interest you in?";shop::set_box(g,2);
    loop {
        shop::clear(g);ui::ink(g,10);
        let choices=labels(g,0x2042,2);
        let choice=shop::choices(g,"Welcome to the weapons store. How may I help you?",&choices);
        if choice==0 {g.mem.set_u16(0x7012,1);return}
        if choice==2 {
            loop {
                let who=shop::choose_payer(g,"Who will sell the items?",5,0);if who==-1 {break}
                let payer=shop::choose_payer(g,"Who will collect the money?",0,-1);if payer==-1 {continue}
                if !sell_weapons(g,tech,who,payer,shop::cash(g,payer),law) {
                    render::front(g);g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,format!("{} has nothing I want.",shop::name(g,who)).as_bytes(),false,who as u8+1);walk::wait_ticks(g,36);
                }
                break;
            }
            continue;
        }
        loop {
            let choices=labels(g,0x204A,3);let mode=shop::choices(g,title,&choices);
            if mode==0 {title="What are you looking for?";break}
            shop::clear(g);
            loop {
                let who=shop::choose_payer(g,"Who will recieve the items?",5,-1);if who==-1 {break}
                let payer=shop::choose_payer(g,"Who will buy the items?",0,0);if payer==-1 {continue}
                let mut money=shop::cash(g,payer);
                if mode==2 {
                    let result=buy_ammo(g,payer,who,money,law);render::front(g);g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);
                    if result<1 {
                        let (msg,color)=if result==0 {(format!("{:>12} has nothing that needs ammo.",shop::name(g,who).chars().take(12).collect::<String>()),who)}
                            else {(format!("{} cannot afford to pay for ammo.",if payer==-2 {"Party account".to_string()} else {shop::name(g,payer)}),payer)};
                        shop::say(g,msg.as_bytes(),false,color.wrapping_add(1) as u8);walk::wait_ticks(g,36);
                    }
                    title="Anything else?";break;
                }
                if mode==3 {buy_consumables(g,payer,who,money);break}
                g.mem.set_u8(0x6E2B,1);
                loop {
                    ui::ink(g,if payer==-2 {6} else {payer as u8+1});let x=g.mem.i16(0x6E08);shop::limits(g,x+100,x+140);let mut quantities=vec![1;ids.len()];
                    let pick=shop::run(g,shop::Menu {title:offer,names:&names,prices:&prices,quantities:&mut quantities,limits:&vec![0;ids.len()],rows:ids.len().min(10),select_only:1,money,max_load:0,weights:None,kind:0,sign:1,colors:None});
                    render::front(g);
                    if pick==0 {g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);title="Pleasure doing business. Anything else?";offer="Good to see you again.";g.mem.set_u8(0x6E2B,0);break}
                    let i=pick-1;
                    if prices[i] as i32>money as i32 {
                        g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,b"Hey you can't afford that!",false,payer.wrapping_add(1) as u8);walk::wait_ticks(g,18);ui::ink(g,10);
                        #[allow(unused_assignments)] {title="Anything else?";}
                    } else if objects::give(g,who,ids[i],Allocation::Empty)==-1 {
                        g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);shop::say(g,b"There isn't enough room.",false,who as u8+1);walk::wait_ticks(g,36);
                        #[allow(unused_assignments)] {title="Anything else?";}
                    } else {money=shop::cash(g,payer).wrapping_sub(prices[i]);shop::set_cash(g,payer,money);g.mem.set_u8(0x6E2A,g.mem.u16(0x6E04) as u8);offer="Deal. What else would you like?"}
                }
                break;
            }
        }
    }
}
