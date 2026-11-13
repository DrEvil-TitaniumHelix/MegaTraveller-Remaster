use crate::ground::{commands_items as items, state::Ground};

#[derive(Clone, Copy)]
pub enum Allocation { Empty, Full, Store }

pub fn alloc(g: &mut Ground, id: i16, mode: Allocation) -> i16 {
    for index in 1..=256 {
        let p = items::object(g, index);
        if g.mem.read(p, 1) != 255 { continue }
        let d = items::definition(g, id);
        let state = g.mem.read(d.add(0x27), 1);
        g.mem.write(p, id as u8 as u32, 1);
        g.mem.write(p.add(1), match mode { Allocation::Empty => 0,
            Allocation::Full => state & 15, Allocation::Store => state }, 1);
        if !matches!(mode, Allocation::Store) {
            g.mem.write(p.add(2), g.mem.read(d.add(0x28), 1), 1);
        }
        return index;
    }
    0
}
pub fn give(g: &mut Ground, who: i16, id: i16, mode: Allocation) -> i16 {
    let obj = alloc(g, id, mode);
    if obj == 0 { return -1 }
    let slot = items::add(g, who, obj);
    if slot == -1 { g.mem.write(items::object(g, obj), 255, 1) }
    if matches!(mode, Allocation::Store) && slot != -1 { 1 } else { slot }
}
pub fn give_gun_with_ammo(g: &mut Ground, who: i16, id: i16) -> i16 {
    let obj = alloc(g, id, Allocation::Full);
    if obj == 0 { return -1 }
    let ammo = match id { i16::MIN..=34 => 40, 35..=37 => 41, 38 => 42, 39 => 43,
        _ => { g.mem.write(items::object(g, obj), 255, 1); return -1 } };
    let second = alloc(g, ammo, Allocation::Full);
    if second == 0 { g.mem.write(items::object(g, obj), 255, 1); return 0 }
    let slot = items::add(g, who, obj);
    if slot == -1 || items::add(g, who, second) == -1 {
        if slot != -1 { items::remove(g, who, slot) }
        g.mem.write(items::object(g, obj), 255, 1);
        g.mem.write(items::object(g, second), 255, 1);
        return -1;
    }
    1
}
pub fn discard_slot_item(g: &mut Ground, who: i16, slot: i16) -> i16 {
    items::remove(g, who, slot);
    let obj = g.mem.party_u16(items::inv(who, slot)) as i16;
    g.mem.write(items::object(g, obj), 255, 1);
    1
}
