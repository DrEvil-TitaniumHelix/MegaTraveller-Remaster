use super::*;
use crate::{io::{Input, MouseState}, sim::tick::Clock};
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionExit { AwaitingDecision, NewGame(Root), Restore(Snapshot), InvalidChoice(String), ReplayMismatch(String), MissingDecision, SequenceExhausted }
impl SessionExit { pub fn leave(self) -> ! { std::panic::resume_unwind(Box::new(self)) } }

type Service = Box<dyn FnMut(&mut Play)>;
type SceneCopy = Box<dyn Fn(&dyn Any) -> Maybe<Box<dyn Any>>>;
#[derive(Default)]
pub struct Play {
    pub(super) pending: Vec<Decision>, pub(super) sequence: u64,
    events: VecDeque<Event>, intents: VecDeque<NativeIntent>,
    grant: Maybe<(Grant, Maybe<Key>)>,
    pub(super) journal: Journal, pub(super) anchor: Maybe<Anchor>,
    frame: Vec<u32>, mouse: MouseState, scene: Maybe<Box<dyn Any>>, scene_copy: Maybe<SceneCopy>,
    service: Maybe<Service>, choices: VecDeque<Choice>,
    pub(super) replay: Maybe<super::replay::Replay>,
}
impl Play {
    pub fn new() -> Self { Self::default() }
    pub fn with_service(service: impl FnMut(&mut Play) + 'static) -> Self { Self { service: Some(Box::new(service)), ..Self::new() } }
    pub fn scripted(choices: impl IntoIterator<Item = Choice>) -> Self { Self { choices: choices.into_iter().collect(), ..Self::new() } }
    pub fn pending(&self) -> Maybe<&Decision> { self.pending.last() }
    pub fn pending_public(&self) -> Maybe<PublicDecision> { self.pending().map(Decision::public) }
    pub fn stack(&self) -> &[Decision] { &self.pending }
    pub fn sequence(&self) -> u64 { self.sequence }
    pub fn start_sequence(&mut self, sequence: u64) -> Result<(), String> {
        if self.sequence != 0 || !self.pending.is_empty() || !self.journal.entries.is_empty() { return Err("play: session already started".into()) }
        if sequence >= u64::MAX - 1 { return Err("play: decision sequence exhausted".into()) }
        self.sequence = sequence; Ok(())
    }
    pub fn journal(&self) -> &Journal { &self.journal }
    pub fn latest_anchor(&self) -> Maybe<&Anchor> { self.anchor.as_ref() }
    pub fn frame(&self) -> &[u32] { &self.frame }
    pub fn poll_needs_answer(&self)->bool {self.events.is_empty() && !self.pending.is_empty() && self.grant.is_none()}
    pub fn handoff(&mut self) -> Result<Checkpoint,String> {
        if self.pending.is_empty() || self.playing() || self.grant.is_some() || self.replaying() {return Err("play: not at a verified handoff".into())}
        Ok(Checkpoint {releases:self.events.drain(..).filter_map(|e|if let Event::KeyUp(k)=e {Some(k)} else {None}).collect(),mouse:self.mouse,frame:self.frame.clone()})
    }
    pub fn grant(&self) -> Maybe<&Grant> { self.grant.as_ref().map(|(g, _)| g) }
    pub fn copy_scene<T: Any + Clone>(&mut self) { self.scene_copy = Some(Box::new(|s| s.downcast_ref::<T>().map(|s| Box::new(s.clone()) as Box<dyn Any>))); }
    pub fn scene_ref<T: Any>(&self) -> Maybe<&T> { self.scene.as_ref()?.downcast_ref() }

    fn playing(&self) -> bool { !self.intents.is_empty() || self.events.iter().any(|e| !matches!(e, Event::KeyUp(_))) }
    pub fn register(&mut self, d: &Decision) -> bool {
        if self.playing() || (d.kind.is_command() && self.grant.is_some()) { return false }
        let sequence = self.sequence.checked_add(1).filter(|&n| n < u64::MAX).unwrap_or_else(|| SessionExit::SequenceExhausted.leave());
        if let Some((grant, held)) = self.grant.take() {
            if let Some(entry) = self.journal.entries.last_mut() { entry.grant = grant; }
            if let Some(key) = held { self.events.push_back(Event::KeyUp(key)); }
        }
        self.sequence = sequence;
        let mut d = d.clone(); d.id = format!("d:{}", self.sequence); d.context.depth = self.pending.len();
        let mut h = Fnv::default(); h.write(&HASH_VERSION.to_le_bytes()); h.field(d.state.as_bytes());
        h.write(&self.sequence.to_le_bytes()); h.write(&(self.pending.len() as u64).to_le_bytes());
        for parent in &self.pending { h.field(parent.id.as_bytes()); h.field(parent.state.as_bytes()); }
        h.field(d.kind.as_str().as_bytes()); h.field(format!("{:?}", d.context).as_bytes());
        h.field(format!("{:?}", d.default).as_bytes());
        h.field(format!("{:?}", d.list).as_bytes()); h.field(format!("{:?}", self.events).as_bytes());
        h.field(format!("{:?}", d.input).as_bytes()); h.field(format!("{:?}", d.time).as_bytes());
        h.field(format!("{:?}", d.movement).as_bytes());
        d.state = h.state();
        if let Some(entry) = self.journal.entries.last_mut().filter(|e| e.after.is_none()) { entry.after = Some(d.state.clone()); }
        self.pending.push(d);
        if let Err(error) = self.check_replay() { SessionExit::ReplayMismatch(error).leave(); }
        true
    }
    pub fn choose(&mut self, choice: Choice) -> Result<(), String> {
        let _profile = profile::span("journal_choose");
        let d = self.pending().ok_or("play: no pending decision")?;
        if choice.decision != d.id { return Err(format!("play: stale decision {}; expected {}", choice.decision, d.id)) }
        if self.playing() || self.grant.is_some() { return Err("play: native input is still being consumed".into()) }
        let route = route(d, &choice.pick)?;
        let before = d.state.clone();
        self.check_replay_choice(&choice)?;
        self.pending.pop();
        self.events.extend(route.events); self.intents.extend(route.intent);
        let grant = route.grant.as_ref().map(|(g, _)| g.clone()).unwrap_or_default();
        self.grant = route.grant;
        self.journal.entries.push(Entry { choice, before, after: None, label_hash: route.label_hash, grant });
        if let Some(replay) = &mut self.replay { replay.next += 1; }
        Ok(())
    }
    fn answer(&mut self) {
        while !self.pending.is_empty() && !self.playing() && self.grant.is_none() {
            if let Some(choice) = self.replay_choice() {
                if let Err(e) = self.choose(choice) { SessionExit::ReplayMismatch(e).leave(); }
            } else if let Some(choice) = self.choices.pop_front() {
                if let Err(e) = self.choose(choice) { SessionExit::InvalidChoice(e).leave(); }
            } else if let Some(mut service) = self.service.take() {
                service(self); self.service = Some(service);
            } else { SessionExit::AwaitingDecision.leave(); }
            if !self.events.is_empty() || !self.intents.is_empty() { break }
        }
    }
}
impl Input for Play {
    fn data_mode(&self) -> bool { true }
    fn play_checkpoint(&self) -> Maybe<Checkpoint> {
        if !self.pending.is_empty() || self.playing() || self.grant.is_some() { return None }
        Some(Checkpoint { releases: self.events.iter().filter_map(|e| if let Event::KeyUp(k)=e {Some(*k)} else {None}).collect(), mouse:self.mouse, frame:self.frame.clone() })
    }
    fn resume_play_checkpoint(&mut self, state: &Checkpoint) -> Result<(), String> {
        if !self.pending.is_empty() || !self.events.is_empty() || !self.intents.is_empty() || self.grant.is_some() { return Err("play: cannot install transport state in an active session".into()) }
        self.events=state.releases.iter().copied().map(Event::KeyUp).collect(); self.mouse=state.mouse; self.frame=state.frame.clone(); Ok(())
    }
    fn wants_decision(&self) -> bool { self.pending.is_empty() && !self.playing() }
    fn decide(&mut self, d: &Decision) { self.register(d); }
    fn anchor(&mut self, name: &str, image: &[u8]) {
        if self.play_checkpoint().is_none() { return }
        self.anchor = Some(Anchor { name: name.into(), image: image.to_vec(), sequence: self.sequence, journal_at: self.journal.entries.len() });
    }
    fn next_key(&mut self) -> Key {
        loop {
            if let Some(event) = self.events.pop_front() { if let Event::Key(k) = event { return k } continue }
            if self.pending.is_empty() { SessionExit::MissingDecision.leave(); }
            self.answer();
        }
    }
    fn key_pending(&mut self) -> bool { self.grant.is_none() && (!self.pending.is_empty() || self.events.iter().any(|e| matches!(e, Event::Key(_)))) }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event {
        if self.events.is_empty() && !self.pending.is_empty() { self.answer(); }
        if let Some(e) = self.events.pop_front() {
            if let Event::Mouse(m) = e { self.mouse = MouseState { buttons: m.buttons, col: m.col, row: m.row }; }
            return e
        }
        if !self.intents.is_empty() { return Event::Idle }
        if let Some((grant, held)) = &mut self.grant {
            clock.wait_tick(); grant.used += 1; grant.remaining -= 1;
            if let Some(entry) = self.journal.entries.last_mut() { entry.grant = grant.clone(); }
            if grant.remaining == 0 { if let Some(k) = held { self.events.push_back(Event::KeyUp(*k)); } self.grant = None; }
            return Event::Idle
        }
        clock.wait_tick(); Event::Idle
    }
    fn intent(&mut self) -> Maybe<Box<dyn Any>> {
        if self.intents.is_empty() && self.pending().is_some_and(|d| d.kind == Kind::CombatTurn) { self.answer(); }
        self.intents.pop_front().map(|i| i.make())
    }
    fn mouse(&mut self) -> MouseState { self.mouse }
    fn refuse_grant(&mut self) {
        if !matches!(self.grant, Some((_, Some(_)))) { return }
        let Some((grant, held)) = self.grant.take() else { return };
        if let Some(entry) = self.journal.entries.last_mut() { entry.grant = grant; }
        if let Some(k) = held { self.events.push_back(Event::KeyUp(k)); }
    }
    fn set_mouse_position(&mut self, col: i16, row: i16) { self.mouse.col = col; self.mouse.row = row; }
    fn present(&mut self, rgb: &[u32]) { self.frame.clear(); self.frame.extend_from_slice(rgb); }
    fn scene(&mut self, scene: &dyn Any) { if let Some(copy) = &self.scene_copy { self.scene = copy(scene); } }
}

#[derive(Default)]
struct Route { events: Vec<Event>, intent: Maybe<NativeIntent>, grant: Maybe<(Grant, Maybe<Key>)>, label_hash: Maybe<u32> }
fn native(events: &[Event]) -> Result<Vec<Event>, String> {
    let mut out = Vec::new();
    for (i, &event) in events.iter().enumerate() {
        if matches!(event, Event::Idle | Event::Boundary) { return Err("play: native option contains a time or harness event".into()) }
        out.push(event);
        if let Event::Key(k) = event { if events.get(i + 1) != Some(&Event::KeyUp(k)) { out.push(Event::KeyUp(k)); } }
    }
    Ok(out)
}
fn route(d: &Decision, pick: &Pick) -> Result<Route, String> {
    if let Pick::Id(id) = pick {
        if let Some(option) = d.options.iter().find(|o| &o.id == id) {
            if !option.enabled { return Err(format!("play: option {id} is disabled")) }
            if option.input.is_empty() && option.intent.is_none() { return Err("play: option has no native route".into()) }
            return Ok(Route { events: native(&option.input)?, intent: option.intent.clone(), label_hash: Some(fnv(option.label.as_bytes()) as u32), ..Route::default() })
        }
        if d.kind.is_command() { return time_route(d, id) }
        if d.movement.is_some() { return point_route(d, id) }
    }
    match (d.kind, pick) {
        (Kind::Text, Pick::Text(value) | Pick::Id(value)) => text_route(d, value),
        (Kind::Number, Pick::Number(value)) if *value >= 0 => text_route(d, &value.to_string()),
        (Kind::List, Pick::Row { row, qty }) => {
            let list = d.list.as_ref().ok_or("play: no list metadata")?;
            let row = list.rows.iter().find(|r| &r.id == row).ok_or("play: unknown row")?;
            if !row.enabled { return Err("play: row is disabled".into()) }
            if *qty < row.min_qty { return Err("play: quantity is below row minimum".into()) }
            if *qty > row.limit { return Err("play: quantity exceeds row limit".into()) }
            if row.price.checked_mul(u64::from(*qty)).is_none_or(|cost| cost > list.money) { return Err("play: insufficient money".into()) }
            let delta = qty.abs_diff(row.qty);
            let step = if *qty >= row.qty { &list.increase } else { &list.decrease };
            if delta != 0 && step.is_empty() { return Err("play: quantity cannot be changed".into()) }
            if delta > 65535 { return Err("play: quantity sequence exceeds 65535 steps".into()) }
            let mut events = native(&row.input)?;
            let step = native(step)?; for _ in 0..delta { events.extend_from_slice(&step); }
            events.extend(native(&list.submit)?);
            if events.is_empty() { return Err("play: row has no native route".into()) }
            Ok(Route { events, label_hash: Some(fnv(row.label.as_bytes()) as u32), ..Route::default() })
        }
        _ => Err("play: pick is not offered by this decision".into()),
    }
}
fn text_route(d: &Decision, value: &str) -> Result<Route, String> {
    let input = d.input.as_ref().ok_or("play: no text metadata")?;
    if value.len() >= input.max_len { return Err("play: text exceeds native buffer length".into()) }
    if !value.is_ascii() || !value.bytes().all(|b| match &input.charset {
        Charset::Digits => b.is_ascii_digit(), Charset::Upper => b.is_ascii_uppercase() || b.is_ascii_digit(),
        Charset::Ascii(chars) => chars.as_bytes().contains(&b),
    }) { return Err("play: text contains a character outside the native alphabet".into()) }
    if input.submit.is_empty() { return Err("play: text has no submit route".into()) }
    if let Some((limit, widths)) = &input.width {
        let width = value.bytes().try_fold(0u32, |n, b| n.checked_add(u32::from(*widths.get(b as usize)?)));
        if width.is_none_or(|n| n > *limit) { return Err("play: text exceeds native field width".into()) }
    }
    let mut events = native(&value.bytes().map(|b| Event::Key(u16::from(b))).collect::<Vec<_>>())?;
    events.extend(native(&input.submit)?); Ok(Route { events, ..Route::default() })
}
fn point_route(d: &Decision, id: &str) -> Result<Route, String> {
    let m = d.movement.as_ref().ok_or("play: movement is not offered")?;
    let coordinate = |s: &str| -> Result<(i16,i16),String> {
        let (r,c) = s.split_once(',').ok_or("play: expected row,column")?;
        Ok((r.parse().map_err(|_| "play: invalid row")?, c.parse().map_err(|_| "play: invalid column")?))
    };
    let (cell, menu) = if let Some(s) = id.strip_prefix("menu:cell/") { (coordinate(s)?, true) }
    else if let Some(s) = id.strip_prefix("move:cell/") { (coordinate(s)?, false) }
    else {
        let parts:Vec<_> = id.split(':').collect();
        let ["move",direction,n] = parts.as_slice() else { return Err("play: expected move:<direction>:<cells>, move:cell/<row>,<column> or menu:cell/<row>,<column>".into()) };
        let &(dr,dc) = m.directions.get(*direction).ok_or("play: direction is not offered")?;
        if n.is_empty() || !n.bytes().all(|b|b.is_ascii_digit()) { return Err("play: cells must be positive decimal digits".into()) }
        let n:u16 = n.parse().map_err(|_| "play: cell count exceeds u16")?;
        if n==0 || n>m.max_steps { return Err(format!("play: cells must be in 1..={}",m.max_steps)) }
        let r=i32::from(m.origin.0)+i32::from(dr)*i32::from(n); let c=i32::from(m.origin.1)+i32::from(dc)*i32::from(n);
        ((i16::try_from(r).map_err(|_| "play: row out of bounds")?,i16::try_from(c).map_err(|_| "play: column out of bounds")?),false)
    };
    if cell.0<0 || cell.1<0 || cell.0>=m.bounds.0 || cell.1>=m.bounds.1 { return Err("play: cell out of bounds".into()) }
    if !menu && !id.starts_with("move:cell/") && m.cells.binary_search(&cell).is_err() { return Err("play: cell is outside the reachable movement range; request its menu for a longer route".into()) }
    let route=d.point.as_ref().ok_or("play: cell has no native route")?;
    let label=if menu {format!("Actions: cell/{},{}",cell.0,cell.1)} else {"Move".into()};
    Ok(Route { intent:Some(route.make(cell,menu)?), label_hash:Some(fnv(label.as_bytes()) as u32), ..Route::default() })
}
fn time_route(d: &Decision, id: &str) -> Result<Route, String> {
    let time = d.time.as_ref().ok_or("play: time is not offered")?;
    let parts: Vec<_> = id.split(':').collect();
    let (ticks, held) = match parts.as_slice() {
        ["wait", ticks] => (*ticks, None),
        ["move", direction, ticks] => (*ticks, Some(*time.directions.get(*direction).ok_or("play: direction is not offered")?)),
        _ => return Err("play: expected wait:<ticks> or move:<direction>:<ticks>".into()),
    };
    if ticks.is_empty() || !ticks.bytes().all(|b| b.is_ascii_digit()) { return Err("play: ticks must be positive decimal digits".into()) }
    let ticks: u32 = ticks.parse().map_err(|_| "play: ticks exceed u32")?;
    if ticks == 0 || ticks > time.max_ticks { return Err(format!("play: ticks must be in 1..={}", time.max_ticks)) }
    Ok(Route { events: held.map(Event::Key).into_iter().collect(), grant: Some((Grant { requested: ticks, used: 0, remaining: ticks }, held)), ..Route::default() })
}
