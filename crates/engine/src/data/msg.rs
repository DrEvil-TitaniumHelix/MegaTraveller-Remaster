use super::bin::{check, fit, Rd, Result, Wr};
use super::Variant;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Msgs { pub msgs: Vec<Vec<i16>>, pub tail: Vec<u8> }
pub fn decode(v: Variant, d: &[u8]) -> Result<Msgs> {
    let Variant::Mt1 = v;
    let mut r = Rd::new(d);
    let n = r.u16()? as usize;
    let (mut msgs, mut end) = (Vec::with_capacity(n), 2 + 6 * n);
    for _ in 0..n {
        let (c, o) = (r.u16()? as usize, r.u32()? as usize);
        check("msg token offset", o as i64 + 2, end as i64)?;
        let mut t = Rd { d, p: end }; msgs.push(t.i16s(c)?); end = t.p;
    }
    Ok(Msgs { msgs, tail: d[end.min(d.len())..].to_vec() })
}
pub fn encode(v: Variant, p: &Msgs) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    let (mut w, mut b, mut o) = (Vec::new(), Vec::new(), 6 * p.msgs.len());
    w.u16(fit("msg count", p.msgs.len())?);
    for m in &p.msgs { w.u16(fit("msg token count", m.len())?); w.u32(fit("msg token offset", o)?); for &t in m { b.i16(t); } o += 2 * m.len(); }
    w.bytes(&b); w.bytes(&p.tail); Ok(w)
}

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip() {
        let p = Msgs { msgs: vec![vec![1, -1, 300], vec![], vec![7]], tail: vec![0xee] };
        let e = encode(Variant::Mt1, &p).unwrap();
        assert_eq!(e[..2], [3, 0]);
        assert_eq!(e[2..8], [3, 0, 18, 0, 0, 0]);
        assert_eq!(decode(Variant::Mt1, &e).unwrap(), p);
        let mut bad = e.clone(); bad[4] = 19; assert!(decode(Variant::Mt1, &bad).is_err());
        assert!(decode(Variant::Mt1, &[2, 0, 1, 0, 0x0c, 0, 0, 0]).is_err());
        assert!(encode(Variant::Mt1, &Msgs { msgs: vec![vec![]; 65536], tail: vec![] }).is_err());
    }
}
