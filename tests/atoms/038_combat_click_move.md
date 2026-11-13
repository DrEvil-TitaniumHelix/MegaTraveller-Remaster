# T038 Combat: click a square to move

OPEN AT: A fight on Efate, Magellan selected
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/038.json
ACTIONS:
  1. Click an empty square two squares away
EVALUATE:
  - Magellan walks to it
  - The others stay where they are
