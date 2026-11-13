# T145 Enter the Chiros ice warehouse

OPEN AT: Chiros in the Pixie system, on the approach two squares west of the ice warehouse entrance. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/145.json. Scenario: tests/atoms/scenarios/atom_world_chiros.toml.
ACTIONS:
  1. Read "place: Chiros" on the top line of the location panel at the right
  2. Click one square east along the approach [Right]
  3. Click the warehouse entrance one more square east [Right]
  4. Look around the warehouse entrance
EVALUATE:
  - The approach crosses pale icy terrain beside the warehouse walls
  - Entering reveals a warehouse floor divided by walls and doors
  - The party remains visible; warehouse personnel are peaceful for this prepared visit
