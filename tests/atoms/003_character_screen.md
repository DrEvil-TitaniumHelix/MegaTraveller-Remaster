# T003 Open the character screen

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/003.json
ACTIONS:
  1. Click Traveller 1 in the black Commands bar at the bottom (or press F1)
  2. Click Close on the record (or press Esc)
EVALUATE:
  - The character screen for the first Traveller opens
  - Esc returns to the map
  - The party has not moved
