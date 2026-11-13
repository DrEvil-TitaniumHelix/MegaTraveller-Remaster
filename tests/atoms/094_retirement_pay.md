# T094 Collect retirement pay at the bank

OPEN AT: The Efate starport, Bank > Retmt. pay open on Magellan's payment message: two months of retirement pay, 2,640 credits. His payment has just been credited.
SETUP KEYS: snapshot acceptance/local/atoms/094.json
ACTIONS:
  1. Read Magellan's payment, then click to continue through the remaining Travellers' payment messages [Space five times]
  2. Click Retmt. pay again [Enter]
EVALUATE:
  - Each Traveller's payment message names the recipient and the amount collected
  - After the last message, the starport services return and the Party Account is unchanged
  - A second collection says No one recieved pension payment; nobody is paid twice
