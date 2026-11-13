use mt_engine::data::{ly::Entry, pic::Pic};
use mt_engine::io::{keys, Input, Key};
use mt_engine::present::{fb::Fb, ly::{self, Pen}, pic as pre_pic, prim::Raster, text::{self, Font}};
use mt_engine::sim::tick::Clock;

const R: Raster = Raster::Wrap;
pub const DS: usize = 0x81F0;
const PAL: usize = 0x0094; const TABLES: usize = 0x02A8; const SECRET: usize = 0x0893; const CAPTION: usize = 0x089C; const WIDTHS: usize = 0x0C0A; const FLAGS: usize = 0x0ECB;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct TcRand { pub seed: u32 }
impl TcRand { pub fn next(&mut self) -> u32 { self.seed = self.seed.wrapping_mul(22695477).wrapping_add(1); (self.seed >> 16) & 0x7FFF } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit { Game, Quit }

pub struct Title { pub fb: Fb, pub display: usize, layouts: Vec<Entry>, font: Font, caption: Vec<u8>, secret: Vec<u8>, widths: [u8; 8], xs: [u8; 8], ys: [u8; 8], tit: Pic, zod: Pic, pen: Pen }
fn cstr(d: &[u8], at: usize) -> Vec<u8> { d[at..].iter().take_while(|&&b| b != 0).copied().collect() }
impl Title {
    pub fn new(startup: &[u8], layouts: Vec<Entry>, font_ly: Vec<Entry>, tit: Pic, zod: Pic) -> Option<Title> {
        let base = u16::from_le_bytes([*startup.get(8)?, *startup.get(9)?]) as usize * 16 + DS;
        let font = Font::from_tables(font_ly, startup, base + WIDTHS, base + FLAGS, 8)?;
        let t = |k: usize| -> Option<[u8; 8]> { startup.get(base + TABLES + 8 * k..base + TABLES + 8 * k + 8)?.try_into().ok() };
        let mut fb = Fb::new();
        for (i, &v) in startup.get(base + PAL..base + PAL + 16)?.iter().enumerate() { fb.set_reg(i, v) }
        Some(Title { fb, display: 0, layouts, font, caption: cstr(startup, base + CAPTION), secret: cstr(startup, base + SECRET), widths: t(0)?, xs: t(1)?, ys: t(2)?, tit, zod, pen: Pen::new(0) })
    }
    fn back(&self) -> usize { self.display ^ 1 }
    fn layout(&mut self, i: usize, y: i32, x: i32) { let p = self.back(); ly::layout(&mut self.fb, R, p, &self.layouts, i, y, x, &mut self.pen) }
    pub fn star_field(&mut self, rand: &mut dyn FnMut() -> u32) {
        let p = self.back();
        for &(n, c) in &[(40, 8u8), (30, 3), (23, 7), (20, 14)] { for _ in 0..n { let x = (rand() % 320) as i32; let y = (rand() % 200) as i32; self.fb.pixel(R, p, y, x, c, 0) } }
        for &(i, n, h, w) in &[(0usize, 15, 186, 316), (1, 10, 184, 314), (2, 5, 180, 310)] { for _ in 0..n { let x = (rand() % w) as i32; let y = (rand() % h) as i32; self.layout(i, y, x) } }
    }
    pub fn copy_to_front(&mut self) { let (b, d) = (self.back(), self.display); self.fb.copy(b, 0, 0, 200, 40, d, 0, 0) }
    fn picture(&mut self, pic: &Pic, y: i32, x: i32, sy: i32, sx: i32, h: i32, w: i32, op: u8, mask: bool) {
        let (px, pw, p) = (pre_pic::unpack(pic), pre_pic::width(pic) as i32, self.back());
        if !pre_pic::consistent(pic) { return }
        for row in 0..h { for col in 0..w * 8 {
            let (py, pxx) = (sy + row, sx * 8 + col);
            if !(0..pic.h as i32).contains(&py) || !(0..pw).contains(&pxx) { continue }
            let ink = px[(py * pw + pxx) as usize];
            self.fb.pixel(R, p, y + row, x * 8 + col, if mask { if ink == 0 { 15 } else { 0 } } else { ink }, op);
        } }
    }
    pub fn title_objects(&mut self) {
        self.layout(3, 0, 11); self.layout(4, 8, 0);
        self.pen.colour = 0;
        let (caption, p) = (self.caption.clone(), self.back());
        let mut x = 41i64;
        for &c in &caption { x = text::glyph(&mut self.fb, R, p, &self.font, c, 41, x as i32, &mut self.pen, 0, None) }
        let (tit, h, w) = (self.tit.clone(), self.tit.h as i32, self.tit.wbytes4 as i32);
        self.picture(&tit, 150, 8, 0, 0, h, w, 8, true); self.picture(&tit, 150, 8, 0, 0, h, w, 24, false);
    }
    pub fn frame(&mut self, f: usize) {
        let f = f & 7; let p = self.back();
        self.fb.fill(R, p, 62, 120, 81, 80, 0, 0);
        let zod = self.zod.clone();
        self.picture(&zod, 62, 15 + self.ys[f] as i32, 0, self.xs[f] as i32, 81, self.widths[f] as i32, 0, false);
        self.display ^= 1;
    }
    pub fn run(&mut self, rand: &mut dyn FnMut() -> u32, clock: &mut dyn Clock, input: &mut dyn Input, mut show: impl FnMut(&Title, &mut dyn Input)) -> Exit {
        let mut wait = |t: &Title, clock: &mut dyn Clock, input: &mut dyn Input, ticks: u32| { show(t, input); let end = clock.now().wrapping_add(ticks); while (clock.now().wrapping_sub(end) as i32) < 0 { input.key_pending(); clock.wait_tick() } };
        let mut play_draws = mt_engine::play::Fnv::default();
        if input.wants_decision() { self.star_field(&mut || { let n=rand(); play_draws.write(&n.to_le_bytes()); n }); }
        else { self.star_field(rand); }
        self.copy_to_front();
        wait(self, clock, input, 36);
        self.title_objects(); self.copy_to_front();
        let (mut f, mut step, mut cursor, mut first) = (4usize, 1usize, 0usize, true);
        loop {
            self.frame(f); f = (f + step) & 7;
            if input.wants_decision() {
                let mut d = crate::play_prompts::keys_decision("title","0297",mt_engine::play::Kind::Ack,
                    &[("start","Start game",keys::ENTER),("quit","Quit",keys::ESC)]);
                let mut h=mt_engine::play::Fnv::default(); h.write(&mt_engine::play::HASH_VERSION.to_le_bytes()); h.field(b"title");
                h.write(&play_draws.finish().to_le_bytes()); h.write(&clock.now().to_le_bytes()); h.write(&clock.ticks().to_le_bytes());
                for value in [f,step,cursor,self.display] { h.write(&(value as u64).to_le_bytes()); }
                h.write(&[first as u8,self.pen.colour,self.pen.op]); h.field(&self.fb.regs); d.state=h.state();
                input.present(&self.fb.rgb(self.display,&mt_engine::present::palette::ega_dac()));
                if input.data_mode() { input.scene(&crate::play::observation::Observation::program(crate::ui::ScreenKind::Startup, clock.now())); }
                input.decide(&d);
            }
            if input.key_pending() {
                let k: Key = input.next_key();
                if k == keys::ESC { return Exit::Quit }
                let a = if k < 0x100 { k as u8 } else { 0 };
                if cursor >= self.secret.len() || a != self.secret[cursor] { return Exit::Game }
                cursor += 1; if cursor == self.secret.len() { step = 7 }
            }
            wait(self, clock, input, if first { 8 } else { 2 }); first = false;
        }
    }
}
#[cfg(test)]
mod t {
mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::data::ly::Rec;
    use mt_engine::sim::tick::Counter;
    fn startup_tables() -> Option<(Vec<u8>, Vec<u8>)> {
        let gf = oracle_env::gamefiles()?;
        let real = mt_engine::data::exepack::unpack(&std::fs::read(std::path::Path::new(&gf).join("STARTUP.EXE")).ok()?).ok()?.exe;
        let b = u16::from_le_bytes([real[8], real[9]]) as usize * 16 + DS;
        Some((real.get(b + PAL..b + PAL + 16)?.to_vec(), real.get(b + TABLES..b + TABLES + 24)?.to_vec()))
    }
    fn title(pal: &[u8], tables: &[u8]) -> Title {
        let mut s = vec![0u8; 0x81F0 + 0x1000];
        s[0x81F0 + PAL..0x81F0 + PAL + 16].copy_from_slice(pal);
        s[0x81F0 + TABLES..0x81F0 + TABLES + 24].copy_from_slice(tables);
        s[0x81F0 + SECRET..0x81F0 + SECRET + 2].copy_from_slice(b"mi"); s[0x81F0 + CAPTION..0x81F0 + CAPTION + 1].copy_from_slice(b"a"); s[0x81F0 + WIDTHS + b'a' as usize] = 3;
        let mut g: Vec<Entry> = vec![vec![]; 0x62]; g[b'a' as usize] = vec![Rec { op: 4, colour: 16, y1: 0, x1: 0, y2: 0, x2: 0 }];
        let lay: Vec<Entry> = (0..5).map(|i| vec![Rec { op: 4, colour: 9 + i, y1: 0, x1: 0, y2: 0, x2: 0 }]).collect();
        let pic = |wb: u16| Pic { h: 81, wbytes4: wb, pal: [0; 16], px: vec![0x12; 81 * wb as usize * 4], tail: vec![] };
        Title::new(&s, lay, g, pic(16), pic(64)).unwrap()
    }
    #[test]
    fn star_field_title_and_frames() {
        let Some((pal, tables)) = startup_tables() else { println!("SKIP: MT1_GAMEFILES unset or STARTUP.EXE unreadable; title unit test not run"); return };
        let title = || title(&pal, &tables);
        let mut t = title();
        assert_eq!(t.fb.regs[1], pal[1]);
        let mut r = TcRand { seed: 0 }; assert_eq!(r.next(), 0);
        let mut v = (0..).map(|i| i as u32 * 7); let mut rand = move || v.next().unwrap();
        t.star_field(&mut rand);
        assert_eq!((t.fb.get(1, 7, 0), t.fb.get(0, 7, 0)), (8, 0));
        t.copy_to_front(); assert_eq!(t.fb.get(0, 7, 0), 8);
        t.title_objects();
        assert_eq!(t.fb.get(1, 0, 11), 12); assert_eq!(t.fb.get(1, 8, 0), 13);
        assert_eq!(t.fb.get(1, 41, 41), 0);
        assert_eq!((t.fb.get(1, 150, 64), t.fb.get(1, 150, 65)), (1, 2));
        let d = t.display; t.frame(4); assert_eq!(t.display, d ^ 1);
        let x0 = (15 + tables[16 + 4] as i32) * 8;
        assert_eq!((t.fb.get(1, 62, x0), t.fb.get(1, 62, x0 + 1), t.fb.get(1, 62, x0 - 1)), (1, 2, 0));
        struct Keys(std::collections::VecDeque<Key>);
        impl Input for Keys { fn next_key(&mut self) -> Key { self.0.pop_front().unwrap_or(27) } fn key_pending(&mut self) -> bool { !self.0.is_empty() } }
        let (mut c, mut i) = (Counter::default(), Keys([b'm' as Key, b'i' as Key, 13].into()));
        let mut t = title();
        assert_eq!(t.run(&mut rand, &mut c, &mut i, |_, _| {}), Exit::Game);
        let mut e = Keys([27].into()); assert_eq!(t.run(&mut rand, &mut c, &mut e, |_, _| {}), Exit::Quit);
    }
}
