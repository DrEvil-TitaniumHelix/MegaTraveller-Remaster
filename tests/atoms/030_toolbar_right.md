# T030 Toolbar: Reload, Next, Travellers

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/030.json
ACTIONS:
  1. Click Reload
  2. Click Next
  3. Click the Traveller 1 button, then Close on the record screen
  4. Click Next, then the Traveller 5 button, then Close
  5. Click a greyed button
EVALUATE:
  - Each opens what its key opened; Reload outside a fight only writes a log line (Magellan reloads, or nothing to reload)
  - The Traveller buttons open that Traveller's screen
  - A greyed button (Orders outside a fight) does nothing: no message, no change
