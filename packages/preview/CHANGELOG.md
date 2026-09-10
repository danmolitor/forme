# Changelog — @formepdf/preview

## [0.22.0] - 2026-09-10

### Changed

- **Joined the shared release line.** `@formepdf/preview` was born at 0.21.0/0.21.1 out-of-band; its `@formepdf/html` and `@formepdf/fonts-standard` dependencies are caret ranges, which on a 0.x line cannot resolve the next minor — so it now bumps with everything else (`scripts/bump-version.sh` handles it).

## [0.21.1] - 2026-09-09

### Fixed

- Guard `npm publish` with a build step (`prepublishOnly`).

## [0.21.0] - 2026-09-09

- Initial release.
