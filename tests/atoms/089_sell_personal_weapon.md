# T089 Sell a Traveller weapon

OPEN AT: The Efate weapons store, Which weapons are you selling showing Magellan's dagger and its price. The Party Account is chosen to receive the money.
SETUP KEYS: snapshot acceptance/local/atoms/089.json
ACTIONS:
  1. Click dagger to sell it [Enter]
  2. Click Back to close the store [Esc]
  3. Click Traveller 1 in the Commands bar to inspect Magellan's pack [F1]
EVALUATE:
  - The dagger disappears from the sale list and the store returns to Buy Weapons / Sell Weapons
  - The Party Account gains the displayed sale price; Magellan's own money is unchanged
  - The dagger is gone from Magellan's pack
