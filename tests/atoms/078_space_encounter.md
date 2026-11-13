# T078 Fight a ship encounter to its end

OPEN AT: The Interloper in the Efate system with a hostile ship alongside, the pilot station open
SCENARIO: atom_encounter
SETUP KEYS: snapshot acceptance/local/atoms/078.json
ACTIONS:
  1. Click View, then the GUN1 station (or press V, Right, Right, Enter)
  2. Click the turn-gun-right arrow four times (or press Right four times) onto the enemy's bearing
  3. Click Target to aim (or press T)
  4. Click Fire (or press Enter); Target and Fire again until the enemy is destroyed. On the enemy's cargo screen click the In-sys travel box (or press Esc) to return to flight
EVALUATE:
  - The gunner station opens; after turning the gun the enemy ship is in view
  - T puts the aim on the enemy
  - Each Enter fires and the hits show
  - The encounter ends: the enemy is destroyed or breaks off, and the flight screen returns
