# report-monthly

The monthly operating report: eight KPI blocks over a performance-against-plan table, commentary, and carried-forward actions with owners. Change the period, the KPI values and the performance rows in `data.json` first; segment rows must sum to the revenue row. The KPI figures use the accent keyline treatment; nothing else on the page is coloured.

Engine features: CSS grid KPI rows, grouped-subtotal table rows, flex column split with explicit percentage widths, single-page fill with footer push.

Render: `npx @formepdf/html templates/report-monthly/index.html -o report-monthly.pdf`
