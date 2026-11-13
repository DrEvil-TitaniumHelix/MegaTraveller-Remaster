# T142 Enter the Neaera research station

OPEN AT: Neaera in the Boughene system, on the approach two squares west of the research station entrance. Magellan holds the station passcard. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/142.json. Scenario: tests/atoms/scenarios/atom_world_neaera.toml.
ACTIONS:
  1. Read "place: Neaera" on the top line of the location panel at the right
  2. Click the approach one square east [Right]
  3. Click the station entrance one more square east [Right]
  4. Look around the first room
EVALUATE:
  - The outdoor approach crosses pale ground to a walled research station
  - Entering reveals the station floor, walls and corridors; the map stays visible
  - Station personnel are peaceful for this visit; the party keeps its passcard
