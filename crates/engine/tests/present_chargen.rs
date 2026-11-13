mod common_m2;
use common_m2::*;
use mt_engine::data::ly::{self as lyd, Entry};
use mt_engine::present::{fb::Fb, ly::{self, Pen}, prim::Raster, text::{self, Font}};

const FRAMES: usize = 28;

#[test]
fn golden_run_1_frames() {
    let Some(gf) = gamefiles() else { return };
    let load = |n: &str| -> Vec<Entry> { lyd::decode(&std::fs::read(gf.join(n)).unwrap()).unwrap() };
    let (font_ly, cg) = (load("FONT.LY"), load("CG.LY"));
    let Some(cases) = fixtures("chargen", "m2_dump_chargen.py") else { return };
    assert_eq!(cases.len(), FRAMES, "input-step frames");
    let (mut font, w) = (None::<Font>, Raster::Wrap);
    replay(&cases, |fb: &mut Fb, name, a| {
        let i = |k: usize| a[k].i() as i32;
        let u = |k: usize| a[k].i() as usize;
        match name {
            "font" => font = Font::from_tables(font_ly.clone(), &[a[0].hex(), a[1].hex()].concat(), 0, 256, 1),
            "layout" => ly::layout(fb, w, u(0), if a[1].s() == "font" { &font_ly } else { &cg }, a[2].i() as usize, i(3), i(4), &mut Pen::new(i(5) as u8)),
            "text" => { text::draw(fb, w, u(0), font.as_ref().unwrap(), &a[1].hex(), i(2), i(3), i(4) as u8, (a[5].i() >= 0).then(|| a[5].i() as u8)); }
            "fill" => fb.fill(w, u(0), i(1), i(2), i(3), i(4), i(5) as u8, i(6) as u8),
            "line" => fb.line(w, u(0), i(1), i(2), i(3), i(4), i(5) as u8, i(6) as u8),
            "pixel" => fb.pixel(w, u(0), i(1), i(2), i(3) as u8, i(4) as u8),
            n => panic!("unknown call {n}"),
        }
    });
    println!("{} frames byte-equal", cases.len());
}
