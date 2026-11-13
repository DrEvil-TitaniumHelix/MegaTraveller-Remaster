# T129b Continue Old Game from Welcome

OPEN AT: The title Welcome menu, with Start New Game selected.
SETUP KEYS: snapshot acceptance/local/atoms/129b.json
ACTIONS:
  1. Click: Continue Old Game   [Down, Enter]
EVALUATE:
  - With more than one saved game: a list headed Continue which game? shows them (name, place, day, credits, date)
  - With one saved game: it loads at once; with none: the message No saved game.
