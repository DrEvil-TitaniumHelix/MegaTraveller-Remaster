mod oracle_env;
use mt1::ground::{state::{Far,Ground,Mem},commands_items as items,starport::{casino,objects::{self,Allocation},police,shop_menu}};
use mt_engine::{io::{Dir,Event,Input},sim::tick::{Clock,Counter}};
use std::{path::PathBuf,process::Command};
struct Idle;
impl Input for Idle {
    fn next_key(&mut self)->u16 {27}
    fn poll(&mut self,clock:&mut dyn Clock)->Event {clock.wait_tick();Event::Idle}
}
fn host()->Ground {
    let mut g=Ground::from_ds(vec![],vec![0;Mem::PARTY_SIZE],Box::new(Idle),Box::new(Counter::default()),Box::new(Dir(PathBuf::new())));
    g.mem.alloc(0x7000,65536);g.mem.set_pointer(0x6D6D,Far::new(0,0x7000));
    for i in 1..=256 {let p=items::object(&g,i);g.mem.write_bytes(p,&[255,0xa5,0x3c,0xde,0xad])}
    for (i,(lo,hi)) in [(0,9),(10,12),(13,21),(22,26),(28,29),(30,33),(34,36),(37,45),(46,47),(48,59),(60,64),(70,78)].into_iter().enumerate() {
        g.mem.set_i16(0x209e+4*i as u16,lo);g.mem.set_i16(0x20a0+4*i as u16,hi);
    }
    g
}
fn hex(s:&str)->Vec<u8>{(0..s.len()/2).map(|i|u8::from_str_radix(&s[2*i..2*i+2],16).unwrap()).collect()}
#[test]
fn independent_services_allocators_and_exhaustive_casino() {
    let Some(oracle)=oracle_env::oracle() else {return};
    let output=Command::new("python").arg("-B").arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/ground_shops_oracle.py")).arg(oracle).env("PYTHONDONTWRITEBYTECODE","1").output().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let g=host();let mut counts=[0;4];let mut complete=false;
    for line in String::from_utf8(output.stdout).unwrap().lines() {
        let f:Vec<_>=line.split_whitespace().collect();
        match f[0] {
            "R"=>{assert_eq!(casino::reel_symbol(&g,f[1].parse().unwrap()),f[2].parse::<i16>().unwrap());counts[0]+=1},
            "P"=>{
                let a=f[1].parse::<i16>().unwrap();let data=hex(f[2]);
                for b in 0..78i16 {for c in 0..78i16 {
                    let at=(b as usize*78+c as usize)*4;let expected=u32::from_le_bytes(data[at..at+4].try_into().unwrap());
                    let reels=[a,b,c];assert_eq!(casino::award(reels,reels.map(|v|casino::reel_symbol(&g,v))),expected,"reels {reels:?}");counts[1]+=1;
                }}
            },
            "D"=>{
                let skill=f[1].parse::<i16>().unwrap();let mut totals=[0u64;3];
                for a in 0..78i16 {
                    let lo=if skill==0 {0} else {(a-2*(10-skill)).max(0)};
                    let width=if skill==0 {78} else {(4*(10-skill)).min(78-lo)};
                    let weight=|v:i16,n:i16|32768u64/n as u64+u64::from((v as u64)<32768%n as u64);
                    for b in lo..lo+width {for c in lo..lo+width {
                        let reels=[a,b,c];let pay=casino::award(reels,reels.map(|v|casino::reel_symbol(&g,v))) as u64;
                        let w=weight(a,78)*weight(b-lo,width)*weight(c-lo,width);
                        totals[0]+=w;totals[1]+=w*u64::from(pay!=0);totals[2]+=w*pay;
                    }}
                }
                assert_eq!(totals,[f[2].parse().unwrap(),f[3].parse().unwrap(),f[4].parse().unwrap()],"skill {skill}");counts[2]+=1;
            },
            "A"=>{
                let mut h=host();let mode=f[1].parse::<usize>().unwrap();let first=f[2].parse::<usize>().unwrap();let id=f[3].parse::<i16>().unwrap();
                for i in 0..first {let p=items::object(&h,i as i16+1);h.mem.write(p,7,1)}
                let def=items::definition(&h,id);h.mem.write(def.add(0x27),(id as u8).wrapping_mul(31) as u32,1);h.mem.write(def.add(0x28),(id as u8).wrapping_mul(7) as u32,1);
                assert_eq!(objects::alloc(&mut h,id,[Allocation::Empty,Allocation::Full,Allocation::Store][mode]),f[4].parse::<i16>().unwrap());
                assert_eq!(h.mem.bytes(items::object(&h,1),1280),hex(f[5]),"mode {mode}, first {first}, id {id}");counts[3]+=1;
            },
            "PASS"=>complete=true,
            _=>panic!("unexpected oracle record"),
        }
    }
    assert!(complete);assert_eq!(counts,[153,78*78*78,10,75]);
}
#[test]
fn allocation_rollbacks_and_original_discard_bug() {
    let mut g=host();
    for slot in 0..19 {g.mem.set_party_u16(items::inv(0,slot),1)}
    assert_eq!(objects::give(&mut g,0,45,Allocation::Store),-1);
    assert_eq!(g.mem.read(items::object(&g,1),1),255);
    for slot in 0..19 {g.mem.set_party_u16(items::inv(0,slot),0)}
    for i in 2..=256 {let p=items::object(&g,i);g.mem.write(p,0,1)}
    assert_eq!(objects::give_gun_with_ammo(&mut g,0,29),0,"second allocation failure returns success-like zero");
    assert_eq!(g.mem.read(items::object(&g,1),1),255);
    let obj=items::object(&g,1);g.mem.write(obj,29,1);g.mem.set_party_u16(items::inv(0,0),1);
    objects::discard_slot_item(&mut g,0,0);
    assert_eq!(g.mem.party_u16(items::inv(0,0)),0);assert_eq!(g.mem.read(obj,1),29);
    assert_eq!(g.mem.party_u8(0xA7D),255,"17970 frees object zero after remove cleared the slot");
}
#[test]
fn police_bail_uses_stale_sixth_character_and_clears_held_list() {
    let mut g=host();g.mem.alloc(0x7100,16);g.mem.set_pointer(0x5E02,Far::new(0,0x7100));g.mem.write(Far::new(6,0x7100),8,1);
    g.mem.set_u8(0x5E1A,8);g.mem.set_outdoors(1);g.mem.set_provoked(2);g.mem.set_ship_count(1);
    g.mem.set_party_u8(0x127,1);shop_menu::set_cash(&mut g,0,30000);shop_menu::set_cash(&mut g,-2,4000);
    g.mem.set_party_u8(0x627,1);shop_menu::set_cash(&mut g,5,9000);
    police::encounter(&mut g);
    assert_eq!(shop_menu::cash(&g,-2),0);assert_eq!(shop_menu::cash(&g,0),30000);assert_eq!(shop_menu::cash(&g,5),3000);
    assert_eq!((g.mem.provoked(),g.mem.outdoors(),g.mem.ship_count()),(0,0,0));assert_eq!(g.now(),90);
}
#[test]
fn repeated_search_clears_latch_and_extreme_selected_byte_wraps() {
    let mut g=host();g.mem.set_outdoors(1);g.mem.set_key(80);
    assert_eq!(police::weapons_search(&mut g),1);assert_eq!(g.mem.key(),0);
    g.mem.set_selected_char(255);g.mem.set_party_u8(1,255);
    shop_menu::skill_bonus(&mut g);casino::slot_spin(&mut g,&mut [0;3],0,true);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
