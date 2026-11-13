# T059 Sell one lot of cargo

OPEN AT: The Efate starport screen, its list of services showing, four lots of water in the hold
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/059.json
ACTIONS:
  1. Click sell in the Cargo row (or press Down eight times to Sell cargo, then Enter)
  2. Click Party Account (or press Down five times, then Enter)
  3. Click [+] on water, then Done (or press + once on water, then Enter)
EVALUATE:
  - Who will recieve the money? lists the five Travellers and the Party Account
  - Cargo Available lists water, the only cargo aboard, with its price 50
  - After water is chosen the starport list shows again
