mod oracle_env;
use mt1::space::{Space, mouse::*, render::Render, select::HOTSPOTS, state::*, tick::Hooks};
use mt_engine::{io::{Dir, Event, Input, mouse::{MouseEvent, MouseState}}, present::fb::{PAGE, PLANE}, sim::tick::{Clock, Counter}};
use std::collections::VecDeque;

struct NoPointer;
impl Input for NoPointer {
    fn next_key(&mut self) -> mt_engine::io::Key { panic!("unexpected keyboard read") }
    fn poll(&mut self, _clock: &mut dyn Clock) -> Event { Event::Idle }
    fn mouse(&mut self) -> MouseState { MouseState { col: -1, row: -1, buttons: 0 } }
}

fn space() -> Option<Space> {
    let dir = oracle_env::gamefiles()?;
    let data = std::fs::read(dir.join("S.DAT")).unwrap();
    Some(Space::new(&data, vec![0; PARTY_SIZE], vec![], Box::new(NoPointer),
        Box::new(Counter::default()), Box::new(Dir(dir))).unwrap())
}
fn click(col: i16, row: i16) -> MouseEvent { MouseEvent { event: 4, buttons: 0, col, row } }

#[test]
fn original_flight_and_pilot_tables_and_all_pixel_hits() {
    let Some(mut sp) = space() else { return };
    let flight = [[67,253,10,61,0x1F], [79,253,10,61,0x2E], [91,253,10,61,0x24], [103,253,10,61,0x10],
        [139,293,17,21,0x48], [178,293,17,21,0x50], [138,252,15,22,0x4B], [155,268,14,23,0x4D]];
    let pilot = [[3,209,20,43,1], [61,23,10,49,0x1E], [72,23,10,49,0x2F], [88,5,12,14,0x2D],
        [116,148,13,18,0x48], [131,148,13,18,0x50], [152,128,15,20,0x4B], [168,150,16,18,0x4D]];
    for (at, expected) in [(FLIGHT_HITS, flight), (PILOT_HITS, pilot)] {
        let got: Vec<_> = table(&sp.s, at).map(|h| [h.top,h.left,h.height,h.width,h.id as i16]).collect();
        assert_eq!(got, expected, "S.DAT table {at:X}");
        for row in -1..=200 { for col in -1..=320 {
            let id = expected.iter().find(|r| row >= r[0] && col >= r[1] && row < r[0]+r[2] && col < r[1]+r[3])
                .map(|r| r[4] as u8);
            sp.s.set_word(CALLBACK, 4); sp.s.set_word(CALLBACK + 4, row); sp.s.set_word(CALLBACK + 6, col);
            sp.s.set_byte(KEY, 0x6E); sp.s.set_byte(PENDING, 0);
            sp.mouse_command(at);
            assert_eq!((sp.s.byte(KEY), sp.s.byte(PENDING), sp.s.word(CALLBACK)),
                (id.unwrap_or(0x6E), u8::from(id.is_some()), if id.is_some() { 0 } else { 4 }),
                "table {at:X}, ({col},{row})");
        } }
    }
    for (i, id) in [0x1E, 0x2F, 0, 0x48].into_iter().enumerate() {
        let at = HOTSPOTS + 9 * i as u16;
        for (k, v) in [10,20,10,10].into_iter().enumerate() { sp.s.set_word(at + 2*k as u16, v); }
        sp.s.set_byte(at + 8, id);
    }
    sp.mouse_callback(click(25, 15)); sp.mouse_command(HOTSPOTS);
    assert_eq!(sp.s.byte(KEY), 0x1E);
    sp.s.set_byte(HOTSPOTS + 8, 0);
    sp.mouse_callback(click(25, 15)); sp.mouse_command(HOTSPOTS);
    assert_eq!(sp.s.byte(KEY), 0x6E);
}

#[test]
fn callback_release_codes_and_driver_ranges() {
    let Some(mut sp) = space() else { return };
    sp.init_mouse();
    assert_eq!((sp.s.word(ENABLED), sp.s.word(ENABLED+2)), (-1, 2));
    assert_eq!((sp.mouse.x, sp.mouse.y), (320, 100));
    for (event, key) in [(4,0x6E), (0x10,1), (0x14,0x6E)] {
        sp.mouse_callback(MouseEvent { event, buttons: 2, col: 303, row: 147 });
        assert_eq!((sp.s.word(CALLBACK), sp.s.word(CALLBACK+2), sp.s.word(CALLBACK+4), sp.s.word(CALLBACK+6)),
            (event as i16, 2, 147, 303));
        assert_eq!(sp.s.byte(KEY), key);
    }
    sp.mouse.moved(200, 30); sp.mouse_bounds(false);
    assert_eq!((sp.mouse.x, sp.mouse.y), (200, 30));
    sp.mouse_callback(click(0, 0));
    assert_eq!((sp.s.word(CALLBACK+6), sp.s.word(CALLBACK+4)), (253, 67));
    sp.mouse_callback(click(319, 199));
    assert_eq!((sp.s.word(CALLBACK+6), sp.s.word(CALLBACK+4)), (312, 192));
}

#[test]
fn station_table_selection_matches_4707() {
    let Some(mut sp) = space() else { return };
    for (station, weapon, boarding, at) in [(10,0,0,PILOT_HITS), (13,0,0,COMPUTER_HITS),
        (14,0,0,ENGINEER_HITS), (15,0,0,COMMS_HITS), (16,0,0,MEDICAL_HITS), (17,0,0,MEDICAL_HITS),
        (11,0,0,GUN_HITS), (12,0,0,GUN_HITS), (11,9,0,MISSILE_HITS), (12,9,0,MISSILE_HITS),
        (10,0,1,BOARDING_HITS)] {
        sp.s.set_word(STATION, station); sp.s.set_word(STATION_ACTIVE, 1);
        sp.s.set_pbyte(station as u16 + 0xBD, weapon); sp.s.set_byte(BOARDING, boarding);
        let hits: Vec<_> = table(&sp.s, at).collect();
        assert!(!hits.is_empty());
        for h in hits {
            sp.mouse_callback(click(h.left, h.top));
            sp.station_mouse_command();
            assert_eq!(sp.s.byte(KEY), h.id, "station {station}, weapon {weapon}, table {at:X}");
        }
    }
    sp.s.set_byte(BOARDING, 0); sp.s.set_word(STATION_ACTIVE, 0);
    sp.mouse_callback(click(209, 3)); sp.station_mouse_command();
    assert_eq!(sp.s.byte(KEY), 0x6E);
    sp.s.set_byte(BOARDING, 1); sp.station_mouse_command();
    assert_eq!(sp.s.byte(KEY), 1, "boarding takes precedence over station_active");
}

#[test]
fn read_mouse_clips_to_the_side_it_came_from() {
    let Some(mut sp) = space() else { return };
    sp.init_mouse(); sp.s.set_word(CLIP, 1);
    for (old_y, x, y, expected) in [(46,100,80,(100,46)), (47,100,80,(193,80)),
        (47,192,47,(193,47)), (46,192,46,(192,46)), (46,193,80,(193,80))] {
        sp.s.set_word(LIVE+6, old_y); sp.mouse.moved(x*2, y);
        assert_eq!(sp.read_mouse(), (0,expected.0,expected.1));
        assert_eq!((sp.mouse.x,sp.mouse.y), (expected.0*2,expected.1));
        assert_eq!((sp.s.word(LIVE+4),sp.s.word(LIVE+6)), expected);
    }
    sp.s.set_word(CLIP, 0); sp.mouse.moved(201, 80);
    assert_eq!(sp.read_mouse(), (0,100,80), "driver X is halved");
}

struct Clicks(VecDeque<MouseEvent>);
impl Hooks for Clicks {
    fn update_cursor(&mut self, sp: &mut Space) { sp.mouse_callback(self.0.pop_front().expect("unexpected extra cursor poll")); }
}
#[test]
fn rectangle_selector_accepts_mouse_hit_after_miss_and_right_release_cancels() {
    let Some(mut sp) = space() else { return };
    sp.r.initialize(&mut sp.s);
    let rects = [[61,73,23,28], [61,101,15,28]];
    sp.hooks = Some(Box::new(Clicks([click(0,100),click(102,62)].into())));
    assert_eq!(sp.select_rect(&rects, 0, 15, 0, None), 1);
    assert_eq!((sp.s.byte(KEY),sp.s.byte(PENDING),Render::page(&sp.s)), (0,0,1));
    sp.hooks = Some(Box::new(Clicks([MouseEvent { event: 0x10, ..click(102,62) }].into())));
    assert_eq!(sp.select_rect(&rects, 0, 15, 0, None), -1);
}

fn page_bytes(sp: &Space, page: usize) -> Vec<u8> {
    (0..4).flat_map(|p| sp.r.fb.planes[p*PLANE+page*PAGE..p*PLANE+(page+1)*PAGE].iter().copied()).collect()
}
fn pattern(sp: &mut Space) {
    for (i,b) in sp.r.fb.planes.iter_mut().enumerate() { *b = (i.wrapping_mul(73) ^ (i >> 5)) as u8; }
}
#[test]
fn cursor_show_move_hide_restores_display_pages_and_selection() {
    let Some(mut sp) = space() else { return };
    sp.r.initialize(&mut sp.s); sp.init_mouse(); sp.s.set_word(CLIP, 0); pattern(&mut sp);
    for hidden in [false,true] { for (x,y) in [(0,0),(160,100),(311,192)] {
        if hidden { sp.r.back(&mut sp.s) } else { sp.r.front(&mut sp.s) }
        sp.mouse.moved(x*2,y);
        let pages: Vec<_> = (0..8).map(|p| page_bytes(&sp,p)).collect();
        sp.show_software_cursor();
        assert_eq!(sp.s.byte(CURSOR_VISIBLE), 1);
        assert_eq!((sp.s.word(CURSOR_X),sp.s.word(CURSOR_Y)), (x,y));
        assert_eq!(Render::page(&sp.s), usize::from(hidden));
        assert_ne!(page_bytes(&sp,0), pages[0], "cursor must draw");
        let shown = sp.r.fb.planes.clone(); sp.show_software_cursor(); sp.update_software_cursor();
        assert_eq!(sp.r.fb.planes, shown, "stationary/already shown is unchanged");
        sp.mouse.moved(if x == 311 { 608 } else { (x+1)*2 }, if y == 192 { 191 } else { y+1 });
        sp.update_software_cursor(); sp.hide_software_cursor();
        for (page, bytes) in pages.iter().enumerate() {
            if page != CURSOR_PAGE { assert_eq!(page_bytes(&sp,page), *bytes, "page {page}, ({x},{y})"); }
        }
        assert_eq!((sp.s.byte(CURSOR_VISIBLE),sp.s.word(CURSOR_X),sp.s.word(CURSOR_Y)), (0,-1,-1));
        assert_eq!(Render::page(&sp.s), usize::from(hidden));
        assert_eq!(sp.ev.copies, 0, "cursor scratch copies are outside the measured game-copy count");
    } }
}

#[test]
fn cursor_present_restores_old_page_and_saves_new_background() {
    let Some(mut sp) = space() else { return };
    sp.r.initialize(&mut sp.s); sp.init_mouse(); sp.s.set_word(CLIP, 0); pattern(&mut sp);
    let pages = [page_bytes(&sp,0),page_bytes(&sp,1)];
    sp.r.back(&mut sp.s); sp.show_software_cursor(); sp.mouse.moved(606,147); sp.present();
    assert_eq!(sp.s.word(ACTIVE_PAGE), 1);
    assert_eq!(page_bytes(&sp,0), pages[0]);
    assert_ne!(page_bytes(&sp,1), pages[1]);
    assert_eq!((sp.s.word(CURSOR_X),sp.s.word(CURSOR_Y)), (303,147));
    sp.hide_software_cursor();
    assert_eq!(page_bytes(&sp,1), pages[1]);
    assert_eq!(Render::page(&sp.s), 0);
    assert_eq!(sp.ev.copies, 0);
    sp.s.set_word(ENABLED,0);
    let before = sp.r.fb.planes.clone(); sp.show_software_cursor(); sp.update_software_cursor();
    assert_eq!(sp.r.fb.planes, before);
    assert_eq!(sp.s.byte(CURSOR_VISIBLE),0);
}

#[test]
fn zz_skip_report() { oracle_env::report() }
