# T008 A pick stands

OPEN AT: A fight on Efate, Magellan selected, nobody has acted yet
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/008.json
ACTIONS:
  1. Click the portrait of Dennar (or press 4)
  2. Right-click the nearest thug: its menu opens (or press Tab, then M)
  3. Press Esc to close the menu (a click elsewhere leaves it open; the menu draws no Back)
EVALUATE:
  - Dennar becomes the Traveller to act
  - The status line reads Your move: Dennar
  - Dennar stays selected after the menu closes
