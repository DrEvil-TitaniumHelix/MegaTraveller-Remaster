use super::*;
use ui::{Row,Shop};
fn owner(g:&Ground,who:i16)->String {drawing::string(&if who==-2 {string(g,0x242A)} else {drawing::bytes(g,party(g,0x102+who as i32*256))})}
fn pause_pay(g:&mut Ground,live:&[u8])->u16 {let line=string(g,0x3079);let words=ui::drawn_words(g,&line,&[b"Pay",b"Esc"]);crate::play_prompts::named_state(g,"shipyard.pay",&[("pay","Pay",80),("back","Cancel",27)],live,&words);loop {let k=getkey(g);if matches!(k,27|80|112) {return k}}}
fn bill(g:&mut Ground,payer:i16,money:u32,original:u32) {
    let color=if payer==-2 {6} else {payer as u8+1};let h=g.mem.i16(0x6E08);ui::limits(g,h+80,h+80);
    ui::say(g,&ui::font_bytes(format!("{}:^{money}$",owner(g,payer))),false,color);
    ui::say(g,&ui::font_bytes(format!("{}:^{}$",drawing::string(&string(g,0x2FED)),original.wrapping_sub(money))),false,color);
}
pub fn refuel(g:&mut Ground) {
    let purifier=g.mem.party_u8(0xC1)&16!=0;let mut unrefined=0u8;let mut payer=-1i16;
    ui::clear(g);if g.mem.party_u8(0xFC)==124 {ui::say_at(g,0x2F5C,true);return}ui::limits(g,242,242);
    while payer==-1 {
        let mut rows:Vec<_>=(usize::from(purifier)..2).map(|i|Row::new(pointed(g,0x1FD7+i as u16*4),1,0)).collect();let n=rows.len();
        let c=ui::shop(g,&string(g,0x2F7C),&mut rows,Shop::select(n));if c==0 {ui::clear(g);return}
        let typ=c+usize::from(purifier);let price=if typ==2 {48u32} else {80};payer=-4;
        while payer==-4 {
            let mut fuel=g.mem.party_u8(0xFC) as u16;let (mut quit,mut finished)=(false,false);ui::clear(g);
            render::copy_front_to_back(g,0,0,200,40);
            while !finished {
                payer=ui::payer(g,&string(g,0x2F90),0,price as i16);let mut money=0;
                if payer!=-1 {
                    money=cash(g,payer);let original=money;
                    while price<money&&fuel<124&&!quit {
                        render::back(g);ui::clear(g);let h=g.mem.i16(0x6E08);let product=g.scene.is_some();
                        if product {ui::limits(g,h+86,h+118)} else {ui::limits(g,h+50,h+90)}
                        ui::say_at(g,if typ==2 {0x2FAC} else {0x2FBE},true);
                        if g.scene.is_some() {ui::say(g,&ui::font_bytes(format!("  Fuel: {fuel} of 124")),false,6)} else {ui::say_at(g,0x2FCE,false)}
                        if fuel!=0 {let y=g.mem.i16(0x7139);let width=(fuel*150/124) as i16;ui::fill(g,y+15,h+1,10,width+1,6);ui::fill(g,y+26,h+1,1,width+1,0);render::draw_line(g,(y+15) as i32,(width+h+1) as i32,(y+26) as i32,(width+h+1) as i32,0)}
                        g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(4));
                        let words=string(g,0x2FD7);
                        let words=if product {ui::font_bytes(drawing::string(&words).replacen("Fill","Buy one unit",1))} else {words};
                        ui::say(g,&words,false,10);let hot=ui::drawn_words(g,&words,&[if product {&b"Buy one unit"[..]} else {b"Fill"},b"All",b"Pay"]);bill(g,payer,money,original);render::flip(g);
                        crate::play_prompts::named_state(g,"16374",&[("fuel","Buy one unit",if product {66} else {70}),("all","Fill tank",65),("pay","Pay",80),("back","Cancel",27)],&[fuel as u64,money as u64,original as u64,payer as u64,unrefined as u64,typ as u64].map(u64::to_le_bytes).concat(),&hot);
                        loop {match getkey(g) {
                            70|102=>{fuel+=1;money=money.wrapping_sub(price);if typ==2 {unrefined=unrefined.wrapping_add(1)}break},
                            66|98 if product=>{fuel+=1;money=money.wrapping_sub(price);if typ==2 {unrefined=unrefined.wrapping_add(1)}break},
                            65|97=>{let needed=(124-fuel) as u32*price;if needed>money {fuel+=(money/price) as u16;money%=price}else{money-=needed;fuel=124}break},
                            80|112=>{quit=true;finished=true;break},
                            27=>{quit=true;finished=true;fuel=g.mem.party_u8(0xFC) as u16;payer=-4;break},_=>{}}
                        }
                    }
                    render::front(g);
                    if !quit {
                        ui::clear(g);
                        if price>money {if payer==-2 {ui::say_at(g,0x2FF3,true)}else {ui::say(g,&ui::font_bytes(format!("There is no more money in {}'s account.",owner(g,payer))),true,payer as u8+1)}}
                        else {ui::say(g,&ui::font_bytes(format!("The Interloper's tank is full at a cost of {}$.",cash(g,payer).wrapping_sub(money))),true,6);finished=true}
                        ui::say(g,&string(g,0x3079),false,10);
                        if pause_pay(g,&[fuel as u64,money as u64,payer as u64,unrefined as u64,typ as u64].map(u64::to_le_bytes).concat())==27 {payer=-1;fuel=g.mem.party_u8(0xFC) as u16;finished=false}
                    }
                } else {finished=true}
                if payer!=-1 {put_cash(g,payer,money);g.mem.set_party_u8(0xFD,unrefined);g.mem.set_party_u8(0xFC,fuel as u8)}
            }
        }
    }
    ui::clear(g);
}
fn ship_stat(g:&mut Ground,index:u8) {
    let kind=if index>1 {index-1} else {index};let mut skills=[0i16;7];
    if g.mem.party_u8(0xA4)<5 {for (s,id) in skills.iter_mut().zip([13,17,19,52,42,68,27]) {*s=use_skill(g,id)}}
    let weights=match kind {0=>[3,3,2,1,0,0,0],1=>[0,2,3,2,3,1,0],2=>[0,0,3,0,2,0,1],_=>[1;7]};
    let total=(g.mem.u8(0x1C59+kind as u16*2) as i16+skills.iter().zip(weights).map(|(&s,w)|s*w).sum::<i16>()).min(g.mem.u8(0x1C5A+kind as u16*2) as i16);
    g.mem.set_party_u8(0xFE+index as u16,(100-g.mem.party_u8(0xC2+index as u16) as i16).wrapping_mul(total) as u8);
}
fn repair_all(cost:u32,damage:&mut i16,money:&mut u32,all_affordable:&mut bool) {
    let needed=(cost as i16).wrapping_mul(*damage) as i32 as u32;
    if needed>*money&&cost!=0 {
        *damage=damage.wrapping_sub((*money/cost) as i16);
        *money%=cost;*all_affordable=true;
    } else {*money=money.wrapping_sub(needed);*damage=0}
}
pub fn repair(g:&mut Ground) {
    let sections:Vec<u16>=(0..5).filter(|&i|g.mem.party_u8(0xC2+i)!=0).collect();ui::clear(g);
    if sections.is_empty() {ui::clear(g);ui::say_at(g,0x30A8,true);return}
    render::copy_front_to_back(g,0,0,200,40);
    let mut rows:Vec<_>=sections.iter().map(|&i|Row::new(pointed(g,0x1FDF+i*4),1,0)).collect();
    let (mut payer,mut all_affordable)=(-1,false);let mut money=0u32;let mut damage;
    while payer==-1&&!all_affordable {
        ui::ink(g,10);let n=rows.len();let choice=ui::shop(g,&string(g,0x30DC),&mut rows,Shop::select(n));if choice==0 {ui::clear(g);return}
        let section=sections[choice-1];let title=pointed(g,0x1FDF+section*4);let base=g.mem.u8(0x1FF3+section) as u32;let cost=base/5*g.mem.u8(0x6DEE) as u32+base;
        let h=g.mem.i16(0x6E08);ui::limits(g,h+50,h+90);
        loop {
            let mut quit=false;damage=g.mem.party_u8(0xC2+section) as i16;payer=ui::payer(g,&string(g,0x30F2),0,cost as i16);
            if payer!=-1 {
                money=cash(g,payer);let original=money;let color=if payer==-2 {6} else {payer as u8+1};
                while cost<money&&damage>0&&!quit {
                    render::back(g);ui::clear(g);ui::say(g,&title,true,6);
                    ui::say(g,&ui::font_bytes(format!("Repairs:  {cost} cpd")),false,6);ui::say(g,&ui::font_bytes(format!("{}: ",drawing::string(&title))),false,6);
                    if damage<100 {let width=(100-damage) as i16*3/2;let y=g.mem.i16(0x7139);ui::fill(g,y+15,h+1,5,width,6);ui::fill(g,y+20,h+1,1,width,0);render::draw_line(g,(y+15) as i32,(width+h) as i32,(y+20) as i32,(width+h) as i32,0)}
                    g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(4));ui::limits(g,h+50,h+90);let words=string(g,0x3122);ui::say(g,&words,false,10);let hot=ui::drawn_words(g,&words,&[b"Repair",b"All",b"Pay"]);
                    ui::say(g,&ui::font_bytes(format!("{}:^^{money}$",if payer==-2 {"Party Account".into()}else{owner(g,payer)})),false,color);ui::say(g,&ui::font_bytes(format!("Total:^^{}$",original.wrapping_sub(money))),false,color);render::flip(g);
                    crate::play_prompts::named_state(g,"16A2F",&[("repair","Repair one point",82),("all","Repair all",65),("pay","Pay",80),("back","Cancel",27)],&[damage as u64,money as u64,original as u64,payer as u64,section as u64,all_affordable as u64].map(u64::to_le_bytes).concat(),&hot);
                    loop {match getkey(g) {
                        82|114=>{damage-=1;money=money.wrapping_sub(cost);break},
                        65|97=>{repair_all(cost,&mut damage,&mut money,&mut all_affordable);break},
                        80|112=>{quit=true;break},27=>{quit=true;payer=-4;break},_=>{}}
                    }render::front(g);
                }
                if !quit {
                    ui::clear(g);
                    if all_affordable {if payer==-2 {ui::say_at(g,0x2FF3,true)}else{ui::say(g,&ui::font_bytes(format!("There is no more money in {}'s account.",owner(g,payer))),true,payer as u8+1)}}
                    else {ui::say(g,&ui::font_bytes(format!("The Interloper's {} is completely fixed at a cost of {}$.",drawing::string(&title),cash(g,payer).wrapping_sub(money))),true,6)}
                    ui::say(g,&string(g,0x3079),false,10);
                    if pause_pay(g,&[damage as u64,money as u64,payer as u64,section as u64,all_affordable as u64].map(u64::to_le_bytes).concat())!=27 {put_cash(g,payer,money);g.mem.set_party_u8(0xC2+section,damage as u8);if section<4 {ship_stat(g,section as u8)}}payer=-4;
                    if !all_affordable {payer=-1;all_affordable=true}else{all_affordable=false}
                }
            }
            if payer!=-4 {break}
        }
        if payer==-2||(payer>0&&payer<5) {put_cash(g,payer,money);g.mem.set_party_u8(0xC2+section,damage as u8);if section<4 {ship_stat(g,section as u8)}}
    }
    ui::clear(g);
}
fn broker_once(g:&mut Ground)->i16 {use_skill(g,0x43)+use_skill(g,0x3D)/3+use_skill(g,9)/3+use_skill(g,0x28)/3}
fn weapon_name(i:usize,sell:bool)->String {
    if i==8 {return if sell {"Missile Launcher"}else{"Missile Launcher^"}.into()}
    let tl=if i/2%2!=0 {13}else{8};let kind=if i<4 {"Beam"}else{"Pulse"};let power=if i%2!=0 {50}else{25};
    if sell {format!("TL{tl:<3}{:<12}(pwr:{power}) ",format!("{kind} Laser"))}else{format!("TL{tl} ^{kind} Laser (pwr:{power})")}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repair_all_signed_overflow_matches_trade_py() {
        let Some(output)=trade_edge_oracle("repair") else {return};
        let mut count=0;let mut negative=false;
        for line in output.lines() {
            let values:Vec<i64>=line.split_whitespace().map(|s|s.parse().unwrap()).collect();
            assert_eq!(values.len(),6);
            let (mut damage,mut money,mut all)=(values[1] as i16,values[2] as u32,false);
            repair_all(values[0] as u32,&mut damage,&mut money,&mut all);
            assert_eq!((damage as i64,damage as u8 as i64,money as i64),(values[3],values[4],values[5]),"{line}");
            negative|=damage<0;count+=1;
        }
        assert_eq!(count,9);assert!(negative,"must cover wrapped negative damage before its byte store");
    }

    #[test]
    fn sell_weapon_row_bytes_match_15088() {
        let expected=[
            "TL8  Beam Laser  (pwr:25) ","TL8  Beam Laser  (pwr:50) ",
            "TL13 Beam Laser  (pwr:25) ","TL13 Beam Laser  (pwr:50) ",
            "TL8  Pulse Laser (pwr:25) ","TL8  Pulse Laser (pwr:50) ",
            "TL13 Pulse Laser (pwr:25) ","TL13 Pulse Laser (pwr:50) ","Missile Launcher",
        ];
        for (i,name) in expected.into_iter().enumerate() {assert_eq!(weapon_name(i,true).as_bytes(),name.as_bytes())}
    }

    #[test]
    fn cancellation_account_is_party_relative_and_repair_does_not_write_it() {
        let Some(output)=trade_edge_oracle("cancel") else {return};
        assert!(output.contains("launcher account(-4): 9000:FDFE, not DS:6B63"));
        struct Idle;
        impl mt_engine::io::Input for Idle {fn next_key(&mut self)->u16 {27}}
        let mut g=Ground::from_ds(vec![],vec![0;Mem::PARTY_SIZE],Box::new(Idle),
            Box::new(mt_engine::sim::tick::Counter::default()),Box::new(mt_engine::io::Dir(std::path::PathBuf::new())));
        assert_eq!(account(&g,-4),Far::new(0xFDFE,Mem::PARTY_SEG));
        g.mem.set_u32(0x6B63,0x12345678);let before=g.mem.party().to_vec();put_cash(&mut g,-4,99999);
        assert_eq!(g.mem.u32(0x6B63),0x12345678);assert_eq!(g.mem.party(),before);
        g.mem.alloc(Mem::PARTY_SEG,65536);put_cash(&mut g,-4,0x12345678);
        assert_eq!(g.mem.bytes(account(&g,-4),4),[0x78,0x56,0x34,0x12]);
    }
}
pub fn sell_weapon(g:&mut Ground) {
    if g.mem.party_u8(0xC8)==0&&g.mem.party_u8(0xC9)==0 {ui::say(g,b"The Interloper's gun bays are empty",true,6);return}
    ui::clear(g);let pct=(broker_once(g)+100).min(120) as i32;
    let mut prices:[u32;9]=std::array::from_fn(|i|((g.mem.u32(0x1E82+i as u16*4) as i32).wrapping_mul(pct)/100) as u32);
    loop {
        let payer=ui::payer(g,b"Who will get the money?",0,-1);if payer==-1 {ui::clear(g);return}
        let pct=(broker_once(g)+80).min(95) as i32-10;for p in &mut prices {*p=(*p as i32).wrapping_mul(pct).wrapping_div(100) as u32}
        let mut bays=Vec::new();let mut rows=Vec::new();let mut offers=Vec::new();
        for bay in 0..2u16 {let gun=g.mem.party_u8(0xC8+bay);if gun==0 {continue}let condition=100-g.mem.party_u8(0xC4+bay) as i32;let factor=if condition<21 {80}else{condition-20};
            let base=prices.get(gun as usize-1).copied().unwrap_or(0);let offer=(base as i32).wrapping_mul(factor).wrapping_div(100) as u32;
            rows.push(Row::new(ui::font_bytes(format!("Gun {} :^{}^{}",bay+1,weapon_name(gun as usize-1,true),super::money(g,offer))),1,255));bays.push(bay);offers.push(offer);
        }
        set_text_box(g,1);ui::limits(g,70,170);let n=rows.len();let c=ui::shop(g,b"Select A Gun to sell?",&mut rows,Shop::select(n));redraw(g);if c==0 {continue}
        g.mem.set_party_u8(0xC8+bays[c-1],0);put_cash(g,payer,cash(g,payer).wrapping_add(offers[c-1]));ui::clear(g);return
    }
}
pub fn buy_weapon(g:&mut Ground) {
    if g.mem.party_u8(0xC8)!=0&&g.mem.party_u8(0xC9)!=0 {ui::say(g,b"The Interloper's gun bays are full.",true,6);return}
    ui::clear(g);let pct=(100-broker_once(g)).max(80) as i32;
    let prices:[u32;9]=std::array::from_fn(|i|(g.mem.u32(0x1EA6+i as u16*4) as i32).wrapping_mul(pct).wrapping_div(100) as u32);
    loop {
        let payer=ui::payer(g,b"Who will pay for purchases?",0,1);if payer==-1 {ui::clear(g);return}let money=cash(g,payer);let bay=u16::from(g.mem.party_u8(0xC8)!=0);
        let mut rows:Vec<_>=prices.iter().enumerate().map(|(i,&p)|Row::new(ui::font_bytes(format!("{}^{}",weapon_name(i,false),super::money(g,p))),p,255)).collect();
        set_text_box(g,1);ui::limits(g,50,170);let c=ui::shop(g,b"Weapon System                 Cost",&mut rows,Shop{rows:9,select:true,money,room:2,kind:7,sign:1});redraw(g);if c==0 {continue}
        g.mem.set_party_u8(0xC8+bay,c as u8);g.mem.set_party_u8(0xC4+bay,0);put_cash(g,payer,cash(g,payer).wrapping_sub(prices[c-1]));ui::clear(g);return
    }
}
