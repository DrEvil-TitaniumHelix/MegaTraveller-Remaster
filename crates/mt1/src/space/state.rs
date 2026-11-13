use mt_engine::data::exepack;
use crate::screens::title::TcRand;

pub const DS: u16 = 0x261D;
pub const PARTY_SIZE: usize = 0xF86;
pub const PARTY_SEG: u16 = 0x9000;
pub const WORLD_SEG: u16 = 0x9100;
pub const OBJECTS: usize = 80;
pub const SHIPS: usize = 18;
pub const PARTY_PTR: u16 = 0x3456;
pub const WORLD_PTR: u16 = 0x3AAC;
pub const RNG: u16 = 0x343A;
pub const X: u16 = 0x37D6;
pub const Y: u16 = 0x387C;
pub const CURRENT: u16 = 0x034B;
pub const DEPARTED: u16 = 0x01C9;
pub const VX: u16 = 0x034D;
pub const VY: u16 = 0x034F;
pub const THRUST: u16 = 0x0351;
pub const MOVING: u16 = 0x0343;
pub const DIRECTION: u16 = 0x0353;
pub const HEADING: u16 = 0x34B8;
pub const VIEW: u16 = 0x0345;
pub const QUADRANT: u16 = 0x0347;
pub const WORLD_TIME: u16 = 0x345A;
pub const SHIP_COUNT: u16 = 0x3454;
pub const STATION: u16 = 0x0399;
pub const STATION_ACTIVE: u16 = 0x0395;
pub const TACTICAL_DRAWN: u16 = 0x0397;
pub const PREVIOUS_STATION: u16 = 0x039B;
pub const ENCOUNTER: u16 = 0x01C3;
pub const TACTICAL_X: u16 = 0x01C5;
pub const TACTICAL_Y: u16 = 0x01C7;
pub const DRAW_COUNTER: u16 = 0x01B6;
pub const KEY: u16 = 0x483A;
pub const PENDING_KEY: u16 = 0x463C;
pub const MOUSE_BUTTONS: u16 = 0x461D;
pub const VIDEO_MODE: u16 = 0x2796;
pub const ACTIVE_PAGE: u16 = 0x2A5A;
pub const ENCOUNTERS: u16 = 0x3AB0;
pub const ENEMIES: u16 = 0x3CB9;
pub const ENEMY_SIZE: u16 = 30;
pub const GRID: u16 = 0x3882;
pub const ICON_SAVED: u16 = 0x4628;
pub const SYSTEM_NAMES: u16 = 0x0481;
pub const WORLD_NAMES: u16 = 0x04A1;
pub const WORLD_BASE: u16 = 0x039F;
pub const DANGER: u16 = 0x03B1;

pub fn ds_image(s_dat: &[u8]) -> Result<Vec<u8>, String> {
    let exe = exepack::unpack(s_dat).map_err(|e| format!("S.DAT: {e:?}"))?.exe;
    let word = |at: usize| exe.get(at..at + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize);
    let header = word(8).ok_or("S.DAT: short header")? * 16;
    let start = 0x161D0;
    let mut ds = exe.get(header + start..).filter(|b| b.len() >= 0x343E).ok_or("S.DAT: short DGROUP")?.to_vec();
    ds.resize(65536, 0);
    let count = word(6).ok_or("S.DAT: short relocation header")?;
    let table = word(0x18).ok_or("S.DAT: short relocation table")?;
    for i in 0..count {
        let at = word(table + 4 * i).ok_or("S.DAT: short relocation")? + 16 * word(table + 4 * i + 2).ok_or("S.DAT: short relocation")?;
        if let Some(off) = at.checked_sub(start).filter(|&n| n + 1 < 65536) {
            let value = u16::from_le_bytes([ds[off], ds[off + 1]]).wrapping_add(0x1000);
            ds[off..off + 2].copy_from_slice(&value.to_le_bytes());
        }
    }
    Ok(ds)
}

pub struct State { ds: Vec<u8>, pub party: Vec<u8>, pub world: Vec<u8> }
impl State {
    pub fn new(s_dat: &[u8], party: Vec<u8>, world: Vec<u8>) -> Result<Self, String> { Self::from_ds(ds_image(s_dat)?, party, world) }
    pub fn from_ds(mut ds: Vec<u8>, party: Vec<u8>, world: Vec<u8>) -> Result<Self, String> {
        if party.len() != PARTY_SIZE { return Err("S: invalid party block length".into()); }
        ds.resize(65536, 0);
        let mut s = Self { ds, party, world };
        s.set_word(PARTY_PTR, 0); s.set_word(PARTY_PTR + 2, PARTY_SEG as i16);
        Ok(s)
    }
    pub fn image(&self) -> &[u8] { &self.ds }
    pub fn image_mut(&mut self) -> &mut [u8] { &mut self.ds }
    pub fn byte(&self, off: u16) -> u8 { self.ds[off as usize] }
    pub fn set_byte(&mut self, off: u16, value: u8) { self.ds[off as usize] = value; }
    pub fn word(&self, off: u16) -> i16 { i16::from_le_bytes([self.byte(off), self.byte(off.wrapping_add(1))]) }
    pub fn set_word(&mut self, off: u16, value: i16) { let b = value.to_le_bytes(); self.set_byte(off, b[0]); self.set_byte(off.wrapping_add(1), b[1]); }
    pub fn dword(&self, off: u16) -> u32 { self.word(off) as u16 as u32 | ((self.word(off.wrapping_add(2)) as u16 as u32) << 16) }
    pub fn set_dword(&mut self, off: u16, value: u32) { self.set_word(off, value as i16); self.set_word(off.wrapping_add(2), (value >> 16) as i16); }
    pub fn array(&self, off: u16, index: i16) -> i16 { self.word(off.wrapping_add((index as u16).wrapping_mul(2))) }
    pub fn set_array(&mut self, off: u16, index: i16, value: i16) { self.set_word(off.wrapping_add((index as u16).wrapping_mul(2)), value); }
    pub fn pbyte(&self, off: u16) -> u8 { self.party.get(off as usize).copied().unwrap_or(0) }
    pub fn set_pbyte(&mut self, off: u16, value: u8) { if let Some(b) = self.party.get_mut(off as usize) { *b = value; } }
    pub fn pword(&self, off: u16) -> i16 { i16::from_le_bytes([self.pbyte(off), self.pbyte(off.wrapping_add(1))]) }
    pub fn set_pword(&mut self, off: u16, value: i16) { let b = value.to_le_bytes(); self.set_pbyte(off, b[0]); self.set_pbyte(off.wrapping_add(1), b[1]); }
    pub fn string(&self, off: u16) -> Vec<u8> { (0..65536).map(|i| self.byte(off.wrapping_add(i as u16))).take_while(|&b| b != 0).collect() }
    pub fn pointer_string(&self, off: u16) -> Option<Vec<u8>> {
        (self.word(off.wrapping_add(2)) as u16 == DS).then(|| self.string(self.word(off) as u16))
    }
    pub fn enc(&self, i: i16, off: u16) -> i16 { self.word(ENCOUNTERS.wrapping_add((i as u16).wrapping_mul(10)).wrapping_add(off)) }
    pub fn set_enc(&mut self, i: i16, off: u16, v: i16) { self.set_word(ENCOUNTERS.wrapping_add((i as u16).wrapping_mul(10)).wrapping_add(off), v) }
    pub fn enemy(&self, i: i16, off: u16) -> i16 { self.word(ENEMIES.wrapping_add((i as u16).wrapping_mul(ENEMY_SIZE)).wrapping_add(off)) }
    pub fn set_enemy(&mut self, i: i16, off: u16, v: i16) { self.set_word(ENEMIES.wrapping_add((i as u16).wrapping_mul(ENEMY_SIZE)).wrapping_add(off), v) }
    pub fn srand(&mut self, seed: u16) { self.set_dword(RNG, seed as u32); }
    pub fn rand_raw(&mut self) -> i16 { let mut r = TcRand { seed: self.dword(RNG) }; let n = r.next(); self.set_dword(RNG, r.seed); n as i16 }
}
