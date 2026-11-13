# T119 Run out of fuel in flight

OPEN AT: The Interloper paused in free flight at Efate, away from any planet, with 12 tons of fuel: a clear blue section about one tenth of the fuel bar. Thrust reads 0; the ship coasts when resumed.
SETUP KEYS: snapshot acceptance/local/atoms/119.json
ACTIONS:
  1. Resume: flight   [P]
  2. Click: thrust up   [Up]
  3. Wait: empty fuel bar and thrust 0 (about one minute)
  4. Click: thrust up again   [Up]
  5. Watch: continued drift
EVALUATE:
  - The opening fuel bar has a clearly visible blue section
  - Thrust shows 0 before thrust up, then 1 while fuel burns
  - The blue fuel bar empties within about a minute and thrust returns to 0
  - The message reads "Out of fuel: the engines are off. The ship drifts."
  - Another thrust command leaves the number at 0 and the out-of-fuel words visible
  - The ship continues drifting with no fuel instead of refilling or landing itself
