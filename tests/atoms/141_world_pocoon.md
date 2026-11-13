# T141 Walk through Pocoon

OPEN AT: Pocoon in the Boughene system, one square west of the local starport entrance. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/141.json. Scenario: tests/atoms/scenarios/atom_world_pocoon.toml.
ACTIONS:
  1. Read "place: Pocoon" on the top line of the location panel at the right
  2. Click one square west of the party [Left]
  3. Click the starport entrance two squares east [hold Right for two squares]
  4. If the police ask to search for illegal weapons: click Yes
EVALUATE:
  - Pocoon has a compact group of buildings and paved passages beside the road south
  - The party walks between the buildings and entering the local starport opens its services
  - Local people remain present and peaceful in this prepared walk
