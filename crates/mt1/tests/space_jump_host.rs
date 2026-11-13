mod oracle_env;
use mt1::space::{self, Space, jump::Tables, state::*, system::COUNT, tick::Hooks};
use mt_engine::{io::{Dir, Scripted}, sim::tick::Clock};

struct Auto(u32);
impl Clock for Auto { fn now(&mut self) -> u32 { self.0 += 1; self.0 } fn set(&mut self, n: u32) { self.0 = n } fn wait_tick(&mut self) { self.0 += 1 } }

enum Act { Set(Box<dyn Fn(&mut Space)>), Key(u8), Check(Box<dyn Fn(&Space)>) }
struct Script { acts: std::collections::VecDeque<Act>, polls: u32, last: u32 }
impl Hooks for Script {
    fn update_cursor(&mut self, sp: &mut Space) {
        self.polls += 1;
        assert!(self.polls < 5_000_000, "poll budget");
        if sp.s.byte(KEY) != 0 || sp.s.byte(PENDING_KEY) != 0 || self.polls - self.last < 40 { return }
        while let Some(act) = self.acts.pop_front() {
            match act {
                Act::Set(f) => f(sp),
                Act::Check(f) => f(sp),
                Act::Key(k) => { sp.keyboard_isr(k); sp.keyboard_isr(k | 0x80); self.last = self.polls; break }
            }
        }
    }
}
const KJ: u8 = 0x24; const KF1: u8 = 0x3B; const KQ: u8 = 0x10; const KY: u8 = 0x15;

#[test]
fn forced_misjumps_and_a_real_jump_on_the_core_host() {
    let Some(dir) = oracle_env::gamefiles() else { return };
    let s_dat = std::fs::read(dir.join("S.DAT")).unwrap();
    let tables = Tables::from_ds(&space::state::ds_image(&s_dat).unwrap()).unwrap();
    let mut party = mt1::ground::party::initial();
    party[6] = 2; party[7] = 255; party[0xFC] = 100; party[0xC1] = 1; party[0xC0] = 2;
    for (i, p) in [163u8, 0, 164, 1, 5].iter().enumerate() { party[0xAC + i] = *p; }
    party[0xF84] = 0; party[0xF85] = 0;
    let mut sp = Space::new(&s_dat, party, vec![0; 0x2000], Box::new(Scripted::new([])), Box::new(Auto(1000)), Box::new(Dir(dir))).unwrap();
    let expect = |sp: &Space, system: u8, fuel: u8, clock: i16, x: i16, y: i16, count: i16, tag: &str| {
        assert_eq!((sp.s.pbyte(6), sp.s.pbyte(7), sp.s.pbyte(0xFC), sp.s.pword(0xF84)), (system, 255, fuel, clock), "{tag} party");
        assert_eq!((sp.s.word(X), sp.s.word(Y), sp.s.word(COUNT)), (x, y, count), "{tag} position / objects");
    };
    let t = tables.clone();
    let first_row = move |sp: &Space| { let m = space::jump::menu(&sp.s.party.as_slice().try_into().unwrap(), &t).unwrap(); t.system_for_row[m.rows[0] as usize] };
    let acts: Vec<Act> = vec![
        Act::Set(Box::new(|sp| { sp.s.set_word(CURRENT, -1); sp.s.set_dword(RNG, 965963027) })), Act::Key(KJ), Act::Key(KF1),
        Act::Check(Box::new(move |sp| expect(sp, 6, 50, 7, 129, 19, 49, "misjump 1"))),
        Act::Set(Box::new(|sp| { sp.s.set_dword(RNG, 2606494563); sp.s.set_pbyte(0xFC, 100) })), Act::Key(KJ), Act::Key(KF1),
        Act::Check(Box::new(move |sp| expect(sp, 0, 50, 14, 14, 94, 21, "misjump 2"))),
        Act::Set(Box::new(|sp| sp.s.set_pbyte(0xFC, 124))),
        Act::Check(Box::new(move |sp| { let d = first_row(sp); assert_ne!(d, 0); DEST.with(|c| c.set(d)); VIEWS.with(|c| c.set(sp.ev.view)) })),
        Act::Key(KJ), Act::Key(KF1),
        Act::Check(Box::new(move |sp| {
            let d = DEST.with(|c| c.get());
            assert_eq!((sp.s.pbyte(6), sp.s.pbyte(7), sp.s.pbyte(0xFC), sp.s.pword(0xF84)), (d, 255, 62, 21), "real jump party");
            assert!(sp.ev.view - VIEWS.with(|c| c.get()) >= 100, "0364 drew fewer than 100 frames");
            assert_eq!(sp.s.word(CURRENT), -1); assert!(sp.s.word(COUNT) > 0);
        })),
        Act::Key(KQ), Act::Key(KY),
    ];
    sp.hooks = Some(Box::new(Script { acts: acts.into(), polls: 0, last: 0 }));
    assert_eq!(sp.space_main(&[b'0', b'A']), space::Exit::GameOver);
}
thread_local! { static DEST: std::cell::Cell<u8> = const { std::cell::Cell::new(0) }; static VIEWS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }

#[test]
fn zz_skip_report() { oracle_env::report() }
