# T087 Pause in flight

OPEN AT: The Interloper in space above Efate, the flight picture filling the game area; the panel rows read Stations, Comms, Jump, Quit, Pause. The flight opens paused: PAUSED - press P to fly.
SETUP KEYS: snapshot acceptance/local/atoms/087.json
ACTIONS:
  1. Press P: the flight starts (it opened paused)
  2. Press P again (or click the Pause row): the ship stands still under PAUSED
  3. Press P once more
EVALUATE:
  - PAUSED shows over the view and nothing moves while it does
  - The second P removes it and the flight goes on where it stood
  - Hover over any panel control names it and its key (Thrust up - Up arrow, Pause - P)
