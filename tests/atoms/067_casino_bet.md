# T067 One bet at the casino

OPEN AT: The Efate casino, the words 'Spin ($100)' and 'Leave' on the screen, Magellan holding 100,000 Cr
SCENARIO: m5_casino_rich
SETUP KEYS: snapshot acceptance/local/atoms/067.json
ACTIONS:
  1. Click 'Spin ($100)' (or press S), then wait for the reels to stop (about three seconds)
  2. Click 'Leave' (or press L or Esc)
EVALUATE:
  - The reels spin and stop; the result is written in words: 'No win this time / You lose $100' or 'We have a winner!! / You win $N'
  - After the spin 'Spin ($100)' and 'Leave' are on the screen again
  - Leave returns to the map outside the casino
