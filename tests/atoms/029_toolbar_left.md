# T029 Toolbar: Game to Vehicle

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/029.json
ACTIONS:
  1. Click Game, then Back
  2. Click Converse
  3. Click Party, then Back
  4. Click Items, then Back
  5. Click Vehicle, then Back
EVALUATE:
  - Each opens what its key opened
  - Back returns to the map each time
