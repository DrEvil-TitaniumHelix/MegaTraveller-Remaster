use super::bin::{check, fit, Error, Rd, Result, Wr};

pub fn decode(d: &[u8]) -> Result<Vec<&[u8]>> {
    let mut r = Rd::new(d);
    let n = r.u32()? as usize;
    let mut o = r.u32s(n)?.into_iter().map(|x| x as usize).collect::<Vec<_>>();
    if n > 0 { check("container first offset", o[0] as i64, (4 + 4 * n) as i64)?; }
    if o.windows(2).any(|w| w[0] > w[1]) { return Err(Error::Bad("container offsets not sorted")); }
    if let Some(&l) = o.last() { if l > d.len() { return Err(Error::Short { at: l, need: 0, have: d.len() }); } }
    o.push(d.len());
    Ok((0..n).map(|i| &d[o[i]..o[i + 1]]).collect())
}
pub fn encode(chunks: &[impl AsRef<[u8]>]) -> Result<Vec<u8>> {
    let mut w = Vec::with_capacity(4 + 4 * chunks.len() + chunks.iter().map(|c| c.as_ref().len()).sum::<usize>());
    w.u32(fit("container count", chunks.len())?);
    let mut o = 4 + 4 * chunks.len();
    for c in chunks { w.u32(fit("container offset", o)?); o += c.as_ref().len(); }
    for c in chunks { w.bytes(c.as_ref()); }
    Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip() {
        let ch: Vec<Vec<u8>> = vec![vec![1, 2, 3], vec![], vec![9]];
        let e = encode(&ch).unwrap();
        assert_eq!(e, [3, 0, 0, 0, 16, 0, 0, 0, 19, 0, 0, 0, 19, 0, 0, 0, 1, 2, 3, 9]);
        assert_eq!(decode(&e).unwrap(), ch.iter().map(|c| c.as_slice()).collect::<Vec<_>>());
        assert!(decode(&[1, 0, 0, 0, 9, 0, 0, 0]).is_err());
        assert_eq!(decode(&[0, 0, 0, 0]).unwrap().len(), 0);
    }
}
