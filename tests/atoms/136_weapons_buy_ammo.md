# T136 Buy ammunition for a pistol

OPEN AT: The Efate weapons store's What ammo would you like? list, showing ammunition for Magellan's 9mm pistol. Magellan is the receiver and the Party Account is paying.
SETUP KEYS: snapshot acceptance/local/atoms/136.json
ACTIONS:
  1. Click the 9mm ammo row, then Yes to buy one reload [from the list: + once, Enter]
  2. Click Back to leave the buying choices [Esc], then Back to leave the store [Esc]
  3. Click Magellan's portrait [F1], select INSPECT OBJECT [8], then click View [Enter]
EVALUATE:
  - The purchase returns to Anything else? and deducts the displayed price from the Party Account.
  - The pistol's detail panel shows two spare reloads instead of one.
  - The pistol remains in Magellan's pack.
