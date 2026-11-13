use super::bin::{check, fit, Rd, Result, Wr};
use super::{container, packbits, Variant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pic { pub h: u16, pub wbytes4: u16, pub pal: [u8; 16], pub px: Vec<u8>, pub tail: Vec<u8> }

pub fn decode(v: Variant, d: &[u8]) -> Result<Pic> {
    let Variant::Mt1 = v;
    let mut r = Rd::new(d);
    let (h, wb) = (r.u16()?, r.u16()?);
    let pal = r.arr()?;
    let (ul, pl) = (r.u16()? as usize, r.u16()? as usize);
    check("pic unpacked length", ul as i64, h as i64 * wb as i64 * 4)?;
    let (px, used) = packbits::decode(r.take(pl)?, ul)?;
    check("pic packed length", used as i64, pl as i64)?;
    Ok(Pic { h, wbytes4: wb, pal, px, tail: r.rest().to_vec() })
}
pub fn encode(v: Variant, p: &Pic) -> Result<Vec<u8>> {
    let Variant::Mt1 = v;
    let pk = packbits::encode(&p.px);
    let mut w = Vec::with_capacity(24 + pk.len() + p.tail.len());
    w.u16(p.h); w.u16(p.wbytes4); w.bytes(&p.pal); w.u16(fit("pic unpacked length", p.px.len())?); w.u16(fit("pic packed length", pk.len())?); w.bytes(&pk); w.bytes(&p.tail);
    Ok(w)
}
pub fn decode_set(v: Variant, d: &[u8]) -> Result<Vec<Pic>> { container::decode(d)?.into_iter().map(|c| decode(v, c)).collect() }
pub fn encode_set(v: Variant, p: &[Pic]) -> Result<Vec<u8>> { container::encode(&p.iter().map(|x| encode(v, x)).collect::<Result<Vec<_>>>()?) }

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn round_trip_synthetic() {
        let p = Pic { h: 2, wbytes4: 1, pal: [0; 16], px: vec![0x12, 0x12, 0x12, 0x34, 0x56, 0x78, 0x9a, 0xbc], tail: vec![0xff] };
        let e = encode(Variant::Mt1, &p).unwrap();
        assert_eq!(&e[..24], &[2, 0, 1, 0, 0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0, 8, 0, 8, 0]);
        assert_eq!(decode(Variant::Mt1, &e).unwrap(), p);
        let set = encode_set(Variant::Mt1, &[p.clone(), p.clone()]).unwrap();
        assert_eq!(decode_set(Variant::Mt1, &set).unwrap(), vec![p.clone(), p]);
        let mut bad = e.clone(); bad[20] = 9;
        assert!(decode(Variant::Mt1, &bad).is_err());
        let big = Pic { h: 1, wbytes4: 1, pal: [0; 16], px: vec![0; 65536], tail: vec![] };
        assert!(matches!(encode(Variant::Mt1, &big), Err(super::super::bin::Error::Range { bits: 16, .. })));
    }
}
