# Changelog

## [0.15.0]

### Changed (behavior — read the migration note)

- **`position: absolute` now resolves against the nearest positioned ancestor, not the direct parent.** This retires a v0 divergence and matches every browser.

  **The rule.** An absolute element's containing block is its nearest ancestor with `position: relative` or `absolute`. If no ancestor is positioned, it resolves against the page content box. Previously it always resolved against its direct parent, positioned or not.

  **Detection recipe.** If you use `position: 'absolute'` inside a container that has **no** `position` set, add `position: 'relative'` to the intended container. Otherwise the absolute box now resolves against the page (or a higher positioned ancestor) instead of that parent — most visibly with negative offsets, which will push content off the page.

  **Worked example (from this repo).** The `catalog` demo template's "SALE"/"NEW" badge is `position: 'absolute'; top: -18; right: -18` — negative offsets meant to overhang a product card's corner. The card had no `position`, so under the old rule the badge sat on the card; under the new rule it escaped to the page and overflowed the content box by 16pt. Our own structural-regression gate caught it before it shipped. The fix was one line — `position: 'relative'` on the card — which is exactly the migration. (`templates/catalog.tsx`.)

  **How to find affected templates.** Any absolute-positioned element whose parent lacks `position` is a candidate — grep for `position: 'absolute'` and check each ancestor. If you use `@pdf-testkit` structural snapshots, your own baselines will tell you, exactly as ours did here; templates and docs snippets without baselines need the manual grep.

## [0.14.0] - 2026-08-28

### Added

- **`Table` wrapper element in layout output.** `layout_table` emits a `Table` container per page fragment (clone semantics, like breakable Views), so table-level `border`/`background` finally have a paint target and structural consumers (tagged PDF `/Table`, downstream extractors) get a real table node instead of loose rows.
- **`vertical-align` for table cells.** New `VerticalAlign` on `Style` (top/middle/bottom, default top): cell content offsets within the resolved row box.
- **`@page :first` support.** `Document.first_page` gives page one its own `PageConfig`; `PageCursor` tracks a page index; fixed elements carry a `FixedPageFilter` (All/First/NotFirst) honored at cursor-spacing time and authoritatively at injection by the real page index.
- **Block-level min/max constraints.** `max_width`/`min_width`/`min_height` now clamp in `layout_view`, height measurement, and the auto-margin centering branch — auto width + finite max-width is the centered-column idiom (block fills, clamp shrinks, auto margins split the rest).

### Fixed

- **Runs-based text measured zero intrinsic width.** `measure_intrinsic_width` ignored `runs` (and measured leaf `Heading`s as 0, and whole multi-line strings instead of the widest line); flex rows collapsed such text to one character per line.
- **`col_span` was ignored when indexing column widths.** Every cell after a colspan cell sat one column too far left; the spanning cell now consumes its columns' combined width in both layout and row-height measurement.
- **`wrap: false` on tables was silently ignored** (break-inside: avoid): row-by-row pagination never consulted breakability. An unbreakable table that fits a fresh page now moves there whole.
- **A flex line taller than the page emitted a blank leading page** before overflowing anyway; the break check now skips when the current page is empty.

## [0.13.0] - 2026-08-27

### Fixed

- **A bookmarked container that both overflows a page and carries visual styling no longer writes its outline entry twice.** A genuine document defect, not a reporting one: the PDF shipped two identical `/Outlines` entries and `/Count 2` for a single `bookmark` prop. The overflow path emits a zero-height marker element so an unstyled view can't lose its bookmark; when the view *is* styled it also builds a wrapper, the marker drains into that wrapper's children, and `collect_bookmarks` — which recurses — found the same bookmark twice. Wrappers no longer carry `bookmark`; the marker is the sole carrier on every path.

- **`bookmark` on a container that fits its page now emits a marker element too.** Previously only the overflow path produced one, so anything walking `render_with_layout` output for bookmarked containers missed most of them. PDF output is unaffected — `collect_bookmarks` reads the `bookmark` field and ignores node type, so the outline itself was always complete.

- **The bookmark marker reports `nodeType: "Bookmark"` instead of `"None"`.** With the node type left unset, the layout serializer fell back to `kind.to_string()` and leaked the `DrawCommand::None` variant name into the JSON.

### Changed

- **Outline destination for one case.** The marker now sits at the view's outer top edge rather than its padding/border-inset content top, so every container path resolves a bookmark to the same coordinate. Only an *unstyled, overflowing, breakable* view with top padding or border moves (Letter, 36pt margin, padding 20: destination Y 736.00 → 756.00). Styled views were already governed by a first-match entry at 756.00 and are unchanged.

### Internal

- Both container paths now build the marker through one shared `bookmark_marker()` helper rather than two hand-maintained copies — the divergence that produced all three bugs above.
- `test_styled_breakable_bookmark_emits_exactly_one_outline_entry` asserts outline *counts*, not presence. The pre-existing bookmark tests all used `contains`, which a duplicate entry satisfies perfectly well, which is why the double emission went unnoticed.

## [0.12.1] - 2026-08-26

_Version bump only — no engine changes. Aligned with the 0.12.1 monorepo release (LayoutInfo/ElementInfo type-declaration fixes and new `@formepdf/core/layout` accessor helpers in `@formepdf/core`)._

## [0.12.0] - 2026-08-25

_Version bump only — no engine changes. Aligned with the 0.12.0 monorepo release (new `@formepdf/preact` npm package)._

## [0.11.1] - 2026-08-20

### Fixed
- `<Svg>` content now honors `stroke-linecap` and `stroke-linejoin`. The SVG parser was silently dropping both attributes on every element — `SvgCommand::SetLineCap` / `SvgCommand::SetLineJoin` only ever fired from the Canvas API, never from SVG content, so every SVG stroke rendered with the PDF default (butt caps / miter joins) regardless of the source attribute. `stroke-linecap="round"` on a signature composed of many short cubic bezier paths now blends the segment endpoints into round semicircles instead of leaving visible flat rectangular caps. Reported against a real handwritten-signature repro
- SVG attribute values inherit through `<g>` group ancestors on the same stack as `fill` / `stroke` / `stroke-width` / `opacity`. `SetLineCap` / `SetLineJoin` are emitted unconditionally inside every shape's `q/Q` wrapper so an enclosing `<g stroke-linecap="round">` can't leak state to a subsequent shape

### Internal
- Four new integration tests decompress the FlateDecode content streams and assert on the PDF operators: `1 J` for round caps, `0 J` for default butt, `1 j` for round joins, plus a `<g>` inheritance case. All verified to fail without the fix. Full engine suite now 214 unit + 299 integration

## [0.11.0] - 2026-08-09

_Version bump only — no engine changes. Aligned with the 0.11.0 monorepo release (new `@formepdf/shared` + `@formepdf/svelte` npm packages)._

## [0.10.5] - 2026-06-29

### Fixed
- `<Table>` with `<Row header>` no longer orphans the header at the bottom of a page when the header alone fits in the remaining space but the first body row does not. Closes the GitHub orphan-header bug. The 0.10.4 pre-fit check gated only on `total_header_h > remaining_height`; the body row could still overflow afterward, leaving an orphaned header that re-emitted on the next page. The check now folds in the first body row's measured height (`needed = total_header_h + first_body_h`), capped at fresh-page available height so the existing `!is_header` cell-overflow guard in `layout_table_row` still handles the rare case where the combined block is genuinely taller than a page. Also fixes the long-token-header contamination edge case where a wrapped multi-line header rendered 3 pages with header text leaking onto a prior page

### Internal
- Two new integration tests: `test_table_header_no_orphan_when_first_body_row_doesnt_fit` and `test_table_long_header_text_no_page_contamination`. Both verified to fail without the fix and pass with it

## [0.10.4] - 2026-06-05

### Fixed
- `<Table>` with `<Row header>` no longer inflates page count 3–5× when starting low enough on a page that the header doesn't fit before the page break. The header loop had no pre-fit check (unlike the body loop), so when forced to lay out where it didn't fit, each header cell's inner View/Text triggered a widow/orphan page-break that `layout_table_row` then captured as a "trial" snapshot page. Each successive cell's snapshot accumulated one more cell of the in-progress row — the reporter's "doubled header sliding one column right per page" symptom. `layout_table` now page-breaks before laying out headers if they don't fit; header rows additionally drop any cell-overflow trial pages instead of committing them
- A `<View>` wrapping a `<Table>` no longer inflates to roughly the page height. `measure_node_height` had no arms for `NodeKind::Table` or `TableRow`, so they fell through to the generic column-summing branch — a 3-column row of 16pt cells measured to 48pt instead of 16pt, and the wrapping View inherited the inflated value. Now delegates to the same `resolve_column_widths` + `measure_table_row_height` helpers `layout_table` already uses, so measurement matches what gets rendered
- `<Svg width={W} height={H} viewBox="x y w h">` now scales content to fit the display box. `parse_svg`'s viewBox parameters were unused and the PDF emission's `cm` transform scaled by `element.width / display_width` (always 1.0), so paths rendered at raw viewBox coordinates and overflowed. PDF emission now implements the SVG viewport algorithm with `xMidYMid meet` as the default `preserveAspectRatio` (uniform `min(sx, sy)` scale + centering)
- `marginTop` / `marginBottom: 'auto'` on a child in a column-flex parent with fixed height now pushes the child to the bottom / centers it / etc., matching CSS flex spec. The flex-row cross-axis already had auto-margin slack handling; the column branch of `layout_children` did not. Mirrors that block, ordered before `justify-content` so auto-margins consume free space first

### Internal
- New `measure_node_height` arms for `NodeKind::Table` and `TableRow`, plus an auto-vertical-margin pass in `layout_children`'s column branch. SVG viewBox dimensions are plumbed through `DrawCommand::Svg`

## [0.10.3] - 2026-05-28

### Fixed
- `<Text style={{ width }}>` inside a flex row now renders at the requested width instead of the parent row's full width. A 0.10.2 regression positioned such text using the requested width but sized its box to the row width; combined with `textAlign: 'right'` this pushed glyphs off the page (silent corruption — PDF bytes were deterministic so byte-hash snapshots passed). `layout_text` and the text branch of `measure_node_height` now honor a resolved fixed `style.width`, matching how `layout_view` and `Image` already behave

## [0.10.2] - 2026-05-21

### Fixed
- Flex row children with percentage widths (e.g. `width: '30%'`) are no longer double-resolved against their own already-resolved width — the percentage now correctly resolves against the parent's content width. Two children at 100% now shrink to 50/50 instead of collapsing
- Grid containers that wrap over a page break now page-break by row (all columns moved to the next page together) instead of letting each cell trigger its own page break, which was scattering the columns across separate pages

### Internal
- `layout_node` now accepts a `forced_outer_width: Option<f64>` parameter so flex parents can hand the distributed width to the child without re-running style resolution

## [0.10.1]

_Skipped — npm-only patch._

## [0.10.0] - 2026-05-19

### Added
- `opacity` now cascades to children — wrapping happens at the element level (single `q ... Q` covering own paint and child recursion) instead of the previous per-Rect wrap that left text inside opaque parents at full alpha. Nested opacities multiply via the PDF graphics-state stack
- `wordSpacing` style property — user-facing, emits the PDF `Tw` operator. Stacks with `text-align: justify`'s computed slack
- Rounded clipping when `overflow: hidden` + `borderRadius` — clip path uses the rounded rectangle (m/c/h W n) instead of the rectangular `re W n`
- `boxShadow` style property — offset filled rect behind the element, honors borderRadius, alpha routed through ExtGState
- Page `backgroundImage` with `backgroundOpacity` / `backgroundSize` (fill/cover/contain) / `backgroundPosition`. XObjects dedupe across pages by URL
- `background` style property accepting CSS linear and radial gradients. 2-stop gradients use a Type 2 (exponential) Shading; 3+ stops use a Type 3 (stitching) function. CSS angle convention (0deg = bottom→top, 180deg = top→bottom)

### Internal
- CI now runs a PDF size regression check (`.github/scripts/check-pdf-size.sh`) against a fixture that exercises all six new features. Fails on >5% byte growth over the committed baseline

## [0.9.2] - 2026-04-28

### Fixed
- Redaction text-stripping now uses real per-CID glyph advances when locating regions, so partial-line redactions match the visible overlay precisely instead of dropping the surrounding text
- CID font decoding in the redaction text extractor — handles Type0/CIDFontType2 streams correctly
- PDF parsing in CID redaction is robust to binary font streams that previously confused the tokenizer
- `text_decoration` is now part of the glyph style grouping key, so a `line-through` span inside an otherwise plain text node is no longer merged with its neighbors during PDF emission

## [0.9.1] - 2026-04-06

_Version bump only._

## [0.9.0] - 2026-04-04

### Added
- `certify_pdf()` — apply a PKCS#7 digital signature to an existing PDF
- `redact_pdf()` — redact regions from a PDF (removes underlying text)
- `redact_text()` — find text by pattern and redact matching regions
- `find_text_regions()` — find text regions in a PDF without redacting
- `merge_pdfs()` — combine multiple PDFs into one
- PKCS#1 private key auto-conversion: `parse_pem_private_key()` now accepts both PKCS#8 (`BEGIN PRIVATE KEY`) and PKCS#1 (`BEGIN RSA PRIVATE KEY`) formats, falling back automatically

### Fixed
- WASI timestamp: `current_timestamp_secs()` now uses `std::time::SystemTime` on wasm32-wasip1 targets instead of `js_sys::Date` (which is only available in browser WASM)

## [0.8.3] - 2026-04-01

### Added
- SVG element opacity support: `opacity`, `fill-opacity`, and `stroke-opacity` attributes rendered via PDF ExtGState with inheritance through `<g>` groups

### Fixed
- Page node style now resolves against root style, so properties like `fontFamily` set on `<Page style={...}>` correctly inherit to child nodes

## [0.8.2] - 2026-03-30

### Fixed
- Fix PDF serializer ignoring custom font weights — multiple weights for the same family (e.g. 200, 400, 700) now produce distinct font objects instead of collapsing to 400/700

## [0.8.1] - 2026-03-30

### Fixed
- Fix Latin Extended character widths in standard font tables (Å, Ä, Ö, etc. no longer stack)
- Fix page number placeholder width mismatch during layout ({{pageNumber}} measured at actual width)
- Two-pass rendering: sentinel width now matches actual page count digits (1-9→"0", 10-99→"00", 100+→"000")

## [0.8.0] - 2026-03-29

### Added
- AcroForm support: `NodeKind::TextField`, `NodeKind::Checkbox`, `NodeKind::Dropdown`, `NodeKind::RadioButton`
- Form field layout functions and PDF AcroForm widget rendering
- `flattenForms` option in render pipeline to convert form fields to static content
- PDF/UA-1 compliance: structure tree (`StructTreeRoot`), tab order, role map, artifact tagging for headers/footers
- `pdf/tagged.rs`: structure tree generation for tagged PDF
- PDF/A compliance: sRGB output intent, XMP metadata with `pdfaid:part/conformance`, full font embedding mode
- `pdf/signing.rs`: PKCS#7 detached digital signatures with ByteRange placeholder
- `Document.pdf_ua`, `Document.pdfa`, `Document.signature` fields
- `Image.alt` and `Svg.alt` emit `/Alt` entries in structure elements for PDF/UA

### Fixed
- `extract_cn_from_cert_der()` handles multi-byte DER lengths (CN > 127 bytes)
- Checkbox appearance: checkmark instead of X; radio button: filled circle
- `flatten_forms` renders placeholder text in grey when value is empty
- Signing preserves existing AcroForm `/NeedAppearances` and `/DA` metadata
- Unique signature field names (`Signature1`, `Signature2`) for double-signing
- Form fields tagged as `/Form` role in structure tree (PDF/UA compliance)

### Removed
- `SignatureConfig.page` field (was accepted but silently ignored)

## [0.7.13] - 2026-03-28

### Added
- `chart/` module with shared types (`ChartPrimitive`, `TextAnchor`) and per-type builders (`bar.rs`, `line.rs`, `pie.rs`, `area.rs`, `dot.rs`)
- Five `NodeKind` variants: `BarChart`, `LineChart`, `PieChart`, `AreaChart`, `DotPlot`
- `DrawCommand::Chart { primitives }` with PDF rendering (Y-flip transform, arc sector bezier approximation, Helvetica labels)
- `ChartDataPoint`, `ChartSeries`, `DotPlotGroup` data structs
- 10 integration tests for chart rendering

## [0.7.9] - 2026-03-17

_Version bump only._

## [0.7.8] - 2026-03-17

### Added
- `barcode.rs`: 1D barcode generation via `barcoders` crate (Code128, Code39, EAN13, EAN8, Codabar)
- `NodeKind::Barcode` variant with `data`, `format`, `width`, `height` fields
- `layout_barcode()` function (follows `layout_qrcode` pattern)
- `DrawCommand::Barcode` with filled rectangle emission in PDF serializer
- `wasm_raw` feature with C-ABI exports for non-JS WASM hosts (wasmtime, wasmer)

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
