# T058 Buy one lot of cargo

OPEN AT: The Efate starport screen, its list of services showing
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/058.json
ACTIONS:
  1. Click buy in the Cargo row (or press Down seven times to Buy cargo, then Enter)
  2. Click Party Account (or press Down five times, then Enter)
  3. Click [+] on water, then Done (or press Down three times to water, + once, then Enter)
EVALUATE:
  - Who will pay for the cargo? lists the five Travellers and the Party Account with their money
  - Cargo Available lists the goods with their prices, water at 100
  - After water is chosen the starport list shows again
