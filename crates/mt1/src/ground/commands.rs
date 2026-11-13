use super::{
    input::{menu, ui},
    render,
    state::{Far, Ground, Mem},
    walk,
};

pub fn cell(g: &Ground, x: i16, y: i16) -> Far {
    let offset = x
        .wrapping_mul(g.mem.map_stride())
        .wrapping_add(y)
        .wrapping_mul(3);
    g.mem.pointer(0xBC).add(offset as i32)
}

#[cfg(test)]
mod tests {
    mod oracle_env { include!("../../tests/oracle_env/mod.rs"); }
    use super::*;
    use mt_engine::{
        io::{Dir, Input},
        sim::tick::Counter,
    };

    struct NoInput;
    impl Input for NoInput {
        fn next_key(&mut self) -> u16 {
            0
        }
    }

    #[test]
    fn extreme_map_coordinates_wrap_the_near_offset() {
        let mut g = Ground::from_ds(
            vec![],
            vec![0; Mem::PARTY_SIZE],
            Box::new(NoInput),
            Box::new(Counter::default()),
            Box::new(Dir(std::path::PathBuf::new())),
        );
        g.mem.set_pointer(0xBC, Far::new(0x1234, 0x8000));
        for edge in [i16::MAX, i16::MIN] {
            g.mem.set_map_stride(edge);
            assert_eq!(cell(&g, edge, edge), Far::new(0x9234, 0x8000));
        }
    }

    #[test]
    fn leaving_each_vehicle_parks_it_and_walking_back_boards_it() {
        use mt_engine::io::{Files, Scripted};
        let Some(dir) = oracle_env::gamefiles() else { return };
        let files = Dir(dir.clone());
        let ds = super::super::state::ds_image(&files.read("T.DAT").unwrap()).unwrap();
        for (vehicle, product) in (1..=3).flat_map(|v| [(v, false), (v, true)]) {
            let mut g = Ground::from_ds(ds.clone(), vec![0; Mem::PARTY_SIZE],
                Box::new(Scripted::new([13])), Box::new(Counter::default()), Box::new(Dir(dir.clone())));
            if product { g.scene = Some(crate::ui::SceneHook::default()) }
            let said = |g: &Ground| if g.mem.message_deadline() == 0 { String::new() } else { g.mem.cstr(Far::new(0, 0x8F00)) };
            g.mem.alloc(0x8000, 12 * 20 * 3);
            g.mem.set_pointer(0xBC, Far::new(0, 0x8000));
            g.mem.write_bytes(Far::new(0, 0x8000), &[1, 20, 0].repeat(12 * 20));
            g.mem.set_map_stride(20); g.mem.set_map_blocks_high(1);
            g.mem.set_u8(0x5C3B, 1);
            walk::set_player_pos(&mut g, 5, 6);
            g.mem.set_encounter_mode(vehicle); g.mem.set_saved_mode(vehicle);
            g.mem.set_vehicle_condition(100); g.mem.set_u8(0x6D31, 1);
            g.mem.set_heading(2); g.mem.set_orders(0); g.mem.set_event(-1);
            let parked = cell(&g, 5, 6).add(2);
            vehicle_menu(&mut g);
            assert_eq!(g.mem.read(parked, 1), 0x40, "vehicle {vehicle} must remain on its old square");
            assert_eq!((g.mem.u8(0x6419), g.mem.u8(0x63F4)), (5, 6));
            assert_eq!(g.mem.encounter_mode(), 0);
            assert_eq!(g.mem.u8(0x6D31), 1);
            assert_eq!(g.mem.vehicle_condition(), 100);
            assert_eq!(said(&g), if product { "You leave the vehicle." } else { "" }, "Leave says so in product, silent as 1990 in faithful");
            let delta = (5 - g.mem.x() / 2, 6 - g.mem.y() / 2);
            let heading = (0..9).map(|d| g.mem.direction(d))
                .find(|&(dx, dy, _)| (dx, dy) == delta).unwrap().2;
            g.mem.set_heading(heading);
            walk::move_party(&mut g, delta.0, delta.1, heading);
            assert_eq!(g.mem.encounter_mode(), vehicle, "walking onto the marker boards the same vehicle");
            assert_eq!(g.mem.read(parked, 1), 0, "boarding consumes only the parked marker");
            if product { assert_eq!(said(&g), "You board the vehicle.") }
        }
    }

    #[test]
    fn parking_remembers_byte_coordinates_and_preserves_other_occupants() {
        let mut g = Ground::from_ds(vec![], vec![0; Mem::PARTY_SIZE], Box::new(NoInput),
            Box::new(Counter::default()), Box::new(Dir(std::path::PathBuf::new())));
        g.mem.set_map_stride(20);
        g.mem.alloc(0x8000, 65536); g.mem.set_pointer(0xBC, Far::new(0, 0x8000));
        let p = cell(&g, 255, 6).add(2);
        for (before, claimed, after) in [(0x80, 1, 0xC0), (0x83, 0, 0x83), (0xC0, 1, 0xC0)] {
            g.mem.write(p, before, 1);
            assert_eq!(walk::claim_cell(&mut g, -1, 13), claimed);
            assert_eq!((g.mem.u8(0x6419), g.mem.u8(0x63F4)), (255, 6));
            assert_eq!(g.mem.read(p, 1), after);
        }
    }
}
pub fn facing_cell(g: &Ground) -> (Far, i16) {
    let direction = g.mem.u8((g.mem.heading() as u16).wrapping_add(0xD8)) as u16;
    let (dx, dy, _) = g.mem.direction(direction);
    let x = (g.mem.x() >> 1).wrapping_add(dx);
    let y = (g.mem.y() >> 1).wrapping_add(dy);
    (cell(g, x, y), y)
}
pub fn window(g: &mut Ground, row: u16, col: u16, height: u16, width: u16) {
    for (off, value) in [
        (0x717C, row),
        (0x7164, col),
        (0x7170, height),
        (0x7166, width),
    ] {
        g.mem.set_u16(off, value)
    }
}
pub fn world_uwp(g: &mut Ground) {
    let mut text = b"World UWP: XXXXXXXX".to_vec();
    text[11] = g.mem.u8(0x5E14).wrapping_add(b'A');
    for i in 1..8 {
        let mut ch = g.mem.u8(0x5E14 + i).wrapping_add(b'0');
        if ch >= b':' {
            ch = ch.wrapping_add(7)
        }
        text[11 + i as usize] = ch;
    }
    ui::message(g, &text);
}
pub fn debug_cheat(g: &mut Ground) {
    render::cursor_off(g);
    let rec = (g.mem.selected_char() as u16 * 256).wrapping_add(0x102);
    g.mem.set_party_u8(
        rec.wrapping_add(0x28),
        g.mem.party_u8(rec.wrapping_add(0x28)) | 1,
    );
    let cash = g.mem.pointer(0x6D65).add(0x88);
    g.mem.write(cash, 900000, 4);
    for off in [0x2E, 0x43, 0x64] {
        g.mem.set_party_u8(rec.wrapping_add(off), 4)
    }
    g.mem.set_party_u8(rec.wrapping_add(0x39), 8);
    g.mem.set_party_u8(
        rec.wrapping_add(0x40),
        g.mem.party_u8(rec.wrapping_add(0x40)).wrapping_add(1),
    );
    for off in [0x25, 0x23] {
        g.mem.set_party_u8(rec.wrapping_add(off), 100)
    }
    g.mem.set_cheat(g.mem.cheat() ^ 1);
    render::show_message(g, Mem::ds(0x12BA));
}
pub fn debug_teleport(g: &mut Ground) {
    render::show_message(g, Mem::ds(0x1216));
    let system = match g.get_scan() as i8 {
        11 => 1,
        n => n,
    };
    render::show_message(g, Mem::ds(0x1239));
    let planet = match g.get_scan() as i8 {
        11 => 1,
        n => n,
    };
    if !(1..7).contains(&planet) || !(1..=8).contains(&system) {
        render::show_message(g, Mem::ds(0x1268));
        return;
    }
    g.mem.set_party_u8(7, (planet - 1) as u8);
    g.mem.set_party_u8(6, (system - 1) as u8);
    for i in 0..5 {
        g.mem.set_party_u8(i * 256 + 0x128, 45)
    }
    clear_view(g);
    for i in 0..g.mem.ship_count() as u16 {
        let item = g.mem.i16(0x6C43 + 4 * i);
        let off = item.wrapping_sub(1).wrapping_mul(5).wrapping_add(0xA82) as u16;
        g.mem.set_party_u8(off, 255);
    }
    if !walk::setup_world(g) {
        return;
    }
    render::show_message(g, Mem::ds(0x125C));
    render::redraw(g);
    g.mem
        .set_party_u16(0xF84, g.mem.party_u16(0xF84).wrapping_add(7));
    g.mem.set_orders(0);
}
pub fn clear_view(g: &mut Ground) {
    g.fb.fill(
        mt_engine::present::prim::Raster::Planar,
        ui::page(g),
        54,
        0,
        113,
        193,
        0,
        0,
    );
}
pub fn vehicle_menu(g: &mut Ground) {
    match menu::run_menu(g, Mem::ds(0x472A), 0, 0) {
        1 => {
            if g.mem.u8(0x6D31) == 0 {
                ui::message(g, b"You have not rented a vehicle on this world.");
            } else {
                let shape = match g.mem.vehicle_condition() {
                    100 => "perfect",
                    70..=255 => "good",
                    50..=69 => "fair",
                    20..=49 => "poor",
                    _ => "pitiful",
                };
                ui::message(g, format!("Your vehicle is in {shape} shape.").as_bytes());
            }
        }
        0 => {
            if g.mem.encounter_mode() == 0 {
                ui::message(g, b"You are not in a vehicle.");
                return;
            }
            let direction = g.mem.u8((g.mem.heading() as u16).wrapping_add(0xD8)) as u16;
            let (dx, dy, _) = g.mem.direction(direction);
            let x = (g.mem.x() >> 1).wrapping_add(dx);
            let y = (g.mem.y() >> 1).wrapping_add(dy);
            let p = cell(g, x, y);
            let tile = (g.mem.read(p.add(1), 1) / 20) as i16;
            if walk::vehicle_terrain(g, p, tile) != 0 && walk::foot_terrain(g, p, tile) != 0 {
                walk::claim_cell(g, g.mem.x(), g.mem.y());
                g.mem.set_event_id(-1);
                g.mem.set_encounter_mode(0);
                walk::set_player_pos(g, x, y);
                if g.scene.is_some() { ui::message(g, b"You leave the vehicle.") }
            } else {
                ui::message(g, b"You can't park your vehicle here.")
            }
        }
        _ => {}
    }
}
