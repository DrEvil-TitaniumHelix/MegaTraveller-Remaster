# T033 Right-click an item on the ground

OPEN AT: The party split on Efate, a small pile of items on the grass two squares east of Magellan
SCENARIO: atom_pile
SETUP KEYS: snapshot acceptance/local/atoms/033.json
ACTIONS:
  1. Right-click the pile
  2. Click Take
EVALUATE:
  - The menu offers Take
  - Picking it takes the item
