# Changelog

## [0.20.0] - 2026-09-05

### Changed

- Version alignment with the 0.20.0 release line; no functional changes in this package.

## [0.15.0] - Unreleased

### Added

- Initial release of `@formepdf/vue`: the Vue 3 adapter for Forme. Author documents as `.vue` single-file components using the full Forme component set, rendered via Vue SSR (`renderToString`) and the shared placeholder parser. 1:1 component and prop parity with `@formepdf/react`, enforced by a cross-framework equivalence gate (a catalog document authored in Vue and React must serialize to the same document model). `serialize`/`render`/`renderToObject` for the hosted API, `renderDocument`/`renderDocumentWithLayout` for one-call local rendering via the optional `@formepdf/core` peer.
