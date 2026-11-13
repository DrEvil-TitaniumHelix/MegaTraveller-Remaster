#![allow(dead_code)]
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn gamefiles() -> Option<PathBuf> {
    match std::env::var_os("MT1_GAMEFILES") {
        Some(p) => Some(PathBuf::from(p)),
        None => { println!("SKIP: MT1_GAMEFILES unset"); None }
    }
}
pub fn workspace() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap() }
pub fn oracle() -> PathBuf { std::env::var_os("MT1_ORACLE").map(PathBuf::from).unwrap_or_else(|| workspace().join("../MegaTraveller")) }
pub fn oracle_json(helper_py: &str, format: &str, file: &Path) -> Option<String> {
    let tools = oracle().join("tools");
    if !tools.join("data_formats.py").is_file() { println!("SKIP: oracle not found at {}", tools.display()); return None; }
    let helper = workspace().join("tools").join(helper_py);
    let out = match Command::new("python").arg(&helper).arg(&tools).arg(format).arg(file).output() {
        Ok(o) => o,
        Err(e) => { println!("SKIP: python not runnable: {e}"); return None; }
    };
    assert!(out.status.success(), "{} failed on {} {}:\n{}", helper_py, format, file.display(), String::from_utf8_lossy(&out.stderr));
    Some(String::from_utf8(out.stdout).unwrap().trim_end().to_string())
}
pub fn files_with_suffix(dir: &Path, suffix: &str) -> Vec<PathBuf> {
    let mut v: Vec<PathBuf> = std::fs::read_dir(dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.is_file() && p.file_name().unwrap().to_string_lossy().to_uppercase().ends_with(suffix)).collect();
    v.sort(); v
}

#[derive(Debug, Clone, PartialEq)]
pub enum J { Obj(Vec<(String, J)>), Arr(Vec<J>), Int(i64), Str(String), Hex(Vec<u8>) }
impl J {
    pub fn obj(kv: Vec<(&str, J)>) -> J { J::Obj(kv.into_iter().map(|(k, v)| (k.to_string(), v)).collect()) }
    pub fn arr<T: Into<J>>(it: impl IntoIterator<Item = T>) -> J { J::Arr(it.into_iter().map(Into::into).collect()) }
    pub fn write(&self, o: &mut String) {
        match self {
            J::Int(i) => o.push_str(&i.to_string()),
            J::Str(s) => write_str(s, o),
            J::Hex(b) => { o.push('"'); for x in b { o.push_str(&format!("{x:02x}")); } o.push('"'); }
            J::Arr(a) => { o.push('['); for (i, v) in a.iter().enumerate() { if i > 0 { o.push(','); } v.write(o); } o.push(']'); }
            J::Obj(kv) => {
                let mut kv: Vec<&(String, J)> = kv.iter().collect(); kv.sort_by(|a, b| a.0.cmp(&b.0));
                o.push('{');
                for (i, (k, v)) in kv.iter().enumerate() { if i > 0 { o.push(','); } write_str(k, o); o.push(':'); v.write(o); }
                o.push('}');
            }
        }
    }
    pub fn to_string(&self) -> String { let mut s = String::new(); self.write(&mut s); s }
}
fn write_str(s: &str, o: &mut String) {
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"), '\u{8}' => o.push_str("\\b"), '\u{c}' => o.push_str("\\f"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => { let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { o.push_str(&format!("\\u{u:04x}")); } }
            c => o.push(c),
        }
    }
    o.push('"');
}
macro_rules! int_from { ($($t:ty),*) => { $(impl From<$t> for J { fn from(v: $t) -> J { J::Int(v as i64) } })* } }
int_from!(u8, i8, u16, i16, u32, i32, i64, usize);
impl From<&[u8]> for J { fn from(b: &[u8]) -> J { J::Hex(b.to_vec()) } }
impl From<Vec<u8>> for J { fn from(b: Vec<u8>) -> J { J::Hex(b) } }
impl<const N: usize> From<[u8; N]> for J { fn from(b: [u8; N]) -> J { J::Hex(b.to_vec()) } }
impl From<&str> for J { fn from(s: &str) -> J { J::Str(s.to_string()) } }
impl From<String> for J { fn from(s: String) -> J { J::Str(s) } }
impl<T: Into<J>> From<Vec<T>> for J where Vec<T>: NotBytes { fn from(v: Vec<T>) -> J { J::arr(v) } }
pub trait NotBytes {}
macro_rules! not_bytes { ($($t:ty),*) => { $(impl NotBytes for Vec<$t> {})* } }
not_bytes!(i8, u16, i16, u32, i32, i64, usize, J, String, &str, Vec<u8>, Vec<i16>, Vec<u16>, Vec<J>);

#[test]
fn canonical_json_matches_python_shape() {
    let v = J::obj(vec![("z", J::from(1u8)), ("a", J::from(vec![-1i16, 2])), ("h", J::from(&[0xabu8, 0][..])), ("s", J::from("a\"\r\n\u{1}é"))]);
    assert_eq!(v.to_string(), "{\"a\":[-1,2],\"h\":\"ab00\",\"s\":\"a\\\"\\r\\n\\u0001\\u00e9\",\"z\":1}");
}
