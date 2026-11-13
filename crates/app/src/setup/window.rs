use crate::window::Win;
use megatraveller::setup::{self, Line};
use mt1::gamefiles::{self, Verdict};
use mt_engine::io::{keys, Input};
use std::path::{Path, PathBuf};
fn show(win: &mut Win, lines: &[Line]) {
    if win.faithful() { win.present(&setup::draw(lines)); return }
    let (rgb, runs) = setup::product(lines); win.present(&rgb); win.setup_text(runs);
}
enum Got { Key(u16), Click(i16, i16), Folder(PathBuf) }
fn wait(win: &mut Win) -> Got {
    static SEAM: std::sync::Once = std::sync::Once::new();
    let mut seam = None; SEAM.call_once(|| seam = std::env::var_os("MT_SETUP_DROP").map(PathBuf::from));
    if let Some(p) = seam { for _ in 0..120 { let _ = win.take_dropped(); } return Got::Folder(p) }
    loop {
        if win.key_pending() { return Got::Key(win.next_key()) }
        if let Some(p) = win.take_dropped() { return Got::Folder(p) }
        if let Some((x, y)) = win.take_click() { return Got::Click(x, y) }
        if win.closed { std::process::exit(1) }
    }
}
pub fn screen(win: &mut Win, dir: &Path, verdict: &Verdict, target: &Path) -> Option<PathBuf> {
    win.accept_drops(true);
    if !verdict.refuses() { show(win, &setup::ready(verdict, false)); return (!matches!(wait(win), Got::Key(keys::ESC))).then(|| dir.to_path_buf()) }
    let mut refused: Option<String> = None;
    loop {
        let lines = setup::waiting(refused.as_deref(), cfg!(target_os = "windows"));
        show(win, &lines);
        let on_button = |x: i16, y: i16| setup::button(&lines).is_some_and(|(x0, y0, x1, y1)| (x0..x1).contains(&i32::from(x)) && (y0..y1).contains(&i32::from(y)));
        let given = match wait(win) {
            Got::Key(keys::ESC) => return None,
            Got::Folder(p) => p,
            Got::Click(x, y) if on_button(x, y) => match win.choose_folder() { Some(p) => p, None => continue },
            Got::Key(k) if cfg!(target_os = "windows") && (k == u16::from(b'c') || k == u16::from(b'C')) => match win.choose_folder() { Some(p) => p, None => continue },
            _ => continue,
        };
        let Some(src) = gamefiles::find_game_dir(&given) else { refused = Some(setup::refusal(&given, None)); continue };
        let v = gamefiles::verify(&src);
        if v.refuses() { eprintln!("{}", v.text(&src)); refused = Some(setup::refusal(&src, Some(&v))); continue }
        show(win, &setup::found(&src, &v));
        let copy = loop { match wait(win) { Got::Key(keys::ENTER) | Got::Click(..) => break true, Got::Key(keys::ESC) => break false, _ => {} } };
        if !copy { refused = None; continue }
        match gamefiles::install(&src, target) {
            Err(e) => { eprintln!("{e}"); refused = Some(format!("The files could not be copied. {e}")) }
            Ok(v) => {
                eprintln!("{}", v.text(target));
                if v.refuses() { refused = Some(setup::refusal(&src, Some(&v))); continue }
                show(win, &setup::ready(&v, true));
                return (!matches!(wait(win), Got::Key(keys::ESC))).then(|| target.to_path_buf());
            }
        }
    }
}
