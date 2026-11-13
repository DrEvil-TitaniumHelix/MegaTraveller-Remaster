use super::attributes::attribute_screen;
use super::finish::finish_character;
use super::muster::muster_out;
use super::service::enlist;
use super::tables::{menu_args, strs, Cg, R};
use super::terms::serve_term;
use crate::ui::CgUi;
use crate::record::{self, DS_USED, W_ITEMS, W_STATE};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit { Dos, Continue, Quick, Play }

pub fn main_menu<H: CgUi>(cg: &mut Cg, ui: &mut H) -> Exit {
    ui.layout(12, 0, 0);
    loop {
        banner(cg, ui, cg.ds.u16(0x1136) as usize);
        let opts = cg.ds.pstrs(0x29D8, 4);
        match ui.menu(&menu_args(&strs(&opts), &[4, 80, 30, 0, 2, 0, 1, 0, 3], -1)) {
            0 => if let Some(e) = party_menu(cg, ui) { return e },
            1 => return Exit::Continue, 2 => return Exit::Quick, 3 => return Exit::Dos, _ => {}
        }
    }
}
fn banner<H: CgUi>(cg: &Cg, ui: &mut H, at: usize) { ui.fill_rect(0, 0, 49, 320, 2, 3); ui.set_text_color(0); let s = cg.ds.str(at); ui.draw_centered(&s, 20); ui.set_text_color(2) }
fn draw_party<H: CgUi>(cg: &Cg, ui: &mut H) {
    ui.fill_rect(120, 208, 47, 85, 2, 3); ui.set_text_color(0);
    for (i, &m) in cg.party.iter().enumerate() { if m != 0 { ui.move_to(124 + 8 * i as i16, 210); let n = slot_name(cg, m as usize - 1); ui.text(&n) } }
}
fn slot_name(cg: &Cg, slot: usize) -> String { cg.pool.get(slot).map(|r| r.iter().take(20).take_while(|&&b| b != 0).map(|&b| b as char).collect()).unwrap_or_default() }

pub fn party_menu<H: CgUi>(cg: &mut Cg, ui: &mut H) -> Option<Exit> {
    let mut choice;
    loop {
        banner(cg, ui, 0x2BBC);
        let opts = cg.ds.pstrs(0x29E0, 3);
        choice = ui.menu(&menu_args(&strs(&opts), &[4, 80, 30, 0, 2, 0, 1, 0, 3], -1));
        match choice {
            -1 => return None,
            0 => { choice = load_pool(cg, ui, "UCHARS.DAT", true); if choice != -1 { cg.set_party_disk(true) } }
            _ => {
                load_pool(cg, ui, "DCHARS.DAT", true);
                if choice == 2 { cg.set_party_disk(false) }
                else if prepare_disk(cg, ui) { cg.set_party_disk(true); save_pool(cg, ui); choice = 0 }
                else { choice = -1 }
            }
        }
        if choice != -1 { break }
    }
    cg.party = [0; 5]; cg.party_count = 0;
    let mut aborted = false;
    let mut last: u8 = choice as u8;
    loop {
        if aborted {
            aborted = false;
            ui.init_windows(); ui.set_line_height(8); ui.set_text_spacing(0); ui.layout(12, 0, 0); ui.set_text_color(2); ui.select_window(0);
        }
        draw_party(cg, ui);
        banner(cg, ui, cg.ds.u16(0x1136) as usize);
        let mut options: Vec<(String, u8)> = vec![];
        let count = cg.pool_count();
        if count < 15 { options.push((cg.ds.str(0x2BF6), 0)) }
        if cg.party_disk() { options.push((cg.ds.str(0x2C0B), 1)) }
        if count != 0 {
            let available = can_add(cg);
            if available { options.push((cg.ds.str(0x2C1F), 2)) }
            options.push((cg.ds.str(0x2C3A), 3)); options.push((cg.ds.str(0x2C49), 7));
            if available && cg.party_count < 5 { options.push((cg.ds.str(0x2C59), 4)) }
            if cg.party_count != 0 { options.push((cg.ds.str(0x2C66), 5)); options.push((cg.ds.str(0x2C78), 6)) }
        }
        let names: Vec<&str> = options.iter().map(|o| o.0.as_str()).collect();
        let default = options.iter().rposition(|o| o.1 == last).map_or(-1, |i| i as i16);
        let sel = ui.menu(&menu_args(&names, &[4, 80, 30, 0, 2, 0, 1, 0, 3], default));
        if sel == -1 { return None }
        last = options[sel as usize].1;
        match options[sel as usize].1 {
            0 => if generate(cg, ui).is_err() { cg.esc_aborts = false; aborted = true },
            1 => save_pool(cg, ui),
            2 => delete_character(cg, ui),
            3 => view_character(cg, ui),
            4 => add_party_member(cg, ui),
            5 => remove_party_member(cg, ui),
            6 => return Some(Exit::Play),
            _ => {}
        }
    }
}
fn can_add(cg: &Cg) -> bool { cg.pool_count() > 5 || cg.pool.occupied().any(|(i, _)| !cg.party.contains(&(i as u8 + 1))) }
fn choose_character<H: CgUi>(cg: &Cg, ui: &mut H, include_party: bool) -> i16 {
    let slots: Vec<usize> = cg.pool.occupied().map(|(i, _)| i).filter(|&i| include_party || !cg.party.contains(&(i as u8 + 1))).collect();
    if slots.is_empty() { return -1 }
    let names: Vec<String> = slots.iter().map(|&s| slot_name(cg, s)).collect();
    ui.layout(20, 0, 0); ui.set_text_color(2);
    let i = ui.menu(&menu_args(&strs(&names), &[4, 55, 30, 0, 2, 0, 1, 0, 3], -1));
    ui.layout(12, 0, 0);
    if i == -1 { -1 } else { slots[i as usize] as i16 }
}
fn choose_party_member<H: CgUi>(cg: &Cg, ui: &mut H) -> i16 {
    let idx: Vec<usize> = (0..5).filter(|&i| cg.party[i] != 0).collect();
    if idx.is_empty() { return -1 }
    let names: Vec<String> = idx.iter().map(|&i| slot_name(cg, cg.party[i] as usize - 1)).collect();
    let r = ui.menu(&menu_args(&strs(&names), &[4, 85, 30, 0, 2, 0, 1, 0, 3], -1));
    if r == -1 { -1 } else { idx[r as usize] as i16 }
}
pub fn delete_character<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    if cg.pool_count() == 0 { return }
    banner(cg, ui, 0x2AA9);
    let sel = choose_character(cg, ui, false);
    if sel != -1 && ui.confirm(&format!("Delete {} for good?", slot_name(cg, sel as usize))) { cg.pool.clear(sel as usize); cg.ds.set_u8(DS_USED + sel as usize, 0); cg.set_next_slot(sel as usize); let c = cg.pool_count(); cg.set_pool_count(c - 1) }
}
fn view_character<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    if cg.pool_count() == 0 { banner(cg, ui, 0x2ADB); let _ = ui.delay(36); return }
    let sel = choose_character(cg, ui, true);
    if sel == -1 { return }
    ui.layout(19, 0, 0);
    for (row, col, s, color) in cg.ds.text_list(0x2CEA) { ui.move_to(row, col); ui.set_text_color(color as i16); ui.text(&s) }
    let rec = cg.pool.get(sel as usize).unwrap_or(&[]).to_vec();
    ui.character_sheet(sel as usize, &rec);
    ui.read_key();
    ui.layout(12, 0, 0);
}
fn add_party_member<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    let sel = choose_character(cg, ui, false) + 1;
    if sel == 0 { return }
    match cg.party.iter().position(|&m| m == 0) {
        Some(i) => { cg.party[i] = sel as u8; cg.party_count += 1 }
        None => { banner(cg, ui, 0x2BAA); let _ = ui.delay(20); }
    }
}
fn remove_party_member<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    let sel = choose_party_member(cg, ui);
    if sel != -1 { cg.party[sel as usize] = 0; draw_party(cg, ui); cg.party_count -= 1 }
}

pub fn load_pool<H: CgUi>(cg: &mut Cg, ui: &mut H, name: &str, report: bool) -> i16 {
    let Some(f) = ui.files().read(name) else { if report { banner(cg, ui, 0x2AF1); let _ = ui.delay(36); } return -1 };
    record::load_pool(&mut cg.pool, &mut cg.ds.image_mut(), &f);
    0
}
pub fn save_pool<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    if cg.party_disk() {
        banner(cg, ui, 0x2A40);
        let mut bytes = record::save_pool(&cg.pool).unwrap_or_default();
        if let Some(b) = bytes.first_mut() { *b = cg.pool_count() }
        if ui.files().write("UCHARS.DAT", &bytes).is_err() { banner(cg, ui, 0x2A61); let _ = ui.delay(36); }
    } else { banner(cg, ui, 0x2A7F); let _ = ui.delay(36); }
}
fn prepare_disk<H: CgUi>(cg: &mut Cg, ui: &mut H) -> bool {
    if ui.files().read("UCHARS.DAT").is_some() {
        ui.set_text_color(0); ui.fill_rect(0, 0, 51, 320, 2, 3);
        let p = cg.ds.next_str(cg.ds.next_str(cg.ds.next_str(0x2B1F)));
        let (a, b) = (cg.ds.str(p), cg.ds.str(cg.ds.next_str(p)));
        ui.draw_centered(&a, 10); ui.draw_centered(&b, 20);
        ui.set_text_color(2);
        let yn = cg.ds.pstrs(0x1826, 2);
        return ui.yes_no(&[&yn[0], &yn[1]])
    }
    true
}
fn add_to_pool<H: CgUi>(cg: &mut Cg, ui: &mut H) {
    let slot = cg.next_slot().min(14);
    let before: [u8; 256] = cg.pool.get(slot).unwrap().try_into().unwrap();
    let skills: Vec<(u8, u8)> = cg.skills.iter().map(|n| (n.id, n.level as u8)).collect();
    let rec = cg.ds.with_tables(|t| record::add_to_pool(ui.work(), &skills, t, &before));
    let (w, mut cell) = (ui.work(), 0usize);
    cg.ds.with_tables(|t| for i in (0..16).rev() {
        let it = w[W_ITEMS + i] as i8;
        if it != -1 && t.two_cell(it) { cell += 2; w[W_ITEMS + i] = 0xFF; if cell >= 19 { break } if matches!(cell, 2 | 5 | 8 | 13) { cell += 1 } }
    });
    cg.pool.put(slot, &rec); cg.ds.set_u8(DS_USED + slot, 1); let c = cg.pool_count(); cg.set_pool_count(c.wrapping_add(1));
    cg.ds.image_mut()[0x1840+slot*256..0x1940+slot*256].copy_from_slice(&rec);
    if let Some(n) = cg.pool.first_free() { cg.set_next_slot(n) }
    ui.pool_added(&rec);
}

pub fn generate<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    if !cg.party_disk() && !cg.warned {
        ui.fill_rect(0, 0, 51, 320, 2, 3); ui.set_text_color(4);
        let (a, b, c) = (cg.ds.str(0x28B4), cg.ds.str(cg.ds.next_str(0x28B4)), cg.ds.str(cg.ds.next_str(cg.ds.next_str(0x28B4))));
        ui.draw_centered(&a, 10); ui.draw_centered(&b, 20); ui.draw_centered(&c, 30);
        ui.set_text_color(2); let _ = ui.delay(140); cg.warned = true;
    }
    cg.esc_aborts = true;
    cg.init_character(ui);
    ui.work()[W_STATE] = 1; cg.term = 0; cg.ds.set_u8(0x1823,cg.term);
    attribute_screen(cg, ui)?;
    enlist(cg, ui)?;
    while cg.state(ui) == 1 {
        cg.earned = 0; cg.term = cg.term.wrapping_add(1); cg.ds.set_u8(0x1823,cg.term);
        ui.select_window(0); ui.fill_rect(27, 130, 10, 15, 2, 3); ui.set_text_color(0);
        ui.draw_text_at(0, 27, 130, 8, 5, &cg.term.to_string());
        let s = serve_term(cg, ui)?; ui.work()[W_STATE] = s;
        if s != 0 { let s = muster_out(cg, ui)?; ui.work()[W_STATE] = s }
    }
    if cg.state(ui) != 0 {
        finish_character(cg, ui)?;
        let w = ui.work(); w[26] = w[18].wrapping_add(w[19]);
        add_to_pool(cg, ui);
    } else { let _ = ui.delay(36); }
    cg.esc_aborts = false;
    ui.layout(12, 0, 0); ui.set_text_color(2); ui.init_windows();
    Ok(())
}
