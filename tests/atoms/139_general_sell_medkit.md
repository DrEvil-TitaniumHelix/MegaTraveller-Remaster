# T139 Sell an item at the general store

OPEN AT: The Efate general store, showing Magellan has the following items to sell: and his tl9 med kit with its offer price. The Party Account will receive the money.
SETUP KEYS: snapshot acceptance/local/atoms/139.json
ACTIONS:
  1. Click the tl9 med kit row, then Yes to sell it [from the list: + once, Enter]
  2. Click Back to leave the store [Esc]
  3. Click Magellan's portrait [F1] and check his pack
EVALUATE:
  - The sale returns to the general store's welcome menu.
  - The Party Account gains the displayed offer price.
  - The med kit has gone from Magellan's pack.
