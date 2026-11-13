# T080 Land on the new world

OPEN AT: The Interloper in orbit at Louzy, the flight screen showing. The flight does not open paused here: an orbit counts as docked (only free flight opens paused).
SCENARIO: m6_landing_louzy
SETUP KEYS: snapshot acceptance/local/atoms/080.json
ACTIONS:
  1. Click Land in the list at the right (or press L): in orbit the rows read Land / Take-off (the 1990 draws Jump / Quit there but its mouse lands on that row; product mode draws what the row does)
EVALUATE:
  - The ship lands
  - The Louzy ground map shows with the party standing on it
  - The side panel names Louzy
