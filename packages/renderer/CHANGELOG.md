# Changelog

## [0.15.0] - Unreleased

### Added

- **Svelte and Vue input paths**: `renderSvelteFromFile`/`renderSvelteFromSource` and `renderVueFromFile`/`renderVueFromSource` compile `.svelte`/`.vue` single-file components, serialize them through their adapter, and return the same `RenderResult` (PDF + `LayoutInfo` + `warnings: []`) as the JSX and HTML paths — so preview surfaces light up unchanged. The SFC compiler (`svelte/compiler`, `@vue/compiler-sfc`) and framework runtime resolve from the *user's* workspace, mirroring how the JSX path externalizes react/@formepdf/\*; nothing framework-specific is bundled.
- **Preact reconciler support on the JSX path**: `bundleFile`/`bundleSource` gain a `flavor` (`react` | `preact`), detected from the `@formepdf/preact` import signature — `jsxImportSource: 'preact'` at bundle, preact's `serialize`/`isValidElement` at render. A `.tsx` Preact template now renders through the same dispatch instead of failing on react's `isValidElement`.
- **Named workspace-dependency errors** (`friendlyDependencyError`): a missing framework compiler or runtime surfaces as "\"vue\" is not installed in this workspace. Run `npm install vue`…" rather than a module-resolution stack trace, across all input paths.

### Changed

- Extracted the shared render tail (`renderDocToResult`: page-size override → asset resolution → WASM render) so every input path converges on one `RenderResult` shape. Temp render modules now use a collision-proof name (previously `Date.now()` alone, which collided under concurrent renders).

## [0.14.0] - 2026-08-28

### Added

- **HTML input path**: `renderHtmlFromFile` / `renderHtmlFromSource` render `.html` sources through `@formepdf/html` (WASM), returning the same shape as the JSX pipeline — PDF bytes + `LayoutInfo` + subset warnings — so preview surfaces light up unchanged. The dual-mode preview HTML gains HTML-input support.

### Changed

- New runtime dependency on `@formepdf/html`, pinned to the shared version line (a skew here would pair the renderer with a mismatched engine build).

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

### Fixed
- Image path resolution: `basePath` was not passed to `resolveImageSources` in compiled output

## [0.7.2] - 2026-03-07

_No changes._

## [0.7.1] - 2026-03-07

_No changes._

## [0.7.0] - 2026-03-06

### Added
- Initial release: shared render pipeline for VS Code extension and future integrations
