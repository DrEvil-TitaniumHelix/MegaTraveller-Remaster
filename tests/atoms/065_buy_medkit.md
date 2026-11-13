# T065 Buy a medkit

OPEN AT: The Efate general store, its menu Welcome to the general store. What is your business? showing
SCENARIO: h9e_general_store_efate
SETUP KEYS: snapshot acceptance/local/atoms/065.json
ACTIONS:
  1. Click buy items (or press Enter on Buy)
  2. Click Party Account (or press Down five times, then Enter)
  3. Click Magellan (or press Enter)
  4. Click the tl9 med kit row, then Yes in the pop-up Buy this item? (or press + once, then Enter)
EVALUATE:
  - Who will buy these supplies? lists the five Travellers and the Party Account
  - Who will recieve the supplies? lists the five Travellers
  - Here are supplies you can buy: lists the tl9 med kit first with its price
  - After Enter the med kit is bought: the money leaves the Party Account and the store menu shows again
