mod common_m2;
use common_m2::*;
use mt_engine::data::{ly::{self as lyd, Entry}, pic::{self as picd, Pic}, Variant};
use mt_engine::present::{fb::Fb, ly::{self, Pen}, pic, prim::Raster, text::{self, Cursor, Font}};
use std::collections::HashMap;

const CASES: usize = 37;

#[derive(Default)]
struct St { lys: HashMap<String, Vec<Entry>>, fonts: HashMap<String, Font>, pics: HashMap<String, Pic>, cursor: Option<Cursor> }

fn apply(st: &mut St, fb: &mut Fb, name: &str, a: &[Arg]) {
    let raster = |s: &Arg| match s.s() { "clip" => Raster::Clip, "wrap" => Raster::Wrap, r => panic!("raster {r}") };
    let i = |k: usize| a[k].i() as i32;
    let u = |k: usize| a[k].i() as usize;
    let tr = |k: usize| (a[k].i() >= 0).then(|| a[k].i() as u8);
    let gf = || gamefiles().expect("a gamefiles call without MT1_GAMEFILES");
    match name {
        "lyfile" => { st.lys.insert(a[0].s().into(), lyd::decode(&std::fs::read(gf().join(a[1].s())).unwrap()).unwrap()); }
        "lyhex" => { st.lys.insert(a[0].s().into(), lyd::decode(&a[1].hex()).unwrap()); }
        "ly" => ly::layout(fb, raster(&a[0]), u(1), &st.lys[a[2].s()], a[3].i() as usize, i(4), i(5), &mut Pen::new(i(6) as u8)),
        "font" => {
            let (w, f) = (a[2].hex(), a[3].hex());
            st.fonts.insert(a[0].s().into(), Font::from_tables(st.lys[a[1].s()].clone(), &[w, f].concat(), 0, 256, i(4) as u8).unwrap());
        }
        "stext" => { text::draw(fb, Raster::Wrap, u(0), &st.fonts[a[1].s()], &a[2].hex(), i(3), i(4), i(5) as u8, tr(6)); }
        "tcursor" => st.cursor = Some(Cursor { r: Raster::Clip, page: u(0), y: a[1].i(), x: a[2].i(), pen: i(3) as u8, colour: i(4) as u8, shadow: i(5) != 0, bg: i(6) as u8, tab: i(7) as u8, tabs: [a[8].i(), a[9].i()], window_x: a[10].i(), ops: ly::OPS }),
        "ttext" => st.cursor.as_mut().unwrap().text(fb, &st.fonts[a[0].s()], &a[1].hex()),
        "twrapped" => st.cursor.as_mut().unwrap().wrapped(fb, &st.fonts[a[0].s()], &a[1].hex(), a[2].i(), a[3].i(), a[4].i(), a[5].i(), a[6].i(), i(7) as u8, i(8) as u8),
        "tcounted" => st.cursor.as_mut().unwrap().counted_glyph(fb, &st.fonts[a[0].s()], i(1) as u8, i(2) as u8, i(3) as u8, a[4].i(), a[5].i(), &a[6].hex(), &a[7].hex()),
        "picfile" => {
            let d = std::fs::read(gf().join(a[1].s())).unwrap();
            let p = if a[2].i() < 0 { picd::decode(Variant::Mt1, &d).unwrap() } else { picd::decode_set(Variant::Mt1, &d).unwrap().swap_remove(u(2)) };
            st.pics.insert(a[0].s().into(), p);
        }
        "pichex" => { st.pics.insert(a[0].s().into(), picd::decode(Variant::Mt1, &a[1].hex()).unwrap()); }
        "pic" => pic::draw(fb, u(0), &st.pics[a[1].s()], i(2), i(3), tr(4)),
        "picrect" => pic::draw_rect(fb, u(0), &st.pics[a[1].s()], i(2), i(3), i(4), i(5), i(6), i(7), tr(8)),
        "icon" => pic::icon(fb, u(0), &st.pics[a[1].s()], &st.pics[a[2].s()], i(3), i(4), i(5)),
        "upload" => pic::upload(fb, &a[0].hex(), u(1), u(2), u(3), a[4].i() as u16, a[5].i() as u16, i(6) as u8, i(7) as u8),
        "atlas" => pic::atlas(fb, &st.pics[a[0].s()], u(1)),
        n => panic!("unknown call {n}"),
    }
}

#[test]
fn oracle_fixtures() {
    let Some(cases) = fixtures("text", "m2_dump_text.py") else { return };
    let partial = std::fs::read_to_string(workspace().join("target/m2-fixtures/text/PARTIAL")).ok();
    if std::env::var_os("MT1_GAMEFILES").is_some() {
        assert!(partial.is_none(), "helper dropped cases: {}", partial.unwrap_or_default());
        assert_eq!(cases.len(), CASES, "case count with MT1_GAMEFILES set");
    } else { println!("PARTIAL: synthetic cases only ({})", partial.unwrap_or_default().trim()) }
    let mut st = St::default();
    replay(&cases, |fb, name, args| apply(&mut st, fb, name, args));
    println!("{} cases", cases.len());
}
