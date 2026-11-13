use super::{Space, render::Render, state::*};
use mt_engine::{io::mouse::{hit, Hotspot, MouseEvent}, present::{ly, prim::Raster}};

pub const ENABLED: u16 = 0x4856;
pub const LIVE: u16 = 0x484E;
pub const CALLBACK: u16 = 0x485A;
pub const CURSOR_Y: u16 = 0x4862;
pub const CURSOR_X: u16 = 0x4864;
pub const DRAWING_STARTED: u16 = 0x4866;
pub const CURSOR_VISIBLE: u16 = 0x4867;
pub const CLIP: u16 = 0x4868;
pub const PENDING: u16 = 0x486A;
pub const BOARDING: u16 = 0x4626;
pub const CURSOR_PAGE: usize = 5;
pub const FLIGHT_HITS: u16 = 0x0D16;
pub const CREW_HITS: u16 = 0x063A;
pub const PILOT_HITS: u16 = 0x0A58;
pub const COMPUTER_HITS: u16 = 0x0AA9;
pub const ENGINEER_HITS: u16 = 0x0AF1;
pub const COMMS_HITS: u16 = 0x0B27;
pub const MEDICAL_HITS: u16 = 0x0B6F;
pub const GUN_HITS: u16 = 0x0BE4;
pub const MISSILE_HITS: u16 = 0x0C35;
pub const BOARDING_HITS: u16 = 0x0C8F;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Driver { pub x: i16, pub y: i16, pub buttons: u16, pub range: [i16; 4], pub(crate) seen: (i16, i16) }
impl Default for Driver { fn default() -> Self { Driver { x: 320, y: 100, buttons: 0, range: [0, 639, 0, 199], seen: (-1, -1) } } }
impl Driver {
    pub fn moved(&mut self, x: i16, y: i16) { self.x = x.clamp(self.range[0], self.range[1]); self.y = y.clamp(self.range[2], self.range[3]); }
}

pub fn table(s: &State, table: u16) -> impl Iterator<Item = Hotspot> + '_ {
    (0..65536 / 9).map(move |i| {
        let at = table.wrapping_add(9 * i as u16);
        Hotspot { top: s.word(at), left: s.word(at.wrapping_add(2)), height: s.word(at.wrapping_add(4)), width: s.word(at.wrapping_add(6)), id: s.byte(at.wrapping_add(8)) }
    }).take_while(|h| h.id != 0)
}

impl Space {
    pub fn init_mouse(&mut self) {
        self.mouse = Driver::default();
        self.s.set_word(ENABLED, -1); self.s.set_word(ENABLED + 2, 2); self.s.set_byte(CURSOR_VISIBLE, 0);
        self.mouse_bounds(true);
    }
    pub fn mouse_bounds(&mut self, full: bool) {
        self.mouse.range = if full { [0, 622, 0, 192] } else { [506, 624, 67, 192] };
    }
    fn live(&mut self) {
        if self.observer.is_some() { return; }
        let m = self.input.mouse();
        self.mouse.buttons = m.buttons;
        if m.col >= 0 && m.row >= 0 && (m.col, m.row) != self.mouse.seen { self.mouse.seen = (m.col, m.row); self.mouse.moved(m.col.wrapping_mul(2), m.row); }
    }
    pub fn read_mouse(&mut self) -> (u16, i16, i16) {
        self.live();
        let (buttons, mut x, mut y) = (self.mouse.buttons, self.mouse.x >> 1, self.mouse.y);
        self.s.set_word(LIVE, buttons as i16);
        if self.s.word(CLIP) != 0 && y > 46 && x <= 192 {
            if self.s.word(LIVE + 6) <= 46 { y = 46 } else { x = 193 }
            self.mouse.moved(x << 1, y);
            self.input.set_mouse_position(x, y);
        }
        self.s.set_word(LIVE + 6, y); self.s.set_word(LIVE + 4, x);
        (buttons, x, y)
    }
    pub fn mouse_callback(&mut self, m: MouseEvent) {
        self.mouse.moved(m.col.wrapping_mul(2), m.row);
        self.mouse.seen = (m.col, m.row);
        let (x, y) = (self.mouse.x >> 1, self.mouse.y);
        self.s.set_word(CALLBACK, m.event as i16); self.s.set_word(CALLBACK + 2, m.buttons as i16);
        self.s.set_word(CALLBACK + 4, y); self.s.set_word(CALLBACK + 6, x);
        self.s.set_byte(KEY, if m.event == 0x10 { 1 } else { 0x6E });
    }
    pub(crate) fn drop_stale_clicks(&mut self) {
        if !self.product || self.observer.is_some() || self.input.data_mode() { return }
        while self.input.arrived() {
            match self.input.poll(self.clock.as_mut()) { mt_engine::io::Event::Mouse(_) => {} e @ (mt_engine::io::Event::Key(_) | mt_engine::io::Event::KeyUp(_)) => self.apply(e), _ => break }
        }
        if self.s.byte(KEY) == 0x6E { self.s.set_byte(KEY, 0); self.s.set_word(CALLBACK, 0); }
    }
    pub fn mouse_command(&mut self, hits: u16) {
        let (row, col) = (self.s.word(CALLBACK + 4), self.s.word(CALLBACK + 6));
        let found = hit(table(&self.s, hits), col, row);
        if let Some(id) = found { self.s.set_byte(PENDING, 1); self.s.set_byte(KEY, id); self.s.set_word(CALLBACK, 0); }
    }
    pub fn station_mouse_command(&mut self) { if let Some(hits) = self.station_hits() { self.mouse_command(hits); } }
    pub fn station_hits(&self) -> Option<u16> {
        if self.s.byte(BOARDING) != 0 { return Some(BOARDING_HITS); }
        if self.s.word(STATION_ACTIVE) == 0 { return None; }
        let station = self.s.word(STATION);
        Some(match station.wrapping_sub(10) {
            0 => PILOT_HITS, 3 => COMPUTER_HITS, 4 => ENGINEER_HITS, 5 => COMMS_HITS, 6 | 7 => MEDICAL_HITS,
            1 | 2 => if self.s.pbyte((station as u16).wrapping_add(0xBD)) == 9 { MISSILE_HITS } else { GUN_HITS },
            _ => return None,
        })
    }

    fn cursor_pixels(&self) -> bool { crate::ui::software_cursor() && !self.product }
    fn cursor_rows(y: i16) -> i32 { if y as i32 + 8 >= 200 { 200 - y as i32 } else { 8 } }
    fn save_cursor_background(&mut self, y: i16, x: i16, slot: i32) {
        if !self.cursor_pixels() { return; }
        let page = Render::page(&self.s);
        self.r.fb.copy(page, y as i32, (x >> 3) as i32, Self::cursor_rows(y), 2, CURSOR_PAGE, 120, slot);
    }
    fn restore_cursor_background(&mut self, y: i16, x: i16) {
        if !self.cursor_pixels() { return; }
        let page = Render::page(&self.s);
        self.r.fb.copy(CURSOR_PAGE, 120, 0, Self::cursor_rows(y), 2, page, y as i32, (x >> 3) as i32);
    }
    fn draw_cursor(&mut self, y: i16, x: i16) {
        if !self.cursor_pixels() { (self.r.pen.colour, self.r.pen.op) = crate::ui::CURSOR_PEN; return; }
        let page = Render::page(&self.s);
        ly::layout(&mut self.r.fb, Raster::Wrap, page, self.r.assets.layout("FONT.LY"), 10, y as i32, x as i32, &mut self.r.pen);
    }
    pub fn show_software_cursor(&mut self) {
        if self.s.word(ENABLED) == 0 || self.s.byte(CURSOR_VISIBLE) != 0 { return; }
        self.s.set_byte(DRAWING_STARTED, 0); self.s.set_byte(CURSOR_VISIBLE, 1);
        let (_, x, y) = self.read_mouse();
        self.s.set_word(CURSOR_Y, y); self.s.set_word(CURSOR_X, x);
        let started = self.s.word(0x49EE) == 0;
        if started { self.s.set_byte(DRAWING_STARTED, 1); self.r.front(&mut self.s); }
        self.save_cursor_background(y, x, 0);
        self.draw_cursor(y, x);
        if self.s.byte(DRAWING_STARTED) != 0 { self.r.back(&mut self.s); }
    }
    pub fn hide_software_cursor(&mut self) {
        if self.s.byte(CURSOR_VISIBLE) == 0 { return; }
        self.s.set_byte(DRAWING_STARTED, 0);
        let started = self.s.word(0x49EE) == 0;
        if started { self.s.set_byte(DRAWING_STARTED, 1); self.r.front(&mut self.s); }
        let (y, x) = (self.s.word(CURSOR_Y), self.s.word(CURSOR_X));
        self.restore_cursor_background(y, x);
        self.s.set_byte(CURSOR_VISIBLE, 0); self.s.set_word(CURSOR_Y, -1); self.s.set_word(CURSOR_X, -1);
        if self.s.byte(DRAWING_STARTED) != 0 { self.r.back(&mut self.s); }
    }
    pub fn update_software_cursor(&mut self) {
        if self.s.byte(CURSOR_VISIBLE) == 0 || self.s.word(ENABLED) == 0 { return; }
        let (_, x, y) = self.read_mouse();
        if y == self.s.word(CURSOR_Y) && x == self.s.word(CURSOR_X) { return; }
        self.s.set_byte(DRAWING_STARTED, 0);
        let started = self.s.word(0x49EE) == 0;
        if started { self.s.set_byte(DRAWING_STARTED, 1); self.r.front(&mut self.s); }
        let (oy, ox) = (self.s.word(CURSOR_Y), self.s.word(CURSOR_X));
        self.restore_cursor_background(oy, ox);
        self.s.set_word(CURSOR_Y, y); self.s.set_word(CURSOR_X, x);
        self.save_cursor_background(y, x, 0);
        self.draw_cursor(y, x);
        if self.s.byte(DRAWING_STARTED) != 0 { self.r.back(&mut self.s); }
    }
    pub fn present(&mut self) {
        let (_, x, y) = self.read_mouse();
        let visible = self.s.byte(CURSOR_VISIBLE) != 0;
        if visible { self.r.back(&mut self.s); self.save_cursor_background(y, x, 2); self.draw_cursor(y, x); }
        self.r.flip(&mut self.s);
        if visible {
            let (oy, ox) = (self.s.word(CURSOR_Y), self.s.word(CURSOR_X));
            self.restore_cursor_background(oy, ox);
            if crate::ui::software_cursor() { self.r.fb.copy(CURSOR_PAGE, 120, 2, 8, 2, CURSOR_PAGE, 120, 0); }
            self.s.set_word(CURSOR_Y, y); self.s.set_word(CURSOR_X, x);
        }
    }
    pub fn flip_with_cursor(&mut self) {
        let restore = self.s.byte(CURSOR_VISIBLE) != 0;
        if restore { self.hide_cursor(); }
        self.present();
        if restore { self.show_cursor(); }
    }
}

#[cfg(test)]
mod t {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{io::{Dir, Event, Input, mouse::MouseState}, present::textlog, sim::tick::{Clock, Counter}};
    struct NoPointer;
    impl Input for NoPointer {
        fn next_key(&mut self) -> mt_engine::io::Key { panic!("unexpected keyboard read") }
        fn poll(&mut self, _clock: &mut dyn Clock) -> Event { Event::Idle }
        fn mouse(&mut self) -> MouseState { MouseState { col: -1, row: -1, buttons: 0 } }
    }
    #[test]
    fn a_click_that_arrived_before_the_screen_opened_is_dropped() {
        struct Queued(std::collections::VecDeque<Event>);
        impl Input for Queued {
            fn next_key(&mut self) -> mt_engine::io::Key { 0 }
            fn poll(&mut self, _clock: &mut dyn Clock) -> Event { self.0.pop_front().unwrap_or(Event::Idle) }
            fn arrived(&mut self) -> bool { !self.0.is_empty() }
        }
        let Some(dir) = oracle_env::gamefiles() else { return };
        let data = std::fs::read(dir.join("S.DAT")).unwrap();
        let click = Event::Mouse(MouseEvent { event: 4, buttons: 0, col: 100, row: 50 });
        for product in [false, true] {
            let input = Queued([click, Event::Key(b'A' as u16), click].into());
            let mut sp = Space::new(&data, vec![0; PARTY_SIZE], vec![], Box::new(input), Box::new(Counter::default()), Box::new(Dir(dir.clone()))).unwrap();
            sp.product = product; sp.s.set_byte(KEY, 0x6E);
            sp.drop_stale_clicks();
            assert_eq!((sp.s.byte(KEY), sp.input.arrived()), if product { (0x1E, false) } else { (0x6E, true) }, "product {product}: the A stays, the clicks are gone");
        }
    }
    #[test]
    fn the_flight_title_says_where_the_ship_is_and_what_comes_before_a_jump() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let data = std::fs::read(dir.join("S.DAT")).unwrap();
        for product in [false, true] {
            let mut party = vec![0; PARTY_SIZE]; party[6] = 2;
            let mut sp = Space::new(&data, party, vec![], Box::new(NoPointer), Box::new(Counter::default()), Box::new(Dir(dir.clone()))).unwrap();
            sp.product = product; sp.r.initialize(&mut sp.s); crate::space::system::load(&mut sp.s, sp.files.as_mut(), 2).unwrap(); textlog::set_enabled(true);
            let title = |sp: &mut Space, mode: i16| {
                textlog::clear(&sp.r.fb); sp.r.back(&mut sp.s); sp.hud(mode);
                let page = Render::page(&sp.s);
                let mut runs: Vec<_> = textlog::frame(&sp.r.fb, page).visible.into_iter().filter(|r| r.y < 8 && r.x < 250 && r.colour == 0).collect(); runs.sort_by_key(|r| r.x);
                runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect::<String>()
            };
            let free = title(&mut sp, -1);
            let world = (0..sp.s.word(crate::space::system::COUNT)).find(|&i| sp.s.array(crate::space::system::WORLD, i) != 0).expect("a world in Efate");
            sp.s.set_word(CURRENT, world);
            let docked = title(&mut sp, world);
            let stray = |sp: &mut Space, mode: i16| { sp.hud(mode); let page = Render::page(&sp.s);
                textlog::frame(&sp.r.fb, page).visible.into_iter().filter(|r| r.y < 8 && r.x < 250 && r.colour != 0 && r.bitmap != r.background).map(|r| r.bytes[0] as char).collect::<String>() };
            if product {
                assert_eq!(stray(&mut sp, world), "", "the dock title drawn again");
                assert_eq!(stray(&mut sp, 99), "", "the take-off erasure"); sp.s.set_word(CURRENT, -1);
                assert_eq!((stray(&mut sp, -1), title(&mut sp, -1)), (String::new(), "Flying in the Efate system".to_string()), "free again");
            }
            textlog::set_enabled(false);
            if product { assert_eq!((free.as_str(), docked.as_str()), ("Flying in the Efate system", "Docked at Efate. Take off to fly or jump.")); }
            else { assert_eq!((free.as_str(), docked.as_str()), ("efate in-system travel", "tethered to efate space dock")); }
        }
    }
    #[test]
    fn product_reads_comms_where_the_1990_writes_commo() {
        assert_eq!(crate::space::ui::comms(b"Commo COMMO: Jurgen COMM commo".to_vec()), b"Comms COMMS: Jurgen COMM commo");
        let Some(dir) = oracle_env::gamefiles() else { return };
        let data = std::fs::read(dir.join("S.DAT")).unwrap();
        for product in [false, true] {
            let mut sp = Space::new(&data, vec![0; PARTY_SIZE], vec![], Box::new(NoPointer), Box::new(Counter::default()), Box::new(Dir(dir.clone()))).unwrap();
            sp.product = product; sp.r.initialize(&mut sp.s); textlog::set_enabled(true);
            sp.frame(); sp.set_text_position(120, 20); sp.draw_text(b"COMMO: Jurgen");
            let page = Render::page(&sp.s);
            let mut runs = textlog::frame(&sp.r.fb, page).visible; runs.sort_by_key(|r| (r.y, r.x));
            let seen: String = runs.iter().flat_map(|r| r.bytes.iter().map(|&b| b as char)).collect();
            textlog::set_enabled(false);
            let (new, old) = (seen.contains("Comms") && seen.contains("COMMS:"), seen.contains("Commo") && seen.contains("COMMO:"));
            assert_eq!((new, old), (product, !product), "product {product}: {seen}");
        }
    }
    #[test]
    fn product_cursor_leaves_pixels_and_text_cells_alone() {
        let Some(dir) = oracle_env::gamefiles() else { return };
        let data = std::fs::read(dir.join("S.DAT")).unwrap();
        let mut sp = Space::new(&data, vec![0; PARTY_SIZE], vec![], Box::new(NoPointer), Box::new(Counter::default()), Box::new(Dir(dir))).unwrap();
        sp.r.initialize(&mut sp.s); sp.init_mouse(); sp.s.set_word(CLIP, 0);
        crate::ui::set_software_cursor(false); textlog::set_enabled(true);
        sp.r.text(&sp.s, b"Vehicle", 100, 96, 15);
        let page = Render::page(&sp.s);
        let (planes, cells) = (sp.r.fb.planes.clone(), textlog::frame(&sp.r.fb, page).visible);
        assert!(!cells.is_empty());
        sp.mouse.moved(200, 100); sp.show_software_cursor();
        assert_eq!((sp.s.byte(CURSOR_VISIBLE), sp.s.word(CURSOR_X), sp.s.word(CURSOR_Y)), (1, 100, 100));
        sp.mouse.moved(208, 103); sp.update_software_cursor();
        assert_eq!((sp.s.word(CURSOR_X), sp.s.word(CURSOR_Y)), (104, 103));
        sp.present(); sp.hide_software_cursor();
        assert_eq!((sp.s.byte(CURSOR_VISIBLE), sp.s.word(CURSOR_X), sp.s.word(CURSOR_Y)), (0, -1, -1));
        assert_eq!(sp.r.fb.planes, planes, "no pixel of the 1990 cursor in product mode");
        assert_eq!(textlog::frame(&sp.r.fb, page).visible, cells, "the text cells under the pointer survive");
        assert_eq!((sp.r.pen.colour, sp.r.pen.op), crate::ui::CURSOR_PEN);
        crate::ui::set_software_cursor(true); sp.r.pen.colour = 7; sp.show_software_cursor();
        assert_ne!(sp.r.fb.planes, planes); assert_eq!((sp.r.pen.colour, sp.r.pen.op), crate::ui::CURSOR_PEN);
        sp.hide_software_cursor(); sp.product = true; let planes = sp.r.fb.planes.clone(); let cells = textlog::frame(&sp.r.fb, page).visible;
        sp.mouse.moved(200, 100); sp.show_software_cursor(); sp.mouse.moved(208, 103); sp.update_software_cursor(); sp.present(); sp.hide_software_cursor();
        assert_eq!(sp.r.fb.planes, planes, "no pixel of the 1990 cursor in product mode (space)");
        assert_eq!(textlog::frame(&sp.r.fb, page).visible, cells, "the text cells under the pointer survive");
        textlog::set_enabled(false);
    }
}
