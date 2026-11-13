use std::{fs, path::{Path, PathBuf}};
use crate::gamefiles_manifest::MANIFEST;
pub const CODE: [&str; 4] = ["T.DAT", "S.DAT", "CG.DAT", "STARTUP.EXE"];
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Verdict { pub missing: Vec<String>, pub data_differ: Vec<String>, pub code_differ: Vec<String> }
impl Verdict {
    pub fn ok(&self) -> bool { self.missing.is_empty() && self.data_differ.is_empty() && self.code_differ.is_empty() }
    pub fn refuses(&self) -> bool { !self.missing.is_empty() || !self.code_differ.is_empty() }
    pub fn text(&self, dir: &Path) -> String {
        let mut s = format!("gamefiles {}: ", dir.display());
        if self.ok() { s.push_str("all 64 original files present and as in the 1990 set"); return s }
        s.push('\n');
        if !self.missing.is_empty() { s.push_str(&format!("MISSING {} of 64 files:\n  {}\n", self.missing.len(), self.missing.join(" "))); }
        for n in &self.code_differ {
            let (_, size, sha) = expected(n).unwrap_or(("", 0, ""));
            s.push_str(&format!("REFUSED: {n} is not the 1990 file the game reads its tables from.\n  required: {size} bytes, SHA-256 {sha}\n"));
        }
        for n in &self.data_differ { s.push_str(&format!("warning: different copy of {n}, the game may still work\n")); }
        if self.refuses() { s.push_str("Drop the folder of your original MegaTraveller 1 game (the folder that contains the file TRAV.COM) onto the game's window, or copy the files into the gamefiles folder beside the program."); }
        s
    }
}
pub fn expected(name: &str) -> Option<(&'static str, u64, &'static str)> { MANIFEST.iter().copied().find(|(n, _, _)| *n == name) }
pub fn find(dir: &Path, name: &str) -> Option<PathBuf> {
    fs::read_dir(dir).ok()?.flatten().map(|e| e.path()).find(|p| p.file_name().is_some_and(|f| f.to_string_lossy().eq_ignore_ascii_case(name)))
}
pub fn verify(dir: &Path) -> Verdict {
    let mut v = Verdict::default();
    for &(name, size, sha) in MANIFEST {
        match find(dir, name).and_then(|p| fs::read(p).ok()) {
            None => v.missing.push(name.into()),
            Some(b) if b.len() as u64 == size && hex(&sha256(&b)) == sha => {}
            Some(_) => if CODE.contains(&name) { v.code_differ.push(name.into()) } else { v.data_differ.push(name.into()) },
        }
    }
    v
}
pub fn find_game_dir(dropped: &Path) -> Option<PathBuf> {
    fn walk(d: &Path, depth: u8) -> Option<PathBuf> {
        if find(d, "T.DAT").is_some() { return Some(d.to_path_buf()) }
        if depth == 0 { return None }
        let mut subs: Vec<_> = fs::read_dir(d).ok()?.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect(); subs.sort();
        subs.iter().find_map(|s| walk(s, depth - 1))
    }
    let dir = if dropped.is_file() { dropped.parent()? } else { dropped };
    walk(dir, 3)
}
pub fn changes(src: &Path, dst: &Path) -> Vec<String> {
    MANIFEST.iter().filter(|(n, _, _)| {
        let a = find(src, n).and_then(|p| fs::read(p).ok()); let b = find(dst, n).and_then(|p| fs::read(p).ok());
        a.is_some() && a != b
    }).map(|(n, _, _)| n.to_string()).collect()
}
pub fn install(src: &Path, dst: &Path) -> Result<Verdict, String> {
    let v = verify(src);
    if v.refuses() { return Err(v.text(src)) }
    fs::create_dir_all(dst).map_err(|e| format!("{}: {e}", dst.display()))?;
    for &(name, _, _) in MANIFEST {
        let from = find(src, name).ok_or_else(|| format!("{name}: vanished"))?;
        fs::copy(&from, dst.join(name)).map_err(|e| format!("{name}: {e}"))?;
    }
    Ok(verify(dst))
}
pub fn hex(d: &[u8]) -> String { d.iter().map(|b| format!("{b:02x}")).collect() }
pub fn sha256(data: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [0x428a2f98,0x71374491,0xb5c0fbcf,0xe9b5dba5,0x3956c25b,0x59f111f1,0x923f82a4,0xab1c5ed5,0xd807aa98,0x12835b01,0x243185be,0x550c7dc3,0x72be5d74,0x80deb1fe,0x9bdc06a7,0xc19bf174,
        0xe49b69c1,0xefbe4786,0x0fc19dc6,0x240ca1cc,0x2de92c6f,0x4a7484aa,0x5cb0a9dc,0x76f988da,0x983e5152,0xa831c66d,0xb00327c8,0xbf597fc7,0xc6e00bf3,0xd5a79147,0x06ca6351,0x14292967,
        0x27b70a85,0x2e1b2138,0x4d2c6dfc,0x53380d13,0x650a7354,0x766a0abb,0x81c2c92e,0x92722c85,0xa2bfe8a1,0xa81a664b,0xc24b8b70,0xc76c51a3,0xd192e819,0xd6990624,0xf40e3585,0x106aa070,
        0x19a4c116,0x1e376c08,0x2748774c,0x34b0bcb5,0x391c0cb3,0x4ed8aa4a,0x5b9cca4f,0x682e6ff3,0x748f82ee,0x78a5636f,0x84c87814,0x8cc70208,0x90befffa,0xa4506ceb,0xbef9a3f7,0xc67178f2];
    let mut h: [u32; 8] = [0x6a09e667,0xbb67ae85,0x3c6ef372,0xa54ff53a,0x510e527f,0x9b05688c,0x1f83d9ab,0x5be0cd19];
    let mut m = data.to_vec(); m.push(0x80); while m.len() % 64 != 56 { m.push(0) } m.extend_from_slice(&(data.len() as u64 * 8).to_be_bytes());
    for chunk in m.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 { w[i] = u32::from_be_bytes(chunk[i*4..i*4+4].try_into().unwrap()) }
        for i in 16..64 {
            let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
            let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
            w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
        }
        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh] = h;
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let t1 = hh.wrapping_add(s1).wrapping_add((e & f) ^ (!e & g)).wrapping_add(K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let t2 = s0.wrapping_add((a & b) ^ (a & c) ^ (b & c));
            hh = g; g = f; f = e; e = d.wrapping_add(t1); d = c; c = b; b = a; a = t1.wrapping_add(t2);
        }
        for (x, y) in h.iter_mut().zip([a, b, c, d, e, f, g, hh]) { *x = x.wrapping_add(y) }
    }
    let mut out = [0u8; 32];
    for (i, x) in h.iter().enumerate() { out[i*4..i*4+4].copy_from_slice(&x.to_be_bytes()) }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    mod oracle_env { include!("../tests/oracle_env/mod.rs"); }
    fn oracle() -> Option<PathBuf> { oracle_env::gamefiles() }
    fn tmp(tag: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("mt1_gamefiles_{tag}_{}", std::process::id())); let _ = fs::remove_dir_all(&d); fs::create_dir_all(&d).unwrap(); d }
    fn flip(dir: &Path, name: &str) { let p = find(dir, name).unwrap(); let mut b = fs::read(&p).unwrap(); let mid = b.len() / 2; b[mid] ^= 1; fs::write(p, b).unwrap(); }
    #[test]
    fn sha256_vectors() {
        assert_eq!(hex(&sha256(b"")), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
        assert_eq!(hex(&sha256(b"abc")), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(hex(&sha256(&[b'a'; 1000])), "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3");
    }
    #[test]
    fn manifest_covers_required_files_once() {
        assert_eq!(MANIFEST.len(), 64);
        for (n, _, sha) in MANIFEST { assert!(crate::REQUIRED_FILES.contains(n), "{n}"); assert_eq!(sha.len(), 64); }
        for n in CODE { assert!(expected(n).is_some()) }
    }
    #[test]
    fn manifest_equals_oracle_copy() {
        let Some(dir) = oracle() else { eprintln!("SKIP manifest_equals_oracle_copy: MT1_GAMEFILES unset"); return };
        let v = verify(&dir); assert!(v.ok(), "{}", v.text(&dir));
    }
    #[test]
    fn three_outcomes_and_install() {
        let Some(src) = oracle() else { eprintln!("SKIP three_outcomes_and_install: MT1_GAMEFILES unset"); return };
        let copy = tmp("copy"); install(&src, &copy).unwrap(); assert!(verify(&copy).ok());
        fs::remove_file(copy.join("A0.PIC")).unwrap(); fs::remove_file(copy.join("WALK.VMF")).unwrap();
        let v = verify(&copy); assert_eq!(v.missing, ["A0.PIC", "WALK.VMF"]); assert!(v.refuses()); assert!(v.text(&copy).contains("MISSING 2 of 64"));
        assert!(install(&copy, &tmp("never")).is_err());
        let data = tmp("data"); install(&src, &data).unwrap(); flip(&data, "MAP.PAL");
        let v = verify(&data); assert_eq!((v.data_differ.as_slice(), v.refuses()), (&["MAP.PAL".to_string()][..], false));
        assert!(v.text(&data).contains("different copy of MAP.PAL, the game may still work"));
        let code = tmp("code"); install(&src, &code).unwrap(); flip(&code, "T.DAT");
        let v = verify(&code); assert_eq!(v.code_differ, ["T.DAT"]); assert!(v.refuses());
        let (_, size, sha) = expected("T.DAT").unwrap(); let t = v.text(&code); assert!(t.contains(&format!("{size} bytes")) && t.contains(sha));
        assert_eq!(changes(&src, &copy), ["A0.PIC", "WALK.VMF"]); assert_eq!(changes(&src, &code), ["T.DAT"]);
        assert!(changes(&src, &data).contains(&"MAP.PAL".to_string())); assert!(install(&src, &code).unwrap().ok());
        for d in [copy, data, code, tmp("never")] { let _ = fs::remove_dir_all(d); }
    }
    #[test]
    fn a_parent_folder_is_searched_for_the_game() {
        let root = tmp("parent"); let game = root.join("MegaTraveller 1 (1990)").join("MegaTra1");
        fs::create_dir_all(root.join("MegaTraveller 1 (1990)").join("Manual")).unwrap(); fs::create_dir_all(&game).unwrap();
        fs::write(game.join("t.dat"), b"x").unwrap();
        assert_eq!(find_game_dir(&root).unwrap(), game); assert_eq!(find_game_dir(&game).unwrap(), game);
        assert_eq!(find_game_dir(&game.join("t.dat")).unwrap(), game);
        assert_eq!(find_game_dir(&root.join("MegaTraveller 1 (1990)").join("Manual")), None);
        let _ = fs::remove_dir_all(root);
    }
}
