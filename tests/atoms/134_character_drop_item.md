# T134 Drop an item from the pack

OPEN AT: Magellan's character screen on quiet Efate ground, outdoors. The white med kit in his pack is highlighted and Move object is showing.
SETUP KEYS: snapshot acceptance/local/atoms/134.json
ACTIONS:
  1. Click Close while the med kit is still highlighted [Esc]
  2. Click Magellan's portrait [F1] to check his pack, then Close [Esc]
  3. Click Items [I], then Take [Enter] without walking away; choose the first Traveller, Magellan [Enter]
  4. Click Magellan's portrait [F1] to check his pack again
EVALUATE:
  - Closing the character screen drops the held med kit at the party's feet and returns to the ground view.
  - Magellan's pack no longer contains the med kit; his pistol and jacket remain.
  - Items, Take picks the med kit back up, and it is in Magellan's pack again.
