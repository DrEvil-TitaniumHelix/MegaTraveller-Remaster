# T037 Combat: one click on a thug fires

OPEN AT: A fight on Efate, Magellan selected, thugs close by
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/037.json
ACTIONS:
  1. Click a thug
  2. Click the same thug again (the Attack is spent: the log says no Attack left)
EVALUATE:
  - The first click fires at it at once (one click = target + fire; the log shows the shot, the side panel reads Attack 0)
  - The second click is refused: no Attack left
