extern crate self as megatraveller;
pub mod look;
pub mod tone;
pub mod pipeline;
pub mod setup;
pub mod font_menu;
pub mod panels;
pub mod mouse;
pub mod plus;
pub mod scenario;
pub mod play_store;
pub mod mcp;
pub mod lockstep { pub mod schema; pub mod symbolic; }

#[cfg(all(test, target_env = "msvc"))]
#[test]
fn the_exe_carries_its_own_c_runtime() { assert!(cfg!(target_feature = "crt-static")); }
