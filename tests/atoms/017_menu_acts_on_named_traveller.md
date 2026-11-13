# T017 Reload acts on the Traveller selected

OPEN AT: A fight on Efate, Magellan selected, Effane not selected
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/017.json
ACTIONS:
  1. Click the portrait of Effane (or press 3)
  2. Click Reload (or press R)
EVALUATE:
  - Effane reloads, not Magellan
  - The status line moves to Your move: Effane
  - Magellan's ammunition is unchanged
