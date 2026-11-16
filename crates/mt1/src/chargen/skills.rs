use super::attributes::change_attribute;
use super::tables::{menu_args, strs, Cg, Node, R};
use crate::record::{W_SERVICE, W_STAT};
use crate::ui::CgUi;

pub fn add_skill<H: CgUi>(cg: &mut Cg, ui: &mut H, id: u8, amount: i8) -> i8 {
    if cg.ds.skill_type(id) == 1 { change_attribute(ui, id, amount); return amount }
    if amount > 0 && cg.ds.unused(id) { return 0 }
    let first = cg.first_skill as i32;
    if let Some(pos) = cg.find_skill(id) {
        let n = &mut cg.skills[pos];
        n.level = n.level.wrapping_add(amount);
        if n.level < 0 { n.level = n.level.wrapping_sub(amount); cg.message(ui, 0x2E30, &[]); return 0 }
        let level = n.level;
        let p = pos as i32;
        if p >= first && p < first + 16 {
            let h = ui.line_height() as i32;
            ui.fill_rect((h * (p - first) + 66) as i16, 72, 8, 10, 0, 3);
            ui.set_text_color(10); ui.set_text_mode(0);
            ui.draw_text_at(0, ((p - first) * h + 66) as i16, 72, 8, 5, &level.to_string());
        }
        return amount
    }
    cg.skill_count = cg.skill_count.wrapping_add(1);
    let added = Node { id, level: amount };
    let index = cg.skills.iter().position(|n| (n.id as i8) > (id as i8)).unwrap_or(cg.skills.len());
    let before_end = index < cg.skills.len();
    cg.skills.insert(index, added);
    let h = ui.line_height() as i32;
    if before_end {
        let i = index as i32;
        if i >= first && i <= first + 15 {
            let (mut i, mut y) = (i - first, h * (i - first) + 66);
            for k in index..cg.skills.len() {
                let n = cg.skills[k];
                ui.fill_rect(y as i16, 72, 8, 10, 0, 3); ui.fill_rect(y as i16, 4, 8, 65, 0, 3);
                ui.set_text_color(10); ui.set_text_mode(0);
                ui.draw_text_at(0, y as i16, 72, 8, 5, &n.level.to_string());
                let short = cg.ds.skill_short(n.id); ui.draw_text_at(0, y as i16, 5, 8, 5, &short);
                i += 1; if i == 16 { break }
                y += h;
            }
        }
    } else {
        let i = index as i32 - first;
        if (0..16).contains(&i) {
            let y = (h * i + 66) as i16;
            ui.set_text_color(10); ui.set_text_mode(0);
            ui.draw_text_at(0, y, 72, 8, 5, &amount.to_string());
            let short = cg.ds.skill_short(id); ui.draw_text_at(0, y, 5, 8, 5, &short);
        }
    }
    amount
}

pub fn show_skill_table(cg: &mut Cg, ui: &mut dyn CgUi, table: i16) {
    let old_height = ui.line_height();
    ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_mode(0);
    let (saved_window, saved_center) = (ui.current_window(), ui.get_center());
    if cg.skill_window == -1 {
        let title = cg.ds.str(0x2D54);
        let width = ui.string_width(&title) + 4;
        cg.skill_window = ui.open_window(145, 170, 53, 103, 1, 2, 0);
        cg.title_window = ui.open_window(145, 168 - width, 11, width, 1, 1, 2);
        ui.select_window(cg.title_window); ui.move_to(1, 1); ui.set_text_color(3); ui.text(&title);
    }
    ui.select_window(cg.skill_window); ui.clear_window();
    ui.set_center(50); ui.set_text_spacing(5);
    let col = ui.work()[W_SERVICE] as i8 as isize;
    let base = cg.ds.u16(0x2D48 + 2 * (table.max(0) as usize)) as isize;
    for i in 0..6 {
        let id = cg.ds.u8((base + col + 5 * i) as usize);
        ui.set_text_color(if cg.ds.skill_type(id) == 2 { 3 } else { 2 });
        let s = if cg.ds.skill_type(id) == 1 { format!("+1 {}", cg.ds.skill_short(id)) } else { cg.ds.skill_short(id) };
        ui.draw_centered(&s, (1 + 8 * i) as i16);
    }
    ui.select_window(saved_window); ui.set_text_mode(32); ui.set_line_height(old_height); ui.set_text_spacing(0); ui.set_center(saved_center);
}

pub fn choose_skills<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    let service = ui.work()[W_SERVICE] as i8 as isize;
    let mut last_table: i16 = -1;
    for remaining in (1..=cg.earned).rev() {
        cg.message(ui, 0x2DAF, &[remaining.into()]);
        ui.reset_text(); ui.set_text_spacing(5); ui.set_line_height(8); ui.set_text_color(2);
        cg.skill_window = -1;
        let window = ui.open_window(103, 95, 11, 73, 1, 1, 2);
        ui.select_window(window); ui.set_text_color(3);
        ui.move_to(1, 1); let label = cg.ds.str(0x2DD2); ui.text(&label); ui.set_text_color(2);
        let tables = if (ui.work()[W_STAT + 4] as i8) < 8 { 3 } else { 4 };
        let names = cg.ds.pstrs(0x2E28, tables);
        let items = strs(&names);
        let m = menu_args(&items, &[4, 103, 170, 0, 2, 0, 1, 0, 3], last_table);
        let mut on_change = |ui: &mut dyn CgUi, t: i16| show_skill_table(cg, ui, t);
        let r = ui.menu_cb(&m, Some(&mut on_change), None);
        let table = cg.esc(r)?;
        last_table = table;
        ui.close_window(window); ui.close_window(cg.title_window);
        let row = |die: i16| cg.ds.u8((cg.ds.u16(0x2D48 + 2 * table as usize) as isize + 5 * die as isize + service) as usize);
        let any_live = (0..6).any(|die| cg.live(row(die)));
        let mut skill = loop {
            let die = cg.dice(ui, 1) - 1;
            ui.flash(147, 171, 8, 97, 4, die);
            if !any_live || cg.live(row(die)) { break row(die) }
        };
        if cg.ds.skill_type(skill) == 2 {
            cg.message(ui, 0x2DE0, &[cg.ds.skill_name(skill).into()]);
            ui.reset_text(); ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2);
            let members = cg.specialties.iter().find(|(id, _)| *id == skill).map(|(_, l)| l.iter().copied().filter(|&m| !cg.ds.unused(m)).collect::<Vec<u8>>()).unwrap_or_default();
            let labels: Vec<String> = members.iter().map(|&m| format!("{}{}", if cg.ds.skill_type(m) == 1 { "+1 " } else { "" }, cg.ds.skill_name(m))).collect();
            let choice = cg.menu(ui, &menu_args(&strs(&labels), &[4, 103, 95, 0, 2, 0, 1, 0, 3], -1))?;
            skill = members.get(choice.max(0) as usize).copied().unwrap_or(skill);
        }
        ui.close_window(cg.skill_window);
        add_skill(cg, ui, skill, 1);
        if cg.ds.skill_type(skill) == 1 { cg.message(ui, 0x2E04, &[cg.ds.skill_name(skill).into()]) }
        else { let level = cg.find_skill(skill).map_or(0, |p| cg.skills[p].level); cg.message(ui, 0x2E16, &[cg.ds.skill_name(skill).into(), level.into()]) }
    }
    Ok(())
}
