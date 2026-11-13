# T182 Try to jump without a working jump drive

OPEN AT: The flight screen in the Efate system, the Interloper standing still in open space with thrust 0 and no drift. The title reads Flying in the Efate system; PAUSED - press P to fly shows. The ship has 124 tons of fuel and running Navigation and Jump software, but no jump drive.
SETUP KEYS: Open snapshot acceptance/local/atoms/182.json. Scenario: tests/atoms/scenarios/atom_182_no_drive.toml.
ACTIONS:
  1. Click Jump [J]. If the click only clears PAUSED, click Jump once more; pressing J opens it directly
  2. Read the missing-drive refusal over the jump map
  3. Click to return to flight [Space]
  4. Click Jump [J] to try again
EVALUATE:
  - Entering Jump shows a message saying the ship has no jump drive and a shipyard can fit one
  - Acknowledging returns to flight at Efate with fuel and date unchanged; no jump takes place
  - Opening Jump again shows the same refusal
