# T010 A menu costs no time in a fight

OPEN AT: A fight on Efate, Effane not selected, nobody has acted yet
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/010.json
ACTIONS:
  1. Right-click the selected Traveller: the menu opens (or press M)
  2. Press Esc to close it
  3. Right-click Effane, then click Character (or press F3)
  4. Click Close on the record (or press Esc)
EVALUATE:
  - Nothing moves while the menu or the character screen is open
  - No thug fires
  - The initiative bar shows the same round, the status line reads Your move: Magellan
