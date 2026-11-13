# T133 Inspect an item in the character screen

OPEN AT: Magellan's character screen with INSPECT OBJECT selected and the blue detail panel for the 9mm auto pistol open at the lower right.
SETUP KEYS: snapshot acceptance/local/atoms/133.json
ACTIONS:
  1. Read the pistol's detail panel
  2. Click the detail panel to dismiss it [Space]
  3. Click View [Enter] to inspect the next item in the pack
  4. Click the detail panel to dismiss it [Space]
EVALUATE:
  - The pistol panel names the weapon and shows its reloads, weight and value.
  - Dismissing the panel returns to the character screen with INSPECT OBJECT selected.
  - View opens details for the flak jacket next; dismissing it leaves all pack items intact.
