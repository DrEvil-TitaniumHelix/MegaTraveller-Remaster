# T056 A hunting thug off the screen starts no fight until he walks onto it

OPEN AT: The party split on Efate beside a lake, no fight yet, the status line reading Alerted: 1, no thug showing on the map
SCENARIO: atom_screen_edge
SETUP KEYS: snapshot acceptance/local/atoms/056.json
ACTIONS:
  1. Click the square south of the party (or tap the South arrow once)
  2. Click the square north (or tap the North arrow once)
  3. Keep clicking south and north (or tapping) until a fight starts
EVALUATE:
  - No fight starts while the thug is off the screen
  - The thug steps onto the screen from the east
  - The fight starts as he comes on, not before
