use super::{input::{self, Flow}, render, state::Ground, walk};
use mt_engine::sim::sched::{Job, SECOND};
pub const AIR_IDLE: u32 = 108; pub const AIR_KEY: u32 = 54; pub const SHORT: u32 = 9;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Resume { #[default] Top, OuterTurn, OuterPoll, IdleTurn, IdlePoll }
fn pad(col: i16, row: i16) -> Option<u8> { (140 < row && row < 172 && 197 < col && col < 266).then(|| (0x47 + ((row - 141) / 10) * 4 + (col - 196) / 21) as u8) }
pub fn run(g: &mut Ground) { run_session(g, false) }
pub fn resume_exact(g: &mut Ground) -> Result<(), String> {
    if !g.loop_vars.initialized { return Err("play: anchor has no initialized LOOP state".into()) }
    run_session(g, true); Ok(())
}
pub(super) struct GameOver;
fn run_session(g:&mut Ground, exact:bool) {
    if g.scene.is_none() || g.harness || g.lockstep.is_some() { run_inner(g,exact); return; }
    if let Err(exit)=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||run_inner(g,exact))) {
        if !exit.is::<GameOver>() { std::panic::resume_unwind(exit); }
    }
}
fn run_inner(g: &mut Ground, mut exact: bool) {
    let mut resume=if exact {g.play_resume} else {Resume::Top};
    'restart: loop {
    if !exact {
    g.observe_boot("main_loop", &[]);
    g.mem.set_phase(0); render::redraw(g);
    g.boot_checkpoint("main_loop_ready");
    g.boot_calibrating=false;
    g.loop_vars.last = g.now();
    if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("main_loop_ready now={} ticks={}", g.loop_vars.last, g.clock.ticks()) }
    g.loop_vars.second = Job::at(g.loop_vars.last, SECOND); g.loop_vars.air = Job::at(g.loop_vars.last, AIR_IDLE); g.loop_vars.npc = Job::at(g.loop_vars.last, SECOND);
    render::cursor_on(g);
    g.loop_vars.mouse_state = 0; g.loop_vars.now = 0;
    g.loop_vars.pass_tick=g.clock.now().wrapping_sub(1);
    g.loop_vars.initialized = true;
    }
    exact = false;
    loop {
        if resume==Resume::Top {
        crate::play::anchors::offer(g);
        crate::play::diagnostic::anchor(g);
        if g.input.closed() { break }
        if crate::save::resume(g) { continue 'restart }
        if g.tool_poll() {continue 'restart}
        }
        if matches!(resume,Resume::Top|Resume::OuterTurn) {
        crate::play::anchors::boundary(g,Resume::OuterTurn);
        crate::play::turns::wait(g);
        if g.scene.is_some() { super::seen::mark(g); let intents = super::squad::poll(g); super::round::pass(g, &intents); }
        if let Some(frozen) = super::round::clock(g) { super::round::freeze(g, frozen) }
        if g.mem.cursor_visible() == 0 { render::cursor_on(g) }
        if g.mem.redraw_pending() != 0 { g.mem.set_redraw_pending(0); render::redraw(g) }
        }
        if !matches!(resume,Resume::IdleTurn|Resume::IdlePoll) {
        if resume==Resume::OuterPoll || g.loop_vars.pass_tick==g.clock.now() {crate::play::anchors::boundary(g,Resume::OuterPoll);crate::play::commands::wait(g);g.poll();super::round::refreeze(g)}
        g.loop_vars.pass_tick=g.clock.now();
        g.loop_vars.short = g.now().wrapping_add(SHORT);
        if g.loop_vars.mouse_state != 0 { g.mem.set_key(0); if g.loop_vars.mouse_state == 1 { g.loop_vars.mouse_state = 0 } }
        g.loop_vars.changed = 0;
        g.loop_vars.first_idle=true;
        resume=Resume::Top;
        }
        g.main_wait = true;
        while g.mem.key() == 0 {
            if resume!=Resume::IdlePoll {
            if resume!=Resume::IdleTurn {
            if g.tool_poll() {continue 'restart}
            }
            resume=Resume::Top;
            crate::play::anchors::boundary(g,Resume::IdleTurn);
            crate::play::turns::wait(g);
            if g.scene.is_some() { super::seen::mark(g); let intents = super::squad::poll(g); super::round::pass(g, &intents); if g.mem.event() != -1 { break } }
            if let Some(frozen) = super::round::clock(g) { super::round::freeze(g, frozen) }
            if g.mem.cursor_visible() == 0 { render::cursor_on(g) }
            }
            if resume==Resume::IdlePoll || !g.loop_vars.first_idle && (!g.harness || g.loop_vars.pass_tick==g.clock.now()) {
                crate::play::anchors::boundary(g,Resume::IdlePoll);crate::play::commands::wait(g);g.poll();super::round::refreeze(g);
                resume=Resume::Top;
                if g.mem.key()!=0 && !g.phase_checkpoint("idle_key") {break}
            }
            resume=Resume::Top;
            if g.squad.is_some() && g.mem.key() == 0 { g.input.refuse_grant() }
            g.loop_vars.first_idle=false;g.loop_vars.pass_tick=g.clock.now();
            if g.mem.redraw_pending() != 0 { g.mem.set_redraw_pending(0); render::redraw(g) }
            if g.harness && g.mem.key() != 0 { g.phase_checkpoint("idle_key"); }
            if g.mem.orders() == 0 {
                g.loop_vars.now = g.now();
                if g.mem.encounter_mode() != 0 && g.loop_vars.last != g.loop_vars.now {walk::play_sound(g,g.mem.encounter_mode() as u16);let clock=g.now();g.set_clock(clock.wrapping_add((walk::sound_busy(g) as i16/10) as u32));g.loop_vars.last=g.loop_vars.now;}
                walk::check_party_alive(g);
            }
            let (buttons, mx, my) = (g.mem.u16(super::state::MOUSE_WORDS), g.mem.i16(super::state::MOUSE_WORDS + 4), g.mem.i16(super::state::MOUSE_WORDS + 6));
            if 140 < my && my < 172 && 197 < mx && mx < 266 && buttons & 1 != 0 { g.mem.set_key(b'o') } else { g.loop_vars.mouse_state = 0 }
            if g.mem.orders() == 0 {
                if g.mem.phase() != 0 && g.loop_vars.now > g.loop_vars.short { g.mem.set_phase(0); render::redraw(g) }
                if g.mem.countdown() != 100 && g.loop_vars.second.due(g.loop_vars.now) { render::cursor_off(g); g.loop_vars.second.arm(g.loop_vars.now, SECOND); walk::self_destruct_tick(g) }
                if g.mem.arik() != 0 && g.loop_vars.second.due(g.loop_vars.now) { walk::alarm_tick(g); g.loop_vars.second.arm(g.loop_vars.now, SECOND) }
                if walk::atmosphere_hostile(g) != 0 && g.loop_vars.air.due(g.loop_vars.now) { render::cursor_off(g); g.loop_vars.air.arm(g.loop_vars.now, AIR_IDLE); walk::air_tick(g) }
                if g.mem.message_deadline() != 0 && g.loop_vars.now > g.mem.message_deadline() { render::cursor_off(g); render::dismiss_message(g, 1) }
                if g.loop_vars.npc.due(g.loop_vars.now) {
                    if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("npc job (idle) now={} ticks={} deadline={}", g.loop_vars.now, g.clock.ticks(), g.loop_vars.npc.deadline) }
                    walk::atmosphere_tick(g); walk::npc_tick(g, 1); g.loop_vars.npc.arm(g.loop_vars.now, SECOND);
                    if g.mem.world_pending() != 0 { walk::enter_world(g, 2, 0xBD, 0xBE, 0xBF) }
                }
            }
            if g.mem.combat() != 0 && g.mem.individual() != 0 && g.loop_vars.now > g.mem.target_deadline() { g.mem.set_redraw_pending(1) }
        }
        if g.mem.key() == b'n' {
            input::special_key(g, super::state::HOTSPOTS_GROUND, super::state::HOTSPOTS_GROUND_N);
            if g.mem.key() == b'n' { g.mem.set_key(0); continue }
        }
        if g.mem.key() == b'o' {
            let (mx, my) = (g.mem.i16(super::state::MOUSE_WORDS + 4), g.mem.i16(super::state::MOUSE_WORDS + 6));
            g.mem.set_key(pad(mx, my).unwrap_or(0x47));
            if g.mem.key() == 0x4C { if g.loop_vars.mouse_state == 0 { g.mem.set_key(15) } g.loop_vars.mouse_state = 2 } else { g.loop_vars.mouse_state = 1 }
        }
        g.main_wait = false;
        let key = g.mem.key();
        if g.mem.orders() == 0 {
            walk::check_party_alive(g); g.loop_vars.now = g.now();
            if g.mem.message_deadline() != 0 && g.loop_vars.now > g.mem.message_deadline() { render::dismiss_message(g, 1) }
            if g.mem.arik() != 0 && g.loop_vars.second.due(g.loop_vars.now) { walk::alarm_tick(g); g.loop_vars.second.arm(g.loop_vars.now, SECOND) }
            if g.mem.countdown() != 100 && g.loop_vars.second.due(g.loop_vars.now) { g.loop_vars.second.arm(g.loop_vars.now, SECOND); walk::self_destruct_tick(g) }
            if walk::atmosphere_hostile(g) != 0 && g.loop_vars.air.due(g.loop_vars.now) { g.loop_vars.air.arm(g.loop_vars.now, AIR_KEY); walk::air_tick(g) }
            if g.loop_vars.npc.due(g.loop_vars.now) { if std::env::var_os("MT1_LOCKSTEP_TRACE").is_some() { eprintln!("npc job (key) now={} ticks={} deadline={}", g.loop_vars.now, g.clock.ticks(), g.loop_vars.npc.deadline) } walk::atmosphere_tick(g); g.loop_vars.changed = walk::npc_tick(g, 0) & 255; g.loop_vars.npc.arm(g.loop_vars.now, SECOND) }
        }
        if crate::save::key(g, key) { g.mem.set_key(0); continue }
        if super::round::key(g, key) { g.mem.set_key(0); g.mem.set_redraw_pending(1); continue }
        if (0x47..=0x51).contains(&key) && key != 0x4E {
            let (dx, dy, d) = g.mem.direction((key - 0x47) as u16);
            let before = (g.mem.x(), g.mem.y(), g.mem.heading());
            if walk::move_party(g, dx, dy, d) == 0 && g.loop_vars.changed != 0 { render::redraw(g) }
            if let Some(k) = g.blocked_by.take() { super::orders::wait_for(g, k) }
            if g.squad.is_some() {
                match super::round::step(before, (g.mem.x(), g.mem.y(), g.mem.heading()), g.mem.event()) { super::round::Step::Walked => { super::round::walked(g, 1); g.mem.set_redraw_pending(1) }  super::round::Step::Refused => { g.input.refuse_grant();
                    if g.scene.is_some() { let (p, _) = super::commands::facing_cell(g); if g.mem.read(p.add(1), 1) > 235 && g.mem.interior_kind() == b'I' { super::orders::log(g, "This door is locked.".into()); g.mem.set_redraw_pending(1) } } }, super::round::Step::Nothing => {} }
            }
        } else if input::dispatch(g, key, g.loop_vars.now, g.loop_vars.changed) == Flow::Quit {
            if g.scene.is_some() && !g.harness && g.lockstep.is_none() { g.game_over=true; }
            break;
        }
        if g.mem.event() != -1 && super::round::locked(g) { g.mem.set_event(-1); render::message(g, b"No way out but through."); }
        if g.mem.event() != -1 {
            render::cursor_off(g); g.mem.set_phase(0); walk::resolve_event(g);
            if g.launch_pending {
                match crate::space::launch(g) {
                    Ok(crate::space::Exit::Land) => continue 'restart,
                    Ok(crate::space::Exit::GameOver) => { g.game_over = true; return }
                    Err(error) => { eprintln!("{error}"); return; }
                }
            }
            render::redraw(g)
        }
    }
    render::cursor_off(g);
    break;
    }
}
#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn pad_cells() {
        assert_eq!(pad(198, 141), Some(0x47)); assert_eq!(pad(230, 155), Some(0x4C)); assert_eq!(pad(230, 150), Some(0x48)); assert_eq!(pad(265, 171), Some(0x56));
        assert_eq!(pad(197, 150), None); assert_eq!(pad(230, 172), None); assert_eq!(pad(259, 141), Some(0x4A));
    }
}
