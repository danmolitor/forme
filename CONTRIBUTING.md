# Contributing to Forme

Thanks for wanting to help. Forme is a document engine: a Rust core compiled to
WASM, wrapped by npm packages that let you render PDFs from HTML/CSS or from
React/Svelte/Vue/Preact components. This guide gets you from a clean clone to a
green test run, and explains the few house rules that keep the project
coherent.

## What lives where

The repo is an npm workspace (`packages/*`) around a Rust core.

| Path | What it is |
|------|-----------|
| `engine/` | The Rust engine (`forme-pdf` on crates.io) — layout, text shaping, PDF writing, PDF/UA + PDF/A. The source of truth for how a document renders. |
| `html/` | The Rust `@formepdf/html` crate — parses HTML + CSS into the engine's document model. This is the "bring your own HTML" path. |
| `packages/core` | WASM build of the engine plus the JS render entry points (`renderDocument`, `@formepdf/core/browser`). |
| `packages/html` | JS wrapper + CLI for the `html` crate — `npx @formepdf/html`, `renderHtml()`, Workers build. |
| `packages/react`, `svelte`, `vue`, `preact` | Component authoring layers. Identical props, identical output; they all serialize to the same document model. |
| `packages/shared` | The shared document model + serializer the framework packages build on. |
| `packages/fonts-standard` | Metric-compatible base-14 fonts (Liberation, SIL OFL) for PDF/UA + PDF/A embedding. Optional, so core carries no font payload. |
| `packages/tailwind`, `cli`, `mcp`, `hono`, `next`, `resend`, `renderer` | Adapters and tooling. |
| `packages/vscode` | The VS Code extension. |
| `templates/` | The shipped example templates, also used as conformance fixtures. |
| `scripts/` | CI verification scripts — including the veraPDF conformance gates and the parity page generator. |
| `docs/` | Mintlify docs (published at docs.formepdf.com). |

`CLAUDE.md` has a deeper architecture tour, the compliance gates, and the
release flow.

## Setup

You need **Rust** (the toolchain is pinned to 1.98.0 by `rust-toolchain.toml`,
so rustup selects it automatically — you don't set it yourself), **Node 20**,
and **wasm-pack** for the WASM builds:

```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

Then, from a clean clone:

```bash
npm install                 # installs all workspaces
cd engine && cargo test     # builds + tests the engine (this is the long one)
```

The engine build dominates first-run time — expect several minutes cold, fast
after that. The JS packages are quick by comparison.

## Running tests per area

Work in the package you're touching and run its tests. What "green" looks like:
every suite passes with zero failures, and no test is skipped to get there.

```bash
# Engine (Rust) — lib + integration tests
cd engine && cargo test

# HTML crate (Rust)
cd html && cargo test

# A framework package (React shown; svelte / vue / preact / core the same)
cd packages/react && npm test

# HTML JS wrapper + CLI
cd packages/html && npm test          # smoke + links
cd packages/html && npm run test:workers   # Cloudflare Workers target

# Formatting / lint before you push
cd engine && cargo fmt && cargo clippy
```

CI runs the same commands across the engine, the html crate, every package, a
coverage pass, and the PDF/UA + PDF/A conformance gates. If it's green locally
in the package you changed, it's usually green in CI.

## Conformance and the parity page

Two things are validated on every push and must stay true:

- **PDF/UA-1 and PDF/A-2** — the shipped templates and HTML fixtures are
  validated with veraPDF as a CI gate (`scripts/verify-pdfua.mjs`,
  `scripts/verify-pdfa.mjs`). A change that regresses conformance fails CI.
- **[parity.formepdf.com](https://parity.formepdf.com)** — a public page
  generated from that CI output on every commit to `main`. It is never
  hand-edited; it renders whatever the run produced.

If you touch the engine's PDF writing, tagging, or font path, run the verify
scripts locally (they need [veraPDF](https://verapdf.org) installed) before
opening the PR.

## House rules

A few conventions that aren't obvious from the code but keep it honest:

- **Fail loud, never silently drop.** The HTML path's contract is that anything
  outside the [documented subset](https://docs.formepdf.com/html) is named in
  `warnings` — never quietly ignored. If you add handling for a property, make
  the unsupported-value branch push a warning that names the property and the
  remedy. Look at `letter-spacing` / `float` in `html/src/css.rs` for the
  pattern.
- **Keep the subset table in sync.** If you add or change what CSS the HTML
  path accepts, update the subset docs (`docs/html.mdx`) in the *same* commit.
  The docs are the spec; drift between them and the code is a bug.
- **Tests come with fixes.** A bug fix includes a test that fails without it and
  passes with it. A new CSS property or component includes a test that exercises
  it.
- **Engine changes: propose first.** Changes to `engine/` — new layout
  behavior, PDF structure, the style model — affect every package and the
  conformance gates. Open an issue describing the change before writing it, so
  we can talk through the layout/PDF implications. Wiring an *already-supported*
  engine feature through the HTML mapper or a framework package doesn't need
  this — that's exactly the kind of change that makes a good first PR.
- **Match the surrounding code.** Rust is `cargo fmt` + `cargo clippy` clean.
  TypeScript is strict. Follow the idioms already in the file you're editing.

## Contributor License Agreement

Forme is MIT-licensed and stays MIT-licensed. The CLA exists only so the option
to relicense in the future isn't accidentally closed off — you keep full rights
to your own work, and it costs you nothing.

It's a one-time, automatic step: when you open your first pull request, a bot
comments with instructions, you reply with a single comment, and you're done for
all future contributions. See [CLA.md](./CLA.md) for the text.

## Opening a PR

Fork, branch, make the change with its test, run the relevant package's tests
and `cargo fmt`/`clippy`, and open the PR. For anything more than a typo or a
small fix, an issue first is appreciated — it saves you from building something
that doesn't fit the direction.
