# T023 A press and drag walks nobody

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/023.json
ACTIONS:
  1. Press on a square, drag a few squares, release
  2. Press and hold the button on a square
EVALUATE:
  - The drag walks nobody
  - Holding the button walks nobody
