# T060 Refuel the ship

OPEN AT: The Efate starport screen, the Interloper with 60 of 125 tons of fuel
SCENARIO: atom_low_fuel
SETUP KEYS: snapshot acceptance/local/atoms/060.json
ACTIONS:
  1. Click refuel in the Shipyard row (or press Down nine times to Refuel, then Enter)
  2. Click Refined (or press Enter)
  3. Click Party Account (or press Down five times, then Enter)
  4. Click All (or press A): the tank fills and Price shows the cost
  5. Click Pay (or press P) on the line Esc or Pay
EVALUATE:
  - Which type of fuel? offers Refined at 80 and Unrefined at 48 a ton
  - Who will pay for refueling? lists the Travellers and the Party Account
  - The refuel bill offers Buy one unit (B or F), All (A), Pay (P) and Cancel (Esc)
  - After Pay the starport list shows again
