use super::tables::{menu_args, strs, Cg, R};
use crate::record::W_STAT;
use crate::ui::CgUi;
use mt_engine::sim::rng::Rng;

pub fn roll_attributes(rng: &mut Rng) -> [u8; 6] { std::array::from_fn(|_| { let (a, b) = (rng.next(), rng.next()); (2 + a % 6 + b % 6) as u8 }) }

fn roll_in_situ(ui: &mut dyn CgUi, panel: i16) {
    ui.select_window(panel); ui.clear_window();
    let v = roll_attributes(ui.rng());
    ui.work()[W_STAT..W_STAT + 6].copy_from_slice(&v);
    ui.set_text_color(10); ui.set_text_mode(0);
    for (i, x) in v.iter().enumerate() { ui.draw_text_at(panel, i as i16 * 12 + 2, 5, 12, 2, &x.to_string()) }
}
pub fn attribute_screen<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    ui.reset_text(); ui.set_line_height(12); ui.set_text_spacing(2);
    let labels = cg.ds.pstrs(0x04BE, 6);
    ui.set_text_color(10);
    let width = labels.iter().map(|l| ui.string_width(l)).max().unwrap_or(0) + 2;
    let page = ui.open_window(118, 100, 12 * 6 + 4, width + 3, 1, 2, 0);
    ui.set_alignment(1);
    for (i, l) in labels.iter().enumerate() { ui.draw_text_at(page, i as i16 * 12 + 2, 1, 12, 2, l) }
    let panel = ui.open_window(118, 100 + width, 12 * 6 + 4, 25, 1, 2, 0);
    cg.message(ui, 0x0496, &[]);
    roll_in_situ(ui, panel);
    ui.set_text_mode(0); ui.set_text_spacing(2); ui.set_text_color(2);
    let yn = cg.ds.pstrs(0x1826, 2); let items = strs(&yn);
    let m = menu_args(&items, &[4, 118, 220, 0, 2, 0, 1, 0, 3], 1);
    let r = ui.hmenu_repeat(&m, &mut |ui, choice| { if choice == 1 { roll_in_situ(ui, panel); true } else { false } });
    cg.esc(r)?;
    ui.set_text_color(4);
    for i in 0..6 { let v = ui.work()[W_STAT + i]; ui.draw_text_at(0, 15, 16 + 11 * i as i16, 8, 5, &format!("{v:X}")) }
    ui.close_window(panel); ui.close_window(panel - 1);
    Ok(())
}
pub fn change_attribute(ui: &mut dyn CgUi, attr: u8, change: i8) {
    ui.select_window(0); ui.reset_text(); ui.set_text_color(4);
    let p = W_STAT + attr as usize; let w = ui.work();
    w[p] = w[p].wrapping_add(change as u8); if w[p] > 15 { w[p] = 15 }
    let v = w[p];
    ui.fill_rect(15, attr as i16 * 11 + 14, 10, 10, 2, 3);
    ui.draw_text_at(0, 15, attr as i16 * 11 + 16, 8, 5, &((v + if v > 9 { 55 } else { 48 }) as char).to_string());
}
