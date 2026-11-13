# T083 Losing the party returns to the title

OPEN AT: A fight on Efate, every Traveller one hit from falling, three thugs close by, Round 1, the party to act
SCENARIO: atom_last_stand
SETUP KEYS: snapshot acceptance/local/atoms/083.json
ACTIONS:
  1. Click End turn in the combat controls (or press Space, then E) until the fight ends
  2. Press a key or click
EVALUATE:
  - The fight waits for you at Round 1
  - The thugs shoot the Travellers down one by one
  - When the last Traveller falls the game says The party is lost and waits
  - After the key or click the MegaTraveller title shows
