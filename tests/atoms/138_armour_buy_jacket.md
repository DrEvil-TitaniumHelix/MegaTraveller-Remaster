# T138 Buy a flak jacket at the armour shop

OPEN AT: The Efate armour shop, showing This is the armor I have. and its list of armour and prices. Magellan has an empty pack and the Party Account is paying.
SETUP KEYS: snapshot acceptance/local/atoms/138.json
ACTIONS:
  1. Click flak jacket, then Yes to buy it [from the list: Down twice, Enter]
  2. Click Back to leave the armour list [Esc], then Back to leave the shop [Esc]
  3. Click Magellan's portrait [F1] and check his pack
EVALUATE:
  - The shop says Done. Anything else? after the purchase.
  - The Party Account pays the displayed flak jacket price.
  - Magellan's pack now contains a flak jacket, ready to put on.
