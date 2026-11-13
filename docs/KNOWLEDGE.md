# KNOWLEDGE — how to play MegaTraveller 1 (the one cited knowledge file)

Feeds BOTH the AI player (`skills/mt1-play/SKILL.md` is
its short form) and the Plus **Help** tab (§H, then §1-§9) and **Journal** tab (§J).

Clean-room / BYOC: everything below is our own words; quotes are a few words at most; no game text, no book excerpt at
length, no original bytes. Facts carry a source key. Where the 1990 books and the as-built code disagree, **the code wins
for the remake** and the disagreement is stated.

## Source keys
| key | source | how cited |
|---|---|---|
| **M** | Paragon 1990 *MegaTraveller 1: The Zhodani Conspiracy* user guide, `MT1_manual.txt` (+ .pdf) | `M p.N` = the `=== PAGE N ===` PDF page marker (not the printed folio) |
| **CB** | Paragon 1990 MT1 cluebook, `MT1_cluebook.txt` (+ .pdf, _alt) | `CB p.N` = PDF page marker (printed folio is ~3 lower) |
| **MAP** | `MT1_map.jpg` (GDW Spinward Marches map; all 8 systems in Regina subsector) | hex readings unverified |
| **D** | decode repo (`MT1_ORACLE`) `notes\*.md` (read-only oracle) | `D file:line` |
| **B** | this repo (as built at 4eed259) | `B path:line` |

Systems are numbered by the code's `system_names` table: **0 Alell, 1 Boughene, 2 Efate, 3 Louzy, 4 Menorb, 5 Pixie,
6 Uakye, 7 Yres** (D MT1CTL.md:271; party byte 0x006, D data_sav.md:30). Map cells are `[row, col]` = `[y, x]`.

---

## H. Help keys per screen (what the Help tab shows FIRST)

The Plus Help tab picks its first card from the current `ScreenKind` (`B crates/mt1/src/ui/scene.rs:18`); Ground has
sub-contexts the code already distinguishes by memory flags (names below are the flags the Help tab can test).
Remake = the default build; **faithful** = `--faithful` (1990 keys only, no Plus at all).

| ScreenKind / sub-context | test | first Help card |
|---|---|---|
| `Startup` | scene.rs:22 | Title: any key = continue, Esc = quit (B crates/mt1/src/screens/title.rs:25-27). The 1990 copy-protection quiz is removed (D STATE.md:84). |
| `Cg` | scene.rs:24 | Main menu: Start New Game / Continue Old Game / **Get Started Quick** / Return to DOS (B crates/mt1/src/chargen/pool.rs:16-27). Every CG menu: arrows (wrap), Enter accept, Esc back, initial-letter hotkeys, mouse click (D spec_chargen_menu.md:22,84-86,116). Known gap: CG skill-reduction keys and PgUp/PgDn skill scroll are not built (B docs/KNOWN_DIFFERENCES.md:41). → §2 |
| `Intro` | scene.rs:28 | New-game intro pages: any key pages on. |
| `Ground` (grouped, walking) | DS:6D0E `combat()` = 0 | Arrows / keypad = 8-way move (Home NW, PgUp NE, End SW, PgDn SE). **C** converse (adjacent + facing), **I** items (Take/Use), **V** vehicle (Leave/Status), **P** party (Break/Regroup), **G** game (Restore/Quit/Sound), **Esc** quit menu, **O** swap marching slots, **F1-F5** character record (D input_map.md:45-79). Remake extras: **Home** = follow selected Traveller, **L** light on / off, **Shift+L** long log, F11 fullscreen, Ctrl+S pixel shape, Ctrl+Shift+L scenario picker (B crates/app/src/window/hosts.rs:264-267; panels/viewport.rs:197-202; window.rs:116-120,206-210). Numpad7 / Ctrl+Home still = NW step (window.rs:385). → §4 |
| `Ground` squad fight (remake default) | squad on iff `!faithful && !classic_combat` (B crates/app/src/main.rs:221); encounter active | Round start: **Space/O** = we go first, **N** = they go first. Per turn: **1-5** select, arrows step (Move points), **F/Enter** fire at target, **Tab/Shift+Tab** cycle targets nearest first, **Q** semi/auto, **R** reload, **=** swap hand, **keypad +/-** tactical points, **H** hold here, **Shift+H** hold (interrupt), **Ctrl+H** hold fire, **M** / right-click verb menu, **O** continue standing orders (Space answers the round start only); **End turn** button forfeits, **Esc** backs out one level. **T** grenades, **C/I/V/G/F1-F5** stay 1990. Left-click = default verb (select / move / fire / take / converse). **D** (hide) not built. No saving during a fight. (B crates/mt1/src/ground/keymap.rs:17-80; round.rs:260-273; orders.rs:66-80; docs/combat/DESIGN.md:137-140; ground/squad.rs:27,32) → §5 |
| `Ground` 1990 real-time fight (faithful, or File > Options > "Classic 1990 combat") | broken + classic | **P > Break** first (refused in the starport area or a vehicle). **1-5** select, **F** twice = target then fire, **Tab** cycle target, **R** reload, **T** grenade (E/C/R type), **=** / keypad + swap hand. **O** = orders mode: the ONLY pause; time stops, give each Traveller a destination / target, **O** again runs it (D input_map.md:49-65; main_loop.md:26-29; M p.9, p.60-61). → §5.2 |
| `Ground` starport panel | DS:E9 `t.event` 9 or 10 (B docs/HARNESS.md:487-488) | Arrows move, Enter invokes, Esc leaves; mouse first click selects, second invokes (D input_map.md:147). Items: info, Save, bank, pensions, recruit, training, cargo buy / sell, refuel (**F** +1 / **A** all affordable / **P** pay), repair, ship weapons buy / sell, supplies (B crates/mt1/src/ground/starport/mod.rs:134-142; D input_map.md:148-149). Lists: Up/Down, +/- quantity, Enter, Esc (D input_map.md:80-86,153). → §7 |
| `Picture` (shops, offices, casino, police) | scene.rs:20 | Shop lists as above. Casino: any key but Esc spins (100 Cr), Esc leaves (D input_map.md:151-152). Amount fields: digits, Backspace, Enter / Esc (D input_map.md:87-89). → §7 |
| `Record` (F1-F5 character record) | scene.rs:30 | Esc close; 1-8 / Up / Down action; Enter or V detail; **P** pick item (then open another record = give, or Esc = drop on ground), **M** move, **U** use / equip, **Q** unequip; F7-F10 equip slots; PgUp/PgDn skill pages (D input_map.md:81-101; M p.10). Time is frozen here (M p.14, p.52). |
| `Space` flight | scene.rs:32 | Up/Down thrust, Left/Right turn, keypad 5 brake; **T** take off, **L** land (at a port world), **J** jump screen, **S** stations, **C** comms, **Q** quit (no save). Esc / I = back to flight (not a pause) (D input_map.md:97-109; M p.9-10). → §8 |
| `Space` stations | station id DS:0399 (D MT1CTL.md:280) | **A** assign, **V** view another station, **Esc/I** leave, **X** full-screen tactical map, **F1-F5** records. Computer **L** load / **R** run / **S** stop / **M** model; Engineering **R** repair; Medical **H** heal / **R** return to duty; Gun **T** aim, **R** reload missiles, Left/Right rotate, arrows move the reticle, Enter/F/T fire; Comms Enter / Up / Down / **X** identify; boarding **T** take cargo, **E** leave (D input_map.md:111-138; M p.10-11). → §8 |
| `Space` jump screen | — | Arrows / 1-8 / F1-F8 / click pick a lit system, Enter jumps, Esc cancels (D input_map.md:110; M p.10). → §8.3 |

Save / restore everywhere: **Save** only from the starport panel (item 2); **Restore** from the ground **G** menu or CG
"Continue Old Game". One slot, `saves\game1.mtsav` (B crates/mt1/src/save.rs:1-16; docs/QOL.md:36-45; ARCHITECTURE "Save
format"). The 1990 manual's 30 named slots are not carried (M p.8); slots / quick-save are M8 candidates (B docs/QOL.md:24-45).

---

## 1. How to play (the shape of the game)

- You run a party of **five** ex-service Travellers (M p.5, p.23). The game starts and ends on **Efate** (M p.64).
- Two halves: **ground** (overhead map of a world, real time in 1990, turn-based squad fights in the remake) and **space**
  (the Interloper flying in a star system, stations, jumps between the 8 systems) (M p.32, p.50).
- Moving between them: on the ground, step on the ship / launch tile (event 8) to go to space; in space, park the ship's
  circle over a port world and press **L** to land (B docs/KNOWN_DIFFERENCES.md:45 B6; M p.33).
- The **starport building** (event 10) is where you save, bank, recruit, train, trade cargo and outfit the ship (M p.43;
  B starport/mod.rs:134-142). The manual's advice: save every time you land or leave (M p.8, p.43).
- Talking to everyone is how the plot advances (M p.53, p.58); paid tips are mostly useless for the plot (CB p.11).
- Remake mouse: left-click = default verb on what is under the pointer, right-click = verb menu (B docs/combat/DESIGN.md:137-140).
  The 1990 mouse: left = Enter, right = Esc, plus an on-screen direction pad (M p.8; D main_loop.md:13-14).

## 2. Character generation

**Quick start (recommended for the AI and the first test):** CG main menu **Get Started Quick** loads DEFAULT.SAV: five
pre-made Travellers (first is "Magellan") on **Efate**, the ship **Interloper**, fuel 125, a jump-1 drive (B pool.rs:13,24;
main.rs:270-273; D data_sav.md:28-55; D MT1CTL.md:137 "alell, louzy, uakye for the jump-1 drive"). **They arrive under fire**: see §9.1.

Rules (M p.21-31):
- Six attributes 1-15 shown as hex (A=10..F=15): STR DEX END INT EDU SOC (M p.22). **INT+EDU = total skill levels allowed**
  (M p.23, p.27); **life force = STR+END+DEX** (M p.23). Re-roll attributes freely before enlisting (M p.23, p.31).
- Five services: Army, Navy, Marines, Scouts, Merchants (M p.24). Enlist (may be refused -> draft) or take the draft.
- Each term: skills (Scouts 2, others 1, +1 per promotion), reenlist roll, possible injury (operation can kill) (M p.25).
  Aging from term 5 costs STR/DEX/END; INT drops after term 9 (CB p.5; M p.25-26). **Characters can die in generation** (M p.25).
- Mustering out: one benefit per term plus rank bonuses; **at most 3 cash draws** (M p.26). **TAS membership and Imperial
  Release only come from mustering out** (M p.27, p.60). Take an Imperial Release where offered: it ends an arrest (CB p.6-7, p.12).
- EDU 8+ unlocks the Advanced Education table (M p.31). Pool of up to 15 characters; recruits come from the pool (M p.21, p.31).

Good builds (CB p.4-6):
- Fighters (Marines / Army / Scouts): STR 10+, END 10-11, DEX 8+. Everyone: END 9+ (oxygen), DEX 8+, INT and EDU 7+, SOC 6-7
  (high SOC hurts haggling, low SOC hurts talking) (CB p.4).
- Scouts: best cash and benefits, almost always accepted, no ranks; Army: easiest entry, best ground mix, lowest cash;
  Navy: ship flying / gunnery; Merchants: Trader; Marines: ground combat, worst surgery survival (CB p.5).
- Skills that matter (CB p.6): ATV, Bribery, Communications, Computer, Engineering, Grav Vehicle, Heavy Weapons, Vacc Suit,
  Laser Weapons, Medical, Navigation, Pilot, Spinal / Turret Weapons, Stealth, Trader, Watercraft; plus gun skills, Leader,
  Tactics, Streetwise, Gambling, Liaison, Carousing. **25 skills do nothing in MT1** (Admin, Aircraft, Combat Engineering,
  Electronics, Forgery, Grav Belt, Gravitics, High Energy Weapons, High-G, Interrogation, Intrusion, Jet Aircraft, Linguistics,
  Mechanical, Recon, Robot Ops, Robotics, Sciences, Sensor Ops, Screens, Ship's Boat, Steward, Survey, Survival, Zero-G):
  never spend INT+EDU on them (CB p.5). Remake note: squad surprise uses Leader+Recon (B round.rs:389-405), so Recon is not
  dead in the remake's squad mode.
- Party roles to cover: two or three gunfighters, a pilot, a navigator / computer operator, an engineer, gunners, a medic, a
  trader (Trader or Streetwise 4+), a briber; Gambling 2+ if you want the casino (CB p.6, p.11, p.16; casino EV in §7.4).
  Match station and leader to skills: the leader's skills are used for talking and trading (M p.35, p.42, p.51, p.58).

## 3. The Zhodani Conspiracy — the plot thread in order

Premise (M p.20, p.66-67): Konrad **Kiefer**, Regina chief of the Sharurshid megacorp, is secretly running arms for the
Zhodani. **Lenara Raclor** of Transom (Sharurshid's covert audit arm) found out; she gave the party Kiefer's encoded file,
the Interloper and some money, and sent them to Boughene to meet **Dr. Arik Toryan**, who built Kiefer's two-key encoding.
Kiefer's man **Viktor Quaam** kidnapped Arik. Goal: rescue Arik, follow the keys, stop Kiefer. **You must rescue Arik to
win** (M p.67); **at least one original party member must survive** to finish (M p.43).

The cluebook's own scenario list has 6 steps (CB p.21); expanded with its NPC and object lists (CB p.17-21):

1. **Money for the jump-2 drive (~2 MCr).** Boughene is not reachable on the stock jump-1 drive (jump-1 from Efate lights
   Alell, Louzy, Uakye only: D MT1CTL.md:137). Fastest money: the ten assassins (1.375 MCr, §7.3), object rewards, trade
   (CB p.13, p.16, p.18, p.21; M p.41).
2. **Prepare for three required fights** (Viktor, Arik's captors, Kiefer): a med kit, a torch, demolition charges, lots of
   ammunition (CB p.11-12). Only those three fights are needed to win (CB p.12).
3. **Boughene (system 1): kill Viktor**, NE of the starport. He carries the passcard for the Neaera station (CB p.11-12, p.17, p.21).
4. **Neaera (Boughene system): enter the research station** S of the starport with Viktor's passcard. The man in the SE room
   carries the key to the lower floor (CB p.10, p.19).
5. **The 5-minute clock**: once inside you have about five minutes to reach Arik, or he is moved (CB p.17).
6. **Lower floor fight**: four men, one of them Arik (CB p.12, p.19).
7. **Show Arik your half of the Imperial Seal**; he gives the other half (CB p.17, p.21). **Before leaving Neaera, take the
   key on the floor** — without it a moved Arik cannot be reached (CB p.17).
   - Late at Neaera: Arik is in the **Chiros** ice warehouse W of the starport (Pixie system); five more minutes; **take the
     passcard on that floor** (CB p.10, p.17, p.20).
   - Late at Chiros: Arik is in the **Streech** cave NW of the starport (Pixie system), final move, six armed men (CB p.10, p.17, p.20).
8. **Yres (system 7) hotel clerk**: show the completed seal; get the pass / decoding key for Lenara (CB p.17, p.20-21).
9. **Lenara**: show her the clerk's key; she gives the passcard to Kiefer's Efate warehouse. Where: the cluebook says Cheika
   (CB p.17, p.21) and also the **Akarates warehouse** W of the starport (CB p.20); only Akarates has a warehouse (CB p.10).
   Try Akarates first, then Cheika (both Yres system).
10. **Back to Efate**: the warehouse in the starport city (CB p.10, p.17, p.21).
11. **Final fight: Kiefer and his gang** — the hardest fight; winning it ends the game (CB p.11-12, p.17, p.21).

Commonly lost: the Neaera floor key, the Chiros floor passcard, ammunition before the set fights, oxygen (CB p.11, p.17).

## 4. Ground

- Party moves as one icon; it splits into five for combat (M p.51). Enemies can shoot the grouped icon: a random member is
  hit (M p.60). Over-weight Travellers slow down progressively (M p.52).
- Terrain (M p.54-55): foot is fine on grass, farmland, roads, paths; swamps impassable except paths; mountains impassable to
  everything (grav only over low ones); cracks need ATV or grav, crevices / trees / large craters grav only; water needs a
  bridge, hydrofoil or grav.
- Vehicles (M p.56, p.59; CB p.8, p.11): rented (fee + deposit; deposit lost if destroyed or abandoned). ATV cheap, grav best
  (carries 10, armoured, fast), hydrofoil on water worlds (Iroth has only watercraft). Rent whenever you can: faster, crosses
  terrain, and absorbs hits instead of people (B combat.rs:176-183). Rental hotkeys A ATV, B grav, G boat (D input_map.md:159-208).
- Buildings: enterable ones have a walkway to the door; same type looks the same everywhere; dark ones need a torch / lantern;
  reinforced doors yield to a demolition charge (M p.58; item 51 breaches the facing door, B docs/combat/ASBUILT.md item table).
  Most key buildings are in the startown just outside the starport (M p.58).
- Talking: leader adjacent and facing, **C**, Talk or Bribe (1,000 / 5,000 / 10,000 / 25,000 / 50,000 Cr) (M p.52). Refusals
  can be haggling; earlier bribes are not wasted (M p.58). Failed bribes still cost money (D spec_services.md:479-486).
  To give an NPC an item the **leader holds it in the primary hand**; rewards go to the leader (M p.9).
- Passcards work from a hand slot; card colour lines must match the door's (M p.9). Item ids 107-125, 127-129 are passcards /
  keys, 130 a master key (B docs/combat/ASBUILT.md:806-808).
- Law levels 0-D are cumulative weapon bans (M p.57). The starport gate confiscates illegal weapons (returned on leaving); **one
  weapon in the secondary slot may be smuggled** (odds by size and skill) (M p.53, p.57; CB p.11). As built, arrest needs law
  8+, outdoors and two provocations; bail = (law-8) x 4,000 + 10,000 Cr, or an **Imperial Release** (item 57) (B
  crates/mt1/src/ground/starport/police.rs:18-43,86-140). The cluebook's "law 6+" is the 1990 book's figure (CB p.11-12).
  Law by world (CB p.11-12): Efate 0, Boughene 1, Pixie 3, Menorb 8, Louzy 8, Uakye 8, Yres 7, Alell C (12), Quan A, Arrica B.
- Healing (M p.55-56; CB p.12): hospital (best, priciest; 13 worlds), **med kit** (most efficient; TL9 item 45, TL12 item 46;
  usable after fights), ship sickbay (slow, needs a medic at the medical station), speed-healing drug (one dose per
  character, full heal; any general store).
- Oxygen: vacuum without a sealed suit **and** helmet kills at once; tainted / exotic air without a respirator or helmet hurts
  steadily; a spare oxygen tank (item 54) refills to full (M p.55; B walk.rs:691-751; ASBUILT item 54).

## 5. Ground combat

### 5.1 Remake default: squad turns (M8A; B docs/combat/RULES.md, DESIGN.md, ASBUILT.md)
- A fight starts when an armed willing NPC within 29 squares sees a Traveller, or the party makes a hostile move; the world
  freezes between decisions; a round is 6 s (B DESIGN.md:131-134,202; round.rs:16).
- **Surprise** once: 2D + attackers' best Leader+Recon − defenders' ≥ 11 → attackers surprise; ≤ 9 → defenders (B RULES.md:55-62).
- **Initiative** each round: the side with the larger Tactics pool picks who goes first; tie → party (B RULES.md:63-70).
  Sides alternate one unit at a time (B RULES.md:72-74).
- A turn = Move (walk 10 MP, run 20; diagonal 1) + one Attack, either order. Reload, fire-mode switch, take / use / converse
  replace the attack; swapping hands is free. Heavy recoil weapons fire only if you did not move (B DESIGN.md:205-211; round.rs:22).
- To-hit bands by Chebyshev distance: 0 close, 1-2 short, 3-29 medium. Handgun difficulty 3 / 7 / 11, rifle 7 / 7 / 11,
  thrown 7 / 11 / 15; cover, rapid fire, unskilled each +1 level. DM = weapon skill + DEX/5 + movement DM + tactical points
  (clamped ±8); a natural 2 always misses (B RULES.md:142-215). Jack-of-all-Trades does nothing in combat (B RULES.md:218-219).
- Damage multiplier by margin (0 graze ×½ … 8+ ×8), cut by armour vs penetration (B RULES.md:302-330). Hits ≤ split →
  unconscious for the fight; 0 → dead (gear dropped, party account −1/5) (B RULES.md:335-357; combat.rs:229-258).
- Holds / interrupts: an unacted unit, 2D + declared speed ≥ 7 (B DESIGN.md:212). Tactical points: up to the side's best
  Tactics, renewed each round (B DESIGN.md:213).
- End: open fights end one round after no willing armed NPC is in sight; **locked story fights** end only when the hostiles
  are dead / routed, the party is down, or a Traveller ends a turn in the escape zone; engaged NPCs stay alerted 10 rounds.
  **No saving during a fight** (B DESIGN.md:217-219; squad.rs:27,32).
- Balance: grenade range one row friendlier (~5 squares); NPC autofire only at
  ≤ 5 squares or a boxed target.

Tactics that work (CB p.12; M p.60-61): keep non-fighters out of the line of fire; split behind cover; with no cover, fight
from range or leave; reload between fights; use a med kit right after.

### 5.2 1990 real time (`--faithful`, or "Classic 1990 combat")
- Nothing waits for you; NPCs act every ~1 s; each armed hostile in sight attacks every heartbeat (B docs/combat/ASBUILT.md:17-40,439-458; M p.60).
- **P > Break** to fire back (refused in the starport area and in vehicles) (B ASBUILT.md:111-130). Party to-hit =
  1d6 + weapon skill + range class + JoT/2 ≥ 5; range by distance only (M p.62 says no range modifiers; the code has four
  classes, B ASBUILT.md:268-296). **In orders mode (O) every shot hits** (B ASBUILT.md:295-296): orders mode is the 1990
  pause and the strongest tool.
- Armour cuts damage, not hit chance (M p.61). Life bar below its white line = unconscious, still losing points; empty = dead
  (M p.51, p.61). **If every Traveller is unconscious the party dies** (M p.61; B walk.rs:683-690).

## 6. Starport guide

- Efate's starport door is event 10 at cell [33, 3], verified from W.DAT chunk 7 map 0 via the oracle decoder (B tools/m8v_docs_audit.py).

- Class decides services (M p.39): A-D everything; F disk / bank / personnel / cargo; G disk / bank / cargo; H disk only;
  X pad only. You can land only where there is a port (M p.33). **Shipyards (refuel / repair / weapons) only at the 8 main
  worlds** (CB p.13). Port tiers per system (CB p.13): Efate U, Llun L, Kra / San / Stur VL, Solon none; Alell U, Iroth L;
  Menorb U; Uakye U; Louzy U, Quan L; Boughene U, Pocoon L (no recruiting), Neaera VL; Pixie U, Chiros L (no recruiting),
  Streech VL; Yres U, Akarates / Cheika / Chayke VL.
- Panel items as built (B starport/mod.rs:134-142): 1 info, 2 save / restore, 4 bank, 5 pensions, 7 recruit, 8 training,
  10 cargo buy, 11 cargo sell, 13 refuel, 14 repair, 15 sell ship weapon, 16 buy ship weapon, 17 supplies (programs, missiles).
- **Bank**: keep most money in the party account; bribes need cash in hand (M p.43). **Pensions**: 5+ terms pay 2,000 Cr
  per term monthly (M p.25). **Recruit** from the pool to replace losses; **training** improves skills you used (M p.27, p.43).
- **Refuel**: refined or cheaper unrefined; unrefined raises misjump odds (M p.43; D spec_trade.md:140-143).
- **Outfitting order** (CB p.13-14; M p.44): jump-2 drive (plot gate, §3 step 1) → TL13 beam laser (the stock TL8 takes heavy
  damage) → missile launcher + constant-guided missiles (cheap, often one-shot kills). Programs you need: Maneuver (turning),
  Target (lasers), Launch (missiles), Navigation + Jump-1 / Jump-2 (jumps). The Model 2 computer you start with is fine;
  Model 2/bis runs 6 programs at once.
- Shops on the ground (M p.58-60; CB p.7-10): weapons (red), armour (blue), general store (white: torches, demolition
  charges, oxygen tanks, drugs), vehicle rental, hospital, TAS (leader must be a member; free lodging and world info),
  library (pay per item), casino (low-law worlds), hotels, bars, museums (artifact buyers). Office events: 11 vehicles,
  12 casino, 13 library, 14 TAS, 15 hospital, 16 weapons, 17 armour, 18 general, 19 Imperial office (B starport/mod.rs:161-171).
- Where it matters (CB p.7-10): unlimited weapons at Efate (grav city NE) and Arrica; unlimited armour at Efate, Boughene,
  Pixie, Uakye; grav vehicles at Efate, Alell, Boughene, Pocoon; the **Imperial Base N of the Alell starport pays all
  assassin bounties** (CB p.19).

## 7. Money and trade

### 7.1 Cargo trade (as built: B crates/mt1/src/ground/starport/cargo.rs; D spec_trade.md:10-19,91-161)
- 30 goods, 20 hold slots (M p.42-43; B cargo.rs:7-9). Each world has a demand row, one special good and one banned good.
- **Brokerage** = 2 × Trader + Streetwise/3 + Carousing/3 + Liaison/3 of the selected Traveller (B starport/mod.rs:40-47).
  Buy price = base × max(90, 100 − brokerage)% + demand × 50 (+ special); sell % = min(90, brokerage + 80) (B cargo.rs:8-42).
  So Trader 5 already reaches the best sale percentage; pick your trader as the payer.
- Big 1990 spreads (CB p.14-16, read as the world's price, buy low / sell high): Streechen wine Pocoon ~1.8k → Boughene ~7.5k;
  computers Pocoon / Quan ~2k → Chiros 5.6k; pharmaceuticals Louzy ~2.1k → Llun 5.6k; hi-tech parts ~2.2k → Yres 5.8k;
  livestock Chiros 50 → Uakye 4k; water ~50 → Louzy 3.4k. Treat these as hints; the formula above is the truth.

### 7.2 Object rewards (CB p.18-21; the reliable early money)
NPCs pay fixed rewards for wanted items, whatever your skill (CB p.11). Best buyers: coynes 50k (Stur museum), gold statue 50k
(Iroth museum), jeweled saber 50k (Pixie museum), groatle fur 40k (Ilantir), ruby necklace 30k each (Uakye museum), Solomani
musket 25k (Iroth), starrghrite 20k (Efate museum), gold pendant 20k (Alell bar), bronze sculpture 20k (Boughene bar), howood
staff 20k (Sino), diamonds 15k (Efate museum). Louzy terrorists: 100k from an Imperial soldier, proof = the terrorist symbol
(item 94) (CB p.18-19). Efate freebies: emeralds in the mountains N of the starport; rubies in the Starfarers Home (CB p.18).
Sell dead NPCs' heavy loot quickly (weight and slots) but don't sell artifacts to stores if an NPC pays more (CB p.11; M p.59).

### 7.3 Bounty hunting (CB p.16-18; M p.81-84)
Ten assassins, 1,375,000 Cr total; bring each one's ID card (items 97-106) to the Imperial Base N of Alell starport:
Alazar (Boughene) 250k, Viridion (Iroth) 225k, Ozar (Boughene) 200k, Zelexian (Louzy interior-city warehouse) 175k, Mortis
(Chiros) 150k, Sanai (Efate) 125k, Hamache (Llun) 100k, Bolo (Yres) 75k, Jannev (Arrica) 50k, Cephalon (Pixie) 25k.

### 7.4 Casino, piracy
- Casino: 100 Cr a spin; expected return ~34 Cr at Gambling 0, ~109 at Gambling 2, ~1,240 at Gambling 9 — **profitable from
  Gambling 2 as built** (D spec_services.md:488-508). The cluebook says 5+ (CB p.16); the code wins.
- Piracy (M p.41-42; CB p.14, p.16): scan first (Comms), skip ships that outgun you or carry nothing; disable its weapons,
  match course and speed, board, **T** take cargo, **E** leave fast — most ships self-destruct after boarding and kill anyone
  still aboard (M p.42). Heavy piracy draws escorts and the Imperial navy (M p.42).

## 8. Space

### 8.1 Launch and flight (M p.32-34; D input_map.md:97-109)
- Ground → space: step on the launch tile (Efate: event 8 at cell [31, 9], D MT1CTL.md:137; verified by B tools/m8v_docs_audit.py). Space opens docked at the port
  world; **T** takes off. Up/Down thrust, Left/Right turn (needs Maneuver running in combat), keypad 5 brakes. Speed persists
  until you brake (M p.33-34).
- Land: bring the ship's circle onto a port world, **L** (M p.33). Gas giants refuel automatically with scoops + purification
  (M p.33); scooping detail D MT1CTL.md:267.
- Hazard: parking on certain objects adds hull damage each pass; hull 100 = ship and party lost (B crates/mt1/src/space/flight.rs:238,282-283).

### 8.2 Stations (M p.35-40; B docs/SPACE_STATIONS.md)
Eight stations (pilot, gun 1, gun 2, computer, engineering, comms, medical, sickbay), one Traveller each; first boarding
auto-assigns by best skill (M p.33, p.35). The **computer is the key station**: a program must be **loaded and running**
(M p.36-37). Load time = 130 − 18 × Computer skill ticks (D spec_s_stations.md:93). Engineering repairs guns, maneuver and
computer, never the jump drive or hull (shipyard only) (M p.37).

### 8.3 Jump (B crates/mt1/src/space/jump.rs; docs/space-jump.md; D spec_s_stations.md:30-41)
- Needs, checked in this order: **Navigation** program running, a **Jump-1 or Jump-2** program running, a jump drive, fuel
  > 86 (B jump.rs:112-118; space-jump.md:38-39). Jump from anywhere in the system (M p.34).
- **Fuel 87-102 = 30% misjump** (random reachable system, even if you cancel); a jump halves fuel and adds 7 days (B
  jump.rs:149-173). Refuel to full (124) before jumping.
- Known-good path from quick start (D spec_s_stations.md:30-41): **T**, Up, Up, keypad 5 (clear of the planet, stopped) →
  **S**, **V**, **4** (computer) → **L** Enter Enter (load Jump-2), wait for the load → **R** Enter (run) → **L** Enter Right
  Enter (load Navigation), wait → **R** Enter → **Esc**, **J**, Enter. Arrives in Louzy (system 2 → 3), 7 days later, fuel
  125 → 61.
- Jump-1 from Efate reaches Alell, Louzy, Uakye (D MT1CTL.md:137). Boughene and Yres (the plot) need the jump-2 drive (§3).

### 8.4 Encounters and space combat (M p.40-42, p.49; CB p.14)
Another ship inside your circle = combat (M p.33). Run Target + Maneuver (+ Launch for missiles); have your best gunners on
the gun stations before engaging; fire until the target is disabled. Predict / evade programs help but slots are few (CB p.14).
Leave by getting off the tactical map (M p.49). Pirates target you more with a full hold (M p.42).

## 9. Common deaths and how to avoid them

1. **The quick-start ambush on Efate.** Three armed hostiles (records 14-16: a 9mm ACR, a 9mm auto pistol, a TL9 laser
   rifle) shoot from the first step (B docs/combat/ASBUILT.md:439-458). In the remake it is the locked story fight
   `efate_quickstart_ambush` (B docs/SCENARIOS.md:175, 516): fight it as a squad (break, cover, focus fire) or reach an escape
   zone; in 1990 mode use orders mode (O) — shots auto-hit there (B ASBUILT.md:295-296).
2. **Everyone unconscious = game over** (B walk.rs:683-690; M p.61). Heal early; keep a med kit.
3. **No original party member left = game over** (B combat.rs:249,489-492; M p.43).
4. **Vacuum or hostile air** without a sealed suit and helmet kills at once; oxygen running out kills; tainted air hurts
   steadily (B walk.rs:691-751; M p.55). END 9+ saves oxygen (CB p.4). Carry spare tanks into the Louzy warehouse (CB p.11).
5. **Self-destruct**: the San research station countdown (CB p.18) and boarded ships (M p.42) end the game at zero (B walk.rs:630-637).
6. **Ship destroyed** (hull 100) in combat, boarding or on a hazard (B space/flight.rs:238,282-283; space/combat.rs:88).
7. **Out of ammunition** in a set fight (CB p.11).
8. Not lethal but costly: misjump (fuel 87-102), arrest (bail or lose gear), traps like the Pocoon package ambush (CB p.20),
   deposits lost on wrecked vehicles (M p.56). No hunger or radiation sickness exists in the code (B grep of crates/mt1/src/ground: no food code; "radiation" only as a grenade type, fire.rs:108).
9. **Quit never saves** (M p.8, p.53). Save at every starport visit.

---

## J. Journal — plot entries and the game event that unlocks each

The Journal shows an entry once its UNLOCK is true. Every UNLOCK names state the code can test (party block offsets per
D data_sav.md:28-104; item ids per B docs/combat/ASBUILT.md:795-808; NPC record fields per B ASBUILT.md:410-435).
Where the exact record id of a plot NPC is not decoded, the entry says `UNLOCK: unknown` + the closest candidate. Plus
reads state only (law 2).

| # | entry (our words) | UNLOCK | source |
|---|---|---|---|
| J1 | Lenara sent you to find Dr. Arik Toryan on Boughene; Kiefer's man Viktor took him. | new game started (any party loaded; `ScreenKind::Ground` first reached) | M p.20, p.66-67 |
| J2 | The ship needs a jump-2 drive (~2 MCr) to reach Boughene. | J1 and current system = 2 (party 0x006 = 2) | CB p.13, p.21 |
| J3 | Assassins carry bounties; the Imperial Base north of Alell starport pays for their ID cards. | any assassin ID card (items 97-106) in any inventory, or party 0x006 = 0 visited | CB p.16-19 |
| J4 | Jump-2 drive fitted: Boughene is in range. | ship_flags (party 0x0C1) bits 0-1 = 2 | CB p.21; D data_sav.md:102; B jump.rs:114-118 |
| J5 | Viktor is NE of the Boughene starport; he holds the Neaera station passcard. | arrived in system 1 (party 0x006 = 1) | CB p.17, p.21 |
| J6 | Viktor is dead; his passcard opens the Neaera station. | `UNLOCK: unknown` — Viktor's NPC id not decoded. Candidate: killed bit (party 0x008 + id/8) of the Boughene NPC dropping a passcard (items 107-125); D data_sav.md:32; B combat.rs:453-457 | CB p.17, p.19 |
| J7 | Inside the Neaera station: about five minutes to reach Arik. | DS:6D0C `arik` timer set to 300 (tile 19, even row) (B crates/mt1/src/ground/walk.rs:950-956; state.rs:139) | CB p.17 |
| J8 | Arik reached. Show him your half of the Imperial Seal. | DS:6D0C `arik` cleared by tile 19 odd row (B walk.rs:948-953) — candidate; confirm with a play trace | CB p.17, p.21 |
| J9 | Arik was moved to the Chiros ice warehouse (Pixie). Take the key from the Neaera floor. | ship_flags bit 6 set by `alarm_tick` (timer ran out in system 1; pursuer NPC 0xF8) (B walk.rs:753-770) — candidate | CB p.17 |
| J10 | Arik was moved again, to the Streech cave (Pixie). Take the passcard from the Chiros floor. | `alarm_tick` timer ran out in system 5 with bit 6 set (pursuer NPC 0x12F active, DS pursuer_active) (B walk.rs:753-770) — candidate | CB p.17 |
| J11 | The Imperial Seal is whole. The Yres hotel clerk will want to see it. | item **90** (imperial seal) in any inventory; item 89 = the broken half (TRAV.ITM read 2026-10-28) | B ASBUILT.md:804; CB p.17, p.21 |
| J12 | The clerk gave you a key for Lenara (Akarates warehouse; else Cheika, Yres system). | item **87** or **88** (decoding key 1 / 2) in any inventory while item 90 is held. Candidate alternative: item **93** (token) — the cluebook calls the clerk's gift a pass in one place and a decoding key in another (CB p.17 vs p.20); confirm with a play trace | CB p.17, p.20-21 |
| J13 | Lenara's passcard opens Kiefer's warehouse in the Efate starport city. | `UNLOCK: unknown` — which passcard id is Lenara's is not decoded. Candidate: conversation state 0x1A = 0x1D2 (done) on the Lenara record, or her reward bit (party 0x03A + id/8) (B ASBUILT.md:416-433; commands_talk.rs:207-211) | CB p.17, p.21 |
| J14 | Kiefer is finished. | `UNLOCK: unknown` — no win check found in code or notes. Candidate: the killed bit of NPC **0x141** (321), whose death calls `enter_world(2, 0xBD, 0xBE, 0xBF)` = a scripted move to Efate (B crates/mt1/src/ground/combat.rs:443-458) | M p.64, p.67; CB p.21 |

Item ids checked against the player's TRAV.ITM (46-byte records after a u16 count, D data_itm.md:5-21): 84 ivory horn,
85 reptile hide, 86 holographic disk, 87 / 88 decoding key 1 / 2, 89 broken imperial seal, 90 imperial seal, 91 package,
92 handcuffs, 93 token, 94 terrorist symbol, 95 package; 57 Imperial Release; 97..106 assassin ID cards; 130 master key.

Side entries (unlock on the item / place; optional): Louzy terrorists (terrorist symbol in inventory; CB p.18-19), the ten
assassins (each ID card, items 97-106), San station self-destruct (DS countdown < 100; B walk.rs:937-940), the flag swap
(items 71 red / 72 blue; CB p.19).

Player notes: free text kept by Plus beside the entries (presentation only; never in the sim save image's party block).
