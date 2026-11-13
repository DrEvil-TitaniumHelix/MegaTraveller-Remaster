# T039 Combat: End turn moves the fight on

OPEN AT: A fight on Efate, Magellan selected, no standing target
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/039.json
ACTIONS:
  1. Click End turn
EVALUATE:
  - Magellan's turn ends with no shot
  - The fight moves on to the next side or Traveller
