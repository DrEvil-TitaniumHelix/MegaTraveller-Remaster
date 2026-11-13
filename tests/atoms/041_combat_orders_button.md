# T041 Combat: Orders with nothing standing

OPEN AT: A fight on Efate, Magellan selected, no standing target
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/041.json
ACTIONS:
  1. Click Orders
EVALUATE:
  - No shot is fired
  - Magellan is still to act
  - The result is the same as pressing O
