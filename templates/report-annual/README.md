# report-annual

The annual report excerpt: a full-bleed accent cover with the monogram knocked out in outline, a dot-leader contents page, and a financial-highlights page whose income statement reconciles top to bottom. Change the year, the KPI values and the income statement in `data.json` first; the cover strap should restate the headline numbers. The folios are editorial ("Page 2", feet 02/03) because these are excerpt pages of a 78-page report, so a "Page n of m" counter would mislead — the cover carries no counter, following the design.

Engine features: named pages (cover / contents / highlights) with per-page margin-box overrides and suppression, full-bleed via absolute margin strips, CSS grid KPI rows, dotted-leader flex rows, `break-before: page`.

Render: `npx @formepdf/html templates/report-annual/index.html -o report-annual.pdf`
