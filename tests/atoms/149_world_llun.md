# T149 Enter Llun startown

OPEN AT: Llun on Efate, on the approach two squares west of the entrance to its enclosed startown. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/149.json. Scenario: tests/atoms/scenarios/atom_world_llun.toml.
ACTIONS:
  1. Read: 'place: Llun' (location panel, right)
  2. Click: one square east   [Right]
  3. Click: the startown entrance, one more square east   [Right]
  4. Read: 'in the interior Startown of Llun' under place:
EVALUATE:
  - Llun has rough outdoor ground around the walls of its enclosed town
  - Entering opens the interior Startown of Llun, with streets and buildings
  - The party stays visible; people in the town are peaceful for this visit
