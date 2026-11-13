# T055 A different code file is refused

OPEN AT: The first-run screen with the drop box (T.DAT is not the 1990 file)
SETUP KEYS: folder acceptance/local/atoms/setup54, start the exe inside it
ACTIONS:
  1. Read the screen
  2. Drag the folder acceptance/local/atoms/setup54/gamefiles (the one this check was started in) onto the game window
  3. Press Esc
EVALUATE:
  - Before the drop: the start screen with the drop box and "Choose folder...", nothing red
  - After the drop, on a red band: "The game in the folder "gamefiles" is a different version: T.DAT is not the 1990 file. Try the original 1990 version."; the drop box and the button are still there
  - Esc closes the game
