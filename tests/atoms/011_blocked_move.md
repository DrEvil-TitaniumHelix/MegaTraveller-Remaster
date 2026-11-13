# T011 A blocked move costs nothing

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, a wall straight to the east
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/011.json
ACTIONS:
  1. Click the wall square east of the party (or hold the East arrow for two seconds): the log says no path
  2. Click it again (or release the arrow)
EVALUATE:
  - The party does not move
  - Nothing else happens on the map
  - A West tap then turns the party west, a second West tap walks it, as 1990
