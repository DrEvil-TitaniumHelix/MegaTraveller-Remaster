use crate::ground::{input::{menu::Menu, ui}, state::{Far, Ground}};
use mt_engine::{io::{mouse::Hotspot, Event}, play::{Decision, Kind, Option}};

pub fn selector(rects: &[Hotspot], selected: usize) -> Decision {
    let n = rects.len().min(255);
    let mut d = Decision::new(Kind::Menu, "t", "DE5E");
    d.rects = rects[..n].iter().enumerate().map(|(i, h)| (i, [h.left as i32, h.top as i32, h.width as i32, h.height as i32])).collect();
    for i in 0..n {
        let keys = if i < 10 { vec![Event::Key(0x13B + i as u16)] } else {
            let mut keys = vec![Event::Key(0x14D); (i + n - selected) % n];
            keys.push(Event::Key(0x11C)); keys
        };
        d.options.push(Option::new(format!("menu:de5e/{i}"), format!("Item {}", i + 1), keys));
    }
    d.default = d.options.get(selected).map(|o| o.id.clone());
    d.options.push(Option::new("back", "Back", vec![Event::Key(0x101)]));
    d
}

pub fn af79(g: &Ground, p: Far, menu: &Menu, selected: i16, rects: &[Hotspot]) -> Decision {
    let n = menu.labels.len();
    let selected = if selected < 0 || selected as usize >= n { 0 } else { selected as usize };
    let mut d = selector(rects, selected);
    d.context.routine = "AF79".into(); d.context.table = Some(format!("{:04X}", p.off));
    for (i, &label) in menu.labels.iter().enumerate() {
        d.options[i].id = format!("menu:af79.{:04X}/{i}", p.off);
        d.options[i].label = ui::bytes(g, label).iter().map(|&b| b as char).collect();
    }
    d.default = Some(d.options[selected].id.clone());
    if g.scene.is_some() && d.options.iter().any(|o| o.label == "ESC") {
        for o in &mut d.options { if o.label == "ESC" { o.label = "Back".into() } }
        d.options.retain(|o| o.id != "back");
    }
    d
}
