use super::*;
use ui::{Row,Shop};
fn name(g:&Ground,who:i16)->String {drawing::string(&drawing::bytes(g,party(g,0x102+who as i32*256)))}
pub fn info(g:&mut Ground) {
    set_text_box(g,1);ui::clear(g);
    for (off,first,blank) in [(0x2AE8,true,1),(0x2B1B,false,2),(0x2B6B,false,1),(0x2BCE,false,1),(0x2C07,false,0)] {
        ui::say_at(g,off,first);g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(blank));
    }
    getkey(g);redraw(g);
}
pub fn pensions(g:&mut Ground) {
    let now=g.mem.party_u16(0xF84) as i16;let mut paid=false;
    for who in 0..5i16 {let b=who as u16*256;let mut date=g.mem.party_u16(b+0x1FC) as i16;
        if date<30 {date=30;g.mem.set_party_u16(b+0x1FC,30)}
        if g.mem.party_u8(b+0x127)>0&&date<=now {
            let months=((now.wrapping_sub(date))/30+1) as i8;
            let amount=(g.mem.party_u8(b+0x129) as i16).wrapping_mul(months as i16).wrapping_mul(20) as i32 as u32;
            if months>0&&amount>0 {
                g.mem.set_party_u16(b+0x1FC,(now/30+1).wrapping_mul(30) as u16);paid=true;
                let msg=format!("{} collects {months} months of retirement pay {amount} credits",name(g,who));
                put_cash(g,who,cash(g,who).wrapping_add(amount));ui::clear(g);ui::say(g,&ui::font_bytes(msg),true,who as u8+1);
                if getkey(g)==27 {return}
            }
        }
    }
    if !paid {ui::clear(g);ui::say(g,b"No one recieved pension payment.",true,6)}
}
pub fn input_field(g:&mut Ground,maxlen:usize,alpha:bool)->Option<Vec<u8>> {
    crate::play_prompts::text(g,maxlen,alpha);
    let origin=(g.mem.i16(0x7139),g.mem.i16(0x713B));let mut s=Vec::new();ui::ink(g,6);
    loop {let k=getkey(g) as u8;
        if k==27 {return None}if k==13 {return Some(s)}
        if (k==14||k==3)&&!s.is_empty() {s.pop();let width=g.mem.i16(0x713B)-origin.1;ui::at(g,origin.0,origin.1);ui::fill(g,origin.0,origin.1,8,width,9);ui::text(g,&s)}
        else if s.len()<maxlen.saturating_sub(1)&&((alpha&&k.is_ascii_uppercase())||k.is_ascii_digit()) {s.push(k);ui::text(g,&[k])}
    }
}
pub fn bank(g:&mut Ground) {
    ui::clear(g);
    loop {
        ui::ink(g,10);let living=(0..5i16).any(|i|g.mem.party_u8(0x127+i as u16*256)!=0&&cash(g,i)!=0);
        let ship=cash(g,-2);let mut rows=Vec::new();
        if living {rows.push(Row::new(pointed(g,0x1ECA),1,0))}if ship!=0 {rows.push(Row::new(pointed(g,0x1ECE),1,0))}
        if rows.is_empty() {ui::clear(g);ui::say(g,b"There are no funds in any of your available accounts.",true,6);return}
        let n=rows.len();let c=ui::shop(g,b"Select transaction type:",&mut rows,Shop {rows:n,select:false,money:ship,room:0,kind:0,sign:1});
        if c==0||c==3 {ui::clear(g);return}let withdrawal=c==2||(!living&&ship!=0);
        loop {
            let who=ui::payer(g,if withdrawal {b"Who will recieve money?"} else {b"Who will deposit money?"},if withdrawal {50} else {255},if withdrawal {1} else {0});
            if who==-1 {break}let balance=if withdrawal {cash(g,-2)} else {cash(g,who)};let h=g.mem.i16(0x6E08);ui::limits(g,h+50,h+50);
            let mut value=None;
            loop {
                ui::clear(g);ui::say(g,if withdrawal {b"^Withdrawal"} else {b"^Deposit"},true,10);
                let text=format!("{} : {}",if withdrawal {drawing::string(&string(g,0x242A))} else {name(g,who)},money(g,balance));
                ui::say(g,&ui::font_bytes(text),false,if withdrawal {6} else {who as u8+1});ui::say(g,b"amount: ",false,if withdrawal {who as u8+1} else {6});
                let Some(s)=input_field(g,8,false) else {break};let amount=drawing::string(&s).parse::<u32>().unwrap_or(0);
                if amount<=balance {value=Some(amount);break}
                g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(4));ui::say(g,b"transaction exceeds account balance!",false,6);walk::wait_ticks(g,18);
            }
            if let Some(v)=value {put_cash(g,who,if withdrawal {cash(g,who).wrapping_add(v)} else {cash(g,who).wrapping_sub(v)});put_cash(g,-2,if withdrawal {cash(g,-2).wrapping_sub(v)} else {cash(g,-2).wrapping_add(v)});break}
        }
    }
}
pub fn training(g:&mut Ground)->bool {
    ui::clear(g);let payer=ui::payer(g,b"Who will pay for training?",0,1);if payer==-1 {ui::clear(g);return true}
    let funds=cash(g,payer);set_text_box(g,1);ui::limits(g,100,240);let mut rows=Vec::new();let mut mapping=Vec::new();
    'members: for who in 0..5u16 {let b=who*256;if g.mem.party_u8(b+0x127)==0 {continue}
        let total:u16=(0..80).map(|j|g.mem.party_u8(b+0x12B+j) as u16).sum();
        if g.mem.party_u8(b+0x119) as u16+g.mem.party_u8(b+0x11A) as u16<=total {continue}
        for skill in 0..80u16 {if g.mem.party_u8(b+0x17B+skill)<50||rows.len()>=50||(skill==36&&g.mem.party_u8(b+0x19F)<100) {continue}
            let price=5000*g.mem.party_u8(b+0x12B+skill) as u32+5000*g.mem.u8(0x6DEE) as u32+10000*g.mem.u8(0x1599+skill) as u32;
            rows.push(Row::new(ui::font_bytes(format!("{}^{}^{}",name(g,who as i16),drawing::string(&pointed(g,0x1479+skill*4)),money(g,format!("{price:4}")))),price,0));mapping.push((who,skill));if rows.len()==49 {break 'members}
        }
    }
    ui::clear(g);
    if rows.is_empty() {
        if no_drives(g) {
            ui::say(g,b"No one has practised a skill enough to train yet.",true,6);g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(1));ui::say(g,b"Press a key ...",false,10);
            g.frame("message");crate::play_prompts::ack(g,"129F5.none");getkey(g);
        } else {ui::say(g,b"No one is eligible for training.",true,6);walk::wait_ticks(g,36)}
        redraw(g);return false
    }
    let height=rows.len().min(16);let c=ui::shop(g,b"Training Eligibility",&mut rows,Shop {rows:height,select:true,money:funds,room:0,kind:0,sign:1});redraw(g);
    if c==0 {return false}ui::clear(g);let cost=rows[c-1].price;let (who,skill)=mapping[c-1];
    if cost<=cash(g,payer) {let b=who*256;g.mem.set_party_u8(b+0x17B+skill,0);g.mem.set_party_u8(b+0x12B+skill,g.mem.party_u8(b+0x12B+skill).wrapping_add(1));put_cash(g,payer,cash(g,payer).wrapping_sub(cost));
        ui::say(g,&ui::font_bytes(format!("{} recieves training in {}.",name(g,who as i16),drawing::string(&pointed(g,0x1479+skill*4)))),true,who as u8+1);
    } else if payer==-2 {ui::say(g,b"The party account is unable to pay for training.",true,6)} else {ui::say(g,&ui::font_bytes(format!("{} is unable to pay for training.",name(g,payer))),true,who as u8+1)}true
}

pub fn no_drives(g:&Ground)->bool {crate::save::window(g)}
pub fn positions(g:&mut Ground,restore:bool)->Option<crate::save::Entry> {
    let list:Vec<_>=crate::save::list(g).into_iter().filter(|e|if restore {e.used()} else {matches!(e.pos,crate::save::Pos::Game(_))}).collect();
    if list.is_empty() {ui::clear(g);ui::say(g,b"No saved game.",true,6);g.frame("message");walk::wait_ticks(g,36);ui::clear(g);return None}
    let widths=drawing::font(g).map(|f|f.widths);
    let px=|s:&str|s.bytes().map(|c|widths.map_or(6,|w|w[c as usize] as i16)).sum::<i16>();
    let (x,w)=(g.mem.i16(0x6E08),g.mem.i16(0x6E00));
    let cols=crate::save::rows(&list);
    let place_x=x+15+cols.iter().map(|c|px(&c[0])).max().unwrap_or(0)+4;
    let detail_x=x+w-1-cols.iter().map(|c|px(&c[2])).max().unwrap_or(0);
    let fits=|s:&str|px(s)<=detail_x-4-place_x;
    let cut=|s:&str| {let mut s=if fits(s) {s} else {s.strip_suffix(" starport").unwrap_or(s)}.to_string();while !fits(&s)&&s.pop().is_some() {}s};
    let mut rows:Vec<_>=cols.iter().map(|c|Row::new(ui::font_bytes(format!("{}^{}^{}",c[0],cut(&c[1]),c[2])),1,0)).collect();
    let visible=((g.mem.i16(0x6E04)-5).max(1) as usize).min(rows.len());
    let old=(g.mem.i16(0x5228),g.mem.i16(0x522A));ui::limits(g,place_x,detail_x);
    let c=ui::shop(g,if restore {crate::save::RESTORE_TITLE} else {crate::save::SAVE_TITLE}.as_bytes(),&mut rows,Shop::select(visible));
    ui::limits(g,old.0,old.1);
    (c!=0).then(||list[c-1].clone())
}
pub fn confirm(g:&mut Ground,question:&str)->bool {
    let mut rows=vec![Row::new(b"No".to_vec(),1,0),Row::new(b"Yes".to_vec(),1,0)];
    ui::shop(g,&ui::font_bytes(question),&mut rows,Shop::select(2))==2
}
fn over_ground<T>(g:&mut Ground,f:impl FnOnce(&mut Ground)->T)->T {
    const BOX:[u16;6]=[0x6E0A,0x6E0E,0x6E08,0x6E00,0x6E06,0x6E04];
    let (old,pointer)=(BOX.map(|o|g.mem.i16(o)),g.mem.i16(0x7012));
    g.screen_kind(crate::ui::ScreenKind::Picture);g.mem.set_i16(0x7012,0);set_text_box(g,1);
    let r=f(g);
    render::main_screen(g,0);for (o,v) in BOX.into_iter().zip(old) {g.mem.set_i16(o,v)}
    g.screen_kind(crate::ui::ScreenKind::Ground);g.mem.set_i16(0x7012,pointer);g.mem.set_redraw_pending(1);r
}
pub fn ask_over_ground(g:&mut Ground,question:&str)->bool {over_ground(g,|g|confirm(g,question))}
pub fn save_over_ground(g:&mut Ground,restore:bool) {save_stop_product(g,restore,crate::save::Mode::Ground)}
fn pick(g:&mut Ground,restore:bool)->Option<crate::save::Entry> {
    let mut pick=positions(g,restore);
    if !restore {if let Some(e)=pick.take_if(|e|e.used()) {if confirm(g,&format!("Overwrite position {}?",e.name())) {pick=Some(e)}}}
    pick
}
fn save_stop_product(g:&mut Ground,restore:bool,mode:crate::save::Mode) {
    let ground=restore||mode==crate::save::Mode::Ground;
    let pick=if ground {over_ground(g,|g|pick(g,restore))} else {
        g.mem.set_i16(0x7012,0);set_text_box(g,1);let p=pick(g,false);
        render::back(g);super::draw_with(g,0);render::front(g);set_text_box(g,0);ui::clear(g);g.mem.set_i16(0x7012,1);p
    };
    let Some(e)=pick else {return};
    let result=if restore {crate::save::read_file(g,&e.file).map(|s|g.restore_pending=Some(s))} else {crate::save::write_file(g,mode,&e.file)};
    if g.restore_pending.is_some() {return}
    let line=result.err().unwrap_or_else(||"Game saved.".to_string());
    if ground {return crate::save::note(g,line)}
    g.mem.set_i16(0x7012,0);ui::say(g,&ui::font_bytes(line),true,6);g.frame("message");walk::wait_ticks(g,36);ui::clear(g);g.mem.set_i16(0x7012,1);
}
pub fn save_stop(g:&mut Ground,restore:bool) {
    if no_drives(g)&&crate::save::available(g) {return save_stop_product(g,restore,crate::save::Mode::Starport)}
    g.mem.set_i16(0x7012,0);set_text_box(g,if restore {2} else {0});ui::clear(g);
    ui::say(g,if restore {b"Select drive to restore from:"} else {b"Select drive to save to:"},true,10);
    g.lockstep_stop(if restore {"restore"} else {"save"});
    crate::play_prompts::ack(g,if restore {"restore"} else {"save"});
    let key=getkey(g);
    ui::clear(g);g.mem.set_i16(0x7012,1);
    if key==27||!crate::save::available(g) {return}
    let result=if restore {crate::save::read(g).map(|s|g.restore_pending=Some(s))} else {crate::save::write(g,crate::save::Mode::Starport)};
    if g.restore_pending.is_some() {return}
    g.mem.set_i16(0x7012,0);ui::say(g,&ui::font_bytes(result.err().unwrap_or_else(||"Game saved.".into())),true,6);g.frame("message");walk::wait_ticks(g,36);ui::clear(g);g.mem.set_i16(0x7012,1);
}

pub fn recruit(g:&mut Ground) {
    set_text_box(g,0);ui::clear(g);
    let Some(who)=(0..5u16).find(|&i|g.mem.party_u8(0x127+i*256)==0) else {ui::clear(g);ui::say(g,b"Your party is full.",true,6);walk::wait_ticks(g,18);ui::clear(g);return};
    let direct=no_drives(g);let mut letter='A';
    loop {
        if !direct {
        let mut drives:Vec<_>=(0..6).map(|i|Row::new(pointed(g,0x1E4D+i*4),1,0)).collect();
        let drive=ui::shop(g,b"Select drive to read characters from:",&mut drives,Shop::select(6));if drive==0 {break}
        ui::clear(g);letter=(drive as u8+b'@') as char;
        ui::say(g,&ui::font_bytes(format!("Ready to load characters from {letter}: If this is a floppy drive, put your save/character disk in drive {letter}: now.")),true,6);ui::say(g,b"Press a key ...",false,10);
        if getkey(g)==27 {continue}
        }
        let Some(data)=g.files().read("UCHARS.DAT").or_else(||g.files().read("DCHARS.DAT")) else {ui::clear(g);ui::say(g,b"No characters found.",true,6);walk::wait_ticks(g,18);if direct {break} continue};
        let count=data.first().copied().unwrap_or(0) as usize;
        if count==0||data.len()<1+256*count {ui::clear(g);ui::say(g,b"No characters found.",true,6);walk::wait_ticks(g,18);if direct {break} continue}
        let pool=&data[1..1+256*count];let mut rows:Vec<_>=pool.chunks_exact(256).map(|c|Row::new(c[..c.iter().position(|&b|b==0).unwrap_or(256)].to_vec(),1,0)).collect();
        let c=ui::shop(g,b"Select character to try to recruit:",&mut rows,Shop::select(count.min(10)));if c==0 {if direct {break} continue}
        let candidate=&pool[(c-1)*256..c*256];let candidate_name=drawing::string(&rows[c-1].name);
        if (0..5).any(|i|name(g,i)==candidate_name) {ui::clear(g);ui::say(g,&ui::font_bytes(format!("{candidate_name} refuses to join your party.")),true,6);walk::wait_ticks(g,36);continue}
        let b=g.mem.u8(0x6B8F) as u16*256;let base3=g.mem.party_u8(b.wrapping_add(0x15C)) as i16*3;
        g.mem.set_party_u8(b.wrapping_add(0x1AC),g.mem.party_u8(b.wrapping_add(0x1AC)).wrapping_add(1));let range=26-base3;
        g.mem.write_bytes(party(g,0x102+who as i32*256),candidate);for slot in 0..23 {g.mem.set_party_u16(0x1CE+who*256+slot*2,0)}ui::clear(g);
        let oxygen=items::allocate(g,40,true);if oxygen!=0 {g.mem.set_party_u16(0x1F4+who*256,oxygen as u16)}
        let id=walk::random(g,range).wrapping_add(base3);let gun=items::allocate(g,id,false);if gun!=0 {super::super::commands_items::add(g,who as i16,gun);}
        let id=walk::random(g,10)+45;let extra=items::allocate(g,id,true);if extra!=0 {super::super::commands_items::add(g,who as i16,extra);}
        ui::say(g,&ui::font_bytes(format!("{} recruited.",name(g,who as i16))),true,6);
        for j in 0..6 {if g.mem.party_u8(0xA0+j)==0 {g.mem.set_party_u8(0xA0+j,who as u8+1);break}}
        g.mem.set_u8(0x6E2A,g.mem.u8(0x6E2A).wrapping_add(2));if !direct {ui::say(g,&ui::font_bytes(format!("If necessary, reinsert any game disk removed from drive {letter}: now.")),false,6)}ui::say(g,b"Press a key ...",false,10);getkey(g);ui::clear(g);return
    }
    ui::clear(g);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt_engine::{io::{keys, Dir, Event, Input, Scripted}, sim::tick::{Clock, Counter}};
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};
    struct Tape { events: VecDeque<Event>, seen: Rc<RefCell<Vec<String>>> }
    impl Input for Tape {
        fn next_key(&mut self) -> u16 { loop { if let Event::Key(k) = self.events.pop_front().expect("tape exhausted") { return k } } }
        fn poll(&mut self, clock: &mut dyn Clock) -> Event { self.events.pop_front().unwrap_or_else(|| { clock.wait_tick(); Event::Idle }) }
        fn wants_decision(&self) -> bool { true }
        fn decide(&mut self, d: &mt_engine::play::Decision) {
            if self.seen.borrow().last().map(String::as_str) == Some(d.context.routine.as_str()) { return }
            self.seen.borrow_mut().push(d.context.routine.clone());
            self.events.extend([Event::Key(keys::ENTER), Event::KeyUp(keys::ENTER)]);
        }
    }
    #[test]
    fn no_course_is_said_in_words_and_waits_for_a_key() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        for product in [true, false] {
            let mut g = Ground::new(&std::fs::read(dir.join("T.DAT")).unwrap(), std::fs::read(dir.join("DEFAULT.SAV")).unwrap(),
                Box::new(Scripted::new([])), Box::new(Counter::at(1234)), Box::new(Dir(dir.clone()))).unwrap();
            g.mouse_driver = true; crate::ground::start(&mut g);
            g.mem.set_party_u8(0xC1, g.mem.party_u8(0xC1) & 127); g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0); g.scenario_boot = true; assert!(render::game_init(&mut g)); g.scenario_boot = false;
            for who in 0..5u16 { for skill in 0..80 { g.mem.set_party_u8(who * 256 + 0x17B + skill, 0) } }
            g.scene = product.then(crate::ui::SceneHook::default);
            let seen = Rc::new(RefCell::new(vec![]));
            g.input = Box::new(Tape { events: VecDeque::new(), seen: seen.clone() });
            let party = g.mem.party().to_vec();
            set_text_box(&mut g, 0); assert!(!training(&mut g));
            assert_eq!(seen.borrow().iter().any(|r| r == "129F5.none"), product, "the no-course wait, product={product}: {:?}", seen.borrow());
            assert_eq!(g.mem.party(), party, "nothing trained, nothing paid");
        }
    }
}
