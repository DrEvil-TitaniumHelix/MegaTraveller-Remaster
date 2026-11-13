# T045 Build a character by clicking

OPEN AT: The MegaTraveller title screen
SETUP KEYS: normal launch with no snapshot: the title cannot be restored (the snapshot launch refuses title and character generation); acceptance/local/atoms/045.json is the headless frame proof only. Machine-tested through the play API: the probe drives character generation decision by decision to the name prompt; every decision on the way is a menu, yes / no, press-a-key or list, each of which the product answers by click.
ACTIONS:
  1. Click Start game
  2. Click Start New Game
  3. Click the choices one after another
EVALUATE:
  - Every choice is clickable
  - Continue buttons show on every press-a-key screen
