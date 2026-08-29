# Changelog

## [0.14.0] - 2026-08-28

### Changed

- Version alignment with the 0.14.0 release line; no functional changes in this package.

## [0.13.0] - 2026-08-27

_Version bump only — 0.13.0 fixes three `bookmark` defects in the engine (duplicate PDF outline entries, no layout marker on the fits path, a `nodeType: "None"` leak) and adds a per-line discount column to the invoice in `@formepdf/templates`. No changes to this package._

## [0.12.1] - 2026-08-26

_Version bump only — 0.12.1 fixes LayoutInfo/ElementInfo type declarations and adds `@formepdf/core/layout` accessor helpers. No changes to this package._

## [0.12.0] - 2026-08-25

_Version bump only — 0.12.0 introduces the new `@formepdf/preact` adapter (Preact 10 authoring). No changes to this package._

## [0.11.1] - 2026-08-20

_Version bump only — engine 0.11.1 fixes SVG `stroke-linecap` / `stroke-linejoin` via `@formepdf/core`._

## [0.11.0] - 2026-08-09

_Version bump only — aligned with the 0.11.0 monorepo release (see `@formepdf/shared` and `@formepdf/svelte` new packages, `@formepdf/react` internal refactor)._

## [0.10.5] - 2026-06-29

_Version bump only — engine 0.10.5 fixes table header page-break orphan + long-header contamination via `@formepdf/core`._

## [0.10.4] - 2026-06-05

_Bump `@formepdf/core` to 0.10.4 — picks up four engine layout fixes (Table repeating-header page-count, View-around-Table auto-height, SVG viewBox scaling, column-axis `marginTop: 'auto'`)._

## [0.10.3] - 2026-05-28

_Bump `@formepdf/core` to 0.10.3 — picks up the engine fix for `<Text style={{ width }}>` rendering at the parent's full width in a flex row._

## [0.10.2] - 2026-05-21

_Bump `@formepdf/core` to 0.10.2 — picks up engine flex row percentage-width and grid page-break layout fixes._

## [0.10.1] - 2026-05-20

_Bump `@formepdf/core` to 0.10.1 — picks up the Cloudflare Workers init crash fix and the missing `pkg-node/` tarball fix._

## [0.10.0] - 2026-05-19

_Dependency bump only._


## [0.9.2] - 2026-04-28

_Version bump only._

## [0.9.1] - 2026-04-06

_Dependency bump only._

## [0.9.0] - 2026-04-04

_Dependency bump only._

## [0.8.3] - 2026-04-01

_Dependency bump only._

## [0.8.2] - 2026-03-30

_Dependency bump only._

## [0.8.1] - 2026-03-30

### Changed
- Version bump to match engine 0.8.1

## [0.8.0] - 2026-03-29

_Dependency bump only._

## [0.7.13] - 2026-03-28

_Dependency bump only._

## [0.7.12] - 2026-03-24

_Dependency bump only._

## [0.7.11] - 2026-03-23

_Dependency bump only._

## [0.7.9] - 2026-03-17

_Dependency bump only._

## [0.7.8] - 2026-03-17

_Dependency bump only._

## [0.7.3] - 2026-03-07

_No changes._

## [0.7.2] - 2026-03-07

_No changes._

## [0.7.1] - 2026-03-07

_No changes._

## [0.7.0] - 2026-03-06

_No changes._

## [0.6.2] - 2026-02-21

_No changes._

## [0.6.1] - 2026-02-14

_No changes._

## [0.6.0] - 2026-02-07

### Added
- `forme build --template` for template compilation (TSX to JSON)
- Font path resolution relative to template directory

## [0.4.3] - 2026-01-03

### Fixed
- Keyboard shortcuts intercepting input in custom size fields

## [0.4.0] - 2025-12-13

### Added
- Font path resolution in dev server for custom fonts

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- `forme dev` live preview dev server with hot reload
- `forme build` CLI command
- Browser preview UI with PDF and layout endpoints
- Click-to-inspect dev tools
- Click-to-source: jump from inspector to IDE
- Component tree sidebar
- Data editor panel
- Page size switcher
- Inspector breadcrumb and copy style button
- Link info in inspector
- `--data` flag for external data files

### Changed
- Package scope renamed from `@forme/cli` to `@formepdf/cli`
