# T046 Character name box

OPEN AT: The MegaTraveller title screen
SETUP KEYS: normal launch with no snapshot: the title cannot be restored (the snapshot launch refuses title and character generation); acceptance/local/atoms/046.json is the headless frame proof only. Machine-tested through the play API: the probe drives character generation to the name prompt (asked at the end) and types a name into it. A window start point at the name prompt (a --snapshot for character generation or a --play route) is not built; the window steps below remain manual.
ACTIONS:
  1. Click Start game
  2. Click Start New Game
  3. Click through to the name box
  4. Click it and type a name
  5. Click OK
EVALUATE:
  - The name box takes the typed name
  - OK accepts it
