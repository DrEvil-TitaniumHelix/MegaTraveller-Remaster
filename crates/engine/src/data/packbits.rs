use super::bin::{check, Error, Result};

pub fn decode(src: &[u8], n: usize) -> Result<(Vec<u8>, usize)> {
    let mut o = Vec::new();
    let mut i = 0;
    while o.len() < n {
        let b = *src.get(i).ok_or(Error::Short { at: i, need: 1, have: 0 })? as usize;
        i += 1;
        if b < 0x80 {
            let s = src.get(i..i + b + 1).ok_or(Error::Short { at: i, need: b + 1, have: src.len().saturating_sub(i) })?;
            o.extend_from_slice(s); i += b + 1;
        } else if b > 0x80 {
            let v = *src.get(i).ok_or(Error::Short { at: i, need: 1, have: 0 })?;
            o.extend(std::iter::repeat(v).take(257 - b)); i += 1;
        }
    }
    check("packbits length", o.len() as i64, n as i64)?;
    Ok((o, i))
}
pub fn encode(px: &[u8]) -> Vec<u8> {
    let mut o = Vec::with_capacity(px.len() / 2);
    let mut lit: Vec<u8> = Vec::with_capacity(128);
    let flush = |o: &mut Vec<u8>, lit: &mut Vec<u8>| { if !lit.is_empty() { o.push(lit.len() as u8 - 1); o.extend_from_slice(lit); lit.clear(); } };
    let (mut i, n) = (0, px.len());
    while i < n {
        let mut r = 1;
        while i + r < n && px[i + r] == px[i] && r < 128 { r += 1; }
        if r >= 3 { flush(&mut o, &mut lit); o.push((257 - r) as u8); o.push(px[i]); i += r; }
        else { lit.push(px[i]); i += 1; if lit.len() == 128 { flush(&mut o, &mut lit); } }
    }
    flush(&mut o, &mut lit);
    o
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn packer_rule_and_round_trip() {
        assert_eq!(encode(&[1, 2, 3]), [2, 1, 2, 3]);
        assert_eq!(encode(&[7, 7, 7, 7]), [253, 7]);
        assert_eq!(encode(&[7, 7, 1]), [2, 7, 7, 1]);
        assert_eq!(encode(&[9; 300]), [129, 9, 129, 9, 213, 9]);
        let px: Vec<u8> = (0..1000u32).map(|i| ((i * 7919) % 5 / 3 * (i % 13)) as u8).collect();
        let e = encode(&px);
        let (d, used) = decode(&e, px.len()).unwrap();
        assert_eq!((d, used), (px, e.len()));
        assert!(decode(&[5, 1], 6).is_err());
        assert!(decode(&[0x81, 9], usize::MAX).is_err());
    }
}
