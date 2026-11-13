use mt1::gamefiles::Verdict;
use std::path::Path;
pub const COLS: usize = 38; pub const ROWS: usize = 25;
pub const NAME: &str = "MegaTraveller 1 Plus";
pub const DROP: [&str; 2] = ["Drop the folder of your original", "MegaTraveller 1 game here"];
pub const HOLDS: [&str; 2] = ["It is the folder that contains", "the file TRAV.COM"];
pub const CHOOSE: &str = "Choose folder...";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {  Title,  Text,  Box,  Button,  Refusal }
pub type Line = (Style, String);
impl Style {
    fn colours(self) -> (u8, u8, u32, u32) {
        match self {
            Style::Title => (15, 5, 0xFFFFFF, 0xAA00AA), Style::Text => (7, 0, 0xAAAAAA, 0), Style::Box => (15, 1, 0xFFFFFF, 0x0000AA),
            Style::Button => (0, 7, 0x000000, 0xAAAAAA), Style::Refusal => (15, 4, 0xFFFFFF, 0xAA0000),
        }
    }
}
fn wrap(out: &mut Vec<Line>, style: Style, text: &str) { for l in crate::plus::help::lines(text, COLS) { out.push((style, l)); } }
fn blank(out: &mut Vec<Line>) { out.push((Style::Text, String::new())); }
fn title(out: &mut Vec<Line>) { for l in ["", NAME, ""] { out.push((Style::Title, l.into())); } blank(out); }
fn close(mut out: Vec<Line>, how: &str) -> Vec<Line> {
    out.truncate(ROWS - 1);
    while out.len() < ROWS - 1 { blank(&mut out); }
    out.push((Style::Text, how.into()));
    out
}
fn folder_name(path: &Path) -> String { path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned()) }
fn names(list: &[String]) -> String { if list.len() > 4 { format!("{} and {} more", list[..4].join(", "), list.len() - 4) } else { list.join(", ") } }
pub fn refusal(given: &Path, found: Option<&Verdict>) -> String {
    let name = folder_name(given);
    match found {
        None => format!("No MegaTraveller 1 game is in the folder \"{name}\". Try the folder that contains the file TRAV.COM."),
        Some(v) if !v.missing.is_empty() => format!("The game in the folder \"{name}\" is not complete: {} of its 64 files are missing ({}). Try a complete copy.", v.missing.len(), names(&v.missing)),
        Some(v) => format!("The game in the folder \"{name}\" is a different version: {} is not the 1990 file. Try the original 1990 version.", names(&v.code_differ)),
    }
}
pub fn waiting(refused: Option<&str>, choose: bool) -> Vec<Line> {
    let mut out = Vec::new();
    title(&mut out);
    match refused {
        None => wrap(&mut out, Style::Text, "To play you need the files of the original 1990 game MegaTraveller 1: The Zhodani Conspiracy. They are not included."),
        Some(why) => wrap(&mut out, Style::Refusal, why),
    }
    blank(&mut out);
    for l in ["", "", DROP[0], DROP[1], "", ""] { out.push((Style::Box, l.into())); }
    for l in HOLDS { out.push((Style::Text, l.into())); }
    if choose { blank(&mut out); blank(&mut out); out.push((Style::Button, CHOOSE.into())); }
    close(out, "Esc quits.")
}
pub fn found(src: &Path, v: &Verdict) -> Vec<Line> {
    let mut out = Vec::new();
    title(&mut out);
    wrap(&mut out, Style::Text, &format!("Found the game in the folder \"{}\".", folder_name(src)));
    if !v.data_differ.is_empty() { blank(&mut out); wrap(&mut out, Style::Text, &differ(v)); }
    blank(&mut out);
    for l in ["", "", "Press Enter or click here", "to copy its files", "", ""] { out.push((Style::Box, l.into())); }
    close(out, "Esc goes back.")
}
fn differ(v: &Verdict) -> String { format!("{} of its files differ from the usual 1990 ones ({}). The game may still work.", v.data_differ.len(), names(&v.data_differ)) }
pub fn ready(v: &Verdict, copied: bool) -> Vec<Line> {
    let mut out = Vec::new();
    title(&mut out);
    out.push((Style::Text, if copied { "The files of the game are copied." } else { "The files of the game are in place." }.into()));
    if !v.data_differ.is_empty() { blank(&mut out); wrap(&mut out, Style::Text, &differ(v)); }
    blank(&mut out);
    for l in ["", "", "Press any key or click here", "to play", "", ""] { out.push((Style::Box, l.into())); }
    close(out, "Esc quits.")
}
fn left(line: &str) -> i32 { (320 - 8 * line.chars().count().min(COLS) as i32) / 2 }
pub fn button(lines: &[Line]) -> Option<(i32, i32, i32, i32)> {
    let (row, line) = lines.iter().take(ROWS).enumerate().find(|(_, l)| l.0 == Style::Button).map(|(r, l)| (r as i32, &l.1))?;
    Some((left(line) - 16, row * 8 - 6, 320 - left(line) + 16, row * 8 + 14))
}
fn cell(c: char) -> u8 { match c { '_' => b'-', '\\' => b'/', c if c.is_ascii() && crate::pipeline::text::shown(c as u8).is_some() => c as u8, _ => b'?' } }
pub fn draw(lines: &[Line]) -> Vec<u32> { picture(lines, true) }
fn picture(lines: &[Line], letters: bool) -> Vec<u32> {
    let mut rgb = vec![0u32; 64000];
    let mut put = |x: i32, y: i32, c: u32| if (0..320).contains(&x) && (0..200).contains(&y) { rgb[(y * 320 + x) as usize] = c; };
    let rows = |s: Style| -> Option<(i32, i32)> {
        let v: Vec<i32> = lines.iter().take(ROWS).enumerate().filter(|(_, l)| l.0 == s).map(|(r, _)| r as i32).collect();
        Some((*v.first()?, *v.last()?))
    };
    for s in [Style::Title, Style::Refusal] { if let Some((a, b)) = rows(s) { for y in a * 8..b * 8 + 8 { for x in 0..320 { put(x, y, s.colours().3); } } } }
    if let Some((a, b)) = rows(Style::Box) {
        let (x0, y0, x1, y1) = (8, a * 8, 312, b * 8 + 8);
        for y in y0..y1 { for x in x0..x1 {
            let edge = y < y0 + 2 || y >= y1 - 2 || x < x0 + 2 || x >= x1 - 2;
            let dash = if y < y0 + 2 || y >= y1 - 2 { (x - x0) % 12 < 8 } else { (y - y0) % 12 < 8 };
            put(x, y, if edge && dash { 0xFFFFFF } else if edge { 0 } else { 0x0000AA });
        } }
    }
    if let Some((x0, y0, x1, y1)) = button(lines) {
        for y in y0..y1 { for x in x0..x1 { put(x, y, if y == y0 || y == y1 - 1 || x == x0 || x == x1 - 1 { 0xFFFFFF } else if y >= y1 - 3 || x >= x1 - 3 { 0x555555 } else { 0xAAAAAA }); } }
    }
    if let Some(face) = crate::font_menu::builtin(2).filter(|_| letters) {
        for (row, (style, line)) in lines.iter().take(ROWS).enumerate() {
            for (col, c) in line.chars().take(COLS).enumerate() {
                let (m, bitmap) = face.font.rasterize(cell(c) as char, 8.0);
                let (x0, y0) = (left(line) + col as i32 * 8 + m.xmin, row as i32 * 8 + 7 - m.height as i32 - m.ymin);
                for (i, &a) in bitmap.iter().enumerate() { if a > 60 { put(x0 + (i % m.width.max(1)) as i32, y0 + (i / m.width.max(1)) as i32, style.colours().2); } }
            }
        }
    }
    rgb
}
pub fn product(lines: &[Line]) -> (Vec<u32>, Vec<mt_engine::present::textlog::Run>) {
    let (rgb, mut runs) = (picture(lines, false), Vec::new());
    for (row, (style, line)) in lines.iter().take(ROWS).enumerate() {
        let (ink, fill, ..) = style.colours();
        for (col, c) in line.chars().take(COLS).enumerate() {
            let mut bitmap = vec![fill; 64];
            if c != ' ' { for y in 1..7 { bitmap[y * 8 + 3] = ink } }
            runs.push(mt_engine::present::textlog::Run { page: 0, x: (left(line) + col as i32 * 8) as i64, y: row as i64 * 8, colour: ink, shadow: None,
                bytes: vec![cell(c)], font_id: 0, width: 8,
                descend: false, background_width: 8, background_height: 8, background: vec![fill; 64], bitmap, covered: false });
        }
    }
    (rgb, runs)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn words(lines: &[Line]) -> String { lines.iter().map(|l| l.1.as_str()).filter(|l| !l.is_empty()).collect::<Vec<_>>().join(" ") }
    #[test]
    fn the_first_run_screen_is_a_title_a_drop_box_and_a_button() {
        let lines = waiting(None, true);
        assert!(lines.len() == ROWS && lines.iter().all(|l| l.1.chars().count() <= COLS), "{lines:#?}");
        let of = |s: Style| words(&lines.iter().filter(|l| l.0 == s).cloned().collect::<Vec<_>>());
        assert_eq!(of(Style::Title), "MegaTraveller 1 Plus");
        assert_eq!(of(Style::Box), "Drop the folder of your original MegaTraveller 1 game here");
        assert_eq!(of(Style::Button), "Choose folder...");
        assert_eq!(words(&lines), "MegaTraveller 1 Plus To play you need the files of the original 1990 game MegaTraveller 1: The Zhodani Conspiracy. \
            They are not included. Drop the folder of your original MegaTraveller 1 game here It is the folder that contains the file TRAV.COM \
            Choose folder... Esc quits.", "these words and no others");
        let at = |s: Style| lines.iter().position(|l| l.0 == s).unwrap();
        assert!(at(Style::Title) < at(Style::Box) && at(Style::Box) < at(Style::Button), "the order on the screen");
        assert!(lines.iter().all(|l| l.0 != Style::Refusal), "nothing red before the player has done anything");
        let (x0, y0, x1, y1) = button(&lines).unwrap(); let row = at(Style::Button) as i32 * 8;
        assert!(x0 > 60 && x1 < 260 && y0 < row && y1 > row + 8 && y1 <= (ROWS as i32 - 1) * 8, "a button, not a bar across the screen, clear of the last row");
        let rgb = draw(&lines);
        let (top, bottom) = (at(Style::Box) as i32 * 8, lines.iter().rposition(|l| l.0 == Style::Box).unwrap() as i32 * 8 + 8);
        assert!(bottom - top >= 40 && (top + bottom) / 2 > 70 && (top + bottom) / 2 < 130, "the box is big and in the middle: rows {top}..{bottom}");
        let edge: Vec<u32> = (8..312).map(|x| rgb[(top * 320 + x) as usize]).collect();
        assert!(edge.contains(&0xFFFFFF) && edge.contains(&0) && rgb[((top + 4) * 320 + 160) as usize] == 0x0000AA, "a dashed frame around a blue field");
        assert!(waiting(None, false).iter().all(|l| l.0 != Style::Button), "no picker, no button");
    }
    #[test]
    fn after_a_folder_the_screen_says_what_was_found_or_what_is_wrong() {
        let src = Path::new(r"D:\Old games\MegaTra1");
        let good = found(src, &Verdict::default());
        assert_eq!(words(&good), "MegaTraveller 1 Plus Found the game in the folder \"MegaTra1\". Press Enter or click here to copy its files Esc goes back.");
        assert!(good.len() == ROWS && good.iter().all(|l| l.0 != Style::Refusal));
        let none = refusal(Path::new(r"C:\Home\me\Documents"), None);
        assert_eq!(none, "No MegaTraveller 1 game is in the folder \"Documents\". Try the folder that contains the file TRAV.COM.");
        let part = refusal(src, Some(&Verdict { missing: vec!["A0.PIC".into()], ..Verdict::default() }));
        assert!(part.contains("\"MegaTra1\" is not complete: 1 of its 64 files are missing (A0.PIC). Try a complete copy."), "{part}");
        let many = refusal(src, Some(&Verdict { missing: (0..40).map(|i| format!("F{i}.DAT")).collect(), ..Verdict::default() }));
        let other = refusal(src, Some(&Verdict { code_differ: vec!["T.DAT".into()], ..Verdict::default() }));
        assert!(other.contains("is a different version: T.DAT is not the 1990 file. Try the original 1990 version."), "{other}");
        for why in [&none, &part, &many, &other] {
            let lines = waiting(Some(why), true);
            assert!(lines.len() == ROWS && lines.iter().all(|l| l.1.chars().count() <= COLS), "{lines:#?}");
            let at = |s: Style| lines.iter().position(|l| l.0 == s).unwrap();
            assert!(at(Style::Refusal) < at(Style::Box) && at(Style::Box) < at(Style::Button), "the refusal, then the box and the button again");
            assert!(words(&lines).contains(why.as_str()) && lines.last().unwrap().1 == "Esc quits.", "the whole sentence shows: {lines:#?}");
            assert!(button(&lines).unwrap().3 <= (ROWS as i32 - 1) * 8);
        }
        let t = words(&ready(&Verdict { data_differ: vec!["MAP.PAL".into()], ..Verdict::default() }, false));
        assert!(t.contains("1 of its files differ from the usual 1990 ones (MAP.PAL). The game may still work.") && t.contains("Press any key or click here to play"), "{t}");
    }
    #[test]
    fn the_setup_screen_is_laid_in_the_default_face_not_left_as_dots() {
        let lines = waiting(Some(&refusal(Path::new(r"C:\Games\My_MT [old]"), None)), true);
        let look = crate::look::Look::default();
        let dots = crate::pipeline::render(&crate::pipeline::Frame::from_rgb(&draw(&lines)), &look, 1280, 800);
        let (rgb, cells) = product(&lines); let mut frame = crate::pipeline::Frame::from_rgb(&rgb);
        assert!(cells.iter().all(|r| crate::pipeline::text::shown(r.bytes[0]).is_some()), "no character the text layer cannot lay");
        assert!(cells.iter().any(|r| r.background[0] == 1 && r.bytes == b"D") && cells.iter().any(|r| r.background[0] == 4), "the words in the blue box, the refusal on red");
        let row: Vec<_> = cells.iter().filter(|r| r.y == 8).collect();
        assert_eq!(row.first().unwrap().x + row.last().unwrap().x + 8, 320, "a line is centred in the picture");
        frame.text.visible = cells.clone(); frame.text.runs = cells;
        let laid = crate::pipeline::render(&frame, &look, 1280, 800);
        let row = lines.iter().position(|l| l.1 == HOLDS[1]).unwrap() * 8 * 320;
        assert!(rgb[row..row + 8 * 320].iter().all(|&c| c == 0) && draw(&lines)[row..row + 8 * 320].iter().any(|&c| c != 0), "the window's picture carries no bitmap letters (one that hangs out of its cell stayed as a stray dot)");
        let white = |p: &[u32]| p.iter().filter(|&&c| c == 0xFFFFFF).count();
        assert!(white(&laid) > white(&dots), "the face lays whole letters: {} against {} dot pixels", white(&laid), white(&dots));
    }
}
