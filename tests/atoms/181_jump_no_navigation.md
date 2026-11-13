# T181 Try to jump without Navigation running

OPEN AT: The flight screen in the Efate system, the Interloper standing still in open space with thrust 0 and no drift. The title reads Flying in the Efate system; PAUSED - press P to fly shows. Jump software is running and the ship has 124 tons of fuel, but Navigation is stopped.
SETUP KEYS: Open snapshot acceptance/local/atoms/181.json. Scenario: tests/atoms/scenarios/atom_181_no_navigation.toml.
ACTIONS:
  1. Click Jump [J]. If the click only clears PAUSED, click Jump once more; pressing J opens it directly
  2. Read the Navigation refusal over the jump map
  3. Click to return to flight [Space]
  4. Read the system name and fuel monitor, then click Jump [J] again
EVALUATE:
  - Entering Jump shows a message saying the Navigation program is not running and explaining how to load and run it
  - Acknowledging returns to flight at Efate with fuel and date unchanged; no jump takes place
  - Opening Jump again shows the same refusal
