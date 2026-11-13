use mt1::space::{mouse, Space};
use mt_engine::io::Dir;
use mt_engine::sim::tick::Counter;
use std::path::PathBuf;
#[test]
#[ignore = "dump"]
fn dump_flight_tables() {
    let files = PathBuf::from(std::env::var_os("MT1_GAMEFILES").unwrap());
    let sp = Space::new(&std::fs::read(files.join("S.DAT")).unwrap(), vec![0; 0xF86], vec![], Box::new(mt_engine::io::Scripted::new([])), Box::new(Counter::at(1)), Box::new(Dir(files))).unwrap();
    for i in 0..10u16 { let at = 0xCB3 + 9 * i; println!("label {i}: y {} x {} text {:?} colour {}", sp.s.word(at), sp.s.word(at + 2), sp.s.pointer_string(at + 4).map(|v| String::from_utf8_lossy(&v).into_owned()), sp.s.byte(at + 8)); }
    for (name, t) in [("flight", mouse::FLIGHT_HITS), ("crew", mouse::CREW_HITS), ("pilot", mouse::PILOT_HITS), ("computer", mouse::COMPUTER_HITS), ("engineer", mouse::ENGINEER_HITS), ("comms", mouse::COMMS_HITS), ("medical", mouse::MEDICAL_HITS), ("gun", mouse::GUN_HITS), ("missile", mouse::MISSILE_HITS), ("boarding", mouse::BOARDING_HITS)] {
        for h in mouse::table(&sp.s, t) { println!("{name} hit: top {} left {} height {} width {} id {:#x}", h.top, h.left, h.height, h.width, h.id); }
    }
    println!("strings: 1779 {:?} 177E {:?}", String::from_utf8_lossy(&sp.s.string(0x1779)), String::from_utf8_lossy(&sp.s.string(0x177E)));
}
