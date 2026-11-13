# T135 Read the next skill page and return to the first

OPEN AT: Magellan's character screen on quiet Efate ground, showing the first page of his skills on the left. This practice character has enough skills to fill another page.
SETUP KEYS: snapshot acceptance/local/atoms/135.json
ACTIONS:
  1. Click the down arrow beside SKILLS [PgDn]
  2. Click the up arrow beside SKILLS [PgUp]
EVALUATE:
  - The down arrow shows later skills instead of the first page.
  - The up arrow restores the original list and skill levels.
  - Magellan's name, equipment and pack stay the same while the skills change pages.
