use crate::ground::{commands, input::{dialogue, ui}, render, starport::shop_menu, Ground};
use mt_engine::{data::{pic, Variant}, present::prim::Raster};

fn winning_code(timer: u16) -> [u8; 28] {
    code((3, 30), timer, *b"010190", 0xFC, (1990, 10, 4))
}

fn code(dos: (u8, u8), timer: u16, bios_date: [u8; 6], model: u8, date: (u16, u8, u8)) -> [u8; 28] {
    let mut out = [0; 28];
    out[..7].copy_from_slice(b"ZDXTRQ1");
    let hex = b"0123456789ABCDEF";
    out[7] = dos.0.wrapping_add(b'0');
    out[8] = dos.1 / 10 + b'0'; out[9] = dos.1 % 10 + b'0';
    for (offset, byte) in [(10, timer as u8), (12, (timer >> 8) as u8), (20, model)] {
        out[offset] = hex[(byte >> 4) as usize]; out[offset + 1] = hex[(byte & 15) as usize];
    }
    out[14..20].copy_from_slice(&bios_date);
    for (offset, value) in [(22, date.1 as u16), (24, date.2 as u16), (26, date.0.wrapping_sub(1900))] {
        out[offset] = (value / 10).wrapping_add(b'0' as u16) as u8;
        out[offset + 1] = (value % 10) as u8 + b'0';
    }
    out
}

fn pages(g: &mut Ground, messages: [u16; 3]) -> Result<(), String> {
    if g.mem.u8(0x7011) == 0 { while g.mem.key() != 0 { g.poll() } }
    g.screen_kind(crate::ui::ScreenKind::Picture);
    render::cursor_off(g);
    dialogue::set_text_box(g, 2);
    render::dismiss_message(g, 1);
    ui::layout(g, 1, 54, 0);
    g.mem.set_u8(0xE5 + g.mem.u16(0x52F4), 100);
    commands::window(g, 54, 0, 146, 40);
    let picture = g.files().read("A2.PIC").ok_or("ending: missing A2.PIC")?;
    let picture = pic::decode(Variant::Mt1, &picture).map_err(|e| format!("ending: A2.PIC: {e:?}"))?;
    {
        let mut p = render::paint::Paint::new(g, Raster::Planar);
        p.fill(55, 192, 146, 128, 7);
        p.picture("A2.PIC", 0, 60, 200, 0, 0, picture.h as i32, mt_engine::present::pic::width(&picture) as i32, None);
        p.finish();
    }
    render::flip(g);
    dialogue::say_message(g, messages[0] as i16, false)?;
    g.get_key();
    let winning = winning_code(g.now() as u16);
    for message in &messages[1..] {
        dialogue::say_message(g, *message as i16, false)?;
        g.get_key();
    }
    ui::target(g, true);
    ui::layout(g, 1, 54, 0);
    shop_menu::say(g, b"Here is your special winning code number!:", true, 6);
    g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
    shop_menu::say(g, &winning, false, 4);
    g.mem.set_u8(0x6E2A, g.mem.u8(0x6E2A).wrapping_add(1));
    shop_menu::say(g, b"Record it and press ESC to return to DOS", false, 6);
    while g.get_key() != 27 {}
    Ok(())
}

pub(super) fn run(g: &mut Ground, messages: [u16; 3]) -> ! {
    if let Err(e) = pages(g, messages) { eprintln!("ground: {e}"); g.dump_step(); std::process::exit(3) }
    crate::ground::game_shutdown(g);
    if g.scene.is_some() && !g.harness && g.lockstep.is_none() {
        g.game_over = true;
        std::panic::resume_unwind(Box::new(crate::ground::main_loop::GameOver));
    }
    g.dump_step();
    std::process::exit(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use mt_engine::{io::{Dir, Files, Scripted}, sim::tick::Counter};

    fn game() -> Option<Ground> {
        let dir = Dir(oracle_env::gamefiles()?);
        let mut g = Ground::new(&dir.read("T.DAT").unwrap(), dir.read("DEFAULT.SAV").unwrap(),
            Box::new(Scripted::new([])), Box::new(Counter::at(0x1234)), Box::new(dir)).unwrap();
        crate::ground::start(&mut g);
        g.scenario_boot = true; g.sound_option = b'A';
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        g.mem.set_party_u8(0xC1, g.mem.party_u8(0xC1) & !128);
        assert!(render::game_init(&mut g));
        g.mem.set_key(0);
        g.input = Box::new(Scripted::new([32, 32, 32, 13, 27]));
        Some(g)
    }

    #[test]
    fn deferred_ending_never_loads_message_189_as_a_world_area() {
        let Some(mut g) = game() else { return };
        g.scene = Some(crate::ui::SceneHook::default());
        let (party, planes, rng) = (g.mem.party().to_vec(), g.fb.planes.clone(), g.rand.lcg.seed);
        crate::ground::walk::enter_world(&mut g, 2, 189, 190, 191);
        assert_eq!(g.mem.world_pending(), 1);
        assert_eq!(g.mem.party(), party); assert_eq!(g.fb.planes, planes);
        let end = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            crate::ground::walk::enter_world(&mut g, 2, 189, 190, 191);
        })).unwrap_err();
        assert!(end.is::<crate::ground::main_loop::GameOver>());
        assert!(g.game_over); assert_eq!(g.rand.lcg.seed, rng);
        assert_eq!((g.mem.party_u8(6), g.mem.party_u8(7)), (party[6], party[7]));
        assert_eq!(g.mem.party_u8(0x128), 45);
        assert_eq!(g.mem.key(), 0);
        let picture = pic::decode(Variant::Mt1, &g.files().read("A2.PIC").unwrap()).unwrap();
        let pixels = mt_engine::present::pic::unpack(&picture);
        let width = mt_engine::present::pic::width(&picture);
        let page = (g.mem.u16(0x52F4) & 1) as usize;
        for y in 0..picture.h as usize { for x in 0..width {
            assert_eq!(g.fb.get(page, 60 + y as i32, 200 + x as i32), pixels[y * width + x], "A2 at ({y},{x})");
        } }
    }

    #[test]
    fn faithful_ending_exits_one_after_the_code_panel() {
        const CHILD: &str = "MT_ENDING_EXIT_TEST";
        if std::env::var_os(CHILD).is_some() {
            let mut g = game().expect("child requires game files");
            g.mem.set_world_pending(1);
            crate::ground::walk::enter_world(&mut g, 2, 189, 190, 191);
            panic!("faithful ending returned");
        }
        if oracle_env::gamefiles().is_none() { return }
        let out = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "ground::walk::ending::tests::faithful_ending_exits_one_after_the_code_panel"])
            .env(CHILD, "1").output().unwrap();
        assert_eq!(out.status.code(), Some(1), "{}", String::from_utf8_lossy(&out.stderr));
    }

    #[test]
    fn winning_code_and_key_sequence_match_original_machine_code() {
        let Some(oracle) = oracle_env::oracle() else { return };
        let out = std::process::Command::new("python")
            .args(["-B", "-c", include_str!("../../../../tests/atoms/169_oracle.py")])
            .arg(oracle).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let text = String::from_utf8(out.stdout).unwrap();
        let cases: Vec<_> = text.lines().collect();
        assert_eq!(cases.len(), 8);
        for case in cases {
            let fields: Vec<_> = case.split('\t').collect();
            let number = |i: usize| fields[i].parse::<u16>().unwrap();
            let actual = code((number(0) as u8, number(1) as u8), number(2),
                fields[3].as_bytes().try_into().unwrap(), number(4) as u8,
                (number(5), number(6) as u8, number(7) as u8));
            assert_eq!(String::from_utf8_lossy(&actual), fields[8], "{case}");
        }
    }
}
