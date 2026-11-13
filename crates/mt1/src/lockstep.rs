use crate::{ground::Ground, screens::cg::Screens, space::Space};
use mt_engine::io::Key;

pub trait CgObserver {
    fn key(&mut self, game: &mut Screens) -> Key;
    fn delay(&mut self, game: &mut Screens, ticks: i16) -> bool;
}
pub trait GroundObserver {
    fn poll(&mut self, game: &mut Ground);
    fn checkpoint(&mut self, game: &mut Ground, name: &str);
    fn stop(&mut self, game: &mut Ground, reason: &str);
    fn space(&mut self, _game: &mut Ground) -> Option<Box<dyn SpaceObserver>> { None }
    fn space_returned(&mut self, _game: &mut Ground, _space: Box<dyn SpaceObserver>) {}
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpaceEvent { Read, Rand, Minimap, MinimapReturn, Cursor, View, Copy, Spin, WaitKeyTest, GunKeyTest, AimKeyTest }
pub trait SpaceObserver {
    fn read(&mut self, game: &mut Space) -> Option<u32>;
    fn event(&mut self, game: &mut Space, event: SpaceEvent);
    fn srand(&mut self, game: &mut Space) -> Option<u16>;
    fn stop(&mut self, game: &mut Space, reason: &str);
}
