use super::{mouse::MouseEvent, Key};
use std::{fs, io, path::Path};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Text { Message(String), Text(String), Menu(Vec<String>) }
pub struct Step<'a> {
    pub key: Option<Key>, pub ticks: Option<u32>, pub mouse: Option<MouseEvent>, pub now: u32,
    pub rand: &'a [u16], pub rand_state: u32, pub text: &'a [Text], pub screen: &'a str, pub planes: &'a [u8], pub regs: &'a [u8], pub record: &'a [u8],
    pub vars: &'a [(&'a str, i64)], pub files: &'a [(&'a str, &'a [u8])],
}
impl<'a> Step<'a> {
    pub fn cg(key: Option<Key>, rand: &'a [u16], rand_state: u32, text: &'a [Text], screen: &'a str, planes: &'a [u8], regs: &'a [u8], record: &'a [u8]) -> Step<'a> {
        Step { key, ticks: None, mouse: None, now: 0, rand, rand_state, text, screen, planes, regs, record, vars: &[], files: &[] }
    }
}

pub fn json_str(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""), '\\' => o.push_str("\\\\"), '\n' => o.push_str("\\n"), '\r' => o.push_str("\\r"), '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => { let mut b = [0u16; 2]; for u in c.encode_utf16(&mut b) { o.push_str(&format!("\\u{u:04x}")) } }
            c => o.push(c),
        }
    }
    o.push('"'); o
}
pub fn step_json(s: &Step) -> String {
    let text: Vec<String> = s.text.iter().map(|t| match t {
        Text::Message(m) => format!("[\"message\",{}]", json_str(m)), Text::Text(m) => format!("[\"text\",{}]", json_str(m)),
        Text::Menu(items) => format!("[\"menu\",[{}]]", items.iter().map(|i| json_str(i)).collect::<Vec<_>>().join(",")),
    }).collect();
    let rand: Vec<String> = s.rand.iter().map(|v| v.to_string()).collect();
    let vars: Vec<String> = s.vars.iter().map(|(n, v)| format!("{}:{v}", json_str(n))).collect();
    format!("{{\"key\":{},\"rand\":[{}],\"rand_state\":{},\"text\":[{}],\"screen\":{},\"now\":{},\"ticks\":{},\"mouse\":{},\"vars\":{{{}}}}}\n",
        s.key.map_or("null".into(), |k| k.to_string()), rand.join(","), s.rand_state, text.join(","), json_str(s.screen), s.now,
        s.ticks.map_or("null".into(), |t| t.to_string()), s.mouse.map_or("null".into(), |m| format!("[{},{},{}]", m.col, m.row, m.event)), vars.join(","))
}
pub fn write_step(dir: &Path, n: usize, s: &Step) -> io::Result<()> {
    let d = dir.join(format!("step_{n:04}"));
    fs::create_dir_all(&d)?;
    fs::write(d.join("planes.bin"), s.planes)?; fs::write(d.join("regs.bin"), s.regs)?; fs::write(d.join("work.bin"), s.record)?;
    for (name, bytes) in s.files { fs::write(d.join(name), bytes)? }
    fs::write(d.join("step.json"), step_json(s))
}
pub fn write_end(dir: &Path, files: &[(&str, &[u8])]) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for (n, b) in files { fs::write(dir.join(n), b)? }
    Ok(())
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn json_and_files() {
        let text = [Text::Message("Do you wish?".into()), Text::Text("\u{e9}\"x".into()), Text::Menu(vec!["Yes".into(), "No".into()])];
        let s = Step::cg(None, &[1, 32767], 4294967295, &text, "cg", &[1, 2], &[3], &[4, 5, 6]);
        assert_eq!(step_json(&s), "{\"key\":null,\"rand\":[1,32767],\"rand_state\":4294967295,\"text\":[[\"message\",\"Do you wish?\"],[\"text\",\"\\u00e9\\\"x\"],[\"menu\",[\"Yes\",\"No\"]]],\"screen\":\"cg\",\"now\":0,\"ticks\":null,\"mouse\":null,\"vars\":{}}\n");
        let d = std::env::temp_dir().join(format!("mt-dump-{}", std::process::id()));
        write_step(&d, 7, &Step { key: Some(0x150), ..s }).unwrap();
        assert_eq!(fs::read(d.join("step_0007/work.bin")).unwrap(), [4, 5, 6]); assert_eq!(fs::read(d.join("step_0007/planes.bin")).unwrap(), [1, 2]);
        assert!(fs::read_to_string(d.join("step_0007/step.json")).unwrap().starts_with("{\"key\":336,"));
        let t = Step { key: None, ticks: Some(40), mouse: Some(MouseEvent { event: 4, buttons: 0, col: 210, row: -1 }), now: 1234, vars: &[("x", 44), ("y", -2)], files: &[("party.bin", &[9, 9][..])], ..Step::cg(None, &[], 0, &[], "t", &[], &[], &[]) };
        assert!(step_json(&t).ends_with("\"screen\":\"t\",\"now\":1234,\"ticks\":40,\"mouse\":[210,-1,4],\"vars\":{\"x\":44,\"y\":-2}}\n"));
        write_step(&d, 8, &t).unwrap(); assert_eq!(fs::read(d.join("step_0008/party.bin")).unwrap(), [9, 9]);
        write_end(&d, &[("UCHARS.DAT", &[9])]).unwrap();
        assert_eq!(fs::read(d.join("UCHARS.DAT")).unwrap(), [9]); assert!(!d.join("pool_record.bin").exists());
        fs::remove_dir_all(&d).unwrap();
    }
}
