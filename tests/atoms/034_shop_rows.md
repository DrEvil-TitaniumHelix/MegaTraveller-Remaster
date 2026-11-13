# T034 Shop: buy by clicking the item

OPEN AT: The weapons store on Efate, its list of weapons and prices showing
SCENARIO: m5_weapons_store_efate_l4
SETUP KEYS: snapshot acceptance/local/atoms/034.json
ACTIONS:
  1. Click a weapon row
  2. Click Yes in the pop-up
  3. Click another weapon row, then Cancel
  4. Click Back in the right-hand buttons (leaves the store)
EVALUATE:
  - Clicking a row opens a small pop-up: that weapon, its price, the credits available, Yes / Cancel
  - Yes buys one: the pop-up closes and the credits drop by the price
  - Cancel closes the pop-up and buys nothing
  - The right-hand + and - buttons do nothing in a shop; Back leaves the store
