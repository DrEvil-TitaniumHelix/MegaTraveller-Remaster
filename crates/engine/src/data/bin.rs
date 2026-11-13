use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Short { at: usize, need: usize, have: usize },
    Value { what: &'static str, got: i64, want: i64 },
    Bad(&'static str),
    Range { what: &'static str, got: usize, bits: usize },
}
pub type Result<T> = std::result::Result<T, Error>;
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Short { at, need, have } => write!(f, "short read at {at}: need {need}, have {have}"),
            Error::Value { what, got, want } => write!(f, "{what}: got {got}, want {want}"),
            Error::Bad(s) => write!(f, "bad data: {s}"),
            Error::Range { what, got, bits } => write!(f, "{what}: {got} does not fit a {bits}-bit field"),
        }
    }
}
impl std::error::Error for Error {}
pub fn check(what: &'static str, got: impl Into<i64>, want: impl Into<i64>) -> Result<()> {
    let (got, want) = (got.into(), want.into());
    if got == want { Ok(()) } else { Err(Error::Value { what, got, want }) }
}
pub fn fit<T: TryFrom<usize>>(what: &'static str, v: usize) -> Result<T> {
    T::try_from(v).map_err(|_| Error::Range { what, got: v, bits: 8 * std::mem::size_of::<T>() })
}

pub struct Rd<'a> { pub d: &'a [u8], pub p: usize }
impl<'a> Rd<'a> {
    pub fn new(d: &'a [u8]) -> Self { Rd { d, p: 0 } }
    pub fn left(&self) -> usize { self.d.len().saturating_sub(self.p) }
    pub fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        if self.left() < n { return Err(Error::Short { at: self.p, need: n, have: self.left() }); }
        let s = &self.d[self.p..self.p + n]; self.p += n; Ok(s)
    }
    pub fn arr<const N: usize>(&mut self) -> Result<[u8; N]> { Ok(self.take(N)?.try_into().unwrap()) }
    pub fn u8(&mut self) -> Result<u8> { Ok(self.take(1)?[0]) }
    pub fn i8(&mut self) -> Result<i8> { Ok(self.u8()? as i8) }
    pub fn u16(&mut self) -> Result<u16> { Ok(u16::from_le_bytes(self.arr()?)) }
    pub fn i16(&mut self) -> Result<i16> { Ok(i16::from_le_bytes(self.arr()?)) }
    pub fn u32(&mut self) -> Result<u32> { Ok(u32::from_le_bytes(self.arr()?)) }
    pub fn u16s(&mut self, n: usize) -> Result<Vec<u16>> { (0..n).map(|_| self.u16()).collect() }
    pub fn i16s(&mut self, n: usize) -> Result<Vec<i16>> { (0..n).map(|_| self.i16()).collect() }
    pub fn u32s(&mut self, n: usize) -> Result<Vec<u32>> { (0..n).map(|_| self.u32()).collect() }
    pub fn rest(&mut self) -> &'a [u8] { let s = &self.d[self.p.min(self.d.len())..]; self.p = self.d.len(); s }
    pub fn done(&self) -> Result<()> {
        if self.p > self.d.len() { return Err(Error::Short { at: self.p, need: 0, have: self.d.len() }); }
        check("trailing bytes", self.left() as i64, 0)
    }
}

pub trait Wr {
    fn u8(&mut self, v: u8); fn i8(&mut self, v: i8); fn u16(&mut self, v: u16); fn i16(&mut self, v: i16); fn u32(&mut self, v: u32);
    fn bytes(&mut self, b: &[u8]);
}
impl Wr for Vec<u8> {
    fn u8(&mut self, v: u8) { self.push(v) }
    fn i8(&mut self, v: i8) { self.push(v as u8) }
    fn u16(&mut self, v: u16) { self.extend_from_slice(&v.to_le_bytes()) }
    fn i16(&mut self, v: i16) { self.extend_from_slice(&v.to_le_bytes()) }
    fn u32(&mut self, v: u32) { self.extend_from_slice(&v.to_le_bytes()) }
    fn bytes(&mut self, b: &[u8]) { self.extend_from_slice(b) }
}

pub trait Record: Sized {
    const SIZE: usize;
    fn read(r: &mut Rd) -> Result<Self>;
    fn write(&self, w: &mut Vec<u8>);
    fn take(r: &mut Rd) -> Result<Self> { parse(r.take(Self::SIZE)?, Self::read) }
    fn take_n(r: &mut Rd, n: usize) -> Result<Vec<Self>> { (0..n).map(|_| Self::take(r)).collect() }
    fn to_bytes(&self) -> Vec<u8> { let mut w = Vec::with_capacity(Self::SIZE); self.write(&mut w); w }
}
pub fn parse<T>(d: &[u8], f: impl FnOnce(&mut Rd) -> Result<T>) -> Result<T> {
    let mut r = Rd::new(d); let v = f(&mut r)?; r.done()?; Ok(v)
}
pub fn write_all<T: Record>(w: &mut Vec<u8>, v: &[T]) { for x in v { x.write(w) } }

#[cfg(test)]
mod t {
    use super::*;
    struct P { a: u16, b: i16, c: [u8; 3] }
    impl Record for P {
        const SIZE: usize = 7;
        fn read(r: &mut Rd) -> Result<Self> { Ok(P { a: r.u16()?, b: r.i16()?, c: r.arr()? }) }
        fn write(&self, w: &mut Vec<u8>) { w.u16(self.a); w.i16(self.b); w.bytes(&self.c) }
    }
    #[test]
    fn cursor_and_record() {
        let d = [1u8, 2, 0xff, 0xff, 7, 8, 9, 0x34, 0x12];
        let mut r = Rd::new(&d);
        let p = P::take(&mut r).unwrap();
        assert_eq!((p.a, p.b, p.c), (0x0201, -1, [7, 8, 9]));
        assert_eq!(r.u16().unwrap(), 0x1234);
        assert!(r.done().is_ok());
        assert_eq!(p.to_bytes(), d[..7]);
        assert!(matches!(Rd::new(&d[..3]).u32(), Err(Error::Short { at: 0, need: 4, have: 3 })));
        assert!(parse(&d, P::read).is_err());
        let mut past = Rd { d: &d, p: 20 };
        assert_eq!(past.left(), 0);
        assert!(matches!(past.u8(), Err(Error::Short { at: 20, need: 1, have: 0 })));
        assert!(past.done().is_err());
        assert_eq!(past.rest(), &[][..]);
        assert_eq!(fit::<u8>("n", 255).unwrap(), 255u8);
        assert_eq!(fit::<u8>("n", 256), Err(Error::Range { what: "n", got: 256, bits: 8 }));
    }
}
