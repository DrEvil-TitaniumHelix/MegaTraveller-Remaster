# T079 A ship without scoops at a gas giant

OPEN AT: The Interloper (no fuel scoops fitted) drifting onto the Boughene gas giant with 2 tons of fuel. The flight opens paused: PAUSED - press P to fly; click Pause (or press P) to fly.
SCENARIO: m6_gasgiant_lowfuel
SETUP KEYS: snapshot acceptance/local/atoms/079.json
ACTIONS:
  1. Click Pause to fly (or press P)
  2. Do nothing for six seconds, then six seconds more
EVALUATE:
  - The ship comes to rest at the gas giant
  - The title strip reads unable to process atmosphere (the 1990 line for a ship without scoops)
  - The fuel gauge does not rise
