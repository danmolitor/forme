# inspection-report

The final inspection checklist, form QA-16: grouped check sections with pass/fail/N-A checkbox columns, an observations block, a disposition box, and a three-slot signature row. Change the job block and the check rows in `data.json` first. Caveat: the checklist is the set's densest page — its row rhythm is deliberately tighter than the shared table spine to hold one page under the engine's Times metrics (tolerances use the "max" idiom — see lab-report for the glyph-coverage note).

Engine features: grouped-header table sections, checkbox cells as bordered blocks, `break-inside`-safe single-page fill dropped for natural flow at the knife edge.

Render: `npx @formepdf/html templates/inspection-report/index.html -o inspection-report.pdf`
