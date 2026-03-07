# Changelog

All notable changes to the Forme monorepo are documented in this file.

## [0.7.0] - 2026-03-06

### Added
- `@formepdf/renderer` package for shared render pipeline (VS Code and future integrations)
- VS Code extension with native sidebar component tree, inspector panel, and hover-to-highlight
- VS Code extension activity bar icon and `forme.autoOpen` setting
- VS Code extension marketplace icon and improved discoverability

### Changed
- Shorter VS Code command titles ("Forme: Preview", "Forme: Preview to Side")

### Fixed
- CI: skip Arabic font fallback test when system font unavailable

## [0.6.2] - 2026-02-21

### Added
- Per-character font fallback for Arabic and CJK scripts
- `overflow: hidden` via PDF clip paths
- Canvas drawing primitive (`<Canvas>` component)
- Chart components: `<BarChart>`, `<LineChart>`, `<PieChart>`
- Watermarks with rotation and opacity
- SVG arc (`A`/`a`) path commands
- Justified text via PDF `Tw` operator
- PDF standard font `/Widths` arrays
- `lineBreaking` toggle
- Chart legend flex-wrap

### Fixed
- Cross-axis stretch propagation for flex layout
- Font weight fallback (opposite weight resolution)
- Shaping cluster byte-to-char conversion for multi-byte characters

## [0.6.1] - 2026-02-14

### Added
- Canvas clipping and arc counterclockwise parameter
- PDF bytes option for `sendPdf` in `@formepdf/resend`

## [0.6.0] - 2026-02-07

### Added
- `@formepdf/mcp` package for AI-powered PDF generation via MCP
- `@formepdf/resend` package for PDF + email via Resend
- `@formepdf/next` package for Next.js App Router route handlers
- `@formepdf/hono` package for Hono middleware (Workers, Deno, Bun, Node)
- CSS shorthands for border, padding, and margin (string and array formats)
- Alt text for images and SVGs
- Document language (`<Document lang="...">`)
- Clickable images and SVGs via `href` prop
- Knuth-Plass optimal line breaking
- UAX#14 Unicode line breaking
- Multi-language hyphenation via hypher (35+ languages)
- Tagged PDF / PDF/A-2a compliance
- Visual regression tests
- OpenType shaping via rustybuzz
- BiDi text support (unicode-bidi + unicode-script)
- CSS Grid layout (track sizing, auto/explicit placement)
- `repeat()` syntax for grid templates
- `textOverflow` (ellipsis/clip)
- Font fallback chains (comma-separated `fontFamily`)
- QR code generation with vector PDF rendering

## [0.4.4] - 2026-01-10

### Changed
- Version bump across packages

## [0.4.3] - 2026-01-03

### Fixed
- Keyboard shortcuts intercepting input in custom size fields
- Shipping label font and layout adjustments

## [0.4.2] - 2025-12-27

### Added
- Resolve HTTP/HTTPS image URLs to base64 data URIs before WASM render

## [0.4.1] - 2025-12-20

### Fixed
- Expose `pkg/` in `@formepdf/core` exports map for browser consumers

## [0.4.0] - 2025-12-13

### Added
- Template expression system for hosted API rendering
- Custom font registration API (`Font.register()` + `<Document fonts>` prop)

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- Page-native PDF rendering engine with real font metrics
- TrueType font embedding with CIDFont objects and subsetting
- `@formepdf/react` JSX-to-JSON serializer package
- `@formepdf/core` WASM build of the Rust engine
- `@formepdf/cli` with `forme dev` live preview and `forme build`
- Click-to-inspect dev tools with source jumping
- Component tree, data editor, and page size switcher
- Widow/orphan control, `align-content`, table cell overflow
- Bookmarks, internal anchor links, letter-spacing
- Absolute positioning, SVG module
- Style shorthand properties
- Background/border preservation on breakable views across page splits
- Nested flex layout, Fragment serialization, footer positioning, dynamic page numbers
