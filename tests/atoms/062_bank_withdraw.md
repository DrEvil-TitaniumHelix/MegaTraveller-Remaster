# T062 Take money out of the bank

OPEN AT: The Efate starport screen, its list of services showing
SCENARIO: m5_starport_desk_efate
SETUP KEYS: snapshot acceptance/local/atoms/062.json
ACTIONS:
  1. In the grid at the lower left, row Bank, click the cell account: the blue box then asks Select transaction type
  2. In the blue box click Withdrawal (it is the second line, under Deposit)
  3. Click Magellan (or press Enter)
  4. Type 1000 and press Enter (the amount box takes the keyboard); click the message to go on (or press Space)
EVALUATE:
  - Select transaction type: offers Deposit and Withdrawal
  - Who will recieve money? lists the five Travellers
  - An amount box opens
  - Magellan has 1000 more after Enter
