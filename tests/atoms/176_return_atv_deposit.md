# T176 Return an undamaged ATV and recover the deposit

OPEN AT: Zenaca Vehicles just after renting an ATV and acknowledging the parking policy. The policy remains visible; Rent, Return and Back are offered. The Party Account has 26000 credits.
SETUP KEYS: snapshot acceptance/local/atoms/176.json
ACTIONS:
  1. Click Return [Down, Enter]
  2. Read the refund message, then acknowledge it [Space]. The office menu returns and the message remains visible.
  3. Click Return again [Down, Enter]
EVALUATE:
  - The office thanks you and says the deposit has been credited to the account.
  - The Party Account gains 2000 credits for the undamaged vehicle parked nearby, reaching 28000. Preparation verifies these balances; the office does not display them.
  - A second return says no vehicle is rented and pays no second deposit.
