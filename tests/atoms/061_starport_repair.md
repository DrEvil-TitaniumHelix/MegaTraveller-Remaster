# T061 Repair the ship at the starport

OPEN AT: The Efate starport screen, its list of services showing, the Interloper damaged while docked
SCENARIO: h9e_repair_efate
SETUP KEYS: snapshot acceptance/local/atoms/061.json
ACTIONS:
  1. Click repair in the Shipyard row (or press Down ten times to Repairs, then Enter)
  2. Click Computer, the first damaged part (or press Enter)
  3. Click Party Account (or press Down five times, then Enter)
  4. Click All to repair it all (or press A), then click Pay (or press P)
EVALUATE:
  - Repairs lists the three damaged parts of the Interloper: Computer, Gun One, Structure
  - Who will pay for repairs? lists the five Travellers and the Party Account
  - The repair page shows the part, Repairs: cost per point, the Party Account and a total; A fixes the part
  - The part is completely fixed at a cost; the money leaves the Party Account; the starport list shows again
