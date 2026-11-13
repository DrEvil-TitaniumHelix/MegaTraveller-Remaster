mod oracle_env;
use mt1::space::{jump::{self, Call, Host, Party, State, Tables}, jump_render::Surface};
use mt_engine::{data::{exepack, ly}, present::{fb::Fb, text::Font}};
use std::{fs, io::{Cursor, Read}, path::PathBuf, process::Command};

fn read<const N: usize>(r: &mut Cursor<Vec<u8>>) -> [u8; N] { let mut b = [0; N]; r.read_exact(&mut b).unwrap(); b }
fn i16r(r: &mut Cursor<Vec<u8>>) -> i16 { i16::from_le_bytes(read(r)) }
fn u32r(r: &mut Cursor<Vec<u8>>) -> u32 { u32::from_le_bytes(read(r)) }
fn state(r: &mut Cursor<Vec<u8>>) -> State {
    State { video_mode: i16r(r), world_time: i16r(r), system_encounter: i16r(r), view_heading: i16r(r),
        minimap_quadrant: i16r(r), thrust: i16r(r), heading: i16r(r), moving: i16r(r) }
}
fn blob(r: &mut Cursor<Vec<u8>>) -> Vec<u8> { let n = u32r(r); let mut b = vec![0; n as usize]; r.read_exact(&mut b).unwrap(); b }

struct Driver { trace: String, draws: usize, rand: [u16; 3], choice: i16, view: i16, heading: i16, thrust: i16 }
impl Host for Driver {
    fn call(&mut self, c: Call, s: &mut State, _: &mut Party) -> Result<i32, String> {
        self.trace.push_str(&format!("{c:?}\n"));
        Ok(match c {
            Call::Rand => { let v = *self.rand.get(self.draws).ok_or("unexpected rand")?; self.draws += 1; v as i32 }
            Call::Select { rects, .. } => if self.choice < 0 { -1 } else { self.choice as i32 % rects.len() as i32 },
            Call::SystemView(_) => { s.heading = self.heading; s.thrust = self.thrust; self.view as i32 }
            _ => 0,
        })
    }
}
struct ClockDriver { trace: String, ticks: u32, period: u32, delta: u32, calls: u32 }
impl Host for ClockDriver {
    fn call(&mut self, c: Call, _: &mut State, _: &mut Party) -> Result<i32, String> {
        self.trace.push_str(&format!("{c:?}\n"));
        if c == Call::Ticks {
            self.calls += 1;
            if self.period == 0 {
                let frame = (self.calls - 1) / 2; let second = (self.calls - 1) % 2;
                self.ticks = if frame == 60 { if second == 0 { u32::MAX } else { 1 } } else { frame * 4 + second * 3 };
            } else if self.calls % self.period == 0 { self.ticks = self.ticks.wrapping_add(self.delta); }
            Ok(self.ticks as i32)
        } else { Ok(0) }
    }
}
struct FrameDriver<'a>(Surface<'a>);
impl Host for FrameDriver<'_> {
    fn call(&mut self, c: Call, _: &mut State, _: &mut Party) -> Result<i32, String> {
        if matches!(c, Call::Select { .. }) { return Err("input boundary".into()); }
        self.0.apply(&c); Ok(0)
    }
}

#[test]
fn jump_and_animation_match_spec_and_menu_planes() {
    let Some((oracle, game)) = oracle_env::both() else { return };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let out = root.join("target/m6-jump-oracle");
    let output = Command::new("python").arg("-B").arg(root.join("tools/run_watch.py"))
        .args(["--seconds", "240", "--mib", "1536", "--", "python", "-B"])
        .arg(root.join("crates/mt1/tests/space_jump_oracle.py")).arg(&oracle).arg(&game).arg(&out)
        .env("PYTHONDONTWRITEBYTECODE", "1").output().unwrap();
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let game = PathBuf::from(game);
    let exe = exepack::unpack(&fs::read(game.join("S.DAT")).unwrap()).unwrap().exe;
    let tables = Tables::from_exe(&exe).unwrap();
    assert_eq!(tables.names[3], b"louzy");
    let mut input = Cursor::new(fs::read(out.join("cases.bin")).unwrap());
    let mut count = 0;
    while input.position() < input.get_ref().len() as u64 {
        let mut party: Party = read(&mut input); let mut s = state(&mut input); let mut t = tables.clone();
        t.links = read(&mut input); for d in &mut t.danger { *d = i16r(&mut input); }
        let choice = i16r(&mut input); let rand = std::array::from_fn(|_| i16r(&mut input) as u16);
        let mut driver = Driver { trace: String::new(), draws: 0, rand, choice, view: i16r(&mut input),
            heading: i16r(&mut input), thrust: i16r(&mut input) };
        let want: Party = read(&mut input); let want_state = state(&mut input); let draws = i16r(&mut input);
        let trace = blob(&mut input);
        jump::jump_screen(&mut driver, &mut s, &mut party, &t).unwrap();
        assert_eq!(party.as_slice(), want.as_slice(), "party case {count}");
        assert_eq!(s, want_state, "state case {count}");
        assert_eq!(driver.draws, draws as usize, "rand case {count}");
        assert_eq!(driver.trace, String::from_utf8(trace).unwrap(), "call trace case {count}");
        count += 1;
    }
    assert_eq!(count, 4880); println!("PASS {count} jump cases: party, state, RNG draws and ordered calls");
    let mut input = Cursor::new(fs::read(out.join("animation.bin")).unwrap());
    let mut count = 0;
    while input.position() < input.get_ref().len() as u64 {
        let mut driver = ClockDriver { trace: String::new(), ticks: u32r(&mut input), period: u32r(&mut input), delta: u32r(&mut input), calls: 0 };
        let heading = i16r(&mut input); let calls = u32r(&mut input); let trace = blob(&mut input);
        let mut s = State { heading: 97, ..State::default() };
        jump::animate_jump_transition(&mut driver, &mut s, &mut [0; 0xF86]).unwrap();
        assert_eq!(s.view_heading, heading); assert_eq!(driver.calls, calls);
        assert_eq!(driver.trace.as_bytes(), trace, "animation case {count}"); count += 1;
    }
    assert_eq!(count, 19); println!("PASS {count} animation traces, including deadline wrap and palette flashes");
    let font = Font::new(ly::decode(&fs::read(game.join("FONT.LY")).unwrap()).unwrap(),
        fs::read(out.join("font-widths.bin")).unwrap().try_into().unwrap(),
        &fs::read(out.join("font-flags.bin")).unwrap().try_into().unwrap(), 8);
    let layouts = ly::decode(&fs::read(game.join("STATIONS.LY")).unwrap()).unwrap();
    let start = fs::read(out.join("frame-start.bin")).unwrap();
    let frames = fs::read_to_string(out.join("frames.txt")).unwrap();
    for row in frames.lines() {
        let fields: Vec<&str> = row.split(' ').collect();
        let num = |i: usize| fields[i].parse::<u8>().unwrap();
        let mut party = [0; 0xF86]; party[6] = num(1); party[0xFC] = num(2); party[0xC1] = num(3);
        for (i, p) in fields[5].split(',').filter(|s| !s.is_empty()).enumerate() { party[0xAC+i] = p.parse::<u8>().unwrap() | 0x20; }
        let mut fb = Fb::new(); fb.planes.copy_from_slice(&start);
        let mut driver = FrameDriver(Surface { fb: &mut fb, layouts: &layouts, font: &font,
            active_page: num(4) as usize, page: 0, color: 0, y: 0, x: 0 });
        assert_eq!(jump::jump_screen(&mut driver, &mut State::default(), &mut party, &tables), Err("input boundary".into()));
        let want = fs::read(out.join(format!("{}.planes", fields[0]))).unwrap();
        assert_eq!(fb.planes.iter().zip(want).position(|(a,b)| *a != b), None, "frame {row}");
    }
    assert_eq!(frames.lines().count(), 160); println!("PASS 160 jump frames: all four 64 KiB planes exact");
}

#[test]
fn program_slots_and_invalid_data() {
    let mut party = [0; 0xF86];
    for slot in 0..20 { for byte in 0..=255 {
        party[0xAC+slot] = byte;
        for program in 0..=16 {
            assert_eq!(jump::is_program_loaded(&party, program), byte & 0x20 != 0 && program == u16::from(byte & 15));
        }
        party[0xAC+slot] = 0;
    }}
    assert!(Tables::from_ds(&[]).is_err()); assert!(Tables::from_exe(&[]).is_err());
}

#[test]
fn zz_skip_report() { oracle_env::report() }
