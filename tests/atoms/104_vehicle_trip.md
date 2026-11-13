# T104 Board, turn and leave a parked vehicle

OPEN AT: The outdoor raised city on Efate: the party is on foot on the paved path, with its rented vehicle two squares directly below it.
SETUP KEYS: snapshot acceptance/local/atoms/104.json
ACTIONS:
  1. Walk: onto the parked vehicle   [Down twice]
  2. Turn: north toward the approach path   [Up once]
  3. Click: Vehicle > Status   [V, S]
  4. Click: Vehicle > Leave   [V, L]
EVALUATE:
  - Walking onto the parked vehicle boards it
  - Up once turns north in place; the square ahead is the open approach path
  - Status says the vehicle is in perfect shape
  - Leave puts the party on foot one square above the original parking square
  - The vehicle remains on the map after Leave
  - "You can't park your vehicle here." is correct when the square ahead is blocked for vehicles or pedestrians
