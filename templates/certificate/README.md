# certificate

The certificate of completion: an engraved double frame (accent outline offset around the main frame), centred Georgia setting, an accent underline beneath the recipient, and the retainage-release note that closes the INV-4421 contract chain. Change the recipient, works and dates in `data.json` first. Caveat: the double frame is two nested bordered boxes with explicit heights — the subset has no `outline` property — so adjusting the page margins means re-deriving those two heights.

Engine features: own `@page` margins, nested bordered frames with content-box height arithmetic, column flex centring, 6px head rule variant.

Render: `npx @formepdf/html templates/certificate/index.html -o certificate.pdf`
