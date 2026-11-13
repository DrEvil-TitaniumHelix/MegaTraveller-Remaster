mod common;
use common::*;
use mt_engine::data::{ly, pal, pic, Variant};
use std::path::{Path, PathBuf};

const V: Variant = Variant::Mt1;
const HELPER: &str = "m1_dump_graphics.py";
const SETS: [(&str, usize); 6] = [("OBJ.DAT", 5), ("SPOBJ.DAT", 2), ("X.DAT", 4), ("Y.DAT", 8), ("Z.DAT", 55), ("C.DAT", 14)];

fn name(p: &Path) -> String { p.file_name().unwrap().to_string_lossy().into_owned() }
fn read(p: &Path) -> Vec<u8> { std::fs::read(p).unwrap_or_else(|e| panic!("{}: {e}", p.display())) }
fn pic_json(p: &pic::Pic) -> J { J::obj(vec![("h", p.h.into()), ("wbytes4", p.wbytes4.into()), ("pal", p.pal.into()), ("px", p.px.clone().into()), ("tail", p.tail.clone().into())]) }
fn ly_json(e: &[ly::Entry]) -> J { J::arr(e.iter().map(|x| J::arr(x.iter().map(|r| J::arr([r.op as i64, r.colour as i64, r.y1 as i64, r.x1 as i64, r.y2 as i64, r.x2 as i64]))))) }
fn pal_json(p: &pal::Pal) -> J { J::obj(vec![("vals", J::arr(p.vals.iter().copied())), ("sep", p.sep.as_str().into()), ("end", p.end.as_str().into())]) }

fn round_trip(files: &[PathBuf], want: usize, f: impl Fn(&[u8]) -> Vec<u8>) {
    assert_eq!(files.len(), want, "{:?}", files);
    for p in files { let d = read(p); assert!(f(&d) == d, "round trip FAIL {}", name(p)); println!("PASS {}", name(p)); }
}
fn cross_check(files: &[PathBuf], fmt: &str, f: impl Fn(&[u8]) -> J) {
    for p in files {
        let Some(want) = oracle_json(HELPER, fmt, p) else { return };
        assert_eq!(f(&read(p)).to_string(), want, "cross-check FAIL {}", name(p));
        println!("CROSS-CHECK PASS {}", name(p));
    }
}

#[test]
fn pic_round_trip() {
    let Some(g) = gamefiles() else { return };
    round_trip(&files_with_suffix(&g, ".PIC"), 9, |d| pic::encode(V, &pic::decode(V, d).unwrap()).unwrap());
}
#[test]
fn pic_cross_check() {
    let Some(g) = gamefiles() else { return };
    cross_check(&files_with_suffix(&g, ".PIC"), "pic", |d| pic_json(&pic::decode(V, d).unwrap()));
}
#[test]
fn ly_round_trip() {
    let Some(g) = gamefiles() else { return };
    round_trip(&files_with_suffix(&g, ".LY"), 11, |d| ly::encode(&ly::decode(d).unwrap()).unwrap());
}
#[test]
fn ly_cross_check() {
    let Some(g) = gamefiles() else { return };
    cross_check(&files_with_suffix(&g, ".LY"), "ly", |d| ly_json(&ly::decode(d).unwrap()));
}
#[test]
fn pal_round_trip() {
    let Some(g) = gamefiles() else { return };
    round_trip(&[g.join("MAP.PAL")], 1, |d| pal::encode(&pal::decode(d).unwrap()));
}
#[test]
fn pal_cross_check() {
    let Some(g) = gamefiles() else { return };
    cross_check(&[g.join("MAP.PAL")], "pal", |d| pal_json(&pal::decode(d).unwrap()));
}
#[test]
fn piccont_round_trip() {
    let Some(g) = gamefiles() else { return };
    for (f, n) in SETS { assert_eq!(pic::decode_set(V, &read(&g.join(f))).unwrap().len(), n, "{f} chunk count"); }
    let files: Vec<PathBuf> = SETS.iter().map(|(f, _)| g.join(f)).collect();
    round_trip(&files, 6, |d| pic::encode_set(V, &pic::decode_set(V, d).unwrap()).unwrap());
}
#[test]
fn piccont_cross_check() {
    let Some(g) = gamefiles() else { return };
    let files: Vec<PathBuf> = SETS.iter().map(|(f, _)| g.join(f)).collect();
    cross_check(&files, "piccont", |d| J::arr(pic::decode_set(V, d).unwrap().iter().map(pic_json)));
}
