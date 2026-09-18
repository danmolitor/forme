# Changelog

All notable changes to the Forme monorepo are documented in this file.

Entries for 0.10.0 through 0.23.0 were backfilled on 2026-09-17 from the
published GitHub release notes, which were the record of those releases
while this file lapsed. They are reproduced, not rewritten.

## [0.24.0] - 2026-09-17

Parallel flex-row fragmentation, and the results of a systematic sweep for
values the engine computed and never read.

### Changed: these move existing documents

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
  were zero and then drawn wider, so text could run past its container. Lines
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
  they corresponded, so a stale build satisfied it completely. 27 of 30 images
  were stale when that was finally measured
- Byte-wall fixtures for the box/text split, relative units, word-spacing and
  fragmented columns. Each was verified to fail before being trusted
- The SDK WASM rebuild is unconditional in `RELEASE.md`. Conditional on
  "if `engine/` changed", it was a judgement call whose failure is silent: an
  SDK keeps a stale WASM and its byte-parity test still passes, against a JS
  package that has moved

## [0.23.0] - 2026-09-11

A minor with a new Python rendering path, a CSS property that had been missing since the HTML path shipped, and two fixes on documented happy paths. All packages share the line: engine (crates.io `forme-pdf`), every `@formepdf/*` npm package, `formepdf` (PyPI), `forme-go` (tag v0.23.0), and the VS Code extension.

### Added

- **Local HTML → PDF from Python, no system libraries.** `render_html(html, ...)` renders in-process through the engine compiled to `wasm32-wasip1` and run via `wasmtime` — no cairo, no pango, no browser. `pip install formepdf[local]` pulls one dependency. The output is **byte-identical to `@formepdf/html`**, because it is the same Rust engine rather than a re-implementation, and a CI job renders the same input through both paths and requires identical bytes on every commit.
- **A shared option parser for both WASM front-ends.** The camelCase render options (`pageSize`, `pageMargin`, `css`, `fonts`, `tagged`, `pdfUa`, `pdfUa2`, `lang`, `pdfA`, `auditContent`) are now parsed by one crate module used by both the wasm-bindgen (JS) build and the wasm32-wasip1 C-ABI build the Python and Go SDKs load — the two agree by construction, which is what makes the byte-identity above hold.
- **`box-sizing` joins the CSS subset.** CSS dimensions are content-box by default while the engine's fixed dimension is always the border box, so a padded, bordered `height: 615pt` frame rendered 69pt short — everywhere, since the HTML path shipped. Point-valued `width`/`height`/`min-*`/`max-*` now grow by padding + border under content-box; a declared `border-box` (the Bootstrap reset) passes through untouched. Percentage dimensions are unchanged on both settings.
- **A worked e-invoicing guide**: [rendering with Forme and converting through a PDP or e-invoicing service](https://docs.formepdf.com/guides/e-invoicing-with-a-pdp), with the EN 16931 mapping table and a runnable example. The round trip is verified end to end — the returned Factur-X passes veraPDF (PDF/A-3b) and Mustangproject (EN 16931 schematron).

### Fixed

- **`pdfa` alone now embeds the fonts-standard substitutes.** The Liberation substitution for base-14 families was gated on `pdfUa` (or PDF 2.0), while PDF/A's own embedded-fonts check counts a base-14 family as embedded *only* via that substitution — so `pdfa` without `pdfUa` failed with "'Helvetica' is not embedded" even with `@formepdf/fonts-standard` registered. If you render PDF/A without also claiming PDF/UA, this is the fix you want.
- **`align-items: center` respects `max-width`.** The centering offset used the unclamped intrinsic width, so a `max-width`'d block inside a centered column sat flush left while its lines wrapped at the clamp.

### Behavior changes

- **Content-box documents get bigger boxes, matching Chrome.** Any document pairing fixed point dimensions with padding or border is affected: fixed-width table columns now include their padding (a squeezed flexible column may wrap differently), bordered checkboxes render at their true size, and framed layouts reach the margins they were designed to reach. Documents that declare `box-sizing: border-box` (the common reset) render byte-identically to before.

Per-package detail in each package's `CHANGELOG.md`.

## [0.22.0] - 2026-09-10

A minor with major conformance surface and called-out behavior changes. All packages share the line: engine (crates.io `forme-pdf`), every `@formepdf/*` npm package (`@formepdf/preview` joins the line this release), `formepdf` (PyPI), `forme-go` (tag v0.22.0), and the VS Code extension.

### Added

- **PDF 2.0 output** — `pdfVersion: "2.0"` writes ISO 32000-2: `%PDF-2.0` header, XMP metadata always, no trailer `/Info`, and **every font must be embedded** (32000-2 removes the standard-14 provision; base-14 output errors by name with the fonts-standard remedy). The default `"1.7"` path is byte-identical to 0.21.0, asserted by pin.
- **PDF/A-4 and PDF/A-4f** (ISO 19005-4:2020) — `pdfa: "4" | "4f"`, implying PDF 2.0. A-4f requires at least one embedded file (an empty claim is itself non-conformant and is refused); base A-4 refuses attachments. Note: **PDF/A-4 is not an accessibility claim** — the a/b/u split is gone; accessibility is PDF/UA-2 territory.
- **PDF/UA-2** (ISO 14289-2:2024) — `pdfUa2`, the PDF 2.0 accessibility claim: single-`Document` structure tree in the 2.0 namespace, ISO 32005 containment (grouping elements' own ink is tagged artifact), graphics as `/Figure` with a barcode/QR's encoded data as `/ActualText` when no alt is given, `ListNumbering`, structure destinations for outlines and internal links. Composes with `pdfa: "4"`/`"4f"` for archival + accessible on PDF 2.0; contradictory claims are refused by name. **All of the above is veraPDF-gated in CI over the standing corpus** — every level of PDF/A (2b/2a/3b/3a/4/4f), PDF/UA-1 on the 1.7 levels, PDF/UA-2 composed on the 2.0 levels.
- **`align-items: baseline`** in flex rows — previously parsed and treated as `flex-start`.
- **Claim-parity guards** — the document-level conformance claims (`tagged`, `pdfa`, `pdfVersion`, `pdfUa`, `pdfUa2`) are asserted at compile time across all five authoring adapters (react, preact, svelte, vue, and the shared markup parser), so a claim can no longer reach serialization without reaching the prop types users compile against.

### Behavior changes

- **Baselines use real font metrics.** Text ink sits where a browser puts it: ascent/descent from the font's own metrics with half-leading, replacing font-size-as-ascent. Every baseline moves by a small per-font delta (~0.15em for Arial-class metrics); line boxes, element geometry, page breaks, and page counts are unchanged. Single glyphs centered by the line-height idiom now center exactly.
- **`letter-spacing` and `word-spacing` now inherit**, as CSS specifies. Tracking declared on a container finally reaches its children.
- **`flex: <n>` is spec-correct** — it now sets `flex-basis: 0` (CSS: `flex: 1` = `1 1 0`), not `basis: auto`. A `flex: 1` text column no longer brings its full unwrapped width to distribution and crushes fixed-width siblings.
- **Intrinsic widths are honest** — explicit fixed widths and flex gaps now count in intrinsic measurement (an empty `width: 33` view measured 0 before). Auto-sized layouts whose contents carry fixed-width boxes or gaps get their true size.
- **AcroForm dictionaries no longer set `/NeedAppearances`** — Forme authors every widget appearance stream itself, so field rendering is now identical across viewers instead of viewer-dependent.

Per-package detail in each package's `CHANGELOG.md`.

## [0.21.0] - 2026-09-09

A minor with new surface and called-out behavior changes. All packages share the line: engine (crates.io `forme-pdf`), every `@formepdf/*` npm package, `formepdf` (PyPI), `forme-go` (tag v0.21.0), and the VS Code extension.

### Added

- **The thirty-template library**, surfaced: a [generated gallery at docs.formepdf.com/templates](https://docs.formepdf.com/templates) — invoices, contracts, reports, labels, built as one system, MIT-licensed, every one rendering warning-free and passing PDF/UA-1 validation (veraPDF) in CI. The gallery and every detail page are emitted from the template sources and freshness-gated in CI, so they cannot drift.
- **Flex containers item-ize their inline children** — `<div style="display:flex; justify-content:space-between"><span>Label</span><span>$1,234</span></div>` now renders as two spread flex items per CSS, instead of fusing into one merged line with the line box sized by whichever font came first.
- **`flex-grow`, `flex-shrink`, and single-number `flex: <n>`** join the HTML CSS subset.
- **Missing-glyph render defect** — a character with no glyph source anywhere (not WinAnsi, not the bundled Noto, not a registered font) used to print `?` silently, at three sites (body text, form fields, chart labels). It now reports one warning per distinct character, naming it and the remedy (register a font containing it). Its first run caught two shipped templates printing `?` for `≤`.
- **`auditContent` through `@formepdf/core`** — the HTML path's opt-in post-render content audit, now reachable from JSX callers via the WithLayout variants. Findings arrive as `render defect:` warnings; off by default, byte-identical when off.

### Behavior changes

- **Absolutely positioned children anchor to the FIRST fragment of a split containing block** (CSS conformance; previously the last). Blast radius surveyed before landing: zero movement across the fixture wall, the 15-template compat corpus, and all thirty repo templates.
- **Multi-style text runs take the same per-character font fallback as single-style text** — non-WinAnsi characters in a `TextRun` now reach the builtin Noto Sans where it covers them, instead of printing `?`.
- **The sequential-split render defect fires on the outcome, not the path — and names the row.** A row relocating whole to the next page no longer warns; a column genuinely serializing across pages still does, with the offending row named. Expect strictly fewer of these warnings, with changed message text.
- **Named-page documents no longer emit trailing blank pages**; an empty document still produces zero pages.
- **Margin-box borders, backgrounds, and padding render seamlessly** (hoisted to the band cell views).

Per-package detail in each package's `CHANGELOG.md`.

## [0.20.1] - 2026-09-07

The types tell the truth: passes on every target

Patch release: the declared types now tell the truth on every target.

### Fixed: `passes` is returned everywhere the type declares it

`RenderHtmlResult.passes` (the layout-pass count) was declared in `@formepdf/html`'s types but returned by only one of the six result constructions — node `renderHtml`. The browser and worker entries, and `renderHtmlWithLayout` on **all three** targets, returned `undefined` while type-checking fine. Found building a Cloudflare Workers template against the published package.

The hole went three layers down: three hand-written per-target result objects (one updated when `passes` was added), a WASM binding that never carried the field on the layout path, and the engine computing the value in `render_with_layout` and discarding it. All three layers are fixed, and the structure that allowed the drift is gone:

- **One shared construction** (`result.js`) now builds every result object for all three entries — a new field is added in exactly one place.
- **Declared type ↔ runtime shape is enforced in both directions**: the expected key sets are derived from `keyof RenderHtmlResult` (a type change that the runtime doesn't match fails `npm test`'s typecheck), and per-target tests assert the exact key set in the real runtime — plain Node, real workerd via `@cloudflare/vitest-pool-workers`, and real headless Chromium.
- Fails-first was demonstrated against the registry: the published 0.20.0 layout path fails the new shape test.

### Engine

- New `render_with_layout_and_passes()` — `render_with_layout()` unchanged (now a thin wrapper).

Output is byte-identical to 0.20.0 across the fixture wall, the cross-target determinism gate, and the 15-template compat corpus (zero warnings-baseline diff). All packages in lockstep at 0.20.1: npm (`@formepdf/*`), crates.io (`forme-pdf`), PyPI (`formepdf`), Go (`v0.20.1`).

## [0.20.0] - 2026-09-05

The corpus campaign closes: 14 of 15, attribute selectors, CSS tables, half-leading

Closing the corpus campaign: the 15 real production templates from [the compat experiment](https://formepdf.com/blog/real-template-compat) now measure **14 of 15 rendering correctly, 1 degrading legibly with its cause named in warnings, 0 broken** (11/4/0 at 0.19.0; 3/8/4 when the experiment started). This release also lands the vertical-centering work, with one global typographic change every user should read first.

### ⚠️ Behavior change: half-leading — every text baseline moves down

Glyph baselines previously sat exactly `font-size` below the line-box top; the leading now splits evenly above and below the glyph block, per the CSS line box model (and every browser). Every baseline moves down by `(line-height − font-size) / 2` — +2.4pt at the default 1.4 ratio on 12pt text. **Line boxes do not move or resize**: layout geometry, page breaks, and page counts are unchanged — only the ink inside each line box shifts, closer to where Chrome puts it. This is what makes the pre-flexbox centering idiom (`line-height` matched to a box height) actually center. Fixed-height flex boxes with `align-items: center` also now truly center (CSS 9.4.8 — the flex line honors the container's definite cross size).

### ⚠️ Behavior change: `position: fixed` renders as `position: absolute`

A paged renderer's viewport is the page, so `fixed` anchors to its containing block on the page where it occurs — **not repeated on every page** (margin boxes remain the running-content mechanism; a warning names the difference). The wkhtmltopdf print-footer idiom `#footer { position: fixed; bottom: 0 }` now sits flush at the page bottom.

### ⚠️ Behavior change: `@media` width measures the page box

Media Queries Level 4 defines `width` in paged media as the width of the **page box** — A4 = 794 CSS px, so `(min-width: 768px)` is true on A4, matching Chrome print. Earlier versions measured the content box (a documented spec misreading, now corrected in the README). Only queries with thresholds between your content-box and page-box widths change outcome.

### HTML input path

- **Attribute selectors** — all seven operators (`[attr]`, `=`, `~=`, `|=`, `^=`, `$=`, `*=`) plus the `i` case flag, with spec specificity. `[class*="span"]` is Bootstrap 2's entire grid, so BS2 templates get their columns back.
- **CSS tables on divs** — `display: table` / `table-row` / `table-cell`, the pre-flexbox equal-height-columns idiom. Multi-row CSS tables use the native table machinery; a single-row one (the columns idiom) becomes a breakable flex row. Known boundary, reported via the render-defect channel: a flex row that splits across pages lays its children sequentially, not as parallel columns.
- **`body { overflow-x: hidden }` is a page-level clip** — the print equivalent of a browser suppressing horizontal overflow; off-viewport-parked furniture (`right: -230px` admin-shell sidebars) disappears instead of smearing into the margin. Engine-side: `PageConfig.clipContentX`, available to JSON callers too.
- **`rowspan` occupancy** — cells after a rowspan land in their correct columns (`table_column_offsets`, one pure function shared by layout, measurement, and column counting).
- `position: running()` suppression, Bootstrap print-stylesheet display values no longer produce warning noise, and the wkhtmltopdf migration guide documents the fixed-footer semantics.

### Engine

- Visual regression suite is now a **live CI gate** (Linux-canonical references, 1% threshold; a missing rasterizer fails the job rather than passing vacuously).
- New render defects: sequential flex-row splits and over-tall atomic table rows report themselves.

Full details in `engine/CHANGELOG.md` and `packages/html/CHANGELOG.md`. All packages in lockstep at 0.20.0: npm (`@formepdf/*`), crates.io (`forme-pdf`), PyPI (`formepdf`), Go (`v0.20.0`).

## [0.19.0] - 2026-09-05

Floats, the measure/layout agreement gate, and the real-template campaign

The corpus-campaign release: we rendered 15 real production invoice templates from GitHub through the engine and fixed what they exposed — [the full story](https://formepdf.com/blog/real-template-compat). Measured result: **11 of 15 render correctly, 4 degrade legibly with every cause named in warnings, 0 broken** (from 3/8/4 at the start).

### ⚠️ Behavior change: absolute offsets anchor the margin edge

`bottom`/`right` offsets on `position: absolute` now position the **margin edge**, per CSS. Previously they anchored the border box, so any absolutely-positioned element combining a `bottom`/`right` offset with margins rendered past its anchor by the margin size. If a positioned element sits closer to the top-left after upgrading, that's the spec-correct position. (Found as a receipt footer rendering below the page bottom with only ascender tips visible — "Thk ft".)

### HTML input path

- **Float support (document subset)** — `float: left/right` + `clear` as column rows: consecutive floated siblings form a row (the Bootstrap `col-*` / left-right header-pair shape). Text wrapping *alongside* a float remains out and warns.
- **Images size like Chrome** — percent widths and `max-width` honored, height from the real aspect ratio, one shared sizing ladder for measurement and layout.
- **Table sections** — `<tfoot>` renders at the bottom regardless of DOM position; only the first `<thead>` repeats as the header; `display: none` now applies to rows and sections.
- `body { width: 21cm }` page-sizing idiom clamps with a named warning; giant single-`<tr>` email layouts no longer produce blank pages; `position: running()` elements leave the flow per spec instead of rendering as stray text.

### Engine: the measure/layout agreement gate

Six shipped bugs shared one shape — intrinsic measurement computing a height layout never produced (phantom gaps, mis-centered running headers, crushed tables). The invariant is now enforced: `FORME_MEASURE_CHECK=1` flags any auto-height container whose measured height exceeds what its children occupy, and a fixture-corpus test gate fails on any emission. The gate found two of the six on its first run. All six are fixed, each with a minimal failing-first test.

Full details in `engine/CHANGELOG.md` and `packages/*/CHANGELOG.md`. All packages in lockstep at 0.19.0: npm (`@formepdf/*`), crates.io (`forme-pdf`), PyPI (`formepdf`), Go (`v0.19.0`).

## [0.18.0] - 2026-09-04

PDF/A-3, e-invoice containers (Factur-X/ZUGFeRD), automatic table layout

### PDF/A-3 + attachments + e-invoice containers

- **PDF/A-3 (`3b`/`3u`/`3a`)** — the PDF/A-2 rule set plus permission for arbitrary embedded files, veraPDF-gated in CI alongside 2b/2a.
- **Attachments (associated files)** — `Document.attachments` embeds files with the PDF/A-3 requirements built in: MIME `/Subtype`, `/F` + `/UF` + `/AFRelationship`, `/Params`, catalog `/AF` membership, and a deterministic default `/ModDate` (byte determinism holds).
- **Factur-X / ZUGFeRD e-invoice containers** — `Document.zugferd` emits the `fx:` XMP identification with the required PDF/A extension schema; `/AFRelationship` defaults per profile. Container only: you supply the EN 16931 XML, Forme never generates or validates invoice semantics. Gated in CI by veraPDF and Mustangproject.

### ⚠️ Behavior change: PDF/A-2 + attachments now errors by name

`embedData`/attachments combined with `pdfa: "2b"` previously emitted a **non-conformant** file silently — ISO 19005-2 §6.8 permits only PDF/A attachments, which the engine cannot verify. It now errors and names the fix (`pdfa: "3b"`). Files previously produced with that combination were never valid PDF/A-2.

### Automatic table layout (HTML input path)

Tables opening with a full-width colspan banner row — the most common invoice shape in existence — previously collapsed to one column with per-character vertical text, silently. Columns are now counted from the widest row's colspan sum and sized by min/max content like a browser; over-specified widths shrink gracefully instead of shredding. Found by rendering 15 real production invoice templates: all four broken renders shared this one bug.

### The render-defect channel + warning dedup

- New warning class prefixed `render defect:` answering "what did **we** get wrong?" (vs. the subset contract's "what did you ask for that we don't support?") — first entries cover clamped and below-min-content table columns.
- Identical warnings collapse to one entry with a count (`(×214)`); framework stylesheets previously produced thousands of identical lines.

Full details in `engine/CHANGELOG.md` and `packages/*/CHANGELOG.md`. All packages published in lockstep at 0.18.0: npm (`@formepdf/*`), crates.io (`forme-pdf`), PyPI (`formepdf`), Go (`github.com/formepdf/forme-go@v0.18.0`).

## [0.17.0] - 2026-09-03

Paged media page selection, CSS Grid in HTML, page-number font fix

One shared version line: `forme-pdf` (crates.io), every `@formepdf/*` npm package, `formepdf` (PyPI), `forme-go` (tag), and the VS Code extension all ship as 0.17.0.

### CSS Paged Media page selection (HTML path + engine)

- **`@page :left` / `:right`** — per-parity margins and margin boxes. Mirrored margins (equal left+right sum) apply as an exact translation — never a re-layout, so no output drift and no perf cost. Page 1 is a `:right` page (LTR page progression per spec); `dir="rtl"` warns rather than silently assuming.
- **Named pages** — `@page <name>` + the `page: <name>` property. A named run starts at a forced break, so real vertical margins work (the classic zero-margin cover). Named margin boxes override or suppress the base running headers per name. Composes with `:first`/`:left`/`:right` (`@page cover:first`); precedence follows spec specificity.
- Out-of-subset Paged Media features warn by name: `:blank`, `:nth()` page groups, `string-set`/`content: string()`, `position: running()`, footnotes.

### CSS Grid through the HTML path

`display: grid` now maps in `@formepdf/html` — the documented subset: template columns/rows (`px`/`pt`/`em`/`fr`/`auto`/`minmax()`/integer `repeat()`), gaps, auto tracks, and explicit placement. Everything outside warns by name.

### Fixed

- **Page-number placeholders no longer switch fonts.** `{{pageNumber}}`/`{{totalPages}}` (and HTML `counter(page)`) in a custom/provided font used to render the digits in non-embedded base-14 Helvetica — splitting one footer line across two typefaces and hard-failing PDF/A. The digits now stay in the surrounding font, are properly subset, and keep their ToUnicode mappings. Default-font documents are byte-identical. (Thanks for the excellent report.)

### Performance

- ~2× fewer allocations in large-document layout (shared glyph font families, elided dead table-cell rollback checkpoints); the sentinel re-layout pass is skipped when no page-number placeholder exists.
- A fixed, deterministic benchmark corpus now feeds the measured numbers on [parity.formepdf.com](https://parity.formepdf.com).

### Housekeeping

- `npm audit`: 18 → 0 across the monorepo (all dev/build-side; test toolchain moved to vitest 4 + current workers pool).

Full per-package detail: `engine/CHANGELOG.md`, `packages/html/CHANGELOG.md`, `packages/core/CHANGELOG.md`.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

## [0.16.0] - 2026-09-01

PDF/A conformance

Forme now produces **PDF/A-2 conforming** archival documents — levels **2b, 2u,
and 2a** — verified by veraPDF in CI, and composable with PDF/UA-1 so a single
file can be **both archival and accessible** (the configuration
government/education/healthcare archives actually require).

### ⚠️ Correction — PDF/A output was never conformant before this release

If you used `pdfa: "2b"` (or `"2a"`) in 0.15.0 or earlier, **those files are not
valid PDF/A** — they carry an OutputIntent whose ICC profile was invalid, so no
conformant PDF/A reader would have accepted them as archival.

The embedded sRGB profile (`srgb2014.icc`) was never an ICC profile. A `curl`
that fetched it around 0.9.0 returned a Cloudflare "Just a moment…" HTML
challenge page instead of the binary; it was embedded unchecked and shipped in
every build since. Nothing caught it because nothing validated the output. It's
now replaced with a real, generated sRGB profile and validated in CI — **and a
test asserts the embedded bytes are a valid ICC profile so this can't recur.**

**How to check:** any file produced with `pdfa` before 0.16.0 will fail veraPDF's
PDF/A profile — `verapdf -f 2b your-file.pdf` reports `FAIL`. Re-render those
files with 0.16.0 (same input, same options) and they'll pass.

### PDF/A-2b / 2u / 2a — verified

The same nine-document corpus the PDF/UA gate uses (five shipped templates + four
HTML fixtures) passes veraPDF's **PDF/A-2b and PDF/A-2a** profiles. Getting there:

- **A real sRGB OutputIntent** — a generated, license-clean sRGB profile (~588
  bytes, ICC v4.3, Little CMS / MIT) embedded in the engine, replacing the
  invalid one above.
- **PDF/A XMP metadata** — pdfaid part/level, plus the pdfaExtension description
  for the PDF/UA identification schema so PDF/A and PDF/UA compose.
- **A deterministic trailer `/ID`** — derived from a content hash, not a
  timestamp or random bytes. Identical input still produces byte-identical
  output, so Forme's determinism guarantee (native == WASM, reproducible builds)
  holds in PDF/A mode too — a property archival pipelines care about.
- **`/F` (Print) on link annotations**, and the font-embedding check now accepts
  the pdfUa metric-compatible substitution — so `pdfA + pdfUa` no longer throws.
- **No width-consistency issue**: the AFM-widths-by-construction font design
  (with its carve-out) satisfies PDF/A's width check — geometry stays identical
  across standard and archival output.

### Archival *and* accessible

PDF/A composes with PDF/UA-1. Set both:

```tsx
import { standardFonts } from '@formepdf/fonts-standard';

<Document pdfa="2a" pdfUa lang="en-US" fonts={standardFonts()}>…</Document>
```

Every corpus file is CI-gated against **PDF/A-2b, PDF/A-2a, and PDF/UA-1
together** (`scripts/verify-pdfa.mjs`) — the claim is enforced, not observed.

### API

- JSX: `<Document pdfa="2b" | "2u" | "2a">` (new: `"2u"`).
- HTML: `pdfA` in `renderHtml` options; `--pdf-a <level>` on the `forme-html` CLI.
- Needs an embeddable font (`@formepdf/fonts-standard`). If none is registered, a
  PDF/A render **fails by name with the remedy** rather than emitting a file that
  falsely claims conformance.

### Full changelog

[`engine`](https://github.com/danmolitor/forme/blob/v0.16.0/engine/CHANGELOG.md) ·
[`@formepdf/html`](https://github.com/danmolitor/forme/blob/v0.16.0/packages/html/CHANGELOG.md) ·
[`@formepdf/react`](https://github.com/danmolitor/forme/blob/v0.16.0/packages/react/CHANGELOG.md)

## [0.15.0] - 2026-09-01

PDF/UA-1 conformance + @formepdf/html in the browser & on the edge

The accessibility-and-reach release. Forme now produces **PDF/UA-1 conforming**
documents (verified end-to-end by veraPDF in CI) and **tags every render by
default**; `@formepdf/html` gains real **browser and Cloudflare Workers**
support; `@formepdf/vue` joins the adapter family with a cross-framework
equivalence guarantee; and a batch of engine fixes lands.

### ⚠️ Migration — `position: absolute` resolves against the nearest positioned ancestor

This retires a v0 divergence and matches every browser. It changes shipped JSX
output, which is why it's a minor bump rather than a patch.

**The rule.** An absolute element's containing block is now its nearest ancestor
with `position: relative` or `absolute`. If no ancestor is positioned, it
resolves against the page content box. Previously it always resolved against its
direct parent, positioned or not.

**Detection recipe.** If you use `position: 'absolute'` inside a container that
has **no** `position` set, add `position: 'relative'` to that container.
Otherwise the absolute box now resolves against the page (or a higher positioned
ancestor) — most visibly with negative offsets, which will push content off the
page.

**Worked example (from this repo).** The `catalog` template's "SALE"/"NEW" badge
is `position: 'absolute'; top: -18; right: -18` — negative offsets meant to
overhang a product card's corner. The card had no `position`, so under the old
rule the badge sat on the card; under the new rule it escaped to the page and
overflowed by 16pt. Our own structural-regression gate caught it before it
shipped. The one-line fix *is* the migration:

```diff
- <View style={{ /* card */ }}>
+ <View style={{ position: 'relative', /* card */ }}>
```

**How to find affected templates.** Grep for `position: 'absolute'` and check
whether each one's parent sets `position`. If you use `@pdf-testkit` structural
snapshots, your baselines will flag it exactly as ours did.

### ⚠️ Default change — tagged PDFs by default

`Document.tagged` now defaults to **true**: every render emits a structure tree
unless you explicitly set `tagged: false`.

**PDF output bytes change for every render; geometry and visual output do not;
set `tagged: false` to restore prior bytes.**

The tag tree is built after layout, so this is structurally additive — no
element moves. But byte-diffing consumers and anyone holding a PDF snapshot
baseline (pdf-testkit and friends) will see every output shift as the structure
objects appear. Expected; re-baseline once.

### PDF/UA-1 — verified, 9 of 9

A nine-document corpus passes [veraPDF](https://verapdf.org) 1.30.2 against the
PDF/UA-1 profile: the five shipped `@formepdf/templates` (invoice, receipt,
report, shipping-label, letter) and four HTML fixtures (letterhead,
dashed-borders, statement, zebra-invoice). It's a CI gate
(`scripts/verify-pdfua.mjs` + a `pdfua-conformance` job that installs veraPDF and
validates the whole corpus), so the number can't quietly regress.

Getting there taught the tagged-PDF writer the structure real documents use:
heading levels, lists wrapped in `/LBody`, table header `/Scope` and cell
`/ColSpan`, links as `/Link` structure elements bound to their annotations via
`OBJR` + `/StructParent`, figure `/Alt`, an emptied `/RoleMap` (a standard type
self-mapping is the circular RoleMap veraPDF rejects), and `/Lang`.

- **`pdfUa` mode** turns it on for JSX: `<Document pdfUa lang="en-US" fonts={standardFonts()}>`.
- **HTML path**: `--tagged` / `--pdf-ua` on the CLI, `tagged` / `pdfUa` / `lang`
  in `renderHtml` options, and `<img alt>` maps to `/Alt`.
- **`@formepdf/fonts-standard`** (new, optional) ships the Liberation
  Sans/Serif/Mono families (SIL OFL) as metric-compatible substitutes for the
  base-14 Helvetica/Times/Courier. PDF/UA requires embedded fonts; the base-14
  set isn't. Registering `standardFonts()` embeds a simple TrueType **at
  write-time only** — the substitution is in the font dictionary, AFM `/Widths`
  are kept, so layout geometry is byte-identical by construction. A per-glyph
  width carve-out covers the ~6 glyphs per family where Liberation diverges from
  the AFM metrics (PDF/A width-consistency). It's a separate package on purpose:
  **core carries no font payload — users who don't need conformance don't inherit
  the 5.8 MB.**
- **Warnings channel.** Renders now return `warnings: string[]` alongside `pdf`
  and `layout` (`renderPdfWithLayout`, the browser/worker entries, the HTML
  wrapper). If `pdfUa` is requested but no embeddable font is registered, the
  render still succeeds and *names* the gap instead of emitting a PDF that
  falsely claims conformance. This previously was a native-only `eprintln` that
  vanished under WASM — the silent-fail the font design exists to prevent now
  reaches WASM callers.

### `@formepdf/html` — now runs in the browser and on the edge

The 0.14.0 debut shipped only a Node build. This release adds the two targets
that make the headline claim real, mirroring `@formepdf/core`:

- **Browser bundlers (Vite, webpack, esbuild, Turbopack):** import from
  **`@formepdf/html/browser`**. The bundler instantiates the WASM at load — no
  init step.
- **Cloudflare Workers / edge:** import from **`@formepdf/html/worker`** and call
  **`await init(wasm)`** once at request time with the `WebAssembly.Module` you
  import from `@formepdf/html/pkg-web/forme_pdf_html_bg.wasm`. (Workers can't use
  the bundler build — its top-level WASM init conflicts with Wrangler's
  WASM-as-ESM contract; the worker entry with explicit `init` is the supported
  edge path, exactly as `@formepdf/core` does it.)
- **Node / npx:** the default import, unchanged.

All three entries expose the identical `renderHtml` / `renderHtmlWithLayout` API
(including the `warnings` array) and are byte-for-byte deterministic — the three
targets embed the same WASM, verified in CI against the fixture corpus, plus a
headless-Chromium render and a workerd render. Each target's WASM is ~7.45 MB
uncompressed (it gzips down substantially over the wire — same class as
`@formepdf/core`); a Workers user watching bundle limits should size for it.

Also in the HTML path:

- **Local `<link rel="stylesheet">` resolution** in the CLI — stylesheets are
  read and inlined in source order (the library itself never fetches).
- **`@media` feature queries** (`width`/`height`) resolve against the page
  content box.
- **`:nth-last-child` / `:nth-last-of-type`.**
- **`position: relative`** offsets — paint-only, flow preserved.
- **`vertical-align: baseline`** in table cells.
- **Dashed and dotted border styles.**
- **`float` / `clear`** are unsupported but now warn with a remedy rather than
  silently mislaying content.

### New & changed packages

- **`@formepdf/vue` (new)** — Vue 3 SFCs → Forme documents, with a
  cross-framework equivalence gate: the same document authored in Vue and in
  React serializes to deep-equal Forme JSON. React/Svelte/Vue/Preact all agree.
- **`@formepdf/shared` gains public surface** — the HTML `parser` and `encode`
  layers are hoisted here so the framework adapters (Svelte, Vue, Preact) share
  one implementation instead of each carrying a copy. This makes `shared` the
  root of the publish order: **shared → core → html → svelte → vue → preact →
  react → renderer → templates → cli → fonts-standard → rest.**

### Engine fixes (user-visible)

- **No more leading blank page** when the first element carried a `break-before`.
- **Empty styled `<p>`** (padding/background, no text) is no longer dropped — it
  paints.
- **Table cell CSS `height` is a *minimum*** row height, not a cap; taller
  content grows the row.
- **Wrapping headings auto-size correctly** — a heading that wraps to multiple
  lines no longer measures as zero height (it was missing from the height
  measurement, collapsing tagged headings).
- **Circular `/Div` RoleMap fixed** — the identity mapping invalidated the whole
  tagged tree under validators; the RoleMap is now empty (every role Forme emits
  is already a standard PDF type).
- **`/Link`, `/LBody`, `/ColSpan` structure tagging** — links, list-item bodies,
  and spanning table cells now carry correct structure (see PDF/UA above).

### Extension

- **Live preview for Svelte, Vue, and Preact templates**, alongside React and
  HTML.
- Preact templates were detected but failed at render; **they now render.**

### Full changelog

[`engine`](https://github.com/danmolitor/forme/blob/v0.15.0/engine/CHANGELOG.md) ·
[`@formepdf/html`](https://github.com/danmolitor/forme/blob/v0.15.0/packages/html/CHANGELOG.md) ·
[`@formepdf/core`](https://github.com/danmolitor/forme/blob/v0.15.0/packages/core/CHANGELOG.md) ·
[`@formepdf/vue`](https://github.com/danmolitor/forme/blob/v0.15.0/packages/vue/CHANGELOG.md) ·
[`@formepdf/fonts-standard`](https://github.com/danmolitor/forme/blob/v0.15.0/packages/fonts-standard/CHANGELOG.md) ·
[`@formepdf/shared`](https://github.com/danmolitor/forme/blob/v0.15.0/packages/shared/CHANGELOG.md)

All other `@formepdf/*` packages, the `forme-pdf` crate, and the Python/Go SDKs
get version-alignment bumps to 0.15.0.

## [0.14.0] - 2026-08-31

HTML + print-CSS input path

The release that adds a second front door to the engine: **`@formepdf/html`**, an HTML + print-CSS input path — "Satori for paginated documents." Write HTML with `@page` rules, feed it to `renderHtml()` or the `forme-html` CLI, get a paginated, deterministic PDF from the same Rust engine that renders JSX. No headless browser anywhere in the pipeline. Also: a layout-shape contract change for tables, four engine layout fixes, and HTML preview support across the renderer, CLI dev server, and VS Code extension.

### Added

**`@formepdf/html` — first public release.** The engine compiled to WASM behind an HTML front end:

- **Stylesheets, not just inline styles**: type/class/id/universal selectors, compounds, descendant/child combinators, grouping, the `:nth-child` and `:nth-of-type` families — with full cascade semantics including `!important`.
- **Paged media**: `@page` size and margins, `:first` variants, margin boxes with `counter(page)` / `counter(pages)`, `break-*` (plus legacy `page-break-*` aliases), `orphans`/`widows`, and `@media` media-type evaluation — print is the native media type.
- **Tables**: `border-collapse` emulation, `<thead>` repetition across page breaks, colspan/rowspan, `vertical-align` (and legacy `valign`).
- **Typography**: justified text, `text-transform`, `letter-spacing`, and provided fonts via `options.fonts` / `--font`, with a documented migration recipe for web fonts.
- **The warnings contract**: everything outside the supported subset is *named* — skipped stylesheet links, `@import`s, `@font-face` families, unsupported properties. Nothing is silently dropped.

Determinism is load-bearing: CI renders the fixture corpus through both the native binary and the WASM build and requires byte-identical PDFs.

**Engine — `@page :first`, vertical-align, min/max constraints.** Page one can carry its own `PageConfig` (size, margins, fixed-element filtering via First/NotFirst); table cells align content top/middle/bottom; and `max-width`/`min-width`/`min-height` now clamp properly — auto width + finite max-width + auto margins is the centered-column idiom, and it works.

**Renderer / CLI / VS Code — HTML preview everywhere.** `renderHtmlFromFile` / `renderHtmlFromSource` in `@formepdf/renderer` return the same PDF-bytes + `LayoutInfo` shape as the JSX pipeline, so every preview surface lights up unchanged: the CLI dev server and the VS Code extension both live-preview `.html` files with the full component tree, inspector, and layout overlays. The extension now bundles two WASM snapshots and hash-verifies both against their package sources at build time.

### ⚠️ Layout-shape contract change — `Table` wrapper node

`ElementNodeType` gains `'Table'`. Layout used to unwrap `<Table>` into loose sibling `TableRow` nodes; it now emits a `Table` container element per page fragment with the rows nested inside. Two reasons: table-level `border`/`background` finally have a paint target, and structural consumers (tagged PDF `/Table`, extractors) get a real table node. If you walk `LayoutInfo` for `TableRow` nodes directly, look inside `Table` wrappers now — or use `getTableRows()` from `@formepdf/core/layout`, which handles both shapes (loose rows from stored pre-0.14 layouts still return).

### Fixed

- **Runs-based text measured zero intrinsic width.** `measure_intrinsic_width` ignored `runs` (and measured leaf `Heading`s as 0, and whole multi-line strings instead of the widest line) — flex rows could collapse styled text to one character per line.
- **`col_span` was ignored when indexing column widths.** Every cell after a colspan cell sat one column too far left; the spanning cell now consumes its columns' combined width in both layout and row-height measurement.
- **`wrap: false` on tables was silently ignored.** Row-by-row pagination never consulted breakability; an unbreakable table that fits a fresh page now moves there whole (`break-inside: avoid` in the HTML path).
- **A flex line taller than the page emitted a blank leading page** before overflowing anyway; the break check now skips when the current page is empty.

### Docker images — security refresh

`formepdf/forme` and `formepdf/rasterizer` are published again at 0.14.0 (multi-arch, amd64 + arm64), rejoining the shared version line after being frozen at 0.10.5. Rebuilt on Alpine/musl with PDFium `chromium/8021` — Docker Scout reports **zero vulnerable packages** on both, and the server image dropped from 150 MB to 24 MB.

### Full changelog

[`@formepdf/html`](https://github.com/danmolitor/forme/blob/v0.14.0/packages/html/CHANGELOG.md) · [`engine`](https://github.com/danmolitor/forme/blob/v0.14.0/engine/CHANGELOG.md) · [`@formepdf/core`](https://github.com/danmolitor/forme/blob/v0.14.0/packages/core/CHANGELOG.md) · [`@formepdf/renderer`](https://github.com/danmolitor/forme/blob/v0.14.0/packages/renderer/CHANGELOG.md) · [`vscode`](https://github.com/danmolitor/forme/blob/v0.14.0/packages/vscode/CHANGELOG.md) · [`@formepdf/svelte`](https://github.com/danmolitor/forme/blob/v0.14.0/packages/svelte/CHANGELOG.md)

All other `@formepdf/*` packages, the `forme-pdf` crate on crates.io, the Python SDK, and the Go SDK got version-alignment bumps to 0.14.0.

## [0.13.0] - 2026-08-31

bookmark outline fixes + invoice discount column

Three defects in `bookmark` handling — one of which shipped real PDF corruption (duplicate outline entries) — plus a per-line discount column in the invoice template. Also the release where the VS Code extension joins the shared version line.

### Fixed

**Engine — three `bookmark` defects, one shared root cause.**

- **Duplicate PDF outline entries.** A bookmarked container that both overflows a page *and* carries visual styling wrote its outline entry twice — two identical `/Outlines` entries and `/Count 2` for a single `bookmark` prop. A genuine document defect, not a reporting one. The overflow path emits a zero-height marker so unstyled views can't lose their bookmark; styled views *also* built a wrapper carrying the same `bookmark`, and `collect_bookmarks` recurses — so it found both. Wrappers no longer carry `bookmark`; the marker is the sole carrier on every path.
- **Bookmarks on content that fits its page now appear in `LayoutInfo`.** The marker was only emitted on the overflow path, so consumers walking layout output for `Bookmark` nodes silently missed most bookmarks. The PDF was never wrong here — the outline was always complete — this was a `LayoutInfo` blind spot. PDF output is byte-identical after the fix (verified by hash: `8f41f7e9fb58b3cb` before and after).
- **The marker reports `nodeType: "Bookmark"` instead of `"None"`.** The unset node type fell back to `kind.to_string()` and leaked the `DrawCommand::None` variant name — a value never present in `ElementNodeType`.

All three came from the same divergence: two hand-maintained copies of the marker-building code. Both paths now share one `bookmark_marker()` helper.

### ⚠️ Behavior notes

- **Outline destination moves for one narrow case.** Markers now sit at the view's *outer* top edge, so every container path resolves a bookmark to the same coordinate. Only an unstyled, overflowing, breakable view with top padding or border shifts (Letter, 36pt margin, padding 20: destination Y 736.00 → 756.00). Styled views are unchanged.
- **Consumers matching `nodeType === 'None'`** (which the types never permitted) must switch to `'Bookmark'`. `"None"` was an accident of the fallback, not a semantic role.

### Added

**`@formepdf/templates` — per-line `discount` on the invoice template.** A decimal fraction off the line total, matching `taxRate`'s units rather than introducing a second convention. Applies **before** tax. The items table grows a fifth column; discounted lines render `-15%` in green, undiscounted ones an em dash. `invoiceExample` expanded from 5 to 19 line items (three pages), so the fixture actually exercises header repetition and page breaks — the template's main job.

### VS Code extension — version jump 0.10.5 → 0.13.0

The extension now tracks the monorepo version. It bundles the engine and `@formepdf/renderer` wholesale, so its independently-drifting number told you nothing about what was in the VSIX. No features were skipped; 0.11.x and 0.12.x simply never shipped as extension releases. Also picks up the sidebar fixes: bookmarked containers no longer show `None` in the component tree, and bookmarks on fitting content appear at all.

### How these were found

Dogfooding, continued from v0.12.1: structural regression baselines (via pdf-testkit) extended to the shipped templates. The `catalog` template's bookmarks tripped all three engine defects — the duplicate-entry bug had survived because every pre-existing bookmark test asserted `contains`, which two identical entries satisfy perfectly well. The new test asserts *counts*.

### Full changelog

[`engine`](https://github.com/danmolitor/forme/blob/v0.13.0/engine/CHANGELOG.md) · [`@formepdf/core`](https://github.com/danmolitor/forme/blob/v0.13.0/packages/core/CHANGELOG.md) · [`@formepdf/templates`](https://github.com/danmolitor/forme/blob/v0.13.0/packages/templates/CHANGELOG.md) · [`vscode`](https://github.com/danmolitor/forme/blob/v0.13.0/packages/vscode/CHANGELOG.md)

All other `@formepdf/*` packages got version-alignment bumps to 0.13.0. No functional changes.

## [0.12.1] - 2026-08-26

LayoutInfo type/runtime drift fix + accessor helpers

Patch release. **No runtime behavior changed** — the engine emits the exact same JSON it always did. The declared TypeScript types for `renderDocumentWithLayout()`'s output had drifted from that JSON in eight places, first flagged internally, then again by an external consumer's dogfood test. This release fixes them at the root and adds enforcement so it doesn't happen a third time.

### Fixed

**`@formepdf/core` — declared types now match runtime output.**

- **`ElementNodeType`** is now a closed literal union of the 30 nodeType values the engine actually emits (was `string`). Code like `if (element.nodeType === 'Heading')` — silently wrong before, since reality is discrete `'H1'`–`'H6'` — is now a TypeScript compile error. Same treatment for `ElementKind` (10 values) and 11 style enums (`ElementFlexDirection`, `ElementJustifyContent`, `ElementAlignItems`, `ElementAlignContent`, `ElementFlexWrap`, `ElementFontStyle`, `ElementTextAlign`, `ElementTextDecoration`, `ElementTextTransform`, `ElementOverflow`, `ElementPosition`). All exported for narrowing.
- **`ElementStyleInfo`** expanded from 19 to 34 fields. Previously missing: `alignContent`, `breakBefore`, `breakable`, `columnGap`, `rowGap`, `flexGrow`, `flexShrink`, `letterSpacing`, `minOrphanLines`, `minWidowLines`, `overflow`, `position`, `top`/`right`/`bottom`/`left`, `width`/`height`, `textDecoration`, `textTransform`.
- **`textContent`** on `ElementInfo` is now typed as `string | null | undefined` (was `string?`). Only populated on `TextLine` leaves — every non-`TextLine` node emits `null` at runtime. The old declaration made consumers reach for the wrong node.
- Every layout-time transform now documented explicitly on the `ElementInfo` JSDoc: `<Table>` unwraps into sibling `TableRow` nodes, `<OrderedList>` becomes `List` + `ListItem` + `Lbl`, `<Fixed>` splits into `FixedHeader`/`FixedFooter`, headings are discrete `H1`–`H6`, `<Text>` block content is split into `TextLine` leaves, inline elements don't get their own nodes, `<PageBreak>` produces no node.

### Added

**[`@formepdf/core/layout`](https://docs.formepdf.com/layout)** — new subpath export with stable accessor helpers. Additive; the raw `ElementInfo` tree is unchanged.

```ts
import {
  getNodeText, getTextLines,
  getHeadingLevel, getTableRows, getFixedRegions,
  getListItems, getListItemMarker,
  walkElements, findElements, findFirstElement,
  isNodeType,
} from '@formepdf/core/layout';
```

The helpers encapsulate each documented layout-time transform in a narrow, deliberately-maintained surface. Consumers get to say `getNodeText(paragraph)` instead of hand-rolling "walk `TextLine` children, gather `textContent`, join lines." When the transforms change in a future release, the helpers absorb the change — consumers ride through transparently.

See the [Layout API docs](https://docs.formepdf.com/layout) for the full reference and the "prefer helpers unless you need raw" guidance.

### ⚠️ Arguable-break notes — type-tightening exposes latent bugs

This is technically a patch because the underlying bug was in our declarations, not consumer code. But if your TypeScript code compiled against `@formepdf/core@0.12.0` and now fails against `0.12.1`, one of these is almost certainly why:

- **`style.flexDirection === 'row'`** — silently wrong before (runtime always emitted `'Row'`, Rust-side PascalCase); now a compile error. Standard types-tighten territory. Fix: use the PascalCase values, now exported as `ElementFlexDirection`. Same story for the other 10 style enums.
- **`element.textContent`** changing from `string | undefined` to `string | null | undefined` will flag code that assumed non-null. Fix: use the new `getNodeText()` helper (it handles this correctly by walking `TextLine` children), or explicitly handle `null` (which is what the runtime always emitted anyway).

Both cases were bugs before the release; TypeScript is now catching them for you. The CHANGELOG in `@formepdf/core` has more detail.

### Enforcement

Three-directional invariant now enforced in `@formepdf/core`'s own CI:

1. Every emitted `nodeType` / `kind` / style enum → member of its declared union (runtime test)
2. Every declared `ElementNodeType` → appears in the rich fixture (coverage tripwire — catches "component shipped without structural coverage")
3. Every union member → present in the test file's key record (compile-time check — catches "union grew without updating the test")

This closes the drift risk at all three angles. Runtime drift trips one test; declaration-side gaps trip a compile error before the file even runs.

### Full changelog

- [`@formepdf/core`](https://github.com/danmolitor/forme/blob/v0.12.1/packages/core/CHANGELOG.md) — the substantive work

All other `@formepdf/*` npm packages (`shared`, `react`, `svelte`, `preact`, `renderer`, `cli`, `hono`, `next`, `resend`, `mcp`, `sdk`, `tailwind`, `templates`) got version-alignment bumps to 0.12.1. No functional changes.

## [0.12.0] - 2026-08-26

@formepdf/preact adapter

Third-party framework support gets its second entry. Preact 10 now has a native authoring adapter, alongside the existing React and Svelte adapters.                                                             
                                                                                                                                                                                                                   
  ## New                                                                                                                                                                                                           
                                                                                                                                                                                                                   
  **[`@formepdf/preact`](https://www.npmjs.com/package/@formepdf/preact)** — Preact 10 adapter. Same component set as `@formepdf/react` (30+ components: `Document`, `Page`, `View`, `Text`, `H1`–`H6`, lists, inline formatting, tables, media, charts, form fields, layout primitives) with identical props and byte-identical serialized JSON. Requested via GitHub issue; parity is enforced by a fixture suite.            
                                                                                                                                                                                                                   
  ```bash                                                                                                                                                                                                          
  npm install @formepdf/preact @formepdf/core   
```                                                                                                                                                                   
           
```preact                                                                                                                                                                                                        
  /** @jsxImportSource preact */                                                                                                                                                                                   
  import { Document, Page, View, Text, renderDocument } from '@formepdf/preact';                                                                                                                                   
                                                                                                                                                                                                                   
  const pdf = await renderDocument(                                                                                                                                                                                
    <Document>                                                                                                                                                                                                     
      <Page size="Letter" margin={36}>                                                                                                                                                                             
        <Text style={{ fontSize: 24, fontWeight: 'bold' }}>Invoice</Text>                                                                                                                                          
      </Page>                                                                                                                                                                                                      
    </Document>                                                                                                                                                                                                    
  );      
```                                                                                                                                                                                                         
                                                                                                                                                                                                                   
Why this exists (vs preact/compat aliasing with @formepdf/react): no compat shim in your bundle (~7-8KB gzipped saved), no unmet-peer warning from npm about React not being installed, Preact-native JSX runtime. See the Preact guide.

Other packages                                                                                                                                                                                                   
                                                                                                                                                                                                                   
All other @formepdf/* npm packages (shared, react, svelte, core, renderer, cli, hono, next, resend, mcp, sdk, tailwind, templates) got version-alignment bumps to 0.12.0. No functional changes.                 
                                                                                                                                                                                                                   
  Full changelogs                                                                                                                                                                                                  
                                                                                                                                                                                                                   
  - @formepdf/preact

## [0.11.1] - 2026-08-21

SVG stroke-linecap / stroke-linejoin fix

Patch release. Fixes a real user-reported SVG rendering bug.                                                                                                                                                     
                                                                                                                                                                                                                   
  ## Fixed                                                                                                                                                                                                         
                                                                                                                                                                                                                   
  **SVG `stroke-linecap` and `stroke-linejoin` are now honored.** The engine's SVG parser was silently dropping both attributes on every element — `SvgCommand::SetLineCap` / `SvgCommand::SetLineJoin` only ever  
  fired from the Canvas API, never from SVG content. Every SVG stroke rendered with the PDF default (butt caps, miter joins) regardless of what the source said.                                                   
                                                                                                                                                                                                                   
  Reported against a real handwritten-signature repro: 68 short cubic bezier paths with `stroke-linecap="round"` rendered as visible flat rectangular cap protrusions at every segment terminus instead of blending
  into smooth round semicircles. Fixed.                                                                                                                                                                            
                                                                                                                                                                                                                   
  Attribute values also inherit through `<g>` group ancestors on the same stack as `fill` / `stroke` / `stroke-width` / `opacity`, so `<g stroke-linecap="round">…</g>` works as expected.                         
                                                                                                                                                                                                                   
  ## Who this affects                                                                                                                                                                                              
                                                                                                                                                                                                                   
  Anyone rendering SVG content — through `<Svg content="..." />` in `@formepdf/react` or `@formepdf/svelte`, `renderPdf` on `@formepdf/core`, or the `forme-pdf` Rust crate — where any element uses               
  `stroke-linecap` other than the default `butt`, or `stroke-linejoin` other than the default `miter`. Very common pattern for signature capture widgets, chart annotations, and hand-drawn overlays.              
                                                                                                                                                                                                                   
  ## Install                                                                                                                                                                                                       
                                                                                                                                                                                                                   
  ```bash                                                                                                                                                                                                          
  npm install @formepdf/react@0.11.1 @formepdf/core@0.11.1                                                                                                                                                         
  # or                                                                                                                                                                                                             
  npm install @formepdf/svelte@0.11.1 @formepdf/core@0.11.1 
```

Rust:                                                                                                                                                                                                            
  forme-pdf = "0.11.1"                                                                                                                                                                                             
                                                                                                                                                                                                                   
  No API changes. Drop-in.                                                                                                                                                                                         
                                                                                                                                                                                                                   
  Full changelogs                                                                                                                                                                                                  
                                                                                                                                                                                                                   
  - engine — the fix + 4 new integration tests                                                                                                                                                                     
  - @formepdf/core — WASM rebuild carrying the fix                                                                                                                                                                 
                                                                                                                                                                                                                   
  All other npm packages (shared, react, svelte, renderer, cli, hono, next, resend, mcp, sdk, tailwind, templates) got version-alignment bumps only.

## [0.11.0] - 2026-08-10

Svelte adapter + @formepdf/shared

Two brand-new packages join the family, and the existing packages get an internal refactor to support them.                                                                                                      
                                                                                                                                                                                                                   
### New packages
                                                                                                                                                                                                                   
  **[@formepdf/svelte](https://www.npmjs.com/package/@formepdf/svelte)** — Svelte 5 authoring adapter with the full component set as `.svelte` files. Layout (`Document`, `Page`, `View`, `Text`), semantic headings (`H1`–`H6`), lists, inline formatting, tables, media (`Image`, `Svg`, `QrCode`, `Barcode`, `Canvas`, `Watermark`), all five chart types, form fields, and `PageBreak` / `Fixed`. Same props as `@formepdf/react`, so anything you know from JSX just works. Includes a `formePreview()` SvelteKit route helper for the live preview UI and one-call `renderDocument()` / `renderDocumentWithLayout()` wrappers over `@formepdf/core`.                                                                                                                                                                                           
                                                                                                                                                                                                                   
  ```bash                                                                                                                                                                                                          
  npm install @formepdf/svelte @formepdf/core
```                                                                                                                                                                      
                                                                                                                                                                                                                   
  See the Svelte guide.                                                                                                                                                                                            
                                                                                                                                                                                                                   
  @formepdf/shared — framework-neutral serialization core. Extracted from @formepdf/react so both React and Svelte adapters (and future ones) share the same document-model types, Style mapping, CSS shorthand parsing, Font registration store, Canvas recorder, chart kind builders, and semantic-component defaults. Internal-facing but published for adapter authors.                                                      
                                                                                                                                                                                                                   
  Changed                                                                                                                                                                                                          
                                                                                                                                                                                                                   
  - @formepdf/react — internal refactor: framework-neutral internals moved to @formepdf/shared and re-exported. Public API is unchanged — all 215 tests pass without modification.                                 
  - @formepdf/core — new renderSerializedDoc(doc, options?) and renderSerializedDocWithLayout(doc, options?) exports. Accept a pre-serialized FormeDocument (JSON), used by the Svelte adapter to hand off after its SSR-then-parse pass.                                                                                                                                                                                         
  - All other npm packages: version alignment only, no functional changes.                                                                                                                                         
                                                                                                                                                                                                                   
  Full changelogs                                                                                                                                                                                                  
                                                                                                                                                                                                                   
  - @formepdf/svelte                                                                                                                                                                                               
  - @formepdf/shared                                                                                                                                                                                               
  - @formepdf/react                                                                                                                                                                                                
  - @formepdf/core                                                                                                                                                                                                 
                                                                                                                                                                                                                   
  Svelte adapter contributed by @cmjoseph07 in #21.

## [0.10.5] - 2026-07-03

**Patch release.** Fixes two table page-break regressions reported on GitHub. Zero breaking changes — safe drop-in from `0.10.4`.

---

### What's fixed

#### 1. Orphan header at the bottom of a page

When a `<Table>` with `<Row header>` sat low enough on a page that the header alone fit in the remaining space but the first body row did not, the header rendered at the bottom of the page with nothing beneath it, then repeated above the actual rows on the next page.

**Before:**

```
Page 1: [ …content… ]  [Header]                ← orphaned
Page 2: [Header]  [Row 1]  [Row 2]  …
```

**After:**

```
Page 1: [ …content… ]
Page 2: [Header]  [Row 1]  [Row 2]  …
```

Closes GitHub issue reported against 0.10.4.

#### 2. Long-token header text contamination

When a header cell contained a long token with no line-break opportunities (e.g. `"Amount".repeat(40)`), it wrapped to many lines, and the table starting low on a page could leak header text onto the previous page while the actual table rendered across three pages instead of two.

Same fix — the table now moves cleanly to a fresh page where the tall header has full page height to wrap into. No cross-page contamination, correct page count.

---

### What changed under the hood

The 0.10.4 pre-fit check in `layout_table` gated only on `total_header_h > remaining_height`. It now also folds in the first body row's measured height:

```rust
let needed = total_header_h + first_body_h;
if needed > cursor.remaining_height() && needed <= fresh_page_available {
    // page-break the table before the header, not after it
}
```

The fresh-page cap keeps the 0.10.4 `!is_header` cell-overflow guard as the safety net for the rare case where `header + first row` is genuinely taller than a page.

Two new integration tests — both verified to fail without the fix and pass with it:

- `test_table_header_no_orphan_when_first_body_row_doesnt_fit`
- `test_table_long_header_text_no_page_contamination`

---

### Which packages carry the fix

The fix lives in the Rust engine, so every consumer of engine 0.10.5 picks it up automatically:

- npm: `@formepdf/react`, `@formepdf/core`, `@formepdf/renderer`, `@formepdf/cli`, `@formepdf/hono`, `@formepdf/next`, `@formepdf/mcp`, `@formepdf/resend`, `@formepdf/sdk`, `@formepdf/tailwind`, `@formepdf/templates` — all `0.10.5`
- PyPI: `formepdf` — `0.10.5`
- crates.io: `forme-pdf` — `0.10.5`
- Go: `github.com/formepdf/forme-go` — `v0.10.5`
- Docker Hub: `formepdf/rasterizer:0.10.5`, `formepdf/forme:0.10.5`
- VS Code Marketplace: `forme-pdf` — `0.10.5`

---

### Upgrade

```bash
# npm — bump both simultaneously; @formepdf/core carries the WASM
npm install @formepdf/react@0.10.5 @formepdf/core@0.10.5

# Python
pip install --upgrade formepdf==0.10.5

# Rust
cargo update -p forme-pdf

# Go
go get github.com/formepdf/forme-go@v0.10.5

# Docker (self-hosted)
docker pull formepdf/forme:0.10.5
```

---

### Full changelogs

- Engine: [`engine/CHANGELOG.md`](engine/CHANGELOG.md)
- Server: [`server/CHANGELOG.md`](server/CHANGELOG.md)
- Individual packages: `packages/*/CHANGELOG.md`

## [0.10.4] - 2026-06-05

Four layout bug fixes, all user-reported. The
table header one is silent — upgrade if you use
`<Row header>`.

---

### Fixed

**Tables with `<Row header>` no longer inflate
page count 3–5×**

When a table started low enough on a page that
the header didn't fit before a page break, the
engine emitted multiple near-duplicate pages —
the same body rows repeated, with the header
visibly "doubling and sliding one column to the
right" on each successive page. A 24-row,
5-column table produced 8 pages where the same
content without a header row produced 3.

```tsx
// Before: 8 pages, garbled doubled headers
// After:  3 pages, correct

  
  
    {headerCells}
    {rows.map(r => {...})}
  

```

**`<View>` wrapping a `<Table>` no longer
auto-grows to roughly the page height**

`measure_node_height` had no handler for
`Table` or `TableRow`, so they fell into a
generic path that summed cell heights instead
of taking the max — a 3-column row of 16pt
cells measured to 48pt, and any wrapping
`<View>` inherited that inflation. Now
delegates to the same helpers `layout_table`
already uses, so measurement matches what
renders.

**`<Svg viewBox="…">` content scales to fit
the display box**

SVG paths previously rendered at raw viewBox
coordinates and overflowed — the viewBox
parameters were parsed but unused, and the
PDF scale was always 1.0. Now implements the
SVG viewport algorithm with `xMidYMid meet`
as the default `preserveAspectRatio` (uniform
`min(sx, sy)` scale + centering).

```tsx
// Before: paths spilled outside the 200×80 box
// After:  scaled to fit

  …paths…

```

**`marginTop: 'auto'` works in column layouts**

Previously a no-op in `flexDirection: 'column'`
parents — only the horizontal version worked.
Now distributes slack the same way: top-only
pushes to bottom, both autos center, bottom-only
carries forward. Auto margins consume slack
before `justifyContent`, per the CSS spec.

```tsx
// "Sign here" now sits at the bottom, not the top

  
    Sign here
  

```

---

### Upgrade

```bash
npm install @formepdf/core@0.10.4 @formepdf/react@0.10.4
# plus any of: cli renderer hono next mcp resend sdk tailwind templates
```

Other consumers:
- Rust: `cargo add forme-pdf@0.10.4`
- Python: `pip install formepdf==0.10.4`
- Go: `go get github.com/formepdf/forme-go@v0.10.4`
- Docker: `docker pull formepdf/forme:0.10.4`
  `docker pull formepdf/rasterizer:0.10.4`
- VS Code: update "Forme PDF Preview" in the
  Extensions panel

## [0.10.3] - 2026-05-28

Bug-fix release. If you're on 0.10.2, upgrade - it shipped a silent text-layout regression that caused right-aligned text to render off-page.

---

### Fixed

**`<Text style={{ width }}>` inside a flex row now renders at the correct width**

A 0.10.2 regression caused text elements with an explicit width inside a flex row to be sized to the full row width instead. With `textAlign: 'right'`, glyphs were aligned to the right edge of the oversized box and pushed off the page — clipped by PDF viewers and effectively invisible.

The bug was easy to miss: PDF bytes were deterministic, so byte-hash snapshot tests still passed. The corruption only appeared when opening the file.

```tsx
// Broken on 0.10.2 — "$10.00" rendered off-page
// Correct again on 0.10.3
<View style={{ flexDirection: 'row', justifyContent: 'flex-end' }}>
  <Text style={{ width: 120, textAlign: 'right' }}>Tax:</Text>
  <Text style={{ width: 80,  textAlign: 'right' }}>$10.00</Text>
</View>
```

`layout_text` and the text branch of `measure_node_height` now honor a resolved fixed `style.width`, matching how `<View>` and `<Image>` already behaved. The `<View style={{ width: N }}>` wrapper workaround is no longer needed.

The 0.10.2 flex-percentage and grid-page-break fixes are unaffected.

---

### Upgrade

```bash
npm install @formepdf/core@0.10.3 @formepdf/react@0.10.3
# plus any of: cli renderer hono next mcp resend sdk tailwind templates
```

Other consumers:
- Rust: `cargo add forme-pdf@0.10.3`
- Python: `pip install formepdf==0.10.3`
- Go: `go get github.com/formepdf/forme-go@v0.10.3`
- Docker: `docker pull formepdf/forme:0.10.3`
  `docker pull formepdf/rasterizer:0.10.3`
- VS Code: update "Forme PDF Preview" in the
  Extensions panel

## [0.10.2] - 2026-05-21

Two engine layout bug fixes. No API changes — 
upgrade is drop-in.

---

### Fixed

**Flex row percentage widths resolve against 
the parent**

Children of a `flexDirection: 'row'` container 
with explicit percentage widths were being 
double-resolved — `width: '30%'` was computed 
as 30% of the child's already-distributed width 
(~9% of the row) instead of 30% of the row itself.

```tsx
<View style={{ flexDirection: 'row' }}>
  <View style={{ width: '30%' }}><Text>30%</Text></View>
  <View style={{ width: '70%' }}><Text>70%</Text></View>
</View>
// Before: 30% child was ~44pt wide on a 487pt row
// After:  30% child is 146pt, 70% child is 341pt
```

**Grid page-break keeps columns aligned**

Grid containers that didn't fit on the current 
page were scattering each column onto its own 
page. An off-by-one guard in the first row's 
page-break check suppressed the break, causing 
each cell's content to individually overflow 
and trigger its own new page. The entire row 
now moves to the next page together, keeping 
columns at the same y position.

---

### Upgrade

```bash
npm install @formepdf/core@0.10.2 @formepdf/react@0.10.2
# plus any of: cli renderer hono next mcp resend sdk tailwind templates
```

Other consumers:
- Rust: `cargo add forme-pdf@0.10.2`
- Python: `pip install formepdf==0.10.2`
- Go: `go get github.com/formepdf/forme-go@v0.10.2`
- Docker: `docker pull formepdf/forme:0.10.2`

## [0.10.1] - 2026-05-20

Patch release fixing two regressions introduced in 0.10.0. No engine or rendering changes — existing PDFs render identically.
 
---
 
### Fixed
 
**Cloudflare Workers crash on import**
 
`@formepdf/core@0.10.0` called `wasm.__wbindgen_start()` at the top level of the bundler-target build. Wrangler passes `import wasm from '*.wasm'` back as `{ default: WebAssembly.Module }` rather than an instantiated namespace, so this threw immediately:
 
```
TypeError: wasm.__wbindgen_start is not a function
```
 
Fixed by shipping a third build (`--target web` → `pkg-web/`) and a new `dist/worker.js` entry that takes an explicit `init(wasmModule)` call. The `worker` / `edge-light` / `deno` conditional exports now route here. Your 0.9.x pattern works again:
 
```ts
import { init, renderDocument } from '@formepdf/core'
import wasm from '@formepdf/core/pkg-web/forme_bg.wasm'  // recommended
// or:
import wasm from '@formepdf/core/pkg/forme_bg.wasm'      // legacy, also works
await init(wasm)
```
 
**Missing `pkg-node/` in the published tarball**
 
`wasm-pack`'s `--target nodejs` output ships with a `.gitignore` containing `*`. `npm publish` honored it and silently dropped the entire directory. The 0.10.0 Node entry tried to import from `../pkg-node/forme.js`, which wasn't there:
 
```
Cannot find module '../pkg-node/forme.js' from
'node_modules/@formepdf/core/dist/index.js'
```
 
Fixed by stripping `.gitignore` from all three `pkg` dirs during the WASM build, and adding a `prepublishOnly` assertion that re-packs the tarball and fails if anything required is missing.
 
**VS Code extension build path mismatch**
 
The bundled extension read `${__dirname}/forme_bg.wasm` but the esbuild config still copied the WASM to the old `pkg/` location. Now copies to `dist/forme_bg.wasm`. Only affected fresh 0.10.x rebuilds of the extension.
 
---
 
### Added
 
- `@formepdf/core/worker` subpath export
- `@formepdf/core/pkg-web/forme.js` and `@formepdf/core/pkg-web/forme_bg.wasm` subpath exports
- `packages/core/scripts/assert-tarball.sh` — runs as `prepublishOnly` and on every CI PR. Asserts every entry-point file is present in the tarball and that no `pkg` dir ships a stray `.gitignore`. Would have caught both regressions above at PR time.
- Workerd smoke test (`@cloudflare/vitest-pool-workers`) under `npm run test:workers`. Runs inside real workerd, calls `init(wasm)` + `renderPdf()`, and verifies the result has a `%PDF` header. Catches the exact regression class that Node-based unit tests can't see.
---
 
### Migration
 
**You don't need to change anything.** Existing 0.10.0 code works on 0.10.1.
 
If you're on Cloudflare Workers and were hitting the `__wbindgen_start` crash, upgrade to 0.10.1 — the `import` + `init()` pattern from 0.9.x is restored. `pkg-web/forme_bg.wasm` is recommended over `pkg/forme_bg.wasm` for new Workers code, but the legacy path still works.
 
---
 
### Packages updated
 
| Package | Reason |
|---------|--------|
| `@formepdf/core` | The fix |
| `@formepdf/renderer`, `@formepdf/cli`, `@formepdf/hono`, `@formepdf/next`, `@formepdf/mcp`, `@formepdf/resend` | Updated to pull the core fix transitively |
| `@formepdf/react`, `@formepdf/sdk`, `@formepdf/tailwind`, `@formepdf/templates` | Version parity bump only — no code changes |
| VS Code extension | Build config fix for new core layout |
 
**Not changed:** Rust engine (same WASM bytecode across all three targets), Rust crate, Python SDK, Go SDK, Docker images, rasterizer — all still on 0.10.0.

## [0.10.0] - 2026-05-19

### New visual properties

Six new style features in the engine, available in React JSX and JSON document inputs.

**`opacity` cascades to children**
Previously, `opacity: 0.5` on a `<View>` faded the background but left text at full alpha. Opacity now wraps the full element subtree — children inherit the fade, and nested opacities multiply correctly.

```tsx
<View style={{ opacity: 0.5 }}>
  <Text>Fades with the background</Text>
</View>
```

**`wordSpacing`**
Maps to the PDF `Tw` operator. Stacks additively with `text-align: 'justify'`.

```tsx
<Text style={{ wordSpacing: 4 }}>extra space between words</Text>
```

**`boxShadow`**
Offset drop shadow behind any element. Honors `borderRadius`. Accepts an object or CSS shorthand string.

```tsx
<View style={{
  borderRadius: 12,
  boxShadow: { offsetX: 2, offsetY: 4, blur: 0, color: '#00000033' },
}}>...</View>

// or:
<View style={{ boxShadow: '4 6 0 #00000040' }}>...</View>
```

**`borderRadius` + rounded clipping**
`borderRadius` now also rounds the overflow clip path when `overflow: 'hidden'` is set. Children that exceed the parent's bounds clip to the rounded corners, not a sharp rectangle.

```tsx
<View style={{ overflow: 'hidden', borderRadius: 16 }}>
  <Image src="..." />  {/* clipped to rounded corners */}
</View>
```

**Page `backgroundImage`**
Watermark-style background images on every page. Supports `backgroundSize`, `backgroundPosition`, and `backgroundOpacity`. The same URL across multiple pages shares a single embedded XObject.

```tsx
<Page
  backgroundImage="https://cdn.example.com/logo.png"
  backgroundSize="cover"
  backgroundPosition="center"
  backgroundOpacity={0.08}
>...</Page>
```

**CSS gradients via `background`**
Linear and radial gradients with CSS-compatible syntax. Multi-stop gradients use PDF Type 3 stitching functions. Supports `deg`, `turn`, `rad`, `grad` units and `to <side>` keywords.

```tsx
<View style={{ background: 'linear-gradient(135deg, #667eea, #764ba2)' }} />
<View style={{ background: 'radial-gradient(circle, #10b981, #059669)' }} />
<View style={{ background: 'linear-gradient(180deg, #ff0000 0%, #00ff00 50%, #0000ff 100%)' }} />
```

---

### Build fix: Next.js / Webpack / Turbopack

`@formepdf/core@0.9.x` shipped a `--target web` WASM build that referenced `./forme_bg.js` — a file `wasm-pack` doesn't emit for that target. Static-analysis bundlers (Next.js Webpack and Turbopack) failed on the missing file.

`@formepdf/core@0.10.0` now ships two WASM builds:
- `pkg/` (`--target bundler`) — for Vite, Webpack, Turbopack, Wrangler
- `pkg-node/` (`--target nodejs`) — for Node SSR; self-initializes via `fs`

`@formepdf/next` and `@formepdf/hono` drop their previous `init(wasm)` workaround. The browser entry now instantiates WASM implicitly at module load.

#### Vite users: action required

```bash
npm install -D vite-plugin-wasm vite-plugin-top-level-await
```

```ts
// vite.config.ts
import wasm from 'vite-plugin-wasm';
import topLevelAwait from 'vite-plugin-top-level-await';

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  worker: {
    format: 'es',
    plugins: () => [wasm(), topLevelAwait()],
  },
});
```

Without these plugins, Vite throws `"ESM integration proposal for Wasm" is not supported currently`.

---

### Security: `@formepdf/mcp` sandbox hardening

The `render_custom_pdf` sandbox has been rebuilt from the ground up.

**What was wrong:** The previous `new Function(...)` evaluator was bypassable in one line via `new Function('return process')()`. The 30-second timeout only covered the WASM render step — a `while(true){}` template hung the MCP server indefinitely. `validateOutputPath` was effectively a no-op.

**What changed:**
- Worker-thread isolation with 128 MB memory cap and crash containment
- `vm.Context` with `codeGeneration: false` — blocks `eval` and string-based `Function`
- `vm.runInContext` with a 5-second sync timeout that actually interrupts infinite loops
- 10-second wall-clock timeout backed by `worker.terminate()`
- AST denylist (acorn) — clear error messages for blocked patterns before the worker starts
- Post-eval asset sanitizer — font/image `src` must be `data:` URIs; closes the file-path exfiltration vector
- Output path allowlist — writes restricted to CWD by default; opt-in via `FORME_MCP_OUTPUT_DIRS`

**Trust model:** This sandbox is hardened for accidental misuse on a trusted local machine. It is not a service-grade boundary for arbitrary attacker code — for that, use containers or `isolated-vm`. The README now says this explicitly.

**Known limitation:** The 5-second sync timeout does not interrupt async hangs. A template that `await`s an unresolved Promise is caught by the outer 10-second wall-clock timeout instead.

---

### Install

```bash
# JavaScript / TypeScript
npm install @formepdf/react@0.10.0 @formepdf/core@0.10.0

# Python
pip install formepdf==0.10.0

# Rust
cargo add forme-pdf@0.10.0

# Go
go get github.com/formepdf/forme-go@v0.10.0

# Docker
docker pull formepdf/forme:0.10.0
docker pull formepdf/rasterizer:0.10.0
```

Per-package changelogs: [[engine](https://claude.ai/chat/engine/CHANGELOG.md)](engine/CHANGELOG.md) · [[core](https://claude.ai/chat/packages/core/CHANGELOG.md)](packages/core/CHANGELOG.md) · [[react](https://claude.ai/chat/packages/react/CHANGELOG.md)](packages/react/CHANGELOG.md) · [[mcp](https://claude.ai/chat/packages/mcp/CHANGELOG.md)](packages/mcp/CHANGELOG.md) · [[next](https://claude.ai/chat/packages/next/CHANGELOG.md)](packages/next/CHANGELOG.md) · [[hono](https://claude.ai/chat/packages/hono/CHANGELOG.md)](packages/hono/CHANGELOG.md)

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
