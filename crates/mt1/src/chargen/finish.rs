use super::skills::add_skill;
use super::tables::{fmt, menu_args, strs, Abort, Cg, R};
use crate::ui::CgUi;
use crate::record::{W_HITS_A, W_HITS_B, W_NAME, W_STAT};

fn draw_totals<H: CgUi>(cg: &mut Cg, ui: &mut H, total: u8, limit: u8) {
    if cg.totals == Some((total, limit)) { return }
    let a = fmt(&cg.ds.str(0x050C), &[limit.into()]);
    let width = ui.string_width(&a) + 2;
    ui.fill_rect(101, 101, 19, width, 2, 3); ui.draw_border(100, 100, 21, width + 2, 15, 3);
    ui.draw_text_at(0, 111, 102, 8, 5, &a);
    let b = fmt(&cg.ds.str(0x0529), &[total.into()]);
    ui.draw_text_at(0, 102, 102, 8, 5, &b);
    cg.totals = Some((total, limit));
}
fn skill_cursor<H: CgUi>(cg: &Cg, ui: &mut H) { ui.fill_rect(67 + (cg.selected_skill << 3), 4, 7, 65, 7, 0x1803) }

pub fn finish_character<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    let original = cg.skills.clone();
    let total0 = cg.skills.iter().fold(0u8, |t, n| t.wrapping_add(n.level as u8));
    let mut total = total0;
    let w = ui.work();
    let limit = w[W_STAT + 3].wrapping_add(w[W_STAT + 4]);
    if total > limit {
        cg.message(ui, 0x0555, &[total.into(), limit.into()]);
        cg.message(ui, 0x04D0, &[]);
        loop {
            ui.set_text_color(3); draw_totals(cg, ui, total, limit); skill_cursor(cg, ui);
            ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2);
            let opts = cg.ds.pstrs(0x0620, if total > limit { 2 } else { 3 });
            let mut choice = cg.hmenu(ui, &menu_args(&strs(&opts), &[4, 122, 100, 0, 2, 0, 1, 0, 3], -1))? - 1;
            if choice >= 0 { choice = 1 - choice }
            skill_cursor(cg, ui);
            let position = (cg.selected_skill + cg.first_skill).max(0) as usize;
            if choice == 0 { break }
            let Some((src, orig)) = cg.skills.get(position).copied().zip(original.get(position).copied()) else { return Err(Abort) };
            if src.level as i16 + choice > orig.level as i16 { cg.message(ui, 0x05B1, &[cg.ds.skill_name(src.id).into()]) }
            else { total = total.wrapping_add(add_skill(cg, ui, src.id, choice as i8) as u8) }
        }
        ui.fill_rect(100, 100, 21, 200, 18, 3);
    }
    let w = ui.work();
    let hit = (0..3).map(|i| w[W_STAT + i] as i8 as i32).sum::<i32>() / 3 - 1;
    let at = (0x0602i32 + 2 * hit) as usize;
    w[W_HITS_A] = cg.ds.u8(at).wrapping_mul(2); w[W_HITS_B] = cg.ds.u8(at + 1).wrapping_mul(3);
    let hits = fmt(&cg.ds.str(0x05E4), &[(w[W_HITS_B] as i8).into(), (w[W_HITS_A] as i8).into()]);
    ui.set_text_color(0); ui.draw_text_at(0, 38, 144, 8, 5, &hits);
    cg.message(ui, 0x05EA, &[]);
    ui.set_text_spacing(5); ui.set_text_color(2);
    let name = loop { match ui.name(15) { None => return Err(Abort), Some(n) if n.is_empty() => continue, Some(n) => break n } };
    let w = ui.work();
    let bytes: Vec<u8> = name.chars().map(|c| c as u32 as u8).take(15).collect();
    w[W_NAME..W_NAME + 15].fill(0); w[W_NAME..W_NAME + bytes.len()].copy_from_slice(&bytes);
    let shown: String = bytes.iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
    ui.set_text_color(0); ui.set_alignment(1); ui.draw_text_at(0, 5, 125, 8, 5, &shown);
    let _ = ui.delay(18);
    Ok(())
}
