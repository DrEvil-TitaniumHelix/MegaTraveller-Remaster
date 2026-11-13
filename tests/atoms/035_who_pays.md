# T035 Who pays

OPEN AT: The who-pays list for refuelling at the Efate starport
SCENARIO: atom_low_fuel
SETUP KEYS: snapshot acceptance/local/atoms/035.json
ACTIONS:
  1. Click Party Account on the list
  2. Click the words Buy one unit: the bar grows a notch, Fuel: reads one more of 124, Price shows the cost
  3. Click the word All: the bar fills, Fuel: reads 124 of 124, Price shows the whole cost, the account line shows what is left
  4. Click the word Pay
EVALUATE:
  - The list shows each Traveller and the Party Account with their money
  - Each word (Buy one unit, All, Pay) answers a click and shows its key letter underlined; the bar, the Fuel: number and the Price update after Buy one unit and All
  - The Party Account pays, the Travellers keep their money; the fuel goes up
