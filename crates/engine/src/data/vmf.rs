use super::bin::{check, Error, Rd, Record, Result, Wr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Header {
    pub length: u16,
    pub length_hi: u8,
    pub b3: u8,
    pub b4: u8,
    pub zero: [u8; 114],
    pub magic: [u8; 9],
}
impl Record for Header {
    const SIZE: usize = 128;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(Self { length: r.u16()?, length_hi: r.u8()?, b3: r.u8()?, b4: r.u8()?, zero: r.arr()?, magic: r.arr()? })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.u16(self.length); w.bytes(&[self.length_hi, self.b3, self.b4]); w.bytes(&self.zero); w.bytes(&self.magic);
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Vmf { pub hdr: Header, pub pcm: Vec<u8> }

fn validate(v: &Vmf) -> Result<()> {
    check("VMF PCM length", v.pcm.len() as i64, i64::from(v.hdr.length) + 65536 * i64::from(v.hdr.length_hi))?;
    if &v.hdr.magic != b"GEVATOSKY" { return Err(Error::Bad("VMF magic")); }
    Ok(())
}
pub fn decode(d: &[u8]) -> Result<Vmf> {
    let mut r = Rd::new(d);
    let v = Vmf { hdr: Header::take(&mut r)?, pcm: r.rest().to_vec() };
    validate(&v)?; Ok(v)
}
pub fn encode(v: &Vmf) -> Result<Vec<u8>> {
    validate(v)?;
    let mut w = Vec::with_capacity(Header::SIZE + v.pcm.len());
    v.hdr.write(&mut w); w.bytes(&v.pcm); Ok(w)
}

pub const SOUND_RATES: [(&str, u16); 19] = [
    ("walk", 5000), ("atv", 6000), ("grav", 5000), ("boat", 5000),
    ("mark", 7000), ("thump", 5000), ("boom", 5000), ("siren", 5000),
    ("gun1", 5000), ("shot", 5000), ("lgun", 5000), ("gunshot", 5000),
    ("smg1", 5000), ("shotgun", 5000), ("plasma", 5000), ("ma", 5000),
    ("launch", 5000), ("door", 5000), ("whooosh", 5000),
];
