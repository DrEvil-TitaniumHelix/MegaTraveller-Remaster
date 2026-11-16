use crate::record::{self, Work, W_SKILLS_PTR, W_STATE};
use crate::ui::{CgUi, Menu};
use mt_engine::sim::record::Slots;
use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

#[derive(Clone)]
pub struct Ds { exe: Rc<RefCell<Vec<u8>>>, base: usize }
impl Ds {
    pub fn load(cg_dat: &[u8]) -> Option<Ds> { Ds::from_exe(mt_engine::data::exepack::unpack(cg_dat).ok()?.exe) }
    pub fn from_exe(mut exe: Vec<u8>) -> Option<Ds> {
        let base = u16::from_le_bytes([*exe.get(8)?, *exe.get(9)?]) as usize * 16 + record::CgTables::DS;
        if exe.len().checked_sub(base)? < 0x3600 { return None }
        exe.resize(base + 0x10000, 0);
        record::CgTables::new(&exe).is_some().then_some(Ds { exe: Rc::new(RefCell::new(exe)), base })
    }
    pub fn image(&self) -> Ref<'_, [u8]> { Ref::map(self.exe.borrow(), |e| &e[self.base..self.base + 0x10000]) }
    pub fn image_mut(&mut self) -> RefMut<'_, [u8]> { RefMut::map(self.exe.borrow_mut(), |e| &mut e[self.base..self.base + 0x10000]) }
    pub fn set_u8(&mut self, p: usize, v: u8) { self.image_mut()[p & 0xFFFF] = v }
    pub fn set_u16(&mut self, p: usize, v: u16) { let [a, b] = v.to_le_bytes(); self.set_u8(p, a); self.set_u8(p + 1, b) }
    pub fn u8(&self, p: usize) -> u8 { self.image()[p & 0xFFFF] }
    pub fn i8(&self, p: usize) -> i8 { self.u8(p) as i8 }
    pub fn u16(&self, p: usize) -> u16 { u16::from_le_bytes([self.u8(p), self.u8(p + 1)]) }
    pub fn u32(&self, p: usize) -> u32 { u32::from_le_bytes([self.u8(p), self.u8(p + 1), self.u8(p + 2), self.u8(p + 3)]) }
    pub fn str(&self, p: usize) -> String { let p = p & 0xFFFF; self.image()[p..].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect() }
    pub fn next_str(&self, p: usize) -> usize { (p & 0xFFFF) + self.str(p).len() + 1 }
    pub fn pstr(&self, table: usize, i: usize) -> String { self.str(self.u16(table + 2 * i) as usize) }
    pub fn pstrs(&self, table: usize, n: usize) -> Vec<String> { (0..n).map(|i| self.pstr(table, i)).collect() }
    pub fn with_tables<R>(&self, f: impl FnOnce(&record::CgTables) -> R) -> R { let e = self.exe.borrow(); f(&record::CgTables::new(&e).expect("validated at Ds::from_exe")) }
    pub fn skill_name(&self, id: u8) -> String { self.str(self.u16(0x1184 + 8 * id as usize) as usize) }
    pub fn skill_short(&self, id: u8) -> String { self.str(self.u16(0x1186 + 8 * id as usize) as usize) }
    pub fn skill_type(&self, id: u8) -> u8 { self.u8(0x118A + 8 * id as usize) }
    pub fn unused(&self, id: u8) -> bool { self.skill_type(id) == 0 && record::unused_skill(self.u8(0x118B + 8 * id as usize)) }
    pub fn item_name(&self, id: u8) -> String { self.str(self.u16(0x16C3 + 3 * id as usize) as usize) }
    pub fn service_name(&self, s: u8) -> String { self.pstr(0x14EE, s as usize) }
    pub fn rule(&self, service: u8, k: usize) -> i8 { self.i8(0x1502 + 6 * service as usize + k) }
    pub fn rank_title(&self, rank: u8, service: u8) -> u8 { self.u8(0x1800 + 5 * rank as usize + service as usize) }
    pub fn specialties(&self) -> Vec<(u8, Vec<u8>)> {
        let (mut p, mut out) = (0x147Cusize, vec![]);
        for id in 0..95u8 {
            if self.skill_type(id) != 2 { continue }
            let mut l = vec![];
            while self.u8(p) != 0xFF && l.len() < 256 { l.push(self.u8(p)); p += 1 }
            p += 1; out.push((id, l));
        }
        out
    }
    pub fn text_list(&self, mut p: usize) -> Vec<(i16, i16, String, u8)> {
        let mut v = vec![];
        while self.u8(p + 6) != 255 && v.len() < 64 { if self.u16(p + 4) != 0 { v.push((self.u16(p) as i16, self.u16(p + 2) as i16, self.pstr(p + 4, 0), self.u8(p + 6))) } p += 7 }
        v
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum A { I(i32), L(i64), S(String) }
impl From<u8> for A { fn from(v: u8) -> A { A::I(v as i32) } } impl From<i8> for A { fn from(v: i8) -> A { A::I(v as i32) } }
impl From<u16> for A { fn from(v: u16) -> A { A::I(v as i32) } } impl From<i16> for A { fn from(v: i16) -> A { A::I(v as i32) } }
impl From<i32> for A { fn from(v: i32) -> A { A::I(v) } } impl From<u32> for A { fn from(v: u32) -> A { A::L(v as i64) } }
impl From<&str> for A { fn from(v: &str) -> A { A::S(v.into()) } } impl From<String> for A { fn from(v: String) -> A { A::S(v) } }
pub fn fmt(f: &str, args: &[A]) -> String {
    let (mut o, mut it, c) = (String::new(), args.iter(), f.chars().collect::<Vec<_>>());
    let mut i = 0;
    let word = |a: Option<&A>| match a { Some(A::I(v)) => *v, Some(A::L(v)) => *v as i32, _ => 0 };
    while i < c.len() {
        if c[i] != '%' { o.push(c[i]); i += 1; continue }
        i += 1; if i >= c.len() { break }
        if c[i] == '%' { o.push('%'); i += 1; continue }
        let (mut zero, mut left) = (false, false);
        while i < c.len() && matches!(c[i], '-' | '+' | ' ' | '#' | '0') { zero |= c[i] == '0'; left |= c[i] == '-'; i += 1 }
        let mut width = None;
        if i < c.len() && c[i] == '*' { width = Some(word(it.next()) as isize); i += 1 }
        else { let s = i; while i < c.len() && c[i].is_ascii_digit() { i += 1 } if i > s { width = c[s..i].iter().collect::<String>().parse().ok() } }
        let mut prec = None;
        if i < c.len() && c[i] == '.' { i += 1; if i < c.len() && c[i] == '*' { prec = Some(word(it.next()) as isize); i += 1 } else { let s = i; while i < c.len() && c[i].is_ascii_digit() { i += 1 } prec = Some(c[s..i].iter().collect::<String>().parse().unwrap_or(0)) } }
        let mut long = false;
        while i < c.len() && matches!(c[i], 'l' | 'h') { long |= c[i] == 'l'; i += 1 }
        if i >= c.len() { break }
        let a = it.next();
        let conv = c[i]; i += 1;
        let s = match conv {
            'd' | 'i' => (if long { match a { Some(A::L(v)) => *v as i32 as i64, Some(A::I(v)) => *v as i64, _ => 0 } } else { word(a) as u16 as i16 as i64 }).to_string(),
            'u' => (if long { match a { Some(A::L(v)) => *v as u32 as i64, Some(A::I(v)) => *v as u32 as i64, _ => 0 } } else { word(a) as u16 as i64 }).to_string(),
            'x' => format!("{:x}", word(a) as u16), 'X' => format!("{:X}", word(a) as u16), 'c' => ((word(a) & 0xFF) as u8 as char).to_string(),
            's' => { let s = match a { Some(A::S(s)) => s.clone(), _ => String::new() }; match prec { Some(p) if p >= 0 => s.chars().take(p as usize).collect(), _ => s } }
            _ => String::new(),
        };
        let (w, n) = (width.unwrap_or(0).max(0) as usize, s.chars().count());
        if n >= w { o.push_str(&s) }
        else if left { o.push_str(&s); o.extend(std::iter::repeat(' ').take(w - n)) }
        else if zero && !matches!(conv, 's' | 'c') { let (sign, digits) = match s.strip_prefix('-') { Some(d) => ("-", d), None => ("", &s[..]) }; o.push_str(sign); o.extend(std::iter::repeat('0').take(w - n)); o.push_str(digits) }
        else { o.extend(std::iter::repeat(' ').take(w - n)); o.push_str(&s) }
    }
    o
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node { pub id: u8, pub level: i8 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Abort;
pub type R<T = ()> = Result<T, Abort>;

pub fn emblem(ui: &mut dyn CgUi, row: i16, col: i16, service: u8) { ui.reset_text(); ui.move_to(row, if service == 3 { col - 4 } else { col }); ui.put_char(service) }

pub struct Cg {
    pub ds: Ds,
    pub skills: Vec<Node>,
    pub term: u8, pub earned: u8, pub drafted: u8, pub warned: bool,
    pub pool: Slots, pub party: [u8; 5], pub party_count: u8,
    pub first_skill: i16, pub selected_skill: i16, pub skill_count: u16, pub totals: Option<(u8, u8)>,
    pub benefit_labels: Vec<String>, pub cash_labels: Vec<String>, pub help_width: i16, pub help_left: i16,
    pub skill_window: i16, pub title_window: i16,
    pub esc_aborts: bool,
    pub specialties: Vec<(u8, Vec<u8>)>,
}
impl Cg {
    pub fn new(ds: Ds) -> Cg {
        let specialties = ds.specialties();
        Cg { ds, skills: vec![], term: 0, earned: 0, drafted: 0, warned: false, pool: record::pool(),
             party: [0; 5], party_count: 0, first_skill: 0, selected_skill: 0, skill_count: 0, totals: None, benefit_labels: vec![], cash_labels: vec![],
             help_width: 0, help_left: 0, skill_window: -1, title_window: -1, esc_aborts: false, specialties }
    }
    pub fn pool_count(&self) -> u8 { record::pool_count(&self.ds.image()) }
    pub fn set_pool_count(&mut self, c: u8) { self.ds.set_u8(record::DS_COUNT, c) }
    pub fn next_slot(&self) -> usize { record::next_free(&self.ds.image()) as usize }
    pub fn set_next_slot(&mut self, n: usize) { self.ds.set_u16(record::DS_NEXT, n as u16) }
    pub fn party_disk(&self) -> bool { self.ds.u8(record::DS_SAVE_OK) != 0 }
    pub fn set_party_disk(&mut self, on: bool) { self.ds.set_u8(record::DS_SAVE_OK, on as u8) }
    pub fn init_character<H: CgUi>(&mut self, ui: &mut H) {
        ui.layout(0, 0, 0);
        for (row, col, s, color) in self.ds.text_list(0x2CC0) { ui.move_to(row, col); ui.set_text_color(color as i16); ui.text(&s) }
        ui.open_window(57, 97, 38, 214, 0, 0, 7);
        self.skills.clear(); self.skill_count = 0;
        let w = ui.work(); for i in 0..57 { w[i] = self.ds.u8(0x2D0E + i) } w[W_SKILLS_PTR] = 0; w[W_SKILLS_PTR + 1] = 0;
        ui.set_text_color(0); let s = self.ds.str(0x2CBC); ui.draw_text_at(0, 27, 185, 8, 5, &s);
    }
    pub fn live(&self, id: u8) -> bool {
        if self.ds.skill_type(id) != 2 { return !self.ds.unused(id) }
        self.specialties.iter().find(|(c, _)| *c == id).is_none_or(|(_, l)| l.iter().any(|&m| !self.ds.unused(m)))
    }
    pub fn find_skill(&self, id: u8) -> Option<usize> { self.skills.iter().position(|n| n.id as i8 as i32 == id as i32) }
    pub fn find_item(&self, w: &Work, item: u8) -> usize { (1..=16).rev().find(|&s| w[29 + s] as i8 as i32 == item as i32).unwrap_or(0) }
    pub fn esc(&self, r: i16) -> R<i16> { if r == -1 && self.esc_aborts { Err(Abort) } else { Ok(r) } }
    pub fn menu<H: CgUi>(&self, ui: &mut H, m: &Menu) -> R<i16> { self.esc(ui.menu(m)) }
    pub fn hmenu<H: CgUi>(&self, ui: &mut H, m: &Menu) -> R<i16> { self.esc(ui.hmenu(m)) }
    pub fn message<H: CgUi>(&self, ui: &mut H, fmt_at: usize, args: &[A]) { let s = fmt(&self.ds.str(fmt_at), args); ui.message(&s) }
    pub fn state<H: CgUi>(&self, ui: &mut H) -> u8 { ui.work()[W_STATE] }
    pub fn dice<H: CgUi>(&self, ui: &mut H, n: u8) -> i16 { let r = ui.rng(); (0..n).fold(n as i16, |t, _| t + (r.next() % 6) as i16) }
    pub fn menu_style<H: CgUi>(&self, ui: &mut H) { ui.reset_text(); ui.set_line_height(8); ui.set_text_spacing(5); ui.set_text_color(2) }
}
pub fn menu_args<'a>(items: &'a [&'a str], a: &'a [i16], initial: i16) -> Menu<'a> { Menu { items, highlight: a[0], row: a[1], col: a[2], allow_unique: a[3], border: a[4], color: a[5], extra: &a[6..], initial } }
pub fn strs(v: &[String]) -> Vec<&str> { v.iter().map(String::as_str).collect() }

#[cfg(test)]
mod t {
    use super::*;
    #[test]
    fn printf_subset() {
        assert_eq!(fmt("For term %d you receive %d skill%s.", &[1u8.into(), 3u8.into(), "s".into()]), "For term 1 you receive 3 skills.");
        assert_eq!(fmt("%d/%d %ld %u %03d|%-3d|%3d|%0.8s|%%|%*s", &[A::I(-5), A::I(70000), A::L(-70000), A::I(-1), A::I(7), A::I(7), A::I(7), "ABCDEFGHIJ".into(), A::I(2), "x".into()]), "-5/4464 -70000 65535 007|7  |  7|ABCDEFGH|%| x");
        assert_eq!(fmt("%s-%d", &["Pilot".into(), A::I(2)]), "Pilot-2");
        assert_eq!(fmt("%", &[]), "");
    }
    #[test]
    fn ds_reads() {
        let mut d = vec![0u8; 0x10000]; d[0x100..0x106].copy_from_slice(b"Navy\0X"); d[0x200] = 0x00; d[0x201] = 0x01;
        d[0x300..0x30E].copy_from_slice(&[5, 0, 95, 0, 0, 1, 4, 0, 0, 0, 0, 0, 0, 255]);
        let mut ds = Ds { exe: Rc::new(RefCell::new(d)), base: 0 }; let shared = ds.clone(); ds.set_u16(0x400, 0x1234);
        assert_eq!((ds.u16(0x400), ds.image()[0x401], shared.u16(0x400)), (0x1234, 0x12, 0x1234));
        assert_eq!(ds.str(0x100), "Navy"); assert_eq!(ds.pstr(0x200, 0), "Navy"); assert_eq!(ds.u16(0x200), 0x100); assert_eq!(ds.i8(0x105), 0x58);
        assert_eq!(ds.next_str(0x100), 0x105); assert_eq!(ds.text_list(0x300), [(5, 95, "Navy".to_string(), 4)]);
        assert_eq!(ds.str(0xFFFF), ""); assert_eq!(ds.u8(0x1FFFF), 0); assert!(Ds::from_exe(vec![0; 16]).is_none()); assert!(Ds::load(&[0; 64]).is_none());
    }
}
