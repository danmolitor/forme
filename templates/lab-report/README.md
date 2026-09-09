# lab-report

The certificate of analysis: results against reference ranges with exactly one flagged out-of-range value — accent, bold, set larger, and captioned "Exceeds", never colour alone. Change the sample block and the results rows in `data.json` first; keep at most one flagged row per the accent policy. Caveat: spec limits are written in the set's "max" idiom ("1.3 max"), not "≤" — the bundled Unicode fallback font does not cover the math-operator block, and the engine reports any such uncovered glyph as a render defect; register a font containing it if you need the symbol form.

Engine features: solid accent tab via flex shrink-wrap, per-cell flagged styling in a data table, three-slot signature row, single-page fill.

Render: `npx @formepdf/html templates/lab-report/index.html -o lab-report.pdf`
