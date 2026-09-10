# Changelog

## [0.22.0] - 2026-09-10

### Added

- **`pdfUa2` prop** on `<Document>` — PDF/UA-2 (ISO 14289-2:2024), the PDF 2.0 accessibility claim. Implies `pdfVersion: "2.0"` (every font must be embedded); composes with `pdfa: "4"`/`"4f"`; contradicts `pdfUa` and the 1.7 `pdfa` levels (refused by name at render).
- **`pdfVersion` prop and `pdfa: "4" | "4f"`** now declared in this adapter's prop TYPES — they reached serialization in 0.21.x but not the types users compile against.
- **Compile-time claim-parity guard**: the Document component's props array is asserted (under `vue-tsc`) to contain every claim in `FormeDocumentClaimProps`; array-form props carry no types, so the shared parser's guard covers the types on this path.

## [0.20.1] - 2026-09-07

### Changed

- Version alignment with the 0.20.1 release line; no functional changes in this package.

## [0.20.0] - 2026-09-05

### Changed

- Version alignment with the 0.20.0 release line; no functional changes in this package.

## [0.15.0] - Unreleased

### Added

- Initial release of `@formepdf/vue`: the Vue 3 adapter for Forme. Author documents as `.vue` single-file components using the full Forme component set, rendered via Vue SSR (`renderToString`) and the shared placeholder parser. 1:1 component and prop parity with `@formepdf/react`, enforced by a cross-framework equivalence gate (a catalog document authored in Vue and React must serialize to the same document model). `serialize`/`render`/`renderToObject` for the hosted API, `renderDocument`/`renderDocumentWithLayout` for one-call local rendering via the optional `@formepdf/core` peer.
