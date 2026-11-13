# T113 Repair a damaged ship component

OPEN AT: The Engineering station aboard the Interloper at Efate, Verti at the controls, Computer at 75 percent and Structure at 95 percent.
SETUP KEYS: snapshot acceptance/local/atoms/113.json
ACTIONS:
  1. Click: Repair > Computer   [R, Enter]
  2. Wait 40 seconds (the header counts the seconds down)
  3. Watch: the header, the Computer number
EVALUATE:
  - The header reads 'comp +1 in 40s, all 17m': the seconds to the next point count down, the minutes are the time until the Computer is at 100
  - When the seconds reach 0 the Computer number goes 75 -> 76 and the countdown starts again
  - Structure stays at 95; repairing the Computer does not repair the hull
