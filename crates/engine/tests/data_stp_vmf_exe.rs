mod common;
use common::{files_with_suffix, gamefiles, oracle_json, J};
use mt_engine::data::{exepack, stp, vmf};
use std::path::Path;

const HELPER: &str = "m1_dump_stp_vmf_exe.py";
const PROGRAMS: [&str; 4] = ["STARTUP.EXE", "T.DAT", "S.DAT", "CG.DAT"];

fn stp_json(s: &stp::Stp) -> J {
    J::obj(vec![("default_x", s.default_x.into()), ("default_y", s.default_y.into()),
        ("objects", J::arr(s.objects.iter().map(|o| J::obj(vec![
            ("world", o.world.into()), ("orbit", o.orbit.into()), ("phase", o.phase.into()),
            ("angle", o.angle.into()), ("moons", o.moons.into()), ("sprite", o.sprite.into()),
        ]))))])
}
fn vmf_json(v: &vmf::Vmf) -> J {
    let h = &v.hdr;
    J::obj(vec![("hdr", J::obj(vec![
        ("length", h.length.into()), ("length_hi", h.length_hi.into()), ("b3", h.b3.into()), ("b4", h.b4.into()),
        ("zero", h.zero.into()), ("magic", h.magic.into()),
    ])), ("pcm", v.pcm.clone().into())])
}
fn compare(format: &str, file: &Path, actual: J) -> bool {
    let Some(expected) = oracle_json(HELPER, format, file) else { return false; };
    let actual = actual.to_string();
    assert!(actual == expected, "{} oracle mismatch at JSON byte {:?}", file.display(),
        actual.bytes().zip(expected.bytes()).position(|(a, b)| a != b).or_else(||
            (actual.len() != expected.len()).then_some(actual.len().min(expected.len()))));
    println!("CROSS-CHECK PASS {}", file.file_name().unwrap().to_string_lossy()); true
}

#[test]
fn stp_round_trip() {
    let Some(dir) = gamefiles() else { return; };
    let files = files_with_suffix(&dir, ".STP"); assert_eq!(files.len(), 8);
    for file in files {
        let d = std::fs::read(&file).unwrap();
        assert!(stp::encode(&stp::decode(&d).unwrap()).unwrap() == d, "{} round trip", file.display());
        println!("PASS {}", file.file_name().unwrap().to_string_lossy());
    }
}
#[test]
fn stp_cross_check() {
    let Some(dir) = gamefiles() else { return; };
    let files = files_with_suffix(&dir, ".STP"); assert_eq!(files.len(), 8);
    for file in files {
        let s = stp::decode(&std::fs::read(&file).unwrap()).unwrap();
        if !compare("STP", &file, stp_json(&s)) { return; }
    }
}
#[test]
fn vmf_round_trip() {
    let Some(dir) = gamefiles() else { return; };
    let files = files_with_suffix(&dir, ".VMF"); assert_eq!(files.len(), 19);
    for file in files {
        let d = std::fs::read(&file).unwrap();
        assert!(vmf::encode(&vmf::decode(&d).unwrap()).unwrap() == d, "{} round trip", file.display());
        println!("PASS {}", file.file_name().unwrap().to_string_lossy());
    }
}
#[test]
fn vmf_cross_check() {
    let Some(dir) = gamefiles() else { return; };
    let files = files_with_suffix(&dir, ".VMF"); assert_eq!(files.len(), 19);
    for file in files {
        let v = vmf::decode(&std::fs::read(&file).unwrap()).unwrap();
        if !compare("VMF", &file, vmf_json(&v)) { return; }
    }
}
#[test]
fn exepack_cross_check() {
    let Some(dir) = gamefiles() else { return; };
    for name in PROGRAMS {
        let file = dir.join(name);
        let u = exepack::unpack(&std::fs::read(&file).unwrap()).unwrap();
        if !compare("EXEPACK", &file, J::arr([J::from(u.exe), u.relocations.into(), u.prefix_end.into()])) { return; }
    }
}

#[test]
fn stp_fields_and_bounds() {
    let d = [1, 254, 253, 252, 255, 250, 249, 248, 247];
    let s = stp::decode(&d).unwrap();
    assert_eq!(s, stp::Stp { default_x: 254, default_y: 253,
        objects: vec![stp::Object { world: 252, orbit: 255, phase: 250, angle: 249, moons: 248, sprite: 247 }] });
    assert_eq!(stp::encode(&s).unwrap(), d);
    for end in 0..d.len() { assert!(stp::decode(&d[..end]).is_err()); }
    assert!(stp::decode(&[0, 1, 2, 3]).is_err());
    let empty = stp::decode(&[0, 1, 2]).unwrap();
    assert_eq!(stp::encode(&empty).unwrap(), [0, 1, 2]);
    let mut large = stp::Stp { objects: vec![s.objects[0].clone(); 255], ..empty };
    assert_eq!(stp::decode(&stp::encode(&large).unwrap()).unwrap(), large);
    large.objects.push(s.objects[0].clone()); assert!(stp::encode(&large).is_err());
}

#[test]
fn vmf_opaque_header_and_u24_length() {
    let mut d = vec![0x80; 128 + 65539];
    d[..5].copy_from_slice(&[3, 0, 1, 0xfe, 0xfd]);
    d[5..119].fill(0xa5); d[119..128].copy_from_slice(b"GEVATOSKY");
    let mut v = vmf::decode(&d).unwrap();
    assert_eq!((v.hdr.length, v.hdr.length_hi, v.hdr.b3, v.hdr.b4), (3, 1, 0xfe, 0xfd));
    assert_eq!(v.hdr.zero, [0xa5; 114]); assert_eq!(v.pcm, vec![0x80; 65539]);
    assert_eq!(vmf::encode(&v).unwrap(), d);
    for end in 0..128 { assert!(vmf::decode(&d[..end]).is_err()); }
    assert!(vmf::decode(&d[..d.len() - 1]).is_err());
    v.hdr.length = 4; assert!(vmf::encode(&v).is_err());
    d[119] = b'X'; assert!(vmf::decode(&d).is_err());
    v.pcm.clear(); v.hdr.length = 0; v.hdr.length_hi = 0;
    assert_eq!(vmf::decode(&vmf::encode(&v).unwrap()).unwrap(), v);
    v.hdr.magic[0] = b'X'; assert!(vmf::encode(&v).is_err());
}

fn set_word(d: &mut [u8], at: usize, n: u16) { d[at..at + 2].copy_from_slice(&n.to_le_bytes()); }
fn packed(stream: &[u8], paragraphs: u16, signature_at: usize) -> Vec<u8> {
    assert!(stream.len() <= 16);
    let mut d = vec![0; 48 + signature_at + 2];
    d[..2].copy_from_slice(b"MZ"); set_word(&mut d, 8, 2); set_word(&mut d, 10, 0x1234);
    set_word(&mut d, 0x16, 1);
    d[32..48].fill(0xff); d[32..32 + stream.len()].copy_from_slice(stream);
    for (i, v) in [0x4567, 0x0089, 0, 0, 0xabcd, 0x0123, paragraphs].into_iter().enumerate() {
        set_word(&mut d, 48 + 2 * i, v);
    }
    d[48 + signature_at..].copy_from_slice(b"RB");
    d.extend_from_slice(b"Packed file is corrupt");
    for seg in 0..16 {
        match seg {
            0 => d.extend_from_slice(&[2, 0, 2, 0, 4, 0]),
            15 => d.extend_from_slice(&[1, 0, 0x20, 0]),
            _ => d.extend_from_slice(&[0, 0]),
        }
    }
    d
}

#[test]
fn exepack_commands_prefix_header_and_relocations() {
    for signature_at in [14, 16] {
        let d = packed(b"HEAD\xaa\x19\0\xb1\x31\x42\x53\x03\0\xb2", 2, signature_at);
        let u = exepack::unpack(&d).unwrap();
        assert_eq!((u.relocations, u.prefix_end, u.exe.len()), (3, 4, 80));
        assert_eq!(&u.exe[..28], &[
            b'M', b'Z', 80, 0, 1, 0, 3, 0, 3, 0, 0x34, 0x12, 0xff, 0xff,
            0x23, 0x01, 0xcd, 0xab, 0, 0, 0x67, 0x45, 0x89, 0, 0x1c, 0, 0, 0,
        ]);
        assert_eq!(&u.exe[28..40], &[2, 0, 0, 0, 4, 0, 0, 0, 0x20, 0, 0, 0xf0]);
        assert_eq!(&u.exe[40..48], &[0; 8]);
        assert_eq!(&u.exe[48..52], b"HEAD"); assert_eq!(&u.exe[52..77], &[0xaa; 25]);
        assert_eq!(&u.exe[77..], &[0x31, 0x42, 0x53]);
    }
    let u = exepack::unpack(&packed(b"HEADxyz\x03\0\xb3\xaa\x19\0\xb0", 2, 14)).unwrap();
    assert_eq!(&u.exe[48..55], b"HEADxyz"); assert_eq!(&u.exe[55..], &[0xaa; 25]);
    let d = packed(b"\0\0\xb3", 2, 14);
    let u = exepack::unpack(&d).unwrap();
    assert_eq!(u.prefix_end, 32); assert_eq!(&u.exe[48..64], &d[32..48]); assert_eq!(&u.exe[64..], &[0; 16]);
    let u = exepack::unpack(&packed(b"\0\0\xb3", 29, 14)).unwrap();
    assert_eq!(u.exe.len(), 512); assert_eq!(&u.exe[2..6], &[0, 0, 1, 0]);
}

#[test]
fn exepack_rejects_malformed_input() {
    let good = packed(b"HEAD\xaa\x19\0\xb1\x31\x42\x53\x03\0\xb2", 2, 14);
    for end in 0..good.len() { assert!(exepack::unpack(&good[..end]).is_err(), "truncated at {end}"); }
    for (at, value) in [(0, b'N'), (8, 0xff), (0x16, 0xff), (48 + 14, b'X'), (48 + 16, b'X')] {
        let mut bad = good.clone(); bad[at] = value; assert!(exepack::unpack(&bad).is_err(), "offset {at}");
    }
    for stream in [
        &b""[..],
        &b"\x01\0\xb1"[..],
        &b"\x01\0\xb3"[..],
        &b"\xaa\x21\0\xb1"[..],
        &b"\xaa\x01\0\xb4"[..],
        &b"\xaa\x01\0\xb0"[..],
        &b"HEAD\xaa\x1d\0\xb1"[..],
    ] { assert!(exepack::unpack(&packed(stream, 2, 14)).is_err(), "stream {stream:?}"); }
    assert!(exepack::unpack(&packed(b"\0\0\xb3", 0, 14)).is_err());
}
