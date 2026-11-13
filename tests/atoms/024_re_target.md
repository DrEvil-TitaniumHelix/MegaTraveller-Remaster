# T024 A second click adds a waypoint

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/024.json
ACTIONS:
  1. Click a far paved square to the south-west
  2. At once click a different paved square
EVALUATE:
  - The party finishes the walk to the first square
  - Then it walks on to the second square and stops there
