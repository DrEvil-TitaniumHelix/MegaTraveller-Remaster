# T021 The pointer goes anywhere

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/021.json
ACTIONS:
  1. Move the mouse over the whole map, every edge and the middle
  2. Move it over the portraits and side panels and back
EVALUATE:
  - The pointer goes wherever it is moved
  - It is never pulled to the edge or the side panel
  - The square under it is outlined
