mod computer;
mod identify;
mod inventory;
pub mod raster;
mod render;
mod support;

pub const STACK: i32 = 0x9000;
pub const KEY: i32 = 0x483A;
pub const PENDING: i32 = 0x463C;
pub const STATION: i32 = 0x399;
pub const PREVIOUS: i32 = 0x39B;
pub const TACTICAL_DRAWN: i32 = 0x397;
pub type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Reply {
    pub ax: u16,
    pub dx: u16,
}
impl Reply {
    pub fn int(n: i32) -> Self {
        Self { ax: n as u16, dx: 0 }
    }
    pub fn signed(self) -> i32 {
        self.ax as i16 as i32
    }
    pub fn long(self) -> u32 {
        self.ax as u32 | ((self.dx as u32) << 16)
    }
}

pub trait Host {
    fn crew_records_available(&self)->bool {true}
    fn take_switch(&mut self) -> Option<i32> { None }
    fn unmanned(&mut self, _station: i32) {}
    fn repair_sign(&mut self) -> Option<Vec<u8>> { None }
    fn click_through(&self) -> bool { false }
    fn bars(&mut self, _on: Option<bool>) -> bool { false }
    fn ds(&self) -> i32;
    fn decision_wait(&mut self, _routine: &str, _selector: Option<(usize,i16)>, _live: &[i32]) {}
    fn entry_di(&self) -> Result<u16> {
        Err("A456: destroyed-weapon text requires incoming DI".into())
    }
    fn read(&self, off: i32, seg: i32, width: usize) -> u32;
    fn write(&mut self, off: i32, seg: i32, value: u32, width: usize);
    fn call(&mut self, name: &str, args: &[i32]) -> Result<Reply>;
    fn key_spin(&mut self) -> Result<()> {
        Err("4707: stable unmapped-key spin at 48D3".into())
    }
    fn int_call(&mut self, name: &str, args: &[i32]) -> Result<i32> {
        Ok(self.call(name, args)?.signed())
    }
    fn b(&self, off: i32) -> i32 {
        self.read(off & 65535, self.ds(), 1) as u8 as i32
    }
    fn w(&self, off: i32) -> i32 {
        self.read(off & 65535, self.ds(), 2) as u16 as i16 as i32
    }
    fn sb(&mut self, off: i32, v: i32) {
        self.write(off & 65535, self.ds(), v as u8 as u32, 1)
    }
    fn sw(&mut self, off: i32, v: i32) {
        self.write(off & 65535, self.ds(), v as u16 as u32, 2)
    }
    fn sd(&mut self, off: i32, v: u32) {
        self.write(off & 65535, self.ds(), v, 4)
    }
    fn ptr(&self, off: i32) -> [i32; 2] {
        [self.w(off) & 65535, self.w(off + 2) & 65535]
    }
    fn pb(&self, off: i32) -> i32 {
        let p = self.ptr(0x3456);
        self.read((p[0] + off) & 65535, p[1], 1) as u8 as i32
    }
    fn pw(&self, off: i32) -> i32 {
        let p = self.ptr(0x3456);
        self.read((p[0] + off) & 65535, p[1], 2) as u16 as i16 as i32
    }
    fn sp(&mut self, off: i32, v: i32) {
        let p = self.ptr(0x3456);
        self.write((p[0] + off) & 65535, p[1], v as u8 as u32, 1)
    }
}
fn ticks(h: &mut dyn Host) -> Result<u32> {
    Ok(h.call("get_ticks", &[])?.long())
}
fn copy(h: &mut dyn Host, src: i32, dst: i32, count: i32) -> Result<()> {
    h.int_call("struct_copy", &[src, h.ds(), dst, STACK, count])?;
    Ok(())
}
fn word(h: &dyn Host, off: i32, seg: i32) -> i32 {
    h.read(off & 65535, seg, 2) as u16 as i16 as i32
}
fn put_word(h: &mut dyn Host, off: i32, seg: i32, n: i32) {
    h.write(off & 65535, seg, n as u16 as u32, 2)
}
fn rect(h: &dyn Host, off: i32, seg: i32) -> [i32; 4] {
    std::array::from_fn(|i| word(h, off + i as i32 * 2, seg))
}
fn put_rect(h: &mut dyn Host, off: i32, seg: i32, r: [i32; 4]) {
    for (i, n) in r.into_iter().enumerate() {
        put_word(h, off + i as i32 * 2, seg, n)
    }
}
fn capacity(model: i32) -> i32 {
    model / 2 * 3 + i32::from(model & 0xFE == 0) + 3
}
fn double(program: i32) -> i32 {
    i32::from(matches!((program - 1) & 15, 2 | 6 | 7 | 10))
}
fn name(h: &dyn Host, crew: i32) -> [i32; 2] {
    let p = h.ptr(0x3456);
    [(p[0] + ((crew - 1) << 8) + 0x102) & 65535, p[1]]
}
fn exit(h: &mut dyn Host) {
    h.sw(PREVIOUS, h.w(STATION));
    h.sw(STATION, -1)
}

pub fn invoke(h: &mut dyn Host, routine: &str, a: &[i32], bp: i32) -> Option<Result<Reply>> {
    let n = |i: usize| a.get(i).copied().unwrap_or(0);
    let result = match routine {
        "dispatch_ship_stations" => dispatch(h),
        "handle_station_key" | "maybe_crew_record" => crew_key(h),
        "assign_station" => assign(h),
        "choose_station" => choose(h),
        "is_double_program" => Ok(double(n(0))),
        "matching_program_is_loaded" => Ok(if n(0) < 0 {
            0
        } else {
            i32::from((0..10).any(|i| h.pb(0xAC + i) > 15 && h.pb(0xAC + n(0)) & 15 == h.pb(0xAC + i) & 15))
        }),
        "repair_interval" => Ok(repair_interval(h, n(0), n(1))),
        "healing_interval" => Ok(healing_interval(h, n(0), n(1))),
        "station_menu" => menu(h, a),
        "pilot" | "pilot_station" => pilot(h),
        "navigation" | "computer_station" => computer::run(h, n(0), bp),
        "engineering" | "engineer_station" => engineer(h, n(0), bp),
        "crew" | "medical_station" => medical(h, n(0), bp),
        "communications" | "communications_station" => communications(h),
        "draw_pilot" | "draw_pilot_station" => render::pilot(h, bp),
        "draw_engineer_station" => render::engineer(h, bp),
        "draw_medical_station" => render::medical(h, n(0), n(1), n(2), bp),
        "draw_computer_station" => render::computer(h, n(0), n(1), bp),
        "redraw_communications" => render::communications(h, n(0), bp),
        "identify_encounter" => identify::run(h, bp),
        "inventory_screen" | "crew_record" => inventory::run(h, n(0)),
        "draw_inventory_panel" => inventory::panel(h, n(0), n(1)),
        "draw_inventory_detail_panel" => inventory::detail(h, n(0), n(1), n(2)),
        "outline_station_item" => {
            let r = rect(h, n(0) + n(2) * 8, n(1));
            if h.bars(None) {
                h.int_call("draw_rectangle", &[r[0], r[1], r[0] + r[2] - 1, r[1] + 1, n(3)])
                    .and_then(|_| h.int_call("draw_rectangle", &[r[0], r[1] + r[3] - 1, r[0] + r[2] - 1, r[1] + r[3], n(3)]))
            } else {
                h.int_call("draw_rectangle", &[r[0], r[1], r[0] + r[2] - 1, r[1] + r[3], n(3)])
            }
        }
        "remap_numeric_glyphs" => {
            for i in 0..65536 {
                let at = (n(0) + i) & 65535;
                let ch = h.read(at, n(1), 1);
                if ch == 0 {
                    break;
                }
                let v = if ch == 47 {
                    175
                } else if (48..=57).contains(&ch) {
                    (ch as i32 + n(2)) as u8 as u32
                } else {
                    continue;
                };
                h.write(at, n(1), v, 1)
            }
            return Some(Ok(Reply {
                ax: n(0) as u16,
                dx: n(1) as u16,
            }));
        }
        _ => return support::invoke(h, routine, a),
    };
    Some(result.map(Reply::int))
}

fn dispatch(h: &mut dyn Host) -> Result<i32> {
    let wanted = h.w(PREVIOUS);
    let station = if wanted == -1 {
        let who = h.pb(1) + 1;
        let slot = (0..12).find(|&i| h.pb(0xA0 + i) == who).unwrap_or(12);
        if slot > 6 {
            17
        } else {
            slot + 10
        }
    } else {
        wanted
    };
    h.sw(STATION, station);
    while h.w(STATION) != -1 {
        match h.w(STATION) {
            10 => {
                h.int_call("pilot", &[])?;
            }
            11 | 12 => {
                h.int_call("gun", &[h.w(STATION) - 11])?;
            }
            13 => {
                h.int_call("navigation", &[3])?;
            }
            14 => {
                h.int_call("engineering", &[4])?;
            }
            15 => {
                h.int_call("communications", &[])?;
            }
            16 => {
                if h.pb(0xA6) == 0 {
                    return Err("AF17: unoccupied station 16 repeats without calls".into());
                }
                h.int_call("crew", &[6])?;
            }
            17 => {
                h.int_call("crew", &[7])?;
            }
            _ => exit(h),
        }
    }
    Ok(0)
}

fn crew_key(h: &mut dyn Host) -> Result<i32> {
    if h.b(KEY) == 110 {
        h.int_call("mouse_command", &[0x63A, h.ds()])?;
    }
    if (59..=63).contains(&h.b(KEY)) {
        if h.crew_records_available() {h.int_call("crew_record", &[h.b(KEY) - 59])?;}
        h.int_call("hide_cursor", &[])?;
        h.sb(KEY, 0);
        Ok(1)
    } else {
        Ok(0)
    }
}
fn choose(h: &mut dyn Host) -> Result<i32> {
    h.sw(TACTICAL_DRAWN, 0);
    let dest = h.int_call("station_menu", &[0x355, h.ds(), 8, h.w(STATION) - 10, 12, 8, 0x19BA, h.ds(), 1])?;
    let manned = dest != -1 && if dest == 7 { (7..12).any(|i| h.pb(0xA0 + i) != 0) } else { h.pb(0xA0 + dest) != 0 };
    if dest != -1 && !manned {
        h.unmanned(dest);
    }
    Ok(if manned { dest } else { -1 })
}
fn assign(h: &mut dyn Host) -> Result<i32> {
    h.sw(TACTICAL_DRAWN, 0);
    let source = loop {
        let v = h.int_call("station_menu", &[0x355, h.ds(), 8, h.w(STATION) - 10, 6, 8, 0x19A8, h.ds(), 1])?;
        if v == -1 {
            return Ok(0);
        }
        if h.pb(0xA0 + v) != 0 {
            break v;
        }
    };
    let mut dest = h.int_call(
        "station_menu",
        &[0x355, h.ds(), 8, source, h.pb(0xA0 + source), 8, 0x19B1, h.ds(), 1],
    )?;
    if dest == -1 {
        return Ok(0);
    }
    for slot in [source, dest] {
        match slot {
            6 => h.sw(0x34BA, -1),
            4 => {
                for i in 1..5 {
                    h.sw(0x34BA + 2 * i, -1)
                }
            }
            3 => {
                let at = 0xAC + h.w(0x34C4);
                h.sp(at, h.pb(at) - 64);
                h.sw(0x34C4, -1)
            }
            _ => (),
        }
    }
    if dest == 7 {
        dest = (8..12).find(|&i| h.pb(0xA0 + i) == 0).unwrap_or(12);
        if dest == 12 {
            return Ok(0);
        }
    }
    let a = h.pb(0xA0 + source);
    let b = h.pb(0xA0 + dest);
    h.sp(0xA0 + source, b);
    h.sp(0xA0 + dest, a);
    if h.pb(0xA0 + source) - 1 == h.w(0x34BA) {
        h.sw(0x34BA, -1)
    }
    h.sw(STATION, if dest > 7 { 17 } else { dest + 10 });
    Ok(0)
}
fn repair_interval(h: &dyn Host, component: i32, crew: i32) -> i32 {
    let at = crew << 8;
    let mut el = h.pb(at + 0x138);
    let mut me = h.pb(at + 0x155);
    let mut half = h.pb(at + 0x13C) / 2;
    match component {
        0 => {
            el /= 2;
            me = 0
        }
        1 => {
            el = 0;
            half = 0
        }
        2 | 3 => {
            me /= 2;
            el = 0
        }
        _ => (),
    }
    ((((5 - h.pb(at + 0x13E)) * 3 - el - me - half) * 18 + 540) as i16 as i32).max(0)
}
fn healing_interval(h: &dyn Host, patient: i32, medic: i32) -> i32 {
    let p = patient << 8;
    let m = medic << 8;
    let rating = (h.pb(m + 0x160) + h.pb(m + 0x12B)) / 3 + h.pb(m + 0x156) + (h.pb(m + 0x11A) + h.pb(m + 0x119)) / 4;
    (112 - rating * 8 - (h.pb(p + 0x116) + h.pb(p + 0x118)) / 4 * 2).max(0) * 7 + 840
}

fn menu(h: &mut dyn Host, a: &[i32]) -> Result<i32> {
    if a.len() != 9 {
        return Err("1C1C: expected nine arguments".into());
    }
    let (off, seg, count) = (a[0], a[1], a[2]);
    if !(1..=32).contains(&count) {
        return Err("1C1C: invalid rectangle count".into());
    }
    let mut selected = if a[3] < 0 || a[3] >= count { 0 } else { a[3] };
    let on = if h.w(0x2796) == 2 { 0 } else { a[4] };
    let bg = a[5];
    for i in 0..count {
        for j in 0..8 {
            let v = h.read((off + i * 8 + j) & 65535, seg, 1);
            h.sb(0x48D3 + i * 9 + j, v as i32)
        }
        h.sb(0x48D3 + i * 9 + 8, i + 1)
    }
    h.sb(0x48D3 + count * 9 + 8, 0);
    h.int_call("hide_cursor", &[])?;
    h.int_call("select_draw_page", &[])?;
    h.int_call("outline_station_item", &[off, seg, selected, on])?;
    let mut active = true;
    let mut through = false;
    let mut deadline = ticks(h)?.wrapping_add(9);
    h.decision_wait("1C1C",Some((count as usize,selected as i16)),a);
    loop {
        match h.take_switch() { Some(n) if n < count => h.sb(KEY, 59 + n), _ => h.sb(KEY, 0) }
        h.int_call("show_cursor", &[])?;
        while h.b(KEY) == 0 {
            h.int_call("update_cursor", &[])?;
            if a[8] != 0 {
                h.int_call("poll_tasks", &[])?;
            } else {
                let v = h.int_call("update_encounter_countdown", &[h.w(0x1CB), 1])?;
                h.sw(0x1CB, v);
                if v == 0 {
                    return Ok(-2);
                }
            }
            if ticks(h)? > deadline {
                h.int_call("hide_cursor", &[])?;
                h.int_call("select_draw_page", &[])?;
                deadline = ticks(h)?.wrapping_add(9);
                h.int_call("outline_station_item", &[off, seg, selected, if active { bg } else { on }])?;
                active = !active;
                h.int_call("show_cursor", &[])?;
            }
        }
        h.int_call("hide_cursor", &[])?;
        h.int_call("select_draw_page", &[])?;
        let key = h.b(KEY);
        if key == 110 {
            h.int_call("mouse_command", &[0x48D3, h.ds()])?;
            if h.b(KEY) == 110 {
                if a[8] != 0 && h.click_through() {
                    if active {
                        h.int_call("outline_station_item", &[off, seg, selected, bg])?;
                    }
                    selected = -1;
                    through = true;
                    break;
                }
                h.sb(KEY, 0);
                continue;
            }
            if active {
                h.int_call("outline_station_item", &[off, seg, selected, bg])?;
            }
            selected = h.b(KEY) - 1;
            h.sb(KEY, 0);
            h.sb(0x486A, 0);
            break;
        }
        if key == 28 {
            if active {
                h.int_call("outline_station_item", &[off, seg, selected, bg])?;
            }
            break;
        }
        if matches!(key, 75 | 72 | 77 | 80) {
            if active {
                h.int_call("outline_station_item", &[off, seg, selected, bg])?;
            }
            selected = (selected + if key == 75 || key == 72 { -1 } else { 1 }).rem_euclid(count);
            h.int_call("outline_station_item", &[off, seg, selected, on])?;
            active = true;
            deadline = ticks(h)?.wrapping_add(9);
        } else if (59..=68).contains(&key) && key - 59 < count {
            if active {
                h.int_call("outline_station_item", &[off, seg, selected, bg])?;
            }
            selected = key - 59;
            break;
        } else if key == 1 {
            if active {
                h.int_call("outline_station_item", &[off, seg, selected, bg])?;
            }
            selected = -1;
            break;
        } else if a[6] != 0 || a[7] != 0 {
            let p = h.call("strchr", &[a[6], a[7], h.b(0x27A5 + key)])?;
            if (p.ax != 0 || p.dx != 0) && h.read(p.ax as i32, p.dx as i32, 1) != 0 {
                if active {
                    h.int_call("outline_station_item", &[off, seg, selected, bg])?;
                }
                selected = (p.ax as i32 - a[6]) as i16 as i32;
                break;
            }
        }
    }
    h.int_call("back_buffer", &[])?;
    h.sb(KEY, if through { 110 } else { 0 });
    Ok(selected)
}

fn engineer(h: &mut dyn Host, station: i32, bp: i32) -> Result<i32> {
    let boxes = bp - 36;
    copy(h, 0x8B6, boxes, 32)?;
    h.int_call("draw_engineer_station", &[])?;
    let mut active = h.w(STATION) - 10;
    while active == station {
        h.int_call("show_cursor", &[])?;
        h.decision_wait("engineer",None,&[station,active]);
        let mut sign = h.repair_sign();
        while h.b(KEY) == 0 {
            if h.int_call("run_tasks", &[1])? != 0 || h.repair_sign() != sign {
                h.int_call("draw_engineer_station", &[])?;
                sign = h.repair_sign();
            }
        }
        h.int_call("hide_cursor", &[])?;
        if h.int_call("handle_station_key", &[])? != 0 {
            h.int_call("draw_engineer_station", &[])?;
            continue;
        }
        match h.int_call("translate_station_key", &[])? {
            27 => {
                exit(h);
                active = -1
            }
            65 => {
                h.int_call("assign_station", &[])?;
                h.int_call("redraw_active_station", &[])?;
                if h.w(STATION) - 10 == active {
                    h.int_call("draw_engineer_station", &[])?;
                }
                active = h.w(STATION) - 10
            }
            88 => {
                h.int_call("show_tactical_map", &[])?;
            }
            86 => {
                let dest = h.int_call("choose_station", &[])?;
                if dest != -1 {
                    active = dest;
                    h.sw(STATION, dest + 10)
                }
            }
            82 if h.pb(0xA4) != 0 => {
                let product = h.click_through();
                h.bars(Some(product));
                let component = loop {
                    let i = h.int_call("station_menu", &[boxes, STACK, 4, 0, 6, 8, 0x1671, h.ds(), 1])?;
                    if i == -1 {
                        h.int_call("draw_engineer_station", &[])?;
                        break -1;
                    }
                    if h.pb(0xC2 + i) != 0 {
                        break i;
                    }
                };
                h.bars(Some(false));
                if component != -1 {
                    for i in 1..5 {
                        h.sw(0x34BA + i * 2, -1)
                    }
                    let i = component + 1;
                    h.sw(0x34BA + i * 2, 1);
                    let interval = h.int_call("repair_interval", &[component, h.pb(0xA0 + component) - 1])?;
                    h.sw(0x349A + i * 2, interval);
                    let deadline = ticks(h)?.wrapping_add(h.w(0x349A + i * 2) as u32);
                    h.sd(0x345E + i * 4, deadline);
                    h.int_call("draw_engineer_station", &[])?;
                }
            }
            _ => (),
        }
    }
    Ok(0)
}
fn medical(h: &mut dyn Host, station: i32, bp: i32) -> Result<i32> {
    let boxes = bp - 52;
    let slots = bp - 20;
    copy(h, 0x978, boxes, 32)?;
    let mut count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
    let mut active = h.w(STATION) - 10;
    while active == station {
        h.int_call("show_cursor", &[])?;
        h.decision_wait("medical",None,&[station,active,count]);
        while h.b(KEY) == 0 {
            if h.int_call("run_tasks", &[0])? != 0 {
                count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
            }
        }
        h.int_call("hide_cursor", &[])?;
        if h.int_call("handle_station_key", &[])? != 0 {
            count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
            continue;
        }
        let cmd = h.int_call("translate_station_key", &[])?;
        match cmd {
            27 => {
                exit(h);
                active = -1
            }
            65 => {
                h.int_call("assign_station", &[])?;
                if h.w(STATION) - 10 == active {
                    count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
                }
                active = h.w(STATION) - 10
            }
            88 => {
                h.int_call("show_tactical_map", &[])?;
            }
            86 => {
                let d = h.int_call("choose_station", &[])?;
                if d != -1 {
                    active = d;
                    h.sw(STATION, d + 10)
                }
            }
            _ if (cmd == 72 && h.pb(0xA6) != 0 && count != 0) || (cmd == 82 && h.pb(0xA7) == 0 && count != 0) => {
                let selected = h.int_call(
                    "station_menu",
                    &[boxes, STACK, count, 0, 6, 8, if cmd == 72 { 0x16BD } else { 0x16C6 }, h.ds(), 1],
                )?;
                if selected == -1 {
                    count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
                    continue;
                }
                let slot = word(h, slots + selected * 2, STACK);
                let patient = h.pb(0xA0 + slot) - 1;
                let current = h.pb((patient << 8) + 0x127);
                if cmd == 72 {
                    if current == h.pb((patient << 8) + 0x125) {
                        continue;
                    }
                    h.sw(0x34BA, patient);
                    let interval = h.int_call("healing_interval", &[patient, h.pb(0xA6) - 1])?;
                    h.sw(0x349A, interval);
                    let deadline = ticks(h)?.wrapping_add(h.w(0x349A) as u32);
                    h.sd(0x345E, deadline)
                } else {
                    if current <= h.pb((patient << 8) + 0x126) {
                        continue;
                    }
                    h.sp(0xA7, patient + 1);
                    if patient == h.w(0x34BA) {
                        h.sw(0x34BA, -1)
                    }
                    h.sp(0xA0 + slot, 0);
                    active -= 1
                }
                count = h.int_call("draw_medical_station", &[station, slots, STACK])?;
            }
            _ => (),
        }
    }
    Ok(0)
}

fn pilot(h: &mut dyn Host) -> Result<i32> {
    let mut deadline = 0;
    h.int_call("draw_pilot", &[])?;
    let mut mode = h.w(STATION) - 10;
    while mode == 0 {
        h.sb(KEY, 0);
        h.sb(PENDING, 0);
        h.int_call("show_cursor", &[])?;
        h.decision_wait("pilot",None,&[mode,deadline as i32]);
        while h.b(KEY) == 0 {
            h.int_call("snapshot_clock", &[])?;
            if h.int_call("run_tasks", &[1])? != 0 {
                h.int_call("draw_pilot", &[])?;
            }
            if h.b(PENDING) != 0 {
                h.sb(KEY, h.b(PENDING))
            }
            let thrust = h.w(0x351);
            if thrust != 0 && ticks(h)? > deadline {
                deadline = ticks(h)?.wrapping_add(((3 - thrust.abs()) * 36) as i16 as u32);
                let (r, col) = (h.w(0x1C5), h.w(0x1C7));
                h.sb(0x3882 + r * 15 + col, 0);
                let direction = if thrust > 0 { 1 } else { -1 };
                let delta = match h.w(0x34B8) {
                    90 => (1, 0),
                    91 => (1, -1),
                    92 => (0, -1),
                    93 => (-1, -1),
                    94 => (-1, 0),
                    95 => (-1, 1),
                    96 => (0, 1),
                    97 => (1, 1),
                    _ => (0, 0),
                };
                if delta.0 != 0 {
                    h.sw(0x1C5, r + direction * delta.0)
                }
                if delta.1 != 0 {
                    h.sw(0x1C7, col + direction * delta.1)
                }
                for coordinate in [0x1C5, 0x1C7] {
                    let value = h.w(coordinate);
                    if value < 0 {
                        h.sw(coordinate, 0)
                    } else if value > 9 {
                        h.sw(coordinate, 9)
                    }
                }
                if h.b(0x3882 + h.w(0x1C5) * 15 + h.w(0x1C7)) != 0 {
                    h.sw(0x1C5, r);
                    h.sw(0x1C7, col)
                }
                h.sb(0x3882 + h.w(0x1C5) * 15 + h.w(0x1C7), 255);
                h.sw(TACTICAL_DRAWN, 0);
            }
        }
        h.int_call("hide_cursor", &[])?;
        if h.int_call("handle_station_key", &[])? != 0 {
            h.int_call("draw_pilot", &[])?;
            if h.click_through() {
                continue;
            }
        }
        let mut cmd = h.int_call("translate_station_key", &[])?;
        match cmd {
            27 => {
                exit(h);
                mode = -1
            }
            65 => {
                h.int_call("assign_station", &[])?;
                if h.w(STATION) - 10 == mode {
                    h.int_call("draw_pilot", &[])?;
                }
                mode = h.w(STATION) - 10
            }
            88 => {
                h.int_call("show_tactical_map", &[])?;
            }
            _ => {
                if cmd == 86 {
                    let dest = h.int_call("choose_station", &[])?;
                    if dest != -1 {
                        mode = dest;
                        h.sw(STATION, dest + 10);
                        continue;
                    }
                    cmd = 1
                }
                if cmd == 1 || cmd == 2 {
                    if h.pb(0xFC) == 0 {
                        h.sw(0x343, 0);
                        continue;
                    }
                    if h.w(0x1C3) < 1 || h.w(0x351) == if cmd == 1 { 2 } else { -2 } {
                        continue;
                    }
                    h.sw(0x351, h.w(0x351) + if cmd == 1 { 1 } else { -1 });
                    h.sw(0x343, i32::from(h.w(0x351) != 0));
                    if cmd == 1 {
                        h.int_call("back_buffer", &[])?;
                    }
                    h.int_call("draw_pilot", &[])?;
                    if h.int_call("redraw_tactical", &[1])? != 0 {
                        mode = -1
                    }
                } else if (cmd == 3 || cmd == 4) && h.w(0x1C3) >= 1 && h.pb(0xC3) < 100 {
                    let heading = h.w(0x34B8) + if cmd == 3 { 1 } else { -1 };
                    h.sw(
                        0x34B8,
                        if heading < 90 {
                            97
                        } else if heading > 97 {
                            90
                        } else {
                            heading
                        },
                    );
                    h.sw(TACTICAL_DRAWN, 0)
                }
            }
        }
    }
    Ok(0)
}
fn communications(h: &mut dyn Host) -> Result<i32> {
    let (mut mode, mut station, mut identifying) = (1, 15, false);
    h.int_call("redraw_communications", &[mode])?;
    while station == 15 {
        h.int_call("show_cursor", &[])?;
        h.decision_wait("communications",None,&[mode,station,identifying as i32]);
        while h.b(KEY) == 0 {
            h.int_call("run_tasks", &[0])?;
        }
        h.int_call("hide_cursor", &[])?;
        if h.int_call("handle_station_key", &[])? != 0 {
            h.int_call("redraw_communications", &[mode])?;
            continue;
        }
        let mut cmd = h.int_call("translate_station_key", &[])?;
        if cmd == 27 {
            if !identifying {
                exit(h);
                station = -1;
                continue;
            }
            identifying = false;
            cmd = 13
        }
        if cmd == 13 {
            mode = -mode;
            h.int_call("redraw_communications", &[mode])?;
        } else if (cmd == 1 && mode >= 2) || (cmd == 2 && (1..=7).contains(&mode)) {
            h.int_call("select_draw_page", &[])?;
            let p = h.ptr(0x2792);
            h.int_call(
                "copy_rect",
                &[p[0], p[1], mode * 8 + 119, 1, 9, 1, p[0], p[1], (mode - 1) * 8 + 119, 1],
            )?;
            mode += if cmd == 1 { -1 } else { 1 };
            h.int_call("set_text_position", &[(mode - 1) * 8 + 119, 8])?;
            h.int_call("draw_character", &[5])?;
        } else if cmd == 65 {
            h.int_call("assign_station", &[])?;
            if h.w(STATION) - 10 == station {
                h.int_call("redraw_communications", &[mode])?;
            }
            station = h.w(STATION) - 10
        } else if cmd == 88 && h.w(0x1C3) >= 1 {
            h.int_call("identify_encounter", &[])?;
            mode = -mode;
            identifying = true
        } else if cmd == 86 {
            let d = h.int_call("choose_station", &[])?;
            if d != -1 {
                station = d;
                h.sw(STATION, d + 10)
            }
        }
    }
    Ok(0)
}
