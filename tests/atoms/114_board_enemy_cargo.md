# T114 Take cargo from a defeated ship

OPEN AT: The defeated ship's cargo screen, its goods on the right and empty spaces in the Interloper's hold on the left.
SETUP KEYS: snapshot acceptance/local/atoms/114.json
ACTIONS:
  1. Click Take [T]
  2. Click the first lot in the enemy hold [Enter]
  3. Click an empty slot in your hold [Enter]
  4. Click Exit ship [Esc]
EVALUATE:
  - Both holds are visible, with goods in the defeated ship
  - The chosen lot moves to your hold and its old slot becomes empty
  - Exit closes the cargo screen and returns to flight
