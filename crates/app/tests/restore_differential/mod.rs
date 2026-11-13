use super::*;
use super::restore_tests::{assets,root,scenario_root};

struct Rng(u32);
impl Rng { fn n(&mut self,m:usize)->usize {self.0=self.0.wrapping_mul(1103515245).wrapping_add(12345);((self.0>>8) as usize)%m.max(1)} }
fn choose_pick(d:&Decision,r:&mut Rng,script:&mut VecDeque<String>)->Vec<Value> {
    let mut out=vec![];
    while let Some(s)=script.pop_front() { if d.options.iter().any(|o|o.enabled&&o.id==s) {out.push(json!(s));break} }
    let bad=|s:&str|{let s=s.to_lowercase();s.contains("quit")||s.contains("save")||s.contains("restore")||s.contains("load")||s.contains("dos")};
    let opts:Vec<&str>=d.options.iter().filter(|o|o.enabled&&!bad(&o.id)&&!bad(&o.label)).map(|o|o.id.as_str()).collect();
    if let Some(t)=&d.time { let ticks=[1u32,3,9,20][r.n(4)].min(t.max_ticks);
        if r.n(3)==0 {out.push(json!(format!("wait:{ticks}")))}
        else if r.n(3)==0 && !t.directions.is_empty() {let k:Vec<_>=t.directions.keys().collect();out.push(json!(format!("move:{}:{ticks}",k[r.n(k.len())])))} }
    if d.kind==Kind::CombatTurn { if let Some(m)=&d.movement { if !m.cells.is_empty() && r.n(2)==0 {let c=m.cells[r.n(m.cells.len())];out.push(json!(format!("move:cell/{},{}",c.0,c.1)))} } if r.n(3)==0 {out.push(json!("end_turn"))} }
    if !opts.is_empty() {let i=r.n(opts.len());out.push(json!(opts[i]))}
    if let Some(l)=&d.list { for row in l.rows.iter().filter(|x|x.enabled) {out.push(json!({"row":row.id,"qty":row.min_qty.max(row.qty)}))} }
    if d.input.is_some() {out.push(json!("A"));out.push(json!(1))}
    if let Some(x)=&d.default {out.push(json!(x))}
    out.push(json!("back"));out.push(json!("end_turn"));out.push(json!("wait:1"));
    out
}
#[derive(Clone)] struct Step {id:String,kind:String,state:String,frame:String,public:String,anchor:Option<(u64,Vec<u8>)>,pick:Value,snap:Option<Snapshot>}
fn fnvb(b:&[u8])->u64 {b.iter().fold(0xcbf29ce484222325u64,|h,&x|(h^x as u64).wrapping_mul(0x100000001b3))}
fn observe(p:&Play,d:&Decision)->(String,Option<(u64,Vec<u8>)>) {
    let mut pub_=public_decision(d); if let Some(o)=pub_.as_object_mut(){o.remove("state");}
    (pub_.to_string(),p.latest_anchor().map(|a|(a.sequence,a.image.clone())))
}
fn trace_run(dir:&Path,start:&dyn Fn(Box<dyn Input>,Rc<RefCell<Observed>>),base:&Root,n:usize,seed:u32,script:&[&str],every:usize)->Vec<Step> {
    let out:Rc<RefCell<Vec<Step>>>=Rc::default();let o=out.clone();let base=base.clone();
    let mut r=Rng(seed);let mut script:VecDeque<String>=script.iter().map(|s|s.to_string()).collect();
    let p=Play::with_service(move|p| {
        if o.borrow().len()>=n {SessionExit::AwaitingDecision.leave()}
        let d=p.pending().unwrap().clone();
        let (public,anchor)=observe(p,&d);
        let snap=(o.borrow().len()%every==0).then(||p.snapshot(&base,"diff","Diff","now",env!("MT_BUILD_HASH")).unwrap());
        let frame=launch::frame_hash(p.frame());
        for v in choose_pick(&d,&mut r,&mut script) {
            let Ok(pk)=pick(&v,&d) else {continue};
            if p.choose(Choice::new(d.id.clone(),pk)).is_ok() {
                o.borrow_mut().push(Step{id:d.id.clone(),kind:d.kind.as_str().into(),state:d.state.clone(),frame,public,anchor,pick:v,snap});return
            }
        }
        eprintln!("DIFF-TRACE no legal pick at {} {:?}",d.id,d.kind);SessionExit::AwaitingDecision.leave();
    });
    let observed:Rc<RefCell<Observed>>=Rc::default();
    let input:Box<dyn Input>=Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None});
    let _=dir;
    let res=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||start(input,observed)));
    if let Err(e)=res { if !matches!(e.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)) {eprintln!("DIFF-TRACE exit {:?}",e.downcast_ref::<SessionExit>());} }
    else {eprintln!("DIFF-TRACE game ended after {}",out.borrow().len());}
    let r=out.borrow().clone();assert_eq!(r.len(),n,"recording ended early");r
}
fn verify(name:&str,dir:&Path,trace:&[Step],at:usize,reach:usize)->usize {
    mt_engine::present::textlog::set_enabled(false);mt_engine::present::textlog::set_enabled(true);
    let snap=trace[at].snap.clone().unwrap();let root=snap.root.clone();
    let tail=snap.journal.entries.len();
    let diffs:Rc<RefCell<usize>>=Rc::default();let d2=diffs.clone();
    let t:Vec<Step>=trace[at..].iter().take(reach+1).cloned().collect();let mut idx=0usize;let nm=name.to_string();
    let seen=Rc::new(std::cell::Cell::new(0));let seen2=seen.clone();let expected=t.len();
    let mut p=Play::with_service(move|p| {
        if idx>=t.len() {SessionExit::AwaitingDecision.leave()}
        let d=p.pending().unwrap().clone();let s=&t[idx];
        let (public,anchor)=observe(p,&d);let frame=launch::frame_hash(p.frame());
        let report=|what:&str,a:String,b:String|{*d2.borrow_mut()+=1;eprintln!("DIFF {nm} restore@{} tail={tail} step+{idx} {} {} field={what}\n   orig={}\n   rest={}",t[0].id,s.id,s.kind,&a[..a.len().min(400)],&b[..b.len().min(400)]);};
        if d.id!=s.id||d.state!=s.state {report("state",s.state.clone(),d.state.clone());}
        if public!=s.public {let i=public.bytes().zip(s.public.bytes()).position(|(a,b)|a!=b).unwrap_or(0);report("public",s.public[i.min(s.public.len())..].to_string(),public[i.min(public.len())..].to_string());}
        if frame!=s.frame {report("frame",s.frame.clone(),frame.clone());}
        if idx>0 {if anchor!=s.anchor {report("anchor",format!("{:?}",s.anchor.as_ref().map(|(n,b)|(n,fnvb(b)))),format!("{:?}",anchor.as_ref().map(|(n,b)|(n,fnvb(b)))));}}
        let pk=pick(&s.pick,&d).unwrap();
        if let Err(e)=p.choose(Choice::new(d.id.clone(),pk)) {report("choose",s.pick.to_string(),e);SessionExit::AwaitingDecision.leave()}
        idx+=1;seen2.set(idx);
    });
    p.restore(snap).unwrap();let observed:Rc<RefCell<Observed>>=Rc::default();
    let dir=dir.to_path_buf();
    let e=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||boot::run(&root,&dir,Path::new("."),Box::new(Answerer{play:p,observed:observed.clone(),anchors:true,heartbeat:None}),observed)));
    match e {
        Ok(r)=>{eprintln!("DIFF {name} restore@{at} game ended in restored run {r:?}");*diffs.borrow_mut()+=1;}
        Err(x)=>{ if !matches!(x.downcast_ref::<SessionExit>(),Some(SessionExit::AwaitingDecision)) {eprintln!("DIFF {name} restore@{at} exit {:?} {:?}",x.downcast_ref::<SessionExit>(),x.downcast_ref::<String>());*diffs.borrow_mut()+=1;} }
    }
    assert_eq!(seen.get(),expected,"restored continuation ended early");
    let n=*diffs.borrow();n
}
fn run_diff(name:&str,dir:&Path,base:&Root,start:&dyn Fn(Box<dyn Input>,Rc<RefCell<Observed>>),n:usize,seed:u32,script:&[&str],every:usize,reach:usize) {
    mt_engine::present::textlog::set_enabled(false);mt_engine::present::textlog::set_enabled(true);
    let trace=trace_run(dir,start,base,n,seed,script,every);
    let kinds:std::collections::BTreeMap<String,usize>=trace.iter().fold(Default::default(),|mut m,s|{*m.entry(s.kind.clone()).or_insert(0)+=1;m});
    let (mut pts,mut diffs,mut anchored,mut maxtail)=(0,0,0,0);
    for at in (0..trace.len()).filter(|i|trace[*i].snap.is_some()) {
        let s=trace[at].snap.as_ref().unwrap();if s.root.kind==RootKind::Anchor {anchored+=1} maxtail=maxtail.max(s.journal.entries.len());
        pts+=1;diffs+=verify(name,dir,&trace,at,reach);
    }
    eprintln!("DIFF-SUMMARY {name} seed={seed} steps={} kinds={kinds:?} restore_points={pts} anchored={anchored} maxtail={maxtail} diffs={diffs}",trace.len());
    assert!(pts>0);assert_eq!(diffs,0,"{name} seed={seed}");
}

fn ground_case(name:&str,seed:u32,n:usize) {
    let dir=assets();let base=if name=="quickstart" {root(&dir)}else{scenario_root(&dir,name)};
    let (b,d)=(base.clone(),dir.clone());
    run_diff(name,&dir,&base,&move|input,observed|{boot::run(&b,&d,Path::new("."),input,observed).unwrap();},n,seed,&[],5,25);
}
fn space_case(seed:u32,n:usize) {
    let dir=assets();let base=root(&dir);let d=dir.clone();
    let start=move|input:Box<dyn Input>,observed:Rc<RefCell<Observed>>| {
        let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m6_efate_orbit_ready.toml");
        let prepared=crate::scenario::load(&path,&d).unwrap();let mut g=prepared.game;
        g.loop_vars.initialized=true;g.boot_calibrating=false;g.scene=Some(mt1::ui::SceneHook::default());
        g.squad=Some(mt1::ground::squad::State::with_compendium(mt1::ground::rules::compendium::Compendium::parse(include_str!("../../../../data/npcs.toml")).unwrap()));
        boot::attach(&mut g,&d,prepared.extensions,observed,true);g.input=input;
        if mt1::space::launch(&mut g).unwrap()==mt1::space::Exit::Land {mt1::ground::main_loop::run(&mut g)}
    };
    let script:&[&str]=if seed%2==1 {&["cmd:land"]}else{&["cmd:stations","back","cmd:land"]};
    run_diff("space",&dir,&base,&start,n,seed,script,4,20);
}
#[test]
fn restore_differential_normal_quickstart_and_space() {
    let start=std::time::Instant::now();ground_case("quickstart",1,30);space_case(1,24);
    assert!(start.elapsed().as_secs_f64()<300.,"two seeds took {:?}",start.elapsed());
}
#[test] #[ignore="reader seven-run table"] fn reader_quickstart_seed1(){ground_case("quickstart",1,120)}
#[test] #[ignore="reader seven-run table"] fn reader_efate_broken_seed1(){ground_case("efate_broken",1,120)}
#[test] #[ignore="reader seven-run table"] fn reader_efate_alerted_seed2(){ground_case("efate_alerted",2,70)}
#[test] #[ignore="reader seven-run table"] fn reader_efate_door_seed2(){ground_case("efate_door",2,70)}
#[test] #[ignore="reader seven-run table"] fn reader_space_seed1(){space_case(1,60)}
#[test] #[ignore="reader seven-run table"] fn reader_space_seed2(){space_case(2,60)}
#[test] #[ignore="reader seven-run table"] fn reader_space_seed3(){space_case(3,50)}
