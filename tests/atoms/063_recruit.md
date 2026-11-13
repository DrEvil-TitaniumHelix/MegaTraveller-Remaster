# T063 Recruit a Traveller

OPEN AT: The Efate starport screen, the party four strong with one berth free
SCENARIO: atom_recruit
SETUP KEYS: snapshot acceptance/local/atoms/063.json
ACTIONS:
  1. Click recruit in the Personnel row (or press Down twice to Recruit, then Enter)
EVALUATE:
  - Nothing asks for a drive or a disk
  - Select character to try to recruit: lists the characters at once
