# Changelog

## [0.7.6] - 2026-03-13

### Added
- `Document.embedded_data` field for embedding JSON as a FlateDecode-compressed PDF file attachment
- PDF serializer emits EmbeddedFile stream + Names tree for `forme-data.json`

## [0.7.3] - 2026-03-07

_No changes._

## [0.7.2] - 2026-03-07

_No changes._

## [0.7.1] - 2026-03-07

### Added
- Builtin Noto Sans Regular (400) and Bold (700) fonts via `include_bytes!()` (`font/builtin.rs`)
- `Document.default_style` field for global style defaults (inherited by all children)
- Automatic per-character font fallback to Noto Sans for chars not covered by the primary font

### Changed
- `FontRegistry::new()` now registers Noto Sans alongside standard PDF fonts
- `resolve_for_char()` tries Noto Sans before Helvetica as last-resort fallback
- `segment_by_font()` checks glyph coverage even for single-family text
- `char_width()` uses per-char resolution when primary font lacks a glyph

## [0.7.0] - 2026-03-06

### Fixed
- Skip Arabic font fallback test when system font unavailable (CI fix)

## [0.6.2] - 2026-02-21

### Added
- Per-character font fallback (`font/fallback.rs`, `segment_by_font`)
- `overflow: hidden` via PDF clip path operators (`q / re W n / Q`)
- Canvas drawing primitive (`CanvasOp` enum, reuses SVG command pipeline)
- SVG arc (`A`/`a`) path commands (`svg_arc_to_curves`, W3C F.6.5/F.6.6)
- Watermarks with rotation matrix and opacity in PDF output
- Justified text via PDF `Tw` (word spacing) operator
- PDF standard font `/Widths` arrays for Helvetica, Times, Courier
- `lineBreaking` toggle

### Fixed
- Cross-axis stretch propagation (`cross_axis_height` parameter in `layout_node`)
- Font weight fallback with opposite weight resolution (700 to 400 and vice versa)
- Shaping cluster byte-to-char conversion for multi-byte characters
- `measure_intrinsic_width` accounts for `textTransform`

## [0.6.1] - 2026-02-14

### Added
- Canvas clipping to bounds via `DrawCommand::Svg { clip: true }`
- Arc counterclockwise parameter support

## [0.6.0] - 2026-02-07

### Added
- Knuth-Plass optimal line breaking algorithm
- UAX#14 Unicode line breaking
- Multi-language hyphenation via hypher crate (35+ languages)
- OpenType shaping via rustybuzz
- BiDi text support (unicode-bidi + unicode-script)
- CSS Grid layout (track sizing, auto/explicit placement)
- Tagged PDF / PDF/A-2a compliance with structure tree
- Visual regression test framework
- QR code generation (`qrcode.rs`, vector PDF rendering)
- `textOverflow` (ellipsis/clip) truncation
- Font fallback chains (comma-separated `fontFamily` resolution)
- Alt text field on `LayoutElement`
- Document language (`/Lang` in PDF Catalog)
- Clickable images/SVGs via `href`

## [0.4.0] - 2025-12-13

### Added
- Template expression evaluator (`template.rs`)
- Custom font registration and base64 font loading
- Font subsetting for embedded custom fonts

## [0.1.0 - 0.3.0] - Pre-releases

### Added
- Page-native layout engine with `PageCursor`
- PDF 1.7 serializer (from scratch)
- TrueType font embedding with CIDFont objects and subsetting
- Standard font metrics (Helvetica, Times, Courier) with WinAnsi mapping
- Flex layout (row/column, grow/shrink/wrap)
- Table layout with header repetition across pages
- Image loading (JPEG, PNG, WebP, data URIs)
- SVG parsing and rendering
- Widow/orphan control
- `align-content` for flex wrap
- Table cell overflow preservation
- Bookmarks and internal anchor links
- Letter-spacing
- Absolute positioning
- Fixed height containers
- Background/border on breakable views across page splits
