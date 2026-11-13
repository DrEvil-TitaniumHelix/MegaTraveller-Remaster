use super::*;
use crate::ground::{state::{Far, Mem}, squad};
use mt_engine::{io::{MouseEvent, Overlay, Scripted}, sim::tick::Counter};

fn ground() -> Ground {
    Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])),
        Box::new(Counter::at(123)), Box::new(Overlay::new(None)))
}
#[test]
fn squad_image_keeps_residual_combat_state() {
    let mut s = squad::State::default();
    s.encounter = true; s.round = 8; s.pools.insert(14, 9);
    s.p1.log.push("residual combat".into()); s.p1.turns[3].attacked = true;
    s.fire.rapid = Some(vec![14,15]); s.orders.hover = "hover".into();
    let captured = anchors::Squad::capture(&Some(s.clone())).unwrap();
    assert_eq!(anchors::Squad::decode(&captured.encode()).unwrap().state(), &Some(s));
}
#[test]
fn stable_hash_and_all_covered_fields() {
    let expected = hash(&mut ground());
    assert_eq!(expected, hash(&mut ground()));
    let changes: &[fn(&mut Ground)] = &[
        |g| g.mem.ds[0] ^= 1, |g| g.mem.segs.get_mut(&Mem::PARTY_SEG).unwrap()[1] ^= 1,
        |g| { g.mem.alloc(0x8800, 16); g.mem.write(Far::new(0, 0x8800), 1, 1); },
        |g| g.mem.bios_ticks[0] ^= 1, |g| g.rand.lcg.seed ^= 1,
        |g| g.clock.wait_tick(), |g| g.clock.set(122),
        |g| g.landing = true, |g| g.sound_option ^= 1, |g| g.speaker_refresh ^= 1,
        |g| g.fb.regs[0] ^= 1, |g| g.dac[0][0] ^= 1,
        |g| g.loop_vars.initialized = true,
        |g| g.loop_vars.second.deadline ^= 1, |g| g.loop_vars.air.deadline ^= 1,
        |g| g.loop_vars.npc.deadline ^= 1, |g| g.loop_vars.mouse_state ^= 1,
        |g| g.loop_vars.now ^= 1, |g| g.loop_vars.pass_tick ^= 1, |g| g.loop_vars.last ^= 1,
        |g| g.loop_vars.changed ^= 1, |g| g.loop_vars.short ^= 1, |g| g.loop_vars.first_idle = true,
        |g| g.squad = Some(squad::State::default()), |g| g.bios_key ^= 1,
        |g| g.mouse_event = Some(MouseEvent::default()), |g| g.mouse_driver = !g.mouse_driver, |g| g.launch_pending = true,
        |g| g.game_over = true, |g| g.restore_pending = Some(Saved::capture(g, Mode::Ground, "")),
        |g| g.boot_party = Some(vec![1]), |g| g.scenario_boot = true, |g| g.boot_calibrating = true,
        |g| g.rand.force = Some(0),
    ];
    for (n, change) in changes.iter().enumerate() { let mut g = ground(); change(&mut g); assert_ne!(expected, hash(&mut g), "covered field {n}"); }
    let mut a = ground(); a.squad = Some(squad::State::default());
    let before = hash(&mut a); a.squad.as_mut().unwrap().pools.insert(14, 5); assert_ne!(before, hash(&mut a));
    let mut a = ground(); a.mouse_event = Some(MouseEvent::default());
    let before = hash(&mut a); a.mouse_event.as_mut().unwrap().col = 1; assert_ne!(before, hash(&mut a));
}
#[test]
fn balance_preferences_change_hash_without_changing_save_image() {
    let mut g = ground();
    g.loop_vars.initialized = true; g.mem.set_event(-1);
    let saved = Saved::capture(&mut g, Mode::Ground, "").encode();
    let mut hashes = std::collections::BTreeSet::new();
    for grenade_range in [false, true] { for npc_selective_autofire in [false, true] {
        g.balance = crate::ground::rules::balance::Options { grenade_range, npc_selective_autofire };
        assert!(hashes.insert(hash(&mut g)), "each preference combination must have its own hash");
        assert_eq!(saved, Saved::capture(&mut g, Mode::Ground, "").encode());
        assert_eq!(g.clock.ticks(), 123);
        let anchor = anchors::capture(&mut g, 42, 123).unwrap();
        let decoded = Saved::decode(&anchor.encode()).unwrap();
        assert_eq!(decoded.play.as_ref().unwrap().balance, g.balance);
        let mut restored = crate::save::ground_exact(decoded, Box::new(Scripted::new([])), Box::new(Counter::at(123)), Box::new(Overlay::new(None))).unwrap();
        assert_eq!(restored.balance, g.balance); assert_eq!(hash(&mut restored), hash(&mut g));
    } }
}
#[test]
fn evidence_and_presentation_do_not_change_the_hash_or_clock() {
    let mut g = ground(); let before = hash(&mut g);
    g.fb = mt_engine::present::fb::Fb::new();
    g.frames.push(("evidence".into(), vec![42])); g.rand.trace.push(99); g.rand.recording = true;
    g.screen = "a presentation label"; g.save_slot = Some(crate::save::Slot { dir: "unused".into(), build: "other build" });
    g.harness = true;
    assert_eq!(before, hash(&mut g)); assert_eq!(g.clock.ticks(), 123);
}

#[test]
fn anchor_round_trip_and_residual_state_coverage() {
    use crate::ground::{round, rules::compendium::Compendium};
    let mut g = ground(); g.loop_vars.initialized = true; g.mem.set_event(-1); g.bios_key = 0x147; g.mouse_driver = true;
    let mut s = squad::State::with_compendium(Compendium::parse(crate::ground::rules::compendium::NPCS_TOML).unwrap());
    s.p1.defs.push(round::Def::default_sight()); s.p1.done.push("finished".into()); s.p1.clock = Some(129); s.p1.pending = 18; s.p1.walked = 5; s.p1.explore_ticks = 67;
    s.orders.full_auto[2] = true; s.ablat_worn[1] = 2; s.derived.push(14);
    s.p1.npcs.insert(14, round::NpcSt { role: Some("thug".into()), alerted: Some(3), skills: [("Recon".into(), 2)].into(), ..Default::default() });
    g.squad = Some(s);
    let saved = anchors::capture(&mut g, 42, 123).unwrap();
    let bytes = saved.encode(); assert_eq!(Saved::decode(&bytes).unwrap(), saved);
    let mut restored = crate::save::ground_exact(Saved::decode(&bytes).unwrap(), Box::new(Scripted::new([])), Box::new(Counter::at(saved.ticks)), Box::new(Overlay::new(None))).unwrap();
    assert_eq!(g.squad, restored.squad); assert_eq!(hash(&mut g), hash(&mut restored));
    let sqd = saved.squad.unwrap().encode();
    for n in 0..sqd.len() { assert!(anchors::Squad::decode(&sqd[..n]).is_err(), "truncated SQD {n}"); }
    let mut trailing = sqd.clone(); trailing.push(0); assert!(anchors::Squad::decode(&trailing).is_err());
    g.squad.as_mut().unwrap().pools.insert(14, 3);
    assert!(anchors::capture(&mut g, 42, 123).is_ok());
    g.squad.as_mut().unwrap().pools.clear(); g.squad.as_mut().unwrap().p1.log.push("combat has ended".into());
    assert!(anchors::capture(&mut g, 42, 123).is_ok());
    g.squad = None; g.scenario_boot = true; assert!(anchors::capture(&mut g, 42, 123).is_ok());
}
