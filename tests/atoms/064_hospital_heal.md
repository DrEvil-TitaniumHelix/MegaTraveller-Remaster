# T064 Heal a wounded Traveller at the hospital

OPEN AT: The Efate hospital, Magellan wounded (5 of his hits), the list Who will pay the doctor's bills? showing
SCENARIO: h9c_hospital_efate
SETUP KEYS: snapshot acceptance/local/atoms/064.json
ACTIONS:
  1. Click Party Account (or press Down five times, then Enter)
  2. Click Magellan, then Yes in the pop-up Check in for this fee? (or press Enter on Magellan)
EVALUATE:
  - Who will pay the doctor's bills? lists the five Travellers and the Party Account with their money
  - Who would like to check in? lists Magellan with his fee
  - Magellan is healed: his hits go up; the fee leaves the Party Account
