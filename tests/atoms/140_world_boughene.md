# T140 Walk into Boughene starport

OPEN AT: Boughene, on the pavement southeast of the starport entrance. The entrance is two squares north and one west of the party. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/140.json. Scenario: tests/atoms/scenarios/atom_world_boughene.toml.
ACTIONS:
  1. Read "place: Boughene" on the top line of the location panel at the right
  2. Click the pavement two squares north [hold Up for two squares]
  3. Click the starport entrance one square west [Left]
  4. The police ask to search for illegal weapons: click Yes
EVALUATE:
  - Paving and solid starport walls border the green ground outside Boughene; the party is visible on the pavement
  - The party walks to the entrance and the Boughene starport services open
  - The local people are peaceful in this prepared walk
