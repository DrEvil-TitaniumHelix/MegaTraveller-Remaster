# T004 The starport door

OPEN AT: The Efate starport yard, the party standing one square east of the starport door
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/004.json
ACTIONS:
  1. Click the starport door, one square west (or tap the West arrow once)
EVALUATE:
  - The party goes in through the door
  - The starport screen shows with its list of services
  - The screen offers Bank, Refuel and Save
