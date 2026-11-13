pub mod scene;
pub mod context;
pub use scene::{Scene, SceneHook, ScreenKind, Intent, Unit, Target, Verb, Cell, Traveller, Preset, ComponentId};
use crate::record::Work;
use mt_engine::io::Files;
use mt_engine::sim::rng::Rng;

thread_local! { static SOFTWARE_CURSOR: std::cell::Cell<bool> = const { std::cell::Cell::new(true) }; }
pub fn software_cursor() -> bool { SOFTWARE_CURSOR.with(std::cell::Cell::get) }
pub fn set_software_cursor(on: bool) { SOFTWARE_CURSOR.with(|v| v.set(on)) }
pub const CURSOR_PEN: (u8, u8) = (0, 0);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu<'a> { pub items: &'a [&'a str], pub highlight: i16, pub row: i16, pub col: i16, pub allow_unique: i16, pub border: i16, pub color: i16, pub extra: &'a [i16], pub initial: i16 }
pub type MenuCb<'a> = &'a mut dyn FnMut(&mut dyn CgUi, i16);
pub type SelectCb<'a> = &'a mut dyn FnMut(&mut dyn CgUi, i16) -> bool;

pub trait CgUi {
    fn rng(&mut self) -> &mut Rng;
    fn work(&mut self) -> &mut Work;
    fn files(&mut self) -> &mut dyn Files;
    fn pool_added(&mut self, _rec: &[u8; 256]) {}

    fn menu_cb(&mut self, m: &Menu, on_change: Option<MenuCb>, on_select: Option<MenuCb>) -> i16;
    fn menu(&mut self, m: &Menu) -> i16 { self.menu_cb(m, None, None) }
    fn hmenu_cb(&mut self, m: &Menu, on_change: Option<MenuCb>, on_select: Option<MenuCb>) -> i16;
    fn hmenu(&mut self, m: &Menu) -> i16 { self.hmenu_cb(m, None, None) }
    fn message(&mut self, s: &str);
    fn text(&mut self, s: &str);
    fn name(&mut self, size: usize) -> Option<String>;
    fn yes_no(&mut self, yes_no: &[&str; 2]) -> bool {
        let r = self.menu(&Menu { items: yes_no, highlight: 4, row: 80, col: 30, allow_unique: 1, border: 2, color: 0, extra: &[1, 0, 3], initial: -1 });
        (if r == -1 { 0 } else { r }) ^ 1 != 0
    }
    fn confirm(&mut self, _question: &str) -> bool { true }
    fn read_key(&mut self) -> mt_engine::io::Key;
    fn start_timer(&mut self);
    fn delay(&mut self, ticks: i16) -> bool;
    fn fatal_error(&mut self, s: &str) -> !;

    fn draw_text_at(&mut self, page: i16, row: i16, col: i16, h: i16, color: i16, s: &str);
    fn fill_rect(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, op: i16);
    fn open_window(&mut self, row: i16, col: i16, h: i16, w: i16, flag: i16, border: i16, color: i16) -> i16;
    fn close_window(&mut self, window: i16);
    fn select_window(&mut self, window: i16);
    fn current_window(&self) -> i16;
    fn reset_text(&mut self);
    fn set_text_color(&mut self, color: i16);
    fn set_text_mode(&mut self, mode: i16);
    fn set_text_spacing(&mut self, spacing: i16);
    fn set_line_height(&mut self, h: i16);
    fn line_height(&mut self) -> i16;
    fn set_alignment(&mut self, a: i16);
    fn set_center(&mut self, x: i16);
    fn get_center(&mut self) -> i16;
    fn draw_centered(&mut self, s: &str, row: i16);
    fn string_width(&mut self, s: &str) -> i16;
    fn get_text_origin(&mut self) -> (i16, i16);
    fn move_to(&mut self, row: i16, col: i16);
    fn get_text_pos(&mut self) -> (i16, i16);
    fn clear_window(&mut self);
    fn init_windows(&mut self);
    fn draw_hline(&mut self, row: i16, col: i16, w: i16, color: i16, op: i16);
    fn draw_vline(&mut self, col: i16, row: i16, h: i16, color: i16, op: i16);
    fn draw_border(&mut self, row: i16, col: i16, h: i16, w: i16, color: i16, op: i16);
    fn put_char(&mut self, ch: u8);
    fn char_width(&mut self, ch: u8) -> i16;
    fn layout(&mut self, _index: i16, _row: i16, _col: i16) {}
    fn flash(&mut self, _row: i16, _col: i16, _h: i16, _w: i16, _color: i16, _index: i16) {}
    fn character_sheet(&mut self, _slot: usize, _rec: &[u8]) {}
    fn hmenu_repeat(&mut self, m: &Menu, on_select: SelectCb) -> i16;
}
#[cfg(test)]
mod t {
    use super::*;
    use mt_engine::io::Overlay;
    struct Fixed { r: i16, log: Vec<String>, rng: Rng, work: Work, files: Overlay }
    impl Fixed { fn new(r: i16) -> Fixed { Fixed { r, log: vec![], rng: Rng::new(1), work: [0; 57], files: Overlay::new(None) } } }
    impl CgUi for Fixed {
        fn rng(&mut self) -> &mut Rng { &mut self.rng } fn work(&mut self) -> &mut Work { &mut self.work } fn files(&mut self) -> &mut dyn Files { &mut self.files }
        fn menu_cb(&mut self, m: &Menu, on_change: Option<MenuCb>, on_select: Option<MenuCb>) -> i16 {
            self.log.push(format!("{:?} {} {} {} {}", m.items, m.row, m.col, m.extra.len(), m.initial));
            let r = self.r; if let Some(c) = on_change { c(self, 0) } if let Some(s) = on_select { s(self, r) } r
        }
        fn hmenu_cb(&mut self, _: &Menu, _: Option<MenuCb>, _: Option<MenuCb>) -> i16 { self.r }
        fn hmenu_repeat(&mut self, m: &Menu, on_select: SelectCb) -> i16 {
            for i in m.items { self.log.push(i.to_string()) } let r = self.r; let mut n = 0; while on_select(self, r) { n += 1; assert!(n < 5) } r
        }
        fn message(&mut self, _: &str) {} fn text(&mut self, s: &str) { self.log.push(s.into()) } fn name(&mut self, _: usize) -> Option<String> { None }
        fn draw_text_at(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, s: &str) { self.log.push(s.into()) }
        fn fill_rect(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) {} fn open_window(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) -> i16 { 1 }
        fn close_window(&mut self, _: i16) {} fn select_window(&mut self, _: i16) {} fn current_window(&self) -> i16 { 0 } fn reset_text(&mut self) {} fn set_text_color(&mut self, _: i16) {}
        fn set_text_mode(&mut self, _: i16) {} fn set_text_spacing(&mut self, _: i16) {} fn set_line_height(&mut self, _: i16) {} fn line_height(&mut self) -> i16 { 8 }
        fn set_alignment(&mut self, _: i16) {} fn set_center(&mut self, _: i16) {} fn get_center(&mut self) -> i16 { 0 } fn draw_centered(&mut self, s: &str, _: i16) { self.log.push(s.into()) }
        fn string_width(&mut self, s: &str) -> i16 { s.len() as i16 * 5 } fn get_text_origin(&mut self) -> (i16, i16) { (0, 0) }
        fn read_key(&mut self) -> mt_engine::io::Key { 13 } fn start_timer(&mut self) {} fn delay(&mut self, _: i16) -> bool { false } fn fatal_error(&mut self, s: &str) -> ! { panic!("{s}") }
        fn move_to(&mut self, _: i16, _: i16) {} fn get_text_pos(&mut self) -> (i16, i16) { (1, 1) } fn clear_window(&mut self) {} fn init_windows(&mut self) {}
        fn draw_hline(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16) {} fn draw_vline(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16) {}
        fn draw_border(&mut self, _: i16, _: i16, _: i16, _: i16, _: i16, _: i16) {} fn put_char(&mut self, _: u8) {} fn char_width(&mut self, _: u8) -> i16 { 5 }
    }
    #[test]
    fn yes_no_as_1aa2_and_callbacks_and_state() {
        let yn = ["Yes", "No"];
        assert!(Fixed::new(0).yes_no(&yn)); assert!(!Fixed::new(1).yes_no(&yn)); assert!(Fixed::new(-1).yes_no(&yn));
        let mut f = Fixed::new(0); f.yes_no(&yn); assert_eq!(f.log, ["[\"Yes\", \"No\"] 80 30 3 -1"]);
        let mut f = Fixed::new(1); f.work()[24] = 18; let v = f.rng().next();
        let m = Menu { items: &["a", "b"], highlight: 4, row: 0, col: 0, allow_unique: 0, border: 2, color: 0, extra: &[], initial: 1 };
        let mut seen = vec![];
        let r = f.menu_cb(&m, Some(&mut |ui: &mut dyn CgUi, i| { let age = ui.work()[24]; ui.draw_text_at(0, 15, 16, 8, 5, &format!("{age}/{i}")) }), Some(&mut |ui, i| seen.push((i, ui.rng().state))));
        assert_eq!((r, &f.log[..]), (1, &["[\"a\", \"b\"] 0 0 0 1".to_string(), "18/0".to_string()][..])); assert_eq!(seen, [(1, f.rng.state)]); assert_eq!(f.rng.trace, [v]);
        f.files().write("UCHARS.DAT", &[1]).unwrap(); assert_eq!(f.files().read("uchars.dat"), Some(vec![1]));
        let mut f = Fixed::new(0); let mut rolls = 0;
        let r = f.hmenu_repeat(&m, &mut |ui, i| { rolls += 1; ui.work()[16] = rolls; ui.draw_text_at(0, 0, 0, 8, 2, &format!("roll{i}")); rolls < 3 });
        assert_eq!((r, f.work[16], &f.log[..]), (0, 3, &["a".to_string(), "b".into(), "roll0".into(), "roll0".into(), "roll0".into()][..]));
    }
}
