# Changelog

## [0.11.1] - 2026-08-20

_Version bump only — engine 0.11.1 fixes SVG `stroke-linecap` / `stroke-linejoin` via `@formepdf/core`._

## [0.11.0] - 2026-08-09

Initial release.

### Added
- Framework-neutral serialization core extracted from `@formepdf/react`: document-model (`Forme*`) types, `Style` mapping and CSS shorthand parsing, the `Font` registration store, the `Canvas` operation recorder, chart kind builders, and semantic-component constants (heading defaults, inline-formatting defaults, list marker mapping)
- `@formepdf/react` re-exports everything it previously exported, so its public API is unchanged
