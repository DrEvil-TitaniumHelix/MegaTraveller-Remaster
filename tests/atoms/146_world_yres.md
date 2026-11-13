# T146 Enter the hotel on Yres

OPEN AT: Yres, beside the hotel in the starport town. Its entrance is two squares north and one west of the party. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/146.json. Scenario: tests/atoms/scenarios/atom_world_yres.toml.
ACTIONS:
  1. Read "place: Yres" on the top line of the location panel at the right
  2. Click two squares north along the hotel wall [hold Up for two squares]
  3. Click the hotel entrance one square west [Left]
  4. Read "in the Starfarer's Home on Yres" under place: in the location panel at the right, then click three squares west into the lobby to look for its clerk [hold Left for three squares]
EVALUATE:
  - Yres has roads and buildings around the Starfarer's Home hotel
  - Entering shows a furnished hotel interior with its clerk and the party
  - The hotel's name identifies the Starfarer's Home on Yres; the locals are peaceful for this visit
