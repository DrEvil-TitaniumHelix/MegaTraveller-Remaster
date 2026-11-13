# T028 Hover text

OPEN AT: A fight on Efate, three thugs close by
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/028.json
ACTIONS:
  1. Hover over a Traveller
  2. Hover over a thug
  3. Hover over an empty square
EVALUATE:
  - One line shows beside the pointer
  - It is the game's own hover text for each
  - The line goes when the pointer moves off
