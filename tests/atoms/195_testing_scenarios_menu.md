# T195 Start somewhere else with Testing scenarios

OPEN AT: The quiet Efate street outside the starport, with the five Travellers grouped between the blue-and-white pennants.
SETUP KEYS: Open snapshot acceptance/local/atoms/195.json. The menu destination is the shipped scenario m5_starport_desk_efate, also used by existing starport checks.
ACTIONS:
  1. Click Testing scenarios on the menu bar [Ctrl+Shift+L]
  2. Open Efate starport and click Starport desk [arrows, Enter]
  3. Close the Plus Test page that opens [F12] and look at the new starting square
  4. Click the doorway one square to the left [Left]
EVALUATE:
  - The menu says Testing scenarios (not part of the game) and groups its destinations by place
  - Choosing Starport desk places the five Travellers just east of the Efate starport door and opens the Plus Test page
  - Walking left enters the starport services desk, offering Information, Save, Bank, Pensions, Recruit, Training, cargo, fuel, repairs, weapons and Ship supplies
  - Leave starport is available on the services screen
