# Changelog — @formepdf/html

## [Unreleased]

### Added

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
