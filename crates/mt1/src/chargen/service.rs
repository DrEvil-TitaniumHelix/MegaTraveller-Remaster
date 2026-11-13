use super::skills::add_skill;
use super::tables::{menu_args, strs, Cg, R};
use crate::ui::CgUi;
use crate::record::{W_SERVICE, W_STAT};

pub fn enlist<H: CgUi>(cg: &mut Cg, ui: &mut H) -> R {
    cg.drafted = 0;
    let mut service: i16 = -1;
    cg.message(ui, 0x2791, &[]);
    ui.reset_text(); ui.set_text_mode(0); ui.set_text_spacing(4); ui.set_text_color(2); ui.set_line_height(12);
    let opts = cg.ds.pstrs(0x28B0, 2);
    if cg.hmenu(ui, &menu_args(&strs(&opts), &[4, 118, 170, 0, 2, 0, 1, 0, 3], -1))? == 0 {
        cg.message(ui, 0x27DC, &[]);
        ui.set_text_spacing(4); ui.set_text_color(2);
        let names = cg.ds.pstrs(0x14EE, 5);
        let r = cg.menu(ui, &menu_args(&strs(&names), &[4, 110, 150, 0, 3, 0, 1, 0, 3, 0x27EC, 0], -1))?;
        service = r;
        let s = service as u8;
        let mut target = cg.ds.rule(s, 0) as i16;
        let m = 0x289C + 4 * s as usize;
        let w = ui.work();
        if w[(W_STAT as isize + cg.ds.i8(m) as isize) as usize] as i8 >= cg.ds.i8(m + 1) { target -= 1 }
        if w[(W_STAT as isize + cg.ds.i8(m + 2) as isize) as usize] as i8 >= cg.ds.i8(m + 3) { target -= 2 }
        if cg.dice(ui, 2) >= target { cg.message(ui, 0x27F5, &[names[s as usize].as_str().into()]) }
        else { cg.message(ui, 0x2829, &[names[s as usize].as_str().into()]); cg.drafted = s; service = -1 }
    }
    if service == -1 {
        loop { let r = ui.rng().next() as i16; service = r - (r / 5) * 5; if service as u8 != cg.drafted { break } }
        cg.drafted = 1;
        cg.message(ui, 0x2878, &[cg.ds.service_name(service as u8).into()]);
    }
    let s = service as u8;
    ui.set_text_color(0);
    let short = cg.ds.pstr(0x14F8, s as usize); ui.draw_text_at(0, 28, 39, 8, 5, &short);
    super::tables::emblem(ui, 30, 15, s);
    ui.work()[W_SERVICE] = s;
    let entry = cg.ds.pstr(0x1138, cg.ds.rank_title(0, s) as usize); ui.draw_text_at(0, 36, 39, 8, 5, &entry);
    for i in 0..3 {
        let (c, a) = (0x17CA + 3 * i, 0x17D3 + 9 * s as usize + 3 * i);
        add_skill(cg, ui, cg.ds.u8(c + 1), cg.ds.i8(c + 2));
        if cg.ds.u8(a) == 0 && cg.ds.u8(a + 1) != 255 { add_skill(cg, ui, cg.ds.u8(a + 1), cg.ds.i8(a + 2)); }
    }
    Ok(())
}
