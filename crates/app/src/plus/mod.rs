pub mod png;
pub mod bug;
#[cfg(feature = "dev")]
pub mod bridge;
#[cfg(not(feature = "dev"))]
pub mod bridge { pub const AVAILABLE: bool = false; }

pub mod help;
pub mod journal;
#[derive(Clone, Debug, Default)]
pub struct State {
    #[cfg(feature = "dev")]
    pub bridge: bridge::State,
    pub report: Option<bug::Report>,
    pub journal: journal::Journal,
    pub open: bool, pub tab: u8, pub kind: u8,
    pub draft: String, pub note: String, pub scroll: usize, pub status: String,
}
pub fn now() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }
pub fn one_line(text: &str) -> String { text.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(1024).collect() }
pub fn root() -> std::path::PathBuf {
    #[cfg(feature = "dev")]
    { bridge::repo() }
    #[cfg(not(feature = "dev"))]
    {
    let cwd = std::env::current_dir().unwrap_or_default();
    if cwd.join("Cargo.toml").is_file() && cwd.join("docs/KNOWLEDGE.md").is_file() { return cwd; }
    std::env::current_exe().ok().and_then(|p| p.parent().map(std::path::Path::to_path_buf)).unwrap_or(cwd)
    }
}
impl State {
    pub fn apply(&mut self, action: mt1::ui::scene::PlusAction) {
        use mt1::ui::scene::PlusAction::*;
        #[cfg(feature = "dev")]
        if self.tab == 0 && self.bridge.edit(action) { return; }
        match action {
            Open => self.open = true, Close => self.open = false,
            Tab(n) => { self.tab = n.min(2); self.scroll = 0; self.status.clear(); }
            Kind(n) => self.kind = n.min(2),
            Text(c) if !c.is_control() => {
                let text = if self.tab == 0 { &mut self.draft } else if self.tab == 2 { &mut self.note } else { return };
                if text.len() + c.len_utf8() <= 1024 { text.push(c); }
            }
            Backspace => { if self.tab == 0 { self.draft.pop(); } else if self.tab == 2 { self.note.pop(); } }
            Scroll(n) => self.scroll = self.scroll.saturating_add_signed(n as isize),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mt1::ui::scene::PlusAction as A;
    #[test]
    fn text_fields_are_separate_bounded_and_help_is_read_only() {
        let mut s = State::default();
        s.draft = "a".repeat(1023); s.apply(A::Text('é'));
        assert_eq!(s.draft.len(), 1023);
        s.apply(A::Backspace); s.apply(A::Text('é')); assert_eq!(s.draft.len(), 1024);
        s.apply(A::Backspace); assert_eq!(s.draft.len(), 1022);
        s.apply(A::Tab(2)); for c in "a note\n".chars() { s.apply(A::Text(c)); }
        assert_eq!(s.note, "a note");
        s.apply(A::Scroll(10)); s.status = "saved".into(); s.apply(A::Tab(1));
        assert_eq!(s.scroll, 0); assert!(s.status.is_empty());
        s.apply(A::Text('x')); s.apply(A::Backspace);
        assert_eq!(s.note, "a note"); assert_eq!(s.draft, "a".repeat(1022));
        s.apply(A::Scroll(i32::MIN)); assert_eq!(s.scroll, 0);
        s.apply(A::Kind(255)); s.apply(A::Tab(255)); assert_eq!((s.kind, s.tab), (2, 2));
    }
}
