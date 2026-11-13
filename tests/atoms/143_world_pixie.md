# T143 Walk into Pixie starport

OPEN AT: Pixie, on the pavement northeast of the starport entrance. The entrance is two squares south and two west. The locals are peaceful for this check.
SETUP KEYS: Open snapshot acceptance/local/atoms/143.json. Scenario: tests/atoms/scenarios/atom_world_pixie.toml.
ACTIONS:
  1. Read "place: Pixie" on the top line of the location panel at the right
  2. Click the pavement two squares south [hold Down for two squares]
  3. Click the starport entrance two squares west [hold Left for two squares]
  4. If the police ask to search for illegal weapons: click Yes
EVALUATE:
  - Pixie has a compact paved starport compound beside rough ground
  - The party walks along the building and entering opens the Pixie starport services
  - The travellers remain visible together; any local people are peaceful in this setup
