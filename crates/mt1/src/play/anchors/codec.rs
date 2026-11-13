use std::collections::BTreeMap;
use crate::ground::{round, rules::{compendium, weapons::ArmourValues}};

pub(crate) trait Value: Sized {
    fn put(&self, out: &mut Vec<u8>);
    fn get(input: &mut &[u8]) -> Result<Self, String>;
}
fn take<'a>(input: &mut &'a [u8], n: usize) -> Result<&'a [u8], String> {
    if input.len() < n { return Err("play: truncated SQD/PLAY section".into()) }
    let (value, rest) = input.split_at(n); *input = rest; Ok(value)
}
macro_rules! number { ($($t:ty),*) => {$(
    impl Value for $t {
        fn put(&self, out: &mut Vec<u8>) { out.extend_from_slice(&self.to_le_bytes()); }
        fn get(input: &mut &[u8]) -> Result<Self, String> { Ok(Self::from_le_bytes(take(input, std::mem::size_of::<Self>())?.try_into().unwrap())) }
    }
)*}; }
number!(u8, u16, u32, u64, i16, i32, i64);
impl Value for bool {
    fn put(&self, out: &mut Vec<u8>) { (*self as u8).put(out); }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(false), 1 => Ok(true), _ => Err("play: invalid boolean".into()) } }
}
impl<T: Value> Value for Option<T> {
    fn put(&self, out: &mut Vec<u8>) { self.is_some().put(out); if let Some(v) = self { v.put(out); } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { if bool::get(input)? { Ok(Some(T::get(input)?)) } else { Ok(None) } }
}
impl<T: Value> Value for Vec<T> {
    fn put(&self, out: &mut Vec<u8>) { (self.len() as u32).put(out); for v in self { v.put(out); } }
    fn get(input: &mut &[u8]) -> Result<Self, String> {
        let n = u32::get(input)? as usize;
        if n > 16 << 20 || n > input.len() { return Err("play: invalid collection length".into()) }
        (0..n).map(|_| T::get(input)).collect()
    }
}
impl Value for String {
    fn put(&self, out: &mut Vec<u8>) { self.as_bytes().to_vec().put(out); }
    fn get(input: &mut &[u8]) -> Result<Self, String> { String::from_utf8(Vec::<u8>::get(input)?).map_err(|_| "play: invalid UTF-8".into()) }
}
impl<T: Value, const N: usize> Value for [T; N] {
    fn put(&self, out: &mut Vec<u8>) { for v in self { v.put(out); } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { let v: Result<Vec<_>, _> = (0..N).map(|_| T::get(input)).collect(); v?.try_into().map_err(|_| "play: bad array".into()) }
}
impl<K: Value + Ord, V: Value> Value for BTreeMap<K, V> {
    fn put(&self, out: &mut Vec<u8>) { (self.len() as u32).put(out); for (k, v) in self { k.put(out); v.put(out); } }
    fn get(input: &mut &[u8]) -> Result<Self, String> {
        let n = u32::get(input)? as usize;
        if n > 65536 || n > input.len() { return Err("play: invalid map length".into()) }
        let mut map = Self::new(); for _ in 0..n { if map.insert(K::get(input)?, V::get(input)?).is_some() { return Err("play: duplicate map key".into()) } } Ok(map)
    }
}
impl<A: Value, B: Value> Value for (A, B) {
    fn put(&self, out: &mut Vec<u8>) { self.0.put(out); self.1.put(out); }
    fn get(input: &mut &[u8]) -> Result<Self, String> { Ok((A::get(input)?, B::get(input)?)) }
}
impl<A: Value, B: Value, C: Value> Value for (A, B, C) {
    fn put(&self, out: &mut Vec<u8>) { self.0.put(out); self.1.put(out); self.2.put(out); }
    fn get(input: &mut &[u8]) -> Result<Self, String> { Ok((A::get(input)?, B::get(input)?, C::get(input)?)) }
}
macro_rules! record { ($t:path, $($f:ident),+ $(,)?) => {
    impl Value for $t {
        fn put(&self, out: &mut Vec<u8>) { $(self.$f.put(out);)+ }
        fn get(input: &mut &[u8]) -> Result<Self, String> { Ok(Self { $($f: Value::get(input)?,)+ }) }
    }
}; }
record!(mt_engine::present::textlog::Run, page, x, y, colour, shadow, bytes, font_id, width, descend, background_width, background_height, background, bitmap, covered);
record!(mt_engine::present::textlog::TextLog, runs, visible, overflowed, page, regs);
record!(mt_engine::present::textlog::SurfaceImage, pending, cells, overflowed, events, frame_bytes, last);
record!(ArmourValues, bullets, lasers, melee, exposed);
record!(compendium::Entry, id, label, animal, upp, hits, skills, tactics, leader, recon, morale, weapon, armour, from_record, source, animal_attack);
record!(compendium::Compendium, version, weapon_skill, roles, picture, records);
record!(round::Rect, r0, c0, r1, c1);
record!(round::Def, id, start, end, boundary, participants, auto_break, allow_save, surprise, attacker, alerted_rounds);
record!(round::NpcSt, role, skills, engaged, alerted, alert_full, last_seen, speed, acted, clip, reload, routed);
macro_rules! enumeration { ($t:path, $($tag:literal => $variant:ident),+ $(,)?) => {
    impl Value for $t {
        fn put(&self, out: &mut Vec<u8>) { let n: u8 = match self { $(Self::$variant => $tag,)+ }; n.put(out); }
        fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { $($tag => Ok(Self::$variant),)+ _ => Err("play: invalid enum".into()) } }
    }
}; }
enumeration!(compendium::Morale, 0 => VeryLow, 1 => Low, 2 => Average, 3 => High);
enumeration!(round::Surprise, 0 => Roll, 1 => Attackers, 2 => Defenders, 3 => None);
enumeration!(round::Side, 0 => Party, 1 => Npcs);
impl Value for round::Start {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Sight => 0u8.put(out), Self::Area(v) => { 1u8.put(out); v.put(out); }, Self::Unsupported(v) => { 2u8.put(out); v.put(out); } } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Sight), 1 => Ok(Self::Area(Value::get(input)?)), 2 => Ok(Self::Unsupported(Value::get(input)?)), _ => Err("play: invalid start".into()) } }
}
impl Value for round::End {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Open { rounds } => { 0u8.put(out); rounds.put(out); }, Self::Locked { escape } => { 1u8.put(out); escape.put(out); } } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Open { rounds: Value::get(input)? }), 1 => Ok(Self::Locked { escape: Value::get(input)? }), _ => Err("play: invalid end".into()) } }
}
impl Value for round::Boundary {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Radius(v) => { 0u8.put(out); v.put(out); }, Self::Area(v) => { 1u8.put(out); v.put(out); }, Self::Level => 2u8.put(out), Self::Screen => 3u8.put(out) } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Radius(Value::get(input)?)), 1 => Ok(Self::Area(Value::get(input)?)), 2 => Ok(Self::Level), 3 => Ok(Self::Screen), _ => Err("play: invalid boundary".into()) } }
}

use crate::{ground::{squad, orders, fire, tactics, standing, npc_turn, rules}, ui::{Unit, Target}};
record!(squad::State, encounter, round, pools, ablat_worn, compendium, derived, p1, orders, fire, p22, p6);
record!(round::State, defs, done, encounter, turns, npcs, strip, log, acting, fx, logged, clock, pending, explore_ticks, walked);
record!(round::Turn, moved, attacked, done, hold, speed, evade, mp);
record!(round::Encounter, def, round, side, first, choosing, surprised, quiet, grouped, group_moved, pools, npc_start, withdrawing, morale_round, interrupt);
record!(round::Fx, seq, unit, walk, shots, hits, blasts);
record!(orders::State, full_auto, menu, hover, path, target, hovered);
record!(orders::Menu, target, items, default);
record!(orders::MenuItem, key, label, op, grey);
record!(fire::State, rapid, reloading, disarmed, pending);
record!(fire::Pending, who, cell, weapon, task, round, launcher);
record!(tactics::State, declared, spent, round, paused);
record!(standing::State, advance);
record!(rules::Task, difficulty, target, dm);
record!(mt_engine::play::Checkpoint, releases, mouse, frame);
record!(mt_engine::io::MouseState, buttons, col, row);
record!(crate::space::mouse::Driver, x, y, buttons, range, seen);
record!(mt_engine::present::ly::Pen, colour, op, ops);
record!(crate::space::flight::Flight, scroll, old_x, old_y, command, stop, old_direction);
record!(crate::space::sounds::Image, ds, segs, bios_ticks, refresh, samples, duration);
enumeration!(orders::Speed, 0 => Auto, 1 => Run, 2 => Evade, 3 => Walk);
enumeration!(rules::Difficulty, 0 => Simple, 1 => Routine, 2 => Difficult, 3 => Formidable, 4 => Impossible);
enumeration!(crate::ground::main_loop::Resume, 0 => Top, 1 => OuterTurn, 2 => OuterPoll, 3 => IdleTurn, 4 => IdlePoll);
impl Value for usize {
    fn put(&self, out: &mut Vec<u8>) { (*self as u64).put(out); }
    fn get(input: &mut &[u8]) -> Result<Self, String> { usize::try_from(u64::get(input)?).map_err(|_| "play: integer exceeds platform size".into()) }
}
impl Value for Unit {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Traveller(n) => { 0u8.put(out); n.put(out) }, Self::Npc(n) => { 1u8.put(out); n.put(out) } } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Traveller(Value::get(input)?)), 1 => Ok(Self::Npc(Value::get(input)?)), _ => Err("play: invalid unit".into()) } }
}
impl Value for Target {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Cell(v) => { 0u8.put(out); v.put(out) }, Self::Unit(v) => { 1u8.put(out); v.put(out) }, Self::Object(v) => { 2u8.put(out); v.put(out) }, Self::Portrait(v) => { 3u8.put(out); v.put(out) } } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Cell(Value::get(input)?)), 1 => Ok(Self::Unit(Value::get(input)?)), 2 => Ok(Self::Object(Value::get(input)?)), 3 => Ok(Self::Portrait(Value::get(input)?)), _ => Err("play: invalid target".into()) } }
}
impl Value for npc_turn::Plan {
    fn put(&self, out: &mut Vec<u8>) { match self { Self::Flee { left, attack } => { 0u8.put(out); left.put(out); attack.put(out) }, Self::Approach { left } => { 1u8.put(out); left.put(out) }, Self::Attack { checked } => { 2u8.put(out); checked.put(out) } } }
    fn get(input: &mut &[u8]) -> Result<Self, String> { match u8::get(input)? { 0 => Ok(Self::Flee { left: Value::get(input)?, attack: Value::get(input)? }), 1 => Ok(Self::Approach { left: Value::get(input)? }), 2 => Ok(Self::Attack { checked: Value::get(input)? }), _ => Err("play: invalid NPC continuation".into()) } }
}
impl Value for orders::Op {
    fn put(&self, out: &mut Vec<u8>) {
        use orders::Op::*;
        let tag: u8 = match self { Move(_) => 0, HoldHere => 1, Fire => 2, SetTarget => 3, Converse => 4, Take => 5, Select(_) => 6, Reload => 7,
            SwapHand => 8, FireMode => 9, Hold => 10, Character(_) => 11, Party => 12, Vehicle => 13, Later(_) => 14,
            FullAuto => 15, Rapid => 16, Pinpoint => 17, Melee => 18, Disarm => 19, Throw => 20, Launch => 21,
            Advance => 22, HoldFire => 23, HoldRun => 24, Tactics => 25, Look => 26 };
        tag.put(out);
        match self { Move(v) => v.put(out), Select(v) | Character(v) => v.put(out), Later(v) => v.to_string().put(out), _ => {} }
    }
    fn get(input: &mut &[u8]) -> Result<Self, String> {
        use orders::Op::*;
        Ok(match u8::get(input)? { 0 => Move(Value::get(input)?), 1 => HoldHere, 2 => Fire, 3 => SetTarget, 4 => Converse, 5 => Take, 6 => Select(Value::get(input)?), 7 => Reload,
            8 => SwapHand, 9 => FireMode, 10 => Hold, 11 => Character(Value::get(input)?), 12 => Party, 13 => Vehicle,
            14 => Later(match String::get(input)?.as_str() { "P15" => "P15", "P16" => "P16", "P22" => "P22", _ => return Err("play: unknown deferred operation".into()) }),
            15 => FullAuto, 16 => Rapid, 17 => Pinpoint, 18 => Melee, 19 => Disarm, 20 => Throw, 21 => Launch, 22 => Advance, 23 => HoldFire, 24 => HoldRun, 25 => Tactics, 26 => Look,
            _ => return Err("play: invalid operation".into()) })
    }
}
