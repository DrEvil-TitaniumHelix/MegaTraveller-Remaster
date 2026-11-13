use super::*;
fn emit(h: &mut dyn Host, text: i32, y: &mut i32, fmt: i32, args: &[i32], remap: bool) -> Result<()> {
    let mut a = vec![text, STACK, fmt, h.ds()];
    a.extend_from_slice(args);
    h.int_call("sprintf", &a)?;
    h.int_call("set_text_position", &[*y, 8])?;
    if remap {
        h.int_call("remap_numeric_glyphs", &[text, STACK, 128])?;
    }
    h.int_call("draw_text", &[text, STACK])?;
    *y += 8;
    Ok(())
}
pub(super) fn run(h: &mut dyn Host, bp: i32) -> Result<i32> {
    let weapons = bp - 62;
    let text = bp - 50;
    copy(h, 0xF86, weapons, 12)?;
    if h.w(0x1C3) < 1 {
        return Ok(0);
    }
    let ship = h.int_call("show_tactical_map", &[])?;
    if ship == -1 {
        return Ok(0);
    }
    h.int_call("redraw_communications", &[0])?;
    let crew = h.pb(0xA5) - 1;
    let mut skill = 30;
    for (k, w) in [(12, 5), (55, 3), (63, 3), (20, 1)] {
        skill += h.int_call("improve_skill", &[crew, k])? * w
    }
    skill = skill as i16 as i32;
    let enemy = 0x3CB9 + ship * 30;
    let mut info = [0; 8];
    if h.int_call("rand", &[])? % 100 > skill {
        for k in [-12, -55, -63, -20] {
            h.int_call("improve_skill", &[crew, k])?;
        }
        info[0] = h.w(enemy + 14) - 1;
        info[1] = (h.w(enemy) - 110) / 10;
        info[2] = h.w(enemy + 2);
        info[3] = h.w(enemy + 16);
        for i in 0..4 {
            info[i + 4] = h.w(enemy + 4 + i as i32 * 2)
        }
    } else {
        info[0] = ((h.int_call("rand", &[])? % 8 + h.w(enemy + 14) - 5) as i16 as i32).clamp(0, 12);
        info[1] = (h.w(enemy) - 110) / 10;
        info[2] = (h.int_call("rand", &[])? % 7 + h.w(enemy + 2)) as i16 as i32;
        info[3] = ((h.int_call("rand", &[])? % 1000 + h.w(enemy + 16) - 500) as i16 as i32).max(0);
        for item in &mut info[4..] {
            *item = h.int_call("rand", &[])? % 9
        }
    }
    h.int_call("select_draw_page", &[])?;
    h.int_call("select_font", &[1])?;
    h.int_call("set_color", &[10])?;
    let enc = 0x3AB0 + (h.w(0x1C3) - 1) * 10;
    let mut destination = 0;
    if h.w(enc + 6) == -1 {
        destination = -1
    } else {
        for i in 0..h.w(0x36D0) {
            if h.w(enc + 6) == h.w(0x3B79 + i * 2) && h.w(enc + 8) == h.w(0x3C19 + i * 2) {
                destination = if h.w(0x3578 + i * 2) != 0 { i + 1 } else { 0 };
                break;
            }
        }
    }
    h.int_call("set_text_origin", &[110, 110])?;
    let mut y = 120;
    let p = h.ptr(0x431 + info[0] * 4);
    let q = h.ptr(0x465 + info[1] * 4);
    emit(h, text, &mut y, 0x189A, &[p[0], p[1], q[0], q[1]], false)?;
    h.int_call("set_color", &[6])?;
    emit(h, text, &mut y, 0x18A0, &[info[2]], true)?;
    emit(h, text, &mut y, 0x18B0, &[info[3]], true)?;
    for i in 0..4 {
        if info[i + 4] == 0 {
            continue;
        }
        if info[i + 4] < 0 {
            let di = h.entry_di()? as i32;
            emit(h, text, &mut y, 0x18C0, &[di], true)?;
        } else {
            let at = weapons + 4 * (h.w(enemy + 4 + i as i32 * 2) / 4);
            let p = [word(h, at, STACK) & 65535, word(h, at + 2, STACK) & 65535];
            emit(h, text, &mut y, 0x18DD, &[i as i32 + 1, p[0], p[1]], true)?;
        }
    }
    if destination == 0 {
        emit(h, text, &mut y, 0x18F1, &[], false)?;
    } else if destination == -1 {
        emit(h, text, &mut y, 0x1908, &[], false)?;
    } else {
        let p = h.ptr(0x4A1 + 4 * (h.w(0x3578 + (destination - 1) * 2) + h.w(0x39F + h.pb(6) * 2) - 1));
        emit(h, text, &mut y, 0x1922, &p, false)?;
    }
    h.int_call("finish_text", &[1])?;
    h.int_call("back_buffer", &[])?;
    Ok(0)
}
