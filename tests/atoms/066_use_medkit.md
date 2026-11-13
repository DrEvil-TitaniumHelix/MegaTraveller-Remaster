# T066 Use a medkit on a wounded Traveller

OPEN AT: Efate, nobody in sight, Magellan wounded (8 of 21 hits) with a tl9 med kit in his pack
SCENARIO: m4b_medkit_in_pack
SETUP KEYS: snapshot acceptance/local/atoms/066.json
ACTIONS:
  1. Click Items in the side panel (or press I)
  2. Click Use in the side panel (or press Down, then Enter)
  3. Click char 1 (or press Enter)
EVALUATE:
  - Items offers Take and Use
  - Use asks which Traveller: char 1 to char 5
  - Magellan stays at 8 hits and the medkit in his pack is not consumed; Use only reads held items

Oracle: `MegaTraveller/match/fc61.c:31-51` reads only primary/secondary hands (slots 21/22), then asks for a target for a held medkit. This setup has a medkit in the pack, so these three keys do not heal. Once held and targeted, `match/f8c4.c:66-83` restores missing hits and spends at least one medkit charge. No rule change.
