# T112 The laser gun station: this ship has no missiles

OPEN AT: The Interloper's first gunner station at Efate, Jurgen at a Beam laser, the charge gauge and Target and Fire controls visible.
SETUP KEYS: snapshot acceptance/local/atoms/112.json
ACTIONS:
  1. Read: the weapon name below the gun view
  2. Look: only Target and Fire buttons, no Reload button (a laser has nothing to reload)
  3. Click: GUN2 (station strip)   [V, Right, Enter]
  4. Read: 'GUN2: no weapon fitted' (title line) and 'A shipyard can fit one.' (in the gun view); no weapon, no tubes, no buttons
EVALUATE:
  - The first gun is a Beam laser, with a charge gauge instead of missile tubes
  - The laser station has Target and Fire only; there is no Reload button and no missile tubes
  - The second gun bay is empty; this ship has no missile rack fitted. T200 separately checks a real missile launch.
