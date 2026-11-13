use super::{
    input::dialogue, render,
    state::{Far, Ground, Mem, MAP_PTR, PARTY_PTR},
    walk,
};
const MARKER_SHOWN: u16 = 0x6FB2;
const NEXT_LEADER: u16 = 0x6D2E;
fn i16w(v: i32) -> i32 { v as i16 as i32 }
fn abs16(v: i32) -> i32 { i16w(i16w(v).abs()) }
fn b(g: &Ground, p: Far, o: i32) -> i32 { g.mem.read(p.add(o), 1) as i32 }
fn sb(g: &Ground, p: Far, o: i32) -> i32 { g.mem.read_i(p.add(o), 1) }
fn w(g: &Ground, p: Far, o: i32) -> i32 { g.mem.read_i(p.add(o), 2) }
fn wb(g: &mut Ground, p: Far, o: i32, v: i32) { g.mem.write(p.add(o), v as u32, 1) }
fn ww(g: &mut Ground, p: Far, o: i32, v: i32) { g.mem.write(p.add(o), v as u32, 2) }
fn party(g: &Ground) -> Far { g.mem.pointer(PARTY_PTR) }
fn cell(g: &Ground, row: i32, col: i32) -> Far {
    g.mem.pointer(MAP_PTR).add(row.wrapping_mul(g.mem.map_stride() as i32).wrapping_add(col).wrapping_mul(3))
}
fn pixels(g: &Ground, row: i32, col: i32) -> (i32, i32) {
    (i16w((row - g.mem.view_x() as i32) * 8 + 0x3E), i16w((col - g.mem.view_y() as i32) * 8 + 8))
}
fn random(g: &mut Ground, n: i32) -> i32 { walk::random(g, n as i16) as i32 }

pub fn range_class(row1: i32, col1: i32, row2: i32, col2: i32) -> i32 {
    let distance = i16w(abs16(row1 - row2) + abs16(col1 - col2));
    if distance <= 4 { 2 } else if distance <= 6 { 1 } else if distance >= 14 { -1 } else { 0 }
}
pub fn cell_enterable(g: &Ground, c: Far, dir: i32, flag: i32) -> i32 {
    let t = b(g, c, 1);
    if b(g, c, 0) & 1 != 0 {
        if g.mem.interior_kind() == b'I' {
            if t > 4 && dir < 5 { return 1 }
            let door = t == 0x9f || t == 0xb2 || t == 0xb3;
            return (door || if flag != 0 { (0xc6..0xec).contains(&t) } else { (0xc8..=0xd3).contains(&t) || (0xd6..=0xdb).contains(&t) }) as i32;
        }
        if flag != 0 { return 1 }
        if t > 4 && (dir < 5 || (7..=8).contains(&dir)) { return 1 }
        return ((0x6c..=0x75).contains(&t) || (0x86..=0x8b).contains(&t) || (0xb4..=0xbe).contains(&t) || (0xcb..=0xd9).contains(&t) || (0xdc..=0xeb).contains(&t)) as i32;
    }
    if flag != 0 { (dir != 11) as i32 } else { (t > 0xe && dir < 2 && t != 0x26 && t != 0x27) as i32 }
}
pub fn trace_shot(g: &mut Ground, row: i32, col: i32, tr: &mut i32, tc: &mut i32, mode: i32, power: i32) -> i32 {
    let (vr, vc) = (g.mem.view_x() as i32, g.mem.view_y() as i32);
    if *tr < vr || *tc < vc || i16w(*tr - vr) >= 14 || i16w(*tc - vc) >= 24 { *tr = row; *tc = col; return 1 }
    let (mut dr, mut dc) = (i16w(*tr - row) >> 1, i16w(*tc - col) >> 1);
    if abs16(dr) < 6 && abs16(dc) < 6 { dr = i16w(dr * 2); dc = i16w(dc * 2) }
    let (mut pr, mut pc) = (i16w(row * 8 + 8), i16w(col * 8 + 8));
    render::front(g);
    for _ in 0..26 {
        if pr >> 4 == *tr >> 1 && pc >> 4 == *tc >> 1 { render::back(g); return 0 }
        let c = cell(g, pr >> 4, pc >> 4);
        if mode != -2 && cell_enterable(g, c, b(g, c, 1) / 20, power) == 0 { *tr = i16w(pr >> 3); *tc = i16w(pc >> 3); render::back(g); return 1 }
        pr = i16w(pr + dr); pc = i16w(pc + dc);
        if mode != 0 { render::fill_rect(g, i16w(pr - g.mem.view_x() as i32 * 8 + 0x36), i16w(pc - g.mem.view_y() as i32 * 8), 2, 2, 0) }
    }
    *tr = row; *tc = col; render::back(g); 1
}
fn weapon_effects(kind: i32, default_sound: i32, beam_colour: i32) -> (i32, i32, i32) {
    match kind {
        0xAB => (8, 0, 14), 0xAC => (6, 6, 14), 0xAA => (beam_colour, 0, 10), 0x49 | 0x69 => (-2, 0, 0),
        0 => (-1, 0, 17), _ => (-1, 0, default_sound),
    }
}
fn scatter(g: &mut Ground, tr: &mut i32, tc: &mut i32) {
    let mut row = i16w(*tr + (random(g, 3) - 1) * 2);
    let mut col = i16w(*tc + (random(g, 3) - 1) * 2);
    let (vr, vc) = (g.mem.view_x() as i32, g.mem.view_y() as i32);
    let (dr, dc) = (i16w(row - vr) >> 1, i16w(col - vc) >> 1);
    if dr <= 0 { row = vr } if dc <= 0 { col = vc } if dr >= 6 { row = vr + 12 } if dc >= 11 { col = vc + 22 }
    *tr = i16w(row); *tc = i16w(col);
}
fn trace_weapon(g: &mut Ground, kind: i32, row: i32, col: i32, tr: &mut i32, tc: &mut i32) {
    match kind {
        0x68 | 0x78 | 0xAA | 0xAB | 0xAC => { trace_shot(g, row, col, tr, tc, 0, 0); }
        0x49 | 0x69 => { trace_shot(g, row, col, tr, tc, 0, 1); }
        _ => {}
    }
}
pub fn draw_shot(g: &mut Ground, kind: i32, row: i32, col: i32, tr: i32, tc: i32, colour: i32) -> (i32, i32) { draw_shot_with(g, kind, row, col, tr, tc, 8, colour) }
#[allow(clippy::too_many_arguments)]
pub fn draw_shot_with(g: &mut Ground, kind: i32, row: i32, col: i32, tr: i32, tc: i32, default_sound: i32, colour: i32) -> (i32, i32) {
    let (trail, shadow, sound) = weapon_effects(kind, default_sound, colour);
    let (mut tr, mut tc) = (tr, tc);
    trace_weapon(g, kind, row, col, &mut tr, &mut tc);
    let (sx, sy) = pixels(g, row, col);
    let (ex, ey) = pixels(g, tr, tc);
    if kind != 0 && trail < 0 { trace_shot(g, row, col, &mut tr, &mut tc, trail, 0); }
    if trail > 0 { render::front(g); render::draw_line(g, ex + 1, ey + 1, sx + 1, sy + 1, shadow as u8); render::draw_line(g, ex, ey, sx, sy, trail as u8); render::back(g); }
    if let Some(s) = g.squad.as_mut() { s.p1.fx.shots.push((((row >> 1) as i16, (col >> 1) as i16), ((tr >> 1) as i16, (tc >> 1) as i16), kind as u8)) }
    g.mem.set_shot_sound((sound + 1) as i8);
    (tr, tc)
}
pub fn flash_marker(g: &mut Ground, val: i32, x: i32, y: i32) {
    render::front(g);
    let (sx, sy) = ((x - g.mem.view_x() as i32) * 8 + 0x36, (y - g.mem.view_y() as i32) * 8);
    if sx < 0x36 || sy < 0 || sx >= 0x9e || sy >= 0xb8 { return }
    render::marker(g, sx, sy, val);
    walk::wait_ticks(g, if g.mem.u8(0x6B68) == 0 { 2 } else { 1 });
    render::back(g);
}
#[allow(clippy::too_many_arguments)]
pub fn shot_impact(g: &mut Ground, sx: i32, sy: i32, ex: i32, ey: i32, mut impact: i32, trail: i32, shadow: i32, damage: i32,
                   penetration: i32, target_row: i32, target_col: i32, rubble: i32, target: i32, sound: i32) -> i32 {
    g.mem.set_u8(MARKER_SHOWN, 0);
    if trail > 0 {
        render::front(g);
        render::draw_line(g, ex + 1, ey + 1, sx + 1, sy + 1, shadow as u8);
        render::draw_line(g, ex, ey, sx, sy, trail as u8);
        render::back(g);
    }
    g.mem.set_shot_sound((sound + 1) as i8);
    let (row, col) = (target_row >> 1, target_col >> 1);
    let c = cell(g, row, col);
    if trail == -2 { impact = 6 }
    let mut hit = 0;
    if b(g, c, 2) & 0x3f != 0 {
        if hit_npc(g, row, col, damage, penetration) != 0 {
            npc_dies(g, row, col);
            walk::set_cell_flags(g, i16w(row * 2) as i16, i16w(col * 2) as i16, 0x80);
            g.mem.set_event_id(-1);
            g.mem.set_impact_sound(1);
            let p = party(g); wb(g, p, 0x81, 1);
        } else { g.mem.set_extra_sound((impact + 1) as i8); impact = -1 }
        hit = 1;
    } else {
        let victim = if g.mem.combat() == 0 {
            (i16w(row * 2) == g.mem.x() as i32 && i16w(col * 2) == g.mem.y() as i32).then_some(target)
        } else {
            (0..5).find(|&ch| { let wk = Mem::walker(ch as i16); b(g, wk, 0) != 0 && w(g, wk, 8) >> 1 == row && w(g, wk, 10) >> 1 == col })
        };
        match victim {
            None => flash_marker(g, -1, row * 2, col * 2),
            Some(v) => {
                if take_hit(g, v, penetration, damage) != 2 { g.mem.set_extra_sound((impact + 1) as i8); impact = -1 }
                render::status_panel(g);
                hit = 1;
            }
        }
    }
    if trail == -2 {
        let terrain = b(g, c, 1) / 20;
        let interior = g.mem.interior_kind() == b'I';
        if (!interior && walk::vehicle_terrain(g, c, terrain as i16) != 0) || (interior && terrain < 5) {
            wb(g, c, 0, b(g, c, 0) & !1);
            wb(g, c, 1, (if rubble != 0 { rubble - 1 } else { 0 }) + 0x23);
            g.mem.set_event_id(-1);
        }
    }
    if impact == 6 && g.mem.extra_sound() == 0 { g.mem.set_extra_sound(7) }
    hit
}
pub fn take_hit(g: &mut Ground, character: i32, penetration: i32, mut damage: i32) -> i32 {
    super::squad::note(g, "take_hit", &[character as u16, penetration as u16, damage as u16]);
    let pc = party(g).add((character << 8) + 0x102);
    if g.mem.encounter_mode() != 0 {
        if damage == 255 { damage = 0 }
        let hull = g.mem.vehicle_condition() as i32;
        if hull < damage { damage = hull }
        g.mem.set_vehicle_condition((hull - damage) as u8);
        flash_marker(g, damage, g.mem.x() as i32, g.mem.y() as i32);
        return 0;
    }
    if b(g, pc, 0x25) == 0 { return 0 }
    let knockout = damage == 255;
    let (armour_def, accessory, armour) = (walk::item_def(g, character, 20), walk::item_def(g, character, 19), walk::item_ptr(g, character, 20));
    let null = Far::default();
    if penetration != 100 {
        let wear = i16w(damage * 5);
        let mut rating = if armour != null { b(g, armour, 1) } else { 0 };
        if armour != null && b(g, armour_def, 0x29) != 0 && (accessory == null || b(g, accessory, 0x29) == 0) { rating -= 1 }
        if rating * 4 > penetration { damage = if penetration >= rating { damage >> 1 } else { damage / 10 } }
        if armour != null {
            let minimum = (b(g, armour_def, 0x27) & 0xf0) >> 4;
            if b(g, armour, 1) > minimum {
                let charge = b(g, armour, 2);
                if charge == 0 { wb(g, armour, 1, b(g, armour, 1) - 1); wb(g, armour, 2, b(g, armour_def, 0x28)) }
                else if charge < wear { wb(g, armour, 2, 0) } else { wb(g, armour, 2, charge - wear) }
            }
        }
    }
    let upright = b(g, pc, 0x25) > b(g, pc, 0x24);
    if knockout { damage = if upright { b(g, pc, 0x25) - b(g, pc, 0x24) } else { 0 } }
    else if b(g, pc, 0x25) < damage { damage = b(g, pc, 0x25) }
    if b(g, pc, 0x28) & 0xf0 != 0 { wb(g, pc, 0x28, b(g, pc, 0x28) - 0x10); damage >>= 1 }
    render::front(g);
    if g.mem.combat() != 0 || g.mem.u8(MARKER_SHOWN) == 0 {
        let wk = Mem::walker(character as i16);
        let (row, col) = if g.mem.combat() != 0 { (w(g, wk, 8), w(g, wk, 10)) } else { (g.mem.x() as i32, g.mem.y() as i32) };
        flash_marker(g, damage, row, col);
        g.mem.set_u8(MARKER_SHOWN, 1);
    }
    render::back(g);
    wb(g, pc, 0x25, b(g, pc, 0x25) - damage);
    if b(g, pc, 0x25) == 0 { char_dies(g, character); return 2 }
    if upright && b(g, pc, 0x25) <= b(g, pc, 0x24) {
        let wk = Mem::walker(character as i16);
        render::hp_bar(g, character as u16);
        if character == g.mem.leader() as i32 { let f = first_wounded(g); g.mem.set_i16(NEXT_LEADER, (f + 1) as i16) }
        wb(g, wk, 0x16, 0); wb(g, wk, 5, 0);
        demote_in_order(g, character);
        return 3;
    }
    render::hp_bar(g, character as u16);
    1
}
pub fn char_dies(g: &mut Ground, character: i32) {
    super::squad::note(g, "char_dies", &[character as u16]);
    let p = party(g); let pc = p.add(character << 8);
    if b(g, pc, 0x12A) & 2 != 0 { return }
    wb(g, pc, 0x12A, b(g, pc, 0x12A) | 2);
    wb(g, p, 0, b(g, p, 0) & !(1 << character));
    wb(g, pc, 0x127, 0);
    let funds = g.mem.read(p.add(0x88), 4) as i32;
    g.mem.write(p.add(0x88), funds.wrapping_sub(funds / 5) as u32, 4);
    render::fill_rect(g, 2, character * 0x28 + 7, 0x2e, 0x24, 0);
    render::draw_party_order(g);
    render::status_panel(g);
    walk::play_sound(g, 4);
    for slot in 0..12 { if b(g, p, 0xA0 + slot) == character + 1 { wb(g, p, 0xA0 + slot, 0); break } }
    if b(g, p, 0) == 0 { party_defeated(g) }
    drop_all(g, character);
    if g.mem.combat() != 0 {
        if g.mem.leader() as i32 == character { let f = first_wounded(g); g.mem.set_i16(NEXT_LEADER, (f + 1) as i16) }
        wb(g, Mem::walker(character as i16), 0, 0);
        g.mem.set_redraw_pending(1);
        if g.mem.i16(NEXT_LEADER) != 0 {
            walk::set_leader(g, g.mem.i16(NEXT_LEADER) - 1);
            g.mem.set_i16(NEXT_LEADER, 0);
            if g.mem.combat() != 0 { let v = g.mem.u8(0x111u16.wrapping_add(g.mem.leader() as u16)); render::select_panel(g, 10, v) }
            render::walker_panel(g, false, true);
        }
    }
}
pub fn demote_in_order(g: &mut Ground, character: i32) {
    let p = party(g);
    let previous = (0..12).find(|&s| b(g, p, 0xA0 + s) == character + 1).unwrap_or(8);
    if previous > 8 { return }
    wb(g, p, 0xA0 + previous, 0);
    for slot in 8..12 {
        let occupant = b(g, p, 0xA0 + slot);
        if occupant == 0 { wb(g, p, 0xA0 + slot, character + 1); break }
        let pc = p.add((occupant - 1) << 8);
        if b(g, pc, 0x127) > b(g, pc, 0x126) { wb(g, p, 0xA0 + previous, occupant); wb(g, p, 0xA0 + slot, character + 1); break }
    }
}
pub fn first_wounded(g: &Ground) -> i32 {
    let p = party(g);
    (0..5).find(|&i| b(g, p, (i << 8) + 0x127) > b(g, p, (i << 8) + 0x126)).unwrap_or(0)
}
pub fn skill_roll(g: &mut Ground, c: i32, target: i32, terms: &[(i32, i32, i32)]) -> i32 {
    super::squad::note(g, "skill_roll", &[c as u16, target as u16]);
    let base = party(g).add(c << 8);
    let mut total = random(g, 6) + 1;
    for &(kind, idx, div) in terms {
        let div = if div == 0 { 1 } else { div };
        match kind {
            0 => { total += (div.abs() + b(g, base, idx + 0x12B) - 1) / div; wb(g, base, idx + 0x17B, b(g, base, idx + 0x17B) + 1) }
            1 => total += b(g, base, idx + 0x116) / div,
            2 => total += idx,
            _ => {}
        }
    }
    total += b(g, base, 0x14F) / 2;
    wb(g, base, 0x19F, b(g, base, 0x19F) + 1);
    (i16w(total) >= target) as i32
}
fn fire_fail(g: &Ground, character: i32) -> i32 {
    let _ = (g.mem.leader() as i32 == character, "1B4B:0004 beep: audible only");
    0
}
pub fn party_fire(g: &mut Ground, character: i32) -> i32 {
    super::squad::note(g, "party_fire", &[character as u16]);
    let record = party(g).add(character << 8);
    let wk = Mem::walker(character as i16);
    let (row, col) = (w(g, wk, 8), w(g, wk, 10));
    let slot = b(g, wk, 2) + 21;
    let (obj, def) = (walk::item_ptr(g, character, slot), walk::item_def(g, character, slot));
    let builtin = b(g, wk, 3);
    if builtin != 0 {
        if b(g, record, builtin + 0x1CA) == 0 { return fire_fail(g, character) }
    } else if obj == Far::default() || w(g, def, 0x1D) != 2 || (b(g, def, 0x27) & 8 != 0 && b(g, obj, 2) == 0) {
        return fire_fail(g, character);
    }
    let (mut tr, mut tc) = (w(g, wk, 12), w(g, wk, 14));
    let kind = if builtin != 0 { 0x49 } else { b(g, def, 0x27) };
    let penetration = if builtin != 0 { 2 * builtin + 6 } else { b(g, def, 0x29) };
    let damage = if builtin != 0 { 3 * builtin + 4 } else { b(g, def, 0x2A) };
    let (trail, shadow, sound) = weapon_effects(kind, if builtin != 0 { 0 } else { 8 }, character + 1);
    if kind == 0 && (abs16(tr - row) > 3 || abs16(tc - col) > 3) { return fire_fail(g, character) }
    let distance = range_class(tr, tc, row, col);
    if g.mem.npc_mode() != 0 && trace_shot(g, row, col, &mut tr, &mut tc, 0, (trail == -2) as i32) != 0 {
        g.mem.set_npc_mode(2);
        return fire_fail(g, character);
    }
    tr = w(g, wk, 12); tc = w(g, wk, 14);
    let skill = if builtin != 0 { 16 } else { b(g, def, 0x25) };
    let hit = if g.mem.orders() != 0 { 1 } else { skill_roll(g, character, 5, &[(0, skill, 1), (2, distance, 1)]) };
    if hit == 0 { scatter(g, &mut tr, &mut tc) }
    else if b(g, wk, 5) == 0 {
        let c = cell(g, tr >> 1, tc >> 1);
        wb(g, wk, 5, b(g, c, 2) & 0x3f);
        walk::walker_set_pose(g, wk);
        if g.mem.orders() != 0 { return 1 }
    } else if g.mem.orders() != 0 { return 1 }
    trace_weapon(g, kind, row, col, &mut tr, &mut tc);
    let (sx, sy) = pixels(g, row, col);
    let (ex, ey) = pixels(g, tr, tc);
    if sx == ex && sy == ey { return 1 }
    if builtin != 0 { wb(g, record, builtin + 0x1CA, b(g, record, builtin + 0x1CA) - 1) }
    else if kind & 8 != 0 { wb(g, obj, 2, b(g, obj, 2) - 1) }
    if kind != 0 && trail < 0 { trace_shot(g, row, col, &mut tr, &mut tc, trail, 0); }
    if shot_impact(g, sx, sy, ex, ey, 5, trail, shadow, damage, penetration, tr, tc, builtin, 0, sound) == 0 {
        wb(g, record, 0x17B + skill, b(g, record, 0x17B + skill) + 1);
        wb(g, record, 0x19F, b(g, record, 0x19F) + 1);
    }
    if g.mem.npc_attacking() == 0 { walk::play_pending_sounds(g) }
    render::walker_panel(g, false, true);
    g.mem.set_party_fired(1);
    1
}
pub fn monster_attack(g: &mut Ground, m: Far, mode: i32) -> i32 {
    super::squad::note(g, "monster_attack", &[m.off, m.seg, mode as u16]);
    let p = party(g);
    if b(g, m, 0x1E) == 0 { return 0 }
    let mut target = 0;
    for _ in 0..65536 { target = random(g, 5); if b(g, p, (target << 8) + 0x127) != 0 { break } }
    let (row, col);
    if g.mem.combat() != 0 && w(g, m, 0x2A) != 0 { row = w(g, m, 0x2A); col = w(g, m, 0x2C) }
    else {
        if g.mem.combat() == 0 || (g.mem.individual() == 0 && target == g.mem.leader() as i32) { row = g.mem.x() as i32; col = g.mem.y() as i32 }
        else { let wk = Mem::walker(target as i16); row = w(g, wk, 8); col = w(g, wk, 10) }
        ww(g, m, 0x2A, 0); ww(g, m, 0x2C, 0);
    }
    let (mut tr, mut tc) = (row, col);
    let def = g.mem.pointer(0x6D6D).add(b(g, m, 0x1F) * 0x2E);
    let kind = b(g, def, 0x27);
    let (penetration, damage) = (b(g, def, 0x29), b(g, def, 0x2A));
    let (trail, shadow, sound) = weapon_effects(kind, 11, 9);
    fn clear(g: &mut Ground, m: Far) -> i32 { ww(g, m, 0x2A, 0); ww(g, m, 0x2C, 0); 0 }
    let (row, col) = (w(g, m, 0x20), w(g, m, 0x22));
    if kind == 0 && (abs16(tr - row) > 3 || abs16(tc - col) > 3) { return clear(g, m) }
    let distance = range_class(tr, tc, row, col);
    let (mut pr, mut pc) = (tr, tc);
    if trace_shot(g, row, col, &mut pr, &mut pc, 0, (trail == -2) as i32) != 0 { return clear(g, m) }
    let roll = random(g, 6);
    let hit = i16w(roll + (b(g, m, 10) >> 3) + distance + 1) > 5;
    if !hit { scatter(g, &mut tr, &mut tc) }
    trace_weapon(g, kind, row, col, &mut tr, &mut tc);
    let (sx, sy) = pixels(g, row, col);
    let (ex, ey) = pixels(g, tr, tc);
    if sx == ex && sy == ey { return clear(g, m) }
    if kind != 0 {
        if trail < 0 { trace_shot(g, row, col, &mut tr, &mut tc, trail, 0); }
        wb(g, m, 0x1E, b(g, m, 0x1E) - 1);
    }
    if shot_impact(g, sx, sy, ex, ey, 5, trail, shadow, damage, penetration, tr, tc, 0, target, sound) != 0 { ww(g, m, 0x2A, tr); ww(g, m, 0x2C, tc) }
    else { clear(g, m); }
    npc_reaction(g, m, mode);
    0
}
pub fn npc_reaction(g: &mut Ground, m: Far, mode: i32) {
    if g.mem.panel_flag() == 0 { show_vehicle_pic(g, b(g, m, 9)) }
    let token = w(g, m, 0x16);
    if mode == 0 && token >= 0 {
        let identity = w(g, m, 0);
        if identity == 321 || identity == 221 {
            walk::enter_world(g, (identity / 100 - 2) as u16, token as u16, 0, 0);
            render::redraw(g);
        } else if token != 0 {
            dialogue::set_text_box(g, 3);
            if let Err(e) = dialogue::say_message(g, token as i16, true) { eprintln!("ground: {e}"); std::process::exit(3) }
        }
        ww(g, m, 0x16, -1);
    } else if mode == 1 && g.mem.provoked() != 0 && token >= 0 {
        render::show_message(g, Mem::ds(0xE3F));
        ww(g, m, 0x16, -1);
    }
}
pub fn hit_npc(g: &mut Ground, x: i32, y: i32, mut dmg: i32, range: i32) -> i32 {
    let c = cell(g, x, y);
    let k = b(g, c, 2) & 0x3f;
    if k == 0 { return 0 }
    let n = Mem::npc(k as i16);
    wb(g, n, 0x29, sb(g, n, 0x29) - (dmg << 1));
    wb(g, n, 0x26, 1);
    if sb(g, n, 0x29) <= 0 {
        if b(g, n, 11) > 1 { wb(g, n, 11, b(g, n, 11) - 1) }
        wb(g, n, 0x29, b(g, n, 11) * 10);
    }
    let mut hp = b(g, n, 10);
    if hp == 0xff { dmg = 0 }
    let lvl = b(g, n, 11);
    if dmg == 0xff { dmg = hp >> 1 }
    if range < lvl { dmg /= 10 } else if lvl / 2 + lvl > range { dmg /= 2 }
    render::front(g);
    flash_marker(g, dmg, x * 2, y * 2);
    render::back(g);
    hp -= (dmg + 1) >> 1;
    if b(g, g.mem.pointer(0x5E02), 6) > 8 { g.mem.set_provoked(2) }
    if hp < 0 {
        if b(g, n, 13) & 3 != 0 { g.mem.set_provoked(2) }
        wb(g, n, 10, 0);
        return 1;
    }
    wb(g, n, 10, hp);
    0
}
pub fn npc_dies(g: &mut Ground, x: i32, y: i32) {
    super::squad::note(g, "npc_dies", &[x as u16, y as u16]);
    let c = cell(g, x, y);
    let k = b(g, c, 2) & 0x3f;
    if k == 0 { return }
    let n = Mem::npc(k as i16);
    wb(g, n, 2, 20);
    for drop in [14, 16] {
        if w(g, n, drop) > 0 { let o = walk::new_object(g, w(g, n, drop)); walk::place_object(g, x, y, o); }
    }
    if b(g, n, 13) & 8 != 0 {
        let id = w(g, n, 0);
        let p = party(g);
        wb(g, p, (id >> 3) + 8, b(g, p, (id >> 3) + 8) | (1 << (id & 7)));
    }
    g.mem.set_panel_flag(0);
    if w(g, n, 0) == 0x141 { walk::enter_world(g, 2, 0xbd, 0xbe, 0xbf) }
}
pub fn drop_all(g: &mut Ground, character: i32) {
    super::squad::note(g, "drop_all", &[character as u16]);
    for slot in 0..0x17 {
        if w(g, party(g).add(character << 8), 0x1CE + 2 * slot) > 0 { drop_item(g, character, slot) }
    }
}
fn drop_item(g: &mut Ground, ch: i32, slot: i32) {
    let rec = party(g).add(ch << 8);
    let (x, y) = if g.mem.combat() == 0 || (g.mem.leader() as i32 == ch && g.mem.individual() == 0) { (g.mem.x() as i32, g.mem.y() as i32) }
                 else { let wk = Mem::walker(ch as i16); (w(g, wk, 8), w(g, wk, 10)) };
    if g.mem.outdoors() != 0 {
        let item = w(g, rec, 0x1CE + 2 * slot);
        if walk::place_object(g, x >> 1, y >> 1, item) >= 0 {
            let def = walk::item_def(g, ch, slot);
            if def != Far::default() && b(g, def, 0x26) >= 100 { ww(g, rec, 0x1CE + 2 * (slot + 1), 0) }
            ww(g, rec, 0x1CE + 2 * slot, 0);
        } else {
            let s = walk::ds_string(g, b"You've littered on this planet quite enough already.");
            render::show_message(g, s);
        }
    } else {
        let s = walk::ds_string(g, b"Littering in the Starport area is severly prohibited!");
        render::show_message(g, s);
    }
    g.mem.set_u16(0x6EFC, 0);
}
fn party_defeated(g: &mut Ground) -> ! {
    let s = walk::ds_string(g, b"No original characters are left in your party.");
    walk::game_over(g, s)
}
pub fn tick_party(g: &mut Ground, penetration: i32, damage: i32) {
    g.mem.set_u8(MARKER_SHOWN, 0);
    let (lo, hi) = if g.mem.combat() != 0 { (g.mem.leader() as i32, g.mem.leader() as i32) } else { (0, 4) };
    let mut any = false;
    for i in lo..=hi { if take_hit(g, i, penetration, damage) == 3 { any = true } }
    if any { render::draw_party_order(g) }
    render::status_panel(g);
}
pub fn show_vehicle_pic(g: &mut Ground, v: i32) {
    if g.mem.combat() == 0 || v == 0 { return }
    g.mem.set_u8(0x294, (v - 1) as u8);
    render::vehicle_pic(g, v);
    render::status_panel(g);
    g.mem.set_panel_flag(1);
}
#[cfg(test)]
mod t {
    use super::*;
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    fn ground() -> Ground {
        let mut g = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.set_pointer(PARTY_PTR, Far::new(0, Mem::PARTY_SEG));
        g
    }
    #[test]
    fn range_class_and_skill_roll() {
        assert_eq!(range_class(0, 0, 2, 2), 2); assert_eq!(range_class(0, 0, 3, 3), 1); assert_eq!(range_class(0, 0, 5, 5), 0); assert_eq!(range_class(0, 0, 7, 7), -1);
        assert_eq!(range_class(-32768, 0, 1, 0), -1, "signed-word overflow of the original");
        let mut g = ground();
        g.mem.set_party_u8(7 + 0x12B, 3); g.mem.set_party_u8(0x14F, 4);
        assert_eq!(skill_roll(&mut g, 0, 8, &[(0, 7, 1), (2, 2, 1)]), 1);
        assert_eq!(g.mem.party_u8(7 + 0x17B), 1); assert_eq!(g.mem.party_u8(0x19F), 1);
        assert_eq!(skill_roll(&mut g, 0, 15, &[(0, 7, 1), (2, 2, 1)]), 0);
    }
    #[test]
    fn demote_and_first_wounded() {
        let mut g = ground();
        for (i, v) in [3u8, 1, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0].iter().enumerate() { g.mem.set_party_u8(0xA0 + i as u16, *v) }
        g.mem.set_party_u8((1 << 8) + 0x127, 5); g.mem.set_party_u8((1 << 8) + 0x126, 2);
        assert_eq!(first_wounded(&g), 1);
        demote_in_order(&mut g, 2);
        assert_eq!(g.mem.party_u8(0xA0), 0); assert_eq!(g.mem.party_u8(0xA8), 3);
        demote_in_order(&mut g, 7);
        assert_eq!(g.mem.party_u8(0xA8), 8); assert_eq!(g.mem.party_u8(0xA9), 0);
    }
}
