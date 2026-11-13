use super::state::*;
use mt_engine::{data::stp, io::Files};

pub const WORLD: u16 = 0x3578;
pub const ORBIT: u16 = 0x34D8;
pub const PHASE: u16 = 0x3630;
pub const KIND: u16 = 0x37DC;
pub const MOONS: u16 = 0x3963;
pub const SPRITE: u16 = 0x3702;
pub const RADIUS: u16 = 0x3A04;
pub const OBJECT_X: u16 = 0x3B79;
pub const OBJECT_Y: u16 = 0x3C19;
pub const COUNT: u16 = 0x36D0;
pub const DEFAULT_X: u16 = 0x37BA;
pub const DEFAULT_Y: u16 = 0x37C8;
const FRAME: [i16; 58] = [1,1,1,1,1,1,2,2,2,2,2,1,1,1,1,1,1,2,2,2,2,2,2,3,3,3,5,1,1,1,1,1,1,1,1,1,1,2,
    7,6,5,3,0, 0,3,5,6,7, 3,3,2,1,0, 0,1,2,3,3];

pub fn load(s: &mut State, files: &mut dyn Files, system: u8) -> Result<(), String> {
    let name = format!("{system}.STP");
    let data = files.read(&name).ok_or_else(|| format!("Unable to open system file {name}"))?;
    let stp = stp::decode(&data).map_err(|e| format!("{name}: {e:?}"))?;
    load_decoded(s, &stp, system)
}
pub fn load_decoded(s: &mut State, stp: &stp::Stp, system: u8) -> Result<(), String> {
    if stp.objects.len() > OBJECTS || stp.objects.iter().any(|o| o.sprite >= 38) { return Err("STP: unsupported object count/sprite".into()); }
    s.set_byte(0x147D, b'0'.wrapping_add(system));
    s.set_word(COUNT, stp.objects.len() as i16);
    s.set_word(DEFAULT_X, stp.default_x as i16); s.set_word(DEFAULT_Y, stp.default_y as i16);
    for (i, o) in stp.objects.iter().enumerate() {
        for (a, v) in [(WORLD, o.world as i16), (ORBIT, if o.orbit == 255 { 0 } else { o.orbit as i16 + 1 }),
                       (PHASE, o.phase as i16), (KIND, o.angle as i16), (MOONS, o.moons as i16), (SPRITE, o.sprite as i16)] {
            s.set_array(a, i as i16, v);
        }
    }
    coordinates(s)?;
    for i in 0..s.word(COUNT) {
        if s.pbyte(7) as i16 == s.array(WORLD, i).wrapping_sub(1) {
            s.set_word(X, s.array(OBJECT_X, i)); s.set_word(Y, s.array(OBJECT_Y, i)); return Ok(());
        }
    }
    s.set_word(X, s.word(DEFAULT_X)); s.set_word(Y, s.word(DEFAULT_Y)); Ok(())
}
pub fn coordinates(s: &mut State) -> Result<(), String> {
    if !(0..=OBJECTS as i16).contains(&s.word(COUNT)) { return Err("39AF: invalid object count".into()); }
    let (mut moons, mut px, mut py) = (0i16, 0i16, 0i16);
    for i in 0..s.word(COUNT) {
        let orbit = s.array(ORBIT, i);
        let divisor = orbit.wrapping_add(1).wrapping_mul(16);
        if divisor == 0 { return Err("39AF: zero orbit divisor".into()); }
        let phase = s.array(PHASE, i).wrapping_add((s.word(WORLD_TIME) as i32 % divisor as i32) as i16) % 16;
        let (q, j) = (phase / 4, phase % 4);
        if (0..=3).contains(&q) {
            let (cx, sy) = if moons != 0 {
                let d = orbit.wrapping_add(s.array(RADIUS, i));
                (FRAME[(48 + j) as usize].wrapping_mul(d), FRAME[(53 + j) as usize].wrapping_mul(d))
            } else {
                (FRAME[(38 + j) as usize].wrapping_mul(orbit).wrapping_mul(2), FRAME[(43 + j) as usize].wrapping_mul(orbit).wrapping_mul(2))
            };
            let (x, y) = if moons != 0 { (px, py) } else { (186, 186) };
            s.set_array(OBJECT_X, i, if q == 0 || q == 3 { x.wrapping_add(cx) } else { x.wrapping_sub(cx) });
            s.set_array(OBJECT_Y, i, if q == 0 || q == 1 { y.wrapping_sub(sy) } else { y.wrapping_add(sy) });
        }
        let sprite = s.array(SPRITE, i);
        let radius = FRAME.get(sprite as usize).filter(|_| (0..38).contains(&sprite)).ok_or("39AF: invalid sprite")?;
        s.set_array(RADIUS, i, *radius);
        if moons != 0 { moons = moons.wrapping_sub(1); } else if s.array(MOONS, i) != 0 {
            px = s.array(OBJECT_X, i); py = s.array(OBJECT_Y, i); moons = s.array(MOONS, i);
        }
    }
    Ok(())
}
