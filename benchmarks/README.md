# Forme benchmarks

Honest, CI-generated performance evidence for the Forme engine — measured, not
asserted. The results are emitted into the same parity artifact pipeline as the
conformance and determinism sections (`scripts/parity/*`, `parity.json`) and
published at [parity.formepdf.com](https://parity.formepdf.com), so they carry
provenance (commit, run, timestamp, machine) and cannot drift from reality.

This is not an optimization exercise. The goal is knowing the true numbers and
publishing them honestly — **including anywhere Forme loses.**

## The corpus (`corpus/`)

A **fixed, committed** six-document set that exercises the shapes real users
have, not synthetic microbenchmarks. Everything is HTML/CSS on purpose: it is
the one input surface shared by every Forme target (native `forme-html`,
`@formepdf/html` on node / web / workerd) **and** by Puppeteer — so
Forme-vs-Forme and Forme-vs-Puppeteer both run on byte-identical documents.

| Document | Shape | Pages |
|---|---|---|
| `receipt.html` | Simple single page | 1 |
| `report-6p.html` | Prose report + one table (continuity with the legacy ~26ms figure) | 6 |
| `invoice-50p.html` | Table-heavy statement: repeating `<thead>`, zebra rows, many breaks | 50 |
| `ledger-500p.html` | The 500-page stress shape (the QuestPDF-thread document) | 500 |
| `letterhead-paged.html` | Paged-media overhead: `@page` margin boxes, running header/footer, `counter(page)` | 5 |
| `compliance.html` | Rendered **plain** and as **PDF/UA-1 + PDF/A-2b** — the delta is the cost of conformance | 2 |

### Determinism

Every document is a pure function of constants in `corpus/generate.mjs` — no
`Date.now()`, no randomness, no locale. Regenerating produces byte-identical
HTML, so published numbers stay comparable across runs and machines. The
generated `.html` files and `manifest.json` (a `sha256` of each) are committed.

```bash
node benchmarks/corpus/generate.mjs          # regenerate the corpus + manifest
node benchmarks/corpus/generate.mjs --check    # fail if on-disk drifts from the generator
```

## Method (summary)

- **Targets:** native binary, node WASM, web WASM (headless Chromium), workerd
  isolate. Puppeteer is run as a same-machine baseline on the identical HTML,
  browser reused across warm iterations, cold start reported separately.
- **Cold start** = process/isolate start → first PDF byte, *including* module
  instantiation (the ~6.4 MB core / ~7.1 MB html WASM is part of the cost and
  is reported as such).
- **Warm render** = steady state; median + p95 over a reported iteration count.
- **Memory:** native = max RSS; node/WASM = WASM linear-memory high-water
  **and** process RSS, each captioned with what it does and does not capture.
- **Two machines, both published:** CI `ubuntu-latest` (headline — reproducible,
  provenance-stamped, but a noisy shared runner that understates the engine) and
  a fixed dev machine (quiet, single run). No cherry-picking.

Full methodology and caveats are stamped onto the published page alongside the
numbers.
