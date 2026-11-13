use super::bin::{Error, Rd, Result, Wr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unpacked {
    pub exe: Vec<u8>,
    pub relocations: usize,
    pub prefix_end: usize,
}

fn word(v: usize) -> Result<u16> { u16::try_from(v).map_err(|_| Error::Bad("EXEPACK MZ field exceeds u16")) }

pub fn unpack(d: &[u8]) -> Result<Unpacked> {
    let mut r = Rd::new(d);
    if r.take(2)? != b"MZ" { return Err(Error::Bad("EXEPACK MZ magic")); }
    r.take(6)?;
    let hp = r.u16()? as usize * 16;
    let minalloc = r.u16()?;
    r.take(10)?;
    let h = r.u16()? as usize * 16;
    let mut file = Rd::new(d); file.take(hp)?;
    let img = file.rest();
    let mut image = Rd::new(img);
    let src = image.take(h)?;
    let sig = &img[h..img.len().min(h + 20)];
    if !matches!(sig.windows(2).position(|s| s == b"RB"), Some(14 | 16)) {
        return Err(Error::Bad("EXEPACK RB signature"));
    }
    let ip = image.u16()?;
    let rcs = image.u16()?;
    image.u16()?;
    image.u16()?;
    let sp = image.u16()?;
    let ss = image.u16()?;
    let dlen = image.u16()? as usize * 16;
    if src.len() > dlen { return Err(Error::Bad("EXEPACK packed image exceeds output size")); }
    let mut out = vec![0; dlen]; out[..src.len()].copy_from_slice(src);
    let mut p = src.len();
    while p > 0 && src[p - 1] == 0xff { p -= 1; }
    let mut q = out.len();
    loop {
        if p < 3 { return Err(Error::Bad("EXEPACK missing command")); }
        let cmd = src[p - 1];
        let n = u16::from_le_bytes([src[p - 3], src[p - 2]]) as usize;
        p -= 3;
        let start = q.checked_sub(n).ok_or(Error::Bad("EXEPACK output underflow"))?;
        match cmd & 0xfe {
            0xb0 => {
                p = p.checked_sub(1).ok_or(Error::Bad("EXEPACK missing fill byte"))?;
                out[start..q].fill(src[p]);
            }
            0xb2 => {
                let from = p.checked_sub(n).ok_or(Error::Bad("EXEPACK truncated literal"))?;
                out[start..q].copy_from_slice(&src[from..p]); p = from;
            }
            _ => return Err(Error::Bad("EXEPACK bad command")),
        }
        q = start;
        if cmd & 1 != 0 { break; }
    }
    if q < p { return Err(Error::Bad("EXEPACK overlap")); }

    const MESSAGE: &[u8] = b"Packed file is corrupt";
    let m = img[h..].windows(MESSAGE.len()).position(|s| s == MESSAGE)
        .ok_or(Error::Bad("EXEPACK relocation marker missing"))? + h + MESSAGE.len();
    let mut rel = Vec::new();
    let mut r = Rd::new(img); r.take(m)?;
    for seg in 0..16u16 {
        let count = r.u16()?;
        for _ in 0..count { rel.push((r.u16()?, seg * 0x1000)); }
    }
    let hdrsz = (0x1c + 4 * rel.len() + 15) / 16 * 16;
    let tot = hdrsz + out.len();
    let mut exe = Vec::with_capacity(tot);
    exe.bytes(b"MZ");
    for v in [word(tot % 512)?, word((tot + 511) / 512)?, word(rel.len())?, word(hdrsz / 16)?,
              minalloc, 0xffff, ss, sp, 0, ip, rcs, 0x1c, 0] { exe.u16(v); }
    for &(offset, segment) in &rel { exe.u16(offset); exe.u16(segment); }
    exe.resize(hdrsz, 0); exe.bytes(&out);
    Ok(Unpacked { exe, relocations: rel.len(), prefix_end: q })
}
