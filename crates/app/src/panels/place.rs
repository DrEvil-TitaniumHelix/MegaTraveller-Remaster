use mt_engine::data::{world, Variant};

const BASES: [usize; 9] = [0, 4, 7, 13, 16, 18, 21, 24, 28];
#[derive(Default)]
pub struct Places(Vec<(u8, Vec<String>)>);
impl Places {
    pub fn decode(w: &[u8]) -> Self {
        let name = |b: &[u8; 40]| String::from_utf8_lossy(&b[..b.iter().position(|&c| c == 0).unwrap_or(40)]).trim().to_owned();
        Self(world::decode(Variant::Mt1, w).unwrap_or_default().iter().filter_map(|c| match c { world::Chunk::Area(a) => Some((a.hdr.ly_b, a.hdr.text3.chunks_exact(40).chain(a.hdr.place_names.iter().map(|p| &p[..])).map(|b| name(b.try_into().unwrap())).collect())), _ => None }).collect())
    }
    fn area(&self, world: u8, area: u8) -> Option<&(u8, Vec<String>)> {
        let (w, a) = (world as usize, area as usize);
        if w >= 8 || BASES[w] + a >= BASES[w + 1] { return None }
        self.0.get(BASES[w] + a)
    }
    pub fn get(&self, world: u8, area: u8, level: i8) -> Option<&str> {
        if level < 1 { return None }
        self.area(world, area)?.1.get(level as usize - 1).map(String::as_str).filter(|s| !s.is_empty() && !s.starts_with("map "))
    }
    pub fn area_name_index(&self, world: u8, area: u8) -> Option<u8> { self.area(world, area).map(|a| a.0) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_named_level_of_every_area() {
        let Ok(dir) = std::env::var("MT1_GAMEFILES") else { return };
        let p = Places::decode(&std::fs::read(std::path::Path::new(&dir).join("W.DAT")).unwrap());
        assert_eq!(p.0.len(), 28);
        for w in 0..8u8 { for a in 0..(BASES[w as usize + 1] - BASES[w as usize]) as u8 {
            let names: Vec<_> = (1..=7).map(|l| p.get(w, a, l).unwrap_or("-")).collect();
            println!("world {w} area {a} (name {:?}): {names:?}", p.area_name_index(w, a));
        } }
        assert_eq!(p.get(0, 0, 0), None); assert_eq!(p.get(0, 3, 4), None);
        assert_eq!(p.get(6, 1, 4), Some("the Arrica Holding Facility")); assert_eq!(p.get(0, 0, 4), Some("the Imperial Security HQ"));
        assert_eq!(p.get(9, 0, 1), None);
    }
}
