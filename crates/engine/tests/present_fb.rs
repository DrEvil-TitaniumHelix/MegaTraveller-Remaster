mod common_m2;
use common_m2::*;
use mt_engine::data::pal;
use mt_engine::present::{fb::Fb, palette, prim::Raster};
use std::sync::OnceLock;

static CASES: OnceLock<Option<Vec<Case>>> = OnceLock::new();
fn cases(prefix: &str) -> Option<Vec<&'static Case>> {
    let all = CASES.get_or_init(|| fixtures("fb", "m2_dump_fb.py")).as_ref()?;
    let v: Vec<&Case> = all.iter().filter(|c| c.name.starts_with(prefix)).collect();
    assert!(!v.is_empty(), "no fixture case named {prefix}*");
    Some(v)
}
fn group(prefix: &str) { if let Some(v) = cases(prefix) { for c in v { replay(std::slice::from_ref(c), apply) } } }

fn apply(fb: &mut Fb, name: &str, a: &[Arg]) {
    let raster = |s: &Arg| match s.s() { "clip" => Raster::Clip, "wrap" => Raster::Wrap, r => panic!("raster {r}") };
    let i = |k: usize| a[k].i() as i32;
    let u = |k: usize| a[k].i() as usize;
    match name {
        "pixel" => fb.pixel(raster(&a[0]), u(1), i(2), i(3), i(4) as u8, i(5) as u8),
        "line" => fb.line(raster(&a[0]), u(1), i(2), i(3), i(4), i(5), i(6) as u8, i(7) as u8),
        "fill" => fb.fill(raster(&a[0]), u(1), i(2), i(3), i(4), i(5), i(6) as u8, i(7) as u8),
        "rect" => fb.rect(raster(&a[0]), u(1), i(2), i(3), i(4), i(5), i(6) as u8, i(7) as u8),
        "blit" => { let src = a[1].hex(); fb.blit(u(0), &src, u(2), i(3), i(4), i(5), i(6), i(7), i(8), (i(9) >= 0).then(|| i(9) as u8)) }
        "copy" => fb.copy(u(0), i(1), i(2), i(3), i(4), u(5), i(6), i(7)),
        "copyrect" => fb.copy_rect(u(0), u(1), i(2), i(3), i(4), i(5)),
        "reg" => fb.set_reg(u(0), i(1) as u8),
        n => panic!("unknown call {n}"),
    }
}

#[test] fn pixel() { group("pixel_") }
#[test] fn line_horizontal() { group("line_h_") }
#[test] fn line_vertical() { group("line_v_") }
#[test] fn line_diagonal() { group("line_diag_") }
#[test] fn line_point() { group("line_point_") }
#[test] fn fill() { group("fill_") }
#[test] fn rect() { group("rect_") }
#[test] fn blit() { group("blit") }
#[test] fn copy_video() { group("copy") }
#[test] fn copy_rect() { group("copyrect") }
#[test] fn registers() { group("regs") }
#[test] fn random_mix() { group("random_") }
#[test] fn extreme_arguments() { group("extreme") }
#[test] fn clip_high_page() { group("clip_page") }

#[test]
fn palette_map_pal() {
    let Some(g) = gamefiles() else { return };
    let Some(v) = cases("palette") else { return };
    let case = v[0];
    let regs = palette::regs(&pal::decode(&std::fs::read(g.join("MAP.PAL")).unwrap()).unwrap()).unwrap();
    let mut fb = Fb::new();
    assert!(fb.restore(&case.start));
    for (name, args) in &case.calls { apply(&mut fb, name, args) }
    assert_eq!(fb.regs, regs, "MAP.PAL registers");
    check(case, &fb.snapshot());
    let dac = palette::ega_dac();
    let rd = |f: &str| std::fs::read(case.dir.join(f)).unwrap();
    let triples = |b: &[u8]| b.chunks(3).map(|c| (c[0] as u32) << 16 | (c[1] as u32) << 8 | c[2] as u32).collect::<Vec<u32>>();
    assert_eq!(palette::colors(&fb.regs, &dac).to_vec(), triples(&rd("colors.bin")), "colors");
    for page in 0..2 {
        let want = triples(&rd(&format!("rgb{page}.bin")));
        let got = fb.rgb(page, &dac);
        if let Some(i) = (0..64000).find(|&i| got[i] != want[i]) { panic!("rgb page {page} pixel {i}: expected {:#08x} got {:#08x}", want[i], got[i]) }
        println!("PASS palette rgb page {page}");
    }
}
