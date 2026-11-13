use mt_engine::{io::{Input, Event, MouseState}, play::{Charset, Choice, Decision, Journal, Kind, Pick, Play, Root, RootKind, SessionExit, Snapshot}};
use mt1::play::observation::Observation;
use serde_json::{json, Value};
use std::{any::Any, cell::RefCell, collections::VecDeque, io::{self, BufRead, Read, Write}, path::{Path, PathBuf}, rc::Rc};
mod boot;
mod frame;
pub mod launch;
mod state;
mod wire;
mod route;
mod trace;
pub mod executor;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod restore_tests;
#[cfg(test)]
mod launch_tests;
const SKILL: &str = include_str!("../../../skills/mt1-play/SKILL.md");
const URI: &str = "skill://mt1-play";

pub fn public_decision(d: &Decision) -> Value {
    let d: mt_engine::play::PublicDecision = d.public();
    json!({"id":d.id,"kind":d.kind.as_str(),"prompt":d.prompt,"default":d.default,"state":d.state,
        "options":d.options.iter().map(|o|json!({"id":o.id,"label":o.label,"enabled":o.enabled})).collect::<Vec<_>>(),
        "list":d.list.as_ref().map(|l|json!({"rows":l.rows.iter().map(|r|json!({"id":r.id,"label":r.label,"enabled":r.enabled,
            "price":r.price,"limit":r.limit,"qty":r.qty,"min_qty":r.min_qty})).collect::<Vec<_>>(),"current":l.current,"more":l.more,"money":l.money})),
        "input":d.input.as_ref().map(|i|json!({"max_len":i.max_len,"charset":match &i.charset{Charset::Digits=>"digits",Charset::Upper=>"upper",Charset::Ascii(_)=>"ascii"},
            "alphabet":match &i.charset{Charset::Ascii(a)=>Some(a),_=>None},"width":i.width})),
        "time":d.time.as_ref().map(|t|json!({"max_ticks":t.max_ticks,"directions":t.directions})),
        "movement":d.movement.as_ref().map(|m|json!({"origin":m.origin,"cells":m.cells,"max_steps":m.max_steps,
            "directions":m.directions.keys().collect::<Vec<_>>(),"bounds":m.bounds})),
        "context":{"program":d.context.program,"routine":d.context.routine,"table":d.context.table,"who":d.context.who,"depth":d.context.depth}})
}
fn pick_value(p: &Pick) -> Value { match p {Pick::Id(s)|Pick::Text(s)=>json!(s),Pick::Number(n)=>json!(n),Pick::Row{row,qty}=>json!({"row":row,"qty":qty})} }
fn journal(j: &Journal) -> Value {
    json!({"hash_version":j.hash_version,"entries":j.entries.iter().map(|e|json!({"decision":e.choice.decision,"pick":pick_value(&e.choice.pick),
        "before":e.before,"after":e.after,"label_hash":e.label_hash,"grant":{"requested":e.grant.requested,"used":e.grant.used,"remaining":e.grant.remaining}})).collect::<Vec<_>>()})
}
fn pick(v: &Value, d: &Decision) -> Result<Pick, String> {
    if let Some(s)=v.as_str() { return Ok(if d.kind==Kind::Text && !d.options.iter().any(|o|o.id==s) {Pick::Text(s.into())} else {Pick::Id(s.into())}) }
    if let Some(n)=v.as_i64() {return Ok(Pick::Number(n))}
    if let (Some(row),Some(qty))=(v["row"].as_str(),v["qty"].as_u64().and_then(|n|u32::try_from(n).ok())) {return Ok(Pick::Row{row:row.into(),qty})}
    Err("pick must be an offered id, text, signed integer or {row,qty}".into())
}
#[derive(Default)]
struct Observed { state: Option<Observation>, messages: Vec<String>, loaded: Option<(u8,u8,i8)>, story: crate::plus::journal::Journal, extensions:Vec<u8>, desktop:Option<crate::scenario::Request> }
#[derive(Default,serde::Serialize,serde::Deserialize)]
struct HostState {
    #[serde(flatten)]
    extensions:crate::scenario::Extensions,
    #[serde(default)]
    story:crate::plus::journal::Journal,
    #[serde(default)]
    messages:Vec<String>,
}
struct Answerer { play: Play, observed: Rc<RefCell<Observed>>, anchors: bool, heartbeat: Option<Box<dyn FnMut(&Play)>> }
impl Input for Answerer {
    fn data_mode(&self)->bool {true}
    fn wants_decision(&self)->bool {self.play.wants_decision()}
    fn decide(&mut self,d:&Decision) {
        let sequence=self.play.sequence(); self.play.decide(d);
        if self.play.sequence()!=sequence {if let Some(beat)=&mut self.heartbeat {beat(&self.play)}}
    }
    fn play_checkpoint(&self)->Option<mt_engine::play::Checkpoint> {self.play.play_checkpoint()}
    fn resume_play_checkpoint(&mut self,s:&mt_engine::play::Checkpoint)->Result<(),String> {self.play.resume_play_checkpoint(s)}
    fn anchor(&mut self,name:&str,image:&[u8]) {
        if name=="s.4FCB" || self.anchors && boot::anchor_ready(image,self.observed.borrow().loaded) {
            if let Ok(mut saved)=mt1::save::Saved::decode(image) {
                if let Some(meta)=saved.play.as_mut() {
                    let observed=self.observed.borrow();
                    let extensions=if observed.extensions.is_empty() {Default::default()} else {
                        serde_json::from_slice(&observed.extensions).expect("serialized scenario continuation")
                    };
                    meta.host=serde_json::to_vec(&HostState{extensions,story:observed.story.clone(),messages:observed.messages.clone()}).expect("serializable host continuation");
                }
                self.play.anchor(name,&saved.encode());
            }
        }
    }
    fn next_key(&mut self)->u16 {self.play.next_key()}
    fn key_pending(&mut self)->bool {self.play.key_pending()}
    fn poll(&mut self,c:&mut dyn mt_engine::sim::tick::Clock)->Event {self.play.poll(c)}
    fn intent(&mut self)->Option<Box<dyn Any>> {self.play.intent()}
    fn mouse(&mut self)->MouseState {self.play.mouse()}
    fn refuse_grant(&mut self) {self.play.refuse_grant()}
    fn set_mouse_position(&mut self,x:i16,y:i16) {self.play.set_mouse_position(x,y)}
    fn present(&mut self,rgb:&[u32]) {self.play.present(rgb)}
    fn scene(&mut self,s:&dyn Any) {
        let mut o=self.observed.borrow_mut();
        if let Some(s)=s.downcast_ref::<Observation>() {o.story.observe(&s.scene);o.state=Some(s.clone())}
        if let Some(mt_engine::io::dump::Text::Message(s))=s.downcast_ref::<mt_engine::io::dump::Text>() {
            if o.messages.len()==500 {o.messages.remove(0);} o.messages.push(s.clone());
        }
    }
}
#[derive(Debug)]
enum Control { Restart(Root, Option<Snapshot>), Eof, Io(String) }
fn leave(control:Control)->! {std::panic::resume_unwind(Box::new(control))}
struct Pending {
    id:Value, batch:bool, picks:VecDeque<Value>, stop:Value, limit:usize, applied:usize,
    initial:Value, previous:Value, events:Vec<Value>, journal_at:usize,
}
impl Pending {
    fn start(id:Value)->Self {Self{id,batch:false,picks:VecDeque::new(),stop:Value::Null,limit:0,applied:0,initial:Value::Null,previous:Value::Null,events:vec![],journal_at:0}}
    fn reason(&self,d:&Decision,s:&Value)->Option<&'static str> {
        if !self.batch {return Some("complete")}
        if self.stop["combat"].as_bool().unwrap_or(self.stop["encounter"].as_bool().unwrap_or(true)) && (d.kind==Kind::CombatTurn || state::fighting(s)) {return Some("combat")}
        if self.stop["kind"].as_str()==Some(d.kind.as_str()) {return Some("kind")}
        if self.stop["system"]==true && s["position"]["system"]!=self.initial["position"]["system"] {return Some("system")}
        if self.stop["area"]==true && state::area(s)!=state::area(&self.initial) {return Some("area")}
        if self.stop["program"]==true && s["program"]!=self.initial["program"] {return Some("program")}
        if self.stop["message"]==true && self.events.iter().any(|e|e["type"]=="message"||e["type"]=="messages") {return Some("message")}
        if self.applied>=self.limit {return Some("count")}
        if self.picks.is_empty() {return Some("exhausted")}
        None
    }
}
struct Host {
    reader:Box<dyn BufRead>, writer:Box<dyn Write>, dir:PathBuf, exe_dir:PathBuf, snapshots:PathBuf,
    initialized:bool, observed:Rc<RefCell<Observed>>, root:Option<Root>, pending:Option<Pending>,
}
impl Host {
    fn send(&mut self,id:Value,result:Value) {self.write(json!({"jsonrpc":"2.0","id":id,"result":result}))}
    fn error(&mut self,id:Value,code:i32,message:&str) {self.write(json!({"jsonrpc":"2.0","id":id,"error":{"code":code,"message":message}}))}
    fn write(&mut self,v:Value) {
        let result=(||->io::Result<()>{serde_json::to_writer(&mut self.writer,&v)?;self.writer.write_all(b"\n")?;self.writer.flush()})();
        if let Err(e)=result {leave(Control::Io(e.to_string()))}
    }
    fn result(&mut self,id:Value,v:Value,is_error:bool) {self.send(id,json!({"content":[{"type":"text","text":v.to_string()}],"isError":is_error}))}
    fn read(&mut self)->Value {
        loop {
            let mut bytes=vec![];
            let n=Read::by_ref(&mut self.reader).take((1<<20)+1).read_until(b'\n',&mut bytes).unwrap_or_else(|e|leave(Control::Io(e.to_string())));
            if n==0 {leave(Control::Eof)}
            if n>1<<20 {self.error(Value::Null,-32600,"request exceeds 1 MiB");leave(Control::Eof)}
            if bytes.iter().all(u8::is_ascii_whitespace) {continue}
            match serde_json::from_slice(&bytes) {Ok(v)=>return v,Err(_)=>self.error(Value::Null,-32700,"parse error")}
        }
    }
    fn observation(&self,p:&Play)->Value {state::value(p.pending().expect("service at decision"),&self.observed.borrow())}
    fn envelope(&self,p:&Play)->Value {json!({"decision":p.pending().map(public_decision),"state":self.observation(p)})}
    fn advance(&mut self,p:&mut Play)->bool {
        let Some(mut pending)=self.pending.take() else{return false};
        let current=self.observation(p);
        if !pending.previous.is_null() {pending.events.extend(state::events(&pending.previous,&current))}
        for message in self.observed.borrow_mut().messages.drain(..) {pending.events.push(json!({"type":"message","text":message}));}
        pending.previous=current.clone();
        let d=p.pending().unwrap();
        if let Some(reason)=pending.reason(d,&current) {
            let mut result=self.envelope(p); result["events"]=json!(pending.events);
            result["grants"]=json!(p.journal().entries.iter().skip(pending.journal_at).map(|e|json!({"decision":e.choice.decision,"requested":e.grant.requested,"used":e.grant.used,"remaining":e.grant.remaining})).collect::<Vec<_>>());
            if pending.batch {result["applied"]=json!(pending.applied);result["reason"]=json!(reason)}
            self.result(pending.id,result,false);return false
        }
        let value=pending.picks.pop_front().unwrap();
        match pick(&value,d).and_then(|pick|p.choose(Choice::new(p.pending().unwrap().id.clone(),pick))) {
            Ok(())=>{pending.applied+=1;self.pending=Some(pending);true}
            Err(e)=>{self.result(pending.id,json!({"error":e,"applied":pending.applied,"reason":"refused"}),true);false}
        }
    }
    fn service(&mut self,p:&mut Play) {
        if self.advance(p) {return}
        loop {if self.request(Some(p)) {return}}
    }
    fn request(&mut self,mut play:Option<&mut Play>)->bool {
        let request=self.read();
        let id=request.get("id").cloned().unwrap_or(Value::Null);
        let valid_id=id.is_string()||id.is_i64()||id.is_u64();
        if !request.is_object() || request["jsonrpc"]!="2.0" || !request["method"].is_string() || (request.get("id").is_some()&&!valid_id) {
            self.error(Value::Null,-32600,"invalid JSON-RPC request");return false
        }
        if request.get("id").is_none() {return false}
        let params=request.get("params").cloned().unwrap_or_else(||json!({}));
        if !params.is_object() {self.error(id,-32602,"params must be an object");return false}
        match request["method"].as_str().unwrap() {
            "initialize"=>{
                if self.initialized {self.error(id,-32600,"already initialized");return false}
                let requested=params["protocolVersion"].as_str().unwrap_or("");
                let version=match requested {"2024-11-05"|"2025-03-26"|"2025-06-18"=>requested,_=>"2025-06-18"};
                self.initialized=true;self.send(id,json!({"protocolVersion":version,"capabilities":{"tools":{},"resources":{}},
                    "serverInfo":{"name":"mt1","version":env!("CARGO_PKG_VERSION")},"instructions":SKILL}));
            }
            "ping"=>self.send(id,json!({})),
            _ if !self.initialized=>self.error(id,-32000,"initialize first"),
            "tools/list"=>self.send(id,json!({"tools":wire::tools()})),
            "resources/list"=>self.send(id,json!({"resources":[{"uri":URI,"name":"MT1 play skill","mimeType":"text/markdown","description":"Read before the first gameplay decision."}]})),
            "resources/read"=>if params["uri"]==URI {self.send(id,json!({"contents":[{"uri":URI,"mimeType":"text/markdown","text":SKILL}]}))} else {self.error(id,-32602,"unknown resource")},
            "tools/call"=>{
                let specs=wire::tools();
                let Some(spec)=specs.iter().find(|s|s["name"]==params["name"]) else {self.error(id,-32602,"unknown tool");return false};
                let args=params.get("arguments").cloned().unwrap_or_else(||json!({}));
                if !wire::valid(&args,&spec["inputSchema"]) {self.error(id,-32602,"invalid tool arguments");return false}
                let name=params["name"].as_str().unwrap();
                match self.call(name,&args,id.clone(),play.as_deref_mut()) {
                    Ok(Some(result))=>if !result.is_null(){self.result(id,result,false)},Ok(None)=>return true,
                    Err(error)=>self.result(id,json!({"error":error}),true),
                }
            }
            _=>self.error(id,-32601,"unknown method"),
        } false
    }
    fn scenario(&self,start:&str)->Result<String,String> {
        let file=PathBuf::from(start);
        let file=if file.is_file(){file}else{
            let filename=if start.ends_with(".toml"){start.to_string()}else{format!("{start}.toml")};
            [self.exe_dir.join("scenarios").join(&filename),PathBuf::from("scenarios").join(&filename)].into_iter().find(|p|p.is_file()).ok_or("scenario not found")?
        };
        Ok(format!("scenario:{}",file.canonicalize().map_err(|e|e.to_string())?.display()))
    }
    fn call(&mut self,name:&str,a:&Value,id:Value,play:Option<&mut Play>)->Result<Option<Value>,String> {
        if matches!(name,"game_new"|"game_load"|"snapshot_restore") {
            let (root,snapshot)=if name=="snapshot_restore" || name=="game_load" && a.get("snapshot").is_some() {
                let key=if name=="snapshot_restore"{"id"}else{"snapshot"};let s=crate::play_store::load_for_build(&self.snapshots,a[key].as_str().unwrap(),env!("MT_BUILD_HASH"))?;(s.root.clone(),Some(s))
            } else if name=="game_load" && a.get("path").is_some() {
                let path=Path::new(a["path"].as_str().unwrap());
                if path.extension().is_some_and(|x|x=="json") {
                    let directory=self.snapshots.canonicalize().map_err(|e|e.to_string())?;
                    let path=path.canonicalize().map_err(|e|e.to_string())?;
                    if path.parent()!=Some(directory.as_path()) {return Err("snapshot JSON must be in the snapshot directory; use a local snapshot id".into())}
                    let s=crate::play_store::load_for_build(&directory,path.file_stem().and_then(|s|s.to_str()).ok_or("invalid snapshot path")?,env!("MT_BUILD_HASH"))?;(s.root.clone(),Some(s))
                } else {
                    let mut image=vec![];std::fs::File::open(path).map_err(|e|e.to_string())?.take((16<<20)+1).read_to_end(&mut image).map_err(|e|e.to_string())?;
                    if image.len()>16<<20 {return Err("save exceeds 16 MiB".into())}
                    let saved=mt1::save::Saved::decode(&image)?;
                    (Root{kind:RootKind::Save,image:Some(image),start_tick:saved.ticks,scenario:None,gamefiles:boot::fingerprint(&self.dir)?},None)
                }
            } else {
                let start=if name=="game_load" {self.scenario(a["scenario"].as_str().unwrap())?} else {
                    match a["start"].as_str().unwrap_or("quickstart") {s @ ("quickstart"|"chargen"|"boot")=>s.to_string(),s=>self.scenario(s)?}
                };
                (Root{kind:RootKind::NewGame,image:None,start_tick:a["tick"].as_u64().unwrap_or(1234) as u32,scenario:Some(start),gamefiles:boot::fingerprint(&self.dir)?},None)
            };
            if let Some(s)=&snapshot {crate::play_store::compatible_build(s,env!("MT_BUILD_HASH"))?;}
            boot::validate(&root,&self.dir)?;
            self.pending=Some(Pending::start(id));leave(Control::Restart(root,snapshot))
        }
        if name=="snapshot_list" {return Ok(Some(json!({"snapshots":crate::play_store::list(&self.snapshots)?.iter().map(snapshot_info).collect::<Vec<_>>()})))}
        let p=play.ok_or("no active game; call game_new or game_load")?;
        match name {
            "decision"=>Ok(Some(json!({"decision":p.pending().map(public_decision)}))),
            "state"=>Ok(Some(self.observation(p))),
            "journal"=>Ok(Some(state::story(p.pending().unwrap(),&self.observed.borrow()))),
            "help"=>Ok(Some(state::help(p.pending().unwrap(),&self.observed.borrow())?)),
            "trace"=>Ok(Some(json!({"journal":journal(p.journal()),"at":{"decision":p.pending().unwrap().id,"state":p.pending().unwrap().state},
                "root":{"start_tick":self.root.as_ref().unwrap().start_tick,"scenario":self.root.as_ref().unwrap().scenario,"gamefiles":self.root.as_ref().unwrap().gamefiles}}))),
            "view"=>{
                let data=frame::base64(&frame::png(p.frame())?);
                self.send(id,json!({"content":[{"type":"image","mimeType":"image/png","data":data}],"isError":false}));
                Ok(Some(Value::Null))
            }
            "snapshot_save"=>{
                let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e|e.to_string())?;
                let key=format!("mt1-{}-{}",std::process::id(),now.as_nanos());
                let s=p.snapshot(self.root.as_ref().unwrap(),&key,a["name"].as_str().unwrap_or("Snapshot"),&format!("{}.{:09} UTC Unix",now.as_secs(),now.subsec_nanos()),env!("MT_BUILD_HASH"))?;
                crate::play_store::save(&self.snapshots,&s)?;Ok(Some(snapshot_info(&s)))
            }
            "choose"|"choose_until"=>{
                let expected=a["decision"].as_str().unwrap();let decision=p.pending().unwrap();
                if expected!=decision.id {return Err(format!("play: stale decision {expected}; expected {}",decision.id))}
                let current=self.observation(p);let batch=name=="choose_until";
                let mut pending=Pending{id,batch,picks:VecDeque::new(),stop:a.get("stop").cloned().unwrap_or_else(||json!({})),limit:1,applied:0,
                    initial:current.clone(),previous:current,events:vec![],journal_at:p.journal().entries.len()};
                if batch {
                    pending.picks=a["picks"].as_array().unwrap().iter().cloned().collect();
                    pending.limit=a["stop"]["count"].as_u64().map_or(pending.picks.len(),|n|n as usize);
                    self.pending=Some(pending);
                    if self.advance(p) {Ok(None)} else {Ok(Some(Value::Null))}
                } else {
                    let value=pick(&a["pick"],p.pending().unwrap())?;
                    p.choose(Choice::new(a["decision"].as_str().unwrap(),value))?;
                    pending.applied=1;self.pending=Some(pending);Ok(None)
                }
            }
            _=>Err("unknown tool".into()),
        }
    }
}
fn snapshot_info(s:&Snapshot)->Value {json!({"id":s.id,"name":s.name,"created":s.created,"build":s.build,"at":{"decision":format!("d:{}",s.sequence),"state":s.hash}})}

pub fn serve(dir:PathBuf,exe_dir:PathBuf)->Result<(),String> {
    let host=Rc::new(RefCell::new(Host{reader:Box::new(io::BufReader::new(io::stdin())),writer:Box::new(io::stdout()),dir:dir.clone(),exe_dir:exe_dir.clone(),
        snapshots:PathBuf::from("acceptance/local/play/snapshots"),initialized:false,observed:Rc::default(),root:None,pending:None}));
    let mut next:Option<(Root,Option<Snapshot>)>=None;
    loop {
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||->Result<(),String>{
            if let Some((root,snapshot))=next.take() {
                let h=host.clone();let observed={let mut h=host.borrow_mut();h.root=Some(root.clone());*h.observed.borrow_mut()=Observed::default();h.observed.clone()};
                let mut play=Play::with_service(move |p|h.borrow_mut().service(p));
                if let Some(snapshot)=snapshot {play.restore(snapshot)?}
                let anchors=true;
                boot::run(&root,&dir,&exe_dir,Box::new(Answerer{play,observed:observed.clone(),anchors,heartbeat:None}),observed)?;
                let mut h=host.borrow_mut();if let Some(p)=h.pending.take(){h.result(p.id,json!({"decision":null,"ended":true,"applied":p.applied,"events":p.events}),false)}h.root=None;
            }
            loop {host.borrow_mut().request(None);}
        }));
        let error=match result {
            Ok(Err(e))=>e,Ok(Ok(()))=>unreachable!(),
            Err(panic)=>match panic.downcast::<Control>() {
                Ok(c)=>match *c {Control::Restart(root,snapshot)=>{next=Some((root,snapshot));continue},Control::Eof=>return Ok(()),Control::Io(e)=>return Err(e)},
                Err(panic)=>match panic.downcast::<SessionExit>() {Ok(e)=>format!("play session: {e:?}"),Err(p)=>std::panic::resume_unwind(p)},
            },
        };
        let mut h=host.borrow_mut();h.root=None;
        if let Some(p)=h.pending.take(){h.result(p.id,json!({"error":error}),true)}else{eprintln!("{error}")}
    }
}

#[cfg(test)]
#[path="../tests/restore_differential/mod.rs"]
mod restore_differential;
