# Changelog

## [0.14.0] - 2026-08-28

### Changed

- Regenerated the embedded preview HTML from `@formepdf/renderer` (picks up the preview's HTML-input support). No Svelte-adapter API changes.

## [0.13.0] - 2026-08-27

_Version bump only — 0.13.0 fixes three `bookmark` defects in the engine (duplicate PDF outline entries, no layout marker on the fits path, a `nodeType: "None"` leak) and adds a per-line discount column to the invoice in `@formepdf/templates`. No changes to this package._

## [0.12.1] - 2026-08-26

_Version bump only — 0.12.1 fixes LayoutInfo/ElementInfo type declarations and adds `@formepdf/core/layout` accessor helpers. No changes to this package._

## [0.12.0] - 2026-08-25

_Version bump only — 0.12.0 introduces the new `@formepdf/preact` adapter (Preact 10 authoring). No changes to this package._

## [0.11.1] - 2026-08-20

_Version bump only — engine 0.11.1 fixes SVG `stroke-linecap` / `stroke-linejoin` via `@formepdf/core`._

## [0.11.0] - 2026-08-09

Initial release.

### Added
- Svelte 5 adapter with the full component set authored as `.svelte` files: layout (`Document`, `Page`, `View`, `Text`), semantic headings (`H1`-`H6`), lists (`OrderedList`, `UnorderedList`, `ListItem`), inline formatting (`Strong`, `Em`, `Code`, `Link`), tables, graphics (`Image`, `Svg`, `QrCode`, `Barcode`, `Canvas`, `Watermark`), charts, and form fields
- `renderDocument()` / `renderDocumentWithLayout()` wrappers over the optional `@formepdf/core` peer dependency for one-call SvelteKit endpoints
- `formePreview()` SvelteKit route helper serving the live preview UI with layout overlays and click-to-inspect
- `PAGE_NUMBER` / `TOTAL_PAGES` page-number constants
- `Font` registration and `StyleSheet` parity with `@formepdf/react`; `tw()` from `@formepdf/tailwind` works in `.svelte` templates

### Notes
- Compiled templates (the hosted-API expression system) remain TSX-only
