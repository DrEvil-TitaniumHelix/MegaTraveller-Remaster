# T091 Deposit a Traveller's money

OPEN AT: The Efate bank, the Deposit amount box open for Magellan, showing his 5,000 Cr. The Party Account has 30,000 Cr.
SETUP KEYS: snapshot acceptance/local/atoms/091.json
ACTIONS:
  1. Click the amount box, type 1000 and confirm [1000, Enter]
  2. Click Deposit again [Enter]
  3. Read Magellan's balance, then click Back to close the depositor and transaction lists [Esc twice]
EVALUATE:
  - The bank returns to Select transaction type with 31,000 Cr in the Party Account
  - The depositor list now shows Magellan with 4,000 Cr
  - Closing the lists returns to the starport services
