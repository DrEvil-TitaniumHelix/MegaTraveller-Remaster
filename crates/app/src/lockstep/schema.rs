use serde_json::{Map, Value};

pub fn schema() -> Value {
    serde_json::from_str(include_str!("../../../../docs/lockstep/state.schema.json"))
        .expect("reviewed schema")
}
pub fn node(s: &Value) -> &Value {
    s.get("anyOf")
        .and_then(Value::as_array)
        .map_or(s, |a| &a[0])
}
pub fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
pub fn unhex(s: &str) -> Result<Vec<u8>, String> {
    if !s.is_ascii() || s.len() % 2 != 0 {
        return Err("invalid hex bytes".into());
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).map_err(|_| "invalid hex bytes".into()))
        .collect()
}
fn offset(s: &Value) -> Option<usize> {
    let text = s.get("x-src")?.as_str()?.split_once('+')?.1;
    if let Some(h) = text.strip_prefix("0x") {
        usize::from_str_radix(h, 16).ok()
    } else {
        text.parse().ok()
    }
}
fn ty(s: &Value) -> (&str, usize) {
    let t = s["x-type"].as_str().expect("field type");
    t.split_once('[').map_or((t, 1), |(a, b)| {
        (a, b.trim_end_matches(']').parse().expect("array size"))
    })
}
fn width(t: &str) -> usize {
    match t {
        "u16" | "s16" => 2,
        "u32" | "far" => 4,
        _ => 1,
    }
}
fn integer(b: &[u8], t: &str) -> Value {
    let mut n = 0u32;
    for (i, x) in b.iter().enumerate() {
        n |= (*x as u32) << (8 * i)
    }
    match t {
        "s8" => (n as u8 as i8).into(),
        "s16" => (n as u16 as i16).into(),
        _ => n.into(),
    }
}
pub fn decode(s: &Value, b: &[u8]) -> Value {
    let mut out = Map::new();
    let mut tails = std::collections::BTreeMap::<String, Value>::new();
    for (name, f) in node(s)["properties"].as_object().expect("struct schema") {
        let value = match name.as_str() {
            "chars" => Value::Array(
                (0..8)
                    .map(|i| decode(&f["items"], &b[0x102 + 256 * i..][..256]))
                    .collect(),
            ),
            "objects" => Value::Array(
                (0..256)
                    .map(|i| decode(&f["items"], &b[0xa82 + 5 * i..][..5]))
                    .collect(),
            ),
            "raw" => {
                let mut raw = Map::new();
                for (n, f) in f["properties"].as_object().expect("raw properties") {
                    let at = offset(f).expect("raw offset");
                    let (_, nbytes) = ty(f);
                    raw.insert(n.clone(), hex(&b[at..at + nbytes]).into());
                }
                for (n, v) in &tails {
                    raw.insert(n.clone(), v.clone());
                }
                Value::Object(raw)
            }
            _ => {
                let Some(at) = offset(f) else { continue };
                let (t, n) = ty(f);
                let w = width(t);
                let bytes = &b[at..at + w * n];
                match t {
                    "bytes" => hex(bytes).into(),
                    "cstr" => {
                        let end = bytes.iter().position(|&x| x == 0).unwrap_or(n);
                        if end < n && bytes[end + 1..].iter().any(|&x| x != 0) {
                            tails.insert(name.clone(), Value::String(hex(bytes)));
                        }
                        bytes[..end]
                            .iter()
                            .map(|&x| x as char)
                            .collect::<String>()
                            .into()
                    }
                    "far" => format!(
                        "{:04x}:{:04x}",
                        u16::from_le_bytes([bytes[2], bytes[3]]),
                        u16::from_le_bytes([bytes[0], bytes[1]])
                    )
                    .into(),
                    _ if f["type"] == "array" => {
                        Value::Array(bytes.chunks_exact(w).map(|b| integer(b, t)).collect())
                    }
                    _ => integer(bytes, t),
                }
            }
        };
        out.insert(name.clone(), value);
    }
    Value::Object(out)
}

#[derive(Clone, Debug)]
pub struct Field {
    pub path: String,
    pub offset: usize,
    pub width: usize,
    pub text: bool,
    pub hex: bool,
}
pub fn fields(s: &Value, prefix: &str, base: usize) -> Vec<Field> {
    let mut out = vec![];
    for (name, f) in node(s)["properties"].as_object().expect("struct schema") {
        let p = format!("{prefix}.{name}");
        match name.as_str() {
            "chars" | "objects" => {
                let (n, start, size) = if name == "chars" {
                    (8, 0x102, 256)
                } else {
                    (256, 0xa82, 5)
                };
                for i in 0..n {
                    out.extend(fields(
                        &f["items"],
                        &format!("{p}[{i}]"),
                        base + start + i * size,
                    ));
                }
            }
            "raw" if f["type"] == "object" => out.extend(fields(f, &p, base)),
            _ => {
                if let Some(at) = offset(f) {
                    let (t, n) = ty(f);
                    let w = width(t);
                    if f["type"] == "array" {
                        for i in 0..n {
                            out.push(Field {
                                path: format!("{p}[{i}]"),
                                offset: base + at + w * i,
                                width: w,
                                text: false,
                                hex: false,
                            });
                        }
                    } else {
                        out.push(Field {
                            path: p,
                            offset: base + at,
                            width: w * n,
                            text: t == "cstr",
                            hex: t == "bytes",
                        });
                    }
                }
            }
        }
    }
    out
}
pub fn at<'a>(v: &'a Value, path: &str) -> Option<&'a Value> {
    let mut v = v;
    for part in path.split(['.', '[', ']']).filter(|s| !s.is_empty()) {
        v = if v.is_array() {
            v.get(part.parse::<usize>().ok()?)?
        } else {
            v.get(part)?
        };
    }
    Some(v)
}
pub fn byte(v: &Value, target: &str) -> Option<u8> {
    let (path, k) = target
        .rsplit_once('+')
        .map_or(Some((target, 0)), |(a, b)| {
            Some((a, b.parse::<usize>().ok()?))
        })?;
    let v = at(v, path)?;
    if let Some(n) = v.as_i64() {
        return (k < 4).then(|| ((n as u32) >> (8 * k)) as u8);
    }
    let s = v.as_str()?;
    if path.ends_with("name") {
        return Some(s.chars().nth(k).map_or(0, |c| c as u8));
    }
    unhex(s).ok()?.get(k).copied()
}
pub fn encode_party(s: &Value, value: &Value) -> Result<Vec<u8>, String> {
    let mut b = vec![0; 0xf86];
    let mut owners = vec![0u8; 0xf86];
    let root = Value::Object(Map::from_iter([("party".into(), value.clone())]));
    for f in fields(s, "party", 0) {
        let val = at(&root, &f.path).ok_or_else(|| format!("missing {}", f.path))?;
        let bytes = if f.text {
            let (parent, name) = f.path.rsplit_once('.').unwrap();
            if let Some(raw) = at(&root, &format!("{parent}.raw.{name}")) {
                unhex(raw.as_str().ok_or("raw string")?)?
            } else {
                let mut bytes = vec![0; f.width];
                for (i, c) in val.as_str().ok_or("cstr")?.chars().enumerate() {
                    if i >= bytes.len() || c as u32 > 255 {
                        return Err("cstr range".into());
                    }
                    bytes[i] = c as u8
                }
                bytes
            }
        } else if f.hex {
            unhex(val.as_str().ok_or("hex string")?)?
        } else {
            (val.as_i64().ok_or("integer")? as u32).to_le_bytes()[..f.width].to_vec()
        };
        if bytes.len() != f.width {
            return Err("field width".into());
        }
        b[f.offset..f.offset + f.width].copy_from_slice(&bytes);
        for n in &mut owners[f.offset..f.offset + f.width] {
            *n += 1
        }
    }
    if owners.iter().any(|&n| n != 1) {
        return Err("party schema does not tile block".into());
    }
    Ok(b)
}

pub fn sha1(bytes: &[u8]) -> String {
    let mut b = bytes.to_vec();
    let len = (b.len() as u64) * 8;
    b.push(0x80);
    while b.len() % 64 != 56 {
        b.push(0)
    }
    b.extend_from_slice(&len.to_be_bytes());
    let mut h = [
        0x67452301u32,
        0xefcdab89,
        0x98badcfe,
        0x10325476,
        0xc3d2e1f0,
    ];
    for block in b.chunks_exact(64) {
        let mut w = [0u32; 80];
        for (i, b) in block.chunks_exact(4).enumerate() {
            w[i] = u32::from_be_bytes(b.try_into().unwrap())
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1)
        }
        let [mut a, mut b, mut c, mut d, mut e] = h;
        for (i, w) in w.into_iter().enumerate() {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5a827999),
                20..=39 => (b ^ c ^ d, 0x6ed9eba1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8f1bbcdc),
                _ => (b ^ c ^ d, 0xca62c1d6),
            };
            let t = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = t;
        }
        for (h, v) in h.iter_mut().zip([a, b, c, d, e]) {
            *h = h.wrapping_add(v)
        }
    }
    h.iter().map(|v| format!("{v:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_party_bytes_and_cstr_tails_roundtrip() {
        let s = schema();
        let s = &s["properties"]["party"];
        let mut b: Vec<u8> = (0..0xf86).map(|i| ((i * 73 + i / 7) % 256) as u8).collect();
        b[0x8d] = 0;
        b[0x103] = 0;
        assert_eq!(encode_party(s, &decode(s, &b)).unwrap(), b);
        let sav = mt_engine::data::sav::decode(mt_engine::data::Variant::Mt1, &b).unwrap();
        assert_eq!(
            mt_engine::data::sav::encode(mt_engine::data::Variant::Mt1, &sav).unwrap(),
            b
        );
    }
    #[test]
    fn known_sha1_vectors() {
        assert_eq!(sha1(b""), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
        assert_eq!(sha1(b"abc"), "a9993e364706816aba3e25717850c26c9cd0d89d");
        assert_eq!(
            sha1(&vec![b'a'; 1000]),
            "291e9a6c66994949b57ba5e650361e98fc36b1ba"
        );
    }
    #[test]
    fn symbolic_fields_change_only_their_documented_bytes() {
        let s = schema();
        let s = &s["properties"]["party"];
        let mut party = decode(s, &vec![0; 0xf86]);
        party["credits"] = 0x12345678u32.into();
        party["chars"][3]["inventory"][4] = 0x1234.into();
        let b = encode_party(s, &party).unwrap();
        let expected = [
            (0x88, 0x78),
            (0x89, 0x56),
            (0x8a, 0x34),
            (0x8b, 0x12),
            (0x4d6, 0x34),
            (0x4d7, 0x12),
        ];
        assert_eq!(
            b.iter()
                .copied()
                .enumerate()
                .filter(|(_, v)| *v != 0)
                .collect::<Vec<_>>(),
            expected
        );
        let record = serde_json::json!({"party":party});
        assert_eq!(byte(&record, "party.credits+2"), Some(0x34));
        assert_eq!(byte(&record, "party.chars[3].inventory[4]+1"), Some(0x12));
        assert_eq!(byte(&record, "party.credits+4"), None);
    }
}
