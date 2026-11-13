# T109 Use a spare oxygen tank on the map

OPEN AT: The Efate starport yard with Items open at the lower right: Take, Use and Back. Magellan is carrying a spare oxygen tank in his primary hand and his oxygen reserve is low.
SETUP KEYS: snapshot acceptance/local/atoms/109.json
ACTIONS:
  1. Click Use [U]
  2. Choose the first Traveller, Magellan, as the one carrying the tank [Enter]
  3. Choose Magellan again as the person receiving the oxygen [Enter]
  4. Read the replenishment message, then open Magellan's record [F1]
EVALUATE:
  - Use asks who carries the tank and who receives the oxygen
  - The message reports that the oxygen supply has been replenished
  - Magellan's oxygen reserve is full and the used tank has left his hand
