# T007 Every Traveller gets one turn each round

OPEN AT: A fight on Efate, the status line reads Round 1, party to act, nobody has acted yet
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/007.json
ACTIONS:
  1. Click the left half of the strip above the map, we go first (or press Space)
  2. Click End turn (or press E)
  3. Click End turn (or press E)
  4. Click End turn (or press E)
  5. Click End turn twice (or press E twice)
EVALUATE:
  - The status line reads Your move with a different Traveller each time
  - No Traveller is asked twice before the round changes
  - Each thug acts once between the turns
  - The initiative bar then shows Round 2
