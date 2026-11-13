# T022 Click a square and the party walks there

OPEN AT: The Efate starport yard, the party standing on the paving below the launch pad, nobody in sight
SCENARIO: efate_quiet
SETUP KEYS: snapshot acceptance/local/atoms/022.json
ACTIONS:
  1. Click a paved square about five squares to the south-west and let go
EVALUATE:
  - The party walks the whole way
  - The party stops on that square
