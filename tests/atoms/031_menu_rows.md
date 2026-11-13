# T031 Clickable menu rows

OPEN AT: The Game menu open on the Efate starport yard
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/031.json
ACTIONS:
  1. Look at the rows
  2. Click the row Sound off
EVALUATE:
  - The four rows answer to F1-F4, so no letter is underlined here; a letter-keyed menu (a shop's buy items / sell possessions) shows its letter underlined
  - The row acts as if its key were pressed
  - The menu closes
