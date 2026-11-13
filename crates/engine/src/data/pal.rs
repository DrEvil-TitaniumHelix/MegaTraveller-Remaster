use super::bin::{Error, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pal { pub vals: Vec<u8>, pub sep: String, pub end: String }

pub fn decode(d: &[u8]) -> Result<Pal> {
    if !d.is_ascii() { return Err(Error::Bad("pal: not ascii")); }
    let t = std::str::from_utf8(d).unwrap();
    let vals = t.split_ascii_whitespace().map(|x| x.parse().map_err(|_| Error::Bad("pal: value"))).collect::<Result<Vec<u8>>>()?;
    let first = vals.first().ok_or(Error::Bad("pal: empty"))?.to_string().len();
    let sep = t.get(first..first + 1).unwrap_or("").to_string();
    let end = t[t.trim_end().len()..].to_string();
    Ok(Pal { vals, sep, end })
}
pub fn encode(p: &Pal) -> Vec<u8> {
    let mut s = p.vals.iter().map(|v| v.to_string()).collect::<Vec<_>>().join(&p.sep);
    s.push_str(&p.end); s.into_bytes()
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip_synthetic() {
        let d = b"0 5 63\r\n";
        let p = decode(d).unwrap();
        assert_eq!(p, Pal { vals: vec![0, 5, 63], sep: " ".into(), end: "\r\n".into() });
        assert_eq!(encode(&p), d);
        assert!(decode(b"1 x").is_err());
    }
}
