mod common;
use common::gamefiles;
use mt_engine::data::bin::Error;
use mt_engine::data::world::{wrle_dec, wrle_enc, Cell};
use mt_engine::data::{bin::Rd, chars, dict, exepack, itm, ly, msg, pal, pic, sav, stp, vmf, world, Variant};
use std::panic::{catch_unwind, AssertUnwindSafe};

const V: Variant = Variant::Mt1;
type Dec = fn(&[u8]) -> bool;
fn codec(name: &str) -> Option<Dec> {
    let n = name.to_uppercase();
    Some(match n.as_str() {
        "MAP.PAL" => |d| pal::decode(d).is_ok(),
        "TRAV.DAT" => |d| dict::decode(d).is_ok(),
        "TRAV.MSG" => |d| msg::decode(V, d).is_ok(),
        "TRAV.ITM" => |d| itm::decode(V, d).is_ok(),
        "DCHARS.DAT" | "UCHARS.DAT" => |d| chars::decode(V, d).is_ok(),
        "W.DAT" => |d| world::decode(V, d).is_ok(),
        "OBJ.DAT" | "SPOBJ.DAT" | "X.DAT" | "Y.DAT" | "Z.DAT" | "C.DAT" => |d| pic::decode_set(V, d).is_ok(),
        "STARTUP.EXE" | "T.DAT" | "S.DAT" | "CG.DAT" => |d| exepack::unpack(d).is_ok(),
        _ => match n.rsplit('.').next().unwrap_or("") {
            "PIC" => |d| pic::decode(V, d).is_ok(),
            "LY" => |d| ly::decode(d).is_ok(),
            "STP" => |d| stp::decode(d).is_ok(),
            "VMF" => |d| vmf::decode(d).is_ok(),
            "SAV" => |d| sav::decode(V, d).is_ok(),
            _ => return None,
        },
    })
}
fn lengths(len: usize) -> Vec<usize> {
    if len <= 16 * 1024 { return (0..=len).collect(); }
    (0..len).step_by(997).chain(len.saturating_sub(64)..=len).collect()
}

#[test]
fn decoders_never_panic_on_hostile_input() {
    let Some(g) = gamefiles() else { return };
    let mut files: Vec<_> = std::fs::read_dir(&g).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_file()).collect();
    files.sort();
    let (mut seen, mut panics) = (0, Vec::new());
    for p in &files {
        let name = p.file_name().unwrap().to_string_lossy().into_owned();
        let Some(dec) = codec(&name) else { continue };
        let d = std::fs::read(p).unwrap();
        let mut run = |what: String, b: &[u8]| if catch_unwind(AssertUnwindSafe(|| dec(b))).is_err() { panics.push(format!("{name}: {what}")); };
        let ls = lengths(d.len());
        for &n in &ls { run(format!("prefix {n}"), &d[..n]); }
        let hdr = d.len().min(32);
        for i in 0..hdr { for v in [0x00, 0xff, d[i] ^ 0x80] { let mut m = d.clone(); m[i] = v; run(format!("byte {i} = {v:#04x}"), &m); } }
        println!("HOSTILE PASS {name} ({} prefixes, {} flips)", ls.len(), 3 * hdr);
        if !name.eq_ignore_ascii_case("UCHARS.DAT") { seen += 1; }
    }
    assert!(panics.is_empty(), "decoder panics:\n{}", panics.join("\n"));
    assert_eq!(seen, 64, "expected the 60 data files + 4 programs");
}

#[test]
fn msg_token_offset_past_end_is_err() {
    assert!(matches!(msg::decode(V, &[0x02, 0x00, 0x01, 0x00, 0x0c, 0x00, 0x00, 0x00]), Err(Error::Short { .. })));
    assert!(matches!(Rd { d: &[1, 2], p: 9 }.u16(), Err(Error::Short { at: 9, need: 2, have: 0 })));
}
#[test]
fn wrle_run_limits_match_oracle() {
    let mut o = vec![]; wrle_enc(&vec![[5u8, 5]; 32768], &mut o).unwrap();
    assert_eq!(o, [0x00, 0x80, 5, 5]);
    assert_eq!(wrle_dec(&mut Rd::new(&o), 32768).unwrap(), vec![[5u8, 5]; 32768]);
    assert_eq!(wrle_enc(&vec![[5u8, 5]; 32769], &mut vec![]), Err(Error::Range { what: "wrle run", got: 32769, bits: 16 }));
    let alt: Vec<Cell> = (0..32769u32).map(|i| [(i & 1) as u8, 0]).collect();
    assert!(matches!(wrle_enc(&alt, &mut vec![]), Err(Error::Range { what: "wrle literal count", got: 32768, bits: 16 })));
}
#[test]
fn encode_counts_never_wrap() {
    assert_eq!(chars::encode(V, &vec![chars::Char::zero(); 256]), Err(Error::Range { what: "chars count", got: 256, bits: 8 }));
    assert_eq!(chars::encode(V, &vec![chars::Char::zero(); 255]).unwrap().len(), 1 + 256 * 255);
}
