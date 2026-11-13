# T183 Misjump to the wrong system

OPEN AT: The flight screen in the Efate system, the Interloper standing still in open space with thrust 0 and no drift. The title reads Flying in the Efate system; PAUSED - press P to fly shows. Navigation and Jump are running, with 100 tons of fuel; the prepared random seed makes a jump toward Louzy arrive at Alell.
SETUP KEYS: Open snapshot acceptance/local/atoms/183.json. Scenario: tests/atoms/scenarios/atom_183_misjump.toml.
ACTIONS:
  1. Click Jump [J]. If the click only clears PAUSED, click Jump once more; pressing J opens it directly
  2. Read the low-fuel warning on the jump map
  3. Click Louzy [Enter] and read the confirmation and misjump warning
  4. Click Yes [Y]
  5. Read the arrival message; click to return to flight [Space] and read the system name
EVALUATE:
  - Entering Jump opens the map with a warning that fuel is low and a misjump is possible
  - The Louzy confirmation warns of a possible misjump and states the cost: 50 tons of fuel and seven days
  - Confirming produces a MISJUMP message and arrives at Alell instead of Louzy
  - The fuel falls from 100 to 50 and seven days pass
