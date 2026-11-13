use super::anchors::Squad;
use crate::{ground::{squad,round,orders,fire,tactics,standing,npc_turn,rules},ui::{Unit,Target}};

#[test]
fn every_squad_field_survives_the_image() {
    let unit=Unit::Traveller(2);let cell=(3,4);
    let encounter=round::Encounter {def:"fixture".into(),round:7,side:round::Side::Npcs,first:round::Side::Party,choosing:true,
        surprised:Some(round::Side::Npcs),quiet:2,grouped:true,group_moved:true,pools:(3,5),npc_start:vec![14,15],withdrawing:true,morale_round:6,interrupt:Some((14,2))};
    let mut s=squad::State {
        encounter:true,round:7,pools:[(14,17)].into(),ablat_worn:[1,2,3,4,5],
        compendium:rules::compendium::Compendium::parse(rules::compendium::NPCS_TOML).unwrap(),derived:vec![16],
        p1:round::State {
            defs:vec![round::Def {id:"fixture".into(),start:round::Start::Area(round::Rect{r0:1,c0:2,r1:3,c1:4}),
                end:round::End::Locked{escape:Some(round::Rect{r0:5,c0:6,r1:7,c1:8})},boundary:round::Boundary::Screen,participants:Some(vec![14,15]),
                auto_break:true,allow_save:true,surprise:round::Surprise::Defenders,attacker:round::Side::Npcs,alerted_rounds:9}],
            done:vec!["old".into()],encounter:Some(encounter),
            turns:[round::Turn{moved:true,attacked:true,done:true,hold:true,speed:2,evade:true,mp:7};5],
            npcs:[(14,round::NpcSt{role:Some("thug".into()),skills:[("Recon".into(),3)].into(),engaged:true,alerted:Some(9),alert_full:10,
                last_seen:Some(cell),speed:2,acted:true,clip:8,reload:true,routed:true})].into(),
            strip:"strip".into(),log:vec!["log".into()],acting:Some(unit),
            fx:round::Fx{seq:12,unit:Some(unit),walk:vec![cell],shots:vec![(cell,(5,6),7)],hits:vec![(Unit::Npc(14),cell,9)],blasts:vec![(8,9)]},
            logged:13,clock:Some(1234),pending:21,explore_ticks:22,walked:23,
        },
        orders:orders::State{full_auto:[true;5],menu:Some(orders::Menu{target:Target::Cell(cell),items:vec![orders::MenuItem{key:42,label:"menu".into(),op:orders::Op::Character(2),grey:Some("reason".into())}],default:1}),
            hover:"hover".into(),path:vec![(cell,3)],target:Some(Target::Object(4)),hovered:Some(Target::Portrait(2))},
        fire:fire::State{rapid:Some(vec![14,15]),reloading:[true;5],disarmed:vec![16],pending:vec![fire::Pending{who:unit,cell,weapon:3,
            task:rules::Task{difficulty:rules::Difficulty::Formidable,target:15,dm:-3},round:8,launcher:true}]},
        p22:tactics::State{declared:3,spent:4,round:7,paused:Some((14,npc_turn::Plan::Flee{left:6,attack:true}))},
        p6:standing::State{advance:[true;5]},
    };
    for active in [true,false] {
        s.encounter=active;
        let before=Squad::capture(&Some(s.clone())).unwrap();
        assert_eq!(Squad::decode(&before.encode()).unwrap().state(),&Some(s.clone()));
    }
    use super::anchors::codec::Value;
    fn roundtrip<T:Value+PartialEq+std::fmt::Debug>(value:T) {let mut bytes=vec![];value.put(&mut bytes);assert_eq!(T::get(&mut bytes.as_slice()).unwrap(),value);}
    for boundary in [round::Boundary::Radius(29),round::Boundary::Area(round::Rect{r0:1,c0:2,r1:3,c1:4}),round::Boundary::Level,round::Boundary::Screen] {roundtrip(boundary)}
    for plan in [npc_turn::Plan::Approach{left:9},npc_turn::Plan::Attack{checked:true}] {roundtrip(plan)}
    use orders::{Op::*,Speed};
    for op in [Move(Speed::Auto),Move(Speed::Walk),Move(Speed::Run),Move(Speed::Evade),HoldHere,Fire,SetTarget,Converse,Take,Select(4),Reload,
        SwapHand,FireMode,Hold,Character(3),Party,Vehicle,Later("P15"),Later("P16"),Later("P22"),FullAuto,Rapid,Pinpoint,Melee,Disarm,Throw,Launch,Advance,HoldFire,HoldRun,Tactics] {roundtrip(op)}
}
