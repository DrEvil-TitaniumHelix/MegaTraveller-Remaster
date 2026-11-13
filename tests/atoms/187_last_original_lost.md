# T187 Lose the last original Traveller

OPEN AT: The game-over message on Pixie after Magellan, the last original Traveller in this prepared party, has died outside the protected starport area without breathing equipment.
SETUP KEYS: Open snapshot acceptance/local/atoms/187.json. Scenario: tests/atoms/scenarios/atom_187_last_original.toml.
ACTIONS:
  1. Read the reason the game has ended
  2. Click to continue [Space]
EVALUATE:
  - The message says no original characters are left in the party and that you have failed
  - The message waits for a click or key
  - Continuing leaves the lost party and returns the played game to the title
