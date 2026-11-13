# T053 A missing file

OPEN AT: The first-run screen with the drop box (one file, A0.PIC, is missing)
SETUP KEYS: folder acceptance/local/atoms/setup52, start the exe inside it
ACTIONS:
  1. Read the screen
  2. Click the button Choose folder...
  3. In the Windows folder picker choose the folder MegaTra1 and press OK
  4. Press Enter
EVALUATE:
  - The same start screen as with no files at all: title, drop box, "Choose folder...", nothing red, no file names
  - The button opens the standard Windows folder picker
  - After OK: "Found the game in the folder "MegaTra1"." and the box "Press Enter or click here to copy its files"
  - After Enter: "Press any key or click here to play"; a key starts the game
