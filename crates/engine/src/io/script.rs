use super::{mouse::MouseEvent, Key};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Line { Key(Key), Ticks(u32), Mouse(MouseEvent), Patch { offset: u16, value: u8 }, At { name: Checkpoint, tick: u32 } }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Checkpoint([u8;32]);
impl Checkpoint {
    pub fn parse(s:&str)->Option<Self> {
        if s.is_empty() || s.len()>32 || !s.bytes().all(|b|b.is_ascii_alphanumeric() || b==b'_' || b==b'-') {return None}
        let mut name=[0;32];name[..s.len()].copy_from_slice(s.as_bytes());Some(Self(name))
    }
    pub fn name(&self)->&str {std::str::from_utf8(&self.0[..self.0.iter().position(|&b|b==0).unwrap_or(32)]).unwrap_or("")}
}
pub const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_LINE_BYTES: usize = 1024;
pub const MAX_INSTRUCTIONS: usize = 65536;
fn unsigned(t: &str) -> Option<u32> {
    if t.starts_with(['-', '+']) { return None }
    t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")).map_or_else(|| t.parse().ok(), |h| u32::from_str_radix(h, 16).ok())
}
fn coordinate(t: &str) -> Option<i16> {
    let v = match t.strip_prefix('-') { Some(n) => -(unsigned(n)? as i64), None => unsigned(t)? as i64 };
    i16::try_from(v).ok()
}
pub fn lines(text: &str) -> Result<Vec<Line>, (usize, String)> { Ok(numbered(text)?.into_iter().map(|(_, l)| l).collect()) }
fn numbered(text: &str) -> Result<Vec<(usize, Line)>, (usize, String)> {
    if text.len() > MAX_BYTES { return Err((1, format!("script exceeds {MAX_BYTES} bytes"))) }
    let mut out = Vec::new();
    for (i, l) in text.lines().enumerate() {
        if l.len() > MAX_LINE_BYTES { return Err((i + 1, format!("line exceeds {MAX_LINE_BYTES} bytes"))) }
        let t = l.split('#').next().unwrap_or("").trim();
        if t.is_empty() { continue }
        if out.len() == MAX_INSTRUCTIONS { return Err((i + 1, format!("script exceeds {MAX_INSTRUCTIONS} instructions"))) }
        let bad = || (i + 1, t.to_string());
        let mut w = t.split_whitespace();
        let key = |s: &str| unsigned(s).and_then(|v| Key::try_from(v).ok()).map(Line::Key);
        let line = match (w.next(), w.next(), w.next(), w.next(), w.next()) {
            (Some(k), None, None, None, None) => key(k),
            (Some("key"), Some(k), None, None, None) => key(k),
            (Some("ticks"), Some(n), None, None, None) => unsigned(n).map(Line::Ticks),
            (Some("at"), Some(name), Some(tick), None, None) => (||Some(Line::At {name:Checkpoint::parse(name)?,tick:unsigned(tick)?}))(),
            (Some("patch"), Some(o), Some(v), None, None) => (|| Some(Line::Patch { offset: u16::try_from(unsigned(o)?).ok()?, value: u8::try_from(unsigned(v)?).ok()? }))(),
            (Some("mouse"), Some(c), Some(r), Some(e), None) => (|| Some(Line::Mouse(MouseEvent { event: u16::try_from(unsigned(e)?).ok()?, buttons: 0, col: coordinate(c)?, row: coordinate(r)? })))(),
            _ => None,
        }.ok_or_else(bad)?;
        out.push((i + 1, line));
    }
    Ok(out)
}
pub fn parse(text: &str) -> Result<Vec<Key>, (usize, String)> {
    numbered(text)?.into_iter().map(|(i, l)| match l { Line::Key(k) => Ok(k), _ => Err((i, format!("{l:?}"))) }).collect()
}
pub fn write_lines(lines: &[Line]) -> String {
    lines.iter().map(|l| match *l {
        Line::Key(k) if k >= 0x100 => format!("0x{k:X}\n"), Line::Key(k) => format!("{k}\n"),
        Line::Ticks(n) => format!("ticks {n}\n"), Line::Mouse(m) => format!("mouse {} {} 0x{:X}\n", m.col, m.row, m.event),
        Line::Patch { offset, value } => format!("patch 0x{offset:X} 0x{value:X}\n"),
        Line::At {name,tick} => format!("at {} {tick}\n",name.name()),
    }).collect()
}
pub fn write(keys: &[Key]) -> String { write_lines(&keys.iter().map(|&k| Line::Key(k)).collect::<Vec<_>>()) }
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn parse_and_write() {
        assert_eq!(parse("# new game\n13\n0x150 # down\n\n  27  \n0X14D\n").unwrap(), [13, 0x150, 27, 0x14D]);
        assert_eq!(parse("13\nx\n"), Err((2, "x".into())));
        assert_eq!(parse("70000"), Err((1, "70000".into())));
        assert_eq!(parse(""), Ok(vec![]));
        assert_eq!(parse(&write(&[13, 0x148, 71])).unwrap(), [13, 0x148, 71]);
        let l = lines("key 13\nticks 400\nmouse 210 10 4\nmouse -1 0x10 0x10 # off-screen right release\n0x148\n").unwrap();
        assert_eq!(l, [Line::Key(13), Line::Ticks(400), Line::Mouse(MouseEvent { event: 4, buttons: 0, col: 210, row: 10 }), Line::Mouse(MouseEvent { event: 0x10, buttons: 0, col: -1, row: 16 }), Line::Key(0x148)]);
        assert_eq!(lines(&write_lines(&l)).unwrap(), l);
        assert_eq!(lines("ticks"), Err((1, "ticks".into()))); assert_eq!(lines("mouse 1 2"), Err((1, "mouse 1 2".into()))); assert_eq!(lines("key 0x10000"), Err((1, "key 0x10000".into())));
        for s in ["ticks -1", "ticks --1", "ticks 4294967296", "mouse 65536 0 4", "mouse 0 -32769 4", "mouse --1 0 4"] { assert!(lines(s).is_err(), "{s}") }
        assert!(lines("mouse -32768 32767 4").is_ok());
        assert!(lines("ticks 4294967295").is_ok());
        assert_eq!(lines("ticks -40000"), Err((1, "ticks -40000".into())));
        assert!(parse("ticks 3").is_err());
        assert_eq!(parse("# intro

13

ticks 3
").unwrap_err().0, 5);
        let patches = lines("patch 0x6441 2\npatch 65535 255\n").unwrap();
        assert_eq!(lines(&write_lines(&patches)).unwrap(), patches);
        for invalid in ["patch -1 2", "patch 65536 2", "patch 4 256", "patch 4 -1"] { assert!(lines(invalid).is_err()) }
        assert!(parse("patch 4 2").is_err());
        let at=lines("at show_message 903\nat tileset_loaded 1021\n").unwrap();
        assert_eq!(lines(&write_lines(&at)).unwrap(),at);
        for invalid in ["at x -1","at x 4294967296","at x! 1","at x 1 tail"] {assert!(lines(invalid).is_err())}
        assert!(lines(&format!("at {} 1", "x".repeat(33))).is_err());
        assert!(lines(&format!("at {} 1", "x".repeat(32))).is_ok());
        assert!(parse("at show_message 1").is_err());
    }
    #[test]
    fn budgets_before_collection() {
        assert!(lines(&" ".repeat(MAX_BYTES + 1)).unwrap_err().1.contains("script exceeds"));
        assert!(lines(&" ".repeat(MAX_LINE_BYTES + 1)).unwrap_err().1.contains("line exceeds"));
        assert_eq!(lines(&"0\n".repeat(MAX_INSTRUCTIONS)).unwrap().len(), MAX_INSTRUCTIONS);
        assert!(lines(&"0\n".repeat(MAX_INSTRUCTIONS + 1)).unwrap_err().1.contains("instructions"));
        assert!(lines(&"x ".repeat(MAX_LINE_BYTES / 2)).is_err());
    }
}
