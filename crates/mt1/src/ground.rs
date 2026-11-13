pub mod state;
pub mod trace;
pub mod party;
pub mod main_loop;
pub mod loop_vars;
pub mod render;
pub mod walk;
pub mod combat;
pub mod input;
pub mod commands;
pub mod commands_party;
pub mod npc_turn;
pub mod tactics; pub mod standing; pub mod keymap;
pub mod commands_items;
pub mod commands_talk;
pub mod commands_character;
pub mod starport;
pub mod squad;
pub mod rules;
pub mod round;
pub mod orders; pub mod seen;
pub mod fire;
pub use state::Ground;

pub trait Tooling {
    fn poll(&mut self, game: &mut Ground) -> bool;
    fn pending(&self) -> bool { false }
    fn last_error(&self) -> Option<String> { None }
    fn space_launched(&mut self, _space: &mut crate::space::Space) -> Result<(), String> { Ok(()) }
    fn space_seed(&mut self, _prog: &str) -> Option<u16> { None }
}

pub fn game_shutdown(g: &mut Ground) {
    for i in 0..5u16 { g.mem.set_party_u8(0x128 + (i << 8), 0x2D); }
    let mut used = [false; 256];
    for i in 0..5u16 {
        let rec = 0x102 + (i << 8);
        if g.mem.party_u8(rec + 0x28) & 2 != 0 { continue }
        for j in 0..23u16 {
            let item = g.mem.party_u16(rec + 0xCC + 2 * j);
            if item != 0 { if let Some(u) = used.get_mut(item.wrapping_sub(1) as usize) { *u = true } }
        }
    }
    for (i, u) in used.iter().enumerate() { if !u { g.mem.set_party_u8(0xA82 + 5 * i as u16, 0xFF) } }
    let sound = g.mem.sound_enabled();
    if let Some(items) = g.mem.segs.get_mut(&0x9100) { if let Some(b) = items.get_mut(0x1223) { *b = sound } }
}

pub fn return_from_space(g: &mut Ground) -> Result<(), String> {
    let dat = g.files().read("T.DAT").ok_or("landing: missing T.DAT")?;
    let ds = state::ds_image(&dat).ok_or("landing: invalid T.DAT")?;
    let pointer = g.mem.pointer(state::PARTY_PTR);
    if pointer.off != 0 || pointer.seg != state::Mem::PARTY_SEG {
        return Err("landing: invalid party allocation".into());
    }
    if !g.mem.segs.contains_key(&pointer.seg) || !g.mem.segs.contains_key(&0x9100) {
        return Err("landing: missing shared party/item allocation".into());
    }
    let party = g.mem.segs.remove(&pointer.seg).ok_or("landing: missing party")?;
    let items = g.mem.segs.remove(&0x9100).ok_or("landing: missing shared item table")?;
    g.mem.ds = ds;
    g.mem.segs.clear();
    g.mem.segs.insert(pointer.seg, party);
    g.mem.segs.insert(0x9100, items);
    g.mem.set_pointer(state::PARTY_PTR, pointer);
    g.mem.set_pointer(0x6D6D, state::Far::new(0, 0x9100));
    let force = g.rand.force;
    g.rand = state::Rand { force, ..state::Rand::default() };
    g.rand.srand(g.clock.now());
    g.mem.set_u32(0x5C20, g.rand.lcg.seed);
    g.fb = mt_engine::present::fb::Fb::new();
    g.fb.regs = std::array::from_fn(|i| i as u8);
    g.mouse_event = None;
    g.speaker_refresh = 0;
    g.screen = "t";
    g.landing = true;
    g.launch_pending = false;
    g.boot_calibrating = true;
    if !render::game_init(g) { return Err("landing: unable to load T world/assets".into()); }
    g.boot_checkpoint("main_loop");
    crate::save::landed(g);
    Ok(())
}


pub fn start(g: &mut Ground) {
    if let Some(party) = g.boot_party.take() {
        if g.harness {
            if g.input.poll(g.clock.as_mut()) != mt_engine::io::Event::Boundary {
                eprintln!("ground: missing initial script boundary");
                std::process::exit(3)
            }
            g.dump_step();
        }
        g.rand.srand(g.clock.now());
        g.mem.set_u32(0x5C20, g.rand.lcg.seed);
        g.mem.segs.insert(state::Mem::PARTY_SEG, party);
        g.mem.set_pointer(state::PARTY_PTR, state::Far::new(0, state::Mem::PARTY_SEG));
        g.mem.set_u16(0x7000, if g.mouse_driver { 0xFFFF } else { 0 });
    }
}
pub fn run(g: &mut Ground) {
    start(g);
    g.boot_calibrating=true;
    if !render::game_init(g) {
        eprintln!("ground: unable to load W.DAT world/level");
        g.dump_step();
        std::process::exit(3)
    }
    g.boot_checkpoint("main_loop");
    if !g.fixture.is_empty() {
        if !g.harness { eprintln!("ground: fixture patches require --script"); std::process::exit(3) }
        for (offset, value) in std::mem::take(&mut g.fixture) { g.mem.set_u8(offset, value) }
    }
    main_loop::run(g);
}
