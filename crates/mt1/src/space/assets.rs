use mt_engine::{data::{ly, pic, Variant}, io::Files};
use std::collections::BTreeMap;

pub struct Assets { pub space: pic::Pic, pub stars: pic::Pic, pub objects: Vec<pic::Pic>, pub layouts: BTreeMap<&'static str, Vec<ly::Entry>> }
impl Assets {
    pub fn load(files: &dyn Files) -> Result<Self, String> {
        let read = |n: &str| files.read_limited(n, 1024 * 1024).map_err(|e| format!("{n}: {e}"));
        let mut layouts = BTreeMap::new();
        for name in ["FONT.LY", "CH.LY", "STATIONS.LY", "GUNS.LY", "LED.LY", "SPTOP.LY"] {
            layouts.insert(name, ly::decode(&read(name)?).map_err(|e| format!("{name}: {e:?}"))?);
        }
        let picture = |n: &str| pic::decode(Variant::Mt1, &read(n)?).map_err(|e| format!("{n}: {e:?}"));
        let space = picture("SPCPICS.PIC")?;
        let stars = picture("STARS.PIC")?;
        let objects = pic::decode_set(Variant::Mt1, &read("SPOBJ.DAT")?).map_err(|e| format!("SPOBJ.DAT: {e:?}"))?;
        if objects.len() < 2 { return Err("SPOBJ.DAT: missing station/ship pictures".into()); }
        Ok(Self { space, stars, objects, layouts })
    }
    pub fn layout(&self, name: &str) -> &[ly::Entry] { self.layouts.get(name).map(Vec::as_slice).unwrap_or(&[]) }
}
