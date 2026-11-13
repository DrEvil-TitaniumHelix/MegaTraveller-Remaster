# T032 Right-click menu in a fight

OPEN AT: A fight on Efate, Magellan selected
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/032.json
ACTIONS:
  1. Right-click a Traveller, then Esc
  2. Right-click a thug, then Esc
  3. Right-click an empty square: no menu opens there (the square only shows its walk label); do not press Esc
EVALUATE:
  - Each menu shows only the actions the game offers there
  - Greyed rows give their reason
  - Empty ground has no menu (right-click = information only)
