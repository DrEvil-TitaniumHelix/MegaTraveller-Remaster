pub mod weapons;
pub mod compendium;
pub mod adapters;
pub mod balance;
pub use weapons::{Weapon, Profile, Recoil, Armour, ArmourValues, WEAPONS_PM, GRENADES_PM, ARMOUR_PM, weapon, grenade, armour, armour_named, rounds_per_burst};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Band { Close, Short, Medium, Long, VeryLong, Distant }
pub fn squares(a: (i32, i32), b: (i32, i32)) -> i32 { (a.0 - b.0).abs().max((a.1 - b.1).abs()) }
pub fn band(squares: i32) -> Band {
    match squares { i32::MIN..=0 => Band::Close, 1..=2 => Band::Short, 3..=29 => Band::Medium, 30..=149 => Band::Long, 150..=299 => Band::VeryLong, _ => Band::Distant }
}
impl Band {
    pub fn name(self) -> &'static str { ["close", "short", "medium", "long", "very long", "distant"][self as usize] }
    pub fn movement_pct(self) -> i32 { [100, 100, 50, 25, 10, 5][self as usize] }
    fn from_short(self) -> u32 { (self as u32).saturating_sub(1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Difficulty { Simple, Routine, Difficult, Formidable, Impossible }
impl Difficulty {
    pub fn target(self) -> i32 { [3, 7, 11, 15, 19][self as usize] }
    pub fn name(self) -> &'static str { ["Simple", "Routine", "Difficult", "Formidable", "Impossible"][self as usize] }
    pub fn harder(self, n: i32) -> Difficulty {
        [Difficulty::Simple, Difficulty::Routine, Difficulty::Difficult, Difficulty::Formidable, Difficulty::Impossible][(self as i32 + n).clamp(0, 4) as usize]
    }
}
pub fn difficulty(profile: Profile, band: Band, braced: bool) -> Option<Difficulty> {
    use Difficulty::*;
    let gyro = [None, None, Some(Routine), Some(Difficult), Some(Formidable), Some(Impossible)];
    let row: [Option<Difficulty>; 6] = match profile {
        Profile::Handgun => [Some(Simple), Some(Routine), Some(Difficult), Some(Formidable), Some(Impossible), None],
        Profile::Rifle => [Some(Routine), Some(Routine), Some(Difficult), Some(Difficult), Some(Formidable), Some(Impossible)],
        Profile::Thrown => [Some(Routine), Some(Difficult), Some(Formidable), Some(Impossible), None, None],
        Profile::Indirect | Profile::Melee => return None,
    };
    match (braced, gyro[band as usize]) { (true, Some(d)) => Some(d), _ => row[band as usize] }
}

pub fn char_dm(c: i32) -> i32 { c / 5 }
pub const DM_CAP: i32 = 8;
pub fn movement_dm(target_speed: i32, target_evading: bool, attacker_speed: i32, attacker_evading: bool, band: Band) -> i32 {
    let d = target_speed * if target_evading { 2 } else { 1 } * band.movement_pct() / 100;
    let a = attacker_speed * if attacker_evading { 2 } else { 1 };
    -d - a
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Fire {
    pub profile: Profile, pub band: Band, pub braced: bool, pub cover: bool, pub size: i32, pub rapid: bool, pub dark: bool,
    pub pinpoint: bool, pub unskilled: bool, pub skill: i32, pub dex: i32, pub movement: i32, pub tactics: i32,
}
impl Default for Fire {
    fn default() -> Fire { Fire { profile: Profile::Handgun, band: Band::Short, braced: false, cover: false, size: 0, rapid: false, dark: false, pinpoint: false, unskilled: false, skill: 0, dex: 7, movement: 0, tactics: 0 } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Task { pub difficulty: Difficulty, pub target: i32, pub dm: i32 }
impl Task {
    pub fn needs(&self) -> i32 { (self.target - self.dm).max(3) }
    pub fn chance_pct(&self) -> i32 {
        let hits = (1..=6).flat_map(|a| (1..=6).map(move |b| a + b)).filter(|&r| r != 2 && r + self.dm >= self.target).count() as i32;
        hits * 100 / 36
    }
}
pub fn hit_task(f: &Fire) -> Option<Task> {
    let base = difficulty(f.profile, f.band, f.braced)?;
    let steps = f.cover as i32 + f.size + f.rapid as i32 + f.dark as i32 + f.unskilled as i32 + f.pinpoint as i32;
    let d = base.harder(steps);
    let dm = (f.skill + char_dm(f.dex) + f.movement + f.tactics).clamp(-DM_CAP, DM_CAP);
    Some(Task { difficulty: d, target: d.target(), dm })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level { Marginal, Normal, X2, X4, X8 }
impl Level {
    pub fn of(margin: i32) -> Level { match margin { i32::MIN..=0 => Level::Marginal, 1 => Level::Normal, 2..=3 => Level::X2, 4..=7 => Level::X4, _ => Level::X8 } }
    pub fn name(self) -> &'static str { ["graze", "hit", "x2", "x4", "x8"][self as usize] }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome { pub roll: i32, pub total: i32, pub hit: bool, pub margin: i32, pub level: Option<Level>, pub fumble: bool, pub exceptional_failure: bool }
pub fn resolve(task: &Task, dice: (i32, i32)) -> Outcome {
    let roll = dice.0 + dice.1;
    let total = roll + task.dm;
    let fumble = roll == 2;
    let hit = !fumble && total >= task.target;
    let margin = total - task.target;
    Outcome { roll, total, hit, margin, level: hit.then(|| Level::of(margin)), fumble, exceptional_failure: !hit && total <= task.target - 2 }
}

pub fn pen_at(pen: i32, atten: Option<u8>, band: Band) -> i32 {
    match atten { Some(a) if a > 0 => pen >> (band.from_short() / a as u32), _ => pen }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class { High, Low, Zero }
pub fn class(pen: i32, av: i32) -> Class {
    if pen >= 2 * av && (av > 0 || pen >= 1) { Class::High } else if pen >= av { Class::Low } else { Class::Zero }
}
pub fn damage(pen: i32, av: i32, exposed: bool, level: Level, dmg: i32) -> i32 {
    let i = level as usize;
    match class(pen, av) {
        Class::High => dmg * [50, 100, 200, 400, 800][i] / 100,
        Class::Low => dmg * [25, 50, 100, 200, 400][i] / 100,
        Class::Zero => if exposed || pen * 10 >= av { [0, 0, 1, 2, 4][i] } else { 0 },
    }
}
pub fn life_force(strength: i32, dex: i32, end: i32) -> (i32, i32) {
    const T: [(i32, i32, i32); 15] = [(45, 6, 10), (42, 6, 9), (39, 5, 9), (36, 5, 8), (33, 5, 7), (30, 4, 7), (27, 4, 6), (24, 4, 5), (21, 3, 5), (18, 3, 4), (15, 3, 3), (12, 2, 3), (9, 2, 2), (6, 2, 1), (3, 1, 1)];
    let lf = strength + dex + end;
    T.iter().find(|r| lf >= r.0).map_or((0, 0), |r| (r.1, r.2))
}
pub fn danger_radius(danger_dm: u8) -> i32 { ((2 * danger_dm as i32 + 15) / 30 - 1).max(0) }
pub fn danger_pen(pen: i32, distance: i32) -> i32 { if distance <= 0 { pen } else { pen >> distance.min(30) } }

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn bands_and_profiles() {
        assert_eq!(squares((44, 10), (48, 14)), 4); assert_eq!(squares((0, 0), (1, -2)), 2);
        assert_eq!((band(0), band(1), band(2), band(3), band(29), band(30), band(149), band(150), band(299), band(300)),
            (Band::Close, Band::Short, Band::Short, Band::Medium, Band::Medium, Band::Long, Band::Long, Band::VeryLong, Band::VeryLong, Band::Distant));
        use Difficulty::*; use Profile::*; use Band::*;
        let t = |p, b, br| difficulty(p, b, br).map(|d| d.target());
        assert_eq!([t(Handgun, Close, false), t(Handgun, Short, false), t(Handgun, Medium, false), t(Handgun, Long, false), t(Handgun, VeryLong, false), t(Handgun, Distant, false)], [Some(3), Some(7), Some(11), Some(15), Some(19), None]);
        assert_eq!([t(Rifle, Close, false), t(Rifle, Short, false), t(Rifle, Medium, false), t(Rifle, Long, false), t(Rifle, VeryLong, false), t(Rifle, Distant, false)], [Some(7), Some(7), Some(11), Some(11), Some(15), Some(19)]);
        assert_eq!([t(Thrown, Close, false), t(Thrown, Short, false), t(Thrown, Medium, false), t(Thrown, Long, false), t(Thrown, VeryLong, false)], [Some(7), Some(11), Some(15), Some(19), None]);
        assert_eq!((t(Rifle, Medium, true), t(Handgun, Short, true), t(Rifle, Distant, true)), (Some(7), Some(7), Some(19)), "braced / gyro: Routine at medium, the base row where the Gyrostable row is blank");
        assert_eq!((Simple.harder(1), Impossible.harder(1), Difficult.harder(-1), Routine.harder(3)), (Routine, Impossible, Routine, Impossible));
        assert_eq!(difficulty(Melee, Close, false), None);
    }
    #[test]
    fn the_roll_with_dms_fumble_clamp_and_the_movement_example() {
        assert_eq!((char_dm(6), char_dm(7), char_dm(9), char_dm(10), char_dm(14), char_dm(15)), (1, 1, 1, 2, 2, 3));
        assert_eq!(movement_dm(3, false, 4, false, Band::Short), -7, "PM p.68: attacker speed 4 vs target speed 3 at short = -7");
        assert_eq!(movement_dm(2, true, 0, false, Band::Short), -4, "a running evader: +4 defensive");
        assert_eq!(movement_dm(2, true, 0, false, Band::Medium), -2); assert_eq!(movement_dm(1, false, 0, false, Band::Medium), 0, "50% of 1 drops to 0");
        assert_eq!(movement_dm(2, false, 2, true, Band::Long), -4, "25% of 2 = 0; the evading attacker's own term doubles");
        let f = Fire { skill: 2, dex: 10, movement: -1, tactics: 1, ..Fire::default() };
        let task = hit_task(&f).unwrap();
        assert_eq!((task.difficulty, task.target, task.dm, task.needs()), (Difficulty::Routine, 7, 4, 3));
        assert_eq!(hit_task(&Fire { skill: 9, dex: 15, tactics: 3, ..f }).unwrap().dm, 8, "clamped at +8");
        assert_eq!(hit_task(&Fire { skill: 0, dex: 3, movement: -12, ..f }).unwrap().dm, -8);
        assert_eq!(hit_task(&Fire { cover: true, unskilled: true, ..f }).unwrap().target, 15, "+1 each: cover, unskilled (R-4)"); assert_eq!(hit_task(&Fire { cover: true, unskilled: true, rapid: true, ..f }).unwrap().target, 19, "+ rapid fire = Impossible");
        assert_eq!(hit_task(&Fire { band: Band::Distant, ..f }), None, "a handgun cannot fire at distant range");
        let o = resolve(&task, (1, 1)); assert!(o.fumble && !o.hit && o.level.is_none(), "natural 2 misses regardless of DMs");
        let o = resolve(&task, (1, 2)); assert_eq!((o.roll, o.total, o.hit, o.margin, o.level), (3, 7, true, 0, Some(Level::Marginal)));
        let o = resolve(&task, (6, 6)); assert_eq!((o.total, o.margin, o.level), (16, 9, Some(Level::X8)));
        let t = Task { difficulty: Difficulty::Difficult, target: 11, dm: 0 };
        assert_eq!(resolve(&t, (4, 5)).exceptional_failure, true, "9 vs 11: short by 2"); assert_eq!(resolve(&t, (5, 5)).exceptional_failure, false); assert_eq!(resolve(&t, (5, 5)).hit, false);
        assert_eq!(Task { difficulty: Difficulty::Routine, target: 7, dm: 3 }.chance_pct(), 91, "needs 4+ on 2D: 33 of 36 outcomes");
        assert_eq!(Task { difficulty: Difficulty::Routine, target: 7, dm: 0 }.chance_pct(), 58);
        assert_eq!((Level::of(0), Level::of(1), Level::of(2), Level::of(3), Level::of(4), Level::of(7), Level::of(8), Level::of(20)), (Level::Marginal, Level::Normal, Level::X2, Level::X2, Level::X4, Level::X4, Level::X8, Level::X8));
    }
    #[test]
    fn penetration_attenuation_damage_and_life_force() {
        use Band::*;
        assert_eq!([pen_at(4, Some(2), Close), pen_at(4, Some(2), Short), pen_at(4, Some(2), Medium), pen_at(4, Some(2), Long), pen_at(4, Some(2), VeryLong), pen_at(4, Some(2), Distant)], [4, 4, 4, 2, 2, 1], "/2: full to medium, half at long / very long, quarter beyond");
        assert_eq!([pen_at(3, Some(1), Short), pen_at(3, Some(1), Medium), pen_at(3, Some(1), Long)], [3, 1, 0], "/1 weapons (SMG, shotguns, PGMP) halve at medium");
        assert_eq!((pen_at(7, Some(4), VeryLong), pen_at(7, Some(4), Distant), pen_at(36, None, Distant)), (7, 3, 36));
        assert_eq!((class(10, 5), class(5, 5), class(4, 5), class(1, 0), class(0, 0), class(0, 1)), (Class::High, Class::Low, Class::Zero, Class::High, Class::Low, Class::Zero));
        assert_eq!(damage(3, 0, true, Level::X8, 3), 24, "a beat-by-8 pistol hit = 3 x 8 = 24 (R-5)");
        assert_eq!([damage(10, 5, true, Level::Marginal, 3), damage(10, 5, true, Level::Normal, 3), damage(10, 5, true, Level::X2, 3), damage(10, 5, true, Level::X4, 3)], [1, 3, 6, 12], "High: 50 / 100 / 200 / 400 %, fractions dropped");
        assert_eq!([damage(5, 5, true, Level::Marginal, 4), damage(5, 5, true, Level::Normal, 4), damage(5, 5, true, Level::X2, 4), damage(5, 5, true, Level::X8, 4)], [1, 2, 4, 16], "Low: 25 / 50 / 100 / 400 %");
        assert_eq!([damage(1, 5, true, Level::Normal, 4), damage(1, 5, true, Level::X2, 4), damage(1, 5, true, Level::X4, 4), damage(1, 5, true, Level::X8, 4)], [0, 1, 2, 4], "Zero on exposed areas: fixed points (a shotgun grazing cloth)");
        assert_eq!((damage(1, 20, false, Level::X8, 4), damage(1, 10, false, Level::X8, 4), damage(1, 5, false, Level::X8, 4)), (0, 4, 4), "enclosed: nothing below AV / 10, the fixed points at or above it");
        assert_eq!((damage(0, 0, true, Level::Normal, 3), damage(0, 3, true, Level::X2, 3)), (1, 1), "Pen 0: Low vs AV 0 (body pistol on no armour), Zero vs jack's melee / any AV");
        assert_eq!((life_force(7, 7, 7), life_force(3, 2, 2), life_force(1, 1, 0), life_force(15, 15, 15), life_force(8, 9, 10), life_force(7, 10, 7)), ((3, 5), (2, 1), (0, 0), (6, 10), (4, 6), (4, 5))); assert_eq!(life_force(1, 1, 1), (1, 1));
        assert_eq!((rounds_per_burst(2), rounds_per_burst(3), rounds_per_burst(4), rounds_per_burst(0)), (4, 10, 20, 1));
        assert_eq!((danger_radius(15), danger_radius(30), danger_radius(45), danger_radius(150), danger_radius(0)), (0, 1, 2, 9, 0));
        assert_eq!([danger_pen(9, 0), danger_pen(9, 1), danger_pen(9, 2), danger_pen(9, 3), danger_pen(9, 4), danger_pen(9, 5)], [9, 4, 2, 1, 0, 0], "RAM GL-10 HE: nothing beyond four squares");
    }
    #[test]
    fn the_tables_hold_every_1990_item() {
        assert!(WEAPONS_PM.iter().enumerate().all(|(i, w)| w.id as usize == i)); assert!(ARMOUR_PM.iter().enumerate().all(|(i, a)| a.id as usize == i + 29));
        let (r, g, l) = (&WEAPONS_PM[7], &WEAPONS_PM[9], &WEAPONS_PM[20]);
        assert_eq!((r.rds, r.pen, r.atten, r.dmg), (20, 4, Some(2), 3), "Errata PM p.76 rifle"); assert_eq!((g.dmg, g.gyro, g.auto, g.rapid), (4, true, 3, true)); assert_eq!((l.rds, l.class), (100, 0xAA));
        assert_eq!((WEAPONS_PM[17].pen, WEAPONS_PM[17].danger_dm, WEAPONS_PM[17].profile), (9, 150, Profile::Indirect), "RAM GL-10 HE Pen 9 (errata), 15 m");
        assert_eq!((WEAPONS_PM[15].recoil, WEAPONS_PM[25].recoil, WEAPONS_PM[16].recoil, WEAPONS_PM[17].recoil), (Recoil::Hi, Recoil::Hi, Recoil::Hi, Recoil::Med), "R-19");
        assert_eq!((WEAPONS_PM[27].block, WEAPONS_PM[26].block, WEAPONS_PM[27].skill, weapon(26).unwrap().skill), (3, 1, 65, 15));
        assert_eq!((weapon(29), weapon(0x81).map(|w| w.pen), weapon(0x83).map(|w| w.ignores_armour), grenade(3).map(|w| w.id), grenade(4)), (None, Some(9), Some(true), Some(0x83), None));
        assert!(WEAPONS_PM.iter().filter(|w| w.auto > 0).map(|w| w.id).eq([8, 9, 11, 12, 13, 14]), "autofire per Errata PM p.76");
        let v = ArmourValues::of(armour(33).unwrap()); assert_eq!((v.vs(0x68), v.vs(0xAA), v.vs(0x00), v.exposed), (1, 6, 1, true), "ablat");
        let v = ArmourValues::of(armour(34).unwrap()); assert_eq!((v.vs(0x78), v.vs(0xAA), v.vs(0xAB)), (0, 10, 0), "reflec: lasers only");
        assert_eq!((armour(39).unwrap().exposed, armour(29).unwrap().melee, armour_named("mesh").map(|a| a.id), armour_named("none")), (false, 1, Some(30), None));
    }
}
