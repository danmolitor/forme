# Changelog

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

### Fixed
- Next.js (Webpack + Turbopack) builds no longer fail with `Module not found: Can't resolve './forme_bg.js'`. Root cause was `@formepdf/core` shipping a `--target web` WASM build whose binary referenced `./forme_bg.js`, which the build didn't emit. `@formepdf/core@0.10.0` now ships both a bundler-target and a nodejs-target build, and `@formepdf/next` consumes the bundler-target so Webpack and Turbopack can statically resolve the WASM imports.

### Changed
- Dropped the `import('@formepdf/core/pkg/forme_bg.wasm')` + `browser.init(wasm)` workaround that was previously required to work around the WASM resolution. The bundler-target build makes it unnecessary

## [0.9.2] - 2026-04-28

_Version bump only._

## [0.9.1] - 2026-04-06

### Fixed
- React Compiler compatibility: `renderPdf()` and `pdfResponse()` now detect compiled render callbacks and throw a clear error pointing users to add `'use no memo'`, instead of a cryptic "Invalid hook call".

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

### Fixed
- Edge runtime crash: detection now uses `import.meta.url` instead of `process.versions.node`, fixing environments where `nodejs_compat` polyfills process globals ([#1](https://github.com/danmolitor/forme/issues/1))

### Changed
- Templates imported from shared `@formepdf/templates` package

## [0.7.10] - 2026-03-18

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
- Initial release: PDF route handlers for Next.js App Router
