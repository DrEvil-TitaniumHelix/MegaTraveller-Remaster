# T114b Take cargo before the defeated ship explodes

OPEN AT: The defeated ship's cargo screen, both holds visible and the self-destruct countdown running.
SETUP KEYS: snapshot acceptance/local/atoms/114b.json
ACTIONS:
  1. Click Take [T]
  2. Click the first lot in the enemy hold [Enter]
  3. Click an empty slot in your hold [Enter]
  4. Click Exit ship [Esc]
EVALUATE:
  - Both holds and the self-destruct countdown are visible
  - Taking the first lot moves cargo into the empty hold slot
  - Exiting promptly returns to flight with the cargo and no hull damage
