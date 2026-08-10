# Changelog

## [Unreleased]

Initial release.

### Added
- Svelte 5 adapter with the full component set authored as `.svelte` files: layout (`Document`, `Page`, `View`, `Text`), semantic headings (`H1`-`H6`), lists (`OrderedList`, `UnorderedList`, `ListItem`), inline formatting (`Strong`, `Em`, `Code`, `Link`), tables, graphics (`Image`, `Svg`, `QrCode`, `Barcode`, `Canvas`, `Watermark`), charts, and form fields
- `renderDocument()` / `renderDocumentWithLayout()` wrappers over the optional `@formepdf/core` peer dependency for one-call SvelteKit endpoints
- `formePreview()` SvelteKit route helper serving the live preview UI with layout overlays and click-to-inspect
- `PAGE_NUMBER` / `TOTAL_PAGES` page-number constants
- `Font` registration and `StyleSheet` parity with `@formepdf/react`; `tw()` from `@formepdf/tailwind` works in `.svelte` templates

### Notes
- Compiled templates (the hosted-API expression system) remain TSX-only
