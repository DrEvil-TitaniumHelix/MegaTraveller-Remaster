# T006 One attack per Traveller per round

OPEN AT: A fight on Efate, Magellan selected, three thugs close by, nobody has acted yet
SCENARIO: efate_broken
SETUP KEYS: snapshot acceptance/local/atoms/006.json
ACTIONS:
  1. Click the nearest thug: one click targets and fires (or press Tab until a thug is named, then F)
  2. Right-click another thug: its menu opens (or press Tab, then M); Esc closes it
EVALUATE:
  - Magellan fires once
  - The second menu shows Fire greyed with no Attack left
  - Magellan cannot fire a second time this round
