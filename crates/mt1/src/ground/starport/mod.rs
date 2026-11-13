use super::{input, render, state::{Far, Ground, Mem}, walk};
use input::{dialogue::set_text_box, ui as drawing};
use render::paint::Paint;
use mt_engine::present::prim::Raster;
pub mod ui;
mod cargo;
mod services;
pub use services::{input_field, save_stop, ask_over_ground, save_over_ground};
mod shipyard;
pub mod items;
pub mod supplies;
pub mod police;
pub mod shop_menu;
pub mod objects;
pub mod stores;
pub mod casino;
pub mod offices;

mod oracle_env { include!("../../../tests/oracle_env/mod.rs"); }
#[cfg(test)]
fn trade_edge_oracle(mode:&str)->Option<String> {
let Some(oracle)=oracle_env::oracle() else {return None};
    let root=std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let result=std::process::Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds","120","--mib","1024","--","python","-B"])
        .arg(root.join("crates/mt1/tests/starport_trade_edges.py")).arg(oracle).arg(mode)
        .env("PYTHONDONTWRITEBYTECODE","1").output().expect("Python needed for trade oracle");
    assert!(result.status.success(),"{}{}",String::from_utf8_lossy(&result.stdout),String::from_utf8_lossy(&result.stderr));
    Some(String::from_utf8(result.stdout).unwrap().lines().filter(|s|!s.starts_with("WATCHDOG ")).collect::<Vec<_>>().join("\n"))
}

pub fn money(g: &Ground, n: impl std::fmt::Display) -> String { if g.scene.is_some() { format!("${}", n.to_string().trim()) } else { format!("{n}$") } }
pub fn party(g: &Ground, offset: i32) -> Far { g.mem.pointer(0x6D65).add(offset) }
pub fn cash(g: &Ground, who: i16) -> u32 { g.mem.read(account(g,who),4) }
pub fn account(g: &Ground, who: i16) -> Far { party(g,if who == -2 {0x88} else {((who as i32)<<8)+0x1FE}) }
pub fn put_cash(g: &mut Ground, who: i16, value: u32) { g.mem.write(account(g,who),value,4) }
pub fn string(g: &Ground, off: u16) -> Vec<u8> { drawing::bytes(g,Mem::ds(off)) }
pub fn pointed(g: &Ground, off: u16) -> Vec<u8> { drawing::bytes(g,g.mem.pointer(off)) }
pub fn use_skill(g: &mut Ground, skill: u16) -> i16 {
    let b=(g.mem.u8(0x6B8F) as u16).wrapping_mul(256);
    let p=b.wrapping_add(0x17B).wrapping_add(skill);
    g.mem.set_party_u8(p,g.mem.party_u8(p).wrapping_add(1));
    g.mem.party_u8(b.wrapping_add(0x12B).wrapping_add(skill)) as i16
}
pub fn brokerage(g: &mut Ground) -> i16 { use_skill(g,0x43)*2+use_skill(g,0x3D)/3+use_skill(g,9)/3+use_skill(g,0x28)/3 }

pub const LEAVE_ROW: (i16, i16, i16, i16) = (11, 121, 12, 34);
pub fn cells() -> impl Iterator<Item = (i16, [i32; 4])> {
    (0..5).flat_map(|r| [(3 * r + 1, [67, 104 + 18 * r as i32, 45, 18]), (3 * r + 2, [112, 104 + 18 * r as i32, 45, 18])]).chain((0..3).map(|j| (15 + j, [154 + 53 * j as i32, 176, 53, 18])))
}
#[derive(Clone, Copy, PartialEq)]
enum Desk { Closed, Open, To(i16) }
thread_local! { static DESK: std::cell::Cell<Desk> = const { std::cell::Cell::new(Desk::Closed) }; }
fn tile_at(g: &Ground, col: i16, row: i16) -> Option<i16> {
    let inside = |r: [i32; 4]| (r[0]..r[0] + r[2]).contains(&(col as i32)) && (r[1]..r[1] + r[3]).contains(&(row as i32));
    if inside([LEAVE_ROW.1 as i32, LEAVE_ROW.0 as i32, LEAVE_ROW.3 as i32, LEAVE_ROW.2 as i32]) { return Some(0) }
    let mask = g.mem.u8(0x1AFE + g.mem.u8(0x5E14) as u16);
    cells().find(|&(slot, r)| inside(r) && mask & (1 << if slot >= 15 { 4 } else { slot / 3 }) != 0).map(|c| c.0)
}
pub fn getkey(g: &mut Ground) -> u16 {
    if matches!(DESK.get(), Desk::To(_)) { return 27 }
    let before = g.mouse_event; let key = input::menu_getkey(g);
    if key == 27 && DESK.get() == Desk::Open && g.scene.is_some() && g.lockstep.is_none() && !g.tooling.as_ref().is_some_and(|t| t.pending()) {
        if let Some(m) = g.mouse_event.filter(|m| m.event == mt_engine::io::mouse::RIGHT_RELEASE && (Some(*m) != before || g.mem.u8(0x6FEA) != 1)) {
            if let Some(n) = tile_at(g, m.col, m.row) { DESK.set(Desk::To(n)); g.mouse_event = None; g.mem.set_u16(input::CALLBACK, 0); }
        }
    }
    key
}
pub fn draw(g: &mut Ground) {let greeting=walk::random(g,10)/5;draw_with(g,greeting)}
fn draw_with(g: &mut Ground,greeting:i16) {
    let mut p=Paint::new(g,Raster::Wrap);
    p.ly("STPT",0,0,0);p.ly("STPT",18,21,26);
    p.ly("STPT",p.g.mem.u8(0x5E1F) as usize+10,23,82);
    let mut name=pointed(p.g,0x15E1+p.g.mem.u8(0x5E20) as u16*4);name.make_ascii_uppercase();
    name.extend_from_slice(b" STARPORT");p.text.shadow=true;p.text.bg=8;p.ink(2);p.at(8,26);p.text(&name);
    if p.g.scene.is_some() && 26+name.len() as i32*6 < LEAVE_ROW.1 as i32-2 { p.fill(LEAVE_ROW.0 as i32,LEAVE_ROW.1 as i32,LEAVE_ROW.2 as i32,LEAVE_ROW.3 as i32,7); p.text.shadow=false;p.text.bg=7;p.ink(0);p.at(LEAVE_ROW.0 as i32+2,LEAVE_ROW.1 as i32+4);p.text(b"Leave");p.text.bg=8;p.ink(2);p.text.shadow=true }
    for i in 0..5 {p.at(i*18+109,15);p.pointed_text(0x1C5F+i as u16*4)}
    p.ink(0);p.text.shadow=false;
    let mask=p.g.mem.u8(0x1AFE+p.g.mem.u8(0x5E14) as u16);
    for i in 0..5 {if mask&(1<<i)==0 {continue}
        for j in 0..2 {let off=0x1C73+i as u16*16+j as u16*8;
            if p.g.mem.pointer(off+4)!=Far::default() {
                p.at(i*18+106,j*45+69);p.pointed_text(off);p.at(i*18+113,j*45+69);p.pointed_text(off+4);
            } else {p.at(i*18+109,j*45+69);p.pointed_text(off)}
        }
        if i==4 {for j in 0..3 {p.at(i*18+106,j*53+156);p.pointed_text(0x1CC3+j as u16*8);p.at(i*18+113,j*53+156);p.pointed_text(0x1CC7+j as u16*8)}}
    }
    p.finish();
    let text=string(g,0x5E2A+greeting as u16*150);
    let mut p=Paint::new(g,Raster::Wrap);p.wrapped(&text,52,18,133,100,6,0,0);
    p.rect(p.g.mem.i16(0x6DF9) as i32,p.g.mem.i16(0x6DF5) as i32,p.g.mem.i16(0x6E02) as i32,p.g.mem.i16(0x6DFE) as i32,6);p.finish();
    render::flip(g);drawing::ink(g,6);g.mem.set_u8(0x707A,0);g.mem.set_u8(0x713F,g.mem.u8(0x713F)&!1);
    drawing::frame(g,"draw_starport_screen");
}
pub fn redraw(g:&mut Ground) {render::back(g);draw(g);render::front(g);set_text_box(g,0)}
pub fn restore_stop(g:&mut Ground) {services::save_stop(g,true)}
pub fn resume(g:&mut Ground) {
    g.screen_kind(crate::ui::ScreenKind::Picture);
    let (x,y)=(g.mem.i16(0x6DF5),g.mem.i16(0x6DF9));
    let sel=(0..18i16).find(|&s|{let col=if s>14 {s%15+3} else {s%3};g.mem.i16(0x1E65+col as u16*2)==x&&(if s>14 {176} else {s/3*18+104})==y}).unwrap_or(1);
    render::back(g);draw_with(g,0);render::front(g);set_text_box(g,0);bridge_from(g,g.mem.u8(0x5E14),sel);
}

pub fn enter(g:&mut Ground) {
    g.screen_kind(crate::ui::ScreenKind::Picture);
    for (o,v) in [(0x6DF5,0x43),(0x6DFE,0x70),(0x6DF9,0x68),(0x6E02,0x7A)] {g.mem.set_i16(o,v)}
    g.mem.set_u8(0x6DEE,g.mem.read(g.mem.pointer(0x5E02),1) as u8);
    g.mem.set_i16(0x6DEF,g.mem.party_u8(6) as i16*2+i16::from(g.mem.party_u8(7)!=0));
    redraw(g);
    crate::save::autosave(g,crate::save::Mode::Starport);
    bridge(g,g.mem.u8(0x5E14));
}
fn outline(g:&mut Ground,color:u8) {
    let mut p=Paint::new(g,Raster::Wrap);
    p.rect(p.g.mem.i16(0x6DF9) as i32,p.g.mem.i16(0x6DF5) as i32,p.g.mem.i16(0x6E02) as i32,p.g.mem.i16(0x6DFE) as i32,color);p.finish();
}
pub fn bridge(g:&mut Ground,kind:u8) {bridge_from(g,kind,1)}
fn bridge_from(g:&mut Ground,kind:u8,first:i16) {
    let mask=g.mem.u8(0x1AFE+kind as u16);let color=if g.mem.i16(0x503E)==2 {0} else {6};
    let (mut sel,mut dir,mut count)=(first,0i16,0i16);set_text_box(g,0);
    if mask==0 {return}
    loop {
        if g.mem.i16(0x1E65+sel as u16*2)!=g.mem.i16(0x6DF5)||sel/3*18+104!=g.mem.i16(0x6DF9) {
            outline(g,8);
            for _ in 0..36 {
                if count%3!=0 {count+=1} else {ui::clear(g);count=0}
                let row=(g.mem.i16(0x6E02)-122)/18;
                if row<0 || row>7 || mask&(1u8<<row as u32)==0 {sel+=dir}
                if sel<0 {sel=17} else if sel>17 {sel-=18} else if sel%3==0&&sel<15 {sel+=dir}
                sel=sel.rem_euclid(18);
                let col=if sel>14 {sel%15+3} else {sel%3};
                g.mem.set_i16(0x6DF5,g.mem.i16(0x1E65+col as u16*2));g.mem.set_i16(0x6DFE,g.mem.i16(0x1E67+col as u16*2));
                let y=if sel>14 {176} else {sel/3*18+104};g.mem.set_i16(0x6DF9,y);g.mem.set_i16(0x6E02,y+18);
                if mask&(1<<((y+18-122)/18))!=0 {break}
            }
            outline(g,color);
        }
        crate::play_prompts::bridge(g,mask,sel);
        g.mem.set_u8(0x6E2C,1);let key=input::menu_getkey(g);g.mem.set_u8(0x6E2C,0);
        let mut activate=false;
        if key>200 {let next=key as i16-200;
            let allowed=if key<215 {let row=(next/3*18)/18;row<8&&mask&(1<<row)!=0} else {mask&16!=0};
            if allowed && (0..18).contains(&next) {if next==sel {activate=true} else {sel=next}}
        } else {match key {
            27=>break,
            1 if mask!=1=>{sel=if sel>13 {11} else if sel<3 {sel+12} else {sel-3};dir=-3},
            2 if mask!=1=>{sel=if sel>13 {2} else if sel==13 {1} else {sel+3};dir=3},
            4=>{sel=if sel>12 {if sel==17 {13} else {sel+1}} else if (sel+1)%3==0 {sel/3*3+1} else {sel+1};dir=1},
            3=>{sel=if sel>12 {if sel==13 {17} else {sel-1}} else if (sel-1)%3==0 {sel/3*3+2} else {sel-1};dir=-1},
            13=>activate=true,_=>{}
        }}
        if sel<0 {sel=17} else if sel>17 {sel=0}
        if activate {DESK.set(Desk::Open);match sel {
            1=>services::info(g),2=>services::save_stop(g,false),
            4=>services::bank(g),5=>services::pensions(g),7=>services::recruit(g),
            8=>while !services::training(g) {},
            10=>cargo::buy(g),11=>cargo::sell(g),
            13=>shipyard::refuel(g),14=>shipyard::repair(g),
            15=>shipyard::sell_weapon(g),16=>shipyard::buy_weapon(g),
            17=>if let Err(e)=supplies::run(g) {eprintln!("starport supplies: {e}")},
            _=>{}
        }
        if let Desk::To(n)=DESK.replace(Desk::Closed) {if n!=0 {sel=n}}
        count=1}
    }
}

pub fn office(g: &mut super::state::Ground) -> bool {
    use super::{commands,input::ui,render,state::Mem};
    let event=g.mem.event();
    if !(11..=19).contains(&event) {return false}
    g.screen_kind(crate::ui::ScreenKind::Picture);
    ui::layout(g,1,54,0);
    render::shop_portrait(g,g.mem.u8(0x5DFB+event as u16) as i16,g.mem.i16(0xC0+2*(event as u16-11)));
    g.mem.set_u8(0xE5+g.mem.u16(0x52F4),100);
    commands::window(g,54,0,146,40);render::flip(g);
    if g.mem.u16(0x7000)!=0 {while g.input.mouse().buttons!=0 {g.poll()}g.mem.set_key(0)}
    g.mem.set_u8(0x6C3C,1);
    if (12..=18).contains(&event) {render::draw_menu(g,Mem::ds(0x451D));render::present_menu(g);render::front(g)}
    match event {
        11=>offices::vehicle_return(g),
        12=>casino::run(g),
        13=>offices::library(g,g.mem.u8(0x5E1E) as i16+0x1D,g.mem.u8(0x5E1E) as i16+0x21),
        14=>offices::tas(g),
        15=>offices::hospital(g),
        16=>stores::weapons_store(g,g.mem.u8(0x5E1B),g.mem.u8(0x5E1A)),
        17=>stores::armor_shop(g,g.mem.u8(0x5E1B)),
        18=>stores::general_store(g),
        19=>offices::imperial_refusal(g),
        _=>{}
    }
    if (12..=18).contains(&event) {g.mem.set_event_id(-1)}
    g.mem.set_u8(0x6C3C,0);render::back(g);
    render::draw_menu(g,Mem::ds(0x435B));render::present_menu(g);g.mem.set_key(0);
    render::walker_panel(g,true,true);commands::window(g,54,0,112,24);render::dismiss_message(g,1);
    g.screen_kind(crate::ui::ScreenKind::Ground);
    true
}

#[cfg(test)]
mod money_tests {
    #[test]
    fn money_sign_before_the_amount_in_product_mode_only() {
        let mut g = crate::ground::orders::t::ground();
        assert_eq!((super::money(&g, 3750), super::money(&g, format!("{:4}", 750)), super::money(&g, 111167u32)), ("$3750".to_string(), "$750".to_string(), "$111167".to_string()));
        g.scene = None;
        assert_eq!((super::money(&g, 3750), super::money(&g, format!("{:4}", 750))), ("3750$".to_string(), " 750$".to_string()));
    }

    #[test]
    fn buy_then_sell_ship_weapon_by_native_keys_and_mouse() {
        use super::*;
        use mt_engine::{io::{keys, Dir, Event, Input, MouseEvent, Scripted}, sim::tick::{Clock, Counter}};
        use std::collections::VecDeque;
        struct Native { events: VecDeque<Event>, stages: VecDeque<(&'static str, Vec<Event>)>, polls: usize }
        impl Input for Native {
            fn next_key(&mut self) -> u16 { loop { if let Event::Key(k) = self.events.pop_front().expect("native tape exhausted") { return k } } }
            fn poll(&mut self, clock: &mut dyn Clock) -> Event {
                self.polls += 1; assert!(self.polls < 1000,"native tape stalled");
                self.events.pop_front().unwrap_or_else(|| { clock.wait_tick(); Event::Idle })
            }
            fn wants_decision(&self) -> bool { true }
            fn decide(&mut self, d: &mt_engine::play::Decision) {
                if self.stages.front().is_some_and(|s| s.0 == d.context.routine) && !self.events.iter().any(|e| matches!(e,Event::Key(_)|Event::Mouse(_))) {
                    self.events.extend(self.stages.pop_front().unwrap().1);
                }
            }
        }
        let Some(dir) = oracle_env::gamefiles() else { return };
        for click in [false, true] {
            let mut g = Ground::new(&std::fs::read(dir.join("T.DAT")).unwrap(), std::fs::read(dir.join("DEFAULT.SAV")).unwrap(),
                Box::new(Scripted::new([])), Box::new(Counter::at(1234)), Box::new(Dir(dir.clone()))).unwrap();
            g.mouse_driver = true; crate::ground::start(&mut g);
            g.mem.set_party_u8(0xC1, g.mem.party_u8(0xC1) & 127); g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
            g.scenario_boot = true; assert!(render::game_init(&mut g)); g.scenario_boot = false;
            g.mem.set_party_u8(0xC8, 0); g.mem.set_party_u8(0xC9, 0); put_cash(&mut g, -2, 10_000_000);
            let keys = |ks: &[u16]| ks.iter().flat_map(|&k| [Event::Key(k),Event::KeyUp(k)]).collect::<Vec<_>>();
            let mut sale = if click { vec![Event::Mouse(MouseEvent { event: 4, buttons: 0, col: 180, row: 185 })] } else { keys(&[keys::LEFT]) };
            sale.extend(keys(&[keys::ENTER]));
            let payer = keys(&[keys::DOWN,keys::DOWN,keys::DOWN,keys::DOWN,keys::DOWN,keys::ENTER]);
            let stages = [
                ("13FF2",keys(&[keys::DOWN,keys::DOWN,keys::DOWN,keys::DOWN,keys::RIGHT,keys::RIGHT,keys::RIGHT,keys::ENTER])),
                ("1199B",payer.clone()), ("11571",keys(&[keys::ENTER])),
                ("13FF2",sale), ("1199B",payer), ("11571",keys(&[keys::ENTER])), ("13FF2",keys(&[keys::ESC])),
            ].into();
            g.input = Box::new(Native { events: VecDeque::new(), stages, polls: 0 }); enter(&mut g);
            assert_eq!((g.mem.party_u8(0xC8),g.mem.party_u8(0xC9)),(0,0),"sale removed the purchased laser, click={click}");
            assert!((9_976_501..10_000_000).contains(&cash(&g,-2)),"sale credits part of the 23,500 purchase, click={click}: {}",cash(&g,-2));
        }
    }
}
