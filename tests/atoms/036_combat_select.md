# T036 Combat: click a portrait selects

OPEN AT: A fight on Efate, Magellan selected
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/036.json
ACTIONS:
  1. Click the portrait of Jurgen
EVALUATE:
  - Jurgen becomes the Traveller to act
  - The status line reads Your move: Jurgen
