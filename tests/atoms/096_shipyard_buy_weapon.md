# T096 Fit and sell a weapon on the Interloper

OPEN AT: The Efate shipyard, Buy Weapons open at Weapon System / Cost, listing beam lasers, pulse lasers and a missile launcher. The Party Account is paying and the Interloper has an empty gun bay.
SETUP KEYS: snapshot acceptance/local/atoms/096.json
ACTIONS:
  1. Click: the first TL8 Beam Laser (pwr:25)   [Enter]
  2. Click: Sell Weapons   [Left, Enter]
  3. Click: Party Account > TL8 Beam Laser (pwr:25)   [Down x5, Enter]
  4. Click: Sell Weapons > Party Account: look at what is left
EVALUATE:
  - The chosen laser costs the amount shown and the starport services return
  - Sell Weapons asks who will get the money, then lists the installed laser
  - Selling the laser credits its displayed offer to the Party Account and removes that laser from the installed-gun list
