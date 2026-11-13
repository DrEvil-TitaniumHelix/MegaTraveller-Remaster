# T147 Enter the Arrica holding facility

OPEN AT: Arrica on Uakye, on the path three squares west of the holding facility entrance. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/147.json. Scenario: tests/atoms/scenarios/atom_world_arrica.toml.
ACTIONS:
  1. Read "place: Arrica" on the top line of the location panel at the right
  2. Click two squares east along the path [hold Right for two squares]
  3. Click the holding facility entrance one more square east [Right]
  4. Read "in the Arrica Holding Facility" under place: in the location panel at the right, and look around
EVALUATE:
  - Arrica has green ground and paths between its buildings
  - Entering opens the walled Arrica Holding Facility, with rooms and people inside
  - The party stays visible and the people are peaceful in this prepared visit
