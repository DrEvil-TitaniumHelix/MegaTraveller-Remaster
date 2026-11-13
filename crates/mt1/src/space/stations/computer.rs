use super::*;
fn select(h: &mut dyn Host, at: i32, n: i32, selected: i32) -> Result<i32> {
    h.int_call("select_rect", &[at, STACK, n, selected, 6, 8, 0, 0])
}
fn draw(h: &mut dyn Host, boxes: i32) -> Result<i32> {
    h.int_call("draw_computer_station", &[boxes, STACK])
}
fn is_double(h: &mut dyn Host, slot: i32) -> Result<bool> {
    Ok(h.int_call("is_double_program", &[h.pb(0xAC + slot)])? != 0)
}
fn mapped(v: &[i32], i: i32) -> Result<i32> {
    v.get(i as usize)
        .copied()
        .ok_or_else(|| format!("121C: selector index {i} outside {} rectangles", v.len()))
}
pub(super) fn run(h: &mut dyn Host, station: i32, bp: i32) -> Result<i32> {
    let (boxes, selectable, loads, program_map, run_capacity) = (bp - 252, bp - 168, bp - 412, bp - 62, bp - 86);
    copy(h, 0x670, boxes, 80)?;
    copy(h, 0x6C0, run_capacity, 10)?;
    let mut model = h.pb(0xC0);
    if model > 4 {
        return Err("121C: saved computer model outside 0..4".into());
    }
    draw(h, boxes)?;
    let mut active = h.w(STATION) - 10;
    while active == station {
        h.int_call("show_cursor", &[])?;
        h.decision_wait("121C",None,&[station,active,model]);
        while h.b(KEY) == 0 {
            if h.int_call("run_tasks", &[5])? != 0 {
                draw(h, boxes)?;
            }
        }
        h.int_call("hide_cursor", &[])?;
        if h.int_call("handle_station_key", &[])? != 0 {
            draw(h, boxes)?;
            continue;
        }
        let cmd = h.int_call("translate_station_key", &[])? & 255;
        match cmd {
            27 => {
                exit(h);
                active = -1
            }
            88 => {
                h.int_call("show_tactical_map", &[])?;
            }
            65 => {
                h.int_call("assign_station", &[])?;
                if h.w(STATION) - 10 == active {
                    draw(h, boxes)?;
                }
                active = h.w(STATION) - 10
            }
            86 => {
                let d = h.int_call("choose_station", &[])?;
                if d != -1 {
                    active = d;
                    h.sw(STATION, d + 10)
                }
            }
            82 | 83 => {
                h.int_call("select_draw_page", &[])?;
                let (mut used, mut units) = (0, 0);
                let mut mapping = Vec::new();
                for i in 0..capacity(model) {
                    let value = h.pb(0xAC + i);
                    if (value & 128 != 0 && value & 32 == 0 && cmd == 82) || (value & 32 != 0 && cmd == 83) {
                        if capacity(model) == units {
                            break;
                        }
                        let mut r = rect(h, boxes + i * 8, STACK);
                        if value & 240 != 0 && is_double(h, i)? {
                            r[3] *= 2;
                            units += 2
                        } else {
                            units += 1
                        }
                        put_rect(h, selectable + mapping.len() as i32 * 8, STACK, r);
                        mapping.push(i);
                    } else if value & 32 != 0 && cmd == 82 {
                        used += if is_double(h, i)? { 2 } else { 1 }
                    }
                    is_double(h, i)?;
                }
                let limit = word(h, run_capacity + model * 2, STACK);
                if (limit <= used && cmd == 82) || mapping.is_empty() {
                    continue;
                }
                let mut dest = -2;
                while dest == -2 {
                    let selected = select(h, selectable, mapping.len() as i32, 0)?;
                    if selected == -1 {
                        dest = -1;
                        break;
                    }
                    dest = mapped(&mapping, selected)?;
                    let value = h.pb(0xAC + dest);
                    if cmd == 83 {
                        if value & 32 != 0 {
                            h.sp(0xAC + dest, value - 32)
                        }
                    } else if value & 128 != 0 {
                        if !is_double(h, dest)? || limit - 1 != used {
                            h.sp(0xAC + dest, value + 32)
                        }
                    } else {
                        dest = -2
                    }
                }
                if dest != -1 {
                    draw(h, boxes)?;
                }
            }
            76 => {
                h.int_call("select_draw_page", &[])?;
                h.int_call("draw_layout", &[18, 88, 5])?;
                h.int_call("setmem", &[program_map, STACK, 20, 0])?;
                let mut mapping = Vec::new();
                for i in 0..20 {
                    let p = h.pb(0xAC + i);
                    if (p - 1) & 192 == 0 && p != 0 {
                        let mut r = rect(h, 0x94 + i * 8, h.ds());
                        r[3] = 16;
                        put_rect(h, loads + mapping.len() as i32 * 8, STACK, r);
                        mapping.push(i);
                        h.int_call("draw_station_icon", &[(p - 1) & 15, r[0] + 1, r[1] >> 3, 0])?;
                    }
                }
                if mapping.is_empty() {
                    continue;
                }
                let mut slots = Vec::new();
                let mut i = 0;
                while i < capacity(model) {
                    slots.push(i);
                    let mut r = rect(h, boxes + i * 8, STACK);
                    if h.pb(0xAC + i) & 240 != 0 && is_double(h, i)? {
                        r[3] *= 2;
                        i += 1
                    }
                    put_rect(h, selectable + (slots.len() as i32 - 1) * 8, STACK, r);
                    i += 1
                }
                let mut selected = -1;
                let mut rows = slots.len() as i32;
                let dest = loop {
                    if selected != -1 {
                        h.int_call("select_draw_page", &[])?;
                        h.int_call("set_text_position", &[101, 32])?;
                        h.int_call("set_color", &[6])?;
                        let p = h.ptr(0x134 + h.pb(0xAC + mapped(&mapping, selected)?) * 4);
                        h.int_call("draw_text", &p)?;
                    }
                    selected = -2;
                    loop {
                        if selected != -2 && h.int_call("matching_program_is_loaded", &[mapped(&mapping, selected)?])? == 0 {
                            break;
                        }
                        selected = select(h, loads, mapping.len() as i32, selected)?;
                        if selected == -1 {
                            break;
                        }
                    }
                    if selected == -1 {
                        break -1;
                    }
                    if is_double(h, mapped(&mapping, selected)?)? && !is_double(h, mapped(&slots, rows - 1)?)? {
                        rows -= 1
                    }
                    h.int_call("select_draw_page", &[])?;
                    h.int_call("set_text_position", &[101, 32])?;
                    h.int_call("set_color", &[0])?;
                    let p = h.ptr(0x134 + h.pb(0xAC + mapped(&mapping, selected)?) * 4);
                    h.int_call("draw_text", &p)?;
                    let dest = select(h, selectable, rows, 0)?;
                    if dest != -1 {
                        break dest;
                    }
                    is_double(h, mapped(&mapping, selected)?)?;
                };
                if selected != -1 {
                    let slot = mapped(&slots, dest)?;
                    let mut dest = dest;
                    for i in 0..20 {
                        if h.pb(0xAC + i) & 240 != 0 && i < slot && is_double(h, i)? {
                            dest += 1
                        }
                        if h.pb(0xAC + i) & 64 != 0 {
                            h.sp(0xAC + i, h.pb(0xAC + i) - 64)
                        }
                    }
                    let source = mapped(&mapping, selected)?;
                    h.sw(0x34C4, source);
                    let skill = h.pb(((h.pb(0xA3) - 1) << 8) + 0x138);
                    h.sw(0x34A4, (130 - skill * 18).max(6));
                    h.sw(0x39D, 15);
                    let old = h.pb(0xAC + dest) & 15;
                    h.sp(0xAC + dest, h.pb(0xAC + source));
                    h.sp(0xAC + source, old);
                    h.sp(0xAC + dest, h.pb(0xAC + dest) + 64);
                    if is_double(h, dest)? {
                        h.sp(0xAC + dest + 1, h.pb(0xAC + dest + 1) & 15)
                    }
                    h.sw(0x34C4, dest);
                }
                draw(h, boxes)?;
            }
            77 => {
                model = (model + 1) % 6;
                draw(h, boxes)?;
            }
            _ => (),
        }
    }
    Ok(0)
}
