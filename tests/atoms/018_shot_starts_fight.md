# T018 An off-screen target cannot start a fight

OPEN AT: The party split on Efate beside a lake, nobody else in view, a hostile thug waiting out of view to the east, no fight yet
SCENARIO: atom_shot
SETUP KEYS: snapshot acceptance/local/atoms/018.json
ACTIONS:
  1. Click the portrait of Jurgen (or press Tab: with no enemy in view it only steps through your own Travellers; that is correct here)
  2. Click Fire (or press F): nothing to fire at
EVALUATE:
  - Tab never names a thug (the only one is out of view)
  - After F the status line says no target: press Tab; no shot appears in the log
  - No fight starts on the off-screen thug
  - The initiative row stays empty and the status line still says Party split
