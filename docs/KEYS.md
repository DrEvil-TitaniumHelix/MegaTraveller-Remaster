# Keys (v0.9.0-beta)

The mouse works beside these keys (see README, "Mouse and keys"). Every 1990 function has a key here, not every 1990 key is kept. A key the 1990 game has for the function stays. A new key takes nothing from the 1990 ground table (`notes/input_map.md`, "Ground game", dispatch T:6923).

`works` = the key path exists. `MISSING` = not built; the Note says what it must do. The 1990 dispatch is reached from `crates/mt1/src/ground/main_loop.rs` when the squad table does not consume the key; rows marked `1990` have no squad code of their own.

New keys: **E** End turn, **X** choose a square, **[** / **]** log scroll, **?** help. Free in the 1990 ground table: A B D E H J K L M N S U X Y Z, Space, punctuation. (W and Q are 1990 debug-only; Q is already fire mode.) Inside an open menu its letters win, so E there is Evade.

## Walking (exploration and fight)

| 1990 function | Key | State | Code | Note |
|---|---|---|---|---|
| Walk / step, 8 ways | Arrows, keypad; PgUp NE, End SW, PgDn SE | works | `main_loop.rs` (`0x47..=0x51`); refused step `round.rs` | In a fight 1 MP a square. |
| Walk NW | Keypad 7, Ctrl+Home | works | `window/hosts.rs`, `window.rs` | Bare Home is Follow. |
| Walk to a chosen square | X, arrows, Enter | MISSING | cursor uses `orders::hover` `orders.rs`; Enter `keymap.rs` | See "The square cursor". The 8-way walk stays the plain way to walk; T019 uses it. |
| Marching order swap (grouped) | O | MISSING | swallowed at `round.rs` | 1990 key. Let O through when grouped (`combat() == 0`); split and no encounter stays swallowed. |

## A fight: the turn

| 1990 function | Key | State | Code | Note |
|---|---|---|---|---|
| We go first (round start) | Space or O | works | `round.rs` | |
| They go first (round start) | N | works | `round.rs` | |
| Select a Traveller | 1-5 | works | `orders.rs` | Costs nothing. |
| Next / previous Traveller | Tab / Shift+Tab, when no hostile is in sight | works | `keymap.rs` | |
| Orders: fire standing orders, keep the turn | O | works | `round.rs`, `standing::run` | Space no longer fires orders during a turn; it answers the round start and pages messages only. |
| End turn: fire standing orders, then end | E | works | `Intent::EndTurn` `orders.rs`, `standing::end_turn` | Same call as the button. At Round N - party to act the prompt owns the turn: Space = party first, N = NPCs first (`play/turns.rs`), and E does nothing, as 1990. |
| Hold here (drop the route) | H | works | `keymap.rs` | |
| Hold (interrupt) | Shift+H | works | `keymap.rs` | |
| Hold fire | Ctrl+H | works | `keymap.rs` | |
| Tactical points +/- | Keypad + or main-row + / - | works | `keymap.rs` | Takes keypad + from the 1990 swap hand; = stays for that. |

## A fight: shooting

| 1990 function | Key | State | Code | Note |
|---|---|---|---|---|
| Pick a target among thugs | Tab / Shift+Tab | works | `ground/keymap.rs` | nearest first, only thugs drawn on the fixed 7 x 12 screen with line of sight; sets the standing target and names it. |
| Pick a target, party split, no fight yet | Tab / Shift+Tab | works | `ground/keymap.rs` | Same screen and sight limits as in a fight. With no eligible thug, selects the next / previous Traveller. T018 checks off-screen refusal. |
| Fire at the target | F or Enter | works | `orders.rs` | No target: `no target: press Tab`. An off-screen standing target: `target outside the screen: cannot fire`; no shot or fight starts. Menu Fire has the same refusal. |
| Reload | R | works | `orders.rs` | Acts on the selected Traveller; select first with 1-5. |
| Light on / off | L | works | `commands_items.rs toggle_light`, `input/dispatch.rs 0x26`, the Light button | the first carried torch / lamp, hand or pack, of the first Traveller who has one; log "Light turned on (Magellan's electric torch)"; nobody: "Nobody carries a light". L is free in the 1990 ground table (in flight L = land: ground only). |
| Swap hand | = | works | `keymap.rs` | |
| Fire mode, semi / full auto | Q | works | `keymap.rs` | |
| Grenade: choose type | T, then E / C / R | works, 1990 | `keymap.rs` | Then Tab, M, T at a thug, or X, arrows, M, T at a square. |
| Menu on the target thug | M | works | `panels/keyboard.rs` | Order: hovered thing, else the standing target, else the portrait. Reaches Full auto, Rapid, Pinpoint, Melee, Disarm, Throw, Target, Tactics. |
| Menu letters after M | item's letter, Enter = default, Esc = close | MISSING | `window/hosts.rs` reads `ui.menu`; M sets only `squad.orders.menu` (`keymap.rs`) | The letter must go to the menu, not the 1990 table. Set `ui.menu` the way a click does. |
| Back out one level | Esc | works | `keymap.rs` | Menu, rapid pick, grenade, route, then the 1990 Quit menu. |

## The portrait menu, by key

No separate key. M opens it for the selected Traveller when no target stands, and each function has its own key:

| Function | Key | State | Code |
|---|---|---|---|
| Select | 1-5 | works | `orders.rs` |
| Character | F1-F5 | works, 1990 | `input_map.md` T:A8F7 |
| Reload / Swap / Fire mode / Hold / Hold fire | 1-5, then R / = / Q / Shift+H / Ctrl+H | works | table above |
| Regroup | P | works, 1990 | `orders.rs` |

## The square cursor (X)

X toggles a cursor on the selected Traveller's square, setting `orders.hovered` = `Target::Cell` through `orders::hover`. While it is on, arrows move the cursor, not the Traveller. Enter = the default (Move), M = the cell menu: Run here, Evade to here, Hold here, Throw grenade here, Fire launcher here, Advance. Esc leaves it. Both already read `hovered` (`keymap.rs`, `keymap.rs`), so only the toggle and the arrows are new. It stays on the one 7 x 12 screen.

Run and Evade are rulebook verbs with no 1990 key. They come by this menu and take no key of their own.

## Ground commands

| 1990 function | Key | State | Code |
|---|---|---|---|
| Converse | C | works, 1990 | `input_map.md` T:6923; `orders.rs` says regroup when split |
| Items: Take / Use | I | works, 1990 | T:6923 |
| Vehicle: Leave / Status | V | works, 1990 | T:6923 |
| Party: Break / Regroup | P | works, 1990 | T:6923 |
| Game: Restore / Quit / Sound | G, arrows, Enter; Restore lists the saved positions: arrows, Enter, Esc | works, 1990 | T:6923; `services.rs` `positions` |
| Quit menu | Esc | works, 1990 | T:6923 |
| Character record | F1-F5 | works, 1990 | T:A8F7 |
| Save | Starport list: arrows to Save, Enter; then the position list: arrows, Enter (a used position asks Overwrite: No / Yes) | works | `ground/starport/services.rs` `positions` |
| Quick-save / quick-load | F6 / F9 on the ground map (File > Quick-save / Quick-load); F9 asks Yes / No (No first). File > Save... / Load... open the position list on the ground | works (product) | `mt1/src/save.rs` `key` / `resume` | Not in a fight, not in flight (the game says so). F9 with the 1990 debug flag on stays the debug cheat. |
| Menus and lists | Arrows, Enter, Esc | works | `panels/choices.rs` (gunner side menu uses the keys below) |
| Amount / text box | Type, Backspace, Enter; Esc cancels | works | Focus starts in the box; no mouse click needed. |
| Casino slots | Space spins, Esc leaves | works | Reels take about three seconds; Esc during a spin leaves after its result. |

## Gunner stations

| Function | Key | State | Note |
|---|---|---|---|
| Turn the gun | Left / Right | works | Both gunner stations, S:9FDB; each press turns one bearing. |
| Target | T | works | Enters aim mode; the running Target program aims at a ship in view. |
| Move the aim | Arrows | works | S:955C keeps the 1990 reticle controls. |
| Fire | Enter, F or T while aiming | works | Fires one shot, then leaves aim mode as in 1990. |
| Gunner side menu | PgUp / PgDn, Space | works | Select previous / next row, then activate it. Arrows and Enter always control the gun. |
| Back | Esc | works | Leaves aim mode or the gunner station. |

## Reading the screen

| Function | Key | State | Code | Note |
|---|---|---|---|---|
| Log, one line older / newer | [ / ] | works | `panels/layout.rs`, `panels/combat_log.rs` | Bounded at both ends, including the extra status/hint rows in rounds 1-3. |
| Station screens: switch station | V, then F1-F8 in the chooser (Pilot, Gunner 1, Gunner 2, Computer, Engineering, Communications, Medical, Crew), or click the station's icon in the strip | works | `space/input.rs` 4707 arm, `stations.rs` 1C1C | the icon click is a product scan (60h + n) that becomes V and the chooser's F key (F1-F5 alone are the 1990's crew records on these screens). The In-sys travel box is Back (Esc); Assign / View / the tactical X and every station control answer clicks through 4707's hit tables. |
| Long log | Shift+L | works | `window/hosts.rs` (key 76) | plain L is Light; L used to expand the log as well, which shrank the map to its seven rows. |
| Follow the selected Traveller | Home | works | `panels/viewport.rs` | |
| Help: this table in game words | ? | works | `window/plus.rs` | Every product screen, including menus and prompts; keyboard table first, Esc closes. F1 stays the character record. The 57-column turn hint omits `?: help` because adding it exceeds 63 columns. |
| Zoom | none | removed | `panels/viewport.rs` | Finding 4: Z and the wheel go with the zoom. |
| Fullscreen / pixel shape / scenarios | F11 / Ctrl+S / Ctrl+Shift+L | works | `window.rs`, `window.rs` | |

## Not wired, and why

- Hide (D): says "Hide: not yet" (`keymap.rs`). Rulebook item, unbuilt with the mouse too. No 1990 function lost.
- 1990 orders mode: replaced by the turn (O / E; Space only answers the round start). `DS:94` stays 0 in product mode (`round.rs`).
- Use item, Give to (portrait menu "not yet"): the 1990 routes work: I, and P inside the F1-F5 record.
