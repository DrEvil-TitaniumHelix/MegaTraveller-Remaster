# T005 Save and load

OPEN AT: The starport screen, its list of services showing
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/005.json
ACTIONS:
  1. Click save in the Disk row (or press Right once so Save is lit, then Enter)
  2. Wait: nothing asks for a drive. Game saved. shows for a few seconds and the starport list returns on its own
  3. Click Leave on the title bar (or press Esc) to leave the starport
  4. Click the square east of the party (or walk one square east)
  5. Click Game in the list at the lower right (or in the black Commands bar at the bottom), then Restore (or press G, then Enter on Restore); nothing asks for a drive
EVALUATE:
  - No drive is asked for: Game saved shows
  - The party is outside after leaving
  - Restore puts the game back at the starport screen where it was saved
