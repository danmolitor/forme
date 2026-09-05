# Changelog

## [0.20.0] - 2026-09-05

### Changed

- Version alignment with the 0.20.0 release line; no functional changes in this package.

## [0.15.0] - Unreleased

### Added

- Initial release of `@formepdf/fonts-standard`: embeddable, metric-compatible replacements for the 14 standard PDF fonts (Liberation Sans/Serif/Mono, Regular/Bold/Italic/BoldItalic, SIL OFL 1.1). `standardFonts()` returns the 12 fonts as registrations with `Uint8Array` buffers (no file IO); `BASE14_ALIASES` maps base-14 families to their Liberation equivalents. Enables PDF/UA and PDF/A output, where every font must be embedded.
