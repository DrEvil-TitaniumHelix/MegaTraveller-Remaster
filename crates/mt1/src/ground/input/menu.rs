use super::{special_key, ui, ASCII, MOUSE_PENDING, RECTANGLES};
use crate::ground::{render, state::{Far, Ground, Mem}};
use mt_engine::{io::mouse::Hotspot, present::prim::Raster};

#[derive(Clone, Debug)]
pub struct Menu { pub labels: Vec<Far>, pub hotkeys: Far, pub help: Far, pub colors: Vec<u8>, pub submenus: Vec<Far>, pub callbacks: Vec<u16> }
impl Menu {
    pub fn read(mem: &Mem, p: Far) -> Option<Self> {
        let n = mem.read(p.add(32), 1) as usize;
        if !(1..=6).contains(&n) { return None }
        let ptr = |p| Far::new(mem.read(p, 2) as u16, mem.read(p.add(2), 2) as u16);
        Some(Self { labels: (0..n).map(|i| ptr(p.add(i as i32 * 4))).collect(), hotkeys: ptr(p.add(24)), help: ptr(p.add(28)),
            colors: (0..n).map(|i| mem.read(p.add(33 + i as i32), 1) as u8).collect(),
            submenus: (0..n).map(|i| ptr(p.add(39 + i as i32 * 4))).collect(),
            callbacks: (0..n).map(|i| mem.read(p.add(63 + i as i32 * 2), 2) as u16).collect() })
    }
}
fn nonnull(p: Far) -> bool { p.off != 0 || p.seg != 0 }
pub fn rectangles(g: &Ground, ptr: Far, count: usize) -> Vec<Hotspot> {
    (0..count.min(255)).map(|i| { let p = ptr.add(i as i32 * 8);
        Hotspot { top: g.mem.read_i(p, 2) as i16, left: g.mem.read_i(p.add(2), 2) as i16,
            height: g.mem.read_i(p.add(4), 2) as i16, width: g.mem.read_i(p.add(6), 2) as i16, id: (i+1) as u8 }
    }).collect()
}
fn outline(g: &mut Ground, h: Hotspot, color: u8) {
    g.enter("outline_menu_item"); let p = ui::page(g);
    g.fb.rect(Raster::Wrap, p, h.top as i32, h.left as i32, h.top as i32 + h.height as i32 - 1, h.left as i32 + h.width as i32, color, g.mem.u8(0x52B2));
    g.mem.set_u8(0x52C8, color); ui::frame(g,"outline_menu_item");
}

pub fn select(g: &mut Ground, rects: &[Hotspot], selected: i16, color: u8, highlight: u8, hotkeys: Option<&[u8]>) -> i16 {
    select_with_poll(g, rects, selected, color, highlight, hotkeys, &mut |g| g.poll())
}
pub fn select_with_poll(g: &mut Ground, rects: &[Hotspot], selected: i16, color: u8, highlight: u8, hotkeys: Option<&[u8]>, poll: &mut dyn FnMut(&mut Ground)) -> i16 {
    let n = rects.len().min(255); if n == 0 { return -1 }
    let mut selected = if selected < 0 || selected as usize >= n { 0 } else { selected as usize };
    let color = if g.mem.i16(0x503E) == 2 { 0 } else { color };
    render::cursor_off(g);
    let accept_t = hotkeys.is_some_and(|s| !s.contains(&b'T'));
    for (i, h) in rects[..n].iter().enumerate() {
        let mut h = *h; h.id = (i+1) as u8;
        g.mem.write_bytes(Mem::ds(RECTANGLES + i as u16 * 9), &h.pack());
    }
    g.mem.set_u8(RECTANGLES + n as u16 * 9 + 8, 0);
    ui::target(g, true); outline(g, rects[selected], color);
    if g.input.wants_decision() { crate::play::decide(g, crate::play::menus::selector(&rects[..n], selected)); }
    let mut blink = true; let mut deadline = g.now().wrapping_add(9);
    let result = loop {
        g.mem.set_key(0); render::cursor_on(g);
        while g.mem.key() == 0 {
            poll(g);
            if g.now() > deadline {
                render::cursor_off(g); ui::target(g, true); deadline = g.now().wrapping_add(9);
                outline(g, rects[selected], if blink { highlight } else { color }); blink = !blink;
                render::cursor_on(g);
            }
        }
        render::cursor_off(g); ui::target(g, true);
        let code = g.mem.key();
        if code == 0x6E {
            special_key(g, RECTANGLES, n);
            if g.mem.key() == 0x6E { g.mem.set_key(0); continue }
            if blink { outline(g, rects[selected], highlight) }
            let choice = g.mem.key() as i16 - 1;
            g.mem.set_key(0); g.mem.set_u8(MOUSE_PENDING, 0); break choice;
        }
        if code == 0x1C || (accept_t && code == 0x14) { if blink { outline(g, rects[selected], highlight) } break selected as i16 }
        if matches!(code, 0x4B | 0x48 | 0x4D | 0x50) {
            if blink { outline(g, rects[selected], highlight) }
            selected = if matches!(code, 0x4B | 0x48) { (selected + n - 1) % n } else { (selected + 1) % n };
            outline(g, rects[selected], color); blink = true; deadline = g.now().wrapping_add(9);
        } else {
            let choice = if (0x3B..=0x44).contains(&code) && (code - 0x3B) < n as u8 { Some((code - 0x3B) as i16) }
            else if code == 1 { Some(-1) }
            else { hotkeys.and_then(|s| { let c = g.mem.u8(ASCII + code as u16); (c != 0).then(|| s.iter().position(|&b| b == c)).flatten().map(|i| i as i16) }) };
            if let Some(choice) = choice { if blink { outline(g, rects[selected], highlight) } break choice }
        }
    };
    ui::target(g, false); g.mem.set_key(0); result
}
pub fn weapon_menu(g: &mut Ground, ptr: Far, count: usize, selected: i16, color: u8, highlight: u8, hotkeys: Option<Far>) -> i16 {
    let rects = rectangles(g, ptr, count); let keys = hotkeys.map(|p| ui::bytes(g, p));
    select(g, &rects, selected, color, highlight, keys.as_deref())
}
pub fn draw_menu(g: &mut Ground, p: Far) {
    if Menu::read(&g.mem,p).is_none() {return}
    render::draw_menu(g, p);
}
pub fn present_menu(g: &mut Ground) { render::present_menu(g) }

pub fn run_menu_with(g: &mut Ground, p: Far, depth: u16, selected: i16, action: &mut dyn FnMut(&mut Ground, u16, i16) -> i16) -> i16 {
    if depth >= 16 { return -1 }
    let Some(m) = Menu::read(&g.mem, p) else { return -1 };
    if depth == 0 { render::dismiss_message(g, 1) }
    draw_menu(g, p); present_menu(g); render::cursor_off(g);
    let rects = rectangles(g, Mem::ds(0x480B), m.labels.len());
    let keys = nonnull(m.hotkeys).then(|| ui::bytes(g, m.hotkeys));
    let result = loop {
        if g.input.wants_decision() { crate::play::decide(g, crate::play::menus::af79(g, p, &m, selected, &rects)); }
        let choice = select(g, &rects, selected, 6, 7, keys.as_deref());
        if choice < 0 || choice as usize >= m.labels.len()-1 {
            if depth == 0 { render::dismiss_message(g, 1) } break -1;
        }
        let i = choice as usize;
        let result = if nonnull(m.submenus[i]) { run_menu_with(g, m.submenus[i], depth+1, 0, action) }
        else if m.callbacks[i] != 0 { action(g, m.callbacks[i], choice) } else { choice };
        if result != -1 { break result }
        draw_menu(g, p); present_menu(g);
    };
    if depth == 0 { draw_menu(g, ui::default_menu()); present_menu(g) }
    result
}
pub fn run_menu(g: &mut Ground, p: Far, depth: u16, selected: i16) -> i16 {
    run_menu_with(g, p, depth, selected, &mut |g, callback, choice| {
        match callback {
            0x06CD => crate::ground::commands_items::take(g),
            0x0C61 => crate::ground::commands_items::use_command(g),
            0x5367 | 0x53CB | 0x5427 => crate::ground::commands_talk::bribe_callback(g,callback),
            0xAD91 => { ui::message(g,b"Not Implemented"); 0 },
            0xADAA => { g.mem.set_i16(0x6EFE, choice); 1 }
            _ => { eprintln!("not yet (M4B): menu action {callback:04X}"); -1 }
        }
    })
}
