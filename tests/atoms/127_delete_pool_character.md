# T127 Delete a character from the pool

OPEN AT: The Delete Character From Pool name list, headed Choose Character To Delete: Jurgen is first and no Travellers have been added to the party.
SETUP KEYS: From the title: Start game > Start New Game > Use Default Characters > Delete Character From Pool. This is the unsaved default pool. Headless proof: acceptance/local/atoms/127.json. The window cannot load character-creation snapshots.
ACTIONS:
  1. Click: Jurgen > No   [Enter, Enter]
  2. Click: Delete Character From Pool > Jurgen > Yes   [Enter, Enter, Y]
  3. Click: View Character   [Down, Enter]
EVALUATE:
  - Choosing Jurgen deletes nobody: the top line asks "Delete Jurgen for good?" with Yes and No, No highlighted
  - No returns to the pool menu with Jurgen still in the pool
  - Yes deletes Jurgen and returns to the pool menu
  - View Character no longer lists Jurgen; Verti and the other original characters remain
