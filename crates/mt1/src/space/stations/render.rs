use super::*;
fn lp(labels: i32, index: i32, p: [i32; 2], h: &mut dyn Host) {
    for (i, v) in p.into_iter().enumerate() {
        put_word(h, labels + index * 9 + 4 + i as i32 * 2, STACK, v)
    }
}
fn ptr(h: &dyn Host, at: i32) -> [i32; 2] {
    [word(h, at, STACK) & 65535, word(h, at + 2, STACK) & 65535]
}
fn palette(h: &mut dyn Host, crew: i32) -> Result<i32> {
    h.int_call("set_palette_color", &[10, h.b(if crew != 0 { 0x168 + crew } else { 0x16F })])
}
fn format(dest: i32, fmt: i32, a: &[i32], h: &mut dyn Host) -> Result<i32> {
    let mut args = vec![dest, STACK, fmt, h.ds()];
    args.extend_from_slice(a);
    h.int_call("sprintf", &args)
}
fn remap(h: &mut dyn Host, at: i32, delta: i32) -> Result<i32> {
    h.int_call("remap_numeric_glyphs", &[at, STACK, delta])
}
pub(super) fn pilot(h: &mut dyn Host, bp: i32) -> Result<i32> {
    let (labels, components, status, hull, pilot, speed) = (bp - 480, bp - 336, bp - 320, bp - 120, bp - 80, bp - 40);
    copy(h, 0x7B3, labels, 144)?;
    copy(h, 0x843, components, 16)?;
    h.sw(TACTICAL_DRAWN, 0);
    h.int_call("back_buffer", &[])?;
    h.int_call("redraw_active_station", &[])?;
    h.int_call("draw_layout", &[h.w(STATION), 83, 0])?;
    let crew = h.pb(0xA0);
    if crew != 0 {
        format(pilot, 0x15AD, &name(h, crew), h)?;
        lp(labels, 0, [pilot, STACK], h)
    }
    for i in 0..4 {
        let at = status + i * 40;
        let p = ptr(h, components + i * 4);
        format(at, 0x15B7, &[p[0], p[1], (100 - h.pb(0xC2 + i)).max(0)], h)?;
        remap(h, at, 128)?;
        lp(labels, i + 8, [at, STACK], h)
    }
    let drive = match (h.pb(0xC1) & 3).min(2) {
        0 => 0x15BC,
        1 => 0x15CA,
        _ => 0x15D6,
    };
    lp(labels, 12, [drive, h.ds()], h);
    let health = (100 - h.pb(0xC6)).max(0);
    format(hull, 0x15E2, &[health], h)?;
    lp(labels, 13, [hull, STACK], h);
    h.write(labels + 14 * 9 + 8, STACK, if h.w(0x351) < 0 { 2 } else { 9 }, 1);
    format(speed, 0x15E5, &[h.w(0x351).abs()], h)?;
    remap(h, speed, 160)?;
    lp(labels, 14, [speed, STACK], h);
    h.int_call("draw_labels", &[labels, STACK, 82, 0])?;
    h.int_call("fill_rect", &[153, 207, 6, health, if h.w(0x2796) == 2 { 6 } else { 9 }])?;
    if h.pb(0xFC) != 0 {
        h.int_call("fill_rect", &[175, 176, 5, h.pb(0xFC), 9])?;
    }
    palette(h, crew)?;
    h.int_call("redraw_tactical", &[105])?;
    h.int_call("flip_with_cursor", &[])?;
    Ok(0)
}
pub(super) fn status_rows(h: &mut dyn Host, bp: i32) -> Result<()> {
    let text = bp - 64;
    let width = |h: &dyn Host, t: &[u8]| t.iter().map(|&b| h.b(0x288A + b as i32)).sum::<i32>();
    let names: Vec<Vec<u8>> = (0..4).map(|i| {
        let at = 0x853 + (i + 3) * 9 + 4;
        let (off, seg) = (h.w(at) & 65535, h.w(at + 2) & 65535);
        (0..8).map(|k| h.read(off + k, seg, 1) as u8).take_while(|&b| b != 0).collect()
    }).collect();
    let space = h.b(0x288A + 32);
    let colon = names.iter().map(|n| width(h, n)).max().unwrap_or(0) + space;
    let end = colon + h.b(0x288A + 58) + space + width(h, &[0xB1, 0xB0, 0xB0]);
    for (i, name) in names.into_iter().enumerate() {
        let at = 0x853 + (i as i32 + 3) * 9;
        let (y, x) = (h.w(at) + 82, h.w(at + 2));
        let number: Vec<u8> = (100 - h.pb(0xC2 + i as i32)).max(0).to_string().bytes().map(|b| b + 128).collect();
        h.int_call("set_color", &[h.b(at + 8)])?;
        for (dx, piece) in [(0, name), (colon, b":".to_vec()), (end - width(h, &number), number)] {
            for (k, b) in piece.into_iter().chain([0]).enumerate() { h.write(text + k as i32, STACK, b as u32, 1) }
            h.int_call("set_text_position", &[y, x + dx])?;
            h.int_call("draw_text", &[text, STACK])?;
        }
    }
    Ok(())
}
pub(super) fn engineer(h: &mut dyn Host, bp: i32) -> Result<i32> {
    let (labels, status, hull, number, engineer) = (bp - 500, bp - 400, bp - 200, bp - 100, bp - 50);
    copy(h, 0x853, labels, 99)?;
    h.int_call("back_buffer", &[])?;
    h.int_call("redraw_active_station", &[])?;
    h.sw(TACTICAL_DRAWN, 0);
    for i in [5, 6] {
        let p = ptr(h, labels + i * 9 + 4);
        h.int_call("remap_numeric_glyphs", &[p[0], p[1], 128])?;
    }
    let crew = h.pb(0xA4);
    palette(h, crew)?;
    if crew != 0 {
        format(engineer, 0x162A, &name(h, crew), h)?;
        lp(labels, 0, [engineer, STACK], h)
    }
    if let Some(sign) = h.repair_sign() {
        for (i, b) in sign.iter().chain(&[0]).enumerate() { h.write(engineer + i as i32, STACK, *b as u32, 1) }
        remap(h, engineer, 128)?;
        lp(labels, 0, [engineer, STACK], h)
    }
    let drive = match (h.pb(0xC1) & 3).min(2) {
        0 => 0x1637,
        1 => 0x1645,
        _ => 0x1654,
    };
    lp(labels, 9, [drive, h.ds()], h);
    h.int_call("draw_layout", &[h.w(STATION), 83, 0])?;
    for i in 0..4 {
        let mut color = h.pb(0xA0 + (i + 3) % 4);
        if color == 0 {
            color = 6
        }
        if h.w(0x2796) == 2 {
            color = 0
        }
        if h.w(0x34BA + (i + 1) * 2) != -1 {
            color = crew
        }
        let remaining = 100 - h.pb(0xC2 + i);
        if remaining != 0 {
            h.int_call("fill_rect", &[115 + i * 8, 201, 5, remaining, color])?;
        }
        format(number, 0x1663, &[remaining.max(0)], h)?;
        remap(h, number, 128)?;
        let at = status + i * 50;
        let p = ptr(h, labels + (i + 3) * 9 + 4);
        format(at, 0x1666, &[p[0], p[1], number, STACK], h)?;
        lp(labels, i + 3, if h.click_through() { [0, 0] } else { [at, STACK] }, h);
    }
    h.int_call("fill_rect", &[179, 207, 5, 100 - h.pb(0xC6), 6])?;
    format(hull, 0x166E, &[(100 - h.pb(0xC6)).max(0)], h)?;
    lp(labels, 7, [hull, STACK], h);
    h.int_call("draw_labels", &[labels, STACK, 82, 0])?;
    if h.click_through() {
        status_rows(h, bp - 500)?;
    }
    h.sw(TACTICAL_DRAWN, 0);
    h.int_call("redraw_tactical", &[105])?;
    h.int_call("flip_with_cursor", &[])?;
    Ok(0)
}
pub(super) fn medical(h: &mut dyn Host, _station: i32, off: i32, seg: i32, bp: i32) -> Result<i32> {
    let (labels, caption, details) = (bp - 938, bp - 770, bp - 700);
    copy(h, 0x8D6, labels, 162)?;
    h.int_call("back_buffer", &[])?;
    h.int_call("redraw_active_station", &[])?;
    h.int_call("draw_layout", &[h.w(STATION), 83, 0])?;
    let mut count = 0;
    for slot in 8..12 {
        let patient = h.pb(0xA0 + slot);
        if patient > 0 && patient < 10 {
            let who = patient - 1;
            let p = name(h, patient);
            h.int_call("fill_rect", &[count * 14 + 113, 124, 13, 25, patient])?;
            h.int_call(
                "draw_portrait",
                &[
                    who,
                    h.read(p[0] + 0x20, p[1], 1) as i32,
                    h.read(p[0] + 0x21, p[1], 1) as i32,
                    count * 14 + 114,
                    124,
                ],
            )?;
            lp(labels, count + 2, p, h);
            let current = h.read(p[0] + 0x25, p[1], 1) as i32;
            let unconscious = h.read(p[0] + 0x24, p[1], 1) as i32;
            let mut at = details + count * 140;
            if current - unconscious > 0 {
                format(at, 0x16A0, &[current - unconscious, unconscious], h)?;
            } else {
                format(at, 0x16A6, &[current], h)?;
            }
            remap(h, at, 128)?;
            lp(labels, count + 9, [at, STACK], h);
            h.write(labels + (count + 9) * 9 + 8, STACK, patient as u32, 1);
            at += 70;
            format(at, 0x16AB, &[h.read(p[0] + 0x23, p[1], 1) as i32 - unconscious, unconscious], h)?;
            remap(h, at, 128)?;
            lp(labels, count + 13, [at, STACK], h);
            put_word(h, off + count * 2, seg, slot);
            count += 1;
        }
    }
    let crew = h.pb(0xA6);
    palette(h, crew)?;
    if count == 0 {
        for i in [6, 7, 8] {
            lp(labels, i, [0, 0], h)
        }
    } else if crew == 0 {
        lp(labels, 6, [0, 0], h)
    }
    if crew != 0 {
        format(caption, 0x16B1, &name(h, crew), h)?;
        lp(labels, 0, [caption, STACK], h)
    }
    h.int_call("draw_labels", &[labels, STACK, 82, 0])?;
    h.int_call("redraw_tactical", &[105])?;
    h.int_call("flip_with_cursor", &[])?;
    Ok(count)
}
pub(super) fn computer(h: &mut dyn Host, off: i32, seg: i32, bp: i32) -> Result<i32> {
    let (labels, models, caption) = (bp - 148, bp - 76, bp - 52);
    copy(h, 0x5DE, labels, 72)?;
    copy(h, 0x626, models, 20)?;
    let (load, run) = if h.w(0x2796) == 2 { (6, 0) } else { (4, 9) };
    h.int_call("back_buffer", &[])?;
    h.int_call("redraw_active_station", &[])?;
    h.sw(TACTICAL_DRAWN, 0);
    let model = h.pb(0xC0);
    if model > 4 {
        return Err("E60: saved computer model outside 0..4".into());
    }
    lp(labels, 6, ptr(h, models + 4 * model), h);
    h.int_call("draw_layout", &[h.w(STATION), 83, 0])?;
    h.int_call("draw_layout", &[model / 2 + 30, 83, 0])?;
    for row in 0..=capacity(model) {
        let value = h.pb(0xAC + row);
        if value != 0 && (value - 1) & 192 != 0 {
            let p = value - 1;
            let double = h.int_call("is_double_program", &[p + 1])? != 0;
            for (flag, y, height, color) in [
                (32, 126, 5, load),
                (128, 132, 16, run),
                (64, h.w(0x39D) + 132, 16 - h.w(0x39D), run),
            ] {
                if p & flag != 0 {
                    h.int_call("fill_rect", &[y, row * 16 + 145, height, 15, color])?;
                    if double {
                        h.int_call("fill_rect", &[y, row * 16 + 161, height, 15, color])?;
                    }
                }
            }
            let r = rect(h, off + row * 8, seg);
            h.int_call("draw_station_icon", &[p & 15, r[0] + 1, r[1] >> 3, 1])?;
        }
    }
    let crew = h.pb(0xA3);
    if crew != 0 {
        format(caption, 0x14FD, &name(h, crew), h)?;
        lp(labels, 0, [caption, STACK], h)
    } else {
        for i in [1, 2, 3] {
            lp(labels, i, [0, 0], h)
        }
    }
    h.int_call("draw_labels", &[labels, STACK, 0, 0])?;
    palette(h, crew)?;
    h.int_call("redraw_tactical", &[105])?;
    h.int_call("flip_with_cursor", &[])?;
    Ok(0)
}
fn world_line(text: i32, index: i32, fmt: i32, h: &mut dyn Host) -> Result<()> {
    let p = h.ptr(0x4A1 + index * 4);
    let q = h.ptr(0x3C1 + index * 4);
    format(text, fmt, &[p[0], p[1], q[0], q[1]], h)?;
    h.int_call("format_columns", &[text, STACK, 128])?;
    h.int_call("draw_text", &[text, STACK])?;
    Ok(())
}
pub(super) fn communications(h: &mut dyn Host, mode: i32, bp: i32) -> Result<i32> {
    let (labels, text) = (bp - 118, bp - 54);
    copy(h, 0xF92, labels, 63)?;
    h.int_call("back_buffer", &[])?;
    let sector = h.int_call("direction_sector", &[h.w(0x37D6), h.w(0x387C)])?;
    h.int_call("draw_station_frame", &[])?;
    h.int_call("draw_image", &[15, 83, 0])?;
    let crew = h.pb(0xA5);
    h.int_call("set_station_colors", &[10, h.b(0x168 + crew)])?;
    h.int_call("set_text_color", &[0])?;
    h.int_call("set_text_position", &[91, 132])?;
    format(text, 0x1986, &name(h, crew), h)?;
    h.int_call("draw_text", &[text, STACK])?;
    h.int_call("select_font", &[1])?;
    h.int_call("set_text_color", &[6])?;
    h.int_call("set_text_origin", &[258, 258])?;
    let mut line = 0;
    for i in 0..h.w(0x36D0).max(0) {
        let world = h.w(0x3578 + i * 2);
        if world != 0 && h.int_call("direction_sector", &[h.w(0x3B79 + i * 2), h.w(0x3C19 + i * 2)])? == sector {
            h.int_call("set_text_position", &[line * 8 + 119, 207])?;
            world_line(text, world + h.w(0x39F + h.pb(6) * 2) - 1, 0x1990, h)?;
            line += 1
        }
    }
    if mode == 0 {
        lp(labels, 0, [0, 0], h);
        lp(labels, 2, [0, 0], h)
    } else if mode > 0 {
        lp(labels, 1, [0, 0], h);
        lp(labels, 2, [0, 0], h);
        h.int_call("set_text_origin", &[20, 90])?;
        for i in 0..8 {
            h.int_call("set_text_position", &[i * 8 + 119, 8])?;
            format(text, 0x1996, &h.ptr(0x481 + i * 4), h)?;
            h.int_call("draw_text", &[text, STACK])?;
        }
    } else {
        lp(labels, 1, [0, 0], h);
        lp(labels, 0, [0, 0], h);
        h.int_call("set_text_origin", &[20, 110])?;
        let begin = h.w(0x39F + (-mode - 1) * 2);
        let end = h.w(0x39F + (-mode) * 2);
        for i in begin..end {
            h.int_call("set_text_position", &[(i - begin) * 8 + 119, 8])?;
            world_line(text, i, 0x19A1, h)?;
        }
    }
    h.int_call("finish_text", &[1])?;
    h.int_call("draw_labels", &[labels, STACK, 83, 0])?;
    h.int_call("flip_buffers", &[])?;
    Ok(0)
}
