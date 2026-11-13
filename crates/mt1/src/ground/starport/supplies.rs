use super::*;
use ui::{Row,Shop};

fn name_rows(names:&mut [Vec<u8>],items:impl Iterator<Item=(Vec<u8>,u32)>) {
    let mut count=1;
    for (name,price) in items {
        let label=if price!=0 {count+=1;ui::font_bytes(format!("{}^{price}$",drawing::string(&name)))}else{count=1;name};
        if let Some(row)=names.get_mut(count-1) {*row=label}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_unpriced_item_resets_name_rows_like_trade_py() {
        let Some(output)=trade_edge_oracle("supplies") else {return};
        let prices=[0,10,20,30,0,40];
        let mut names:Vec<_>=(0..6).map(|i|format!("Previous{i}").into_bytes()).collect();
        name_rows(&mut names,prices.into_iter().enumerate().map(|(i,p)|(format!("Item{i}").into_bytes(),p)));
        assert_eq!(output.lines().count(),6);
        for (i,line) in output.lines().enumerate() {
            let fields:Vec<_>=line.split_whitespace().collect();assert_eq!(fields.len(),3);
            let expected:Vec<u8>=(0..fields[0].len()).step_by(2).map(|j|u8::from_str_radix(&fields[0][j..j+2],16).unwrap()).collect();
            assert_eq!(names[i],expected,"row {i}");
            assert_eq!(fields[1].parse::<usize>().unwrap(),i+1);
            assert_eq!(fields[2].parse::<u32>().unwrap(),prices[i]);
        }
        assert_eq!(names[0],b"Item4");assert_eq!(names[1],b"Item5^40$");
        assert_eq!(names[2],b"Item2^20$");assert_eq!(names[5],b"Previous5");
    }
}

pub fn run(g:&mut Ground)->Result<(),&'static str> {
    let mut limits:[u8;32]=std::array::from_fn(|i|g.mem.u8(0x1D7D+i as u16));
    if g.mem.party_u8(0xC1)&8!=0 {limits[23]=0}
    let percent=100-brokerage(g);
    let prices:[u32;32]=std::array::from_fn(|i|(g.mem.u32(0x1D9D+i as u16*4) as i32).wrapping_mul(percent as i32).wrapping_div(100) as u32);
    let percent=(80+use_skill(g,0x43)+use_skill(g,0x3D)/3+use_skill(g,9)/3+use_skill(g,0x28)/3).min(95);
    let percent=(percent-g.mem.party_u8(0xC2) as i16).max(0);
    let trade_in=(prices.get(g.mem.party_u8(0xC0) as usize+14).copied().unwrap_or(0) as i32).wrapping_mul(percent as i32).wrapping_div(100) as u32;
    for i in 0..26 {g.mem.set_u8(0x6E10+i,0)}
    let (mut programs,mut supplies)=(0u16,0u16);let mut max_quantity=None;
    let mut item_names=vec![Vec::new();32];
    loop {
        let payer=ui::payer(g,&string(g,0x261F),0,1);if payer==-1 {ui::clear(g);return Ok(())}let mut money=cash(g,payer);
        while programs<20&&g.mem.party_u8(0xAC+programs)!=0 {programs+=1}while supplies<20&&g.mem.party_u8(0xDE+supplies)!=0 {supplies+=1}
        loop {
            let mut cats=Vec::new();let mut mapping=Vec::new();
            for category in 0..4 {if category>1||(category==0&&supplies<20)||(category==1&&programs<20) {cats.push(Row::new(pointed(g,0x1D5B+category*4),1,0));mapping.push(category as usize)}}
            ui::limits(g,200,200);let n=cats.len();let c=ui::shop(g,&string(g,0x263B),&mut cats,Shop{rows:n,select:false,money:0,room:0,kind:0,sign:1});if c==0 {break}
            let category=[3usize,0,1,2][mapping[c-1]];ui::limits(g,40,150);
            let begin=g.mem.u8(0x1D6B+category as u16) as usize;let end=g.mem.u8(0x1D6C+category as u16) as usize;
            if begin>=end||end>32 {return Err("invalid supply category in asset table")}
            name_rows(&mut item_names,(begin..end).map(|i|(pointed(g,0x1CDB+i as u16*4),prices[i])));
            let mut rows:Vec<_>=(begin..end).map(|i|{let mut r=Row::new(item_names[i-begin].clone(),prices[i],limits[i]);if category==0 {r.weight=g.mem.u8(0x1D70+(i-begin) as u16)}r}).collect();
            let owner=if payer==-2 {string(g,0x242A)}else{drawing::bytes(g,party(g,0x102+payer as i32*256))};
            if category==1 {
                max_quantity=Some(2);ui::clear(g);let computer=pointed(g,0x1CDB+(g.mem.party_u8(0xC0) as u16+14)*4);
                ui::say(g,&ui::font_bytes(format!("{} will recieve {trade_in} credits for the Interloper's old {} computer.",drawing::string(&owner),drawing::string(&computer))),true,6);
                let product=g.scene.is_some();
                if product {
                    let under=g.mem.u8(0x6E2A)+1;let hot=super::shop_menu::choice_words(g,under,b"Accept",b"Cancel");g.frame("trade_in_choices");
                    crate::play_prompts::named(g,"supplies.trade-in",&[("accept","Accept",65),("back","Cancel",67)],&hot);
                } else {crate::play_prompts::named(g,"supplies.trade-in",&[("accept","Accept trade-in",13),("back","Cancel",27)],&[])}
                let accepted=loop {let k=getkey(g);if k==13||k==27 {break k==13}if product&&matches!(k,65|97|67|99) {break k|32==97}};
                if !accepted {set_text_box(g,1);redraw(g);continue}money=money.wrapping_add(trade_in);
            }
            set_text_box(g,1);if category==3 {max_quantity=Some(21-supplies)}set_text_box(g,1);if category==0 {max_quantity=Some(21-programs)}
            let Some(room)=max_quantity else {redraw(g);return Err("first-entry upgrades depend on unmodeled SS:max_quantity")};
            let accepted=ui::shop(g,&owner,&mut rows,Shop{rows:end-begin,select:false,money,room,kind:category as u8,sign:1});
            if accepted==0&&category==1 {money=money.wrapping_sub(trade_in)}redraw(g);if accepted==0 {continue}redraw(g);
            match category {
                0|3=>for (i,r) in rows.iter().enumerate() {if r.quantity==0 {continue}for _ in 0..r.quantity {
                    if category==3 {g.mem.set_party_u8(0xDE+supplies,(i+1) as u8);supplies=supplies.wrapping_add(1)}else{g.mem.set_party_u8(0xAC+programs,i as u8);programs=programs.wrapping_add(1)}
                }put_cash(g,payer,cash(g,payer).wrapping_sub(r.price.wrapping_mul(r.quantity as u32)))},
                1=>for (i,r) in rows.iter().enumerate().rev() {if r.quantity==0 {continue}for cell in 0..20 {g.mem.set_party_u8(0xAC+cell,g.mem.party_u8(0xAC+cell)&15)}g.mem.set_party_u8(0xC0,(i as u8).wrapping_sub(1));g.mem.set_party_u8(0xC2,0);put_cash(g,payer,cash(g,payer).wrapping_sub(r.price.wrapping_sub(trade_in)))},
                _=>{if rows.len()>2&&rows[1].quantity!=0&&rows[2].quantity!=0 {rows[1].quantity=0}
                    for (i,r) in rows.iter().enumerate().rev() {if r.quantity==0 {continue}let flags=g.mem.party_u8(0xC1);
                        if i<4 {g.mem.set_party_u8(0xC1,(flags&248)|((i as u8).wrapping_sub(1)))}
                        else if i==4||i==5 {let bit=if i==4 {8}else{16};if flags&bit!=0 {break}g.mem.set_party_u8(0xC1,flags|bit)}
                        put_cash(g,payer,cash(g,payer).wrapping_sub(r.price));
                    }
                }
            }
            break
        }
    }
}
