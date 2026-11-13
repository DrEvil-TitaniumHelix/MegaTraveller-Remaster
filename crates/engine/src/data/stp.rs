use super::bin::{parse, write_all, Error, Rd, Record, Result, Wr};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Object {
    pub world: u8,
    pub orbit: u8,
    pub phase: u8,
    pub angle: u8,
    pub moons: u8,
    pub sprite: u8,
}
impl Record for Object {
    const SIZE: usize = 6;
    fn read(r: &mut Rd) -> Result<Self> {
        Ok(Self { world: r.u8()?, orbit: r.u8()?, phase: r.u8()?, angle: r.u8()?, moons: r.u8()?, sprite: r.u8()? })
    }
    fn write(&self, w: &mut Vec<u8>) {
        w.bytes(&[self.world, self.orbit, self.phase, self.angle, self.moons, self.sprite]);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stp { pub default_x: u8, pub default_y: u8, pub objects: Vec<Object> }

pub fn decode(d: &[u8]) -> Result<Stp> {
    parse(d, |r| {
        let n = r.u8()? as usize;
        Ok(Stp { default_x: r.u8()?, default_y: r.u8()?, objects: Object::take_n(r, n)? })
    })
}
pub fn encode(s: &Stp) -> Result<Vec<u8>> {
    let n = u8::try_from(s.objects.len()).map_err(|_| Error::Bad("STP object count exceeds u8"))?;
    let mut w = Vec::with_capacity(3 + Object::SIZE * s.objects.len());
    w.bytes(&[n, s.default_x, s.default_y]); write_all(&mut w, &s.objects); Ok(w)
}
