use super::attributes::change_attribute;
use super::skills::add_skill;
use super::tables::{menu_args, strs, Cg, A, R};
use crate::ui::CgUi;
use crate::record::{W_AGE, W_CASH, W_ITEMS, W_PENSION, W_RANK, W_SERVICE, W_TAS};

fn cash(ui: &mut impl CgUi) -> u32 { let w = ui.work(); u32::from_le_bytes([w[W_CASH], w[W_CASH + 1], w[W_CASH + 2], w[W_CASH + 3]]) }
fn add_cash(ui: &mut impl CgUi, v: u32) { let c = cash(ui).wrapping_add(v); ui.work()[W_CASH..W_CASH + 4].copy_from_slice(&c.to_le_bytes()) }
fn draw_cash(ui: &mut impl CgUi) { let c = cash(ui) as i32; ui.draw_text_at(0, 16, 140, 8, 5, &c.to_string()) }

pub fn add_item<H: CgUi>(cg: &Cg, ui: &mut H, item: u8) -> usize {
    let w = ui.work();
    for s in (1..=16).rev() { if w[W_ITEMS + s - 1] == 0xFF { w[W_ITEMS + s - 1] = item; return s } }
    cg.message(ui, 0x3016, &[]); 0
}

pub fn benefit_help(cg: &mut Cg, ui: &mut dyn CgUi, table: i16) {
    let old_height = ui.line_height();
    ui.set_line_height(8); ui.set_text_mode(0);
    let saved_window = ui.current_window();
    if cg.skill_window != -1 { ui.close_window(cg.skill_window) }
    let labels = if table == 1 { cg.help_width = 43; cg.help_left = 225; cg.cash_labels.clone() }
    else { cg.help_width = cg.benefit_labels.iter().map(|l| ui.string_width(l)).max().unwrap_or(0) + 6; cg.help_left = 224 - cg.help_width; cg.benefit_labels.clone() };
    cg.skill_window = ui.open_window(116, cg.help_left, 53, cg.help_width + 2, 1, 2, 0);
    ui.select_window(cg.skill_window);
    for (i, l) in labels.iter().enumerate() { let h = ui.line_height(); ui.move_to(h * i as i16 + 1, 1); ui.set_text_spacing(5); ui.text(l) }
    ui.select_window(saved_window); ui.set_line_height(old_height); ui.set_text_spacing(0);
}

pub fn muster_out<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R<u8> {
    let mut state = cg.state(ui);
    let w = ui.work();
    let (service, age, rank) = (w[W_SERVICE] as i8, w[W_AGE] as i8, w[W_RANK]);
    let s = service as u8;
    let terms = (age as i32 - 18) / 4;
    let name = cg.ds.service_name(s);
    if state == 1 {
        let roll = cg.dice(ui, 2);
        if roll == 12 { cg.message(ui, 0x2E98, &[name.as_str().into(), cg.ds.str(if name.ends_with('s') { 0x2E95 } else { 0x2E96 }).into()]) }
        else {
            let target = cg.ds.rule(s, 5) as i16 - (4 - cg.term as i16).max(0);
            if roll > target {
                cg.message(ui, 0x2EEA, &[]);
                cg.menu_style(ui);
                let opts = [cg.ds.pstr(0x3012, 0), cg.ds.str(if terms >= 5 { 0x2EFE } else { 0x2EF3 })];
                if cg.menu(ui, &menu_args(&strs(&opts), &[4, 120, 150, 0, 2, 0, 1, 0, 3], -1))? == 1 { state = 2 }
            } else { cg.message(ui, 0x2F08, &[name.as_str().into(), cg.ds.str(if name.ends_with('s') { 0x2F05 } else { 0x2F06 }).into()]); state = 2 }
        }
    }
    if state != 2 { return Ok(state) }
    let mut cash_bonus = 0u8;
    if terms >= 5 {
        let pension = (terms as i16 * 2000) as u16;
        ui.work()[W_PENSION..W_PENSION + 2].copy_from_slice(&pension.to_le_bytes());
        cg.message(ui, 0x2F59, &[pension.into()]);
        add_cash(ui, (pension.wrapping_add(11) / 12) as u32);
        cash_bonus = 1;
    }
    ui.set_text_color(0); draw_cash(ui);
    let material_bonus = rank / 5;
    if cg.find_skill(32).is_some() || cg.find_skill(63).is_some() { cash_bonus = 1 }
    let mut cash_rolls = 0u8;
    let mut count = (rank as i32 + 1) / 2 + terms;
    cg.message(ui, 0x2F89, &[terms.into(), cg.ds.str(if terms == 1 { 0x2F86 } else { 0x2F87 }).into(), name.as_str().into(), count.into(), cg.ds.str(if count == 1 { 0x2F83 } else { 0x2F84 }).into()]);
    cg.benefit_labels.clear(); cg.cash_labels.clear();
    for table in 0..6usize {
        let b = 0x15EE + 2 * ((material_bonus as usize + table) * 5 + s as usize);
        cg.benefit_labels.push(super::tables::fmt(&cg.ds.str(0x2FCB), &[cg.ds.pstr(0x1598, cg.ds.u8(b + 1) as usize).into(), cg.ds.pstr(0x1598, cg.ds.u8(b) as usize).into()]));
        let c = cg.ds.u32(0x1634 + 4 * ((table + cash_bonus as usize) * 5 + s as usize));
        cg.cash_labels.push(super::tables::fmt(&cg.ds.str(0x2FD0), &[A::L(c as i64)]));
    }
    while count > 0 {
        let options = if cash_rolls >= 3 { 1 } else { 2 };
        cg.message(ui, 0x2FD4, &[count.into()]);
        cg.menu_style(ui);
        cg.skill_window = -1;
        let names = cg.ds.pstrs(0x300E, options);
        let items = strs(&names);
        let m = menu_args(&items, &[4, 100, 175, 0, 2, 0, 1, 0, 3], -1);
        let mut on_change = |ui: &mut dyn CgUi, t: i16| benefit_help(cg, ui, t);
        let r = ui.hmenu_cb(&m, Some(&mut on_change), None);
        let table = cg.esc(r)? & 0xFF;
        ui.set_text_color(0);
        count -= 1;
        let roll = cg.dice(ui, 1) - 1;
        ui.flash(118, cg.help_left + 1, 8, cg.help_width - 4, 4, roll);
        if table == 0 { material_benefit(cg, ui, material_bonus.wrapping_add(roll as u8))? }
        else {
            let c = cg.ds.u32(0x1634 + 4 * ((cash_bonus as usize + roll as usize) * 5 + s as usize));
            add_cash(ui, c);
            cg.message(ui, 0x2FF6, &[cg.cash_labels[roll as usize].as_str().into()]);
            ui.select_window(0); ui.fill_rect(16, 140, 10, 40, 2, 3); ui.set_text_color(0); draw_cash(ui);
            cash_rolls += 1;
        }
        ui.close_window(cg.skill_window);
    }
    cg.benefit_labels.clear(); cg.cash_labels.clear();
    Ok(state)
}

pub fn material_benefit<H: CgUi>(cg: &mut Cg, ui: &mut H, row: u8) -> R {
    let s = ui.work()[W_SERVICE] as i8 as u8;
    let b = 0x15EE + 2 * (row as usize * 5 + s as usize);
    let (kind, value) = (cg.ds.u8(b), cg.ds.u8(b + 1));
    match kind {
        3 | 4 | 5 => { change_attribute(ui, kind, value as i8); cg.message(ui, 0x3052, &[value.into(), cg.ds.skill_name(kind).into()]); return Ok(()) }
        6 => {
            let w = *ui.work();
            if value == 9 && cg.find_item(&w, 2) != 0 {
                cg.message(ui, 0x3085, &[]);
                ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2);
                let opts = cg.ds.pstrs(0x32D6, 2);
                if cg.menu(ui, &menu_args(&strs(&opts), &[4, 100, 130, 0, 2, 0, 1, 0, 3], -1))? == 0 { cg.message(ui, 0x30F2, &[]); add_item(cg, ui, 2); }
                else { add_skill(cg, ui, 90, 1); let level = cg.find_skill(90).map_or(0, |p| cg.skills[p].level); cg.message(ui, 0x3110, &[level.into()]) }
            } else { cg.message(ui, 0x3129, &[cg.ds.pstr(0x1598, value as usize).into()]); add_item(cg, ui, value.wrapping_sub(7)); }
            return Ok(())
        }
        11 => {
            if ui.work()[W_TAS] == 0 { cg.message(ui, 0x312C, &[]); ui.work()[W_TAS] = 1; ui.layout(10, 13, 180); return Ok(()) }
            cg.message(ui, 0x3153, &[]);
        }
        10 => {}
        _ => return Ok(()),
    }
    let has = |cg: &Cg, id: u8| cg.find_skill(id).is_some();
    let mut choices: Vec<(String, u8, Option<u8>)> = vec![(cg.ds.str(0x319D), 46, Some(40))];
    if has(cg, 42) || has(cg, 25) || has(cg, 52) { choices.push((cg.ds.str(0x31A9), 56, None)) }
    if has(cg, 60) { let w = *ui.work(); if cg.find_item(&w, 59) == 0 || ui.rng().next() % 100 < 30 { choices.push((cg.ds.str(0x31B6), 59, Some(60))) } }
    for (skill, text, item) in [(82u8, 0x31C4usize, 48u8), (66, 0x31D2, 53), (41, 0x31E0, 69), (18, 0x31F0, 47), (85, 0x3202, 42), (22, 0x3208, 41)] {
        if has(cg, skill) { choices.push((cg.ds.str(text), item, Some(skill))) }
    }
    let (item, skill) = if choices.len() > 1 {
        cg.message(ui, 0x320F, &[]);
        let names: Vec<&str> = choices.iter().map(|c| c.0.as_str()).collect();
        let selected = loop {
            ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2);
            let r = cg.menu(ui, &menu_args(&names, &[4, 100, 130, 0, 3, 0, 1, 0, 3, 0x3228, 0], -1))?;
            if r >= 0 { break r as usize }
        };
        let (_, item, skill) = &choices[selected];
        (*item, skill.unwrap_or_else(|| [42u8, 25, 52].into_iter().find(|&k| has(cg, k)).unwrap_or(52)))
    } else { (46, 40) };
    let w = *ui.work();
    let item_name = cg.ds.item_name(item);
    if cg.find_item(&w, item) != 0 {
        let suffix = if item_name.chars().next().is_some_and(|c| "aeiou".contains(c.to_ascii_lowercase())) { "n" } else { "" };
        cg.message(ui, 0x3230, &[suffix.into(), item_name.as_str().into(), cg.ds.skill_name(skill).into()]);
        ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2);
        let opts = cg.ds.pstrs(0x32D2, 2);
        if cg.menu(ui, &menu_args(&strs(&opts), &[4, 100, 130, 0, 2, 0, 1, 0, 3], -1))? == 0 { cg.message(ui, 0x329A, &[item_name.as_str().into()]); add_item(cg, ui, item); }
        else {
            add_skill(cg, ui, skill, 1);
            let n = cg.find_skill(skill).map(|p| cg.skills[p]).unwrap_or(super::tables::Node { id: skill, level: 0 });
            cg.message(ui, 0x32B2, &[cg.ds.skill_name(n.id).into(), n.level.into()]);
        }
    } else { cg.message(ui, 0x32C4, &[item_name.as_str().into()]); add_item(cg, ui, item); }
    Ok(())
}
