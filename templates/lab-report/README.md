# lab-report

The certificate of analysis: results against reference ranges with exactly one flagged out-of-range value — accent, bold, set larger, and captioned "Exceeds", never colour alone. Change the sample block and the results rows in `data.json` first; keep at most one flagged row per the accent policy. Caveat: the ≤ and ₃ glyphs need a registered font (as in the PDF/UA render) — the base-14 preview substitutes "?" because the built-in Unicode fallback does not engage for unregistered base-14 faces.

Engine features: solid accent tab via flex shrink-wrap, per-cell flagged styling in a data table, three-slot signature row, single-page fill.

Render: `npx @formepdf/html templates/lab-report/index.html -o lab-report.pdf`
