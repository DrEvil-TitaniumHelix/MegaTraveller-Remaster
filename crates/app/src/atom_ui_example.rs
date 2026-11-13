#[allow(dead_code)]
mod window;
#[allow(dead_code)]
mod menu;
use megatraveller::scenario;
#[path = "../examples/atom_ui.rs"]
mod atom_ui;
fn main() { atom_ui::main(); }
