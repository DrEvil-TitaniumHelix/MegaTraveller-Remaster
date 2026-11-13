pub struct Span(Option<(&'static str, std::time::Instant)>);
pub fn span(name: &'static str) -> Span {
    Span(std::env::var_os("MT_PLAY_PROFILE").map(|_| (name, std::time::Instant::now())))
}
impl Drop for Span {
    fn drop(&mut self) {
        if let Some((name, start)) = self.0 { eprintln!("PLAY_PROFILE {name} {}", start.elapsed().as_nanos()); }
    }
}
