# T193 Read the explored area on the Map page

OPEN AT: The quiet Efate street outside the starport, with the grouped party between the blue-and-white starport pennants.
SETUP KEYS: Open snapshot acceptance/local/atoms/193.json (the same quiet Efate scenario as T002).
ACTIONS:
  1. Click the purple Plus page button on the menu bar [F12], then click Map
  2. Find the small explored patch around the starport and the purple party marker
  3. Close Plus [F12], click the adjacent street square to the left [Left], then reopen Plus and click Map
EVALUATE:
  - The Map page shows the Efate area fitted inside the page; the street and starport tiles around the starting party are visible and unexplored squares are dark
  - A purple marker marks Magellan and the grouped party at the starting street square, matching the ground view
  - After walking left, the marker moves one square west within the same Efate area; already explored tiles remain visible
