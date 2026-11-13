use super::{compendium::{Compendium, Entry, GUN_COMBAT}, weapons::{self, ArmourValues, Weapon}};
use crate::ground::{state::{Far, Ground, Mem, PARTY_PTR, W_BUILTIN, W_HAND}, walk};
use std::collections::BTreeMap;

pub const SKILL_TACTICS: u8 = 66; pub const SKILL_LEADER: u8 = 39; pub const SKILL_RECON: u8 = 48; pub const SKILL_JOT: u8 = 36;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Skill { pub level: i32, pub unskilled: bool }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source { Traveller, Entry(String), Derived, Invulnerable }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Skills { Record(u8), Entry(BTreeMap<String, i32>, Option<(i32, i32)>), Derived(i32) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fighter { pub label: String, pub strength: i32, pub dex: i32, pub armour: ArmourValues, pub tactics: i32, pub leader: i32, pub recon: i32, pub pool: i32, pub source: Source, pub skills: Skills }
impl Fighter {
    pub fn skill(&self, g: &Ground, comp: &Compendium, weapon: &Weapon) -> Skill {
        if weapon.profile == weapons::Profile::Thrown { return Skill { level: 0, unskilled: false } }
        match &self.skills {
            Skills::Record(who) => { let level = rec_u8(g, *who, 0x12B + weapon.skill as u16) as i32; Skill { level, unskilled: level == 0 } }
            Skills::Derived(level) => Skill { level: *level, unskilled: false },
            Skills::Entry(skills, natural) => {
                if let (Some((level, _)), weapons::Profile::Melee) = (natural, weapon.profile) { return Skill { level: *level, unskilled: false } }
                match comp.skill_name(weapon.skill).and_then(|n| skills.get(n).map(|l| (n, *l))) {
                    Some((_, level)) => Skill { level, unskilled: false },
                    None => { let gun = comp.skill_name(weapon.skill).is_some_and(|n| GUN_COMBAT.contains(&n)); Skill { level: 0, unskilled: !gun } }
                }
            }
        }
    }
}
fn rec(g: &Ground, who: u8) -> Far { g.mem.pointer(PARTY_PTR).add(((who as i32) & 7) << 8) }
fn rec_u8(g: &Ground, who: u8, off: u16) -> u8 { g.mem.read(rec(g, who).add(off as i32), 1) as u8 }
pub fn item_id(g: &Ground, def: Far) -> Option<u8> {
    if def == Far::default() { return None }
    let table = g.mem.pointer(0x6D6D);
    (def.seg == table.seg).then(|| (def.off.wrapping_sub(table.off) / 46) as u8)
}
pub fn traveller(g: &Ground, who: u8, ablat_worn: i32) -> Fighter {
    let r = rec(g, who);
    let label: String = (0..32).map(|i| g.mem.read(r.add(0x102 + i), 1) as u8).take_while(|&b| b != 0).map(|b| b as char).collect();
    let upp: Vec<i32> = (0..6).map(|i| g.mem.read(r.add(0x116 + i), 1) as i32).collect();
    let (suit, accessory) = (walk::item_def(g, who as i32, 20), walk::item_def(g, who as i32, 19));
    let mut armour = item_id(g, suit).and_then(weapons::armour).map(ArmourValues::of).unwrap_or(ArmourValues::uniform(0, true));
    if suit != Far::default() && g.mem.read(suit.add(0x29), 1) == 1 && (accessory == Far::default() || g.mem.read(accessory.add(0x29), 1) == 0) {
        armour = ArmourValues { bullets: armour.bullets - 1, lasers: armour.lasers - 1, melee: armour.melee - 1, ..armour };
    }
    if item_id(g, suit).and_then(weapons::armour).is_some_and(|a| a.ablat) { armour.lasers -= ablat_worn }
    let skill = |i: u8| rec_u8(g, who, 0x12B + i as u16) as i32;
    Fighter { label, strength: upp[0], dex: upp[1], armour, tactics: skill(SKILL_TACTICS), leader: skill(SKILL_LEADER), recon: skill(SKILL_RECON),
        pool: rec_u8(g, who, 0x127) as i32, source: Source::Traveller, skills: Skills::Record(who) }
}
pub fn traveller_weapon(g: &Ground, who: u8) -> Option<&'static Weapon> {
    let wk = Mem::walker(who as i16);
    let builtin = g.mem.read(wk.add(W_BUILTIN), 1) as u8;
    if builtin != 0 { return weapons::grenade(builtin) }
    let def = walk::item_def(g, who as i32, g.mem.read(wk.add(W_HAND), 1) as i32 + 21);
    if def == Far::default() || g.mem.read_i(def.add(0x1D), 2) != 2 { return None }
    item_id(g, def).and_then(weapons::weapon)
}
pub fn here(g: &Ground) -> (u8, u8) { (g.mem.party_u8(6), g.mem.party_u8(7)) }
pub fn npc(g: &Ground, k: u8, comp: &Compendium) -> Fighter {
    let n = Mem::npc(k as i16);
    let (hp, level, picture) = (g.mem.read(n.add(10), 1) as i32, g.mem.read(n.add(11), 1) as i32, g.mem.read(n.add(9), 1) as u8);
    let (world, area) = here(g);
    let derived = |source: Source| Fighter { label: format!("NPC {k}"), strength: 7, dex: 7, armour: ArmourValues::uniform(level, true), tactics: 0, leader: 0, recon: 0,
        pool: hp, source, skills: Skills::Derived(hp >> 3) };
    if hp == 0xFF { return derived(Source::Invulnerable) }
    match comp.entry_of(world, area, k, picture) {
        None => derived(Source::Derived),
        Some(e) => fighter_of(e, level),
    }
}
pub fn fighter_of(e: &Entry, level: i32) -> Fighter {
    let armour = if e.from_record { None } else { e.armour }.unwrap_or(ArmourValues::uniform(level, true));
    Fighter { label: e.label.clone(), strength: e.upp[0], dex: e.upp[1], armour, tactics: e.tactics, leader: e.leader, recon: e.recon, pool: e.hits.0 + e.hits.1,
        source: Source::Entry(e.id.clone()), skills: Skills::Entry(e.skills.clone(), e.animal_attack) }
}
pub fn npc_weapon(g: &Ground, k: u8) -> Option<&'static Weapon> {
    match g.mem.read(Mem::npc(k as i16).add(31), 1) as u8 { 0 => None, id => weapons::weapon(id) }
}
pub fn npc_weapon_values(f: &Fighter, w: &Weapon) -> (i32, i32) {
    match (&f.skills, w.profile) { (Skills::Entry(_, Some((_, hits))), weapons::Profile::Melee) => (*hits, *hits), _ => (w.pen as i32, w.dmg as i32) }
}

#[cfg(test)]
mod t {
    use super::*;
    use crate::ground::rules::{weapons::Profile, Band};
    use mt_engine::{io::{Overlay, Scripted}, sim::tick::Counter};
    const ITEMS: u16 = 0x9100;
    fn ground() -> Ground {
        let mut g = Ground::from_ds(vec![0; 0x10000], vec![0; Mem::PARTY_SIZE], Box::new(Scripted::new([])), Box::new(Counter::default()), Box::new(Overlay::new(None)));
        g.mem.alloc(ITEMS, 46 * 140); g.mem.set_pointer(0x6D6D, Far::new(0, ITEMS));
        for id in 0..=28u16 { g.mem.write(Far::new(46 * id + 0x1D, ITEMS), 2, 2); }
        for id in 35..=39u16 { g.mem.write(Far::new(46 * id + 0x1D, ITEMS), 3, 2); g.mem.write(Far::new(46 * id + 0x29, ITEMS), 1, 1); }
        for id in [41u16, 42, 43] { g.mem.write(Far::new(46 * id + 0x29, ITEMS), 1, 1); }
        g.mem.set_party_u8(6, 2); g.mem.set_party_u8(7, 0);
        g
    }
    fn give(g: &mut Ground, who: u8, slot: u16, n: u16, id: u8) {
        g.mem.set_party_u16(((who as u16) << 8) + 0x1CE + 2 * slot, n);
        g.mem.set_party_u8(0xA82 + 5 * (n - 1), id);
    }
    #[test]
    fn traveller_from_the_record() {
        let mut g = ground();
        let base = 1u16 << 8;
        for (i, b) in b"Vlar\0".iter().enumerate() { g.mem.set_party_u8(base + 0x102 + i as u16, *b) }
        for (i, v) in [9u8, 11, 8, 7, 7, 7].iter().enumerate() { g.mem.set_party_u8(base + 0x116 + i as u16, *v) }
        g.mem.set_party_u8(base + 0x12B + 28, 2); g.mem.set_party_u8(base + 0x12B + SKILL_TACTICS as u16, 1); g.mem.set_party_u8(base + 0x12B + SKILL_LEADER as u16, 3); g.mem.set_party_u8(base + 0x127, 17);
        give(&mut g, 1, 21, 1, 4);
        let f = traveller(&g, 1, 0);
        assert_eq!((f.label.as_str(), f.strength, f.dex, f.armour, f.tactics, f.leader, f.recon, f.pool, &f.source), ("Vlar", 9, 11, ArmourValues::uniform(0, true), 1, 3, 0, 17, &Source::Traveller));
        let comp = Compendium::default();
        let w = traveller_weapon(&g, 1).unwrap(); assert_eq!(w.id, 4);
        assert_eq!(f.skill(&g, &comp, w), Skill { level: 2, unskilled: false });
        assert_eq!(f.skill(&g, &comp, weapons::weapon(7).unwrap()), Skill { level: 0, unskilled: true }, "no Rifleman level in the record: R-4 unskilled");
        assert_eq!(f.skill(&g, &comp, weapons::grenade(1).unwrap()), Skill { level: 0, unskilled: false }, "a grenade throw has no skill");
        give(&mut g, 1, 20, 2, 32); assert_eq!(traveller(&g, 1, 0).armour, ArmourValues { bullets: 5, lasers: 5, melee: 5, exposed: true });
        give(&mut g, 1, 20, 2, 36); assert_eq!(traveller(&g, 1, 0).armour, ArmourValues { bullets: 6, lasers: 6, melee: 6, exposed: false }, "TL9 HE vacc suit 7 - 1 without a helmet");
        give(&mut g, 1, 19, 3, 42); assert_eq!(traveller(&g, 1, 0).armour.bullets, 7, "helmet in slot 19");
        give(&mut g, 1, 20, 2, 33); assert_eq!(traveller(&g, 1, 0).armour.lasers, 6); assert_eq!(traveller(&g, 1, 2).armour.lasers, 4, "ablat: -1 laser AV per laser hit, squad state");
        g.mem.write(Mem::walker(1).add(W_HAND), 1, 1); assert_eq!(traveller_weapon(&g, 1), None);
        give(&mut g, 1, 22, 4, 27); assert_eq!(traveller_weapon(&g, 1).map(|w| w.id), Some(27));
        g.mem.write(Far::new(46 * 27 + 0x1D, ITEMS), 3, 2); assert_eq!(traveller_weapon(&g, 1), None, "not a weapon definition");
        g.mem.write(Mem::walker(1).add(W_BUILTIN), 2, 1); assert_eq!(traveller_weapon(&g, 1).map(|w| w.id), Some(0x82));
    }
    #[test]
    fn npc_from_the_entry_else_derived() {
        let comp = Compendium::parse(crate::ground::rules::compendium::NPCS_TOML).unwrap();
        let mut g = ground();
        let n = Mem::npc(14);
        g.mem.write(n.add(9), 41, 1); g.mem.write(n.add(10), 12, 1); g.mem.write(n.add(11), 3, 1); g.mem.write(n.add(31), 14, 1);
        let f = npc(&g, 14, &comp);
        let thug = &comp.roles["thug"];
        assert_eq!((&f.source, f.label.as_str(), f.pool, f.dex, f.tactics), (&Source::Entry("thug".into()), "thug", thug.hits.0 + thug.hits.1, thug.upp[1], thug.tactics));
        let w = npc_weapon(&g, 14).unwrap(); assert_eq!(w.id, 14);
        let s = f.skill(&g, &comp, w); assert_eq!(s.unskilled, false, "a gun the role has no level in is Gun Combat-0, not unskilled");
        assert_eq!(f.skill(&g, &comp, weapons::weapon(26).unwrap()).unskilled, !thug.skills.contains_key("Small Blade"));
        assert_eq!(npc_weapon_values(&f, w), (4, 3));
        let n = Mem::npc(31); g.mem.write(n.add(9), 0, 1); g.mem.write(n.add(10), 30, 1); g.mem.write(n.add(11), 9, 1); g.mem.write(n.add(31), 25, 1);
        let f = npc(&g, 31, &comp); assert_eq!((&f.source, f.armour.bullets, f.leader), (&Source::Entry("zhodani_officer".into()), 10, 1));
        g.mem.set_party_u8(7, 3); let n = Mem::npc(3); g.mem.write(n.add(9), 25, 1); g.mem.write(n.add(10), 40, 1); g.mem.write(n.add(11), 6, 1);
        let f = npc(&g, 3, &comp); assert_eq!((&f.source, f.armour), (&Source::Entry("assassin_rhevah_hamache".into()), ArmourValues::uniform(6, true)));
        g.mem.set_party_u8(7, 0); let n = Mem::npc(5); g.mem.write(n.add(9), 30, 1); g.mem.write(n.add(10), 60, 1); g.mem.write(n.add(11), 2, 1); g.mem.write(n.add(31), 27, 1);
        let f = npc(&g, 5, &comp); let w = npc_weapon(&g, 5).unwrap();
        assert_eq!((f.pool, f.skill(&g, &comp, w), npc_weapon_values(&f, w), f.armour), (7, Skill { level: 1, unskilled: false }, (2, 2), ArmourValues::uniform(0, true)));
        let n = Mem::npc(6); g.mem.write(n.add(9), 200, 1); g.mem.write(n.add(10), 19, 1); g.mem.write(n.add(11), 4, 1); g.mem.write(n.add(31), 7, 1);
        let f = npc(&g, 6, &comp);
        assert_eq!((&f.source, f.pool, f.armour, f.dex, f.tactics, f.skill(&g, &comp, weapons::weapon(7).unwrap())), (&Source::Derived, 19, ArmourValues::uniform(4, true), 7, 0, Skill { level: 2, unskilled: false }));
        g.mem.write(n.add(10), 0xFF, 1); let f = npc(&g, 6, &comp); assert_eq!((&f.source, f.skill(&g, &comp, weapons::weapon(7).unwrap()).level), (&Source::Invulnerable, 31));
        assert_eq!((npc_weapon(&g, 7), Band::Short.name(), Profile::Melee == weapons::weapon(26).unwrap().profile), (None, "short", true));
    }
}
