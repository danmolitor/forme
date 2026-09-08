# packing-slip

The packing slip: ordered/shipped/back-order accounting with no prices shown, a back-order explanation, and per-package weights that sum to the shipment. Change the line rows and package detail in `data.json` first; the totals row and the label/delivery-note weights must stay consistent (they reconcile across the logistics chain R-07 → QA-2026-2087 → PS-88214 → the label → DN-40118).

Engine features: quantity-accounting table with subtotal row, flex column split, single-page fill.

Render: `npx @formepdf/html templates/packing-slip/index.html -o packing-slip.pdf`
