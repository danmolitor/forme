# shipping-label

The 4 × 6 in thermal shipping label: its own page size, a die-cut ink border, and pure black throughout — the one document in the set with no accent, because thermal printers have no colour to give. Change the carrier, ship-to and pro-number fields in `data.json` first. Caveat: the barcode is a bordered placeholder captioned with the pro number — the engine's Code 128 support is not reachable from the HTML input path, and a fake-real barcode would be worse than an honest placeholder; generate a real symbol at build time if you wire this to a carrier. No page counter, per the design (a die-cut label is not a paged document).

Engine features: custom `@page` size (288 × 432pt), zero-margin bordered frame, flex fact rows, footer push.

Render: `npx @formepdf/html templates/shipping-label/index.html -o shipping-label.pdf`
