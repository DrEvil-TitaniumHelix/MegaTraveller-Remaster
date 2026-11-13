# T092 Transfer money between Travellers

OPEN AT: The Efate bank, Select transaction type showing Deposit and Withdrawal. Magellan has 5,000 Cr, Jurgen 2,000 Cr and the Party Account 30,000 Cr.
SETUP KEYS: snapshot acceptance/local/atoms/092.json
ACTIONS:
  1. Click Deposit [Enter], then Magellan [Enter]
  2. Click the amount box and enter 1000 [1000, Enter]
  3. Click Withdrawal [Down, Enter], then Jurgen [Down, Enter]
  4. Click the amount box and enter 1000 [1000, Enter]
  5. Click Deposit to read both Traveller balances [Enter], then click Back to close both lists [Esc twice]
EVALUATE:
  - Depositing moves 1,000 Cr from Magellan into the Party Account; withdrawing moves it to Jurgen
  - The final depositor list shows Magellan with 4,000 Cr and Jurgen with 3,000 Cr
  - The Party Account ends at 30,000 Cr and the other Travellers keep their money
