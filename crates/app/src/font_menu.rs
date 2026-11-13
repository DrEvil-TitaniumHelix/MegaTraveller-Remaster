use std::{cell::{Cell, RefCell}, collections::BTreeMap, fs, io::Read, path::{Component, Path, PathBuf}, sync::{Arc, Once, OnceLock}, time::SystemTime};

pub const DEFAULT_FACE: u8 = 0;
pub const BUILTINS: [&str; 3] = ["IBM EGA 8x14 (Px437, default)", "IBM Plex Mono", "VT323"];
pub const BUILTIN_IDS: [&str; 3] = ["px437_ibm_ega_8x14", "ibm_plex_mono", "vt323"];
pub const LICENSES: &str = "Default text face: Px437 IBM EGA 8x14 by VileR, The Ultimate Oldschool PC Font Pack (int10h.org), CC BY-SA 4.0. IBM Plex Mono and VT323: SIL Open Font License 1.1.";
const EMBEDDED: [&[u8]; BUILTINS.len()] = [
    include_bytes!("../fonts/px437_IBM_EGA_8x14.ttf"), include_bytes!("../fonts/monospace_IBMPlexMono.ttf"), include_bytes!("../fonts/terminal_VT323.ttf"),
];
pub struct Face { pub font: fontdue::Font, pub bold: bool, pub family: String }
thread_local! { static FONT_OPERATION: Cell<bool> = const { Cell::new(false) }; }
pub(crate) fn font_operation<T>(f: impl FnOnce() -> T + std::panic::UnwindSafe) -> Option<T> {
    static HOOK: Once = Once::new();
    HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if !FONT_OPERATION.try_with(Cell::get).unwrap_or(false) { previous(info); }
        }));
    });
    struct Guard(bool);
    impl Drop for Guard { fn drop(&mut self) { FONT_OPERATION.with(|v| v.set(self.0)); } }
    let _guard = Guard(FONT_OPERATION.with(|v| v.replace(true)));
    std::panic::catch_unwind(f).ok()
}
fn parse(bytes: &[u8]) -> Option<Arc<Face>> {
    preflight(bytes)?;
    let font = font_operation(|| fontdue::Font::from_bytes(bytes, fontdue::FontSettings { load_substitutions: false, ..Default::default() }))?.ok()?;
    for c in (32..=255).filter_map(char::from_u32) {
        let m = font.metrics(c, 32.0);
        if m.width > 1024 || m.height > 1024 { return None; }
    }
    let family = family_name(bytes).or_else(|| font.name().map(str::to_owned))?;
    let bold = table(bytes, b"OS/2").and_then(|s| s.get(62..64))
        .is_some_and(|s| u16::from_be_bytes([s[0], s[1]]) & 32 != 0);
    Some(Arc::new(Face { font, bold, family }))
}
const MAX_FONT_BYTES: usize = 32 * 1024 * 1024;
fn word(data: &[u8], at: usize) -> Option<usize> { Some(u16::from_be_bytes(data.get(at..at.checked_add(2)?)?.try_into().ok()?) as usize) }
fn dword(data: &[u8], at: usize) -> Option<usize> { Some(u32::from_be_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize) }
fn preflight(data: &[u8]) -> Option<()> {
    if data.len() > MAX_FONT_BYTES || !matches!(data.get(..4)?, b"\0\x01\0\0" | b"true" | b"OTTO") { return None; }
    let tables = word(data, 4)?;
    if tables == 0 || tables > 128 { return None; }
    let directory_end = 12 + tables * 16;
    let mut total = 0usize;
    let mut tags = Vec::new();
    for i in 0..tables {
        let rec = data.get(12 + i * 16..28 + i * 16)?;
        let (start, len) = (dword(rec, 8)?, dword(rec, 12)?);
        total = total.checked_add(len)?;
        if start < directory_end || len > 8 * 1024 * 1024 || total > MAX_FONT_BYTES || tags.contains(&&rec[..4]) { return None; }
        data.get(start..start.checked_add(len)?)?;
        tags.push(&rec[..4]);
    }
    if word(table(data, b"maxp")?, 4)? > 16384 { return None; }
    let cmap = table(data, b"cmap")?;
    if cmap.len() > 512 * 1024 || word(cmap, 0)? != 0 { return None; }
    let count = word(cmap, 2)?;
    if count == 0 || count > 32 { return None; }
    let mut work = 0usize;
    for i in 0..count {
        let offset = dword(cmap, 8 + i * 8)?;
        if offset < 4 + count * 8 { return None; }
        let sub = cmap.get(offset..)?;
        let format = word(sub, 0)?;
        let length = match format { 0 | 2 | 4 | 6 => word(sub, 2)?, 10 | 12 | 13 => dword(sub, 4)?, 14 => dword(sub, 2)?, _ => return None };
        let sub = sub.get(..length)?;
        let mut charge = |start: usize, end: usize, lookup: usize| -> Option<()> {
            if start > end || end > 0x10ffff { return None; }
            work = work.checked_add((end - start + 1).checked_mul(lookup)?)?;
            (work <= 1024 * 1024).then_some(())
        };
        match format {
            0 => { sub.get(..262)?; charge(0, 255, 1)?; }
            2 => return None,
            4 => {
                let n2 = word(sub, 6)?;
                if n2 == 0 || n2 % 2 != 0 { return None; }
                let n = n2 / 2;
                sub.get(..16 + n * 8)?;
                let mut previous = None;
                for j in 0..n {
                    let (start, end) = (word(sub, 16 + n * 2 + j * 2)?, word(sub, 14 + j * 2)?);
                    if previous.is_some_and(|p| start <= p) { return None; }
                    charge(start, end, 1)?; previous = Some(end);
                }
            }
            6 | 10 => {
                let (start, n, header) = if format == 6 { (word(sub, 6)?, word(sub, 8)?, 10) } else { (dword(sub, 12)?, dword(sub, 16)?, 20) };
                sub.get(..header + n.checked_mul(2)?)?;
                if n != 0 { charge(start, start.checked_add(n - 1)?, 1)?; }
            }
            12 | 13 => {
                let n = dword(sub, 12)?;
                if n > 16384 { return None; }
                sub.get(..16 + n * 12)?;
                let mut previous = None;
                for j in 0..n {
                    let (start, end) = (dword(sub, 16 + j * 12)?, dword(sub, 20 + j * 12)?);
                    if previous.is_some_and(|p| start <= p) { return None; }
                    charge(start, end, if format == 13 { n } else { 1 })?;
                    previous = Some(end);
                }
            }
            14 => { let n = dword(sub, 6)?; sub.get(..10usize.checked_add(n.checked_mul(11)?)?)?; }
            _ => return None,
        }
    }
    if let Some(kern) = table(data, b"kern") { bounded_kern(kern)?; }
    Some(())
}
fn bounded_kern(data: &[u8]) -> Option<()> {
    let apple = word(data, 0)? == 1;
    let (count, mut at) = if apple { (dword(data, 4)?, 8) } else { (word(data, 2)?, 4) };
    if count > 64 { return None; }
    for _ in 0..count {
        let (len, format, header) = if apple { (dword(data, at)?, *data.get(at + 5)? as usize, 8) }
            else { (word(data, at + 2)?, *data.get(at + 4)? as usize, 6) };
        if len < header { return None; }
        let sub = data.get(at..at.checked_add(len)?)?;
        if format == 3 && word(sub, header)? > 256 { return None; }
        at += len;
    }
    Some(())
}
pub fn builtin(index: u8) -> Option<Arc<Face>> {
    static FACES: [OnceLock<Option<Arc<Face>>>; BUILTINS.len()] = [const { OnceLock::new() }; BUILTINS.len()];
    Some(FACES.get(index as usize)?.get_or_init(|| parse(EMBEDDED[index as usize])).as_ref()?.clone())
}
type CachedFace = (Option<Stamp>, Option<Arc<Face>>);
thread_local! { static FILES: RefCell<BTreeMap<PathBuf, CachedFace>> = const { RefCell::new(BTreeMap::new()) }; }
fn load_file(path: &Path) -> Option<Arc<Face>> {
    let stamp = fs::metadata(path).ok().filter(|m| m.is_file()).map(|m| (m.len(), m.modified().ok()));
    FILES.with(|files| {
        let mut files = files.borrow_mut();
        if let Some((old, face)) = files.get(path) { if *old == stamp { return face.clone(); } }
        let face = stamp.filter(|s| s.0 <= MAX_FONT_BYTES as u64).and_then(|_| {
            let mut bytes = Vec::new();
            fs::File::open(path).ok()?.take(MAX_FONT_BYTES as u64 + 1).read_to_end(&mut bytes).ok()?;
            parse(&bytes)
        });
        if face.is_none() { eprintln!("font skipped: {} (invalid or unsupported font)", path.display()); }
        if files.len() >= 64 { files.clear(); }
        files.insert(path.to_owned(), (stamp, face.clone()));
        face
    })
}
pub fn selected(choice: &crate::look::FontChoice) -> Option<Arc<Face>> {
    match choice {
        crate::look::FontChoice::Original => None,
        crate::look::FontChoice::Builtin(i) => builtin(*i),
        crate::look::FontChoice::File(file) => load_file(&font_path(file)?),
    }
}
fn font_path(file: &str) -> Option<PathBuf> {
    let path = Path::new(file);
    let mut components = path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() { return None; }
    let local = crate::look::Look::path().with_file_name("fonts").join(path);
    if local.is_file() { return Some(local); }
    Some(std::env::var_os("WINDIR").map(|w| PathBuf::from(w).join("Fonts").join(path)).filter(|p| p.is_file()).unwrap_or(local))
}
pub fn plus_font() -> crate::look::FontChoice {
    let segoe = crate::look::FontChoice::File("segoeui.ttf".into());
    if selected(&segoe).is_some() { segoe } else { crate::look::FontChoice::Builtin(1) }
}
pub fn plus_font_bold() -> crate::look::FontChoice {
    let bold = crate::look::FontChoice::File("segoeuib.ttf".into());
    if selected(&bold).is_some() { bold } else { plus_font() }
}
fn table<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    let count = u16::from_be_bytes(bytes.get(4..6)?.try_into().ok()?) as usize;
    for i in 0..count {
        let rec = bytes.get(12 + i * 16..28 + i * 16)?;
        if &rec[..4] != tag { continue; }
        let start = u32::from_be_bytes(rec[8..12].try_into().ok()?) as usize;
        let length = u32::from_be_bytes(rec[12..16].try_into().ok()?) as usize;
        return bytes.get(start..start.checked_add(length)?);
    }
    None
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry { pub name: String, pub file: String }
type Stamp = (u64, Option<SystemTime>);
#[derive(Default)]
pub struct Catalog { cache: BTreeMap<PathBuf, (Stamp, Option<Entry>)> }
impl Catalog {
    pub fn scan(&mut self, dir: &Path) -> Vec<Entry> {
        let mut found = BTreeMap::new();
        for item in fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = item.path();
            if !path.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("ttf") || s.eq_ignore_ascii_case("otf")) { continue; }
            let Ok(meta) = item.metadata() else { continue };
            if !meta.is_file() { continue; }
            let stamp = (meta.len(), meta.modified().ok());
            let entry = self.cache.get(&path).filter(|(old, _)| *old == stamp).map(|(_, entry)| entry.clone()).unwrap_or_else(|| {
                let entry = load_file(&path).and_then(|face| {
                    Some(Entry { name: face.family.clone(), file: path.file_name()?.to_str()?.to_owned() })
                });
                entry
            });
            found.insert(path, (stamp, entry));
        }
        self.cache = found;
        let mut entries: Vec<_> = self.cache.values().filter_map(|(_, entry)| entry.clone()).collect();
        entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.file.cmp(&b.file))); entries
    }
}
fn family_name(data: &[u8]) -> Option<String> {
    let data = table(data, b"name")?;
    let word = |offset: usize| -> Option<usize> { Some(u16::from_be_bytes(data.get(offset..offset.checked_add(2)?)?.try_into().ok()?) as usize) };
    let storage = word(4)?;
    for wanted in [16, 1] {
        for n in 0..word(2)? {
            let pos = 6 + n * 12;
            if word(pos + 6)? != wanted { continue; }
            let start = storage.checked_add(word(pos + 10)?)?;
            let bytes = data.get(start..start.checked_add(word(pos + 8)?)?)?;
            let name = match word(pos)? {
                0 | 3 if bytes.len() % 2 == 0 => String::from_utf16(&bytes.chunks_exact(2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()).ok(),
                1 if bytes.is_ascii() => String::from_utf8(bytes.to_vec()).ok(),
                _ => None,
            };
            if let Some(name) = name.filter(|n| !n.trim().is_empty()) { return Some(name); }
        }
    }
    None
}
#[cfg(test)]
mod tests {
    use super::*;
    fn with_table(tag: &[u8; 4], replacement: &[u8]) -> Vec<u8> {
        let mut bytes = EMBEDDED[1].to_vec();
        for i in 0..word(&bytes, 4).unwrap() {
            let at = 12 + i * 16;
            if &bytes[at..at + 4] == tag {
                let offset = bytes.len() as u32;
                bytes[at + 8..at + 12].copy_from_slice(&offset.to_be_bytes());
                bytes[at + 12..at + 16].copy_from_slice(&(replacement.len() as u32).to_be_bytes());
                bytes.extend_from_slice(replacement); return bytes;
            }
        }
        panic!("fixture table missing");
    }
    fn range_font(format: u16, start: u32, end: u32, records: u16) -> Vec<u8> {
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes()); cmap.extend_from_slice(&records.to_be_bytes());
        for _ in 0..records {
            cmap.extend_from_slice(&3u16.to_be_bytes()); cmap.extend_from_slice(&10u16.to_be_bytes());
            cmap.extend_from_slice(&(4 + records as u32 * 8).to_be_bytes());
        }
        cmap.extend_from_slice(&format.to_be_bytes()); cmap.extend_from_slice(&0u16.to_be_bytes());
        for value in [28, 0, 1, start, end, 1] { cmap.extend_from_slice(&value.to_be_bytes()); }
        with_table(b"cmap", &cmap)
    }
    #[test]
    fn cmap_ranges_aliases_and_table_sizes_are_bounded_before_parsing() {
        let small = range_font(13, 0, 255, 1);
        assert_eq!(small.len(), EMBEDDED[1].len() + 40);
        assert!(preflight(&small).is_some()); assert!(parse(&small).is_some());
        for format in [12, 13] {
            for (start, end, records) in [(0, 0xfffffffe, 1), (0, u32::MAX, 1), (10, 2, 1), (0, 0xffff, 32)] {
                let bytes = range_font(format, start, end, records);
                assert!(preflight(&bytes).is_none()); assert!(parse(&bytes).is_none());
            }
        }
        let mut bad_table = small.clone(); bad_table[24..28].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(parse(&bad_table).is_none());
        let dir = std::env::temp_dir().join(format!("mt-font-range-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("small.ttf"), small).unwrap();
        fs::write(dir.join("hostile.ttf"), range_font(13, 0, 0xfffffffe, 1)).unwrap();
        let mut catalog = Catalog::default();
        let entries = catalog.scan(&dir);
        assert_eq!(entries.len(), 1); assert_eq!(entries[0].file, "small.ttf");
        assert_eq!(catalog.scan(&dir), entries);
        fs::remove_dir_all(&dir).unwrap();
    }
    #[test]
    fn truncated_fonts_and_font_panics_are_contained() {
        for n in [0, 1, 4, 12, 63, 128, 1024] { assert!(parse(&EMBEDDED[1][..n]).is_none()); }
        assert!(font_operation(|| panic!("malformed font probe")).is_none());
        assert!(!FONT_OPERATION.with(Cell::get));
    }
    #[test]
    fn broken_fonts_are_skipped_and_cached() {
        let dir = std::env::temp_dir().join(format!("mt-font-menu-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap(); fs::write(dir.join("junk.TTF"), b"junk").unwrap();
        fs::write(dir.join("ignored.txt"), b"junk").unwrap();
        let mut catalog = Catalog::default(); assert!(catalog.scan(&dir).is_empty()); assert!(catalog.scan(&dir).is_empty());
        assert_eq!(catalog.cache.len(), 1);
        fs::remove_dir_all(&dir).unwrap(); assert!(catalog.scan(&dir).is_empty());
        for size in 0..128 { assert!(family_name(&vec![255; size]).is_none()); }
    }
    #[test]
    fn three_embedded_faces_and_real_font_family_scan() {
        let families = ["Px437 IBM EGA 8x14", "IBM Plex Mono", "VT323"];
        for (i, name) in families.iter().enumerate() {
            let face = builtin(i as u8).expect("embedded face parses");
            assert_eq!(&face.family, name); assert!(face.font.has_glyph('A'));
            assert!(!face.bold);
        }
        assert!(builtin(BUILTINS.len() as u8).is_none());
        let dir = std::env::temp_dir().join(format!("mt-font-family-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("unrelated-filename.OTF"), EMBEDDED[1]).unwrap();
        fs::write(dir.join("junk.ttf"), b"not a font").unwrap();
        let mut catalog = Catalog::default(); let entries = catalog.scan(&dir);
        assert_eq!(entries, [Entry { name: "IBM Plex Mono".into(), file: "unrelated-filename.OTF".into() }]);
        assert_eq!(catalog.scan(&dir), entries);
        fs::write(dir.join("unrelated-filename.OTF"), EMBEDDED[2]).unwrap();
        assert_eq!(catalog.scan(&dir)[0].name, "VT323");
        fs::remove_dir_all(&dir).unwrap(); assert!(catalog.scan(&dir).is_empty());
        for name in ["../foo.ttf", "fonts/foo.ttf", "C:/foo.ttf", "", "../segoeui.ttf", "Fonts/segoeui.ttf"] {
            assert!(selected(&crate::look::FontChoice::File(name.into())).is_none(), "{name}");
        }
        if std::env::var_os("WINDIR").is_some_and(|w| PathBuf::from(w).join("Fonts/segoeui.ttf").is_file()) {
            let face = selected(&crate::look::FontChoice::File("segoeui.ttf".into())).expect("Segoe UI from %WINDIR%/Fonts");
            assert_eq!(face.family, "Segoe UI"); assert!(face.font.has_glyph('A') && !face.bold);
            assert!(!crate::look::Look::path().with_file_name("fonts").join("segoeui.ttf").exists(), "never copied into the build");
        }
        assert!(selected(&crate::look::FontChoice::File("no-such-face.ttf".into())).is_none());
    }
}
