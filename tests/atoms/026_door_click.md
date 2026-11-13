# T026 Click a door to go in

OPEN AT: The Efate starport yard, the party standing one square east of the starport door
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/026.json
ACTIONS:
  1. Click the starport door
EVALUATE:
  - The party walks up and goes in, turning first if it needs to
  - The starport screen shows
