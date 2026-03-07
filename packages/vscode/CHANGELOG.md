# Changelog

## [0.7.2] - 2026-03-07

### Added
- Data tab in sidebar - edit companion JSON data and see the preview update live
- Live preview updates as you type (uses editor buffer, no longer requires save)
- Local image file paths in templates now resolve to base64 data URIs

### Fixed
- Cross-platform VSIX support: switched from platform-specific esbuild to esbuild-wasm

### Changed
- Improved README: added requirements, quick start with example, use cases, comparisons, and React rationale

## [0.7.1] - 2026-03-06

### Fixed
- Component tree showing "No layout data" on marketplace installs (webview ready handshake)
- Preview panel stealing focus when clicking back into the TSX editor with auto-open enabled

## [0.7.0] - 2026-03-06

### Added
- Initial release: VS Code extension for Forme PDF preview
- Native sidebar component tree with hover-to-highlight
- Native sidebar inspector panel (box model, computed styles, Open in Editor, Copy Style)
- Forme activity bar icon
- `forme.autoOpen` setting for auto-preview on file open
- Marketplace icon and improved discoverability (keywords, description)

### Changed
- Shorter command titles ("Forme: Preview", "Forme: Preview to Side")
