use crate::{ground::{commands, orders::{self, Op, Speed}, round::{self, Side}, standing, Ground}, ui::{Intent, Target, Unit, Verb}};
use mt_engine::{io::Event, play::{Decision, Kind, Movement, NativeIntent, NativePoint, Option}};

fn intent(id: impl Into<String>, label: impl Into<String>, value: Intent, enabled: bool) -> Option {
    let mut o = Option::new(id, label, vec![]); o.intent = Some(NativeIntent::new(value)); o.enabled = enabled; o
}

pub fn decision(g: &Ground) -> std::option::Option<Decision> {
    let _profile = mt_engine::play::profile::span("combat_build");
    let s = g.squad.as_ref()?;
    let e = s.p1.encounter.as_ref()?;
    if !e.choosing && e.side != Side::Party && e.interrupt.is_none() { return None }
    let mut d = Decision::new(Kind::CombatTurn, "t", "combat.turn");
    let who = round::selected(g).min(4); d.context.who = Some(who as usize);
    d.prompt = Some(s.p1.strip.clone());
    if e.choosing {
        d.options.push(Option::new("first:party", "Party first", vec![Event::Key(0x139)]));
        d.options.push(Option::new("first:npcs", "NPCs first", vec![Event::Key(0x131)]));
        return Some(d)
    }
    d.options.push(intent("end_turn", "End turn", Intent::EndTurn, true));
    if !round::conscious(g, who) || s.p1.turns[who as usize].done { return Some(d) }
    if s.fire.rapid.is_some() {
        d.options.push(Option::new("fire:rapid", "Fire selected rapid targets", vec![Event::Key(0x11C)]));
    }
    if let Some(offer) = standing::offer(g, who) { d.options.push(intent("continue", offer, Intent::Continue, true)); }
    if let Some(menu) = &s.orders.menu {
        for (i, item) in menu.items.iter().enumerate() {
            let label = item.grey.as_ref().map_or_else(|| item.label.clone(), |reason| format!("{}: {reason}", item.label));
            d.options.push(intent(format!("menu:{i}"), label, Intent::MenuPick(i as u8), item.grey.is_none()));
        }
        d.options.push(intent("back", "Close menu", Intent::CloseMenu, true));
        d.default = Some(format!("menu:{}", menu.default));
        return Some(d)
    }
    for i in round::travellers(g) {
        d.options.push(intent(format!("select:{i}"), round::name(g, i), Intent::Select(i), !s.p1.turns[i as usize].done));
        target(g, &mut d, Target::Unit(Unit::Traveller(i)), format!("traveller/{i}"));
    }
    for k in 1..=g.mem.npc_count() {
        if round::npc_present(g, k) { target(g, &mut d, Target::Unit(Unit::Npc(k)), format!("npc/{k}")); }
    }
    let from = round::traveller_cell(g, who);
    let mut objects = std::collections::BTreeSet::new();
    let range = orders::movement_range(g, who);
    let map=orders::MovementMap::capture(g,who); let reachable=map.reachable(range); let short=reachable.clone();
    d.movement = Some(Movement { origin: from, cells: reachable, max_steps: range as u16,
        bounds: (g.mem.map_blocks_high() as i16 * 12, g.mem.map_stride()),
        directions: [("northwest",(-1,-1)),("north",(-1,0)),("northeast",(-1,1)),("west",(0,-1)),
            ("east",(0,1)),("southwest",(1,-1)),("south",(1,0)),("southeast",(1,1))].into_iter().map(|(n,c)|(n.into(),c)).collect() });
    d.point = Some(NativePoint::new(move |cell, menu| {
        if !menu && short.binary_search(&cell).is_err() && map.reachable(orders::ROUTE_MAX).binary_search(&cell).is_err() { return Err("play: no legal movement route to that cell".into()) }
        Ok(NativeIntent::new(if menu { Intent::OpenMenu {target:Target::Cell(cell),at:(0,0)} }
        else { Intent::Verb {verb:Verb::Move,target:Target::Cell(cell)} }))
    }));
    for r in 0..g.mem.map_blocks_high() as i16 * 12 {
        for c in 0..g.mem.map_stride() {
            let id = g.mem.read(commands::cell(g, r, c), 1) as u8 >> 1;
            if id != 0 { objects.insert(id); }
        }
    }
    for id in objects { target(g, &mut d, Target::Object(id), format!("obj/{id}")); }
    Some(d)
}

fn target(g: &Ground, d: &mut Decision, t: Target, id: String) { add_target(d, t, id, &orders::menu_for(g, t)); }
fn add_target(d: &mut Decision, t: Target, id: String, menu: &orders::Menu) {
    d.options.push(intent(format!("menu:{id}"), format!("Actions: {id}"), Intent::OpenMenu { target: t, at: (0, 0) }, true));
    for item in &menu.items {
        let (verb, name) = match item.op {
            Op::Move(Speed::Auto) if matches!(t, Target::Cell(_)) => (Verb::Move, "move"), Op::Fire => (Verb::Fire, "fire"),
            Op::Converse => (Verb::Converse, "converse"), Op::Take => (Verb::Take, "take"),
            Op::Hold if matches!(t, Target::Unit(Unit::Traveller(i)) if d.context.who == Some(i as usize)) => (Verb::Hold, "hold"),
            _ => continue,
        };
        let id = if name == "hold" { "hold".into() } else { format!("{name}:{id}") };
        if d.options.iter().any(|o| o.id == id) { continue }
        d.options.push(intent(id, &item.label, Intent::Verb { verb, target: t }, item.grey.is_none()));
    }
}

pub fn wait(g: &mut Ground) {
    if g.input.wants_decision() && g.mem.key() == 0 && g.mem.event() == -1 {
        if let Some(d) = decision(g) { super::decide(g, d); }
    }
}
