use super::*;
use super::t::{ground, fight, place, p1};

fn fixture() -> Ground {
    let mut g = ground(); place(&mut g, 14, (24, 9), 0, 14, 41); fight(&mut g);
    g.mem.set_party_u16(256 + 0x1CE + 2 * 21, 2);
    g.mem.set_party_u8(0xA87, 14); g.mem.set_party_u8(0xA88, 3); g.mem.set_party_u8(0xA89, 0);
    for i in 0..2 { g.mem.write(Mem::walker(i).add(W_TARGET), 14, 1); }
    g
}
fn choose(g: &mut Ground, op: Op) {
    intent(g, Intent::OpenMenu { target: Target::Unit(Unit::Traveller(1)), at: (0, 0) });
    let m = g.squad.as_ref().unwrap().orders.menu.as_ref().unwrap();
    let n = m.items.iter().position(|i| i.op == op).unwrap();
    assert_eq!(m.items[n].grey, None, "{op:?}");
    intent(g, Intent::MenuPick(n as u8));
}
#[test]
fn traveller_menu_reload_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::Reload);
    assert_eq!((g.mem.party_u8(0xA83), g.mem.party_u8(0xA88), g.mem.party_u8(0xA89)), (3, 2, 20));
    assert!(!p1(&g).turns[0].attacked && p1(&g).turns[1].attacked);
}
#[test]
fn traveller_menu_swap_hand_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::SwapHand);
    assert_eq!((g.mem.read(Mem::walker(0).add(W_HAND), 1), g.mem.read(Mem::walker(1).add(W_HAND), 1)), (0, 1));
}
#[test]
fn traveller_menu_fire_mode_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::FireMode);
    assert_eq!(g.squad.as_ref().unwrap().orders.full_auto, [false, true, false, false, false]);
    assert!(!p1(&g).turns[0].attacked && p1(&g).turns[1].attacked);
}
#[test]
fn traveller_menu_hold_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::Hold);
    assert!(!p1(&g).turns[0].hold && p1(&g).turns[1].hold);
    assert_eq!(p1(&g).turns[1].speed, 1);
}
#[test]
fn traveller_menu_hold_run_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::HoldRun);
    assert!(!p1(&g).turns[0].hold && p1(&g).turns[1].hold);
    assert_eq!(p1(&g).turns[1].speed, 2);
}
#[test]
fn traveller_menu_hold_fire_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::HoldFire);
    assert_eq!((standing::target(&g, 0), standing::target(&g, 1)), (Some(14), None));
}
#[test]
fn traveller_menu_select_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::Select(1)); assert_eq!(round::selected(&g), 1);
}
#[test]
fn traveller_menu_character_targets_named_traveller() {
    let mut g = fixture(); choose(&mut g, Op::Character(1)); assert_eq!(g.mem.key(), SCAN_F1 + 1);
}
#[test]
fn traveller_menu_break_posts_party_command_when_grouped() {
    let mut g = ground(); g.mem.set_combat(0); choose(&mut g, Op::Party); assert_eq!(g.mem.key(), SCAN_P);
}
#[test]
fn traveller_menu_vehicle_posts_leave_status_when_aboard() {
    let mut g = ground(); g.mem.set_combat(0); g.mem.set_encounter_mode(1);
    choose(&mut g, Op::Vehicle); assert_eq!(g.mem.key(), SCAN_V);
}
#[test]
fn combat_hold_pick_uses_selected_traveller() {
    use mt_engine::{io::Input, play::{Play, Choice, Pick}};
    let mut g = fixture(); select(&mut g, 1);
    let mut play = Play::new(); play.register(&crate::play::turns::decision(&g).unwrap());
    play.choose(Choice::new("d:1", Pick::Id("hold".into()))).unwrap();
    intent(&mut g, *play.intent().unwrap().downcast::<Intent>().unwrap());
    assert!(!p1(&g).turns[0].hold && p1(&g).turns[1].hold);
}
#[test]
fn traveller_menu_disabled_choice_explains_its_reason_in_play_api() {
    let mut g = fixture(); g.squad.as_mut().unwrap().p1.turns[1].done = true;
    intent(&mut g, Intent::OpenMenu { target: Target::Unit(Unit::Traveller(1)), at: (0, 0) });
    let d = crate::play::turns::decision(&g).unwrap();
    let reload = d.options.iter().find(|o| o.label.starts_with("Reload")).unwrap();
    assert!(!reload.enabled && reload.label.contains("has acted"), "{reload:?}");
}
fn grey(op: Op) {
    let mut g = fixture(); let m = menu_for(&g, Target::Unit(Unit::Traveller(1)));
    let (n, item) = m.items.iter().enumerate().find(|(_, i)| i.op == op).unwrap();
    assert!(item.grey.as_ref().is_some_and(|r| !r.is_empty()));
    let party = g.mem.party().to_vec(); let turns = p1(&g).turns;
    g.squad.as_mut().unwrap().orders.menu = Some(m); pick(&mut g, n as u8);
    assert_eq!(g.mem.party(), party); assert_eq!(p1(&g).turns, turns); assert_eq!(round::selected(&g), 0);
}
#[test] fn traveller_menu_hide_is_grey_with_reason() { grey(Op::Later("P22")); }
#[test] fn traveller_menu_use_is_grey_with_reason() { grey(Op::Later("P15")); }
#[test] fn traveller_menu_give_is_grey_with_reason() { grey(Op::Later("P16")); }
#[test] fn traveller_menu_regroup_is_grey_in_fight() { grey(Op::Party); }
#[test]
fn traveller_menu_checks_named_turn_and_interrupt_before_enabling() {
    let mut g = fixture(); g.squad.as_mut().unwrap().p1.turns[1].done = true;
    let m = menu_for(&g, Target::Unit(Unit::Traveller(1)));
    for op in [Op::Reload, Op::FireMode, Op::Hold, Op::HoldRun] {
        assert_eq!(m.items.iter().find(|i| i.op == op).unwrap().grey.as_deref(), Some("has acted"), "{op:?}");
    }
    g.squad.as_mut().unwrap().p1.encounter.as_mut().unwrap().interrupt = Some((14, 0));
    let m = menu_for(&g, Target::Unit(Unit::Traveller(1)));
    for op in [Op::Reload, Op::SwapHand, Op::FireMode, Op::Hold, Op::HoldRun, Op::HoldFire, Op::Select(1)] {
        assert!(m.items.iter().find(|i| i.op == op).unwrap().grey.is_some(), "{op:?}");
    }
}
