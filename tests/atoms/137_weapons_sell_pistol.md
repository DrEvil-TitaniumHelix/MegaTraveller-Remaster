# T137 Sell a weapon at the weapons store

OPEN AT: The Efate weapons store's Which weapons are you selling list, with Magellan's 9mm auto pistol and its offer price. The Party Account will receive the money.
SETUP KEYS: snapshot acceptance/local/atoms/137.json
ACTIONS:
  1. Click the pistol row, then Yes to sell it [from the list: Enter]
  2. Click Back to leave the store [Esc]
  3. Click Magellan's portrait [F1] and check his pack
EVALUATE:
  - Selling the pistol returns to the weapons store's welcome menu.
  - The Party Account gains the displayed offer price.
  - The pistol is gone from Magellan's pack and his hands.
