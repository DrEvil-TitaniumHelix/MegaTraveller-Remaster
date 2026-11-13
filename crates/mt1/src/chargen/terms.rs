use super::attributes::change_attribute;
use super::skills::{add_skill, choose_skills};
use super::tables::{menu_args, strs, Cg, R};
use crate::ui::CgUi;
use crate::record::{W_AGE, W_RANK, W_SERVICE, W_STAT};

pub fn serve_term<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R<u8> {
    if cg.term == 1 { cg.earned = cg.earned.wrapping_add(1) }
    let w = ui.work();
    let service = w[W_SERVICE] as i8;
    let mut target = cg.ds.rule(service as u8, 1) as i16;
    let m = 0x006A + 2 * service as usize;
    if w[(W_STAT as isize + cg.ds.i8(m) as isize) as usize] as i8 >= cg.ds.i8(m + 1) { target -= 2 }
    w[W_AGE] = w[W_AGE].wrapping_add(4);
    let mut state = if cg.dice(ui, 2) >= target { 1 } else { injury(cg, ui)? };
    if state == 1 {
        cg.earned = cg.earned.wrapping_add(1);
        if ui.work()[W_SERVICE] == 3 { cg.earned = cg.earned.wrapping_add(1) }
        process_rank(cg, ui)?;
    }
    ui.fill_rect(27, 185, 8, 15, 2, 3); ui.set_text_color(0);
    let age = ui.work()[W_AGE] as i8; ui.draw_text_at(0, 27, 185, 8, 5, &age.to_string());
    if state == 1 { state = age_character(cg, ui) }
    if state != 0 {
        let e = cg.earned;
        cg.message(ui, 0x0045, &[cg.term.into(), e.into(), cg.ds.str(if e == 1 { 0x0042 } else { 0x0043 }).into()]);
        choose_skills(cg, ui)?;
    }
    Ok(state)
}
pub fn extra_skill(cg: &mut Cg, roll: i16, target: i16) { if roll > target.wrapping_add(4) { cg.earned = cg.earned.wrapping_add(1) } }

pub fn injury<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R<u8> {
    if cg.term == 1 { return Ok(1) }
    let damage = (ui.rng().next() % 3 + 1) as u8;
    let attr = (ui.rng().next() % 3) as u8;
    change_attribute(ui, attr, -(damage as i8));
    cg.message(ui, 0x015B, &[damage.into(), cg.ds.str(if damage == 1 { 0x0158 } else { 0x0159 }).into(), cg.ds.skill_name(attr).into()]);
    cg.message(ui, 0x01C2, &[]);
    ui.set_text_spacing(5); ui.set_line_height(8); ui.set_text_color(2);
    let yn = cg.ds.pstrs(0x1826, 2);
    let choice = cg.hmenu(ui, &menu_args(&strs(&yn), &[4, 103, 207, 0, 2, 0, 1, 0, 3], 1))? & 0xFF;
    if choice == 0 {
        let roll = cg.dice(ui, 2) + 2;
        if roll > ui.work()[W_STAT + 2] as i8 as i16 { cg.message(ui, 0x0206, &[]); return Ok(0) }
        cg.message(ui, 0x0227, &[]);
        change_attribute(ui, attr, damage as i8);
        return Ok(1)
    }
    if choice != 1 { return Ok(2) }
    let w = ui.work(); w[W_AGE] = w[W_AGE].wrapping_sub(2);
    if w[W_STAT + attr as usize] as i8 <= 0 { cg.message(ui, 0x025D, &[]); return Ok(0) }
    let roll = cg.dice(ui, 2) as u8;
    let service = ui.work()[W_SERVICE] as i8;
    let target = cg.ds.u8(0x1502 + service as usize * 6 + 4);
    if roll >= target {
        let prev = cg.earned;
        extra_skill(cg, roll as i16, target as i8 as i16);
        cg.earned = cg.earned.wrapping_add(1);
        let gained = cg.earned.wrapping_sub(prev);
        cg.message(ui, 0x0284, &[gained.into(), cg.ds.str(if gained == 1 { 0x0281 } else { 0x0282 }).into()]);
    }
    if ui.work()[W_SERVICE] == 3 { cg.earned = cg.earned.wrapping_add(1) }
    let service = ui.work()[W_SERVICE] as i8;
    cg.message(ui, 0x02C7, &[cg.ds.pstr(0x14EE, service as u8 as usize).into()]);
    Ok(2)
}

pub fn age_character<H: CgUi>(cg: &mut Cg, ui: &mut H) -> u8 {
    let age = ui.work()[W_AGE] as i8 as i32;
    if age < 34 { return 1 }
    let mut rule = 0x0108 + (((age - 34) / 4).min(8) as usize) * 8;
    let mut alive = 1;
    for attr in 0..4u8 {
        if (cg.dice(ui, 2) as i32) < cg.ds.u8(rule) as i32 {
            let (short, loss) = (cg.ds.skill_short(attr), cg.ds.u8(rule + 1));
            cg.message(ui, 0x0074, &[short.into(), loss.into()]);
            let v = ui.work()[W_STAT + attr as usize];
            if v as i8 as i32 <= loss as i32 {
                if cg.dice(ui, 2) >= 8 { ui.work()[W_STAT + attr as usize] = 1 } else { change_attribute(ui, attr, (v as i8).wrapping_neg()); alive = 0 }
            } else { change_attribute(ui, attr, (loss as i8).wrapping_neg()) }
        }
        rule += 2;
    }
    if alive == 0 { cg.message(ui, 0x009B, &[]) }
    else if (0..4).any(|a| ui.work()[W_STAT + a] as i8 as i32 <= cg.ds.u8(rule + a * 2 + 1) as i32) { cg.message(ui, 0x00C6, &[]) }
    alive
}

pub fn process_rank<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    let old = cg.earned;
    let service = ui.work()[W_SERVICE] as i8;
    let s = service as u8;
    if (cg.drafted == 0 || cg.term != 1) && service != 3 {
        let modifier = 0x355E + s as usize * 4;
        let (mut promoted, mut title, mut roll, mut target) = (false, 0u8, 0i16, 0i16);
        let rank = ui.work()[W_RANK];
        if rank == 0 || rank < 6 {
            let promotion = rank != 0;
            roll = cg.dice(ui, 2);
            target = cg.ds.rule(s, if promotion { 3 } else { 2 }) as i16;
            let attr = cg.ds.i8(modifier + if promotion { 2 } else { 0 });
            let minimum = cg.ds.i8(modifier + if promotion { 3 } else { 1 });
            let w = ui.work();
            if w[(W_STAT as isize + attr as isize) as usize] as i8 >= minimum { target -= 1 }
            if roll >= target {
                if promotion { cg.earned = cg.earned.wrapping_add(1); w[W_RANK] = rank + 1 } else { w[W_RANK] = 1; cg.earned = cg.earned.wrapping_add(1) }
                title = cg.ds.rank_title(w[W_RANK], s);
                if promotion { cg.message(ui, 0x3322, &[cg.ds.pstr(0x115E, title as usize).into()]) }
                else { cg.message(ui, 0x32DA, &[cg.ds.service_name(s).into()]); ui.select_window(0) }
                promoted = true;
            }
        }
        if promoted {
            ui.fill_rect(36, 39, 8, 45, 2, 3); ui.set_text_color(0);
            let short = cg.ds.pstr(0x1138, title as usize); ui.draw_text_at(0, 36, 39, 8, 5, &short);
            extra_skill(cg, roll, target);
            for i in 0..3 {
                let a = 0x17D3 + 9 * s as usize + 3 * i;
                if ui.work()[W_RANK] == cg.ds.u8(a) && cg.ds.u8(a + 1) != 255 { add_skill(cg, ui, cg.ds.u8(a + 1), cg.ds.i8(a + 2)); }
            }
        }
    }
    let roll = cg.dice(ui, 2);
    let target = cg.ds.rule(s, 4) as i16;
    if roll >= target { cg.message(ui, 0x335E, &[]); extra_skill(cg, roll, target); cg.earned = cg.earned.wrapping_add(1) }
    if old == cg.earned {
        let i = (ui.rng().next() % 16) / 2;
        cg.message(ui, 0x3546, &[cg.ds.pstr(0x3572, i as usize).into()]);
    }
    Ok(())
}
