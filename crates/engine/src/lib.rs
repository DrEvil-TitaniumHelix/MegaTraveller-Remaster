pub mod data;
pub mod sim;
pub mod present;
pub mod io;
pub mod play;

pub trait Game {
    fn name(&self) -> &'static str;
    fn required_files(&self) -> &'static [&'static str];
}
