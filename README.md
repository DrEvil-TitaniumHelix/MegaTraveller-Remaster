# MegaTraveller 1 Plus

MegaTraveller 1 Plus is a faithful remaster of **MegaTraveller 1: The Zhodani Conspiracy** (Paragon,
1990), written in Rust, with optional modern quality-of-life features. The
default game has a modern panel layout and squad combat in turns. Classic
1990 combat is available in File > Options.

You need the files of your own copy of the 1990 game.
Original game files are never included in this repository or the release zip.

## Setup (Windows x64)

1. Unzip the release into a writable folder.
2. Start `megatraveller.exe`. On the first run it shows a drop box. Drop the
   folder of your original MegaTraveller 1 game on it (the folder that contains
   the file TRAV.COM), or press **Choose folder...** and pick that folder. The
   game checks the folder and copies the files it needs when you press Enter.
   Alternatively, copy the files into `gamefiles\` beside the program yourself.
3. Follow the title menu to create a party, or choose Get Started Quick.

If the folder will not do, the screen says what is wrong with it and what to
try. `README.txt` in the zip lists the 64 required files. Settings live in `look.toml` beside the executable.

## Saving

- Save at a starport desk (Save), or with File > Save... on the ground map.
  There are eight positions; a used one asks before it is overwritten.
- F6 quick-saves and F9 quick-loads on the ground map (also File > Quick-save
  and Quick-load). The quick-save has its own position.
- The game also saves by itself when you enter a starport and after a landing,
  into three rotating autosave positions. `autosave = "off"` in `look.toml`
  turns that off.
- Load with File > Load..., with G > Restore, or with Continue Old Game on the
  title menu.
- The game cannot be saved in a fight or in flight; it says so.

Saves are in `saves\` beside the executable. Saves of the DOS game are not compatible.

## Mouse and keys

Mouse and keyboard both work, and you can mix them.

With the mouse: click a square of the map to walk there, and click a door to
enter. Click a Traveller's portrait to select them; right-click it for the
character record. In a fight, click an enemy to fire at it, or right-click a
target for the list of its actions. Every menu row, list row and button drawn
on the screen can be clicked. The click that brings the window to the front
only focuses it; the next click acts.

Menu letters apply to the open menu. Press **?** for help and **Esc** to close it.

| Action | Key |
| --- | --- |
| Walk | Arrows or keypad; PgUp / End / PgDn for NE / SW / SE; Ctrl+Home or keypad 7 for NW |
| Select Traveller / character record | 1-5 / F1-F5 |
| Pick target in sight / fire / reload | Tab or Shift+Tab / F or Enter / R |
| Menu on the target or the selected Traveller / choose a square | M / X, arrows, Enter |
| Carry out orders and keep the turn / end the turn | O / E |
| At the round-start prompt: party first / enemies first | Space or O / N |
| Swap hand / semi or full auto / grenade | = / Q / T |
| Hold here / hold to interrupt / hold fire | H / Shift+H / Ctrl+H |
| Converse / items / party / vehicle / game | C / I / P / V / G |
| Light on or off (first carried torch or lamp) | L |
| Read older / newer log lines; long log | [ / ]; Shift+L |
| Follow selected Traveller | Home |
| Quick-save / quick-load | F6 / F9 |
| Menus and lists | Arrows, Enter, Esc |
| Help / the Plus window | ? / F12 |
| Fullscreen / pixel shape | F11 / Ctrl+S |

See [the key reference](docs/KEYS.md) for the full table, the ship stations and remaining gaps.

The Plus window (F12, or the Plus menu) holds the Journal, the game log, the
Characters page, the Map, Help and the bug report page.

## Look

Look > Text offers IBM EGA 8x14 (default), IBM Plex Mono and VT323, plus Bold.
Extra `.ttf` or `.otf` faces go in `fonts\`. Look also controls integer or fill
scaling, 4:3 or square pixels, filters, screen effects, palette and background.
Look settings are cosmetic: they never change the game.

View > Components docks a panel, opens it in its own window or hides it;
View > Reset layout restores the default arrangement. File > Options holds
Classic 1990 combat, Easier grenade range and Selective NPC autofire.

## Testing scenarios

Open **Testing scenarios** with Ctrl+Shift+L to load a prepared setup, or run:

```powershell
.\megatraveller.exe --scenario scenarios/m4b_medkit_in_pack.toml
```

Scenarios replace the current setup and require your original files.
[Release notes](RELEASE_NOTES.md) list known issues and how to report a problem.

## Reporting a bug

1. In the game press F12, open the **Bug** tab, describe what happened and press **Save report**. The page then names the folder the report was saved to (`acceptance\local\bugs\<date>-mt1` beside `megatraveller.exe`).
2. Zip that folder and attach it to a new issue at <https://github.com/DrEvil-TitaniumHelix/MegaTraveller-Remaster/issues>.

## Credits and licence

Original game: Paragon Software (1990). Decode and remaster: DrEvil.
Test tooling (deterministic 8086+EGA emulator, lockstep, autoplay) is adapted from
John Champaign's sword-of-aragon-facade, with permission. None of it ships in the game.

Code: [MIT](LICENSE), Copyright (c) 2026 DrEvil. Bundled fonts retain their own
licences: Px437 IBM EGA 8x14 by VileR / int10h.org (The Ultimate Oldschool PC
Font Pack), CC BY-SA 4.0; IBM Plex Mono, SIL Open Font License 1.1; VT323,
SIL Open Font License 1.1. Full font licences and the Px437 credit are in
`crates/app/fonts/` in the source and `licences\` in the release zip.
