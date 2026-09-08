# product-catalog

Section 4 of the product catalogue, two pages: product cards with captioned image placeholders and accent-emphasised prices, a dimensions table, and grouped options with surcharges. Change the products, prices and options in `data.json` first; real photography drops into the placeholder slots at the same aspect. Caveats: the placeholders are bordered and captioned on the flat light ground — the design's 45° hatch is a gradient, which the engine subset excludes — and the folios are the catalogue's own (Page 4.1 / 4.2), carried by named pages instead of `counter(page)`.

Engine features: named page for per-page folio strings, running header on the continuation page, CSS grid product cards (2-up and 3-up), grouped-option table, continuation marker.

Render: `npx @formepdf/html templates/product-catalog/index.html -o product-catalog.pdf`
