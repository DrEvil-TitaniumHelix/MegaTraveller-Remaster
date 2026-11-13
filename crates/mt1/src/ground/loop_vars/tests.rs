use crate::{ground::{main_loop, Ground, Tooling, state::Mem}, save::{self, Mode, Saved}};
use mt_engine::{io::{Event, Input, Overlay}, sim::tick::{Clock, Counter}};
use std::{cell::RefCell, rc::Rc, panic::{catch_unwind, AssertUnwindSafe}};

struct Ticks;
impl Input for Ticks {
    fn next_key(&mut self) -> u16 { panic!("synthetic loop must never open a modal wait") }
    fn poll(&mut self, clock: &mut dyn Clock) -> Event { clock.wait_tick(); Event::Idle }
}
#[derive(Debug)] struct Stop;
struct Probe { at: usize, out: Rc<RefCell<Vec<Saved>>> }
impl Tooling for Probe {
    fn poll(&mut self, g: &mut Ground) -> bool {
        self.at += 1;
        if self.at == 10 || self.at == 50 { self.out.borrow_mut().push(Saved::capture(g, Mode::Ground, "test")); }
        if self.at == 50 { std::panic::resume_unwind(Box::new(Stop)); }
        false
    }
}
#[test]
fn exact_loop_top_resume_continues_like_the_uninterrupted_loop() {
    let mut a = Ground::from_ds(vec![0; 65536], vec![0; Mem::PARTY_SIZE], Box::new(Ticks), Box::new(Counter::at(100)), Box::new(Overlay::new(None)));
    a.mem.set_party_u8(0x127, 1); a.mem.set_countdown(100); a.mem.set_atmosphere(6); a.mem.set_event(-1);
    a.mem.set_key(0x0F);
    let plain = Rc::new(RefCell::new(vec![])); a.tooling = Some(Box::new(Probe { at: 0, out: plain.clone() }));
    let stop = catch_unwind(AssertUnwindSafe(|| main_loop::run(&mut a))).unwrap_err(); assert!(stop.is::<Stop>());
    let image = Saved::decode(&plain.borrow()[0].encode()).unwrap();
    let mut b = save::ground_exact(image.clone(), Box::new(Ticks), Box::new(Counter::at(image.ticks)), Box::new(Overlay::new(None))).unwrap();
    let resumed = Rc::new(RefCell::new(vec![])); b.tooling = Some(Box::new(Probe { at: 9, out: resumed.clone() }));
    let stop = catch_unwind(AssertUnwindSafe(|| main_loop::resume_exact(&mut b).unwrap())).unwrap_err(); assert!(stop.is::<Stop>());
    assert_eq!(*plain.borrow(), *resumed.borrow());
    assert!(plain.borrow()[1].loop_vars.as_ref().unwrap().npc.deadline > image.loop_vars.as_ref().unwrap().npc.deadline, "NPC deadline actually fired after the anchor");
    let mut c = save::ground_exact(image.clone(), Box::new(Ticks), Box::new(Counter::at(image.ticks)), Box::new(Overlay::new(None))).unwrap();
    let restarted = Rc::new(RefCell::new(vec![])); c.tooling = Some(Box::new(Probe { at: 9, out: restarted.clone() }));
    let stop = catch_unwind(AssertUnwindSafe(|| main_loop::run(&mut c))).unwrap_err(); assert!(stop.is::<Stop>());
    assert_ne!(plain.borrow()[0].loop_vars, restarted.borrow()[0].loop_vars);
}
