# Changelog — @formepdf/html

## [Unreleased]

### Fixed

- **Vertical centering in fixed-height boxes works via flex.** `display: flex; align-items: center` (and `flex-end`) on a fixed-height box now actually aligns its content — the flex line was previously sized at the content's own height, making cross-axis alignment a silent no-op (the logo-mark idiom: a 36×36 box with two letters). Engine fix; see `engine/CHANGELOG.md` for the behavior note.
- **The `line-height` centering idiom works.** Half-leading is applied engine-wide: the pre-flexbox pattern of matching `line-height` to a box height now genuinely centers, and every text baseline sits half the leading lower in its line box (closer to Chrome's placement). Layout geometry and page breaks are unchanged — only the ink inside each line box moves. See `engine/CHANGELOG.md` for the behavior-change note.

## [0.19.0] - 2026-09-04

### Fixed (table sections + absolute positioning)

- **`position: running()` elements no longer render in flow.** Per CSS GCPM the element leaves normal flow entirely (it would only appear where a margin box says `content: element(name)`, which is unsupported) — previously it was warned by name but still rendered in place, producing stray header text at the top of the page. The warning stays and now says the element was removed; `@page` margin boxes are the supported way to get running headers/footers.

- **`<tfoot>` renders at the bottom of the table** regardless of where it sits in the markup — HTML allows (and templates commonly use) tfoot before tbody so streaming renderers see it early; it previously rendered in DOM order.
- **Only the first `<thead>` is the repeating table header.** A later thead is an ordinary row group in DOM order, per HTML — previously its rows were hoisted to the top of the table (a totals block kept in a second thead printed above the line items).
- **`display: none` now applies to table rows and sections.** The row-collection path bypassed the check every other element gets, so rows a template hides for JS to reveal (which we don't run) rendered anyway.
- **`bottom`/`right`-anchored absolute boxes respect their margins** — a behavior change: CSS offsets position the *margin edge*, and we anchored the border box, so any `position:absolute` element with a `bottom`/`right` offset and margins rendered past its anchor by the margin size. If a positioned element sits higher/further left after upgrading, that's the spec-correct spot (a `bottom:0` footer with `margin-top` previously rendered past the page bottom, leaving only ascender tips: the corpus's "Thk ft"). Engine fix — see `engine/CHANGELOG.md`.

### Fixed (measure/layout agreement — phantom vertical gaps)

- **Flex rows with percent-width children no longer reserve phantom height** (rows measured 2.5–4× taller than their content when children carried percent widths — visible as large gaps below float rows and flex headers).
- **Margin-box content centers correctly in its band.** At boundary widths, measurement wrapped text that layout kept on one line, skewing the band's vertical centering (running headers sat too high, page numbers too low).
- **Absolutely-positioned elements no longer leave a gap** equal to their own height in every auto-height ancestor.

These came out of the new engine-level measure/layout agreement gate (`FORME_MEASURE_CHECK=1` + `tests/measure_check.rs`), which renders the fixture corpus and fails on any measured-vs-laid-out height divergence.

### Fixed (the whitespace/fixed-dimensions family)

- **Images size like Chrome.** `width: 100%`, percent widths generally, and `max-width` are honored on `<img>`, with height following the real aspect ratio — a small logo styled `width:100%; max-width:300px` now renders as Chrome does instead of reserving a container-sized phantom block and drawing at intrinsic size (the single biggest whitespace cause in the template corpus).
- **Page-sized body declarations are clamped and warn.** `body { width: 21cm; height: 29.7cm }` (the mPDF idiom for "I am the page") previously cut off the right edge and forced a blank first page; it now clamps to the content box with a warning naming the remedy: page geometry belongs in `@page (size, margin)`.
- **Giant atomic table rows no longer produce blank pages.** The email-template idiom (everything inside one `<tr>`) rendered with empty pages before and after its content; the content now flows and the unfittable-row case reports itself through the render-defect channel.

### Added

- **Float support (document subset).** `float: left/right` + `clear` are in-subset: runs of consecutive floated siblings form a row — left floats in markup order, right floats stacking right-to-left, over-wide runs dropping to new float lines, shrink-to-fit auto widths — which is the shape every affected template in the real-world corpus actually uses (Bootstrap `col-*`, left/right header pairs; zero of them wrap text around a float). Per CSS, `float` is silently ignored on flex items and table-internal elements. The honest boundary stays loud: a non-floated sibling after an uncleared float renders **below** it, not beside it, and warns (`text wrapping alongside floats is not supported; floated siblings are laid out as columns`).


## [0.18.0] - 2026-09-03

### Fixed

- **Banner-row tables no longer collapse.** Tables opening with a colspan row (most real invoices) hit an engine column-count bug that shredded later cells into one-character-per-line vertical text — fixed with browser-style automatic table layout (see engine CHANGELOG). Column widths are also now harvested from the first colspan-free row instead of being discarded whenever the first row spans.

### Added

- **Warning dedup.** Identical warnings collapse to one entry carrying a count ("float is not supported … (×214)"). Framework stylesheets previously produced thousands of identical lines (AdminLTE: 6,905) that drowned the signal.
- Render-defect warnings from the engine (`render defect:` prefix) now surface: silent in-scope layout failures — the gap the template-compat experiment exposed — report themselves.
- `pdfA` accepts `"3b"`, `"3u"`, `"3a"` (PDF/A-3 — same rules as part 2 plus permission for embedded files). The whole corpus is veraPDF-gated at 3b/3a alongside 2b/2a.


## [0.17.0] - 2026-09-03

### Added

- **`@page :left` / `:right`** — mirrored margins (left+right must sum equally with the base `@page`; unequal sums warn and normalize), per-side margin boxes on edges the base `@page` also defines. Page 1 is a `:right` page (LTR page progression); `dir="rtl"` parity is not modeled and warns. `:first` outranks parity.
- **Named pages** — `@page <name>` + the `page: <name>` property. The named run starts at a forced break, so top/bottom margins are real (a zero-margin cover works); left/right follow the mirrored-sum rule. Named margin boxes override or suppress the base running headers per name (`content: none` restores the real margin). Composes: `@page cover:first` / `:left` / `:right`. Out of scope warns by name: `:blank`, `:nth()` page groups, `string-set` / `content: string()`, `position: running()`, footnotes.
- **CSS Grid through the mapper** — the documented subset: `display: grid`, `grid-template-columns/rows` (`px`/`pt`/`em`/`fr`/`auto`/`minmax()`/integer `repeat()`), gaps, `grid-auto-rows/columns`, and item placement via `grid-column`/`grid-row`. Everything outside the subset warns by name.

### Fixed

- **`counter(page)` / `counter(pages)` with a provided font** no longer render the digits in base-14 Helvetica — the running footer stays in the margin box's own font, and PDF/A eligibility survives page numbering (engine fix; see `engine/CHANGELOG.md`).


## [0.14.0] - 2026-08-28

### Added
- First public release of the HTML + print-CSS input path ("Satori for
  paginated documents"): `renderHtml()` and the `forme-html` CLI over the
  Rust engine compiled to WASM — no headless browser.
- Stylesheets with the documented selector subset (type/class/id/universal,
  compounds, descendant/child combinators, grouping, the `:nth-child` and
  `:nth-of-type` families), full cascade with `!important`.
- Paged media: `@page` size/margins, `:first` variants, margin boxes with
  `counter(page)`/`counter(pages)`, `break-*` (+ legacy `page-break-*`),
  `orphans`/`widows`, `@media` media-type evaluation (print is the native
  media type).
- Tables: `border-collapse` emulation, `<thead>` repetition,
  colspan/rowspan, `vertical-align` + legacy `valign`.
- Typography: justify, `text-transform`, `letter-spacing`, provided fonts
  via `options.fonts` / `--font` with the web-fonts migration recipe.
- The warnings contract: everything outside the subset is named — skipped
  stylesheet links, `@import`s, `@font-face` families, unsupported
  properties — never silently dropped.

> Joined the monorepo's shared version line at 0.14.0 (previously 0.1.0,
> unpublished). The `forme-pdf-html` crate ships via npm only.
