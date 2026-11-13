use super::Band;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile { Handgun, Rifle, Thrown, Indirect, Melee }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Recoil { Low, Med, Hi }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Weapon {
    pub id: u8, pub name: &'static str, pub rds: u16, pub pen: u8, pub atten: Option<u8>, pub dmg: u8, pub max: Band, pub auto: u8,
    pub danger_dm: u8, pub recoil: Recoil, pub rapid: bool, pub profile: Profile, pub gyro: bool, pub skill: u8, pub class: u8,
    pub block: u8, pub ignores_armour: bool,
}
const fn w(id: u8, name: &'static str, rds: u16, pen: u8, atten: Option<u8>, dmg: u8, max: Band, auto: u8, danger_dm: u8, recoil: Recoil, rapid: bool, profile: Profile, gyro: bool, skill: u8, class: u8) -> Weapon {
    Weapon { id, name, rds, pen, atten, dmg, max, auto, danger_dm, recoil, rapid, profile, gyro, skill, class, block: 0, ignores_armour: false }
}
const fn melee(id: u8, name: &'static str, pen: u8, block: u8, dmg: u8) -> Weapon {
    Weapon { id, name, rds: 0, pen, atten: None, dmg, max: Band::Close, auto: 0, danger_dm: 0, recoil: Recoil::Low, rapid: false, profile: Profile::Melee, gyro: false, skill: if id == 27 { 65 } else { 15 }, class: 0, block, ignores_armour: false }
}
use Band::*; use Profile::*; use Recoil::*;
pub const WEAPONS_PM: [Weapon; 29] = [
    w(0, "7mm revolver", 6, 1, Some(2), 3, Long, 0, 0, Med, true, Handgun, false, 28, 0x68),
    w(1, "9mm magnum revolver", 6, 3, Some(2), 3, Long, 0, 0, Med, true, Handgun, false, 28, 0x68),
    w(2, "10mm snub pistol", 6, 6, None, 4, Medium, 0, 15, Low, true, Handgun, false, 28, 0x68),
    w(3, "body pistol", 6, 0, None, 3, Medium, 0, 0, Med, true, Handgun, false, 28, 0x68),
    w(4, "9mm auto pistol", 15, 2, Some(2), 3, Long, 0, 0, Med, true, Handgun, false, 28, 0x78),
    w(5, "4mm gauss pistol", 15, 4, Some(2), 4, Long, 0, 0, Low, true, Handgun, false, 28, 0x68),
    w(6, "7mm carbine", 10, 2, Some(2), 3, VeryLong, 0, 0, Med, true, Rifle, false, 50, 0x68),
    w(7, "9mm rifle", 20, 4, Some(2), 3, VeryLong, 0, 0, Med, true, Rifle, false, 50, 0x68),
    w(8, "7mm auto rifle", 20, 3, Some(2), 3, VeryLong, 2, 0, Med, true, Rifle, false, 11, 0x78),
    w(9, "4mm gauss rifle", 40, 7, Some(4), 4, Distant, 3, 0, Low, true, Rifle, true, 11, 0x78),
    w(10, "18mm shotgun", 10, 1, Some(1), 4, Medium, 0, 15, Med, false, Rifle, false, 50, 0x68),
    w(11, "18mm auto shotgun", 20, 1, Some(1), 4, Medium, 2, 15, Med, false, Rifle, false, 11, 0x78),
    w(12, "9mm submachinegun", 30, 3, Some(1), 3, Long, 3, 0, Med, true, Handgun, false, 62, 0x78),
    w(13, "7mm assault rifle", 30, 3, Some(2), 3, VeryLong, 2, 0, Med, false, Rifle, false, 4, 0x78),
    w(14, "9mm advanced combat rifle", 20, 4, Some(3), 3, VeryLong, 2, 0, Med, false, Rifle, true, 11, 0x78),
    w(15, "light assault gun", 5, 8, Some(3), 4, VeryLong, 0, 15, Hi, false, Rifle, false, 29, 0x78),
    w(16, "disposable AT GL-7", 1, 36, None, 6, VeryLong, 0, 15, Hi, false, Indirect, false, 22, 0x49),
    w(17, "4cm RAM GL-10", 3, 9, None, 8, Distant, 0, 150, Med, false, Indirect, false, 22, 0x69),
    w(18, "TL9 laser pistol", 50, 4, Some(2), 3, VeryLong, 0, 0, Low, false, Handgun, false, 38, 0xAA),
    w(19, "TL8 laser carbine", 50, 7, Some(2), 3, Distant, 0, 0, Low, false, Rifle, false, 38, 0xAA),
    w(20, "TL9 laser rifle", 100, 9, Some(2), 3, Distant, 0, 0, Low, false, Rifle, false, 38, 0xAA),
    w(21, "TL12 laser pistol", 120, 5, Some(2), 3, VeryLong, 0, 0, Low, false, Handgun, false, 38, 0xAA),
    w(22, "TL12 laser carbine", 120, 10, Some(2), 3, Distant, 0, 0, Low, false, Rifle, false, 38, 0xAA),
    w(23, "TL12 laser rifle", 120, 18, Some(2), 3, Distant, 0, 0, Low, false, Rifle, false, 38, 0xAA),
    w(24, "neural pistol", 20, 10, Some(2), 255, Long, 0, 0, Low, false, Handgun, false, 45, 0xAB),
    w(25, "TL12 PGMP", 40, 20, Some(1), 12, Distant, 0, 45, Hi, false, Rifle, false, 18, 0xAC),
    melee(26, "dagger", 2, 1, 2),
    melee(27, "sword", 4, 3, 3),
    melee(28, "plastic knife", 2, 1, 2),
];
pub const HANDS: Weapon = melee(0xFF, "hands", 1, 1, 1);
pub const GRENADES_PM: [Weapon; 3] = [
    w(0x81, "explosive grenade", 1, 9, None, 8, Medium, 0, 30, Low, false, Thrown, false, 16, 0x49),
    w(0x82, "chemical grenade", 1, 13, None, 8, Medium, 0, 30, Low, false, Thrown, false, 16, 0x49),
    Weapon { ignores_armour: true, ..w(0x83, "radiation grenade", 1, 8, None, 8, Medium, 0, 30, Low, false, Thrown, false, 16, 0x49) },
];
pub fn weapon(id: u8) -> Option<&'static Weapon> {
    match id { 0..=28 => Some(&WEAPONS_PM[id as usize]), 0x81..=0x83 => Some(&GRENADES_PM[id as usize - 0x81]), _ => None }
}
pub fn grenade(choice: u8) -> Option<&'static Weapon> { (1..=3).contains(&choice).then(|| &GRENADES_PM[choice as usize - 1]) }
pub fn rounds_per_burst(targets: u8) -> u16 { match targets { 2 => 4, 3 => 10, 4 => 20, _ => 1 } }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Armour { pub id: u8, pub name: &'static str, pub bullets: u8, pub lasers: u8, pub melee: u8, pub exposed: bool, pub ablat: bool }
const fn a(id: u8, name: &'static str, bullets: u8, lasers: u8, melee: u8, exposed: bool) -> Armour { Armour { id, name, bullets, lasers, melee, exposed, ablat: id == 33 } }
pub const ARMOUR_PM: [Armour; 11] = [
    a(29, "jack", 0, 0, 1, true), a(30, "mesh", 2, 2, 2, true), a(31, "flak jacket", 3, 3, 3, true), a(32, "cloth", 5, 5, 5, true),
    a(33, "ablat", 1, 6, 1, true), a(34, "reflec", 0, 10, 0, true), a(35, "TL8 vacc suit", 5, 5, 5, false), a(36, "TL9 HE vacc suit", 7, 7, 7, false),
    a(37, "TL12 HE vacc suit", 8, 8, 8, false), a(38, "combat armour", 10, 10, 10, false), a(39, "battle dress", 10, 10, 10, false),
];
pub fn armour(id: u8) -> Option<&'static Armour> { (29..=39).contains(&id).then(|| &ARMOUR_PM[id as usize - 29]) }
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArmourValues { pub bullets: i32, pub lasers: i32, pub melee: i32, pub exposed: bool }
impl ArmourValues {
    pub fn uniform(av: i32, exposed: bool) -> ArmourValues { ArmourValues { bullets: av, lasers: av, melee: av, exposed } }
    pub fn of(a: &Armour) -> ArmourValues { ArmourValues { bullets: a.bullets as i32, lasers: a.lasers as i32, melee: a.melee as i32, exposed: a.exposed } }
    pub fn vs(&self, class: u8) -> i32 { (match class { 0xAA => self.lasers, 0x00 => self.melee, _ => self.bullets }).max(0) }
}
pub fn armour_named(name: &str) -> Option<&'static Armour> { ARMOUR_PM.iter().find(|a| a.name == name) }
