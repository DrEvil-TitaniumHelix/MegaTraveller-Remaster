use crate::io::{Event, Key};
use std::{any::Any, collections::BTreeMap, fmt, option::Option as Maybe, rc::Rc};

mod answerer;
mod replay;
mod public;
pub use answerer::{Play, SessionExit};
pub use public::{PublicDecision, PublicOption, PublicRow, PublicList, PublicTextInput, PublicTimeInput};

pub const HASH_VERSION: u32 = 5;
pub mod profile;

#[derive(Clone, Copy, Debug)]
pub struct Fnv(u64);
impl Default for Fnv { fn default() -> Self { Self(0xcbf29ce484222325) } }
impl Fnv {
    pub fn write(&mut self, bytes: &[u8]) { for &b in bytes { self.0 = (self.0 ^ u64::from(b)).wrapping_mul(0x100000001b3); } }
    pub fn field(&mut self, bytes: &[u8]) { self.write(&(bytes.len() as u64).to_le_bytes()); self.write(bytes); }
    pub fn finish(self) -> u64 { self.0 }
    pub fn state(self) -> String { format!("s:{:016x}", self.0) }
}
pub fn fnv(bytes: &[u8]) -> u64 { let mut h = Fnv::default(); h.write(bytes); h.finish() }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind { Menu, YesNo, List, Text, Number, Ack, Ground, Space, CombatTurn }
impl Kind {
    pub fn as_str(self) -> &'static str { match self { Self::Menu=>"menu", Self::YesNo=>"yesno", Self::List=>"list", Self::Text=>"text", Self::Number=>"number", Self::Ack=>"ack", Self::Ground=>"ground", Self::Space=>"space", Self::CombatTurn=>"combat.turn" } }
    pub fn is_command(self) -> bool { matches!(self, Self::Ground | Self::Space) }
}

#[derive(Clone)]
pub struct NativeIntent(Rc<dyn Fn() -> Box<dyn Any>>);
impl NativeIntent {
    pub fn new<T: Any + Clone>(value: T) -> Self { Self(Rc::new(move || Box::new(value.clone()))) }
    pub(super) fn make(&self) -> Box<dyn Any> { (self.0)() }
}
impl fmt::Debug for NativeIntent { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("NativeIntent(..)") } }

#[derive(Clone)]
pub struct NativePoint(Rc<dyn Fn((i16, i16), bool) -> Result<NativeIntent, String>>);
impl NativePoint {
    pub fn new(route: impl Fn((i16, i16), bool) -> Result<NativeIntent, String> + 'static) -> Self { Self(Rc::new(route)) }
    pub(super) fn make(&self, cell: (i16, i16), menu: bool) -> Result<NativeIntent,String> { (self.0)(cell, menu) }
}
impl fmt::Debug for NativePoint { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str("NativePoint(..)") } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Movement {
    pub origin: (i16, i16), pub cells: Vec<(i16, i16)>, pub max_steps: u16,
    pub directions: BTreeMap<String, (i16, i16)>, pub bounds: (i16, i16),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Checkpoint { pub releases: Vec<Key>, pub mouse: crate::io::MouseState, pub frame: Vec<u32> }

#[derive(Clone, Debug)]
pub struct Option {
    pub id: String, pub label: String, pub enabled: bool,
    pub input: Vec<Event>, pub intent: Maybe<NativeIntent>,
}
impl Option {
    pub fn new(id: impl Into<String>, label: impl Into<String>, input: Vec<Event>) -> Self { Self { id: id.into(), label: label.into(), enabled: true, input, intent: None } }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Context { pub program: String, pub routine: String, pub table: Maybe<String>, pub who: Maybe<usize>, pub depth: usize }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Charset { Digits, Upper,
    Ascii(String) }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextInput {
    pub max_len: usize, pub charset: Charset, pub submit: Vec<Event>,
    pub width: Maybe<(u32, Vec<u8>)>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub id: String, pub label: String, pub enabled: bool, pub price: u64, pub limit: u32, pub qty: u32,
    pub min_qty: u32,
    pub input: Vec<Event>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct List {
    pub rows: Vec<Row>, pub current: usize, pub more: bool, pub money: u64,
    pub increase: Vec<Event>, pub decrease: Vec<Event>, pub submit: Vec<Event>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimeInput { pub max_ticks: u32, pub directions: BTreeMap<String, Key> }
#[derive(Clone, Debug)]
pub struct Decision {
    pub id: String, pub kind: Kind, pub prompt: Maybe<String>, pub options: Vec<Option>,
    pub list: Maybe<List>, pub input: Maybe<TextInput>, pub time: Maybe<TimeInput>,
    pub default: Maybe<String>, pub context: Context,
    pub state: String,
    pub movement: Maybe<Movement>, pub point: Maybe<NativePoint>,
    pub rects: Vec<(usize, [i32; 4])>,
}
impl Decision {
    pub fn new(kind: Kind, program: impl Into<String>, routine: impl Into<String>) -> Self {
        Self { id: String::new(), kind, prompt: None, options: vec![], list: None, input: None, time: None, default: None,
            context: Context { program: program.into(), routine: routine.into(), ..Context::default() }, state: String::new(), movement: None, point: None, rects: vec![] }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pick { Id(String), Row { row: String, qty: u32 }, Text(String), Number(i64) }
impl From<&str> for Pick { fn from(s: &str) -> Self { Self::Id(s.into()) } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice { pub decision: String, pub pick: Pick }
impl Choice { pub fn new(decision: impl Into<String>, pick: impl Into<Pick>) -> Self { Self { decision: decision.into(), pick: pick.into() } } }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Grant { pub requested: u32, pub used: u32, pub remaining: u32 }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub choice: Choice, pub before: String, pub after: Maybe<String>,
    pub label_hash: Maybe<u32>, pub grant: Grant,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Journal { pub hash_version: u32, pub entries: Vec<Entry> }
impl Default for Journal { fn default() -> Self { Self { hash_version: HASH_VERSION, entries: vec![] } } }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RootKind { NewGame, Save, Anchor }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Root {
    pub kind: RootKind, pub image: Maybe<Vec<u8>>, pub start_tick: u32, pub scenario: Maybe<String>, pub gamefiles: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Anchor { pub name: String, pub image: Vec<u8>, pub sequence: u64, pub journal_at: usize }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub id: String, pub name: String, pub created: String, pub build: String, pub root: Root,
    pub journal: Journal, pub sequence: u64, pub hash: String,
}

#[cfg(test)]
mod tests;
