# Changelog

## [0.20.0] - 2026-09-05

### Changed

- Version alignment with the 0.20.0 release line; no functional changes in this package.

## [0.14.0] - 2026-08-28

### Changed

- Version alignment with the 0.14.0 release line; no functional changes in this package.

## [0.13.0] - 2026-08-27

_Version bump only — 0.13.0 fixes three `bookmark` defects in the engine (duplicate PDF outline entries, no layout marker on the fits path, a `nodeType: "None"` leak) and adds a per-line discount column to the invoice in `@formepdf/templates`. No changes to this package._

## [0.12.1] - 2026-08-26

_Version bump only — 0.12.1 fixes LayoutInfo/ElementInfo type declarations and adds `@formepdf/core/layout` accessor helpers. No changes to this package._

## [0.12.0] - 2026-08-25

Initial release.

### Added
- Preact 10 adapter for Forme. Same component set as `@formepdf/react` (`Document`, `Page`, `View`, `Text`, `H1`-`H6`, lists, inline formatting, tables, media, charts, form fields, layout primitives) with identical props and identical serialized output. Authored as ordinary `.tsx` files with Preact's JSX runtime.
- `serialize()`, `render()`, `renderToObject()` — same API as `@formepdf/react`
- Parity test suite: `.preact.tsx` + `.react.tsx` fixture pairs asserting byte-identical Forme JSON output between the two adapters

### Notes
- Compiled templates (the hosted-API expression system) work identically to the React adapter — the `template-proxy` recording layer is framework-agnostic
- Requires `preact ^10.19.0` as a peer dependency
- `@formepdf/core` interop is a runtime concern — the package doesn't depend on it directly; users install it separately if they need local rendering
