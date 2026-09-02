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

## Emitting the evidence

`scripts/parity/benchmarks.mjs` measures one run for the current environment and
emits `$PARITY_DIR/benchmarks.json` (same `emitSection` contract as the other
parity sections). The artifact carries **two runs**:

- **`ci`** — measured fresh on every push to `main` by the `benchmarks` CI job
  (a shared `ubuntu-latest` runner: the conservative, current-commit column).
- **`dev`** — a committed baseline at `benchmarks/results/dev.json`, measured on
  quiet hardware. In CI the emitter loads this baseline and merges it, so the
  page shows both columns. If the CI job is skipped or fails, `assemble.mjs`
  falls back to this committed baseline so the page still renders the dev column.

Each run records `measuredAtCommit` and `measuredAt`, and the page labels a run
that lags the commit being served as **"may lag current commit."**

### Regenerating the dev baseline

Run on a quiet machine when an engine change legitimately moves performance:

```bash
BENCH_SAVE_BASELINE=1 PARITY_DIR=/tmp/pb node scripts/parity/benchmarks.mjs
git add benchmarks/results/dev.json && git commit -m "benchmarks: refresh dev baseline"
```

**Staleness policy.** Treat the **CI column as the current-commit truth** — it is
re-measured on every push to `main`. The **dev column is a periodically-refreshed
clean-hardware reference**, not a per-commit number. Because every run is stamped
with the commit it was measured at, a lagging dev column is *labeled* rather than
silently wrong, so you do **not** need to regenerate it on every perf-affecting
merge — refresh it after a batch of perf work lands (e.g. once the tracked
layout/memory fixes are in) or whenever the "may lag" label has drifted far
enough to be misleading. Do not gate merges on regenerating it.

### Observing a real runner before merging

The `benchmarks` job also runs on `workflow_dispatch`, so you can trigger it on a
feature branch to watch how a real shared runner handles the corpus (notably the
500-page ledger) *before* the code is on `main`. Dispatch never deploys Pages
(that stays `main` + `push` only).
