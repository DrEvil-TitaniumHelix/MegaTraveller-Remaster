use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io::{self, Read}};

pub trait Files {
    fn read(&self, name: &str) -> Option<Vec<u8>>;
    fn read_limited(&self, name: &str, limit: usize) -> io::Result<Vec<u8>>;
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()>;
}
fn too_large() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "file byte budget exceeded") }
pub struct Dir(pub PathBuf);
impl Dir {
    pub fn path(&self, name: &str) -> PathBuf {
        let up = name.to_uppercase();
        fs::read_dir(&self.0).into_iter().flatten().flatten().map(|e| e.path()).find(|p| p.file_name().map_or(false, |f| f.to_string_lossy().to_uppercase() == up)).unwrap_or_else(|| self.0.join(name))
    }
}
impl Files for Dir {
    fn read(&self, name: &str) -> Option<Vec<u8>> { fs::read(self.path(name)).ok() }
    fn read_limited(&self, name: &str, limit: usize) -> io::Result<Vec<u8>> {
        let file = fs::File::open(self.path(name))?;
        if file.metadata()?.len() > limit as u64 { return Err(too_large()); }
        read_limited(file, limit)
    }
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let path = self.path(name);
        let mut tmp = path.clone().into_os_string(); tmp.push(".tmp");
        fs::write(&tmp, data).and_then(|_| fs::rename(&tmp, &path)).inspect_err(|_| { let _ = fs::remove_file(&tmp); })
    }
}
fn read_limited(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.take((limit as u64).saturating_add(1)).read_to_end(&mut data)?;
    if data.len() > limit { return Err(too_large()); }
    Ok(data)
}
pub struct Overlay { base: Option<Dir>, mem: HashMap<String, Option<Vec<u8>>>, order: Vec<String> }
impl Overlay {
    pub fn new(base: Option<&Path>) -> Overlay { Overlay { base: base.map(|p| Dir(p.to_path_buf())), mem: HashMap::new(), order: vec![] } }
    pub fn hide(&mut self, name: &str) { self.mem.insert(name.to_uppercase(), None); }
    pub fn written(&self) -> Vec<(&str, &[u8])> { self.order.iter().filter_map(|n| Some((n.as_str(), self.mem.get(n)?.as_deref()?))).collect() }
}
impl Files for Overlay {
    fn read(&self, name: &str) -> Option<Vec<u8>> {
        match self.mem.get(&name.to_uppercase()) { Some(v) => v.clone(), None => self.base.as_ref()?.read(name) }
    }
    fn read_limited(&self, name: &str, limit: usize) -> io::Result<Vec<u8>> {
        match self.mem.get(&name.to_uppercase()) {
            Some(Some(data)) if data.len() <= limit => Ok(data.clone()),
            Some(Some(_)) => Err(too_large()),
            Some(None) => Err(io::ErrorKind::NotFound.into()),
            None => self.base.as_ref().ok_or(io::ErrorKind::NotFound)?.read_limited(name, limit),
        }
    }
    fn write(&mut self, name: &str, data: &[u8]) -> io::Result<()> {
        let n = name.to_uppercase();
        if !self.order.contains(&n) { self.order.push(n.clone()) }
        self.mem.insert(n, Some(data.to_vec()));
        Ok(())
    }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn bounded_reads_cover_disk_overlay_and_growth() {
        let d = std::env::temp_dir().join(format!("mt-limited-files-{}",std::process::id()));
        fs::create_dir_all(&d).unwrap(); fs::write(d.join("data"),[7;32]).unwrap();
        let dir = Dir(d.clone());
        assert_eq!(dir.read_limited("DATA",32).unwrap(),[7;32]);
        assert_eq!(dir.read_limited("DATA",31).unwrap_err().kind(),io::ErrorKind::InvalidData);
        let mut overlay = Overlay::new(Some(&d));
        assert_eq!(overlay.read_limited("DATA",32).unwrap(),[7;32]);
        assert!(overlay.read_limited("DATA",31).is_err());
        overlay.hide("data");
        assert_eq!(overlay.read_limited("DATA",32).unwrap_err().kind(),io::ErrorKind::NotFound);
        overlay.write("data",&[9;33]).unwrap();
        assert!(overlay.read_limited("DATA",32).is_err());
        assert_eq!(overlay.read_limited("DATA",33).unwrap(),[9;33]);
        assert_eq!(overlay.read_limited("missing",0).unwrap_err().kind(),io::ErrorKind::NotFound);
        let mut stream = io::repeat(5).take(1000);
        assert!(read_limited(&mut stream,32).is_err()); assert_eq!(stream.limit(),967);
        assert_eq!(read_limited(&b""[..],0).unwrap(),[]);
        assert!(read_limited(&b"x"[..],0).is_err());
        fs::remove_dir_all(d).unwrap();
    }
    #[test]
    fn dir_and_overlay() {
        let d = std::env::temp_dir().join(format!("mt-files-{}", std::process::id()));
        fs::create_dir_all(&d).unwrap(); fs::write(d.join("dchars.dat"), [1, 2]).unwrap();
        let mut dir = Dir(d.clone());
        assert_eq!(dir.read("DCHARS.DAT"), Some(vec![1, 2])); assert_eq!(dir.read("UCHARS.DAT"), None);
        dir.write("UCHARS.DAT", &[3]).unwrap(); assert_eq!(fs::read(d.join("UCHARS.DAT")).unwrap(), [3]);
        dir.write("Dchars.DAT", &[9]).unwrap(); assert_eq!(fs::read(d.join("dchars.dat")).unwrap(), [9]);
        assert_eq!(fs::read_dir(&d).unwrap().count(), 2);
        fs::create_dir(d.join("UCHARS.DAT.tmp")).unwrap();
        assert!(dir.write("UCHARS.DAT", &[8]).is_err()); assert_eq!(fs::read(d.join("UCHARS.DAT")).unwrap(), [3]);
        fs::remove_dir(d.join("UCHARS.DAT.tmp")).unwrap();
        let mut o = Overlay::new(Some(&d));
        assert_eq!(o.read("dchars.dat"), Some(vec![9]));
        o.write("uchars.dat", &[4]).unwrap(); o.write("UCHARS.DAT", &[5]).unwrap();
        assert_eq!(o.read("Uchars.Dat"), Some(vec![5])); assert_eq!(fs::read(d.join("UCHARS.DAT")).unwrap(), [3]);
        assert_eq!(o.written(), [("UCHARS.DAT", &[5u8][..])]);
        let e = Overlay::new(None); assert_eq!(e.read("DCHARS.DAT"), None); assert!(e.written().is_empty());
        let mut h = Overlay::new(Some(&d)); h.hide("uchars.dat");
        assert_eq!(h.read("UCHARS.DAT"), None); assert_eq!(h.read("DCHARS.DAT"), Some(vec![9])); assert!(h.written().is_empty());
        h.write("UCHARS.DAT", &[7]).unwrap(); assert_eq!(h.read("uchars.dat"), Some(vec![7])); assert_eq!(h.written(), [("UCHARS.DAT", &[7u8][..])]);
        fs::remove_dir_all(&d).unwrap();
    }
}
