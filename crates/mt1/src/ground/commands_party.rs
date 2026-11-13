use super::{
    commands,
    input::{menu, ui},
    render,
    state::{Ground, Mem},
    walk,
};

fn walker_at(g: &Ground, x: i16, y: i16) -> Option<i16> {
    (0..5).find(|&i| {
        let p = Mem::walker(i);
        g.mem.read_i(p.add(8), 2) == x as i32 && g.mem.read_i(p.add(10), 2) == y as i32
    })
}
fn can_stand(g: &mut Ground, x: i16, y: i16) -> bool {
    let p = commands::cell(g, x >> 1, y >> 1);
    let door = g.mem.read(p, 1) & 1;
    let tile = g.mem.read(p.add(1), 1) as u8;
    if (door == 0 && (tile <= 14 || matches!(tile, 0x26 | 0x27 | 0x11))) || (door != 0 && tile <= 4)
    {
        return false;
    }
    if x < g.mem.view_x()
        || y < g.mem.view_y()
        || g.mem.view_x().wrapping_add(12) < x
        || g.mem.view_y().wrapping_add(22) < y
    {
        return false;
    }
    if g.mem.read(p.add(2), 1) & 0x40 != 0 || walker_at(g, x, y).is_some() {
        return false;
    }
    if g.mem.read(p.add(2), 1) & 0x3F != 0 {
        return false;
    }
    walk::foot_terrain(g, p, (tile / 20) as i16) != 0
}
fn drop_walker(g: &mut Ground, x: i16, y: i16) {
    let who = (1..5)
        .map(|i| g.mem.party_u8(i + 1) as i16)
        .find(|&i| {
            g.mem
                .party_u8((i as u16).wrapping_mul(256).wrapping_add(0x127))
                != 0
                && g.mem.read(Mem::walker(i), 1) == 0
        })
        .unwrap_or(0);
    let p = Mem::walker(who);
    for off in [8, 12] {
        g.mem.write(p.add(off), x as u16 as u32, 2)
    }
    for off in [10, 14] {
        g.mem.write(p.add(off), y as u16 as u32, 2)
    }
    g.mem.write(p.add(1), g.mem.heading() as u8 as u32, 1);
    g.mem.write(p, 1, 1);
    g.mem.set_i16(0x6D1E, g.mem.i16(0x6D1E).wrapping_sub(1));
}
fn spread(g: &mut Ground, x: i16, y: i16) {
    spread_neighbors(g, x, y);
    let mut pending = vec![(x, y, 0u8)];
    while let Some((x, y, next)) = pending.last_mut() {
        if g.mem.i16(0x6D1E) == 0 {
            return;
        }
        if *next == 9 {
            pending.pop();
            continue;
        }
        let cell = *next;
        *next += 1;
        if cell == 4 {
            continue;
        }
        let xx = x.wrapping_add((cell as i16 / 3 - 1) * 2);
        let yy = y.wrapping_add((cell as i16 % 3 - 1) * 2);
        if let Some(w) = walker_at(g, xx, yy) {
            if g.mem.read(Mem::walker(w), 1) == 1 {
                spread_neighbors(g, xx, yy);
                pending.push((xx, yy, 0));
            }
        }
    }
}
fn spread_neighbors(g: &mut Ground, x: i16, y: i16) {
    if g.mem.i16(0x6D1E) == 0 {
        return;
    }
    g.mem
        .write(Mem::walker(walker_at(g, x, y).unwrap_or(-1)), 2, 1);
    for dx in [0i16, 2] {
        for dy in [-2i16, 0, 2] {
            if dy != 0
                && can_stand(g, x.wrapping_add(dx), y.wrapping_add(dy))
                && g.mem.i16(0x6D1E) != 0
            {
                drop_walker(g, x.wrapping_add(dx), y.wrapping_add(dy));
            }
        }
    }
    if g.mem.i16(0x6D1E) == 0 {
        return;
    }
    for dy in [-2i16, 0, 2] {
        if can_stand(g, x.wrapping_sub(2), y.wrapping_add(dy)) && g.mem.i16(0x6D1E) != 0 {
            drop_walker(g, x.wrapping_sub(2), y.wrapping_add(dy))
        }
    }
    if g.mem.i16(0x6D1E) == 0 {
        return;
    }
    if can_stand(g, x.wrapping_add(2), y) && g.mem.i16(0x6D1E) != 0 {
        drop_walker(g, x.wrapping_add(2), y)
    }
}
pub fn split(g: &mut Ground) -> bool {
    let (vx, vy) = (g.mem.view_x(), g.mem.view_y());
    if vx & 1 != 0 {
        g.mem.set_view_x(vx & !1);
        g.mem.set_event_id(-1)
    }
    if vy & 1 != 0 {
        g.mem.set_view_y(vy & !1);
        g.mem.set_event_id(-1)
    }
    for i in 0..5 {
        let p = Mem::walker(i);
        for off in [8, 10] {
            g.mem.write(p.add(off), 65535, 2)
        }
        for off in [16, 18, 20] {
            g.mem.write(p.add(off), 0, 2)
        }
        for off in [0, 22, 5, 3] {
            g.mem.write(p.add(off), 0, 1)
        }
    }
    let who = g.mem.selected_char() as i16;
    let p = Mem::walker(who);
    for off in [8, 12] {
        g.mem.write(p.add(off), g.mem.x() as u16 as u32, 2)
    }
    for off in [10, 14] {
        g.mem.write(p.add(off), g.mem.y() as u16 as u32, 2)
    }
    g.mem.write(p.add(1), g.mem.heading() as u8 as u32, 1);
    g.mem.write(p, 1, 1);
    g.mem.set_leader(who);
    let alive = (0..5)
        .filter(|i| g.mem.party_u8(i * 256 + 0x127) != 0)
        .count() as i16;
    g.mem.set_i16(0x6D1E, alive - 1);
    spread(g, g.mem.x(), g.mem.y());
    if g.mem.i16(0x6D1E) == 0 {
        g.mem.set_combat(1)
    }
    if g.mem.combat() == 0 {
        g.mem.set_view_x(vx);
        g.mem.set_view_y(vy)
    }
    g.mem.set_individual(0);
    g.mem.set_party_fired(0);
    g.mem.set_panel_flag(0);
    if g.mem.combat() != 0 {
        render::walker_panel(g, false, true)
    }
    g.mem.combat() != 0
}
pub fn break_party(g: &mut Ground) {
    super::squad::note(g, "break_party", &[]);
    let msg = if g.mem.outdoors() == 0 {
        Some("You can't break your party in the Starport area.")
    } else if g.mem.encounter_mode() != 0 {
        Some("You can't break your party while in a vehicle.")
    } else if g.mem.combat() != 0 {
        Some("You are already broken.")
    } else if !split(g) {
        Some("Not enough room in this area to break up your party.")
    } else {
        None
    };
    if let Some(msg) = msg {
        ui::message(g, msg.as_bytes())
    } else {
        render::dismiss_message(g, 1)
    }
}
pub fn regroup(g: &mut Ground) {
    if super::round::in_encounter(g) { ui::message(g, b"You can't regroup in a fight."); return }
    if g.mem.combat() == 0 {
        ui::message(g, b"Your party is already grouped.");
        return;
    }
    if g.mem.individual() != 0 {
        return;
    }
    g.mem.set_combat(0);
    g.mem.set_individual(0);
    render::dismiss_message(g, 1);
    render::walker_panel(g, false, true);
    match g.mem.x().wrapping_sub(g.mem.view_x()) >> 1 {
        0 => g.mem.set_view_x(g.mem.view_x().wrapping_sub(1)),
        6 => g.mem.set_view_x(g.mem.view_x().wrapping_add(1)),
        _ => {}
    }
    match g.mem.y().wrapping_sub(g.mem.view_y()) >> 1 {
        0 => g.mem.set_view_y(g.mem.view_y().wrapping_sub(1)),
        11 => g.mem.set_view_y(g.mem.view_y().wrapping_add(1)),
        _ => {}
    }
    for i in 0..5 {
        g.mem.write(Mem::walker(i).add(5), 0, 1)
    }
    render::redraw_party(g);
    g.mem.set_party_fired(0);
    render::main_screen(g, 0);
}
pub fn run(g: &mut Ground) {
    match menu::run_menu(g, Mem::ds(0x44D2), 0, 0) {
        0 => break_party(g),
        1 => regroup(g),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
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
    fn spread_visits_fifth_walker_from_an_unoccupied_origin() {
        let mut g = Ground::from_ds(
            vec![],
            vec![0; Mem::PARTY_SIZE],
            Box::new(NoInput),
            Box::new(Counter::default()),
            Box::new(Dir(std::path::PathBuf::new())),
        );
        g.mem.set_i16(0x6D1E, 1);
        for i in 0..5 {
            let p = Mem::walker(i);
            g.mem.write(p, 1, 1);
            g.mem.write(p.add(8), (2 * (i + 1)) as u32, 2);
            g.mem.write(p.add(10), 0, 2);
        }
        spread(&mut g, 0, 0);
        for i in -1..5 {
            assert_eq!(g.mem.read(Mem::walker(i), 1), 2, "walker {i}");
        }
        assert_eq!(g.mem.i16(0x6D1E), 1);
    }

    #[test]
    fn malformed_membership_does_not_exhaust_the_native_call_stack() {
        let mut g = Ground::from_ds(
            vec![],
            vec![0; Mem::PARTY_SIZE],
            Box::new(NoInput),
            Box::new(Counter::default()),
            Box::new(Dir(std::path::PathBuf::new())),
        );
        g.mem.alloc(0x8000, 65536);
        g.mem
            .set_pointer(0xBC, super::super::state::Far::new(0, 0x8000));
        g.mem.set_map_stride(12);
        for x in 0..7 {
            for y in 0..12 {
                g.mem.write(commands::cell(&g, x, y).add(1), 20, 1);
            }
        }
        for i in 1..5 {
            g.mem.write(Mem::walker(i).add(8), 65535, 2);
            g.mem.write(Mem::walker(i).add(10), 65535, 2);
        }
        g.mem.write(Mem::walker(0), 1, 1);
        g.mem.set_i16(0x6D1E, i16::MIN);
        spread(&mut g, 0, 0);
        assert_eq!(g.mem.i16(0x6D1E), 0);
    }
}
