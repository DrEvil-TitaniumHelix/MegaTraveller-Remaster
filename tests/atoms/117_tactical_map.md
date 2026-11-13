# T117 Open and close the tactical map

OPEN AT: The full-screen tactical map during an encounter in the Efate system, the Interloper and the other ship on the hex grid, a close box at the upper left.
SETUP KEYS: snapshot acceptance/local/atoms/117.json
ACTIONS:
  1. Click the close box or anywhere on the map [Esc]
  2. Click the small map's upper-left X box to open the full map again [X]
  3. Click the close box [Esc]
EVALUATE:
  - The map fills the game picture and shows the ships on a hex grid
  - Closing it restores the Pilot station
  - Opening and closing it again restores the same station without changing the crew assignments
