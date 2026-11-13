# T110 Treat a wounded Traveller in the sick bay

OPEN AT: The Medical station aboard the Interloper at Efate, Verti the medic, Magellan wounded in the sick bay, his portrait and current and full hits showing.
SETUP KEYS: snapshot acceptance/local/atoms/110.json
ACTIONS:
  1. Click: Heal > Magellan   [H, Enter]
  2. Wait: his hits rise
  3. Click: S-BAY (station strip) > In-sys travel   [V, Right, Enter, Esc]
EVALUATE:
  - Medical names Verti and lists Magellan with fewer current hits than full hits
  - Healing raises Magellan's current hits while he stays in the sick bay
  - The Crew station shows the same patient; In-sys travel returns to flight
