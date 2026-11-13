# T043 Click a name in the initiative bar

OPEN AT: A fight on Efate, the initiative bar showing the order
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/043.json
ACTIONS:
  1. Click the name of Jurgen in the initiative bar
EVALUATE:
  - Jurgen is selected
  - The status line reads Your move: Jurgen
