# T013 End Turn fires the standing target, then ends

OPEN AT: The ambush on Efate, Magellan to act with a standing target on a thug, the status line offering to fire
SETUP KEYS: snapshot acceptance/local/atoms/013.json
ACTIONS:
  1. Click End turn (or press E)
EVALUATE:
  - Magellan fires at the standing target
  - The status line moves on to the next Traveller
  - Magellan gets no more this round
