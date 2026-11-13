use super::{Space, jump::{self, Call, Host, Party, State, Tables}, render::Render, state::*, system};

pub const HERE: u8 = 2;
pub const DIM: u8 = 8;
pub const CANCEL: [i16; 4] = [157, 190, 11, 52];
pub const JUMP_FUEL: u8 = 87;
pub const BAND: &[u8] = b"The flashing system is where you will jump.";
pub const HOW: [&[u8]; 2] = [b"To jump: click a system,", b"or use arrows and Enter."];
pub const LEGEND: [(u8, &[u8]); 3] = [(HERE, b"you are here"), (9, b"in reach"), (DIM, b"too far")];
pub struct JumpHost<'a> { pub sp: &'a mut Space, y: i16, x: i16, map: bool, film: bool, cancelled: bool }
impl<'a> JumpHost<'a> { pub fn new(sp: &'a mut Space) -> Self { JumpHost { sp, y: 0, x: 0, map: false, film: false, cancelled: false } } }

pub fn proper(name: &[u8]) -> String { let mut n = name.to_vec(); if let Some(c) = n.first_mut() { c.make_ascii_uppercase(); } String::from_utf8_lossy(&n).into_owned() }
pub fn why_not(error: u16, fuel: u8, drive: u8) -> String {
    match error {
        0x1334 => format!("Not enough fuel to jump: you have {fuel} tons, Jump-{} needs {JUMP_FUEL}. Refuel at a starport or scoop at a gas giant.", drive + 1),
        0x1346 => "This ship has no jump drive: it cannot jump. A shipyard can fit one.".into(),
        0x1354 => "No Jump program is running. Open Stations, Computer, and load and run Jump; then jump again.".into(),
        _ => "The Navigation program is not running. Open Stations, Computer, and load and run Navigation; then jump again.".into(),
    }
}

impl Space {
    pub fn jump_screen(&mut self) -> Result<(), String> {
        let tables = Tables::from_ds(self.s.image())?;
        let mut party: Party = self.s.party.as_slice().try_into().map_err(|_| "jump: party block length")?;
        let mut state = self.jump_state();
        let before = (party, state.clone());
        let mut host = JumpHost::new(self);
        let result = jump::jump_screen(&mut host, &mut state, &mut party, &tables);
        let cancelled = host.cancelled;
        if result.is_err() && self.product { (party, state) = before; }
        if self.played() && party[6] == before.0[6] { party[0xFC] = before.0[0xFC]; }
        self.set_jump_state(&state);
        self.s.party.copy_from_slice(&party);
        if cancelled {
            self.r.back(&mut self.s); self.r.layout(&self.s, 21, 0, 0); self.hud(-1); self.hide_cursor();
            self.s.set_word(VIEW, -1); self.s.set_word(QUADRANT, -1);
            if self.draw_system_view(true) != 0 { self.r.thrust(&mut self.s, state.thrust, 7); }
            let state = self.jump_state();
            self.r.layout(&self.s, state.heading, 100, 128);
            if state.moving != 0 { self.r.layout(&self.s, state.heading.wrapping_add(120), 100, 128); }
            self.r.update_minimap_quadrant(&mut self.s); self.flip_with_cursor(); self.s.set_word(VIEW, 0);
            self.r.thrust(&mut self.s, state.thrust, 9); self.copy_display_rectangle(0, 0, 200, 40);
            return Ok(());
        }
        result
    }
    pub(crate) fn played(&self) -> bool { self.product && self.observer.is_none() && !self.input.data_mode() }
    pub fn text_width(&self, text: &[u8]) -> i16 { text.iter().map(|&c| self.r.font.widths[c as usize] as i16).sum() }
    pub fn wrap(&self, text: &str, w: i16) -> Vec<Vec<u8>> {
        let words: Vec<&str> = text.split(' ').collect();
        let width = |ws: &[&str]| self.text_width(ws.join(" ").as_bytes());
        let (mut lines, mut at) = (Vec::new(), 0);
        while at < words.len() {
            let mut end = at + 1;
            while end < words.len() && width(&words[at..=end]) <= w { end += 1; }
            if end < words.len() && !words[end - 1].ends_with(['.', ':', ';', ',']) {
                if let Some(cut) = (at + 1..end).rev().find(|&c| words[c - 1].ends_with(['.', ':', ';', ','])) { if width(&words[at..cut]) >= w / 2 { end = cut; } }
            }
            lines.push(words[at..end].join(" ").into_bytes());
            at = end;
        }
        lines
    }
    fn refuse_jump(&mut self, error: u16, menu: &Option<(Tables, jump::Menu)>) {
        let mut lines = self.wrap(&why_not(error, self.s.pbyte(0xFC), menu.as_ref().map_or(0, |m| m.1.drive)), 216);
        lines.push(b"Press a key or click to go back.".to_vec());
        let here = menu.as_ref().and_then(|(t, _)| t.labels.get(*t.row_for_system.get(self.s.pbyte(6) as usize)? as usize).map(|l| l.y)).unwrap_or(0);
        self.r.front(&mut self.s);
        self.panel(&lines, &[], 0, if here < 24 + lines.len() as i16 * 10 { 100 } else { 14 });
        self.wait_ack(); self.r.back(&mut self.s);
    }
    pub fn panel(&mut self, lines: &[Vec<u8>], buttons: &[&[u8]], preselected: usize, top: i16) -> Vec<[i32; 4]> {
        let pen = self.r.pen;
        let w = lines.iter().map(|l| self.text_width(l)).max().unwrap_or(0).max(buttons.len() as i16 * 52) + 16;
        let h = lines.len() as i16 * 10 + if buttons.is_empty() { 7 } else { 23 };
        let x = 8 + (240 - w) / 2;
        self.r.fill(&self.s, top, x, h, w, 15); self.r.fill(&self.s, top + 1, x + 1, h - 2, w - 2, 0);
        for (i, l) in lines.iter().enumerate() { let lw = self.text_width(l); self.r.text(&self.s, l, top + 4 + 10 * i as i16, x + (w - lw) / 2, 15); }
        let (by, n) = (top + 5 + 10 * lines.len() as i16, buttons.len() as i16);
        let mut rects = Vec::new();
        for (i, b) in buttons.iter().enumerate() {
            let bx = x + (w - n * 52 + 8) / 2 + 52 * i as i16;
            if i == preselected { self.r.fill(&self.s, by - 1, bx - 1, 15, 46, 15); }
            self.r.fill(&self.s, by, bx, 13, 44, 7);
            let bw = self.text_width(b); self.r.text(&self.s, b, by + 3, bx + (44 - bw) / 2, 0);
            rects.push([bx as i32, by as i32, 44, 13]);
        }
        self.r.pen = pen;
        rects
    }
    pub(crate) fn wait_ack(&mut self) {
        self.drop_stale_clicks(); super::play::notice(self);
        self.s.set_byte(KEY, 0);
        while self.s.byte(KEY) == 0 { self.update_cursor(); }
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
    }
    fn confirm_jump(&mut self, from: &str, to: &str, clear: [i16; 2]) -> bool {
        let fuel = self.s.pbyte(0xFC);
        let mut lines = vec![format!("Jump from {from} to {to}?").into_bytes(), format!("Uses {} tons of fuel, takes 7 days.", fuel - fuel / 2).into_bytes()];
        if fuel <= 102 { lines.push(b"Fuel is low: a misjump is possible.".to_vec()); }
        let h = lines.len() as i16 * 10 + 23;
        let top = [14, 64, 122].into_iter().find(|&t| clear.iter().all(|&y| y + 8 <= t || y >= t + h)).unwrap_or(14);
        self.drop_stale_clicks();
        self.r.front(&mut self.s);
        let page = Render::page(&self.s);
        self.r.fb.copy(page, top as i32, 0, h as i32, 40, 7, 0, 0);
        let b = self.panel(&lines, &[b"Yes", b"No"], 1, top);
        super::play::jump_confirm(self, b[0], b[1]);
        let yes = loop {
            self.s.set_byte(KEY, 0);
            while self.s.byte(KEY) == 0 { self.update_cursor(); }
            match self.s.byte(KEY) {
                0x15 => break true,
                0x31 | 0x1C | 1 => break false,
                0x6E => {
                    let at = (self.s.word(super::mouse::CALLBACK + 6) as i32, self.s.word(super::mouse::CALLBACK + 4) as i32);
                    if let Some(i) = b.iter().position(|r| (r[0]..r[0] + r[2]).contains(&at.0) && (r[1]..r[1] + r[3]).contains(&at.1)) { break i == 0 }
                }
                _ => {}
            }
        };
        self.s.set_byte(KEY, 0); self.s.set_byte(PENDING_KEY, 0); self.s.set_word(super::mouse::CALLBACK, 0);
        self.r.fb.copy(7, 0, 0, h as i32, 40, page, top as i32, 0);
        self.r.back(&mut self.s);
        yes
    }
    fn map_words(&mut self, labels: &[jump::Label], error: Option<u16>, tables: &Tables) {
        let pen = self.r.pen;
        for (i, l) in labels.iter().enumerate().take(8) {
            let r = tables.rects[i];
            let colour = match l.flags { 4 => HERE, 6 if error.is_none() => continue, _ => DIM };
            self.r.fill(&self.s, r[0], r[1], r[2], r[3], colour);
        }
        if error.is_none() { let w = self.text_width(BAND); self.r.fill(&self.s, 11, 5, 9, w + 4, 0); self.r.text(&self.s, BAND, 12, 7, 15); }
        for (i, (colour, words)) in LEGEND.iter().enumerate() {
            let (y, w) = (140 + 10 * i as i16, self.text_width(words));
            self.r.fill(&self.s, y - 2, 5, 10, w + 15, 0); self.r.fill(&self.s, y, 7, 6, 7, *colour); self.r.text(&self.s, words, y, 17, 15);
        }
        let c = CANCEL; self.r.fill(&self.s, c[0], c[1], c[2], c[3], 7);
        let w = self.text_width(b"Cancel"); self.r.text(&self.s, b"Cancel", c[0] + 2, c[1] + (c[3] - w) / 2, 0);
        let lines: [&[u8]; 2] = match error {
            Some(e) => [b"No jump possible:", match e { 0x1334 => b"not enough fuel.", 0x1346 => b"no jump drive.", 0x1354 => b"no Jump program running.", _ => b"no Navigation program." }],
            None if (JUMP_FUEL..=102).contains(&self.s.pbyte(0xFC)) => [b"Fuel low: misjump possible.", b"Click a system, or Enter."],
            None => HOW,
        };
        for (l, y) in lines.iter().zip([174, 185]) { self.r.text(&self.s, l, y, 137, 0); }
        self.r.pen = pen;
    }
    fn jump_state(&self) -> State {
        State { video_mode: self.s.word(VIDEO_MODE), world_time: self.s.word(WORLD_TIME), system_encounter: self.s.word(SHIP_COUNT),
            view_heading: self.s.word(VIEW), minimap_quadrant: self.s.word(QUADRANT), thrust: self.s.word(THRUST), heading: self.s.word(HEADING), moving: self.s.word(MOVING) }
    }
    fn set_jump_state(&mut self, st: &State) {
        for (at, v) in [(VIDEO_MODE, st.video_mode), (WORLD_TIME, st.world_time), (SHIP_COUNT, st.system_encounter), (VIEW, st.view_heading),
                        (QUADRANT, st.minimap_quadrant), (THRUST, st.thrust), (HEADING, st.heading), (MOVING, st.moving)] { self.s.set_word(at, v); }
    }
    fn page_entry(&self, at: u16) -> usize { ((self.s.word(at + 2) as u16).wrapping_sub(0xA000) as usize / 512) % 8 }
    pub fn copy_display_rectangle(&mut self, y: i16, x: i16, h: i16, w: i16) {
        let a = self.s.word(ACTIVE_PAGE) as u16 & 1;
        let (src, dst) = (self.page_entry(0x49BE + (a ^ 1) * 4), self.page_entry(0x49BE + a * 4));
        self.copy(src, y, x, h, w, dst, y, x);
    }
    pub fn restore_display_rectangle(&mut self, y: i16, x: i16, h: i16, w: i16) {
        let a = self.s.word(ACTIVE_PAGE) as u16 & 1;
        let (src, dst) = (self.page_entry(0x49BE + a * 4), self.page_entry(0x49BE + (a ^ 1) * 4));
        self.copy(src, y, x, h, w, dst, y, x);
    }
}

impl Host for JumpHost<'_> {
    fn wait_iteration(&mut self) {
        if self.sp.input.data_mode() { self.sp.clock.wait_tick(); }
        else if self.sp.product { self.sp.clock.busy_wait(); }
    }
    fn call(&mut self, call: Call, state: &mut State, party: &mut Party) -> Result<i32, String> {
        self.sp.s.party.copy_from_slice(party);
        self.sp.set_jump_state(state);
        let menu = if self.sp.product && matches!(call, Call::Labels(_) | Call::Select { .. }) { Tables::from_ds(self.sp.s.image()).ok().and_then(|t| jump::menu(party, &t).ok().map(|m| (t, m))) } else { None };
        let result = if call == Call::Animate {
            self.film = true; let film = jump::animate_jump_transition(self, state, party); self.film = false; film?;
            self.sp.s.party.copy_from_slice(party);
            self.sp.set_jump_state(state);
            0
        } else {
            let sp = &mut *self.sp;
            match call {
                Call::BackBuffer => { sp.r.back(&mut sp.s); 0 }
                Call::CursorMode(m) => { sp.s.set_word(0x4868, m); 0 }
                Call::Viewport(y, x, h, w) => { for (at, v) in [(0x49D2, y), (0x49BA, x), (0x49C6, h), (0x49BC, w)] { sp.s.set_word(at, v); } 0 }
                Call::Layout(i, y, x) => { sp.r.layout(&sp.s, i, y, x); 0 }
                Call::Labels(labels) => {
                    if let Some((t, m)) = &menu { sp.map_words(&labels, m.error, t); self.map = true; }
                    for l in labels.iter() {
                        if l.flags == 255 { break; }
                        if !l.text.is_empty() { sp.r.text(&sp.s, &l.text, l.y, l.x, l.flags); }
                    }
                    0
                }
                Call::Color(c) => { sp.s.set_byte(0x4994, c as u8); 0 }
                Call::TextPosition(y, x) => { self.y = y; self.x = x; 0 }
                Call::Text(_) if self.map && (self.y >= 174 || self.x != 4) => 0,
                Call::Text(v) if self.map => {
                    let line = format!("Jump map - you are at {}", proper(&v));
                    let c = sp.s.byte(0x4994); self.x = sp.r.text(&sp.s, line.as_bytes(), self.y, self.x, c); 0
                }
                Call::Text(v) => { let c = sp.s.byte(0x4994); self.x = sp.r.text(&sp.s, &v, self.y, self.x, c); 0 }
                Call::Fill(y, x, h, w, c) => { sp.r.fill(&sp.s, y, x, h, w, c as u8); 0 }
                Call::Flip => {
                    sp.flip_with_cursor();
                    if self.film && sp.played() { sp.poll_input(); sp.s.set_byte(KEY, 0); sp.s.set_byte(PENDING_KEY, 0); sp.s.set_word(super::mouse::CALLBACK, 0); }
                    0
                }
                Call::Select { rects, initial, highlight, background, keys } => {
                    self.map = false;
                    if sp.input.wants_decision() {
                        let tables = Tables::from_ds(sp.s.image())?;
                        sp.play_labels = rects.iter().map(|rect| tables.rects.iter().position(|r| r == rect)
                            .map(|row| String::from_utf8_lossy(&tables.names[tables.system_for_row[row] as usize]).into_owned())
                            .unwrap_or_else(|| "Destination".into())).collect();
                    }
                    let error = menu.as_ref().and_then(|m| m.1.error);
                    let choice = if let (Some(e), true) = (error, sp.played()) {
                        sp.refuse_jump(e, &menu);
                        0
                    } else {
                        if sp.product { let c = CANCEL; sp.play_back = Some([c[1] as i32, c[0] as i32, c[3] as i32, c[2] as i32]); }
                        let mut at = initial;
                        loop {
                            let Some(c) = sp.select_rect_or_handover(&rects, at, highlight, background, Some(&keys), error.is_some() && sp.product && sp.observer.is_none()) else { sp.refuse_jump(error.unwrap(), &menu); break 0 };
                            let Some((t, m)) = menu.as_ref().filter(|_| c >= 0 && sp.played()) else { break c };
                            let (Some(&row), here) = (m.rows.get(c as usize), sp.s.pbyte(6) as usize) else { break c };
                            let to = t.system_for_row[row as usize];
                            let clear = [t.labels[row as usize].y, t.labels[t.row_for_system[here] as usize].y];
                            if sp.confirm_jump(&proper(&t.names[here]), &proper(&t.names[to as usize]), clear) { sp.jump_asked = Some(to); break c }
                            at = c;
                        }
                    };
                    sp.play_labels.clear(); sp.play_back = None;
                    if choice == -1 && sp.played() { self.cancelled = true; return Err("jump map cancelled".into()); }
                    choice as i32
                },
                Call::Rand => sp.rand() as i32,
                Call::LoadSystem(d) => { system::load(&mut sp.s, sp.files.as_mut(), d)?; 0 }
                Call::InitEncounters => { sp.initialize_system_encounters(); 0 }
                Call::FlightHud(m) => { sp.hud(m); 0 }
                Call::HideCursor => { sp.hide_cursor(); 0 }
                Call::SystemView(_) => sp.draw_system_view(true) as i32,
                Call::Thrust(t, c) => { sp.r.thrust(&mut sp.s, t, c as u8); 0 }
                Call::UpdateMinimap => { sp.r.update_minimap_quadrant(&mut sp.s); 0 }
                Call::Copy(y, x, h, w) => { sp.copy_display_rectangle(y, x, h, w); 0 }
                Call::Restore(y, x, h, w) => { sp.restore_display_rectangle(y, x, h, w); 0 }
                Call::EncounterIcons(_) => { sp.encounter_icons(false); 0 }
                Call::Ticks => sp.ticks() as i32,
                Call::Palette(i, v) => { sp.r.fb.set_reg(i as usize, v as u8); 0 }
                Call::Animate => 0,
            }
        };
        *state = self.sp.jump_state();
        party.copy_from_slice(&self.sp.s.party);
        Ok(result)
    }
}

#[cfg(test)]
mod play_clock_tests {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir,Input,Scripted},play::Play,sim::tick::{Clock,Counter}};
    use std::collections::VecDeque;

    struct Bios {reads:VecDeque<u32>,last:u32}
    impl Clock for Bios {
        fn now(&mut self)->u32 {if let Some(t)=self.reads.pop_front(){self.last=t;}self.last}
        fn ticks(&mut self)->u32 {self.last}
        fn set(&mut self,n:u32) {self.last=n;}
        fn wait_tick(&mut self) {panic!("native jump used the data clock hook")}
    }
    struct Frames<'a> {inner:JumpHost<'a>,at:Vec<u32>}
    impl Host for Frames<'_> {
        fn call(&mut self,c:Call,s:&mut State,p:&mut Party)->Result<i32,String> {
            let flip=c==Call::Flip;let r=self.inner.call(c,s,p)?;
            if flip {self.at.push(self.inner.sp.clock.ticks());}Ok(r)
        }
        fn wait_iteration(&mut self) {self.inner.wait_iteration();}
    }
    #[test]
    fn the_map_words_fit_and_the_refusal_has_the_numbers() {
        assert_eq!(why_not(0x1334, 61, 0), "Not enough fuel to jump: you have 61 tons, Jump-1 needs 87. Refuel at a starport or scoop at a gas giant.");
        assert_eq!(proper(b"louzy"), "Louzy");
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        let sp=Space::new(&bytes,crate::ground::party::initial(),vec![0;0x2000],Box::new(Scripted::new([])),Box::new(Counter::at(1)),Box::new(Dir(dir))).unwrap();
        let w=|t:&[u8]| sp.text_width(t);
        assert!(w(BAND)<=236,"band {}",w(BAND));
        assert!(w(b"Jump map - you are at Boughene")<=236,"title");
        for (i,(_,l)) in LEGEND.iter().enumerate() { assert!(17+w(l)<=[108,108,78][i],"legend {:?} {}",String::from_utf8_lossy(l),w(l)); }
        for l in [HOW[0],HOW[1],b"Fuel low: misjump possible.",b"Click a system, or Enter.",b"No jump possible:",b"no Jump program running.",b"no Navigation program."] { assert!(w(l)<=150,"box {:?} {}",String::from_utf8_lossy(l),w(l)); }
        assert!(w(b"Cancel")<=CANCEL[3]-4);
        for e in [0x1334,0x1346,0x1354,0x136C] { let lines=sp.wrap(&why_not(e,61,1),216); assert!(lines.len()<=5 && lines.iter().all(|l|w(l)<=216),"{lines:?}"); }
    }
    #[test]
    fn product_jump_progresses_without_input_polls() {
        struct Offscreen { clock: Counter, reads: usize }
        impl Clock for Offscreen {
            fn now(&mut self)->u32 { self.reads+=1; assert!(self.reads<1000,"jump stalled without an input boundary"); self.clock.now() }
            fn ticks(&mut self)->u32 { self.clock.ticks() }
            fn set(&mut self,n:u32) { self.clock.set(n); }
            fn wait_tick(&mut self) { self.clock.wait_tick(); }
            fn busy_wait(&mut self) { self.clock.wait_tick(); }
        }
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        let mut sp=Space::new(&bytes,crate::ground::party::initial(),vec![0;0x2000],Box::new(Scripted::new([])),
            Box::new(Offscreen {clock:Counter::at(123),reads:0}),Box::new(Dir(dir))).unwrap();
        sp.product=true;sp.r.initialize(&mut sp.s);system::load(&mut sp.s,sp.files.as_mut(),2).unwrap();
        sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);
        let mut state=sp.jump_state();let mut party=sp.s.party.as_slice().try_into().unwrap();
        jump::animate_jump_transition(&mut JumpHost::new(&mut sp),&mut state,&mut party).unwrap();
        assert_eq!(sp.clock.ticks(),263);
    }
    #[test]
    fn a_refusal_resumed_on_the_map_shows_its_box_and_wraps_at_clauses() {
        use std::{cell::Cell,rc::Rc};
        struct Handoff(Rc<Cell<bool>>);
        impl Input for Handoff { fn next_key(&mut self)->u16 {panic!("keys arrive through Hooks")} fn data_mode(&self)->bool {!self.0.get()} }
        struct Live(u32,Rc<Cell<bool>>);
        impl crate::space::tick::Hooks for Live { fn update_cursor(&mut self,sp:&mut Space) {
            self.0+=1; assert!(self.0<20_000,"the selector, not the refusal box, holds the resumed wait");
            if self.0==40 { self.1.set(true); }
            if self.0>40 && self.0%40==0 && sp.s.byte(KEY)==0 { sp.keyboard_isr(0x39); sp.keyboard_isr(0xB9); }
        } }
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        let live=Rc::new(Cell::new(false));
        let mut party=crate::ground::party::initial();
        party[6]=2;party[7]=255;party[0xFC]=120;party[0xC1]=0;party[0xF84]=0;party[0xF85]=0;
        for (i,p) in [163u8,0,164,1,5].iter().enumerate() {party[0xAC+i]=*p;}
        let mut sp=Space::new(&bytes,party,vec![0;0x2000],Box::new(Handoff(live.clone())),Box::new(Counter::at(123)),Box::new(Dir(dir))).unwrap();
        sp.product=true;sp.r.initialize(&mut sp.s);system::load(&mut sp.s,sp.files.as_mut(),2).unwrap();
        sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);
        sp.hooks=Some(Box::new(Live(0,live)));
        assert_eq!(sp.jump_screen(),Ok(()));
        assert_eq!((sp.s.pbyte(6),sp.s.pbyte(0xFC),sp.s.pword(0xF84)),(2,120,0));
        let text=|e| sp.wrap(&why_not(e,61,0),216).iter().map(|l|String::from_utf8_lossy(l).into_owned()).collect::<Vec<_>>();
        assert_eq!(text(0x1346),["This ship has no jump drive:","it cannot jump. A shipyard can fit one."]);
    }
    #[test]
    fn failed_jump_is_no_jump_in_product() {
        struct F1(u32);
        impl crate::space::tick::Hooks for F1 { fn update_cursor(&mut self,sp:&mut Space) { self.0+=1; assert!(self.0<100_000,"poll budget"); if self.0%40==0 && sp.s.byte(KEY)==0 { let k=if self.0%80==0 {0x15} else {0x3B}; sp.keyboard_isr(k); sp.keyboard_isr(k|0x80); } } }
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        for product in [true,false] {
            let mut party=crate::ground::party::initial();
            party[6]=2;party[7]=255;party[0xFC]=100;party[0xC1]=1;party[0xC0]=2;party[0xF84]=0;party[0xF85]=0;
            for (i,p) in [163u8,0,164,1,5].iter().enumerate() {party[0xAC+i]=*p;}
            let mut files=mt_engine::io::Overlay::new(Some(&dir));files.hide("6.STP");
            let mut sp=Space::new(&bytes,party,vec![0;0x2000],Box::new(Scripted::new([])),Box::new(Counter::at(123)),Box::new(files)).unwrap();
            sp.product=product;sp.r.initialize(&mut sp.s);system::load(&mut sp.s,sp.files.as_mut(),2).unwrap();
            sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);sp.s.set_dword(RNG,965963027);sp.s.set_word(WORLD_TIME,0);
            sp.hooks=Some(Box::new(F1(0)));
            assert_eq!(sp.jump_screen(),Err("Unable to open system file 6.STP".into()));
            let now=(sp.s.pbyte(6),sp.s.pbyte(7),sp.s.pbyte(0xFC),sp.s.pword(0xF84),sp.s.word(WORLD_TIME));
            assert_eq!(now,if product {(2,255,100,0,0)} else {(6,255,100,7,7)},"product {product}");
        }
    }
    #[test]
    fn leaving_the_jump_map_at_yres_costs_no_fuel_in_product() {
        struct Esc(u32);
        impl crate::space::tick::Hooks for Esc { fn update_cursor(&mut self,sp:&mut Space) { self.0+=1; assert!(self.0<100_000,"poll budget"); if self.0%40==0 && sp.s.byte(KEY)==0 { sp.keyboard_isr(1); sp.keyboard_isr(0x81); } } }
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        for product in [true,false] {
            let mut party=crate::ground::party::initial();
            party[6]=7;party[7]=255;party[0xFC]=120;
            party[0xC1]=1;party[0xC0]=2;party[0xF84]=0;party[0xF85]=0;
            for (i,p) in [163u8,0,164,1,5].iter().enumerate() {party[0xAC+i]=*p;}
            let mut sp=Space::new(&bytes,party,vec![0;0x2000],Box::new(Scripted::new([])),Box::new(Counter::at(123)),Box::new(Dir(dir.clone()))).unwrap();
            sp.product=product;sp.r.initialize(&mut sp.s);system::load(&mut sp.s,sp.files.as_mut(),7).unwrap();
            sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);sp.s.set_dword(RNG,965963027);sp.s.set_word(WORLD_TIME,0);
            sp.hooks=Some(Box::new(Esc(0)));
            assert_eq!(sp.jump_screen(),Ok(()));
            assert_eq!((sp.s.pbyte(6),sp.s.pbyte(0xFC),sp.s.pword(0xF84)),if product {(7,120,0)} else {(7,60,0)},"product {product}");
        }
    }
    #[test]
    fn cancelling_a_rolled_misjump_leaves_the_ship_unchanged_in_product() {
        struct Keys { polls: u32, keys: VecDeque<u8> }
        impl crate::space::tick::Hooks for Keys {
            fn update_cursor(&mut self, sp: &mut Space) {
                self.polls+=1; assert!(self.polls<100_000,"poll budget");
                if self.polls%40==0 && sp.s.byte(KEY)==0 {
                    let k=self.keys.pop_front().expect("unexpected input wait"); sp.keyboard_isr(k); sp.keyboard_isr(k|0x80);
                }
            }
        }
        struct Observer;
        impl crate::lockstep::SpaceObserver for Observer {
            fn read(&mut self, _: &mut Space)->Option<u32> {None}
            fn event(&mut self, _: &mut Space, _: crate::lockstep::SpaceEvent) {}
            fn srand(&mut self, _: &mut Space)->Option<u16> {None}
            fn stop(&mut self, _: &mut Space, _: &str) {}
        }
        struct Machine;
        impl Input for Machine {
            fn next_key(&mut self)->u16 {panic!("keys arrive through Hooks")}
            fn data_mode(&self)->bool {true}
        }
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        for (system, destination) in [(7,4),(2,6)] {
            for mode in 0..6 {
                if system==7 && mode==5 {continue}
                let mut party=crate::ground::party::initial();
                party[6]=system; party[7]=255; party[0xFC]=100; party[0xC1]=1; party[0xC0]=2;
                party[0xF84]=0; party[0xF85]=0;
                for (i,p) in [163u8,0,164,1,5].iter().enumerate() {party[0xAC+i]=*p;}
                let input:Box<dyn Input>=if mode==3 {Box::new(Machine)} else {Box::new(Scripted::new([]))};
                let mut sp=Space::new(&bytes,party,vec![0;0x2000],input,Box::new(Counter::at(123)),Box::new(Dir(dir.clone()))).unwrap();
                sp.product=mode!=1; if mode==2 {sp.observer=Some(Box::new(Observer));}
                sp.r.initialize(&mut sp.s); system::load(&mut sp.s,sp.files.as_mut(),system).unwrap();
                sp.s.set_word(HEADING,90); sp.s.set_word(CURRENT,-1); sp.s.set_dword(RNG,965963027); sp.s.set_word(WORLD_TIME,0);
                let before=(sp.s.party.clone(),sp.s.word(X),sp.s.word(Y),sp.s.word(system::COUNT));
                let keys=match mode {4=>vec![0x3B,1,1],5=>vec![0x3B,0x15],_=>vec![1]};
                sp.hooks=Some(Box::new(Keys {polls:0,keys:keys.into()}));
                assert_eq!(sp.jump_screen(),Ok(()),"system {system}, mode {mode}");
                let cancelled=mode==0 || mode==4;
                assert_eq!((sp.s.pbyte(6),sp.s.pbyte(0xFC),sp.s.pword(0xF84),sp.s.word(WORLD_TIME)),
                    if cancelled {(system,100,0,0)} else {(destination,50,7,7)},"system {system}, mode {mode}");
                if cancelled {
                    assert_eq!((sp.s.party.clone(),sp.s.word(X),sp.s.word(Y),sp.s.word(system::COUNT)),before,"cancel changed the ship or system objects");
                    assert_eq!(sp.jump_asked,None);
                } else if mode==5 {assert_eq!(sp.jump_asked,Some(3),"the confirmed Louzy choice still misjumps");}
            }
        }
    }
    #[test]
    fn every_jump_frame_matches_independent_native_bios_schedule() {
        let Some(dir)=oracle_env::gamefiles() else {return};
        let bytes=std::fs::read(dir.join("S.DAT")).unwrap();
        let (mut expected,mut reads,mut t)=(vec![],VecDeque::new(),123u32);
        for frame in 0..100 {
            reads.push_back(t);
            let finish=t+1+frame/60;
            for now in t..=finish {reads.push_back(now);}
            t=finish;expected.push(t);
        }
        let run=|data:bool| {
            let input:Box<dyn Input>=if data {Box::new(Play::new())} else {Box::new(Scripted::new([]))};
            let clock:Box<dyn Clock>=if data {Box::new(Counter::at(123))} else {Box::new(Bios {reads:reads.clone(),last:123})};
            let mut sp=Space::new(&bytes,crate::ground::party::initial(),vec![0;0x2000],input,clock,Box::new(Dir(dir.clone()))).unwrap();
            sp.r.initialize(&mut sp.s);system::load(&mut sp.s,sp.files.as_mut(),2).unwrap();
            sp.s.set_word(HEADING,90);sp.s.set_word(CURRENT,-1);sp.s.set_word(MOVING,0);sp.s.set_word(THRUST,0);
            let mut state=sp.jump_state();let mut party=sp.s.party.as_slice().try_into().unwrap();
            let mut probe=Frames {inner:JumpHost::new(&mut sp),at:vec![]};
            jump::animate_jump_transition(&mut probe,&mut state,&mut party).unwrap();
            let frames=probe.at;assert_eq!(frames,expected);
            assert_eq!(sp.clock.ticks(),263);(frames,super::super::play::hash(&mut sp),sp.ev.reads)
        };
        assert_eq!(run(false),run(true));
    }
}
