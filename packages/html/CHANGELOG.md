# Changelog — @formepdf/html

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
