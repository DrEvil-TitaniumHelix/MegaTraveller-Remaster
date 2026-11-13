# T068 Police checkpoint and weapons search

OPEN AT: Alell, the party one square east of the police post, carrying weapons the law forbids, two offences on record
SCENARIO: m5_police_provoked
SETUP KEYS: snapshot acceptance/local/atoms/068.json
ACTIONS:
  1. Click the square west of the party (or tap the West arrow once): the party steps 27 -> 26
  2. Click the next square west (onto the post): the post asks Will you submit to a search for illegal weapons? Click Yes (or press Space); by keyboard the arrow tap stops at 26 and nothing asks
EVALUATE:
  - By keyboard the party steps one square and the post asks nothing; by mouse the second click (onto the post) asks for the search and Yes walks the party on
  - The weapons remain with the party
  - No bail is charged; the party account remains 30,000 Cr

Oracle: `MegaTraveller/match/f21f2.c:26-31,61-62` only searches when `g_6d20` (outdoors) is zero, then clears the key latch on every exit. This scenario starts with `outdoors = true`; the post does not stop it. Confiscation (`f2097.c`) and bail (`f1b89.c`) are separate flows, not consequences of this crossing. No rule change.
