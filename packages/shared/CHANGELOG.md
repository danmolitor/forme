# Changelog

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
- Framework-neutral serialization core extracted from `@formepdf/react`: document-model (`Forme*`) types, `Style` mapping and CSS shorthand parsing, the `Font` registration store, the `Canvas` operation recorder, chart kind builders, and semantic-component constants (heading defaults, inline-formatting defaults, list marker mapping)
- `@formepdf/react` re-exports everything it previously exported, so its public API is unchanged
