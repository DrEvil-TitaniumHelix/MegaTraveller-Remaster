use crate::ground::{Ground, starport::{shop_menu::Menu, ui}};
use mt_engine::{io::{keys, Event}, play::{Decision, Kind, List, Option, Row}};

fn navigation(m: &Menu<'_>, mut current: usize, mut first: usize, target: usize) -> Vec<Event> {
    let n = m.names.len(); let mut events = vec![];
    for _ in 0..n*2 {
        if current == target { break }
        if target > current {
            events.push(Event::Key(keys::DOWN));
            let end = (first+m.rows).min(n); current += 1;
            while current < end && m.prices[current] == 0 { current += 1; }
            if current >= end {
                first += m.rows; current = first;
                let end = (first+m.rows).min(n);
                while current+1 < end && m.prices[current] == 0 { current += 1; }
            }
        } else {
            events.push(Event::Key(keys::UP));
            let mut next = current as isize-1;
            while next >= first as isize && m.prices[next as usize] == 0 { next -= 1; }
            if next < first as isize {
                first = first.saturating_sub(m.rows); current = (first+m.rows).min(n)-1;
                while current > first && m.prices[current] == 0 { current -= 1; }
            } else { current = next as usize; }
        }
    }
    events
}

fn row_rects(g: &Ground, first: usize, end: usize, pick: impl Fn(usize) -> bool) -> Vec<(usize, [i32; 4])> {
    let (x, y, w) = (g.mem.i16(0x6E08) as i32, g.mem.i16(0x6E0E) as i32, g.mem.i16(0x6E00) as i32);
    (first..end).filter(|&i| pick(i)).map(|i| (i, [x, y + (i - first + 4) as i32 * 8, w, 8])).collect()
}

pub fn shop(g: &mut Ground, m: &Menu<'_>, current: usize, first: usize, more: bool, money: u32, load: u8) {
    if g.lockstep.is_some() || !g.input.wants_decision() { return }
    let quantity = m.select_only == 0;
    let rows = (0..m.names.len()).map(|i| {
        let weight = m.weights.and_then(|w| w.get(i)).copied().unwrap_or(1);
        let (mut limit,mut balance,mut carried)=(m.quantities[i],money,load);
        let signed_price=m.prices[i].wrapping_mul(m.sign as i32 as u32);
        while quantity && limit<m.limits[i] {
            let next=balance.wrapping_sub(signed_price);
            let sum=carried as u16+weight as u16;
            if (next as i32)<0 || !(sum<m.max_load as u16 || (m.max_load==0 && sum<1000)) { break }
            limit+=1; balance=next; carried=carried.wrapping_add(weight);
        }
        Row { id: format!("list:11571/{i}"), label: m.names[i].clone(), enabled: m.prices[i] != 0,
            price: if m.sign < 0 || (!quantity && (m.select_only != 1 || m.money == 0)) { 0 } else { m.prices[i] as u64 },
            limit: if quantity { limit as u32 } else { 1 },
            min_qty: if quantity && g.mem.i16(0x6E0A)==0 { 0 } else { 1 },
            qty: if quantity { m.quantities[i] as u32 } else { 1 }, input: navigation(m,current,first,i) }
    }).collect();
    let mut d = Decision::new(Kind::List,"t","11571");
    d.prompt = Some(m.title.into()); d.default = Some(format!("list:11571/{current}"));
    d.context.table = Some(format!("page:{first}+{}/{}", m.rows, m.names.len()));
    d.rects = row_rects(g, first, (first+m.rows).min(m.names.len()), |i| m.prices[i] != 0);
    d.options.push(Option::new("exit","Exit",vec![Event::Key(keys::ESC)]));
    d.list = Some(List { rows, current, more, money: money as u64,
        increase: if quantity { vec![Event::Key(b'+' as u16)] } else { vec![] },
        decrease: if quantity { vec![Event::Key(0x14A)] } else { vec![] }, submit: vec![Event::Key(keys::ENTER)] });
    crate::play::decide(g,d);
}

pub fn payer(g: &mut Ground, title: &[u8], rows: &[ui::Row]) {
    if g.lockstep.is_some() || !g.input.wants_decision() || rows.is_empty() { return }
    let current = (g.mem.u8(0x6E2B) as usize).saturating_sub(1).min(rows.len()-1);
    let mut d = Decision::new(Kind::List,"t","1199B");
    d.prompt = Some(title.iter().map(|&b| b as char).collect());
    d.options.push(Option::new("exit","Cancel",vec![Event::Key(keys::ESC)]));
    d.list = Some(List { rows: rows.iter().enumerate().map(|(i,r)| Row {
        id: format!("list:1199B/{i}"), label: r.name.iter().map(|&b| b as char).collect(), enabled: true,
        price: 0, limit: 1, qty: 1, min_qty: 1, input: vec![Event::Key(if i>=current {keys::DOWN} else {keys::UP}); i.abs_diff(current)],
    }).collect(), current, more: false, money: 0, increase: vec![], decrease: vec![], submit: vec![Event::Key(keys::ENTER)] });
    d.rects = row_rects(g, 0, rows.len(), |_| true);
    d.default = Some(format!("list:1199B/{current}")); crate::play::decide(g,d);
}
