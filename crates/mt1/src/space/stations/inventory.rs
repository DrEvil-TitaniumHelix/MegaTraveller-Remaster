use super::*;
fn setptr(off: i32, p: [i32; 2], h: &mut dyn Host) {
    h.sw(off, p[0]);
    h.sw(off + 2, p[1])
}
pub(super) fn panel(h: &mut dyn Host, character: i32, refresh: i32) -> Result<i32> {
    h.int_call("hide_cursor", &[])?;
    h.int_call("fill_rectangle", &[53, 0, 10, 320, 0])?;
    setptr(
        0x1D53,
        [if h.w(0x4724) != 0 && h.w(0x46F2) == 0 { 0x24D4 } else { 0x24E0 }, h.ds()],
        h,
    );
    let first = h.int_call("inventory_item_category", &[character, 21])?;
    setptr(0x1D1D, [if first == 2 { 0x24FB } else { 0x24EC }, h.ds()], h);
    let second = h.int_call("inventory_item_category", &[character, 22])?;
    setptr(0x1D26, [if second == 2 { 0x251B } else { 0x250A }, h.ds()], h);
    h.int_call("select_shapes", &h.ptr(0x46EE))?;
    h.int_call("draw_shape", &[0, 54, 0])?;
    h.int_call("draw_labels", &[0x1D07, h.ds(), 0, 0])?;
    h.int_call(
        "draw_portrait",
        &[character, h.pb((character << 8) + 0x122), h.pb((character << 8) + 0x123), 58, 98],
    )?;
    h.int_call("draw_career", &[character, 57, 126])?;
    h.int_call("set_text_position", &[65, 126])?;
    h.int_call("draw_text", &name(h, character + 1))?;
    h.int_call("draw_character_inventory", &[character])?;
    h.int_call("draw_character_skills", &[character])?;
    h.int_call("set_palette_color", &[10, h.b(0x169 + character)])?;
    if refresh != 0 {
        h.int_call("flip_display", &[])?;
    }
    Ok(0)
}
fn deselect(h: &mut dyn Host, character: i32) -> Result<()> {
    h.int_call("highlight_inventory_slot", &[character, h.w(0x4724) - 1, 0])?;
    h.int_call("draw_inventory_slot", &[character, h.w(0x4724) - 1])?;
    h.int_call("draw_inventory_action", &[1])?;
    Ok(())
}
pub(super) fn run(h: &mut dyn Host, mut character: i32) -> Result<i32> {
    if !h.crew_records_available() {return Ok(0)}
    if h.pb((character << 8) + 0x127) == 0 {
        return Ok(0);
    }
    h.int_call("hide_cursor", &[])?;
    h.sw(0x46F6, 0);
    h.sw(0x46F8, 0);
    h.int_call("set_cursor_mode", &[0])?;
    h.sb(0x1E80, 25);
    h.int_call("set_viewport", &[53, 0, 147, 40])?;
    let p = h.call("load_layout", &[0x258E, h.ds()])?;
    setptr(0x46EE, [p.ax as i32, p.dx as i32], h);
    h.int_call("draw_inventory_panel", &[character, 1])?;
    h.int_call("select_draw_page", &[])?;
    h.int_call("highlight_inventory_action", &[h.w(0x2032)])?;
    loop {
        h.int_call("select_draw_page", &[])?;
        let mut key = h.int_call("read_translated_key", &[])? & 255;
        if key == 110 {
            h.int_call("mouse_command", &[0x1E78, h.ds()])?;
            if h.b(KEY) == 110 {
                h.sb(KEY, 0);
                continue;
            }
            key = h.b(KEY)
        }
        if (2..=9).contains(&key) {
            key -= 2;
            if key == h.w(0x2032) {
                key = 47
            } else {
                h.int_call("unhighlight_inventory_action", &[h.w(0x2032)])?;
                h.sw(0x2032, key);
                h.int_call("highlight_inventory_action", &[h.w(0x2032)])?;
                continue;
            }
        }
        if (65..=68).contains(&key) {
            if h.w(0x4724) != 0 {
                h.int_call("equip_inventory_item", &[character, h.w(0x4724) - 1, (key - 65) % 2])?;
            } else {
                h.int_call("unequip_inventory_item", &[character, key - 65])?;
            }
            continue;
        }
        if (150..=168).contains(&key) {
            key -= 150;
            if h.int_call("inventory_slot_marker", &[character, key])? != 0 {
                key = (key - 1) & 255
            }
            if h.w(0x4724) == 0 {
                if h.int_call("inventory_slot_empty", &[character, key])? == 0 {
                    h.sw(0x4724, key + 1);
                    h.int_call("highlight_inventory_slot", &[character, key, 10])?;
                    h.int_call("draw_inventory_action", &[0])?;
                }
            } else if h.w(0x4724) - 1 == key {
                deselect(h, character)?;
                h.sw(0x4724, 0)
            } else if h.int_call("can_swap_inventory_slots", &[character, key])? != 0 {
                h.int_call("swap_inventory_slots", &[character, key])?;
            }
            continue;
        }
        match key {
            1 => break,
            59..=63 => {
                let other = key - 59;
                if h.pb((other << 8) + 0x127) == 0 {
                    continue;
                }
                if other == character {
                    break;
                }
                if h.w(0x4724) != 0 {
                    h.int_call("transfer_selected_item", &[character, other])?;
                }
                character = other;
                h.sw(0x46F6, 0);
                h.sw(0x46F8, 0);
                h.int_call("back_buffer", &[])?;
                h.int_call("draw_inventory_panel", &[character, 1])?;
                h.int_call("select_draw_page", &[])?;
                h.int_call("highlight_inventory_action", &[h.w(0x2032)])?;
            }
            72 | 80 => {
                h.int_call("unhighlight_inventory_action", &[h.w(0x2032)])?;
                h.sw(0x2032, (h.w(0x2032) + if key == 72 { -1 } else { 1 }).rem_euclid(8));
                h.int_call("highlight_inventory_action", &[h.w(0x2032)])?;
            }
            28 | 47 => {
                h.int_call("show_inventory_action_details", &[character, h.w(0x2032)])?;
            }
            16 => {
                h.int_call("unequip_inventory_item", &[character, -1])?;
            }
            25 => {
                if h.w(0x4724) != 0 {
                    deselect(h, character)?;
                }
                h.int_call("select_inventory_slot", &[character, 1])?;
            }
            50 => {
                if h.w(0x4724) != 0 {
                    h.int_call("select_inventory_slot", &[character, 0])?;
                }
            }
            22 => {
                h.int_call("equip_inventory_item", &[character, -1, -1])?;
            }
            73 | 81 => {
                h.int_call(if key == 73 { "first_skill_page" } else { "next_skill_page" }, &[character])?;
                h.int_call("highlight_inventory_action", &[h.w(0x2032)])?;
            }
            _ => (),
        }
    }
    h.int_call("release_buffer", &h.ptr(0x46EE))?;
    h.int_call("select_shapes", &h.ptr(0x4622))?;
    h.sw(0x4724, 0);
    h.int_call("set_viewport", &[0, 0, 200, 40])?;
    Ok(0)
}

fn pointer_call(name: &str, args: &[i32], h: &mut dyn Host) -> Result<[i32; 2]> {
    let p = h.call(name, args)?;
    Ok([p.ax as i32, p.dx as i32])
}
fn pos(y: i32, x: i32, h: &mut dyn Host) -> Result<i32> {
    h.int_call("set_text_position", &[y, x])
}
fn text(p: [i32; 2], h: &mut dyn Host) -> Result<i32> {
    h.int_call("draw_text", &p)
}
fn literal(off: i32, h: &mut dyn Host) -> Result<i32> {
    text([off, h.ds()], h)
}
fn number(n: i32, shift: i32, h: &mut dyn Host) -> Result<i32> {
    let p = pointer_call("number_to_string", &[n], h)?;
    let p = pointer_call("shift_string", &[p[0], p[1], shift], h)?;
    text(p, h)
}
fn longnum(low: i32, high: i32, h: &mut dyn Host) -> Result<i32> {
    let p = pointer_call("long_to_string", &[low, high], h)?;
    let p = pointer_call("shift_string", &[p[0], p[1], 128], h)?;
    text(p, h)
}
fn labels(off: i32, row: i32, h: &mut dyn Host) -> Result<i32> {
    h.int_call("draw_labels", &[off, h.ds(), row, 184])
}
fn byte(h: &dyn Host, p: [i32; 2], offset: i32) -> i32 {
    h.read((p[0] + offset) & 65535, p[1], 1) as u8 as i32
}
fn unsigned(h: &dyn Host, p: [i32; 2], offset: i32) -> i32 {
    h.read((p[0] + offset) & 65535, p[1], 2) as u16 as i32
}

pub(super) fn detail(h: &mut dyn Host, character: i32, action: i32, icon: i32) -> Result<i32> {
    let row = ((action + 1) * 8 + 76) as i16 as i32;
    let rec = character << 8;
    match icon {
        14 => {
            setptr(0x1C7B, name(h, character + 1), h);
            let social = h.pb(rec + 0x11B);
            setptr(0x1CF9, if social >= 11 { h.ptr(0x1A44 + (social - 11) * 4) } else { [0, 0] }, h);
            labels(0x1C77, row, h)?;
            h.int_call("set_color", &[0])?;
            pos(row + 20, 229, h)?;
            let p = h.ptr(0x3456);
            h.int_call("draw_attributes", &[(p[0] + rec + 0x11C) & 65535, p[1], 1])?;
            h.int_call("set_color", &[10])?;
            pos(row + 11, 211, h)?;
            h.int_call("draw_attributes", &[(p[0] + rec + 0x116) & 65535, p[1], 0])?;
            pos(row + 20, 213, h)?;
            h.int_call("draw_attributes", &[(p[0] + rec + 0x116) & 65535, p[1], 1])?;
            pos(row + 95, 251, h)?;
            number((h.pb(rec + 0x124) - 18) / 4, 128, h)?;
            pos(row + 70, 254, h)?;
            longnum(h.pw(rec + 0x1FE), h.pw(rec + 0x200), h)?;
            pos(row + 77, 254, h)?;
            longnum(h.pw(0x88), h.pw(0x8A), h)?;
            pos(row + 102, 261, h)?;
            if h.pb(rec + 0x129) != 0 {
                number(h.pb(rec + 0x129) * 20, 128, h)?;
                h.int_call("draw_character", &[175])?;
                literal(0x252C, h)?;
            } else {
                literal(0x2530, h)?;
            }
            pos(row + 36, 277, h)?;
            number(h.pb(rec + 0x125) - h.pb(rec + 0x126), 128, h)?;
            h.int_call("draw_character", &[175])?;
            number(h.pb(rec + 0x126), 128, h)?;
            h.int_call("draw_career", &[character, row + 87, 187])?;
            h.int_call("draw_portrait", &[character, h.pb(rec + 0x122), h.pb(rec + 0x123), row + 97, 189])?;
            let maximum = h.int_call("weight_limit", &[character])?;
            let weight = h.int_call("carried_weight", &[character])?;
            h.int_call("set_color", &[if weight > maximum { 6 } else { 10 }])?;
            pos(row + 20, 258, h)?;
            let weight = h.int_call("carried_weight", &[character])?;
            number(weight / 10, 128, h)?;
            h.int_call("draw_character", &[46])?;
            let weight = h.int_call("carried_weight", &[character])?;
            number(weight - weight / 10 * 10, 128, h)?;
            literal(0x2535, h)?;
            h.int_call("set_color", &[0])?;
            pos(row + 26, 258, h)?;
            let weight = h.int_call("weight_limit", &[character])?;
            number(weight / 10, 128, h)?;
            literal(0x253A, h)?;
            h.int_call("select_shapes", &h.ptr(0x46EE))?;
            if h.pb(rec + 0x12A) & 1 != 0 {
                h.int_call("draw_shape", &[22, 93, 268])?;
            }
        }
        15 => {
            if h.int_call("inventory_slot_empty", &[character, 19])? != 0 {
                setptr(0x1DE3, [0x2530, h.ds()], h)
            } else {
                let p = pointer_call("get_inventory_definition", &[character, 19], h)?;
                setptr(0x1DE3, p, h);
                let item = h.int_call("get_inventory_item_type", &[character, 19])?;
                h.int_call("draw_item_icon", &[item, row + 26, 23])?;
                let amount = h.int_call("draw_value_bar", &[row + 57, 211, 3, 10, h.pb(rec + 0x128), 45, 99])?;
                pos(row + 53, 187, h)?;
                h.int_call("set_color", &[10])?;
                number(amount, 128, h)?;
            }
            let (hp, maximum, threshold) = (h.pb(rec + 0x127), h.pb(rec + 0x125), h.pb(rec + 0x126));
            let desc = if hp == 0 {
                0x2541
            } else if hp <= threshold {
                0x2546
            } else if hp == maximum {
                0x254C
            } else if maximum - (maximum - threshold) / 2 >= hp {
                0x2554
            } else {
                0x2559
            };
            setptr(0x1DD1, [desc, h.ds()], h);
            h.int_call("draw_value_bar", &[row + 21, 185, 4, 10, hp, maximum, 125])?;
            let value = h.int_call("scaled_bar_value", &[threshold, maximum, 125])?;
            h.int_call("fill_rectangle", &[row + 21, value + 185, 4, 2, 6])?;
            labels(0x1DBB, row, h)?;
            h.int_call("set_color", &[0])?;
            pos(row + 1, 222, h)?;
            number(maximum - threshold, 128, h)?;
            h.int_call("draw_character", &[175])?;
            number(threshold, 128, h)?;
            let (value, i) = if hp - threshold <= 0 {
                (0, hp)
            } else {
                (hp - threshold, threshold)
            };
            pos(row + 10, 222, h)?;
            h.int_call("set_color", &[10])?;
            number(value, 128, h)?;
            h.int_call("set_color", &[0])?;
            h.int_call("draw_character", &[175])?;
            number(i, 128, h)?;
        }
        16 | 17 => {
            let slot = if action == 7 { h.w(0x4724) - 1 } else { action + 19 };
            let definition = pointer_call("get_inventory_definition", &[character, slot], h)?;
            let instance = pointer_call("get_inventory_instance", &[character, slot], h)?;
            let item = h.int_call("get_inventory_item_type", &[character, slot])?;
            h.int_call("draw_item_icon", &[item, row + 1, 23])?;
            if icon == 16 {
                let value = h.int_call("draw_weapon_capacity", &[character, slot, row + 26, 186])?;
                if value == 0 {
                    setptr(0x1E22, [0, 0], h);
                    setptr(0x1E2B, [0x23E6, h.ds()], h);
                    setptr(0x1E46, [0x23E6, h.ds()], h)
                } else {
                    setptr(0x1E2B, [if value == 64 { 0x24B7 } else { 0x255E }, h.ds()], h)
                }
                setptr(0x1E34, definition, h);
                let p = if value != 0 {
                    pointer_call("number_to_string", &[byte(h, instance, 1)], h)?
                } else {
                    [0x23E6, h.ds()]
                };
                setptr(0x1E3D, p, h);
                labels(0x1E1E, row, h)?;
                setptr(0x1E22, [0x24AD, h.ds()], h);
                setptr(0x1E46, [0x24BC, h.ds()], h);
            } else {
                h.int_call("set_color", &[6])?;
                pos(row + 17, 187, h)?;
                text(definition, h)?;
                pos(row + 33, 187, h)?;
                let p = pointer_call("format_tenths", &[unsigned(h, definition, 31), 0x2562, h.ds()], h)?;
                text(p, h)?;
                pos(row + 43, 187, h)?;
                let p = pointer_call("long_to_string", &[unsigned(h, definition, 33), unsigned(h, definition, 35)], h)?;
                text(p, h)?;
                literal(0x256D, h)?;
                match unsigned(h, definition, 29) as i16 {
                    2 => {
                        if h.int_call("draw_weapon_capacity", &[character, slot, row + 25, 189])? != 0 {
                            pos(row + 3, 236, h)?;
                            h.int_call("set_color", &[6])?;
                            literal(0x2576, h)?;
                            number(byte(h, instance, 1), 128, h)?;
                        }
                    }
                    9 => {
                        h.int_call("fill_rectangle", &[row + 27, 191, 3, 100, 8])?;
                        h.int_call(
                            "draw_value_bar",
                            &[row + 27, 191, 3, 6, byte(h, instance, 1), byte(h, definition, 39), 100],
                        )?;
                    }
                    _ => (),
                }
            }
        }
        18 => {
            let mut value = 0;
            h.int_call("set_color", &[6])?;
            for slot in [21, 22] {
                value = (value + h.int_call("draw_equipped_weapon", &[character, slot, row + value * 6])?) as i16 as i32
            }
            for slot in 0..19 {
                value = (value + h.int_call("draw_equipped_weapon", &[character, slot, row + value * 6])?) as i16 as i32;
                if value > 11 {
                    break;
                }
            }
        }
        19 => {
            if h.int_call("inventory_slot_empty", &[character, 20])? != 0 {
                setptr(0x1DFE, [0x2530, h.ds()], h);
                setptr(0x1E07, [0x2580, h.ds()], h)
            } else {
                let definition = pointer_call("get_inventory_definition", &[character, 20], h)?;
                let instance = pointer_call("get_inventory_instance", &[character, 20], h)?;
                setptr(0x1DFE, definition, h);
                let p = pointer_call("format_tenths", &[unsigned(h, definition, 31), 0x2562, h.ds()], h)?;
                setptr(0x1E07, p, h);
                let item = h.int_call("get_inventory_item_type", &[character, 20])?;
                h.int_call("draw_item_icon", &[item, row + 1, 23])?;
                let maximum = (byte(h, definition, 39) & 15) * 20 + 20;
                let amount =
                    byte(h, instance, 1) * 20 + h.int_call("scaled_bar_value", &[byte(h, instance, 2), byte(h, definition, 40), 20])?;
                h.int_call("draw_value_bar", &[row + 37, 185, 4, 10, amount, maximum, 120])?;
            }
            labels(0x1DFA, row, h)?;
        }
        20 => {
            h.int_call("set_color", &[10])?;
            for i in 0..3 {
                pos(row + 12, i * 22 + 198, h)?;
                number(h.pb(rec + 0x1CB + i), 160, h)?;
            }
            labels(0x1E54, row, h)?;
        }
        _ => (),
    }
    Ok(0)
}
