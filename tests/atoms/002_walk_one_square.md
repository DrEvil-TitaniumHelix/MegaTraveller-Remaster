# T002 Walk one square

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/002.json
ACTIONS:
  1. Click the square west of the party (or tap the West arrow once)
EVALUATE:
  - The party moves one square west
  - The party stops when the key is released
  - No fight starts
