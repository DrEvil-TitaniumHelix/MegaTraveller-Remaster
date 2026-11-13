use super::{state::State, Space};
use crate::ground::{state::Mem, trace::Trace, walk::covox_state};
use mt_engine::io::{audio, Files, Input};
use std::collections::BTreeMap;

pub const LOADED: u16 = 0x3B78;
pub const ENABLED: u16 = 0x4627;
const DEVICE: u16 = 0x1A28;
const TABLE: u16 = 0x1A0A;

#[derive(Default)]
struct Sample { payload: Vec<u8>, rate: u16 }

pub struct Sounds {
    driver: Mem,
    refresh: u8,
    samples: [Sample; 3],
    duration: u16,
}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Image {pub ds:Vec<u8>,pub segs:BTreeMap<u16,Vec<u8>>,pub bios_ticks:[u8;4],pub refresh:u8,pub samples:[(Vec<u8>,u16);3],pub duration:u16}
impl Sounds {
    pub fn capture(&self)->Image {Image {ds:self.driver.ds.clone(),segs:self.driver.segs.clone(),bios_ticks:self.driver.bios_ticks,refresh:self.refresh,
        samples:std::array::from_fn(|i|(self.samples[i].payload.clone(),self.samples[i].rate)),duration:self.duration}}
    pub fn install(&mut self,image:&Image)->Result<(),String> {
        if image.ds.len()!=65536 {return Err("play: invalid sound image".into())}
        self.driver.ds=image.ds.clone();self.driver.segs=image.segs.clone();self.driver.bios_ticks=image.bios_ticks;self.refresh=image.refresh;self.duration=image.duration;
        self.samples=std::array::from_fn(|i|Sample {payload:image.samples[i].0.clone(),rate:image.samples[i].1});Ok(())
    }
    pub fn new(s: &State) -> Self {
        let mut driver = Mem { ds: vec![0; 65536], segs: BTreeMap::new(), trace: Trace::default(), bios_ticks: [0; 4] };
        driver.ds[0x5588..0x55A4].copy_from_slice(&s.image()[0x2D9A..0x2DB6]);
        driver.ds[0x536E..0x5376].copy_from_slice(&[255, 85, 255, 170, 255, 85, 255, 170]);
        Self { driver, refresh: 0, samples: std::array::from_fn(|_| Sample::default()), duration: 0 }
    }

    pub fn load(&mut self, s: &State, files: &dyn Files) {
        self.samples = std::array::from_fn(|_| Sample::default());
        for (i, sample) in self.samples.iter_mut().enumerate() {
            let at = TABLE + 6 * i as u16;
            let rate = s.word(at + 4) as u16;
            if rate == 0 { break; }
            let Some(name) = s.pointer_string(at) else { continue };
            let name: String = name.into_iter().map(char::from).collect();
            let Ok(file) = files.read_limited(&name, 64_128) else { continue };
            let Some(payload) = file.get(128..) else { continue };
            *sample = Sample { payload: payload.to_vec(), rate };
        }
    }

    pub fn play(&mut self, s: &State, input: &mut dyn Input, index: u16) -> bool {
        if s.byte(LOADED) == 0 || s.byte(ENABLED) == 0 { return false; }
        let Some(sample) = self.samples.get(index as usize) else { return false };
        if sample.payload.len() < 16 || sample.rate == 0 { return false; }
        let p = &sample.payload;
        if p.starts_with(&[255, 85, 255, 170, 255, 85, 255, 170]) && p[8] != 8 { return false; }
        let rate = u32::from(sample.rate);
        let divisor = (256i32 - 1_193_180 / rate as i32) as u16;
        let device = s.word(DEVICE) as u16;
        let device = if device == u16::MAX { (s.word(0x2DC9) as u16).wrapping_add(2) } else { device };
        let mut ports = Vec::with_capacity(3 * p.len() + 8);
        let result = covox_state(&mut self.driver, p, divisor, device, false, false, &mut self.refresh, &mut ports);
        if result != 0 { return false; }
        self.duration = ((p.len() as u32 * 182 + rate - 1) / rate) as u16;
        let (levels, rest) = audio::levels(&ports, self.driver.u16(0x559A));
        input.audio(index, audio::cadence_hz(rate), &levels, rest);
        true
    }
}

impl Space {
    pub fn play_sound(&mut self, index: u16) -> bool { self.sounds.play(&self.s, self.input.as_mut(), index) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::space::state::{DS, PARTY_SIZE};
    use mt_engine::io::{Key, Overlay};

    #[derive(Default)]
    struct Output(Vec<(u16, u32, Vec<u8>, u8)>);
    impl Input for Output {
        fn next_key(&mut self) -> Key { panic!("sound must not read input") }
        fn audio(&mut self, n: u16, hz: u32, levels: &[u8], rest: u8) { self.0.push((n, hz, levels.to_vec(), rest)); }
    }
    fn fixture() -> (State, Overlay) {
        let mut s = State::from_ds(vec![], vec![7; PARTY_SIZE], vec![11; 0x1300]).unwrap();
        s.set_byte(LOADED, 1); s.set_byte(ENABLED, 1); s.set_word(DEVICE, 8);
        s.set_byte(0x2D9C, 128); s.set_word(0x2DC9, 0x220);
        for (i, port) in [0x22F, 0x24F, 0x28F, 0x2CF].into_iter().enumerate() { s.set_word(0x2DA4 + 2 * i as u16, port); }
        let mut files = Overlay::new(None);
        for (i, name) in ["MA.VMF", "LGUN.VMF", "BOOM.VMF"].into_iter().enumerate() {
            let at = TABLE + 6 * i as u16; let ptr = 0x1900 + 16 * i as u16;
            s.set_word(at, ptr as i16); s.set_word(at + 2, DS as i16); s.set_word(at + 4, 5000);
            for (j, b) in name.bytes().enumerate() { s.set_byte(ptr + j as u16, b); }
            let mut bytes = vec![0xA5; 128];
            bytes.extend((0..32).map(|n| ((n * 47 + i * 23) % 256) as u8));
            files.write(name, &bytes).unwrap();
        }
        (s, files)
    }
    #[test]
    fn muted_invalid_and_missing_are_complete_noops() {
        let (mut s, mut files) = fixture();
        files.hide("LGUN.VMF"); files.write("BOOM.VMF", &[0; 143]).unwrap();
        let mut sounds = Sounds::new(&s); sounds.load(&s, &files);
        let mut out = Output::default();
        assert!(sounds.play(&s, &mut out, 0)); out.0.clear();
        for (loaded, enabled, index) in [(0, 0, 0), (0, 1, 0), (1, 0, 0), (1, 1, 1), (1, 1, 2), (1, 1, 3), (1, 1, u16::MAX)] {
            s.set_byte(LOADED, loaded); s.set_byte(ENABLED, enabled);
            let before = (s.image().to_vec(), s.party.clone(), s.world.clone(), sounds.driver.ds.clone(), sounds.refresh, sounds.duration);
            assert!(!sounds.play(&s, &mut out, index));
            assert_eq!(before, (s.image().to_vec(), s.party.clone(), s.world.clone(), sounds.driver.ds.clone(), sounds.refresh, sounds.duration));
            assert!(out.0.is_empty());
        }
    }
    #[test]
    fn both_devices_preserve_sim_and_emit_original_length_and_cadence() {
        let (mut s, files) = fixture();
        for device in [8, -1] {
            s.set_word(DEVICE, device);
            s.set_byte(crate::space::state::KEY, 0x39); s.set_byte(crate::space::state::PENDING_KEY, 0x1C);
            let before = (s.image().to_vec(), s.party.clone(), s.world.clone());
            let mut sounds = Sounds::new(&s); sounds.load(&s, &files);
            let mut out = Output::default();
            for index in [0, 1, 2, 0] { assert!(sounds.play(&s, &mut out, index)); }
            for (index, hz, levels, rest) in &out.0 {
                assert_eq!((*hz, levels.len(), *rest), (5013, 31, 0));
                if device == -1 { assert_eq!(levels, &sounds.samples[*index as usize].payload[..31]); }
                else { assert!(levels.iter().all(|n| *n == 0 || *n == 255)); }
            }
            assert_eq!(before, (s.image().to_vec(), s.party.clone(), s.world.clone()));
            assert_eq!(sounds.duration, 2);
            assert_eq!(sounds.driver.u8(0x558A), if device == 8 { sounds.samples[0].payload[30] } else { 128 });
        }
    }
    #[test]
    fn load_bounds_and_descriptor_terminator() {
        let (mut s, mut files) = fixture();
        files.write("MA.VMF", &vec![0; 64129]).unwrap();
        files.write("LGUN.VMF", &[0; 127]).unwrap();
        s.set_word(TABLE + 16, 0);
        let mut sounds = Sounds::new(&s); sounds.load(&s, &files);
        let mut out = Output::default();
        for index in 0..3 { assert!(!sounds.play(&s, &mut out, index)); }
        assert!(out.0.is_empty());
        files.write("MA.VMF", &vec![128; 64128]).unwrap();
        sounds.load(&s, &files); assert!(sounds.play(&s, &mut out, 0));
        assert_eq!(out.0[0].2.len(), 63999);
    }
}
