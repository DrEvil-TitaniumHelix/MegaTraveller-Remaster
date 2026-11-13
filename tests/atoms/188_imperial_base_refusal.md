# T188 Be refused entry to the Imperial base

OPEN AT: The Imperial-base guard on Efate, with the message that you are not permitted to enter and must leave. The party remains outside the entrance.
SETUP KEYS: Open snapshot acceptance/local/atoms/188.json. Scenario: tests/atoms/scenarios/atom_188_imperial_refusal.toml.
ACTIONS:
  1. Read the guard's refusal
  2. Click to dismiss it [Space]
  3. Click the entrance one square south [Down] to try again
  4. Click to dismiss the repeated refusal [Space]
EVALUATE:
  - The guard says you are not permitted to enter the official Imperial base
  - Dismissing the message returns to the street outside the same entrance
  - Trying again repeats the refusal without admitting the party
