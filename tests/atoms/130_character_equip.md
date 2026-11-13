# T130 Put a weapon in a hand and wear armour, then take them off

OPEN AT: Magellan's character screen on quiet Efate ground. His pack holds a pistol, a green flak jacket and a white med kit across its top row; his hands and armour slot are empty.
SETUP KEYS: snapshot acceptance/local/atoms/130.json
ACTIONS:
  1. Click the pistol in the pack (or click Pick object [P], then select the pistol [Enter])
  2. Click the empty PRIMARY hand box [F9], then click the pistol in that box again [F9]
  3. Click the green flak jacket in the pack (or Pick object [P], select the jacket with the arrow keys [Enter])
  4. Click the small armour box marked ARM, to the left of the SECONDARY hand [F8], then click the jacket there again [F8]
EVALUATE:
  - The pistol moves from the pack into the primary hand, then returns to the pack.
  - The flak jacket moves from the pack onto Magellan, then returns to the pack.
  - The med kit stays in place; no item disappears or is duplicated.
