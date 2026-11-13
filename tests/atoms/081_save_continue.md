# T081 Save at the starport, then Continue Old Game

OPEN AT: The Efate starport screen, 30,000 Cr in the party account, four water in the hold
SCENARIO: m7_starport_before_save
SETUP KEYS: snapshot acceptance/local/atoms/081.json
ACTIONS:
  1. Click save in the Disk row (or press Down twice to Save, then Enter)
  2. Wait: nothing asks for a drive. Game saved. shows for a few seconds and the starport list returns on its own
  3. Click Leave on the title bar (or press Esc) to leave the starport
  4. Click Game in the list at the lower right (or in the black Commands bar at the bottom), then Quit (or press Esc, then Enter on Quit)
  5. Click the title (or press Space)
  6. Press Down, then Enter on Continue Old Game (the title menu takes no clicks yet; no drive prompt: the save is read at once)
EVALUATE:
  - No drive is asked for: Game saved. shows, then the starport list again
  - Quit returns to the title
  - Continue Old Game reads the save
  - The game opens at the starport as saved: the same list, 30,000 Cr
