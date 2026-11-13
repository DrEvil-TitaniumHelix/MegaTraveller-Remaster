# T148 Enter Imperial Security on Alell

OPEN AT: Alell, on the approach two squares west of Imperial Security HQ in the starport area. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/148.json. Scenario: tests/atoms/scenarios/atom_world_alell.toml.
ACTIONS:
  1. Read "place: Alell" on the top line of the location panel at the right
  2. Click one square east along the approach [Right]
  3. Click the headquarters entrance one more square east [Right]
  4. Read "in the Imperial Security HQ" under place: in the location panel at the right, and look around
EVALUATE:
  - Alell has green ground and paths around the headquarters buildings
  - Entering opens the Imperial Security HQ with its interior walls and staff
  - The party remains visible and the staff are peaceful for this visit
