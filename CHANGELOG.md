# Changelog

All notable changes to the Forme monorepo are documented in this file.

## [0.24.0] - 2026-09-17

Parallel flex-row fragmentation, and the results of a systematic sweep for
values the engine computed and never read.

### Changed — these move existing documents

Four fixes make declarations take effect that were previously discarded. If a
document relies on any of them, its layout will change, and that is the point.

- **`max-width` / `min-width` on a text block with no border, padding or
  background.** A leaf text node honoured `width` and ignored the min/max
  family, so `<p style="max-width: 315pt">` ran the full column. Eight of the
  thirty shipped template images move because of this alone: the Northmoor
  set's prose-measure classes (`.intro`, `.measure78`) were inert and now are
  not. Affects the JSX path equally, through `<Text style={{ maxWidth }}>`
- **`word-spacing` is part of text measurement.** It was applied only at
  PDF-write time through the `Tw` operator, so lines were broken as though it
  were zero and then drawn wider — text could run past its container. Lines
  now break where a browser breaks them
- **Print media queries evaluate against the page CONTENT box.** They briefly
  evaluated against the page box, on an unverified claim that a browser agrees.
  Measured, Chrome matches none of Bootstrap's `768`/`992`/`1200` breakpoints
  when printing A4 or Letter, and neither does Forme now. A Bootstrap
  `.container` that took a desktop width and overran the page no longer does
- **A stretched column paints its band on every page the row crosses.** Under
  `align-items: stretch`, a column whose content ended before its neighbour's
  painted nothing on later pages; a browser continues the band

### Added

- **Parallel flex-row fragmentation.** A `flex-direction: row` that crosses a
  page now continues as parallel columns on each page, instead of laying its
  children out sequentially. Wrapped rows (`flex-wrap: wrap`) keep the
  sequential behaviour and say so through a named render defect
- **`word-spacing` in the HTML CSS subset**, block-level, with `em`/`rem`
- **`<html lang>` reaches `/Lang`.** The attribute was parsed and never read,
  so a PDF/UA file could carry a language contradicting its own content while
  the warning told the author to set the attribute they already had. `lang`
  precedence is now uniform on every render: an explicit option, then the
  document's own declaration, then `"en"` with a warning
- **`em` and `rem` on `gap`, `border-radius` and `border-width`**, which parsed
  correctly and did nothing. `border: 0.5em solid` painted at the `medium`
  default rather than the width asked for

### Fixed

- **Render warnings reach every surface.** `renderTemplateWithLayout()`
  reported "warnings: none" on every render since it shipped, and `forme dev`
  served a complete warnings badge that nothing populated. Font-embedding
  warnings under `pdfUa`, missing glyphs, clamped table columns and sequential
  row splits were all silent on those paths
- **A border no longer cancels the rest of an element's style.** A paragraph
  with a border, padding or background lost roughly fifteen other properties,
  among them `border-style`, `text-transform`, `letter-spacing`, orphan and
  widow control, `position` and its offsets, and `vertical-align`
- **Per-side border colours** on the HTML path, which collapsed to one colour
- **Inline elements** keep their background, padding and border
- **`dir="rtl"` as an attribute** now reaches the engine's BiDi
- **`border-style` with no explicit width** paints, per CSS's `medium` initial
- **`border-collapse`** is honoured on `display: table` elements

### Internal

- The docs gallery freshness gate now verifies which engine build produced the
  images. It hashed the source and rendered with the binary without checking
  they corresponded, so a stale build satisfied it completely — 27 of 30 images
  were stale when that was finally measured
- Byte-wall fixtures for the box/text split, relative units, word-spacing and
  fragmented columns. Each was verified to fail before being trusted
- The SDK WASM rebuild is unconditional in `RELEASE.md`. Conditional on
  "if `engine/` changed", it was a judgement call whose failure is silent: an
  SDK keeps a stale WASM and its byte-parity test still passes, against a JS
  package that has moved

## [0.9.2] - 2026-04-28

### Fixed
- **Redaction precision**: Text-stripping now uses real per-CID glyph advances when locating regions, so partial-line redactions match the visible overlay precisely. Previously, redacting `Molitor` in `Dear Daniel Molitor` would also strip `Dear Daniel`
- **CID font handling**: Decode CID/Type0 fonts in the redaction text extractor, with parsing that survives binary font streams
- **Multi-style text grouping**: `text_decoration` is now part of the glyph style key, so a `line-through` span inside an otherwise plain text node is no longer merged with its neighbors during PDF emission

### Changed
- **Rasterizer body limit**: Default Axum 2 MB request body limit removed — large PDFs now flow through the rasterizer sidecar without 413 errors
- **MCP tool surfaces**: Synced with the current `@formepdf/react` component set so generated prompts reflect shipping components

## [0.9.1] - 2026-04-06

### Fixed
- **React Compiler compatibility**: `serialize()` now detects when a wrapper component has been compiled by React Compiler (which injects `useMemoCache` hooks that can't run outside React's render cycle) and throws a clear, actionable error pointing users to add `'use no memo'` to the component. Previously these failures surfaced as a cryptic "Invalid hook call" error

### Changed
- Bump rasterizer base image to 0.9.1

## [0.9.0] - 2026-04-04

### Added
- **PKCS#1 auto-conversion**: Digital signatures now accept both PKCS#8 and PKCS#1 (RSA) private key formats — PKCS#1 keys are automatically converted
- **Self-hosted server parity**: Field names, validation rules, error shapes, and query parameters aligned with the hosted API

### Changed
- **Certify field names**: `certificatePem` → `certificate`, `privateKeyPem` → `privateKey` (old names still accepted)
- **Self-hosted redact**: Validates preset names, enforces max 20 presets, validates regex patterns before execution
- **Self-hosted render**: Supports `?flattenForms=true` query parameter, returns `Content-Disposition` header on slug renders
- **Self-hosted errors**: Resource listing endpoints return `{ "error": "...", "code": "NOT_IMPLEMENTED" }` instead of plain error strings

### Removed
- `/v1/sign` endpoint — use `/v1/certify` instead

### Fixed
- WASI timestamp: Python SDK and Go SDK WASM builds now use `std::time::SystemTime` instead of browser-only `js_sys::Date`

## [0.8.3] - 2026-04-01

### Added
- SVG element opacity support: `opacity`, `fill-opacity`, and `stroke-opacity` attributes via PDF ExtGState with inheritance through `<g>` groups
- `<Svg>` children API: JSX children as alternative to `content` string prop, with camelCase→kebab-case attribute mapping

### Fixed
- `<Page style={{ fontFamily }}>` now correctly inherits to child nodes (was being discarded during serialization)

## [0.8.2] - 2026-03-30

### Fixed
- PDF serializer ignoring custom font weights — multiple weights for the same family (e.g. 200, 400, 700) now produce distinct font objects instead of collapsing to 400/700

## [0.8.1] - 2026-03-30

### Fixed
- Latin Extended character widths in standard font tables (Å, Ä, Ö, etc. no longer stack)
- Page number placeholder width mismatch during layout — two-pass rendering now measures actual digit count

## [0.8.0] - 2026-03-29

### Added
- **AcroForm components**: `<TextField>`, `<Checkbox>`, `<Dropdown>`, `<RadioButton>` for creating fillable PDF forms
- **Form flattening**: `flattenForms` render option converts interactive fields to static content
- **PDF/UA-1 accessibility**: `<Document pdfUa>` generates tagged PDFs with structure tree, tab order, role map, and artifact tagging
- **PDF/A archival**: `<Document pdfa="2b">` for long-term document preservation (supports `2b`, `2a`)
- **Digital signatures**: `<Document signature={{ certificatePem, privateKeyPem }}>` applies PKCS#7 detached signatures with X.509 certificates
- **`/v1/sign` API endpoint**: Sign existing PDFs via the hosted API
- **`/v1/render/:slug?flattenForms=true`**: Flatten form fields via query parameter on render endpoints
- New docs pages: Forms, Accessibility, Archival, Digital Signatures

### Fixed
- Checked checkboxes now render a checkmark instead of an X (interactive and flattened)
- Multi-byte DER length parsing in certificate CN extraction
- Form flattening renders placeholder text in grey when field value is empty
- Signing preserves existing AcroForm metadata (NeedAppearances, DA)
- Unique signature field names for double-signing (Signature1, Signature2, ...)
- Form fields tagged as /Form in structure tree for PDF/UA compliance

### Breaking
- Removed unused `page` field from `SignatureConfig`

## [0.7.13] - 2026-03-28

### Added
- **Engine-native chart components**: `<BarChart>`, `<LineChart>`, `<PieChart>`, `<AreaChart>`, and `<DotPlot>` are now rendered directly by the Rust engine as PDF vector primitives — no SVG intermediary
- **AreaChart**: New multi-series area chart with semi-transparent fill under each line
- **DotPlot**: New scatter plot for (x, y) data with multiple groups and axis labels
- **Multi-series LineChart**: `series` + `labels` props replace the old single-series `data` + `color` API
- **PieChart donut mode**: `donut` boolean prop replaces `innerRadius`; `showLegend` replaces `showLabels`
- Python SDK: `BarChart`, `LineChart`, `PieChart`, `AreaChart`, `DotPlot` classes
- `renderSerializedDoc()` and `renderSerializedDocWithLayout()` in `@formepdf/core/browser` for rendering pre-serialized documents
- `charts-showcase.tsx` template demonstrating all five chart types

### Changed
- Old SVG-based chart implementations preserved as `LegacyBarChart`, `LegacyLineChart`, `LegacyPieChart` for migration

## [0.7.12] - 2026-03-24

### Fixed
- **Edge runtime support in `@formepdf/core`**: Added `worker`, `edge-light`, `deno`, `react-native`, and `browser` conditional exports so `import { renderDocument } from '@formepdf/core'` automatically resolves to the browser entry point in Cloudflare Workers, Vercel Edge, Deno Deploy, Netlify Edge, Astro edge routes, React Native/Expo, and other non-Node runtimes ([#1](https://github.com/danmolitor/forme/issues/1))

## [0.7.11] - 2026-03-23

### Added
- `@formepdf/templates`: New shared package for built-in PDF templates and Zod schemas
- Templates use `<QrCode>` for shipping label tracking (replaces fake barcode rectangles)
- `@formepdf/templates/schemas` sub-export for Zod schemas with descriptions, fields, and examples
- Root `templates/letter.tsx` demo for `forme dev`

### Fixed
- **Cloudflare Workers crash**: `@formepdf/hono` and `@formepdf/next` now detect edge runtimes via `import.meta.url` instead of `process.versions.node`, fixing `TypeError: The "path" argument must be of type string` when `nodejs_compat` is enabled ([#1](https://github.com/danmolitor/forme/issues/1))

### Changed
- `@formepdf/hono`, `@formepdf/next`, `@formepdf/resend`, `@formepdf/mcp` now import templates from `@formepdf/templates` (single source of truth)

## [0.7.10] - 2026-03-18

### Added
- Auto margin support (`margin-left: auto`, `margin-right: auto`) for horizontal centering — enables `mx-auto` in Tailwind
- Engine: `EdgeValue` enum (`Pt` / `Auto`) and `MarginEdges` struct with auto detection and resolution
- Layout: auto margin resolution in flex row cross-axis and column cross-axis (priority over `align-items`)
- Integration tests for auto margin centering, push-right, and JSON deserialization
- `@formepdf/tailwind`: `mx-auto`, `my-auto`, `mt-auto`, `mr-auto`, `mb-auto`, `ml-auto` support
- Added `@formepdf/tailwind` to the version bump script

### Fixed
- `@formepdf/tailwind` `FormeStyle` type compatibility with `@formepdf/react` `Style` (narrowed `fontWeight`, added `oblique`/`wrap-reverse`, widened `minWidth`/`maxWidth` to accept strings)
- Python SDK `_expand_margin_edges()` preserves `"auto"` string values

## [0.7.9] - 2026-03-17

### Added
- `@formepdf/tailwind` package: style Forme components with Tailwind CSS utility classes (`tw("p-4 text-lg font-bold")`)
- Full Tailwind class coverage: spacing, typography, colors (all shades), layout, flexbox, grid, borders, opacity, negative values, arbitrary bracket values, `self-*` alignment

### Changed
- Rebuilt WASM binary with barcode and Python SDK (wasm-raw) support

## [0.7.8] - 2026-03-17

### Added
- `<Barcode>` component: 1D barcodes (Code 128, Code 39, EAN-13, EAN-8, Codabar) rendered as native PDF vector rectangles
- Python SDK: local rendering via wasmtime with component DSL (`Document`, `Page`, `View`, `Text`, `Image`, `Table`, `QrCode`, `Barcode`, etc.)
- Python SDK: `pip install formepdf[local]` optional dependency for self-hosted PDF generation

## [0.7.7] - 2026-03-16

### Added
- `@formepdf/core`: Browser entry point (`@formepdf/core/browser`) for client-side PDF generation — no Node.js required
- VS Code 0.7.8: Single preview panel that follows the active editor

## [0.7.6] - 2026-03-13

### Added
- Embedded data support: attach JSON to PDFs as file attachments via `renderDocument(el, { embedData })`
- `extractData(pdfBytes)` to read embedded JSON back from Forme-generated PDFs
- `@formepdf/mcp`: `extract_pdf` tool for round-trip data extraction
- `@formepdf/mcp`: `render_pdf` now auto-embeds template data

### Changed
- VS Code: two-way data sync between Data tab and companion JSON file

## [0.7.5] - 2026-03-12

### Removed
- `@formepdf/mcp`: Output path restriction — absolute paths now work

## [0.7.4] - 2026-03-11

### Added
- `@formepdf/mcp`: Theme customization for all templates (accent color, font family, margins)
- `@formepdf/mcp`: Logo/image support for invoice and letter templates
- `@formepdf/mcp`: Watermark parameter on `render_pdf` tool
- `@formepdf/mcp`: MCP prompts for guided PDF generation
- `@formepdf/mcp`: More components available in `render_custom_pdf` (Watermark, QrCode, charts, Canvas)

### Fixed
- `@formepdf/mcp`: Dynamic version from package.json (was hardcoded to 0.4.4)
- `@formepdf/mcp`: Code sandbox for custom JSX evaluation (security)
- `@formepdf/mcp`: Rendering timeout, improved error messages

## [0.7.1] - 2026-03-07

### Added
- Builtin Noto Sans font (Regular + Bold) for automatic non-Latin text support (Cyrillic, Greek, etc.)
- `<Document style>` prop for global default styles (fontFamily, fontSize, color, etc.)
- `Canvas` `line(x1, y1, x2, y2)` convenience method

### Changed
- Single-font text now automatically falls back to builtin Noto Sans when characters are missing
- Image component JSDoc updated with concrete path examples

## [0.7.0] - 2026-03-06

### Added
- `@formepdf/renderer` package for shared render pipeline (VS Code and future integrations)
- VS Code extension with native sidebar component tree, inspector panel, and hover-to-highlight
- VS Code extension activity bar icon and `forme.autoOpen` setting
- VS Code extension marketplace icon and improved discoverability

### Changed
- Shorter VS Code command titles ("Forme: Preview", "Forme: Preview to Side")

### Fixed
- CI: skip Arabic font fallback test when system font unavailable

## [0.6.2] - 2026-02-21

### Added
- Per-character font fallback for Arabic and CJK scripts
- `overflow: hidden` via PDF clip paths
- Canvas drawing primitive (`<Canvas>` component)
- Chart components: `<BarChart>`, `<LineChart>`, `<PieChart>`
- Watermarks with rotation and opacity
- SVG arc (`A`/`a`) path commands
- Justified text via PDF `Tw` operator
- PDF standard font `/Widths` arrays
- `lineBreaking` toggle
- Chart legend flex-wrap

### Fixed
- Cross-axis stretch propagation for flex layout
- Font weight fallback (opposite weight resolution)
- Shaping cluster byte-to-char conversion for multi-byte characters

## [0.6.1] - 2026-02-14

### Added
- Canvas clipping and arc counterclockwise parameter
- PDF bytes option for `sendPdf` in `@formepdf/resend`

## [0.6.0] - 2026-02-07

### Added
- `@formepdf/mcp` package for AI-powered PDF generation via MCP
- `@formepdf/resend` package for PDF + email via Resend
- `@formepdf/next` package for Next.js App Router route handlers
- `@formepdf/hono` package for Hono middleware (Workers, Deno, Bun, Node)
- CSS shorthands for border, padding, and margin (string and array formats)
- Alt text for images and SVGs
- Document language (`<Document lang="...">`)
- Clickable images and SVGs via `href` prop
- Knuth-Plass optimal line breaking
- UAX#14 Unicode line breaking
- Multi-language hyphenation via hypher (35+ languages)
- Tagged PDF / PDF/A-2a compliance
- Visual regression tests
- OpenType shaping via rustybuzz
- BiDi text support (unicode-bidi + unicode-script)
- CSS Grid layout (track sizing, auto/explicit placement)
- `repeat()` syntax for grid templates
- `textOverflow` (ellipsis/clip)
- Font fallback chains (comma-separated `fontFamily`)
- QR code generation with vector PDF rendering

## [0.4.4] - 2026-01-10

### Changed
- Version bump across packages

## [0.4.3] - 2026-01-03

### Fixed
- Keyboard shortcuts intercepting input in custom size fields
- Shipping label font and layout adjustments

## [0.4.2] - 2025-12-27

### Added
- Resolve HTTP/HTTPS image URLs to base64 data URIs before WASM render

## [0.4.1] - 2025-12-20

### Fixed
- Expose `pkg/` in `@formepdf/core` exports map for browser consumers

## [0.4.0] - 2025-12-13

### Added
- Template expression system for hosted API rendering
- Custom font registration API (`Font.register()` + `<Document fonts>` prop)

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- Page-native PDF rendering engine with real font metrics
- TrueType font embedding with CIDFont objects and subsetting
- `@formepdf/react` JSX-to-JSON serializer package
- `@formepdf/core` WASM build of the Rust engine
- `@formepdf/cli` with `forme dev` live preview and `forme build`
- Click-to-inspect dev tools with source jumping
- Component tree, data editor, and page size switcher
- Widow/orphan control, `align-content`, table cell overflow
- Bookmarks, internal anchor links, letter-spacing
- Absolute positioning, SVG module
- Style shorthand properties
- Background/border preservation on breakable views across page splits
- Nested flex layout, Fragment serialization, footer positioning, dynamic page numbers
