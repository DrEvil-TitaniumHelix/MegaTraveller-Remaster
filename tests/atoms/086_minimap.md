# T086 The area map is always there

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad; on the right, above the Traveller panel and below the command bar, the area map with a white frame on it
SETUP KEYS: snapshot acceptance/local/atoms/086.json
ACTIONS:
  1. Look right: the area map, the white frame around the part you see
  2. Press N twice: the map hides, its space stays, then it shows again
  3. Resize the window: the map keeps its place and shape
EVALUATE:
  - The map sits directly above the Traveller panel, under the command bar, at any window size
  - N hides and shows it; the Traveller panel never moves
  - The hover text over it reads Area map: the frame is your view
