# T194 Save a report from the Efate map

OPEN AT: The quiet Efate street outside the starport, with all five Travellers together. This is the screen the report should capture.
SETUP KEYS: Open snapshot acceptance/local/atoms/194.json (the same quiet Efate scenario as T002). The actions create a local sample report.
ACTIONS:
  1. Click: Plus page (purple button, menu bar) > Bug   [F12]
  2. Click: Question > the text box; type: T194 Efate report check
  3. Click: Save report   [Enter]
  4. Read the confirmation; open the folder it names; look at screen.png and bundle.json
  5. Close Plus   [F12]
EVALUATE:
  - The Bug page offers Bug, QoL request and Question above a text box, with Save report below; Question becomes selected and the typed sentence is visible
  - Saving clears the text box and shows Saved to followed by a local report folder
  - screen.png shows this Efate street; bundle.json contains the typed sentence, kind question, world 2, area 0, level 0, cell [32,9], and the build ID shown by the running game's About screen
  - The report journal lists J1 and J2 as unlocked, and closing Plus returns to the same party on Efate
