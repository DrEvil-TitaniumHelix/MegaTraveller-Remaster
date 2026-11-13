# T184 Lose the Interloper in a space fight

OPEN AT: The ship-loss screen after the Interloper has suffered fatal hull damage in an Efate encounter. The game is waiting for a key.
SETUP KEYS: Open snapshot acceptance/local/atoms/184.json. Scenario: tests/atoms/scenarios/atom_184_ship_destroyed.toml.
ACTIONS:
  1. Read both lines on the ship-loss screen
  2. Click to continue [Space]
EVALUATE:
  - The screen says the entire party has died and failed to complete the mission
  - The message waits until you continue
  - Continuing ends this game and returns the played game to its title
