# T200 Launch a missile at a hostile ship

OPEN AT: The Interloper pilot station in an encounter with one hostile ship. GUN2 has a missile launcher and one Constant Guided missile loaded in tube 1; Target and Launch programs are running.
SETUP KEYS: Open snapshot acceptance/local/atoms/200.json. Preparation buys the launcher and missile at the shipyard and loads tube 1 through Reload.
ACTIONS:
  1. Click: GUN2 (station strip)   [V, Right, Right, Enter]
  2. Read: the title line of the gun panel (it says what to click next, every time)
  3. Click: the right purple arrow x4, until the line reads 'Click Tar (red cross-hair)'   [Right x4]
  4. Click: Tar (the button with the red cross-hair, bottom right, between Rel and Fire) > tube 1 (bottom left)   [T, 1]
  5. Click: Fire, when the line reads 'Sight on ship? Click Fire'   [Enter]
EVALUATE:
  - The title line names the thing to click as the screen labels it: 'Purple arrows: find hostile', 'Click Tar (red cross-hair)', 'Click tube 1', 'Sight on ship? Click Fire'
  - A click on a tube or on Fire before Tar changes nothing and the line reads 'Click Tar first'
  - The pointer over a tube, Rel, Tar or Fire shows a tip that says what it is (Tar = Target, Rel = Reload) and its key
  - Firing launches the missile at the hostile; in this prepared encounter the hostile is defeated and its cargo screen opens
