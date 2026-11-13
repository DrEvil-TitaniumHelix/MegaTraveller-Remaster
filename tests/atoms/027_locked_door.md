# T027 A locked door tries once

OPEN AT: A lit room inside a building on Efate (the party carries a torch turned on), the party split in five figures, a closed door straight north of the purple figure at top left
SCENARIO: efate_door
SETUP KEYS: snapshot acceptance/local/atoms/027.json
ACTIONS:
  1. Click the closed door straight north of the purple figure at top left
EVALUATE:
  - One try
  - The message reads This door is locked. (in the strip and the log); the Up arrow says the same
  - Nobody keeps bumping the door
