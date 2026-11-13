# T186 Watch the San station self-destruct countdown

OPEN AT: The central corridor of the Delgado research station on San in the Efate system. All station NPCs are peaceful. The yellow countdown reads 01; Self destruct: 1 is underneath. Exactly four walking squares remain before the loss message.
SETUP KEYS: Open snapshot acceptance/local/atoms/186.json. Scenario: tests/atoms/scenarios/atom_186_san_countdown.toml.
ACTIONS:
  1. Read: yellow countdown; Self destruct: 1
  2. Walk: left, right, left, right
  3. Read: Boom. You are all dead. You have failed.
  4. Press: Space
EVALUATE:
  - The yellow countdown is fully visible in the side panel, with Self destruct: N underneath
  - The countdown falls as the party walks the four squares; no fight starts
  - At zero the screen shakes and the loss message says Boom. You are all dead. You have failed.
  - The loss message waits for a key; Space returns to the title
