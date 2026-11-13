use crate::ground::{round, Ground};
use mt_engine::{io::Event, play::{Decision, Kind, Option, TimeInput}};

pub fn ground(g: &Ground) -> Decision {
    let mut d = Decision::new(Kind::Ground, "t", "6923");
    let mut key = |id: &str, label: &str, scan: u16, enabled: bool| {
        let mut o = Option::new(format!("cmd:{id}"), label, vec![Event::Key(0x100 + scan)]);
        o.enabled = enabled; d.options.push(o);
    };
    let orders = g.mem.orders() != 0;
    key("back", "Back / cancel", 1, true);
    key("game", "Game", 0x22, true);
    key("talk", "Converse", 0x2E, g.mem.combat() == 0);
    key("party", "Party", 0x19, !orders);
    key("inventory", "Items", 0x17, !orders);
    key("vehicle", "Vehicle", 0x2F, !orders);
    key("orders", "Orders", 0x18, g.squad.is_none());
    key("reload", "Reload", 0x13, true);
    if g.scene.is_some() { key("light", "Light", 0x26, true); }
    for i in 0..5 { key(&format!("char/{i}"), &format!("Traveller {}", i + 1), 0x3B + i, true); }
    if g.mem.combat() != 0 {
        for (id, label, scan) in [("fire", "Fire", 0x21), ("target", "Target", 0x14), ("attack", "Attack", 0x1C),
            ("next_target", "Next target", 0x0F), ("fire_mode", "Fire mode", 0x0D)] { key(id, label, scan, true); }
        for i in 0..5 { key(&format!("select/{i}"), &format!("Select Traveller {}", i + 1), 2 + i, round::conscious(g, i as u8)); }
    }
    d.time = Some(TimeInput { max_ticks: 65535, directions: [
        ("northwest", 0x147), ("north", 0x148), ("northeast", 0x149), ("west", 0x14B),
        ("east", 0x14D), ("southwest", 0x14F), ("south", 0x150), ("southeast", 0x151),
    ].into_iter().map(|(name, key)| (name.into(), key)).collect() });
    d
}

pub fn wait(g: &mut Ground) {
    if g.input.wants_decision() && g.mem.key() == 0 && !round::in_encounter(g) {
        super::decide(g, ground(g));
    }
}
