# T054 A different data file warns

OPEN AT: The screen that says one file differs
SETUP KEYS: folder acceptance/local/atoms/setup53, start the exe inside it
ACTIONS:
  1. Read the screen
  2. Press any key
EVALUATE:
  - The screen reads "The files of the game are in place.", then "1 of its files differ from the usual 1990 ones (MAP.PAL). The game may still work.", and the box "Press any key or click here to play"
  - The key shows the original title
