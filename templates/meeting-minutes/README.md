# meeting-minutes

The operations-review minutes: numbered discussion items in the clause voice, then an actions table where every row has an owner, a date and a status. Change the attendance block, the items and the actions in `data.json` first; the content cross-references the wider document set (INV-4102, R-07, PS-88291), so renumber those if you fork the fixture universe.

Engine features: numbered flex items with fixed number column, actions table, three-column meta grid, single-page fill.

Render: `npx @formepdf/html templates/meeting-minutes/index.html -o meeting-minutes.pdf`
