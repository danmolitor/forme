# Changelog

## [0.12.0] - 2026-08-25

_Version bump only — 0.12.0 introduces the new `@formepdf/preact` adapter (Preact 10 authoring). No changes to this package._

## [0.11.1] - 2026-08-20

### Fixed
- WASM rebuilt against engine 0.11.1: `<Svg content="...">` with `stroke-linecap="round"` (or `stroke-linejoin="round"`) now renders round caps and joins instead of falling back to the PDF default. Fixes a reported handwritten-signature repro where 60+ short cubic-bezier paths showed visible flat rectangular cap protrusions at every segment terminus

## [0.11.0] - 2026-08-09

### Added
- `renderSerializedDoc(doc, options?)` and `renderSerializedDocWithLayout(doc, options?)` public exports. Accept a pre-serialized Forme document (`FormeDocument` JSON) and render it through the WASM engine. Used by `@formepdf/svelte`'s `renderDocument` / `renderDocumentWithLayout` helpers, which build the document via the Svelte SSR-then-parse pipeline before handing it to core

## [0.10.5] - 2026-06-29

### Fixed
- WASM rebuilt against engine 0.10.5: `<Table>` with `<Row header>` no longer leaves an orphan header at the bottom of a page when the header alone fits but the first body row does not. The same fix also addresses the long-token-header contamination edge case where a wrapped multi-line header rendered 3 pages with header text leaking onto a prior page

## [0.10.4] - 2026-06-05

### Fixed
- WASM rebuilt against engine 0.10.4, picking up four layout fixes:
  - `<Table>` with `<Row header>` no longer inflates page count 3–5× when the header doesn't fit before a page break (silent — produced "doubled and sliding column" header artifacts)
  - A `<View>` wrapping a `<Table>` no longer auto-grows to roughly the page height — Table/TableRow now have explicit measurement arms that match what `layout_table` actually renders
  - `<Svg viewBox="…">` content now scales to the display box via the SVG viewport algorithm (`xMidYMid meet` default) instead of rendering at raw viewBox coordinates and overflowing
  - `marginTop: 'auto'` on a child in a column-flex parent with fixed height now pushes the child to the bottom, matching the cross-axis behavior already shipped for flex rows

## [0.10.3] - 2026-05-28

### Fixed
- WASM rebuilt against engine 0.10.3: `<Text style={{ width }}>` inside a flex row now renders at the requested width instead of the parent row's full width. The 0.10.2 regression sized such text boxes to the row width, so `textAlign: 'right'` pushed glyphs off the page — silent corruption that byte-hash snapshots didn't catch.

## [0.10.2] - 2026-05-21

### Fixed
- WASM rebuilt against engine 0.10.2, picking up two layout fixes:
  - Flex row children with percentage widths (e.g. `width: '30%'` / `'70%'`) now resolve against the parent's content width instead of being double-resolved against their own already-distributed width. Two children at `width: '100%'` now shrink to 50/50 instead of collapsing
  - Grid containers that span a page break now move the whole row to the next page together — previously each cell triggered its own page break and the columns scattered across separate pages

## [0.10.1] - 2026-05-20

### Fixed
- **Cloudflare Workers crash on import** — 0.10.0's bundler-target `pkg/forme.js` called `wasm.__wbindgen_start()` at module load, which threw in Wrangler because Wrangler returns `{ default: WebAssembly.Module }` for direct `.wasm` imports instead of an instantiated namespace. The package now ships a third build, `pkg-web/` (wasm-pack `--target web`), and a new `dist/worker.js` entry that requires an explicit `init(wasmModule)` call. The `worker`, `edge-light`, and `deno` conditional exports now route here. Workers users can write the same `await init(wasm)` pattern that worked on 0.9.x.
- **Missing `pkg-node/` in the published tarball** — wasm-pack `--target nodejs` writes a `.gitignore` containing `*` inside its output dir, which `npm publish` honours by dropping the directory's contents. The `build:wasm` script removed `pkg/.gitignore` but not `pkg-node/.gitignore`, so 0.10.0's Node entry tried to import a file that wasn't in the tarball. The cleanup step now strips `.gitignore` from all three pkg dirs, and `prepublishOnly` runs an `assert-tarball.sh` check that fails publish if any required file is missing.

### Added
- `@formepdf/core/worker` subpath export for the new edge entry.
- `@formepdf/core/pkg-web/forme.js` and `/forme_bg.wasm` for direct WASM imports in Workers (legacy `pkg/forme_bg.wasm` still works).
- `packages/core/scripts/assert-tarball.sh` — invoked by `prepublishOnly` and by CI on every PR. Lists the required tarball contents and fails loudly if any pkg dir ships a stray `.gitignore`.

## [0.10.0] - 2026-05-19

### Changed
- WASM build now produces two targets: `pkg/` (bundler) for Vite / Webpack / Turbopack / Wrangler consumers, and `pkg-node/` (nodejs, CJS) for Node SSR. The previous `--target web` build shipped `forme.js` without `forme_bg.js`, which broke static-analysis bundlers (Next.js / Turbopack) that resolve WASM imports at build time
- Node entry (`src/index.ts`) now imports from `pkg-node/forme.js` and drops the manual `fileURLToPath` + `readFile` + `initWasm` dance — the nodejs target self-initializes
- Browser entry (`src/browser.ts`) imports named exports directly from `pkg/forme.js`; bundlers instantiate the WASM implicitly. `init()` is kept as a deprecated no-op for backward compatibility
- Picks up engine 0.10.0's six new visual properties (opacity cascade, wordSpacing, rounded clipping, boxShadow, page backgroundImage, CSS gradients)

## [0.9.2] - 2026-04-28

_Version bump only._

## [0.9.1] - 2026-04-06

_Dependency bump only._

## [0.9.0] - 2026-04-04

### Added
- Engine: PKCS#1 private key auto-conversion for digital signatures
- Engine: WASI timestamp fix for Python/Go SDK WASM builds

## [0.8.3] - 2026-04-01

### Added
- SVG opacity support and Svg children API (via engine + react rebuild)

### Fixed
- Page style inheritance: `<Page style={{ fontFamily }}>` now propagates to children

## [0.8.2] - 2026-03-30

### Fixed
- Engine fix: PDF serializer now respects custom font weights (multiple weights per family no longer collapse to 400/700)

## [0.8.1] - 2026-03-30

### Fixed
- Engine fixes: Latin Extended character widths, page number placeholder measurement, two-pass digit-accurate rendering

## [0.8.0] - 2026-03-29

### Added
- WASM rebuilt with AcroForm, PDF/UA, PDF/A, and digital signature support
- `flattenForms` option in `renderDocument()` and `renderDocumentWithLayout()`
- `signPdf()` function for signing existing PDFs without re-rendering

## [0.7.13] - 2026-03-28

### Added
- WASM rebuilt with engine-native chart support (BarChart, LineChart, PieChart, AreaChart, DotPlot)
- `renderSerializedDoc()` and `renderSerializedDocWithLayout()` in browser entry point

## [0.7.12] - 2026-03-24

### Fixed
- Added `worker`, `edge-light`, `deno`, `react-native`, and `browser` conditional exports so edge runtimes (Cloudflare Workers, Vercel Edge, Deno Deploy, Netlify Edge, React Native/Expo) automatically resolve to the browser entry point — no manual `@formepdf/core/browser` import needed

## [0.7.11] - 2026-03-23

_Dependency bump only._

## [0.7.9] - 2026-03-17

### Changed
- Rebuilt WASM with barcode support and wasm-raw C-ABI exports

## [0.7.8] - 2026-03-17

### Added
- Barcode support in WASM engine (Code 128, Code 39, EAN-13, EAN-8, Codabar)

## [0.7.7] - 2026-03-16

### Added
- Browser entry point (`@formepdf/core/browser`) for client-side PDF generation
- `init()` function to pre-load WASM or provide a custom URL/bytes
- Browser-native `extractData()` using DecompressionStream (no node:zlib)

## [0.7.6] - 2026-03-13

### Added
- `renderDocument(el, { embedData })` option to attach JSON as a PDF file attachment
- `extractData(pdfBytes)` to read embedded JSON back from Forme-generated PDFs

## [0.7.3] - 2026-03-07

_No changes._

## [0.7.2] - 2026-03-07

### Fixed
- Rebuilt WASM with bundled Noto Sans fonts (0.7.1 published without them)

## [0.7.1] - 2026-03-07

_No changes._

## [0.7.0] - 2026-03-06

_No changes._

## [0.6.2] - 2026-02-21

_No changes._

## [0.6.1] - 2026-02-14

_No changes._

## [0.6.0] - 2026-02-07

### Added
- `renderTemplate()` and `renderTemplateWithLayout()` WASM bindings
- Font source resolution (file paths, data URIs, Uint8Array to base64)

## [0.4.2] - 2025-12-27

### Added
- Resolve HTTP/HTTPS image URLs to base64 data URIs before WASM render

## [0.4.1] - 2025-12-20

### Fixed
- Expose `pkg/` in exports map for browser consumers

## [0.4.0] - 2025-12-13

### Added
- `resolveFonts()` for base64 font encoding before WASM calls

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- WASM bridge: `renderDocument()`, `renderWithLayout()` JS API
- `wasm-pack` build pipeline with `wasm-opt`

### Changed
- Package scope renamed from `@forme/core` to `@formepdf/core`
