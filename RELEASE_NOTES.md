# Release notes

## Version: v0.9.0-beta

Date: 2026-11-13. MegaTraveller 1 Plus, beta. Tested: full gate, core suites and both play-through routes on the pre-release tree.

### What changed

- **Mouse.** The mouse is on again, beside the keyboard. Click a map square to
  walk, a door to enter, a Traveller to select, an enemy to fire; right-click
  a target for its actions. Menu rows, list rows and the drawn buttons take a click.
- **Saving.** Eight save positions instead of one, a quick-save (F6) and
  quick-load (F9), and three rotating autosaves made on entering a starport and
  after a landing. File > Save... and Load... work on the ground map.
- **Plus window (F12).** Journal, game log, Characters, Map, Help and a bug
  report page that saves a report folder with a picture of the screen.
- **First run.** The first screen asks for one thing: drop the folder of your
  original game on it, or press Choose folder... . The game opens one window.
  The program no longer needs the Microsoft Visual C++ runtime to be installed.
- **Ground screen.** The side panel names the place and the building and shows
  the game day. Each Traveller's name stands under their card. N shows or
  hides the minimap. L switches the first carried torch or lamp on or off.
- **Fights.** Story people are called by name in the fight log. A grenade can
  be thrown at a square. When the party is lost the message stays until a key or a click.
- **Starport.** Services, shop rows and bills take clicks: the refuel bill has
  Buy one unit / All / Pay, the casino has Spin and Leave, a computer trade-in
  has Accept and Cancel.
- **Flight.** The title says where the ship is ("Docked at ..."). The flight
  screen shows the thrust as a number and the fuel left as a percent, and says
  when the fuel is gone.
- **Jump.** The jump map says where you are and what the flashing system
  means, asks before it jumps ("Jump from ... to ...?" with the fuel and the
  seven days), and tells you where you arrived. If no jump is possible it says why.
- **Ship stations.** A click on a station's icon opens that station; an empty
  seat says to assign someone first. The crew records open in flight.
- **Smaller things.** Deleting a character from the pool asks first. The
  marching order picker says what to click. Winning the game shows the ending
  screens. The San self-destruct shakes the screen at zero.
- **Name and modes.** The game is now called MegaTraveller 1 Plus. There is no
  separate 1990 window mode to start.

Recorded verification of the release tree: the full workspace test run was
green. The 130 lockstep comparisons had 68 PASS, 62 documented KNOWN
differences and 0 FAIL. The save-restore sweep passed. Both play-through
routes pass in the debug and the release build.

### Known issues

- Hide is not built (the key answers "Hide: not yet"). In the portrait menu,
  Use item and Give are marked "not yet": use I for items, or P inside a
  character record to give.
- The game cannot be saved in a fight or in flight, and there is no autosave after a jump.
- Saves of the DOS game cannot be read, and the DOS game cannot read these.
- Windows x64 only.

See [keys](docs/KEYS.md) for the key table.

### How to report

1. In the game press F12, open the **Bug** tab, describe what happened and
   press **Save report**. The page names the folder the report was saved to.
2. Zip that folder and attach it to a new issue at
   <https://github.com/DrEvil-TitaniumHelix/MegaTraveller-Remaster/issues>.

Do not attach original game files.
