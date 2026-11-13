# T015 O fires and keeps the turn

OPEN AT: The ambush on Efate, Magellan to act with a standing target on a thug, the status line offering to fire
SETUP KEYS: snapshot acceptance/local/atoms/015.json
ACTIONS:
  1. Click Orders (or press O), without End turn
EVALUATE:
  - Magellan fires at the standing target
  - The status line still reads Your move: Magellan
  - After one South step, Magellan moves one square and MP reads 9
