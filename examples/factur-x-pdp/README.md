# Factur-X via a PDP / conversion service

The runnable example behind
[docs.formepdf.com/guides/e-invoicing-with-a-pdp](https://docs.formepdf.com/guides/e-invoicing-with-a-pdp):
Forme renders a French invoice as **PDF/A-3b** (veraPDF-checked before it
leaves the process), a conversion service embeds the **EN 16931** semantic
model, and the returned **Factur-X** is verified with veraPDF and
Mustangproject.

```
node examples/factur-x-pdp/render-and-convert.mjs
```

Run from the repo root with packages built (`npm install` at the root,
then build `packages/shared` → `fonts-standard` → `html` per the build
order in `/CLAUDE.md`). Outputs land in `out/`.

- `invoice.html` — the invoice, Northmoor's invoice-standard template with
  French parties, EUR amounts, and 20% VAT, so the human-readable PDF and
  the machine-readable XML agree.
- `en16931-invoice.json` — the EN 16931 semantic model for the same
  invoice (SuperPDP's JSON encoding; every field annotated with its
  business term in their OpenAPI spec). The mapping table and the French
  CTC additions are documented on the docs page.
- `render-and-convert.mjs` — render → pre-flight veraPDF → multipart POST
  → verify the returned file (veraPDF PDF/A-3b + Mustang).

Env: `SUPERPDP_URL` (default `https://api.superpdp.tech`, whose convert
endpoint is documented as unauthenticated), `SUPERPDP_TOKEN` (optional
bearer), `VERAPDF`, `MUSTANG_JAR` (validators; skipped with a notice if
absent).

Last verified end to end 2026-09-11: pre-flight PASS, conversion 200,
returned file PASS under veraPDF 1.30.2 (PDF/A-3b) and Mustang 2.26.0
(EN 16931 + Flux2 schematron, profile `urn:cen.eu:en16931:2017`, zero
errors). Known finding, stated on the docs page: sending payment
instructions (BG-16 with a valid IBAN) made the endpoint reject its own
CII serialization under `CII-SR-470`, so the example omits BG-16.
