pub mod ui;
pub mod record;
pub mod chargen;
pub mod screens;
pub mod ground;
pub mod space;
pub mod lockstep;
pub mod save;
pub mod play;
#[path = "play/prompts.rs"]
pub mod play_prompts;
#[path = "play/lists.rs"]
pub mod play_lists;
pub mod gamefiles;
pub mod gamefiles_manifest;
pub struct Mt1;
pub const REQUIRED_FILES: &[&str] = &[
    "A0.PIC", "A1.PIC", "A2.PIC", "SPCPICS.PIC", "STARS.PIC", "START.PIC", "TIT.PIC", "TOP.PIC", "ZOD.PIC",
    "CG.LY", "CH.LY", "FONT.LY", "GROUND.LY", "GUNS.LY", "LED.LY", "SPTOP.LY", "STATIONS.LY", "STPT.LY", "TITLE.LY", "TOP.LY",
    "OBJ.DAT", "SPOBJ.DAT", "MAP.PAL", "X.DAT", "Y.DAT", "Z.DAT", "C.DAT", "W.DAT",
    "TRAV.DAT", "TRAV.MSG", "TRAV.ITM", "DCHARS.DAT", "DEFAULT.SAV",
    "0.STP", "1.STP", "2.STP", "3.STP", "4.STP", "5.STP", "6.STP", "7.STP",
    "ATV.VMF", "BOAT.VMF", "BOOM.VMF", "DOOR.VMF", "GRAV.VMF", "GUN1.VMF", "GUNSHOT.VMF", "LAUNCH.VMF", "LGUN.VMF", "MA.VMF",
    "MARK.VMF", "PLASMA.VMF", "SHOT.VMF", "SHOTGUN.VMF", "SIREN.VMF", "SMG1.VMF", "THUMP.VMF", "WALK.VMF", "WHOOOSH.VMF",
    "STARTUP.EXE", "T.DAT", "S.DAT", "CG.DAT",
];
impl mt_engine::Game for Mt1 {
    fn name(&self) -> &'static str { "MegaTraveller 1: The Zhodani Conspiracy" }
    fn required_files(&self) -> &'static [&'static str] { REQUIRED_FILES }
}
#[cfg(test)]
mod t { #[test] fn sixty_plus_four() { assert_eq!(super::REQUIRED_FILES.len(), 64); } }
