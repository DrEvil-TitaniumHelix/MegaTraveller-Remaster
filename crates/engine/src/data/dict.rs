use super::bin::{Error, Result};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dict { pub words: Vec<Vec<u8>>, pub tail: Vec<u8> }
pub fn decode(d: &[u8]) -> Result<Dict> {
    let e = d.iter().position(|&b| b == b'@').ok_or(Error::Bad("dict: no '@' terminator"))?;
    if e == 0 || d[e - 1] != b'^' { return Err(Error::Bad("dict: last word not '^'-terminated")); }
    Ok(Dict { words: d[..e - 1].split(|&b| b == b'^').map(<[u8]>::to_vec).collect(), tail: d[e + 1..].to_vec() })
}
pub fn encode(p: &Dict) -> Vec<u8> {
    let mut w = Vec::with_capacity(p.words.iter().map(|x| x.len() + 1).sum::<usize>() + 1 + p.tail.len());
    for x in &p.words { w.extend_from_slice(x); w.push(b'^'); }
    w.push(b'@'); w.extend_from_slice(&p.tail); w
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip() {
        let d = b"the^^ship^@xyz";
        let p = decode(d).unwrap();
        assert_eq!(p.words, [&b"the"[..], b"", b"ship"]);
        assert_eq!(p.tail, b"xyz");
        assert_eq!(encode(&p), d);
        assert!(decode(b"the^ship@").is_err());
        assert!(decode(b"@").is_err());
        assert!(decode(b"the^").is_err());
    }
}
