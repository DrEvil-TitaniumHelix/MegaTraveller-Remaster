use super::{Space, state::*};
pub mod image;
use mt_engine::{io::{keys, Event}, play::{Decision, Fnv, Kind, Option, TimeInput, HASH_VERSION}};

pub fn hash(sp: &mut Space) -> String {
    let mut h = Fnv::default(); h.write(&HASH_VERSION.to_le_bytes()); h.field(b"s");
    h.field(sp.s.image()); h.field(&sp.s.party); h.field(&sp.s.world);
    h.write(&sp.last_ticks.to_le_bytes()); h.write(&sp.clock.now().to_le_bytes()); h.write(&sp.clock.ticks().to_le_bytes());
    h.write(&[sp.dead as u8]); h.field(format!("{:?}",sp.mouse).as_bytes()); h.field(format!("{:?}",sp.scenario_entry).as_bytes());
    h.field(&sp.r.fb.regs); for color in sp.dac { h.write(&color); }
    h.field(format!("{:?}",sp.play_flight).as_bytes());
    h.state()
}
pub fn decide(sp: &mut Space, mut d: Decision) {
    if sp.observer.is_some() || !sp.input.wants_decision() { return }
    if sp.input.data_mode() {
        let tick = sp.clock.now(); let state = crate::play::observation::Observation::space(sp, tick); sp.input.scene(&state);
    }
    let base = hash(sp);
    if d.state.is_empty() { d.state = base; }
    else { let mut h=Fnv::default(); h.field(base.as_bytes()); h.field(d.state.as_bytes()); d.state=h.state(); }
    sp.input.decide(&d);
}
pub fn flight(sp: &mut Space) {
    if sp.observer.is_some() || sp.s.byte(KEY)!=0 || sp.s.byte(PENDING_KEY)!=0 || sp.s.word(ENCOUNTER)>-1 { return }
    if !sp.input.wants_decision() && !sp.input.data_mode() { return }
    let mut d = crate::play_prompts::keys_decision("s","4FCB",Kind::Space,&[
        ("cmd:thrust","Increase thrust",keys::UP),("cmd:reverse","Decrease thrust",keys::DOWN),
        ("cmd:left","Turn left",keys::LEFT),("cmd:right","Turn right",keys::RIGHT),
        ("cmd:stop","Stop",0x14C),("cmd:launch","Take off",b'T' as u16),
        ("cmd:land","Land",b'L' as u16),("cmd:jump","Jump",b'J' as u16),
        ("cmd:stations","Crew stations",b'S' as u16),("cmd:communications","Communications",b'C' as u16),
        ("cmd:quit","Quit flight",b'Q' as u16),
    ]);
    sp.play_tactical = false;
    if sp.product { for i in 0..5 { d.options.push(Option::new(format!("cmd:char/{i}"),format!("Traveller {}",i+1),vec![Event::Key(keys::F1+i)])); } }
    if sp.product && !sp.input.data_mode() { d.options.push(Option::new("cmd:pause","Pause",vec![Event::Key(b'P' as u16)])); }
    flight_rects(sp,&mut d);
    d.time = Some(TimeInput { max_ticks: 65535, directions: [("left",keys::LEFT),("right",keys::RIGHT),("forward",keys::UP),("reverse",keys::DOWN)].into_iter().map(|(n,k)|(n.into(),k)).collect() });
    decide(sp,d);
    if sp.input.data_mode() {
        sp.play_command_poll=true;
        loop {
            sp.poll_input();
            if sp.s.byte(KEY)!=0 || !sp.input.key_pending() { break }
        }
        sp.play_command_poll=false;
    }
}
fn flight_rects(sp: &Space, d: &mut Decision) {
    crate::play_prompts::hotspot_rects(d,super::mouse::table(&sp.s,super::mouse::FLIGHT_HITS));
    let at=|d:&Decision,id:&str| d.options.iter().position(|o|o.id==id);
    if sp.s.word(CURRENT)>=0 {
        for (from,to) in [("cmd:jump","cmd:land"),("cmd:quit","cmd:launch")] {
            if let (Some(f),Some(t))=(at(d,from),at(d,to)) { for r in d.rects.iter_mut().filter(|r|r.0==f) { r.0=t; } }
        }
    }
    if sp.product {
        let p=super::render::PAUSE_ROW;
        if let Some(i)=at(d,"cmd:pause") { d.rects.push((i,[p.1 as i32,p.0 as i32,p.3 as i32,p.2 as i32])); }
        if let Some(i)=at(d,"cmd:stop") { d.rects.push((i,STOP_DIGIT)); }
    }
}
pub const STOP_DIGIT: [i32; 4] = [293, 157, 21, 20];
pub fn native_wait(sp: &mut Space, delay: i16) -> std::option::Option<i16> {
    without_input(sp,|sp|sp.wait_with_input(delay))
}
pub fn boarding_countdown(sp: &mut Space, step: i16, single: bool) -> i16 {
    without_input(sp,|sp|sp.update_encounter_countdown(step,single))
}
fn without_input<T>(sp: &mut Space, run: impl FnOnce(&mut Space)->T) -> T {
    let old=(sp.play_time_wait,sp.s.byte(KEY),sp.s.byte(PENDING_KEY));
    sp.play_time_wait=true;sp.s.set_byte(KEY,0);sp.s.set_byte(PENDING_KEY,0);
    let result=run(sp);
    sp.play_time_wait=old.0;sp.s.set_byte(KEY,old.1);sp.s.set_byte(PENDING_KEY,old.2);
    result
}
pub fn station(sp: &mut Space, routine: &str) { station_state(sp,routine,&[]); }
pub fn station_state(sp: &mut Space, routine: &str, live: &[u8]) {
    if routine != "4707" { sp.play_tactical = routine.starts_with("6743"); }
    if sp.observer.is_some() || !sp.input.wants_decision() || sp.s.byte(KEY)!=0 || sp.s.byte(PENDING_KEY)!=0 { return }
    let mut choices=vec![("back","Back",keys::ESC)];
    let directions=[("cmd:up","Up",keys::UP),("cmd:down","Down",keys::DOWN),("cmd:left","Left",keys::LEFT),("cmd:right","Right",keys::RIGHT)];
    let boarding=sp.s.byte(0x4626)!=0;
    if boarding { choices.push(("cmd:transfer","Transfer cargo",b'T' as u16)); }
    else if routine=="955C" { choices.extend(directions); choices.push(("cmd:fire","Fire",keys::ENTER)); }
    else if routine=="6743.select" { choices.extend(directions); choices.push(("accept","Select ship",keys::ENTER)); }
    else if routine!="6743.view" {
        choices.extend([("cmd:assign","Assign crew",b'A' as u16),("cmd:stations","Choose station",b'V' as u16)]);
        if sp.s.word(STATION)!=15 { choices.push(("cmd:tactical","Tactical map",b'X' as u16)); }
        match sp.s.word(STATION) {
            10 => choices.extend(directions),
            11 | 12 => choices.extend([("cmd:left","Turn gun left",keys::LEFT),("cmd:right","Turn gun right",keys::RIGHT),
                ("cmd:reload","Reload missiles",b'R' as u16),("cmd:target","Target",b'T' as u16)]),
            13 => choices.extend([("cmd:load","Load program",b'L' as u16),("cmd:run","Run program",b'R' as u16),("cmd:stop","Stop program",b'S' as u16)]),
            14 => choices.push(("cmd:repair","Repair component",b'R' as u16)),
            15 => choices.extend([("accept","Toggle communications",keys::ENTER),("cmd:up","Previous message",keys::UP),
                ("cmd:down","Next message",keys::DOWN),("cmd:identify","Identify ship",b'X' as u16)]),
            16 | 17 => choices.extend([("cmd:heal","Heal traveller",b'H' as u16),("cmd:rest","Rest traveller",b'R' as u16)]),
            _ => choices.extend(directions),
        }
    }
    let mut d = crate::play_prompts::keys_decision("s",routine,Kind::Menu,&choices);
    d.context.table = Some(format!("station:{}",sp.s.word(STATION)));
    if !boarding && !matches!(routine,"4707"|"955C"|"6743.select"|"6743.view") {
        for i in 0..5 { d.options.push(Option::new(format!("cmd:char/{i}"),format!("Traveller {}",i+1),vec![Event::Key(keys::F1+i)])); }
        crate::play_prompts::hotspot_rects(&mut d,super::mouse::table(&sp.s,super::mouse::CREW_HITS));
    }
    if let Some(hits)=sp.station_hits() { crate::play_prompts::hotspot_rects(&mut d,super::mouse::table(&sp.s,hits)); }
    if routine=="955C" { if let (Some(fire),Some(hits))=(d.options.iter().position(|o|o.id=="cmd:fire"),sp.station_hits()) {
        for h in super::mouse::table(&sp.s,hits).filter(|h|matches!(h.id,0x21|0x14)) { d.rects.push((fire,[h.left as i32,h.top as i32,h.width as i32,h.height as i32])); }
    } }
    if sp.product && routine.starts_with("6743") {
        d.rects.clear();
        if routine=="6743.view" { if let Some(back)=d.options.iter().position(|o|o.id=="back") { d.rects.push((back,[0,0,320,200])); } }
        else { for (k,r) in std::mem::take(&mut sp.play_rects).into_iter().enumerate() {
            d.rects.push((d.options.len(),r));
            d.options.push(Option::new(format!("ship/{k}"),format!("Ship {}",k+1),vec![Event::Mouse(mt_engine::io::MouseEvent{event:4,buttons:0,col:(r[0]+r[2]/2) as i16,row:(r[1]+r[3]/2) as i16})]));
        } }
    }
    if sp.product && !boarding && sp.s.word(STATION_ACTIVE)!=0 && !matches!(routine,"955C"|"6743.select"|"6743.view") {
        for (i,name) in STATIONS.iter().enumerate() {
            d.rects.push((d.options.len(),strip_cell(sp,i)));
            d.options.push(Option::new(format!("cmd:station/{i}"),*name,vec![Event::Key(0x160+i as u16)]));
        }
    }
    if !live.is_empty() { let mut h=Fnv::default(); h.field(live); d.state=h.state(); }
    decide(sp,d);
}
pub const STATIONS: [&str; 8] = ["Pilot", "Gunner 1", "Gunner 2", "Computer", "Engineering", "Communications", "Medical", "Crew"];
pub fn strip_cell(sp: &Space, i: usize) -> [i32; 4] {
    let at=0x355+8*i as u16; [sp.s.word(at+2) as i32,sp.s.word(at) as i32,sp.s.word(at+6) as i32,sp.s.word(at+4) as i32]
}
pub fn selector(sp: &mut Space, routine: &str, count: usize, selected: i16, hotkeys: std::option::Option<&[u8]>) {
    selector_state(sp,routine,count,selected,hotkeys,&[]);
}
pub fn selector_state(sp: &mut Space, routine: &str, count: usize, selected: i16, hotkeys: std::option::Option<&[u8]>, live: &[u8]) {
    if sp.observer.is_some() || !sp.input.wants_decision() { return }
    let mut d = Decision::new(Kind::Menu,"s",routine);
    d.context.table = Some(format!("station:{}",sp.s.word(STATION)));
    let selected = selected.clamp(0,count.saturating_sub(1) as i16);
    let rects: Vec<[i32;4]>=std::mem::take(&mut sp.play_rects).into_iter().take(count).collect();
    let played = sp.product && !sp.input.data_mode();
    for i in 0..count {
        let input = if i<10 { vec![Event::Key(keys::F1+i as u16)] } else if let (true,Some(r))=(played,rects.get(i)) {
            vec![Event::Mouse(mt_engine::io::MouseEvent{event:4,buttons:0,col:(r[0]+r[2]/2) as i16,row:(r[1]+r[3]/2) as i16})]
        } else {
            let mut events=vec![Event::Key(if i as i16 >= selected {keys::DOWN} else {keys::UP}); (i as i16).abs_diff(selected) as usize];
            events.push(Event::Key(keys::ENTER)); events
        };
        let label=sp.play_labels.get(i).cloned().or_else(|| hotkeys.and_then(|h|h.get(i)).map(|&b|format!("Choice {} ({})",i+1,b as char))).unwrap_or_else(||format!("Choice {}",i+1));
        d.options.push(Option::new(format!("menu:{}/{i}",routine.to_ascii_lowercase()),label,input));
    }
    d.default=Some(format!("menu:{}/{selected}",routine.to_ascii_lowercase()));
    d.rects=rects.into_iter().enumerate().collect();
    if let Some(r)=sp.play_back { d.rects.push((d.options.len(),r)); }
    d.options.push(Option::new("back","Back",vec![Event::Key(keys::ESC)]));
    if !live.is_empty() { let mut h=Fnv::default(); h.field(live); d.state=h.state(); }
    decide(sp,d);
}
pub fn confirm(sp: &mut Space) {
    let mut d=crate::play_prompts::keys_decision("s","4ED0",Kind::YesNo,&[("yes","Quit",b'Y' as u16),("no","Continue flight",b'N' as u16)]);
    let rows: Vec<[i32;4]>=super::mouse::table(&sp.s,super::mouse::FLIGHT_HITS).take(2).map(|h|[h.left as i32,h.top as i32,h.width as i32,h.height as i32]).collect();
    d.rects=rows.into_iter().enumerate().take(if sp.product {2} else {1}).collect();
    if sp.product { crate::play_prompts::prefer_no(&mut d); }
    decide(sp,d);
}
pub fn jump_confirm(sp: &mut Space, yes: [i32; 4], no: [i32; 4]) {
    let mut d=crate::play_prompts::keys_decision("s","jump.confirm",Kind::YesNo,&[("yes","Yes",b'Y' as u16),("no","No",b'N' as u16)]);
    d.rects=vec![(0,yes),(1,no)]; crate::play_prompts::prefer_no(&mut d);
    decide(sp,d);
}
pub fn notice(sp: &mut Space) {
    decide(sp,crate::play_prompts::keys_decision("s","notice",Kind::Ack,&[("continue","Continue",32)]));
}
pub const CLOSE_WORD: [i32; 4] = [275,60,29,8];
pub fn record(sp: &mut Space) {
    if sp.observer.is_some() || !sp.input.wants_decision() { return }
    let hits=super::host::record::HITS;
    let mut d = crate::play_prompts::keys_decision("s","12659",Kind::Menu,&[
        ("back","Close",keys::ESC),("cmd:unequip","Quit using",b'Q' as u16),("cmd:equip","Use object",b'U' as u16),
        ("cmd:pick","Pick object",b'P' as u16),("cmd:activate","View",keys::ENTER),("cmd:move","Move item",b'M' as u16),
        ("cmd:next-action","Next action",keys::DOWN),("cmd:previous-action","Previous action",keys::UP),
        ("cmd:next-skill","Next skill",keys::PGDN),("cmd:first-skill","First skill",keys::PGUP),
    ]);
    for i in 0..8 { d.options.push(Option::new(format!("action:{i}"),format!("Action {}",i+1),vec![Event::Key(b'1' as u16+i)])); }
    for i in 0..5 { d.options.push(Option::new(format!("cmd:char/{i}"),format!("Traveller {}",i+1),vec![Event::Key(keys::F1+i)])); }
    for i in 0..4 { d.options.push(Option::new(format!("equipment:{i}"),format!("Equipment {}",i+1),vec![Event::Key(0x141+i)])); }
    for h in super::mouse::table(&sp.s,hits).filter(|h|(0x96..=0xA8).contains(&h.id)) {
        d.options.push(Option::new(format!("inventory:{}",h.id-0x96),format!("Inventory cell {}",h.id-0x96),
            vec![Event::Mouse(mt_engine::io::MouseEvent{event:4,buttons:0,col:h.left+1,row:h.top+1})]));
    }
    d.context.table = Some(format!("held:{}",sp.s.word(super::host::record::SELECTED)));
    crate::play_prompts::hotspot_rects(&mut d,super::mouse::table(&sp.s,hits));
    let at = |id: &str| d.options.iter().position(|o| o.id == id);
    if let (Some(view), Some(h)) = (at("cmd:activate"), super::mouse::table(&sp.s,hits).find(|h| h.id == 47)) { d.rects.push((view,[h.left as i32,h.top as i32,h.width as i32,h.height as i32])); }
    if let Some(close) = at("back") { d.rects.push((close,CLOSE_WORD)); }
    decide(sp,d);
}
pub fn record_detail(sp: &mut Space) {
    decide(sp,crate::play_prompts::keys_decision("s","1247D",Kind::Ack,&[("continue","Continue",32),("back","Back",keys::ESC)]));
}
pub fn death(sp: &mut Space) {
    decide(sp,crate::play_prompts::keys_decision("s","B287",Kind::Ack,&[("continue","Continue",32)]));
}
#[cfg(test)]
mod t {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir, Input, MouseEvent}, sim::tick::{Clock, Counter}};
    use std::{cell::RefCell, collections::VecDeque, rc::Rc};
    struct Seen(Rc<RefCell<Vec<Decision>>>, VecDeque<Event>);
    impl Input for Seen {
        fn wants_decision(&self) -> bool { true }
        fn decide(&mut self, d: &Decision) { self.0.borrow_mut().push(d.clone()); }
        fn next_key(&mut self) -> u16 { 0 }
        fn poll(&mut self, clock: &mut dyn Clock) -> Event { clock.wait_tick(); self.1.pop_front().unwrap_or(Event::Idle) }
    }
    fn station(events: impl IntoIterator<Item = Event>) -> std::option::Option<(Space, Rc<RefCell<Vec<Decision>>>)> {
        let dir = oracle_env::gamefiles()?;
        let seen: Rc<RefCell<Vec<Decision>>> = Rc::default();
        let mut sp = Space::new(&std::fs::read(dir.join("S.DAT")).unwrap(), vec![0; PARTY_SIZE], vec![], Box::new(Seen(seen.clone(), events.into_iter().collect())), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        sp.product = true; sp.r.initialize(&mut sp.s); sp.s.set_word(STATION, 10); sp.s.set_word(STATION_ACTIVE, 1);
        Some((sp, seen))
    }
    #[test]
    fn station_wait_publishes_the_strip_and_its_hit_table() {
        let Some((mut sp, seen)) = station([]) else { return };
        super::station(&mut sp, "pilot");
        let d = seen.borrow()[0].clone();
        let rect = |id: &str| d.rects.iter().find(|(k, _)| d.options[*k].id == id).map(|r| r.1);
        let cells: Vec<[i32; 4]> = (0..8).map(|i| rect(&format!("cmd:station/{i}")).unwrap_or_else(|| panic!("station {i}: {:?}", d.rects))).collect();
        assert!(cells.iter().all(|r| r[2] > 0 && r[3] > 0 && r[0] >= 0 && r[0] + r[2] <= 320 && r[1] >= 0 && r[1] + r[3] <= 200), "{cells:?}");
        assert!(cells.windows(2).all(|w| w[0][0] + w[0][2] <= w[1][0] && w[0][1] == w[1][1]), "left to right on one row, never overlapping: {cells:?}");
        assert_eq!(d.options.iter().filter(|o| o.id.starts_with("cmd:station/")).map(|o| o.input.clone()).collect::<Vec<_>>(), (0..8).map(|i| vec![Event::Key(0x160 + i)]).collect::<Vec<_>>());
        for id in ["cmd:assign", "cmd:stations", "cmd:tactical", "cmd:up", "cmd:down", "cmd:left", "cmd:right"] { assert!(rect(id).is_some(), "{id} has no rectangle: {:?}", d.rects); }
        assert_eq!((0..5).map(|i| rect(&format!("cmd:char/{i}"))).collect::<Vec<_>>(), (0..5).map(|i| Some([6 + 40 * i, 0, 40, 50])).collect::<Vec<_>>());
        sp.product = false; seen.borrow_mut().clear(); super::station(&mut sp, "pilot");
        assert!(!seen.borrow()[0].options.iter().any(|o| o.id.starts_with("cmd:station/")), "the strip choices are the product's");
    }
    #[test]
    fn flight_and_quit_publish_their_rectangles() {
        let Some((mut sp, seen)) = station([]) else { return };
        let rect = |d: &Decision, id: &str| d.rects.iter().find(|(k, _)| d.options[*k].id == id).map(|r| r.1);
        sp.s.set_word(CURRENT, -1); super::flight(&mut sp);
        let d = seen.borrow().last().unwrap().clone(); assert_eq!(d.context.routine, "4FCB");
        for (id, r) in [("cmd:stations", [253, 67, 61, 10]), ("cmd:communications", [253, 79, 61, 10]), ("cmd:jump", [253, 91, 61, 10]), ("cmd:quit", [253, 103, 61, 10]),
            ("cmd:pause", [253, 115, 60, 11]), ("cmd:thrust", [293, 139, 21, 17]), ("cmd:reverse", [293, 178, 21, 17]), ("cmd:left", [252, 138, 22, 15]), ("cmd:right", [268, 155, 23, 14]), ("cmd:stop", STOP_DIGIT)] {
            assert_eq!(rect(&d, id), Some(r), "{id}");
        }
        assert_eq!((rect(&d, "cmd:land"), rect(&d, "cmd:launch")), (None, None), "free flight: the rows are Jump / Quit");
        sp.s.set_word(CURRENT, 0); super::flight(&mut sp);
        let d = seen.borrow().last().unwrap().clone();
        assert_eq!((rect(&d, "cmd:land"), rect(&d, "cmd:launch"), rect(&d, "cmd:jump"), rect(&d, "cmd:quit")), (Some([253, 91, 61, 10]), Some([253, 103, 61, 10]), None, None), "at an object a hit on the two rows lands / takes off");
        sp.product = false; super::flight(&mut sp);
        let d = seen.borrow().last().unwrap().clone();
        assert_eq!(d.rects.len(), 8, "faithful: the 1990 table alone: {:?}", d.rects);
        super::confirm(&mut sp); assert_eq!(seen.borrow().last().unwrap().rects, [(0, [253, 67, 61, 10])], "1990: only Yes has a region");
        sp.product = true; super::confirm(&mut sp);
        let d = seen.borrow().last().unwrap().clone();
        assert_eq!((rect(&d, "yes"), rect(&d, "no")), (Some([253, 67, 61, 10]), Some([253, 79, 61, 10])));
    }
    #[test]
    fn the_selector_publishes_its_rows() {
        let Some((mut sp, seen)) = station([Event::Key(keys::ESC)]) else { return };
        let cells: Vec<[i16; 4]> = (0..12).map(|i| [100 + 18 * (i / 6), 20 + 18 * (i % 6), 16, 16]).collect();
        assert_eq!(sp.select_rect(&cells, 0, 6, 8, None), -1);
        let d = seen.borrow().iter().find(|d| d.context.routine == "D7B4").cloned().expect("published");
        assert_eq!(d.rects.len(), 12);
        assert_eq!((d.rects[0], d.rects[11]), ((0, [20, 100, 16, 16]), (11, [110, 118, 16, 16])), "left, top, width, height");
        assert_eq!(d.options[3].input, [Event::Key(keys::F1 + 3)]);
        assert_eq!(d.options[11].input, [Event::Mouse(MouseEvent { event: 4, buttons: 0, col: 118, row: 126 })], "the twelfth row: a release in its cell");
        let Some((mut sp, _)) = station(d.options[11].input.clone()) else { return };
        assert_eq!(sp.select_rect(&cells, 0, 6, 8, None), 11, "and that release picks the twelfth row");
    }
    #[test]
    fn a_click_outside_a_station_picker_closes_it_and_reaches_the_station() {
        use crate::space::{host::{StationHost, TOP_FRAME}, stations::Host};
        let click = Event::Mouse(MouseEvent { event: 4, buttons: 0, col: 230, row: 12 });
        let pick = [0x355, DS as i32, 8, 0, 12, 8, 0x19BA, DS as i32, 1];
        let Some((mut sp, _)) = station([click]) else { return };
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        assert_eq!(h.int_call("station_menu", &pick), Ok(-1), "closed by the click");
        assert_eq!(sp.s.byte(KEY), 0x6E, "the click is still there");
        assert_eq!(sp.translate_station_key(), 27, "and the station reads it: In-sys travel");
        let Some((mut sp, _)) = station([click, Event::Idle, Event::Key(keys::F1 + 2)]) else { return };
        sp.product = false;
        let mut h = StationHost::new(&mut sp, vec![0; 65536], TOP_FRAME);
        assert_eq!(h.int_call("station_menu", &pick), Ok(2), "1990: the click is dropped, the picker waits for its key");
        assert_eq!(sp.s.byte(KEY), 0);
    }
    #[test]
    fn a_missed_click_publishes_the_station_wait_again() {
        let miss = Event::Mouse(MouseEvent { event: 4, buttons: 0, col: 310, row: 197 });
        let Some((mut sp, seen)) = station([miss, Event::Idle, Event::Key(keys::ESC)]) else { return };
        assert_eq!(sp.translate_station_key(), 27);
        assert_eq!(seen.borrow().iter().filter(|d| d.context.routine == "4707").count(), 2, "at entry, and again after the miss");
    }
}
