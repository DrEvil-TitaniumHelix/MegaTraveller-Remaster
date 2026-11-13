# T131 Move an item within the pack

OPEN AT: Magellan's character screen on quiet Efate ground. A pistol, green flak jacket and white med kit fill the pack's top row; the next row is empty.
SETUP KEYS: snapshot acceptance/local/atoms/131.json
ACTIONS:
  1. Click the white med kit (or Pick object [P], select the med kit with the arrow keys [Enter])
  2. Click the leftmost empty cell in the next row down (or Move item [M], select that cell with the arrow keys [Enter])
  3. Click the med kit in its new cell, then click its original empty cell to move it back
EVALUATE:
  - Picking the med kit highlights it and offers Move object.
  - Placing it fills the chosen cell and leaves its old cell empty.
  - Moving it back restores the original pack; the pistol and jacket stay put.
