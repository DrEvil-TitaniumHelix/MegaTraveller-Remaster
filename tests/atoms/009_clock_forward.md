# T009 Time in a fight only goes forward

OPEN AT: A fight on Efate, the status line reads Round 1, party to act, nobody has acted yet
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/009.json
ACTIONS:
  1. Click the left half of the strip above the map, we go first (or press Space), then End turn (or E) until the initiative bar shows Round 2
EVALUATE:
  - The round number goes up to Round 2 and never back
  - The log shows each shot in order, nothing repeated
  - Magellan's portrait keeps its DOWN label when the initiative bar changes to R2
