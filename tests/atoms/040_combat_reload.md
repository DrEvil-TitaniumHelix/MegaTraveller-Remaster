# T040 Combat: Reload an empty gun

OPEN AT: A fight on Efate, Magellan's gun empty
SCENARIO: efate_dry
SETUP KEYS: snapshot acceptance/local/atoms/040.json
ACTIONS:
  1. Click Reload
  2. Click the reload row in the side panel (the Attack is already spent: the log answers no Attack left)
EVALUATE:
  - The gun is full again
  - A clip is gone
  - The result is the same as pressing R
  - The second reload is refused: one Attack a turn
