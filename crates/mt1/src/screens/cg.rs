use crate::chargen::tables::Ds;
use crate::record::Work;
use crate::ui::{CgUi, Menu, MenuCb, SelectCb};
use mt_engine::data::ly::Entry;
use mt_engine::io::{dump::{Step, Text}, keys, Files, Input, Key, Overlay};
use mt_engine::present::{fb::Fb, ly::{self, Pen}, palette, prim::Raster, text::{self, Font}};
use mt_engine::sim::rng::Rng;
use mt_engine::sim::tick::{Clock, Timer};
#[path = "play_cg.rs"]
mod play;
#[cfg(test)]
mod tests;

const R: Raster = Raster::Wrap;
const MOUSE_PRESENT: bool = true;
const WIDTHS: usize = 0x358E; const CTYPE: usize = 0x3CE5; const PALETTE: usize = 0x3692; const MSG_DELAY: usize = 0x0626;
const SKILLS_W: usize = 0x2D54; const SKILLS_T: usize = 0x2D5C; const FMT_LIMIT: usize = 0x050C; const FMT_TOTAL: usize = 0x0529;
pub type Handler = fn(&mut Screens);
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sheet<'a> { pub name: &'a str, pub stats: [u8; 6], pub service: i16, pub service_name: &'a str, pub rank_name: &'a str, pub cash: i32, pub age: u8, pub hits: (u8, u8), pub tas: bool, pub skills: Vec<(&'a str, u8)>, pub items: Vec<&'a str> }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Win { used: bool, row: i16, col: i16, h: i16, w: i16, restore: bool, cur_row: i16, cur_col: i16, ins_row: i16, ins_col: i16, inner_h: i16, inner_w: i16, color: i16 }

pub struct Screens {
    pub lockstep: Option<Box<dyn crate::lockstep::CgObserver>>,
    pub menu_mouse: (i16, i16),
    pub fb: Fb, pub font: Font, pub cg_ly: Vec<Entry>, pub ds: Ds, ctype: [u8; 256],
    pub input: Box<dyn Input>, pub clock: Box<dyn Clock>, timer: Timer,
    pub rng: Rng, pub work: Work, pub files: Box<dyn Files>, pub overlay: Option<Overlay>, pub pool_record: Option<[u8; 256]>,
    pub log: Vec<Text>, pub screen: &'static str,
    pub snapshots: bool,
    armed: bool, dac: palette::Dac,
    color: i16, pen: Pen, row: i16, col: i16, row_off: i16, col_off: i16, flags: u8, shadow: i16, centre: i16, line_h: i16, spacing: i16,
    wins: [Win; 10], cur_win: usize,
    pub initial: i16, input_changed: bool, msg_delay: i16, text_speed: u16, pub msg_win: i16,
    skills_win: i16, skills_title: i16, help_w: i16, help_left: i16, prev_total: i16, prev_limit: i16,
    bindings: Vec<(Key, Handler, bool)>,
    play_yesno: bool,
}

struct Gated<'a>(&'a mut dyn Input);
impl Input for Gated<'_> { fn next_key(&mut self) -> Key { self.0.next_key() } fn data_mode(&self) -> bool { self.0.data_mode() } fn flush(&mut self) { self.0.flush() } fn reset(&mut self) { self.0.reset() } }
fn bytes(s: &str) -> Vec<u8> { s.chars().map(|c| c as u32 as u8).collect() }
fn latin1(b: &[u8]) -> String { b.iter().map(|&c| c as char).collect() }
fn fmt1(f: &str, v: i32) -> String { f.replacen("%d", &v.to_string(), 1) }
fn tail(border: i16) -> [i16; 3] { if border == 1 { [0, 0, 0] } else { [1, 0, 3] } }

impl Screens {
    pub fn new(exe: Vec<u8>, font_ly: Vec<Entry>, cg_ly: Vec<Entry>, input: Box<dyn Input>, clock: Box<dyn Clock>, files: Box<dyn Files>) -> Option<Screens> {
        let ds = Ds::from_exe(exe)?;
        let font = Font::from_tables(font_ly, &ds.image(), WIDTHS, CTYPE, 2)?;
        let ctype: [u8; 256] = ds.image()[CTYPE..CTYPE + 256].try_into().ok()?;
        let mut fb = Fb::new();
        for i in 0..16 { let v = ds.u16(PALETTE + 2 * i) as i16; if v != -1 { fb.set_reg(i, v as u8) } }
        let msg_delay = ds.u16(MSG_DELAY) as i16;
        let mut s = Screens { lockstep: None, menu_mouse: (-1,-1), fb, font, cg_ly, ds, ctype, input, clock, timer: Timer::default(), rng: Rng::new(1), work: [0; 57], files, overlay: None, pool_record: None, log: vec![], screen: "cg", snapshots: false, armed: true, dac: palette::ega_dac(),
            color: 0, pen: Pen::new(0), row: 0, col: 0, row_off: 0, col_off: 0, flags: 0, shadow: 0, centre: 160, line_h: 0, spacing: 0, wins: [Win::default(); 10], cur_win: 0,
            initial: -1, input_changed: false, msg_delay, text_speed: 0x28, msg_win: 0, skills_win: -1, skills_title: -1, help_w: 0, help_left: 0, prev_total: -1, prev_limit: -1, bindings: vec![], play_yesno: false };
        s.init_windows();
        s.select_window(0);
        Some(s)
    }
    pub fn ds_str(&self, off: usize) -> String { self.ds.str(off) }
    fn frame(&mut self) {
        let rgb = self.fb.rgb(0, &self.dac); self.input.present(&rgb);
        if self.snapshots { self.input.scene(&crate::ui::Scene::program(crate::ui::ScreenKind::Cg)) }
        if self.input.diagnostics() {
            let state = crate::play::diagnostic::cg(self); self.input.scene(&state);
        }
    }
    fn wait_point(&mut self) {
        if self.armed {
            self.armed = false;
            if let Some(o) = &self.overlay {
                let mut f: Vec<(&str, &[u8])> = o.written(); if let Some(r) = &self.pool_record { f.push(("pool_record.bin", &r[..])) }
                self.input.end_files(&f);
            }
            let (rand, text) = (self.rng.take_trace(), std::mem::take(&mut self.log));
            let s = Step::cg(None, &rand, self.rng.state, &text, self.screen, &self.fb.planes, &self.fb.regs, &self.work);
            self.input.step(&s);
        }
        self.frame();
    }
    fn get_key(&mut self) -> Key {
        if let Some(mut observer) = self.lockstep.take() {
            let key = observer.key(self); self.lockstep = Some(observer); return key
        }
        self.wait_point(); let k = self.input.next_key(); self.armed = true; k
    }
    fn read_key_bound(&mut self) -> Key {
        self.input_changed = false;
        loop {
            let k = self.get_key();
            match self.bindings.iter().position(|b| b.0 == k && b.2) {
                Some(i) => { let h = self.bindings[i].1; self.bindings[i].2 = false; h(self); if let Some(b) = self.bindings.iter_mut().find(|b| b.0 == k && b.1 as usize == h as usize) { b.2 = true } }
                None => { self.input_changed = true; return k }
            }
        }
    }
    fn glyph(&mut self, c: u8) {
        let (y, x, colour) = (self.row.wrapping_add(self.row_off), self.col.wrapping_add(self.col_off), self.color as u8);
        let shadow = (self.flags & 1 != 0).then_some(self.shadow as u8);
        if shadow.is_some() { text::glyph(&mut self.fb, R, 0, &self.font, c, y as i32, x as i32, &mut self.pen, colour, shadow); }
        else { ly::layout(&mut self.fb, R, 0, &self.font.glyphs, c as usize, (y + if self.font.descend[c as usize] { 2 } else { 0 }) as i32, x as i32, &mut self.pen); }
        self.col = self.col.wrapping_add(self.font.widths[c as usize] as i16);
    }
    fn fill(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, op: i16) { self.fb.fill(R, 0, row as i32, col as i32, h as i32, w as i32, color as u8, (op >> 8) as u8) }
    fn find_initial(&self, items: &[&str], key: Key, allow_unique: i16) -> i16 {
        let fold = |c: u8| if self.ctype[c as usize] & 1 != 0 { c.wrapping_add(32) } else { c };
        let (key, mut found, mut amb) = (fold(key as u8), -1i16, false);
        for (i, it) in items.iter().enumerate() {
            if fold(bytes(it).first().copied().unwrap_or(0)) == key { if found != -1 { amb = true; break } found = i as i16 }
        }
        found + if found != -1 && (amb || allow_unique == 0) { 1000 } else { 0 }
    }
    fn wait_ticks(&mut self, ticks: i16) { self.input_changed = false; self.frame(); self.run_delay(ticks); }
    fn run_delay(&mut self, ticks: i16) -> bool {
        if let Some(mut observer) = self.lockstep.take() {
            let interrupted = observer.delay(self, ticks); self.lockstep = Some(observer); return interrupted
        }
        if self.overlay.is_some() || self.input.data_mode() { self.timer.delay(self.clock.as_mut(), ticks, &mut Gated(self.input.as_mut())) } else { self.timer.delay(self.clock.as_mut(), ticks, self.input.as_mut()) }
    }
    fn width_of(&self, b: &[u8]) -> i16 { b.iter().fold(0i16, |a, &c| a.wrapping_add(self.font.widths[c as usize] as i16)) }
    pub fn show_message(&mut self, s: &str) {
        let (old_step, old_win) = (self.line_height(), self.cur_win as i16);
        self.set_text_color(3); self.flags |= 1; self.shadow = 0;
        self.select_window(self.msg_win); self.clear_window();
        let (mut t, mut y) = (bytes(s), 0i16);
        loop {
            let spaces: Vec<usize> = t.iter().enumerate().filter(|(_, &c)| c == b' ').map(|(i, _)| i).collect();
            if self.width_of(&t) < 213 || spaces.is_empty() { if !t.is_empty() { let l = latin1(&t); self.draw_text_at(self.msg_win, y, 0, 8, 5, &l) } break }
            let mut last = spaces[0];
            loop {
                let next = t[last + 1..].iter().position(|&c| c == b' ').map(|p| last + 1 + p);
                let width = self.width_of(&t[..next.unwrap_or(t.len())]);
                if next.is_none() && width < 213 { let l = latin1(&t); self.draw_text_at(self.msg_win, y, 0, 8, 5, &l); t.clear(); break }
                if width > 213 || next.is_none() { let l = latin1(&t[..last]); self.draw_text_at(self.msg_win, y, 0, 8, 5, &l); let mut k = last; while k < t.len() && t[k] == b' ' { k += 1 } t.drain(..k); y += 8; break }
                last = next.unwrap();
            }
            if t.is_empty() { break }
        }
        self.flags &= !1; self.set_line_height(old_step); self.set_text_spacing(0); self.select_window(old_win);
    }
    pub fn load_state_line(&mut self, line: &str) {
        let w: Vec<&str> = line.split_whitespace().collect();
        let v = |k: usize| w.get(k).and_then(|t| t.parse::<i32>().ok()).unwrap_or(0) as i16;
        match w.first().copied().unwrap_or("") {
            "color" => { self.color = v(1) } "pen" => { self.pen.colour = v(1) as u8 } "cursor" => { self.row = v(1); self.col = v(2) }
            "origin" => { self.row_off = v(1); self.col_off = v(2) } "flags" => { self.flags = v(1) as u8; self.shadow = v(2) } "center" => { self.centre = v(1) }
            "lineh" => { self.line_h = v(1); self.spacing = v(2) } "curwin" => { self.cur_win = v(1) as usize } "initial" => { self.initial = v(1) }
            "changed" => { self.input_changed = v(1) != 0 } "msgdelay" => { self.msg_delay = v(1) } "msgwin" => { self.msg_win = v(1) }
            "skillswin" => { self.skills_win = v(1); self.skills_title = v(2) } "totals" => { self.prev_total = v(1); self.prev_limit = v(2) }
            "win" => { let i = v(1) as usize; if i < 10 { self.wins[i] = Win { used: v(2) != 0, row: v(3), col: v(4), h: v(5), w: v(6), restore: v(7) != 0, cur_row: v(8), cur_col: v(9), ins_row: v(10), ins_col: v(11), inner_h: v(12), inner_w: v(13), color: v(14) } } }
            _ => {}
        }
    }
    pub fn set_text_flags(&mut self, f: u8) { self.flags |= f }
    pub fn clear_text_flags(&mut self, f: u8) { self.flags &= !f }
    pub fn set_shadow_color(&mut self, c: i16) { self.shadow = c }
    fn bar(&mut self, row: i16, col: i16, h: i16, w: i16, colour: i16) { self.fill(row, col, h, w, colour, 0x1803) }
    fn accept_flash(&mut self, row: i16, col: i16, h: i16, w: i16, colour: i16) { for _ in 0..4 { self.start_timer(); self.bar(row, col, h, w, colour); self.wait_ticks(2) } }
    fn caret(&mut self, row: i16, col: i16, h: i16, colour: i16) { self.fill(row, col, h, 2, colour, 3) }
    pub fn open_window_full(&mut self, row: i16, col: i16, h: i16, w: i16, restore: i16, border: i16, color: i16, colors: [i16; 3], title: Option<(&str, i16)>) -> i16 {
        let Some(i) = self.wins.iter().position(|w| !w.used) else { self.fatal_error("OPENWINDOW: Maximum number of windows [10] already in use.") };
        self.wins[i] = Win { used: true, row, col, h, w, restore: restore != 0, inner_h: h, inner_w: w, color, ..Win::default() };
        if border == 0 { self.msg_win = i as i16 }
        match border {
            0 | 1 => {
                self.fill(row, col, h, w, color, 3);
                if border == 1 { self.draw_border(row, col, h, w, colors[0], 3); let p = &mut self.wins[i]; p.ins_row = 1; p.ins_col = 1; p.inner_h -= 2; p.inner_w -= 2 }
            }
            2 | 3 => {
                self.fill(row, col, h - 2, w - 2, color, 3);
                self.draw_border(row, col, h - 2, w - 2, colors[0], 3);
                self.draw_hline(row + h - 2, col + 1, w - 2, colors[1], 3); self.draw_hline(row + h - 1, col + 2, w - 2, colors[1], 3);
                self.draw_vline(col + w - 2, row + 1, h - 2, colors[2], 3); self.draw_vline(col + w - 1, row + 2, h - 2, colors[2], 3);
                { let p = &mut self.wins[i]; p.ins_row = 1; p.ins_col = 1; p.inner_h -= 4; p.inner_w -= 4 }
                if border == 3 {
                    let (t, tc) = title.unwrap_or(("", 0));
                    let hh = self.line_height() + 4;
                    self.draw_border(row, col, hh, w - 2, colors[0], 3);
                    self.fill(row + 1, col + 1, hh - 2, w - 4, colors[2], 3);
                    let (sr, sc, sw, scol) = (self.row, self.col, self.cur_win as i16, self.color);
                    self.select_window(0);
                    let tw = self.string_width(t);
                    self.move_to(row + 2, ((w - tw - 4) >> 1) + col + 2); self.set_text_color(tc); self.text(t); self.set_text_color(scol);
                    self.select_window(sw); self.move_to(sr, sc);
                    let p = &mut self.wins[i]; p.ins_row += hh; p.inner_h -= hh;
                }
            }
            _ => { self.wins[i].used = false; self.fatal_error(&format!("OPENWINDOW: Illegal border value [{border}].")) }
        }
        i as i16
    }
    pub fn service_emblem(&mut self, row: i16, col: i16, service: i16) { self.move_to(row, if service == 3 { col - 4 } else { col }); self.glyph(service as u8) }
    pub fn reset_message_delay(&mut self) { self.msg_delay = 0 }
    pub fn bind_key(&mut self, key: Key, h: Handler) { self.bindings.push((key, h, true)) }
    pub fn unbind_key(&mut self, key: Key) { self.bindings.retain(|b| b.0 != key) }
    pub fn banner(&mut self, s: &str) { self.set_text_color(0); self.fill(0, 0, 0x31, 0x140, 2, 3); self.draw_centered(s, 20); self.set_text_color(2) }
    pub fn label_panel(&mut self, labels: &[&str], row: i16, col: i16, border: i16, spacing: i16, color: i16, colors: [i16; 3]) -> i16 {
        self.set_text_color(10);
        let mut mw = labels.iter().map(|l| self.string_width(l)).max().unwrap_or(0);
        let h = spacing * labels.len() as i16 + 4; mw += 2;
        let page = self.open_window_full(row, col, h, mw + 3, 1, border, color, colors, None);
        for (i, l) in labels.iter().enumerate() { self.draw_text_at(page, i as i16 * spacing + 2, 1, 12, 2, l) }
        self.open_window_full(row, col + mw, h, 25, 1, border, color, colors, None)
    }
    pub fn attribute_values(&mut self, window: i16, stats: &[u8; 6]) { self.set_text_color(10); for (i, &v) in stats.iter().enumerate() { self.draw_text_at(window, i as i16 * 12 + 2, 5, 12, 2, &(v as i8).to_string()) } }
    pub fn skill_table(&mut self, rows: &[(&str, u8)]) {
        let old_step = self.line_height(); self.set_line_height(8); self.set_text_spacing(5);
        let (old_win, old_c) = (self.cur_win as i16, self.centre);
        if self.skills_win == -1 {
            let (sw, st) = (self.ds_str(SKILLS_W), self.ds_str(SKILLS_T));
            let tw = self.string_width(&sw);
            self.skills_win = self.open_window_full(0x91, 0xAA, 0x35, 0x67, 1, 2, 0, [1, 0, 3], None);
            self.skills_title = self.open_window_full(0x91, 0xA4 - tw, 0xB, tw + 4, 1, 1, 2, [0, 0, 0], None);
            self.select_window(self.skills_title); self.move_to(1, 1); self.set_text_color(3); self.text(&st);
        }
        self.select_window(self.skills_win); self.clear_window(); self.set_center(0x32); self.set_text_spacing(5);
        for (i, &(name, kind)) in rows.iter().enumerate().take(6) {
            self.set_text_color(if kind == 2 { 3 } else { 2 });
            let t = if kind == 1 { format!("+1 {name}") } else { name.to_string() };
            self.draw_centered(&t, 1 + 8 * i as i16);
        }
        self.select_window(old_win); self.set_line_height(old_step); self.set_text_spacing(0); self.set_center(old_c);
    }
    pub fn skill_table_help(&mut self, labels: &[&str], table: i16) {
        let old_step = self.line_height(); self.set_line_height(8);
        let old_win = self.cur_win as i16;
        if self.skills_win != -1 { self.close_window(self.skills_win) }
        if table == 1 { self.help_w = 43; self.help_left = 225 }
        else { self.help_w = labels.iter().take(6).map(|l| self.string_width(l)).max().unwrap_or(0) + 6; self.help_left = 224 - self.help_w }
        self.skills_win = self.open_window_full(116, self.help_left, 53, self.help_w + 2, 1, 2, 0, [1, 0, 3], None);
        self.select_window(self.skills_win);
        for i in 0..6 { let lh = self.line_height(); self.move_to(lh * i + 1, 1); self.set_text_spacing(5); if let Some(l) = labels.get(i as usize) { self.text(l) } }
        self.select_window(old_win); self.set_line_height(old_step); self.set_text_spacing(0);
    }
    pub fn skills_window(&mut self) -> i16 { self.skills_win }
    pub fn set_skills_window(&mut self, w: i16) { self.skills_win = w }
    pub fn skills_title_window(&mut self) -> i16 { self.skills_title }
    pub fn skill_totals(&mut self, total: i16, limit: i16) {
        if self.prev_limit == limit && self.prev_total == total { return }
        let t = fmt1(&self.ds_str(FMT_LIMIT), limit as i32);
        let w = self.string_width(&t) + 2;
        self.fill(101, 101, 19, w, 2, 3); self.draw_border(100, 100, 21, w + 2, 15, 3); self.draw_text_at(0, 111, 102, 8, 5, &t);
        let t = fmt1(&self.ds_str(FMT_TOTAL), total as i32); self.draw_text_at(0, 102, 102, 8, 5, &t);
        self.prev_total = total; self.prev_limit = limit;
    }
    pub fn ticks(&mut self) -> u32 { self.clock.now() }
    pub fn flush_input(&mut self) { self.input.flush() }
    pub fn sheet(&mut self, s: &Sheet) {
        self.set_text_color(4);
        for (i, &v) in s.stats.iter().enumerate() { self.draw_text_at(0, 15, 16 + 11 * i as i16, 8, 5, &format!("{:X}", v as i8)) }
        self.set_text_color(0);
        self.draw_text_at(0, 5, 125, 8, 5, s.name); self.draw_text_at(0, 28, 39, 8, 5, s.service_name); self.service_emblem(30, 15, s.service);
        self.draw_text_at(0, 36, 39, 8, 5, s.rank_name); self.draw_text_at(0, 16, 140, 8, 5, &s.cash.to_string()); self.draw_text_at(0, 27, 130, 8, 5, &s.age.to_string());
        self.draw_text_at(0, 38, 144, 8, 5, &format!("{}/{}", s.hits.0 as i32 - s.hits.1 as i32, s.hits.1));
        if s.tas { self.layout(10, 13, 180) }
        self.set_text_color(10);
        let (mut row, mut col) = (66i16, 5i16);
        for &(name, level) in &s.skills {
            self.move_to(row, col); self.text(name); self.move_to(row, col + 71);
            let small: String = level.to_string().bytes().map(|b| b.wrapping_add(128) as char).collect(); self.text(&small);
            row += 6; if row > 186 { col += 108; row = 66; self.layout(2, 55, 110); self.set_text_color(10) }
        }
        self.set_text_color(4);
        let mut row = 58;
        for it in &s.items { self.move_to(row, 200); row += 8; self.text(it) }
    }
    fn ds_word(&self, off: usize) -> usize { self.ds.u16(off) as usize }
    pub fn skill_cursor(&mut self, selected: i16) { self.fill(67 + 8 * selected, 4, 7, 65, 7, 0x1803) }
    pub fn party_list(&mut self, names: &[&str]) {
        self.fill(120, 208, 47, 85, 2, 3); self.set_text_color(0);
        for (i, n) in names.iter().enumerate().take(5) { if !n.is_empty() { self.move_to(124 + 8 * i as i16, 210); self.text(n) } }
    }
    pub fn input_text(&mut self, len: usize, row: i16, col: i16, width: i16, border: i16, color: i16, colors: [i16; 3]) -> Option<String> {
        let sp = self.line_height(); let mut height = sp + 6; if border == 3 { height += sp + 4 }
        let w = self.open_window_full(row, col, height + 2, width, 1, border, color, colors, None);
        self.select_window(w); self.move_to(1, 1);
        let (mut buf, mut x, mut visible, mut esc) = (Vec::<u8>::new(), 1i16, true, false);
        let mut deadline = self.ticks().wrapping_add(9);
        let tc = self.color; self.draw_vline(col + 1, row + 1, height - 2, tc, 3);
        self.play_text(len,width);
        loop {
            if self.input.diagnostics() {
                let state = crate::play::diagnostic::cg(self); self.input.scene(&state);
            }
            if !self.input.key_pending() && self.ticks() <= deadline { self.frame(); self.clock.wait_tick(); continue }
            if !self.input.key_pending() { visible = !visible; let c = if visible { self.color } else { color }; self.caret(row + 1, col + x, height - 2, c); deadline = self.ticks().wrapping_add(9); continue }
            let k = self.read_key_bound();
            if k == keys::ENTER { break }
            if k == keys::ESC { buf.clear(); esc = true; break }
            if k == keys::BACKSPACE {
                if !buf.is_empty() {
                    if visible { self.caret(row + 1, col + x, height - 2, color) }
                    let c = buf.pop().unwrap(); x -= self.font.widths[c as usize] as i16;
                    self.move_to(1, x); let saved = self.color; self.set_text_color(color); self.glyph(c); self.move_to(1, x); self.set_text_color(saved);
                    let tc = self.color; self.caret(row + 1, col + x, height - 2, tc); visible = true; deadline = self.ticks().wrapping_add(9);
                }
            } else if buf.len() + 1 < len && (32..=126).contains(&k) && x + self.font.widths[k as usize] as i16 <= width - 3 {
                if visible { self.caret(row + 1, col + x, height - 2, color) }
                buf.push(k as u8); x += self.font.widths[k as usize] as i16; self.glyph(k as u8);
                let tc = self.color; self.caret(row + 1, col + x, height - 2, tc); visible = true; deadline = self.ticks().wrapping_add(9);
            }
        }
        self.close_window(w); if esc { None } else { Some(latin1(&buf)) }
    }
}

impl CgUi for Screens {
    fn rng(&mut self) -> &mut Rng { &mut self.rng }
    fn work(&mut self) -> &mut Work { &mut self.work }
    fn files(&mut self) -> &mut dyn Files { match &mut self.overlay { Some(o) => o, None => self.files.as_mut() } }
    fn pool_added(&mut self, rec: &[u8; 256]) { self.pool_record = Some(*rec) }
    fn menu_cb(&mut self, m: &Menu, mut on_change: Option<MenuCb>, mut on_select: Option<MenuCb>) -> i16 {
        self.log.push(Text::Menu(m.items.iter().map(|s| s.to_string()).collect()));
        let n = m.items.len() as i16;
        if n == 0 { return -1 }
        let width = m.items.iter().map(|s| self.string_width(s)).max().unwrap_or(0) + 1;
        let sp = self.line_height();
        let height = sp * n + 6 + if m.border == 3 { sp + 4 } else { 0 };
        let colors: [i16; 3] = m.extra.get(..3).map_or(tail(m.border), |e| [e[0], e[1], e[2]]);
        let title_s = (m.border == 3).then(|| m.extra.get(3).map_or(String::new(), |&p| self.ds_str(p as u16 as usize)));
        let title = title_s.as_deref().map(|t| (t, m.extra.get(4).copied().unwrap_or(0)));
        let w = self.open_window_full(m.row, m.col, height, width + 10, 1, m.border, m.color, colors, title);
        self.select_window(w);
        let saved = self.get_center(); self.set_center((width + 6) >> 1);
        for (i, it) in m.items.iter().enumerate() { self.draw_centered(it, i as i16 * sp + 1) }
        self.set_center(saved);
        let (top, left) = self.get_text_origin(); let left = left + 1;
        let mut sel = if m.initial != -1 { m.initial.clamp(0, n - 1) } else { 0 };
        let bar = |s: &mut Screens, k: i16| s.bar(k * sp + top + 1, left, sp, width + 4, m.highlight);
        bar(self, sel); if let Some(f) = on_change.as_mut() { f(self, sel) }
        self.flush_input(); self.input.reset();
        let mouse_sel = if self.menu_mouse!=(-1,-1) && sp>0 { ((self.menu_mouse.1-top+1)/sp).clamp(0,n-1) } else {0};
        if MOUSE_PRESENT && sel != mouse_sel && !(self.play_yesno && m.initial != -1) { bar(self, sel); sel = mouse_sel; bar(self, sel); if let Some(f) = on_change.as_mut() { f(self, sel) } }
        self.play_menu(m,sel,false);
        loop {
            let k = self.read_key_bound();
            if k == keys::ENTER { break }
            if k == keys::ESC { bar(self, sel); sel = -1; break }
            if (32..=126).contains(&k) {
                let next = self.find_initial(m.items, k, m.allow_unique);
                if next != -1 && next - sel != 1000 {
                    bar(self, sel); sel = if next >= 1000 { next - 1000 } else { next }; bar(self, sel);
                    if let Some(f) = on_change.as_mut() { f(self, sel) }
                    if next < 1000 { break }
                }
            } else if k == keys::UP || k == keys::DOWN {
                bar(self, sel); sel = if k == keys::UP { if sel - 1 < 0 { n - 1 } else { sel - 1 } } else if sel + 1 >= n { 0 } else { sel + 1 }; bar(self, sel);
                if let Some(f) = on_change.as_mut() { f(self, sel) }
            }
        }
        if sel != -1 { self.accept_flash(sel * sp + top + 1, left, sp, width + 4, m.highlight) }
        if let Some(f) = on_select.as_mut() { f(self, sel) }
        self.close_window(w); self.input_changed = true; sel
    }
    fn hmenu_cb(&mut self, m: &Menu, on_change: Option<MenuCb>, mut on_select: Option<MenuCb>) -> i16 {
        self.hmenu_impl(m, on_change, &mut |ui, i| { if let Some(f) = on_select.as_mut() { f(ui, i) } false })
    }
    fn hmenu_repeat(&mut self, m: &Menu, on_select: SelectCb) -> i16 { self.hmenu_impl(m, None, on_select) }
    fn message(&mut self, s: &str) {
        let changed = std::mem::replace(&mut self.input_changed, false);
        if !changed { let d = self.msg_delay; self.wait_ticks(d) }
        self.log.push(Text::Message(s.to_string()));
        self.show_message(s);
        self.msg_delay = (((bytes(s).len() as u32 * self.text_speed as u32) & 0xffff) / 40) as i16;
    }
    fn text(&mut self, s: &str) { self.log.push(Text::Text(s.to_string())); for c in bytes(s) { self.glyph(c) } }
    fn name(&mut self, size: usize) -> Option<String> { self.input_text(size, 100, 125, 80, 1, 0, [3, 0, 0]) }
    fn read_key(&mut self) -> Key { self.play_ack(); self.read_key_bound() }
    fn yes_no(&mut self, yes_no: &[&str; 2]) -> bool {
        let previous = std::mem::replace(&mut self.play_yesno,true);
        let initial = if self.lockstep.is_none() && self.input.wants_decision() { 1 } else { -1 };
        let r = self.menu(&Menu { items: yes_no, highlight: 4, row: 80, col: 30, allow_unique: 1, border: 2, color: 0, extra: &[1,0,3], initial });
        self.play_yesno = previous;
        (if r == -1 {0} else {r}) ^ 1 != 0
    }
    fn confirm(&mut self, question: &str) -> bool {
        if self.lockstep.is_some() || !self.input.wants_decision() { return true }
        self.fill(0, 0, 49, 320, 2, 3); self.set_text_color(0); self.draw_centered(question, 20); self.set_text_color(2);
        let previous = std::mem::replace(&mut self.play_yesno, true);
        let r = self.menu(&Menu { items: &["Yes", "No"], highlight: 4, row: 80, col: 30, allow_unique: 1, border: 2, color: 0, extra: &[1,0,3], initial: 1 });
        self.play_yesno = previous;
        r == 0
    }
    fn start_timer(&mut self) { self.timer.start(self.clock.as_mut()) }
    fn delay(&mut self, ticks: i16) -> bool { self.input_changed = false; self.frame(); self.run_delay(ticks) }
    fn fatal_error(&mut self, s: &str) -> ! { eprintln!("{s}"); std::process::exit(1) }
    fn draw_text_at(&mut self, page: i16, row: i16, col: i16, h: i16, spacing: i16, s: &str) { self.select_window(page); self.move_to(row, col); self.set_line_height(h); self.set_text_spacing(spacing); self.text(s) }
    fn fill_rect(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, op: i16) { self.fill(row, col, h, w, color, op) }
    fn open_window(&mut self, row: i16, col: i16, h: i16, w: i16, flag: i16, border: i16, color: i16) -> i16 { self.open_window_full(row, col, h, w, flag, border, color, tail(border), None) }
    fn close_window(&mut self, w: i16) {
        let i = w as usize;
        if i == 0 { self.fatal_error("CLOSEWINDOW: You cannot close window number 0") }
        if i >= 10 || !self.wins[i].used { self.fatal_error(&format!("CLOSEWINDOW: Attempt to close a non-open window [{w}]")) }
        let p = self.wins[i];
        if p.restore { self.fill(p.row, p.col, p.h, p.w, 0x12, 3) }
        self.wins[i].used = false; self.select_window(0);
    }
    fn select_window(&mut self, w: i16) {
        let c = self.cur_win; self.wins[c].cur_row = self.row; self.wins[c].cur_col = self.col;
        self.cur_win = (w as usize).min(9); let p = self.wins[self.cur_win];
        self.row_off = p.row + p.ins_row; self.col_off = p.col + p.ins_col; self.row = p.cur_row; self.col = p.cur_col;
    }
    fn reset_text(&mut self) {}
    fn set_text_color(&mut self, c: i16) { self.color = c; self.pen.colour = c as u8 }
    fn set_text_mode(&mut self, _m: i16) {}
    fn set_text_spacing(&mut self, s: i16) { self.spacing = s }
    fn set_line_height(&mut self, h: i16) { self.line_h = h }
    fn line_height(&mut self) -> i16 { let d = self.line_h.wrapping_sub(self.spacing); if d < 5 { 8 } else { d } }
    fn set_alignment(&mut self, _a: i16) {}
    fn set_center(&mut self, c: i16) { self.centre = c }
    fn get_center(&mut self) -> i16 { self.centre }
    fn draw_centered(&mut self, s: &str, row: i16) { let w = self.string_width(s); let c = self.centre - (w >> 1); self.move_to(row, c); self.text(s) }
    fn string_width(&mut self, s: &str) -> i16 { self.width_of(&bytes(s)) }
    fn get_text_origin(&mut self) -> (i16, i16) { (self.row_off, self.col_off) }
    fn move_to(&mut self, row: i16, col: i16) { self.row = row; self.col = col }
    fn get_text_pos(&mut self) -> (i16, i16) { (self.row, self.col) }
    fn current_window(&self) -> i16 { self.cur_win as i16 }
    fn clear_window(&mut self) { let p = self.wins[self.cur_win]; self.fill(p.row + p.ins_row, p.col + p.ins_col, p.inner_h, p.inner_w, p.color, 3) }
    fn init_windows(&mut self) { for w in &mut self.wins[1..] { w.used = false } self.wins[0] = Win { used: true, h: 200, w: 320, inner_h: 200, inner_w: 320, ..Win::default() }; self.cur_win = 0 }
    fn draw_hline(&mut self, row: i16, col: i16, w: i16, color: i16, op: i16) { self.fill(row, col, 1, w, color, op) }
    fn draw_vline(&mut self, col: i16, row: i16, h: i16, color: i16, op: i16) { self.fb.line(R, 0, row as i32, col as i32, (row + h - 1) as i32, col as i32, color as u8, (op >> 8) as u8) }
    fn draw_border(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, op: i16) {
        self.draw_hline(row, col, w, color, op); self.draw_hline(row + h - 1, col, w, color, op); self.draw_vline(col, row, h, color, op); self.draw_vline(col + w - 1, row, h, color, op)
    }
    fn put_char(&mut self, ch: u8) { self.glyph(ch) }
    fn char_width(&mut self, ch: u8) -> i16 { self.font.widths[ch as usize] as i16 }
    fn layout(&mut self, i: i16, row: i16, col: i16) { if i >= 0 { ly::layout(&mut self.fb, R, 0, &self.cg_ly, i as usize, row as i32, col as i32, &mut self.pen) } }
    fn flash(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, index: i16) {
        let (w, top) = (w + 2, index * 8 + row);
        for _ in 0..5 { self.start_timer(); self.fill(top, col, h, w, color, 0x1803); self.wait_ticks(2) }
        self.input_changed = true;
    }
    fn character_sheet(&mut self, _slot: usize, rec: &[u8]) {
        if rec.len() < 256 { return }
        let (service, rank) = (rec[0x20] as usize, rec[0x21] as usize);
        let name: String = rec[..20].iter().take_while(|&&b| b != 0).map(|&b| b as char).collect();
        let sn = self.ds_str(self.ds_word(0x14F8 + 2 * service));
        let rn = self.ds_str(self.ds_word((self.ds.u8(0x1800 + 5 * rank + service) as usize) * 2 + 0x1138));
        let (mut skills, mut d) = (vec![], 0x11B4usize);
        for level in &rec[0x2A..0x2A + 79] { while self.ds.u8(d + 7) == 0xFF { d += 8 } if *level != 0 { skills.push((self.ds_str(self.ds_word(d + 2)), *level)) } d += 8 }
        let items: Vec<String> = (0..19).filter_map(|k| { let c = i16::from_le_bytes([rec[0xCC + 2 * k], rec[0xCD + 2 * k]]); (c > 0).then(|| self.ds_str(self.ds_word(0x16C0 + 3 * c as usize))) }).collect();
        let sh = Sheet { name: &name, stats: rec[0x14..0x1A].try_into().unwrap(), service: service as i16, service_name: &sn, rank_name: &rn, cash: i32::from_le_bytes(rec[0xFC..0x100].try_into().unwrap()), age: rec[0x22], hits: (rec[0x25], rec[0x24]), tas: rec[0x28] & 1 != 0,
            skills: skills.iter().map(|(n, l)| (n.as_str(), *l)).collect(), items: items.iter().map(String::as_str).collect() };
        self.sheet(&sh)
    }
}
impl Screens {
    fn hmenu_impl(&mut self, m: &Menu, mut on_change: Option<MenuCb>, on_select: &mut dyn FnMut(&mut dyn CgUi, i16) -> bool) -> i16 {
        let (n, gap, pad) = (m.items.len() as i16, 6i16, 2i16);
        if n == 0 { return -1 }
        let sp = self.line_height();
        let height = sp + 6 + if m.border == 3 { sp + 4 } else { 0 };
        let width = m.items.iter().map(|s| self.string_width(s)).sum::<i16>() + n * gap + 3;
        let colors: [i16; 3] = m.extra.get(..3).map_or(tail(m.border), |e| [e[0], e[1], e[2]]);
        let title_s = (m.border == 3).then(|| m.extra.get(3).map_or(String::new(), |&p| self.ds_str(p as u16 as usize)));
        let title = title_s.as_deref().map(|t| (t, m.extra.get(4).copied().unwrap_or(0)));
        let w = self.open_window_full(m.row, m.col, height, width, 1, m.border, m.color, colors, title);
        self.select_window(w); self.move_to(1, 1);
        for it in m.items { self.text(it); let (r, c) = self.get_text_pos(); self.move_to(r, c + gap) }
        let (top, column) = self.get_text_origin(); let (crow, mut ccol) = (top, column);
        let mut sel = 0;
        if m.initial != -1 { sel = m.initial.clamp(0, n - 1); for k in (1..=sel).rev() { ccol += self.string_width(m.items[k as usize - 1]) + gap } }
        let wd = |s: &mut Screens, k: i16| s.string_width(m.items[k as usize]) + pad;
        let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
        if let Some(f) = on_change.as_mut() { f(self, sel) }
        loop {
            self.flush_input(); self.input.reset();
            let mut mouse_sel=0; let mut mouse_col=column;
            if self.menu_mouse!=(-1,-1) && self.menu_mouse.0>column {
                while mouse_sel<n-1 {
                    if self.menu_mouse.0>=mouse_col && self.menu_mouse.0<=mouse_col+self.string_width(m.items[mouse_sel as usize])+gap {break}
                    mouse_col+=self.string_width(m.items[mouse_sel as usize])+gap;mouse_sel+=1;
                }
            }
            if MOUSE_PRESENT && sel != mouse_sel {
                let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                sel = mouse_sel; ccol = mouse_col;
                let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                if let Some(f) = on_change.as_mut() { f(self, sel) }
            }
            self.play_menu(m,sel,true);
            loop {
                let k = self.read_key_bound();
                if k == keys::ENTER { break }
                if k == keys::ESC { self.input_changed = true; return -1 }
                if (32..=126).contains(&k) {
                    let next = self.find_initial(m.items, k, m.allow_unique);
                    if next != -1 && next - sel != 1000 {
                        let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                        sel = if next >= 1000 { next - 1000 } else { next };
                        ccol = self.get_text_origin().1; for i in 0..sel { ccol += self.string_width(m.items[i as usize]) + gap }
                        let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                        if let Some(f) = on_change.as_mut() { f(self, sel) }
                        if next < 1000 { break }
                    }
                }
                if k == keys::RIGHT && n - sel != 1 {
                    let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                    ccol += self.string_width(m.items[sel as usize]) + gap; sel += 1;
                    let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                    if let Some(f) = on_change.as_mut() { f(self, sel) }
                } else if k == keys::LEFT && sel > 0 {
                    let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                    sel -= 1; ccol -= self.string_width(m.items[sel as usize]) + gap;
                    let ww = wd(self, sel); self.bar(crow + 1, ccol, sp, ww, m.highlight);
                    if let Some(f) = on_change.as_mut() { f(self, sel) }
                }
            }
            let ww = wd(self, sel); self.accept_flash(crow + 1, ccol, sp, ww, m.highlight);
            if !on_select(self, sel) { break }
        }
        self.close_window(w); self.input_changed = true; sel
    }
}

pub fn run(ui: &mut Screens) -> (crate::chargen::pool::Exit, Vec<Option<[u8; 256]>>) {
    let mut cg = crate::chargen::tables::Cg::new(ui.ds.clone());
    let exit = crate::chargen::pool::main_menu(&mut cg, ui);
    let party = crate::ground::party::selected(&cg);
    (exit, party)
}
