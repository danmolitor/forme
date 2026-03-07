# Changelog

## [0.7.0] - 2026-03-06

_No changes._

## [0.6.2] - 2026-02-21

_No changes._

## [0.6.1] - 2026-02-14

_No changes._

## [0.6.0] - 2026-02-07

### Added
- `renderTemplate()` and `renderTemplateWithLayout()` WASM bindings
- Font source resolution (file paths, data URIs, Uint8Array to base64)

## [0.4.2] - 2025-12-27

### Added
- Resolve HTTP/HTTPS image URLs to base64 data URIs before WASM render

## [0.4.1] - 2025-12-20

### Fixed
- Expose `pkg/` in exports map for browser consumers

## [0.4.0] - 2025-12-13

### Added
- `resolveFonts()` for base64 font encoding before WASM calls

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- WASM bridge: `renderDocument()`, `renderWithLayout()` JS API
- `wasm-pack` build pipeline with `wasm-opt`

### Changed
- Package scope renamed from `@forme/core` to `@formepdf/core`
