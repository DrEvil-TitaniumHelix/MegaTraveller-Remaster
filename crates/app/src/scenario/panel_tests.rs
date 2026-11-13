mod oracle_env { include!("../../../mt1/tests/oracle_env/mod.rs"); }
use super::*;
use megatraveller::panels::{assets::Assets as PanelAssets, controls, minimap::Minimap, viewport::{Camera, Viewport}, Component, Surface, UiState};
use mt1::ui::{Scene, ScreenKind};
use mt_engine::io::{Input, Key};
use std::sync::Arc;

#[derive(Default)]
struct Observed { scene: Option<Scene>, ui: UiState }
struct Capture(Rc<RefCell<Observed>>);
impl Input for Capture {
    fn next_key(&mut self)->Key {27}
    fn present(&mut self,rgb:&[u32]) {*self.0.borrow_mut().ui.frame=megatraveller::pipeline::Frame::from_rgb(rgb)}
    fn scene(&mut self,s:&dyn std::any::Any) {
        let scene=s.downcast_ref::<Scene>().unwrap();
        let mut observed=self.0.borrow_mut();
        let old=observed.scene.clone();
        controls::observe(scene,old.as_ref(),&mut observed.ui);
        observed.scene=Some(scene.clone());
    }
}

struct Boundary;
struct StopAtInput(Capture);
impl Input for StopAtInput {
    fn next_key(&mut self)->Key {std::panic::resume_unwind(Box::new(Boundary))}
    fn poll(&mut self,_:&mut dyn Clock)->mt_engine::io::Event {std::panic::resume_unwind(Box::new(Boundary))}
    fn present(&mut self,rgb:&[u32]) {self.0.present(rgb)}
    fn scene(&mut self,s:&dyn std::any::Any) {self.0.scene(s)}
}

#[test]
fn starport_office_and_police_use_the_full_picture_at_input() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let path=Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scenarios/m5_starport_desk_efate.toml");
    for kind in ["starport","office","police"] {
        let observed=Rc::new(RefCell::new(Observed::default()));
        let mut g=load(&path,&dir).unwrap().game;
        g.input=Box::new(StopAtInput(Capture(observed.clone())));attach_scene(&mut g);
        let result=std::panic::catch_unwind(std::panic::AssertUnwindSafe(||match kind {
            "starport"=>ground::starport::enter(&mut g),
            "office"=>{g.mem.set_event(13);ground::starport::office(&mut g);},
            _=>{g.mem.set_outdoors(0);g.mem.set_u8(0x5E1A,12);ground::starport::police::weapons_search(&mut g);},
        }));
        assert!(result.is_err_and(|e|e.is::<Boundary>()),"{kind}: did not reach input");
        let seen=observed.borrow();let scene=seen.scene.as_ref().unwrap();
        assert_eq!(scene.screen,ScreenKind::Picture,"{kind}");
        let mut layout=megatraveller::panels::Layout::default();
        megatraveller::panels::components::register(&mut layout,Arc::default());
        layout.arrange_for(scene,&seen.ui,(320,200));
        let parts=layout.draw(scene,&seen.ui);assert_eq!(parts.len(),1);
        assert_eq!(parts[0].2.indexed,seen.ui.frame.indexed);
    }
}

#[test]
fn product_scenarios_boot_and_swap_render_the_placed_cell() {
    let Some(dir)=oracle_env::gamefiles() else {return};
    let root=Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let assets=Arc::new(PanelAssets::load(&mut Dir(dir.clone())));
    let viewport=Viewport::new(assets.clone());let minimap=Minimap::new(assets);
    let mut paths:Vec<_>=fs::read_dir(root.join("scenarios")).unwrap().map(|e|e.unwrap().path())
        .filter(|p|p.file_name().unwrap().to_str().is_some_and(|s|s.starts_with("m4b_")||s.starts_with("m5_"))).collect();
    paths.sort();assert_eq!(paths.len(),7);
    let observed=Rc::new(RefCell::new(Observed::default()));
    let mut g=load(&paths[0],&dir).unwrap().game;
    g.input=Box::new(Capture(observed.clone()));attach_scene(&mut g);
    let request=Request::default();
    g.tooling=Some(Box::new(Loader::new(request.clone(),dir,Extensions::default())));
    for boot in [true,false] {for path in &paths {
        if boot {
            let dir=std::env::var_os("MT1_GAMEFILES").map(PathBuf::from).unwrap();
            let mut prepared=load(path,&dir).unwrap();
            std::mem::swap(&mut prepared.game.input,&mut g.input);
            std::mem::swap(&mut prepared.game.tooling,&mut g.tooling);
            g=prepared.game;attach_scene(&mut g);
        } else {
            observed.borrow_mut().ui.follow=None;
            observed.borrow_mut().ui.camera=(-900.0,-900.0);
            *request.borrow_mut()=Some(path.clone());assert!(g.tool_poll());
        }
        assert!(g.scene.is_some());
        let memory=g.mem.ds.clone();let party=g.mem.party().to_vec();
        g.frame("scenario_panel_test");
        assert_eq!(g.mem.ds,memory);assert_eq!(g.mem.party(),party);
        let source=validate::document(&fs::read_to_string(path).unwrap()).unwrap();
        let row=source["place"]["cell"][0].as_i64().unwrap() as i16;
        let col=source["place"]["cell"][1].as_i64().unwrap() as i16;
        let seen=observed.borrow();let scene=seen.scene.as_ref().unwrap();
        assert_eq!(scene.screen,ScreenKind::Ground);
        assert_eq!(scene.pos,(row*2,col*2));assert_eq!(scene.level as i64,source["place"]["level"].as_i64().unwrap());
        assert!(scene.map.cell(row as i32,col as i32).is_some());
        assert_eq!(seen.ui.camera,(row as f32+0.5,col as f32+0.5));
        let camera=Camera::new(scene,&seen.ui,(192,112));
        assert_eq!(camera.world((96,56)),(row as f32*16.0+8.0,col as f32*16.0+8.0));
        let mut live=Surface::new(192,112);viewport.draw(scene,&seen.ui,&mut live);
        assert!(live.indexed.iter().any(|&p|p!=0));
        let ui=UiState {camera:Viewport::oracle_camera(scene),..UiState::default()};
        let mut exact=Surface::new(192,112);viewport.draw(scene,&ui,&mut exact);
        let page=(g.mem.u16(0x52F4)&1) as usize;
        for y in 0..112 {for x in 0..192 {assert_eq!(exact.get(x,y),Some(g.fb.get(page,y+54,x)),"{} at {x},{y}",path.display());}}
        let mut map=Surface::new(scene.map.cols as u32*4,scene.map.rows as u32*4);
        minimap.draw(scene,&seen.ui,&mut map);
        let marker=map.get(col as i32*4+2,row as i32*4+2).unwrap();
        assert!((1..=5).contains(&marker),"{}: party marker {marker}",path.display());
        eprintln!("scenario panels {} {}: viewport exact, placed cell centred, minimap marker PASS",if boot {"boot"} else {"swap"},path.file_name().unwrap().to_string_lossy());
    }}
    let old=observed.borrow().scene.clone();let ds=g.mem.ds.clone();
    *request.borrow_mut()=Some(root.join("scenarios/missing.toml"));assert!(!g.tool_poll());
    assert_eq!(g.mem.ds,ds);assert_eq!(observed.borrow().scene,old);assert!(g.scene.is_some());
}
