use super::*;
use mt_engine::sim::tick::Clock;

pub fn frame_hash(frame:&[u32])->String {
    let mut h=mt_engine::play::Fnv::default();
    for pixel in frame {h.write(&pixel.to_le_bytes());} h.state()
}

#[derive(Clone,Copy,Debug)]
pub struct LiveStart {pub ticks:u32,pub now:u32}

pub fn file(path:&Path,dir:&Path,exe_dir:&Path,factory:impl FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>+'static)->Result<(),String> {
    if path.extension().and_then(|s|s.to_str())!=Some("json") {return Err("--snapshot needs a snapshot JSON file".into())}
    let id=path.file_stem().and_then(|s|s.to_str()).ok_or("play: invalid snapshot filename")?;
    let snapshot=crate::play_store::load_for_build(path.parent().unwrap_or(Path::new(".")),id,env!("MT_BUILD_HASH"))?;
    run(snapshot,dir,exe_dir,factory)
}

pub fn run(snapshot:Snapshot,dir:&Path,exe_dir:&Path,factory:impl FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>+'static)->Result<(),String> {
    resume(snapshot,dir,exe_dir,None,factory).map(|_|())
}
fn resume(snapshot:Snapshot,dir:&Path,exe_dir:&Path,request:Option<crate::scenario::Request>,factory:impl FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>+'static)->Result<Option<mt1::ground::Ground>,String> {
    crate::play_store::compatible_build(&snapshot,env!("MT_BUILD_HASH"))?;
    let verdict=mt1::gamefiles::verify(dir);
    if verdict.refuses(){return Err(verdict.text(dir))}
    if !verdict.ok(){eprintln!("{}",verdict.text(dir));}
    snapshot.validate()?; boot::validate(&snapshot.root,dir)?;
    eprintln!("snapshot {}: {:?} boundary, replay {} choices to d:{}",snapshot.id,snapshot.root.kind,snapshot.journal.entries.len(),snapshot.sequence);
    let root=snapshot.root.clone();let mut play=Play::new();play.restore(snapshot)?;
    let observed=Rc::new(RefCell::new(Observed{desktop:request,..Default::default()}));
    let activated=Rc::new(std::cell::Cell::new(false));let ready=activated.clone();
    let factory=move|start| {let result=factory(start);if result.is_ok(){ready.set(true)} result};
    let clock=Rc::new(RefCell::new(ClockState{source:Box::new(mt_engine::sim::tick::Counter::default()),offset:0}));
    let input=Handoff {replay:Answerer{play,observed:observed.clone(),anchors:true,heartbeat:None},live:None,factory:Some(Box::new(factory)),clock,
        scene:None,releases:VecDeque::new(),save_dir:exe_dir.join("saves")};
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run_session(&root,dir,exe_dir,Box::new(input),observed))) {
        Ok(result)=>{let game=result?;if activated.get(){Ok(game)}else{Err("play: game ended before the snapshot decision".into())}},
        Err(exit)=>{
            if let Some(error)=exit.downcast_ref::<LaunchError>() {return Err(error.0.clone())}
            if let Some(SessionExit::ReplayMismatch(error))=exit.downcast_ref::<SessionExit>() {return Err(error.clone())}
            std::panic::resume_unwind(exit)
        }
    }
}

pub fn run_desktop(snapshot:Snapshot,dir:&Path,exe_dir:&Path,request:crate::scenario::Request,factory:impl FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>+'static)->Result<Option<mt1::ground::Ground>,String> {
    resume(snapshot,dir,exe_dir,Some(request),factory).map(|game|game.filter(|g|g.game_over && !g.input.closed()))
}

pub fn file_desktop(path:&Path,dir:&Path,exe_dir:&Path,request:crate::scenario::Request,factory:impl FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>+'static)->Result<Option<mt1::ground::Ground>,String> {
    if path.extension().and_then(|s|s.to_str())!=Some("json") {return Err("--snapshot needs a snapshot JSON file".into())}
    let id=path.file_stem().and_then(|s|s.to_str()).ok_or("play: invalid snapshot filename")?;
    let snapshot=crate::play_store::load_for_build(path.parent().unwrap_or(Path::new(".")),id,env!("MT_BUILD_HASH"))?;
    run_desktop(snapshot,dir,exe_dir,request,factory)
}

#[derive(Debug)]
struct LaunchError(String);
type Factory=Box<dyn FnOnce(LiveStart)->Result<(Box<dyn Input>,Box<dyn Clock>),String>>;
struct ClockState {source:Box<dyn Clock>,offset:u32}
struct SharedClock(Rc<RefCell<ClockState>>);
impl Clock for SharedClock {
    fn now(&mut self)->u32 {self.0.borrow_mut().source.now()}
    fn ticks(&mut self)->u32 {let mut c=self.0.borrow_mut();c.source.ticks().wrapping_add(c.offset)}
    fn set(&mut self,n:u32) {self.0.borrow_mut().source.set(n)}
    fn wait_tick(&mut self) {self.0.borrow_mut().source.wait_tick()}
    fn busy_wait(&mut self) {self.0.borrow_mut().source.busy_wait()}
}
struct Handoff {
    replay:Answerer,live:Option<Box<dyn Input>>,factory:Option<Factory>,clock:Rc<RefCell<ClockState>>,
    scene:Option<mt1::ui::Scene>,releases:VecDeque<Event>,
    save_dir:PathBuf,
}
impl Handoff {
    fn target(&self)->bool {self.live.is_none() && !self.replay.play.replaying() && self.replay.play.pending().is_some()}
    fn input(&mut self)->&mut dyn Input {match self.live.as_mut() {Some(live)=>live.as_mut(),None=>&mut self.replay}}
    fn activate(&mut self) {
        let start={let mut state=self.clock.borrow_mut();LiveStart {now:state.source.now(),ticks:state.source.ticks().wrapping_add(state.offset)}};
        let result=(self.factory.take().expect("single live handoff"))(start);
        let (mut live,mut clock)=result.unwrap_or_else(|e|std::panic::resume_unwind(Box::new(LaunchError(e))));
        let checkpoint=self.replay.play.handoff().expect("verified decision has no unfinished native input");
        self.releases=checkpoint.releases.into_iter().map(Event::KeyUp).collect();
        let mut state=self.clock.borrow_mut();let now=state.source.now();let ticks=state.source.ticks().wrapping_add(state.offset);
        clock.set(now);let offset=ticks.wrapping_sub(clock.ticks());state.source=clock;state.offset=offset;drop(state);
        live.set_mouse_position(checkpoint.mouse.col,checkpoint.mouse.row);
        if let Some(scene)=&self.scene {live.scene(scene)}
        live.decide(self.replay.play.pending().expect("verified decision"));
        live.present(&checkpoint.frame);
        self.live=Some(live);
    }
}
impl Input for Handoff {
    fn live_save_location(&self)->Option<(&Path,&'static str)> {self.live.as_ref().map(|_|(self.save_dir.as_path(),env!("MT_BUILD_HASH")))}
    fn data_mode(&self)->bool {self.live.as_ref().map_or(true,|i|i.data_mode())}
    fn diagnostics(&self)->bool {self.live.as_ref().is_some_and(|i|i.diagnostics())}
    fn wants_decision(&self)->bool {self.live.as_ref().map_or_else(||self.replay.wants_decision(),|i|i.wants_decision())}
    fn decide(&mut self,d:&Decision) {
        if let Some(live)=self.live.as_mut() {live.decide(d);return}
        self.replay.decide(d);
    }
    fn play_clock(&mut self,clock:Box<dyn Clock>)->Box<dyn Clock> {
        *self.clock.borrow_mut()=ClockState{source:clock,offset:0};Box::new(SharedClock(self.clock.clone()))
    }
    fn play_checkpoint(&self)->Option<mt_engine::play::Checkpoint> {if self.live.is_none(){self.replay.play_checkpoint()}else{None}}
    fn resume_play_checkpoint(&mut self,s:&mt_engine::play::Checkpoint)->Result<(),String> {self.replay.resume_play_checkpoint(s)}
    fn anchor(&mut self,n:&str,b:&[u8]) {self.input().anchor(n,b)}
    fn closed(&self)->bool {self.live.as_ref().is_some_and(|i|i.closed())}
    fn next_key(&mut self)->u16 {if self.target(){self.activate()} self.releases.clear();self.input().next_key()}
    fn key_pending(&mut self)->bool {self.input().key_pending()}
    fn poll(&mut self,c:&mut dyn Clock)->Event {
        if self.target() && self.replay.play.poll_needs_answer() {self.activate()}
        self.releases.pop_front().unwrap_or_else(||self.input().poll(c))
    }
    fn arrived(&mut self)->bool {!self.releases.is_empty()||self.input().arrived()}
    fn flush(&mut self) {self.releases.clear();self.input().flush()}
    fn reset(&mut self) {self.input().reset()}
    fn mouse(&mut self)->MouseState {self.input().mouse()}
    fn set_mouse_position(&mut self,x:i16,y:i16) {self.input().set_mouse_position(x,y)}
    fn intent(&mut self)->Option<Box<dyn Any>> {
        if self.target() && self.replay.play.pending().is_some_and(|d|d.kind==Kind::CombatTurn) {self.activate()}
        self.input().intent()
    }
    fn present(&mut self,frame:&[u32]) {self.input().present(frame)}
    fn audio(&mut self,n:u16,hz:u32,levels:&[u8],rest:u8) {self.input().audio(n,hz,levels,rest)}
    fn scene(&mut self,s:&dyn Any) {
        if let Some(scene)=s.downcast_ref::<mt1::ui::Scene>() {self.scene=Some(scene.clone())}
        if let Some(o)=s.downcast_ref::<Observation>() {self.scene=Some(o.scene.clone())}
        self.input().scene(s)
    }
}

#[cfg(test)]
#[test]
fn live_save_location_is_absent_until_verified_handoff() {
    let observed=Rc::default();let mut play=Play::new();play.decide(&Decision::new(Kind::Ack,"test","handoff"));
    let mut input=Handoff {replay:Answerer{play,observed,anchors:true,heartbeat:None},live:None,
        factory:Some(Box::new(|start|Ok((Box::new(mt_engine::io::Scripted::new([])),Box::new(mt_engine::sim::tick::Counter::at(start.ticks)))))),
        clock:Rc::new(RefCell::new(ClockState{source:Box::new(mt_engine::sim::tick::Counter::default()),offset:0})),scene:None,releases:VecDeque::new(),save_dir:"saves".into()};
    assert!(input.live_save_location().is_none());
    input.activate();
    assert!(input.live_save_location().is_some());
}
