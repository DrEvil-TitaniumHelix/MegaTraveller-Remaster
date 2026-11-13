# T199 Save a changed character pool and load it again

OPEN AT: The character-pool menu after creating a sample party disk from the nine supplied characters. Save Character Pool is available; the Party list on the right is empty.
SETUP KEYS: Open snapshot acceptance/local/atoms/199.json. Delete only within this prepared sample pool; its character disk belongs to this snapshot session.
ACTIONS:
  1. Click View Character [Up/Down, Enter], check that Jurgen heads the nine names, then close the list [Esc]
  2. Click Delete Character From Pool, click Jurgen, then click Yes at "Delete Jurgen for good?" [Up/Down, Enter for each choice; Y for Yes]
  3. Click Save Character Pool [Up/Down, Enter]
  4. Leave the pool menu [Esc], click Start New Game, then Use My Party Disk [Up/Down, Enter for each choice]
  5. Click View Character [Up/Down, Enter], read the saved list, then close it [Esc]
EVALUATE:
  - The starting list has nine names, beginning with Jurgen; deleting him returns to the pool menu
  - Save Character Pool returns to the pool menu without a write-error message
  - After loading My Party Disk, View Character shows Verti, Dennar, Effane, Magellan, Alaric, Ripley, Ace and Alexandra, with Jurgen still absent
  - The Party list stays empty: saving the pool does not add a Traveller to the playing party
