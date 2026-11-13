# T191 Read the Game log from the Efate map

OPEN AT: The quiet Efate street outside the starport. All five Travellers are together; the message strip says You aren't facing anybody.
SETUP KEYS: Open snapshot acceptance/local/atoms/191.json (T002's quiet Efate scenario after Converse).
ACTIONS:
  1. Click the purple Plus page button on the menu bar [F12], then click Log (Game log)
  2. Read the latest log line; scroll to the beginning and end [Home, End]
  3. Close Plus [F12], click Converse [C] while still facing the empty street, then reopen Plus and click Log
EVALUATE:
  - The Log page includes You aren't facing anybody., matching the message strip in this opening
  - The short log remains readable at either scroll limit without blanking the line
  - Converse leaves the party on the same square, and the reopened log still contains that response
