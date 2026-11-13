# T097 Leave the starport and return through its door

OPEN AT: The Efate starport services, with Leave beside the EFATE STARPORT title. The party is inside at the desk.
SETUP KEYS: snapshot acceptance/local/atoms/097.json
ACTIONS:
  1. Click Leave beside the starport title [Esc]
  2. Click the starport door immediately west of the party [Left arrow]
EVALUATE:
  - Leave closes the services and shows the outdoor map with the party just east of the door
  - Entering the door opens the Efate starport services again
  - The same Travellers and balances remain; no second click on Leave is needed
