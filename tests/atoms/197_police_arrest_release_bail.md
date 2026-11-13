# T197 Leave police custody with a release or bail

OPEN AT: The police arrest screen on Alell: a guard portrait and the choice Release or Pay bail. Bail is $26000; the party has $30000 and Magellan carries one Imperial Release.
SETUP KEYS: Open snapshot acceptance/local/atoms/197.json for each choice. The prepared party is just entering Alell's starport after two offences.
ACTIONS:
  1. Read the arrest notice, then click Release [Up/Down to Release, Enter]
  2. Click Plus page [F12], then Characters and Magellan; scroll through Money and Carried [PgDn], then close Plus [F12]
  3. Reopen snapshot acceptance/local/atoms/197.json and click Pay bail [Up/Down to Pay bail, Enter]
  4. Open Plus page again [F12], then Characters and Magellan; compare Money and Carried, then close Plus [F12]
EVALUATE:
  - The arrest notice offers an Imperial Release or $26000 bail
  - Release returns to the Alell map, consumes Magellan's Imperial Release and leaves the Party fund at $30000
  - Pay bail from the original snapshot returns to the same Alell map with $4000 in the Party fund; Magellan still carries the Imperial Release
  - Both choices leave all five Travellers alive and able to continue
