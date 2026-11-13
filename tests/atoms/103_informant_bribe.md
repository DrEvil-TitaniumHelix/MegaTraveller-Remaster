# T103 Pay a Boughene informant

OPEN AT: The bribe screen in Boughene's bar: the informant's portrait is at the right; the message shows Magellan's cash and proposed bribe, with Offer, Increase, Decrease and Back below.
SETUP KEYS: snapshot acceptance/local/atoms/103.json
ACTIONS:
  1. Click Increase [I], then 10000 [Down twice, Enter]
  2. Click Done [Esc] to return to the offer
  3. Click Offer [O] and read the answer
  4. Click Back [Esc], then open Magellan's record [F1]
EVALUATE:
  - Increasing the offer changes the proposed bribe without spending it yet
  - The paid informant reveals information and returns to Talk, Bribe and Back
  - Magellan's personal credits fall by the offered amount
