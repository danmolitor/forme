# CLAUDE.md

## What This Is

Forme is a **page-native PDF rendering engine** written in Rust. It takes a tree of document nodes (like a simplified DOM) and produces PDF bytes. The key differentiator is that layout happens INTO pages rather than on an infinite canvas that gets sliced afterward. This means page breaks, table header repetition, and flex layout across pages all work correctly.

## Project Structure

```
forme/
├── CLAUDE.md               # You are here
├── README.md               # Product readme
├── engine/                 # Rust rendering engine
│   ├── Cargo.toml          # Deps: serde, serde_json, miniz_oxide, ttf-parser, qrcode
│   ├── src/
│   │   ├── lib.rs          # Public API: render(), render_json(), render_with_layout()
│   │   ├── main.rs         # CLI binary + example invoice JSON
│   │   ├── model/mod.rs    # Document tree: Node, NodeKind, PageConfig, Edges
│   │   ├── style/mod.rs    # CSS-like styles, resolution with inheritance
│   │   ├── layout/
│   │   │   ├── mod.rs      # THE CORE: page-aware layout engine + element nesting
│   │   │   ├── flex.rs     # Flex grow/shrink/wrap distribution helpers
│   │   │   ├── grid.rs     # CSS Grid track resolution + item placement
│   │   │   └── page_break.rs # Break decision logic (split/move/place)
│   │   ├── text/
│   │   │   ├── mod.rs      # Line breaking + text measurement
│   │   │   ├── bidi.rs     # BiDi analysis (UAX#9) + direction detection
│   │   │   ├── knuth_plass.rs # Optimal line breaking algorithm
│   │   │   └── shaping.rs  # OpenType shaping via rustybuzz
│   │   ├── font/
│   │   │   ├── mod.rs      # Font registry, resolution, FontContext
│   │   │   ├── fallback.rs # Per-character font fallback segmentation
│   │   │   ├── metrics.rs  # Standard font metrics + WinAnsi mapping
│   │   │   └── subset.rs   # TrueType font subsetting for PDF embedding
│   │   ├── image_loader/   # JPEG/PNG/WebP decoding from file paths and data URIs
│   │   ├── svg/mod.rs      # SVG parsing and rendering (rect, circle, line, path, arc)
│   │   ├── qrcode.rs       # QR code generation (qrcode crate → bool matrix)
│   │   ├── template.rs     # Expression evaluator for template system
│   │   └── pdf/
│   │       ├── mod.rs      # PDF 1.7 serializer (from scratch)
│   │       ├── tagged.rs   # Tagged PDF / PDF/A-2a structure tree
│   │       └── xmp.rs      # XMP metadata for PDF/A compliance
│   └── tests/
│       ├── integration.rs  # Full pipeline tests (~170 tests)
│       └── visual_regression.rs # Pixel-level reference image tests
├── templates/              # Example templates for testing + demos
│   ├── grid-dashboard.tsx  # Multi-feature showcase (grid, charts, i18n, RTL)
│   └── grid-dashboard-data.json
└── packages/
    ├── react/              # JSX component library: <Document>, <Page>, <View>, etc.
    │   └── src/
    │       ├── index.ts    # Public exports
    │       ├── components.tsx # Component definitions
    │       ├── charts.tsx  # BarChart, LineChart, PieChart
    │       ├── font.ts     # Font.register() static API + global font store
    │       ├── serialize.ts # JSX → JSON document tree + font merging
    │       ├── template-proxy.ts # Recording proxy for template compilation
    │       └── expr.ts     # Expression helpers for templates
    ├── core/               # WASM bridge: compiles engine to WebAssembly
    │   ├── src/index.ts    # JS API: renderDocument(), renderTemplate(), etc.
    │   └── build.sh        # wasm-pack build + wasm-opt
    ├── cli/                # `forme dev` and `forme build` commands
    │   ├── src/dev.ts      # Dev server with live reload, PDF + layout endpoints
    │   ├── src/preview/    # Browser UI: preview, overlays, click-to-inspect
    │   └── src/template-build.ts # Template compilation (TSX → JSON)
    ├── renderer/           # Shared render pipeline for CLI, VS Code, future integrations
    │   └── src/
    │       ├── index.ts    # Public exports
    │       ├── render.ts   # Render pipeline (TSX → JSON → WASM → PDF/layout)
    │       ├── bundle.ts   # esbuild bundling
    │       ├── resolve.ts  # Font/image resolution (file paths → base64)
    │       ├── element.ts  # Element types for layout overlay
    │       └── preview/
    │           └── index.html # Preview HTML (dual mode: CLI and VS Code)
    ├── vscode/             # VS Code extension for live PDF preview
    │   ├── src/
    │   │   ├── extension.ts           # Activation, wiring store/providers
    │   │   ├── preview-panel.ts       # Webview panel for PDF rendering
    │   │   ├── layout-store.ts        # Event-emitting store decoupling preview/tree/inspector
    │   │   ├── component-tree-provider.ts # Sidebar component tree with hover-to-highlight
    │   │   └── inspector-view-provider.ts # Sidebar inspector: box model, computed styles, Open in Editor
    │   └── resources/
    │       ├── forme-icon.svg         # Activity bar icon
    │       └── icon.png               # Marketplace icon
    ├── hono/               # PDF middleware for Hono (Workers, Deno, Bun, Node)
    ├── next/               # PDF route handlers for Next.js App Router
    ├── resend/             # Render PDF + email via Resend in one call
    └── mcp/                # MCP server for AI-powered PDF generation
```

### Renderer Package (`@formepdf/renderer`)
Shared render pipeline extracted from the CLI dev server so that VS Code (and future integrations) reuse the same bundling, font/image resolution, and WASM rendering code. Key exports: `bundle()` (esbuild TSX → JS), `resolveFonts()` / `resolveImages()` (file paths → base64), `renderPdf()` / `renderLayout()` (JS → WASM → bytes/JSON). The preview HTML (`src/preview/index.html`) supports dual mode: standalone for CLI dev server, and VS Code webview (receives messages instead of fetching endpoints). Build order: `react` → `core` → `renderer` → `cli` / `vscode`.

### VS Code Extension (`forme-pdf`)
Live PDF preview inside VS Code. Architecture: `LayoutStore` is the central event-emitting store — preview panel, component tree, and inspector all subscribe to it, staying decoupled from each other. The preview panel uses the same `index.html` from `@formepdf/renderer` (VS Code mode). Component tree (`TreeView` sidebar) shows the element hierarchy with hover-to-highlight. Inspector (`WebviewView` sidebar) shows box model visualization, computed styles, "Open in Editor" (maps element source locations to editor), and "Copy Style". The extension watches `.tsx` files and re-renders on save.

## Pre-Commit Rules

Before every git commit, run the following and fix any issues:

**Rust (if any `engine/` files changed):**
```bash
cd engine && cargo fmt && cargo clippy -- -W clippy::all
```

**TypeScript (if any `packages/` files changed):**
```bash
# Build affected packages (build order: react → core → cli)
# Run tsc for each changed package, e.g.:
cd packages/react && npm run build
cd packages/core && npm run build
cd packages/mcp && npm run build
```

Also run tests for any package with changes:
```bash
cd packages/react && npm test
```

Do not commit if any command produces warnings or errors.

**Important**: After running `cargo fmt`, always verify the formatted files are staged and committed. CI runs `cargo fmt --check` and will fail if formatting diffs remain. The local `cargo fmt` modifies files in-place but doesn't stage them — easy to miss.

**CI note**: Integration tests that depend on macOS system fonts (e.g., Arial Unicode) must gracefully skip when the font file is absent. CI runs on Linux and doesn't have `/System/Library/Fonts/`.

## Build & Test

```bash
# Engine only
cd engine
cargo build
cargo test
cargo run -- --example > invoice.json    # dump example invoice
cargo run -- invoice.json -o output.pdf  # render to PDF

# Full pipeline (engine → WASM → packages)
cd packages/core && npm run build        # Rust → WASM + TS wrapper
cd packages/cli && npm run build         # TS → JS + copy preview HTML

# Dev server (live preview at http://localhost:4242)
node packages/cli/dist/index.js dev test-preview.tsx

# VS Code extension
cd packages/vscode && npm run build    # esbuild → dist/extension.js
cd packages/vscode && npm run package  # → forme-pdf-{version}.vsix
```

## Architecture (data flow)

```
JSON / API input
      ↓
  Document (model/mod.rs)     # Tree of Node { kind, style, children }
      ↓
  Style Resolution            # Style::resolve() → ResolvedStyle (no Options)
      ↓
  Layout Engine               # PageCursor tracks position, splits across pages
      ↓
  Vec<LayoutPage>             # Each page = list of positioned LayoutElements
      ↓
  PDF Serializer              # Writes %PDF-1.7 header, objects, xref, trailer
      ↓
  Vec<u8>                     # Valid PDF file bytes
```

## Key Design Decisions

### Page-Native Layout (THE DIFFERENTIATOR)
The layout engine uses a `PageCursor` that tracks the current Y position on the current page. Before placing any node, it checks: "does this fit in the remaining space?" If not, it either moves the node to a new page (unbreakable) or splits it (breakable). For tables, header rows are automatically re-drawn on continuation pages.

**This is different from react-pdf**, which lays out everything on an infinite canvas and slices. That's why react-pdf's flex breaks on page boundaries — flex runs once on the full container, then gets sliced, making both halves wrong.

### Flex After Split
When a breakable flex container splits across pages, children are laid out individually into available space. This means flex calculations reflect actual page-constrained dimensions, not pre-split infinite-canvas dimensions.

### No CSS Margin Collapsing
Margins are additive (like flexbox gap), not collapsing. This is a deliberate simplification that makes layout more predictable. Document this to users.

### Coordinate System
Layout: origin at top-left, Y increases downward (like web).
PDF: origin at bottom-left, Y increases upward.
Transform in pdf serializer: `pdf_y = page_height - layout_y - element_height`

## Layout Features

### Widow/Orphan Control
`layout_text` and `layout_text_runs` call `page_break::decide_break()` before placing lines. This prevents a single orphan line at the bottom of a page or a single widow line at the top of the next page. Configurable via `minWidowLines` and `minOrphanLines` style properties (default: 2 each). The decision logic returns `Place` (all lines fit), `MoveToNextPage` (move entire paragraph), or `Split { items_on_current_page }` (break at the right point).

### Flex Wrap + align-content
`layout_flex_row` supports `flex-wrap: wrap` with cross-axis distribution via `align-content`. Supported values: `flex-start` (default), `flex-end`, `center`, `space-between`, `space-around`, `space-evenly`, `stretch`. Only applies when the container has a fixed height (otherwise there's no slack to distribute). Post-layout adjustment shifts wrap lines vertically based on the chosen alignment.

### Table Cell Overflow
`layout_table_row` uses cursor cloning to preserve cell content that exceeds page height. Instead of discarding overflow (the old `&mut Vec::new()` approach), each cell gets a cloned cursor and a `cell_pages` vec. If cell content triggers page breaks, the overflow pages are collected and appended to the real pages list. Content is preserved rather than silently discarded.

### Fixed Height Containers
`SizeConstraint::Fixed(h)` is respected in both `layout_view` (for the container's own Rect height) and `measure_node_height` (so parent containers measure children correctly). When a fixed height is set, it takes precedence over computed children height.

### Column justify-content + align-items
`layout_children` column branch applies `justify-content` (vertical distribution) and `align-items` (horizontal alignment) as post-layout adjustments. Requires a fixed parent height for justify-content to have slack to distribute. Supports all standard values: `flex-start`, `flex-end`, `center`, `space-between`, `space-around`, `space-evenly`. `align-items` supports `flex-start`, `flex-end`, `center`, and `stretch` (default).

When `flex-grow` expands a child's height, `reapply_justify_content()` redistributes that child's children vertically. This enables patterns like a cover page where a `flex: 1` container with `justifyContent: 'center'` vertically centers its content.

**Cross-axis stretch propagation**: When a flex row stretches a child via `alignItems: stretch` (default), the child's style still has `height: Auto`. `layout_node` accepts an optional `cross_axis_height: Option<f64>` parameter — when present and the node's height is `Auto`, it overrides to `Fixed(h)`. This makes justify-content, flex-grow, and other height-dependent logic work inside stretched items. In `layout_flex_row`, when stretch applies, `Some(line_height - margin.vertical())` is passed; all other call sites pass `None`.

For `align-items: center/flex-end`, percentage-width children (e.g., `width: '80%'`) are passed `available_width` to `layout_node` so the percentage resolves correctly against the parent, not the already-resolved child width. Auto-width children receive their intrinsic width instead, preventing them from stretching. `measure_intrinsic_width` for Images accounts for height constraints via aspect ratio, matching `layout_image` behavior.

### Flex Min-Content Width
During flex shrink in `layout_flex_row`, items cannot be compressed below their min-content width (the widest unbreakable word in text nodes). This prevents short words from wrapping inside flex children. Computed by `measure_min_content_width` which delegates to `TextLayout::measure_widest_word` for text nodes.

### Absolute Positioning
`position: 'absolute'` places children relative to their parent's content box, not the page. `top`, `right`, `bottom`, `left` are offsets from the parent's padding edge. Implemented via `parent_box_x` / `parent_box_y` saved at the start of `layout_children`.

### Per-Run Text Decoration
In multi-style text (`TextRun`), decorations like `line-through` and `underline` are applied per-glyph-group in the PDF serializer, not per-line. Each `PositionedGlyph` carries its own `text_decoration` field. This means `<Text>$42.00<Text style={{textDecoration: 'line-through'}}> $56.00</Text></Text>` only strikes through the second span.

### Custom Font Registration
Users register custom TrueType fonts via `Font.register()` (global, react-pdf compatible) or the `<Document fonts={[...]}>` prop (per-document). The data flow:

1. **React layer** (`font.ts` + `serialize.ts`): `Font.register()` stores registrations globally. `serialize()` merges global + document fonts into a `fonts[]` array on the JSON output. Font sources (`src`) pass through unresolved — file paths, data URIs, or `Uint8Array`.
2. **Rendering layer** (`core/index.ts` or `cli/dev.ts`): Resolves font sources to base64 before passing JSON to WASM. File paths are read from disk; `Uint8Array` is base64-encoded; data URIs pass through as-is. In the CLI dev server, file paths resolve relative to the template directory.
3. **Engine** (`lib.rs`): `register_document_fonts()` decodes base64 from each `FontEntry` and calls `FontContext.registry_mut().register()` before layout. The existing `FontRegistry`, `CustomFontMetrics`, and PDF subsetting handle everything from there.

Key files: `packages/react/src/font.ts`, `packages/react/src/serialize.ts` (mergeFonts), `packages/core/src/index.ts` (resolveFonts), `packages/cli/src/dev.ts` (resolveFontPaths), `engine/src/lib.rs` (register_document_fonts), `engine/src/model/mod.rs` (FontEntry).

Merge strategy: fonts are keyed by `family:weight:italic`. Document fonts override global fonts on conflict.

### Template Expression System
Templates enable a hosted API workflow: store template JSON + dynamic data → produce PDFs without a JavaScript runtime. Three layers:

1. **Rust expression evaluator** (`engine/src/template.rs`): `evaluate_template(template, data)` walks a `serde_json::Value` tree, resolving expression nodes (`$ref`, `$each`, `$if`, `$cond`, comparisons, arithmetic, string ops, `$format`, `$count`) against a data object. `EvalContext` holds root data + scoped bindings (from `$each` "as"). `$each` results use a `__flatten` marker so parent arrays flatten them inline. Missing `$ref` paths silently omit the value.

2. **TypeScript template compiler** (`packages/react/src/template-proxy.ts`, `expr.ts`, `serialize.ts`): `createDataProxy()` returns a recording Proxy that captures property access as `$ref` markers and `.map()` calls as `$each` markers. `Symbol.toPrimitive` returns sentinel strings (`\0FORME_REF:path\0`) so JSX string interpolation produces detectable markers. `expr` helpers produce expression markers for operations Proxy can't capture (comparisons, arithmetic, conditionals). `serializeTemplate()` mirrors `serialize()` but uses `flattenTemplateChildren()` (bypasses React's `Children.forEach` which rejects proxy objects) and `processTemplateValue()` to detect markers.

3. **Integration** (`packages/core/src/index.ts`, `packages/cli/src/template-build.ts`): `renderTemplate()` and `renderTemplateWithLayout()` call WASM template functions. CLI `forme build --template` bundles TSX → imports → creates proxy → calls template fn → `serializeTemplate()` → resolves fonts → writes JSON.

Key files: `engine/src/template.rs`, `engine/src/lib.rs` (render_template), `engine/src/wasm.rs` (render_template_pdf), `packages/react/src/template-proxy.ts`, `packages/react/src/expr.ts`, `packages/react/src/serialize.ts` (serializeTemplate), `packages/core/src/index.ts` (renderTemplate), `packages/cli/src/template-build.ts` (buildTemplate), `packages/cli/src/index.ts` (--template flag).

### CSS String Shorthands (React layer only)
Parsed in `mapStyle()` in `serialize.ts` — no engine changes needed. Three capabilities:

1. **Border shorthand**: `border: "1px solid #000"` → parses into `borderWidth` + `borderColor`. Per-side variants: `borderTop: "2px solid #f00"` or `borderBottom: 3` (number = width only). `parseBorderString()` tokenizes by whitespace, recognizes CSS border-style keywords (ignored), numeric tokens (width), and color tokens.
2. **Edge strings**: `padding: "8 16"` or `margin: "8 16 24 32"` → CSS 1-4 value shorthand. Optional `px` suffix stripped. `parseCSSEdges()` handles the parsing.
3. **Edge arrays**: `padding: [8, 16]` or `margin: [20, 40, 20, 40]` → same 1-4 value pattern as arrays.

Cascade priority (highest wins): `borderTopWidth` > `borderWidth` > `borderTop: "..."` > `border: "..."`.

Also widened `<Page margin>` to accept strings and arrays: `<Page margin="36 72">`.

### QR Codes
`<QrCode data="..." size={100} />` renders a vector-based QR code. The engine module `qrcode.rs` uses the `qrcode` crate to generate a `QrMatrix` (bool grid). `NodeKind::QrCode { data, size }` is laid out by `layout_qrcode()` (follows the `layout_image` pattern — compute display size, check page fit, push element). PDF rendering emits filled rectangles (`re f`) for each dark module in the content stream — native vector, not raster. The `DrawCommand::QrCode { modules, module_size, color }` variant carries the matrix data.

Key files: `engine/src/qrcode.rs`, `engine/src/model/mod.rs` (NodeKind::QrCode), `engine/src/layout/mod.rs` (layout_qrcode), `engine/src/pdf/mod.rs` (QrCode rendering), `packages/react/src/components.tsx` (QrCode), `packages/react/src/serialize.ts` (serializeQrCode).

### Text Overflow (Ellipsis/Clip)
`textOverflow: 'ellipsis'` truncates single-line text with "..." (U+2026) when it exceeds available width. `textOverflow: 'clip'` truncates without an indicator. `TextOverflow` enum in `style/mod.rs` with variants `Wrap` (default), `Ellipsis`, `Clip`. When not `Wrap`, `layout_text` and `layout_text_runs` take only the first line from line breaking, then call truncation methods on `TextLayout` (`truncate_with_ellipsis`, `truncate_clip`, `truncate_runs_with_ellipsis`, `truncate_runs_clip`) to fit within `available_width`. No PDF changes needed — text is already truncated before serialization.

### Font Fallback Chains
`fontFamily: "Inter, Helvetica"` tries each comma-separated family in order. `FontRegistry::resolve()` splits on commas, strips quotes, and tries each family in this order:
1. Exact weight (e.g., 700)
2. Snapped weight (700 if weight ≥ 600, else 400)
3. Opposite weight (400 if snapped was 700, else 700)

Falls back to Helvetica if nothing matches. The opposite-weight step is critical for per-character font fallback: a custom font registered only at weight 400 (e.g., ArialUnicode) will still be found when bold text (700) needs it for Arabic/CJK glyphs that Helvetica-Bold lacks. `resolve_for_char()` uses the same three-step resolution with an additional `has_char(ch)` check at each step.

Backward-compatible: a single family name (no comma) behaves identically to the old code. `FontContext` methods (`char_width`, `measure_string`, `font_data`, etc.) get fallback support automatically since they delegate to `resolve()`.

### Intrinsic Width and textTransform
`measure_intrinsic_width()` and `measure_min_content_width()` apply `apply_text_transform()` before measuring text. Without this, containers sized via intrinsic measurement (e.g., auto-width children inside `align-items: center`) would be too narrow when `textTransform: 'uppercase'` is set, because uppercase glyphs are wider than their lowercase counterparts. The same transform is applied in `measure_min_content_width()` for flex shrink min-content calculations. QR codes also report their explicit `size` as intrinsic width (falls back to 0 when unset), fixing centering via `align-items: center`.

### WinAnsi Width Mapping
Standard font `char_width()` in `font/metrics.rs` maps Unicode codepoints through `unicode_to_winansi()` before looking up glyph widths. Characters like em-dash (U+2014), en-dash (U+2013), smart quotes, ellipsis, etc. have Unicode code points above 255 but their widths are stored at WinAnsi positions (0x80–0x9F). The shared `unicode_to_winansi()` function in `font/metrics.rs` is also used by `PdfSerializer` for PDF text encoding — single source of truth for the Windows-1252 mapping.

### Grid repeat() Syntax
React-layer only. `expandRepeat()` in `serialize.ts` pre-processes grid template strings, expanding `repeat(N, tracks)` before the existing split-on-whitespace logic. Example: `repeat(3, 1fr)` → `1fr 1fr 1fr`. Supports mixed: `200 repeat(2, 1fr) 200` → `200 1fr 1fr 200`.

### Alt Text, Document Language, Clickable Images/SVGs
- **Alt text**: `alt` prop on `<Image>` and `<Svg>` flows through `Node.alt` → `LayoutElement.alt`. Carried through the data model for future tagged PDF support (actual `/Alt` emission requires structure elements — follow-up scope).
- **Document language**: `<Document lang="en-US">` → `Metadata.lang` → emitted as `/Lang (en-US)` in the PDF Catalog dictionary.
- **Clickable images/SVGs**: `href` prop on `<Image>` and `<Svg>` passes through to layout via `node.href.clone()`. The PDF serializer already handles `href` on any `LayoutElement` — no PDF-side changes were needed.

### Per-Character Font Fallback
`fontFamily: "Inter, NotoSansArabic, NotoSansSC"` now resolves fonts per-character, not per-block. Fast path: `!families.contains(',')` skips all per-char logic for single-font documents (zero regression). `FontData::has_char(ch)` checks glyph coverage. `FontRegistry::resolve_for_char()` walks comma-separated families per character. `fallback::segment_by_font()` groups consecutive same-font chars into `FontRun` segments. Integrated with BiDi: font segmentation happens within each BiDi run, not across runs. Key files: `engine/src/font/fallback.rs` (new), `engine/src/font/mod.rs` (has_char, resolve_for_char), `engine/src/text/mod.rs` (measure_chars), `engine/src/layout/mod.rs` (build_positioned_glyphs).

### Overflow Hidden
`overflow: 'hidden'` clips children to parent bounds via PDF clip path operators. Visual-only — layout is unaffected. PDF pattern: `q / x y w h re W n / (children) / Q`. Nested `overflow: hidden` composes correctly via the graphics state stack. `Overflow` enum in `style/mod.rs` with `Visible` (default) and `Hidden` variants. Field propagated to `LayoutElement` and set at all construction sites.

### Canvas Drawing Primitive
`<Canvas width={w} height={h} draw={(ctx) => { ... }} />` renders arbitrary vector graphics. `CanvasOp` enum (20 variants) in `model/mod.rs`. Layout follows `layout_svg` pattern (fixed-size leaf, page break check). Operations convert to `SvgCommand` via `canvas_ops_to_svg_commands()`, reusing the existing `DrawCommand::Svg` + `write_svg_commands()` PDF pipeline — no new PDF rendering code. React layer: recording `CanvasContext` executes `draw` callback during serialization, producing `CanvasOp[]` JSON. **Color convention**: Canvas API uses 0-255 RGB (`setFillColor(59, 130, 246)`); `canvas_ops_to_svg_commands` divides by 255.0 to convert to the 0-1 range expected by the PDF/SVG pipeline. **Arc direction**: `CanvasOp::Arc` supports the full HTML Canvas `arc()` signature including `counterclockwise` (default `false`). When `!counterclockwise && sweep < 0`, 2π is added; when `counterclockwise && sweep > 0`, 2π is subtracted. **Clipping**: Canvas content is clipped to its bounds via `DrawCommand::Svg { clip: true }`. The PDF serializer emits `0 0 w h re W n` after coordinate transforms but before `write_svg_commands()`. SVG elements use `clip: false` (no clipping). Key files: `engine/src/model/mod.rs` (CanvasOp, NodeKind::Canvas), `engine/src/layout/mod.rs` (canvas_ops_to_svg_commands, layout_canvas), `engine/src/pdf/mod.rs` (clip rect emission), `packages/react/src/serialize.ts` (serializeCanvas).

### Chart Components (BarChart, LineChart, PieChart)
Pure React-layer components in `packages/react/src/charts.tsx`. Return `<View>` + `<Svg>` + positioned `<Text>` children — no engine changes for the components themselves. Labels use positioned `<Text>` nodes (not SVG text, which is unsupported). SVG arc path (`A`/`a`) support was added to the engine for PieChart slices — `svg_arc_to_curves()` in `engine/src/svg/mod.rs` implements W3C SVG spec F.6.5/F.6.6 (endpoint-to-center parameterization → cubic bezier conversion). **Arc-to-bezier formula**: `alpha = 4/3 * tan(θ/4)` per standard circular arc approximation (NOT `tan(θ/2)` which over-shoots control points). Layout constants: `Y_AXIS_WIDTH = 28`, `X_AXIS_HEIGHT = 20`, `AXIS_LABEL_FONT = 8`. Helpers: `niceNumber()` for axis scaling, `lightenColor()` for area fills, `formatNumber()` for compact labels.

### Watermarks
`<Watermark text="DRAFT" fontSize={60} color="rgba(0,0,0,0.1)" angle={-45} />` renders rotated text behind all page content. Stored on `PageCursor.watermarks` (like fixed_header/fixed_footer), cloned on each page via `new_page()`. `inject_fixed_elements()` shapes the watermark text and creates `DrawCommand::Watermark` elements prepended before all page content. PDF rendering: `q` → opacity ExtGState → translate to page center → rotation matrix (`cos/sin cm`) → BT/Tf/Td/Tj/ET → `Q`. Color alpha from `rgba()` multiplied with style opacity. `parseColor()` in serialize.ts extended to handle `rgba(r,g,b,a)` and `rgb(r,g,b)` formats. Key files: `engine/src/model/mod.rs` (NodeKind::Watermark), `engine/src/layout/mod.rs` (PageCursor.watermarks, DrawCommand::Watermark), `engine/src/pdf/mod.rs` (Watermark rendering), `packages/react/src/serialize.ts` (serializeWatermark, parseColor rgba).

### Justified Text (PDF Tw operator)
`textAlign: 'justify'` distributes extra word spacing via the PDF `Tw` (word spacing) operator. The layout engine computes slack as `available_width - natural_glyph_width_sum` (using the sum of `x_advance` from positioned glyphs, NOT the Knuth-Plass adjusted line width which already bakes justification into `char_positions`). Trailing spaces are excluded via `rposition(|g| g.char_value != ' ')`. The `Tw` value is `slack / space_count`. Both `layout_text` (single-style) and `layout_text_runs` (multi-style) use the same pattern.

### Shaping Cluster Indices
`shape_text_with_direction()` in `text/shaping.rs` converts rustybuzz's byte-offset cluster values to char indices. Rustybuzz returns byte offsets into the UTF-8 string, but downstream code indexes into `Vec<char>`. A `byte_to_char` HashMap maps byte positions to char positions. Without this conversion, multi-byte characters (Arabic, CJK) produce wrong cluster lookups.

### PDF Standard Font Widths
Standard fonts (Helvetica, Times, Courier) now emit `/Widths` arrays in the PDF font dictionary. Previously, PDF viewers substituted system fonts with potentially different metrics. The `/FirstChar 32 /LastChar 255 /Widths [...]` entries ensure viewers use our exact glyph widths, preventing text overflow and misaligned justification.

### Embedded Data (PDF File Attachments)
`renderDocument(element, { embedData })` and `renderDocumentWithLayout(element, { embedData })` attach a JSON object as a compressed `forme-data.json` EmbeddedFile inside the PDF. `extractData(pdfBytes)` reads it back. Three usage patterns:

1. **Programmatic (opt-in)**: Pass `embedData` in the render options. The value is JSON-stringified and set as `doc.embeddedData` on the serialized document before WASM rendering.
2. **Hosted API (automatic)**: `POST /v1/render/:slug` in the hosted API auto-embeds the request body. `POST /v1/extract` accepts raw PDF bytes and returns the embedded JSON.
3. **Templates**: `renderTemplate()` embeds the data JSON automatically.

Data flow: `options.embedData` → `JSON.stringify()` → `doc.embeddedData` (string) → `Document.embedded_data` (Rust `Option<String>`) → `PdfSerializer::serialize(embedded_data)` → FlateDecode-compressed stream in a `/Type /EmbeddedFile` object → `/Names` tree with `/FileSpec` referencing `forme-data.json`.

Extraction: `extractData()` in `packages/core/src/extract.ts` scans PDF bytes for the `forme-data.json` FileSpec, finds the referenced stream object, decompresses (FlateDecode via `node:zlib`), and parses JSON. Returns `null` for PDFs without embedded data.

Key files: `packages/core/src/index.ts` (RenderDocumentOptions.embedData), `packages/core/src/extract.ts` (extractData), `engine/src/model/mod.rs` (Document.embedded_data), `engine/src/pdf/mod.rs` (EmbeddedFile stream + Names tree), `engine/src/lib.rs` (passes embedded_data to serializer).

## Known Issues & Limitations (Current State)

1. No variable font axis support.
2. No vertical text layout (CJK writing modes).
3. No `grid-template-areas` or `grid-auto-flow: dense`.
4. `align-items: baseline` is parsed but treated as `flex-start` (returns 0.0 offset in `layout/mod.rs:1848`).

## Potential Next Steps

### Engine Features

**`align-items: baseline`** (Low effort, high correctness value)
The enum variant exists in `style/mod.rs` and the match arm exists in layout but returns 0.0. Needs: measure each flex child's first text baseline (distance from top of child to the alphabetic baseline of its first line of text), find the max, and offset each child so baselines align. Affects `layout_flex_row` cross-axis positioning. Would require a `measure_baseline()` helper that walks into a node's children to find the first text node and returns its ascender-based offset.

**`grid-template-areas`** (Medium effort, productivity win)
Named grid areas like `gridTemplateAreas: '"header header" "sidebar main"'`. Needs: parse the area string into a 2D grid of names, map each child's `gridArea` name to its row/column span. Most of the grid track sizing and placement machinery in `layout/grid.rs` already works — this is primarily a parsing + name-to-span resolution layer on top.

**`grid-auto-flow: dense`** (Low effort, niche)
Auto-placement currently uses row-major order and never backtracks. Dense packing would scan for earlier gaps that fit the item. Small change to the placement loop in `grid.rs`.

**Variable font support** (High effort, typography value)
Would allow a single `.ttf` file to serve multiple weights/widths via `fvar` axis values. Needs: parse `fvar` table in `font/mod.rs`, interpolate glyph outlines (or use `rustybuzz` variation support), and adjust the registration model so a single font file maps to multiple `FontKey` entries. The subsetter would also need to preserve variation tables.

**Vertical text / CJK writing modes** (Very high effort, Asian market)
`writing-mode: vertical-rl` for Japanese/Chinese/Korean vertical text. Touches nearly every part of the pipeline: text measurement (swap width/height), line breaking (lines flow right-to-left), glyph rotation, page cursor direction. Would be a major architectural addition to `layout/mod.rs` and `text/mod.rs`.

**CMYK color support** (Medium effort, print industry)
PDF natively supports CMYK via `/DeviceCMYK` color space. Would need a `Color::Cmyk { c, m, y, k }` variant in `style/mod.rs`, plumbing through layout to PDF serialization. The PDF side is straightforward (`c m y k K` operators instead of `r g b rg`).

### Platform / Ecosystem

**Serverless PDF API** — A hosted endpoint where users POST template JSON + data and get back PDF bytes. Would use the existing template expression system (`engine/src/template.rs`). No JS runtime needed server-side.

**Figma/design tool importer** — Convert Figma frames to Forme document trees. Figma's auto-layout maps well to Forme's flex model. Would be a separate package that produces `@formepdf/react` JSX or raw JSON.

**More framework integrations** — Express/Fastify middleware, Remix loader, SvelteKit endpoint. Same pattern as `@formepdf/hono` and `@formepdf/next`.

**Performance benchmarks** — Automated benchmarks for layout + PDF serialization speed. Track regressions across releases. Useful for marketing ("renders 100-page document in Xms").

## How the Layout Engine Works (for making changes)

The core loop in `layout/mod.rs`:

```rust
fn layout_node(&self, node, cursor, pages, x, available_width, parent_style) {
    match node.kind {
        Text { content } => layout_text(content, ...),     // Line break, place lines
        View => layout_view(node, ...),                     // Flex container
        Table { columns } => layout_table(node, ...),       // Row-by-row with headers
        Image { .. } => layout_image(node, ...),            // Block placement
        Svg { .. } => layout_svg(node, ...),                // SVG rendering
        QrCode { data, size } => layout_qrcode(node, ...), // Vector QR code
        Canvas { .. } => layout_canvas(node, ...),        // Arbitrary vector graphics
        Watermark { .. } => { cursor.watermarks.push(node) } // Store for per-page injection
        PageBreak => { pages.push(cursor.finalize()); *cursor = cursor.new_page(); }
        Fixed { position } => { store in cursor for repetition }
    }
}
```

**PageCursor** is the central state:
- `y`: current vertical position within content area (increases downward)
- `content_width`, `content_height`: page content area dimensions
- `content_x`, `content_y`: offset of content area (accounts for margins)
- `elements`: laid-out elements on this page
- `remaining_height()`: how much vertical space is left
- `finalize()`: produces a LayoutPage from current state
- `new_page()`: creates fresh page, carries over fixed elements

### Element Nesting (Snapshot-and-Collect Pattern)
Layout elements form a hierarchy that mirrors the document tree. This is critical for the dev server's click-to-inspect (depth-first hit-testing). The pattern used in layout functions:

1. Save `snapshot = cursor.elements.len()` before laying out children
2. Lay out children normally (they push to `cursor.elements`)
3. After layout, `drain(snapshot..)` to collect child elements
4. Create the parent element with `children: child_elements`
5. Push the parent onto `cursor.elements`

This is used in:
- **`layout_view`** (non-breakable path): View rect wraps its children
- **`layout_breakable_view`** (breakable path): Wraps children in a `DrawCommand::Rect` per page when the view has background/border (clone semantics: each page fragment gets full styling)
- **`layout_table_row`**: Row wraps Cells, each Cell wraps its content
- **`layout_text`**: Text container wraps TextLine elements (flushes on page breaks)

**Not** used in `layout_flex_row` (items are laid out individually via `layout_view` which handles its own nesting).

The PDF serializer (`write_element`) and layout overlay (`drawLayoutOverlay`) both recurse into `element.children`. Any new layout function that creates a container element must use this pattern to maintain the hierarchy.

**Adding a new node type:**
1. Add variant to `NodeKind` in `model/mod.rs`
2. Add match arm in `layout_node` in `layout/mod.rs`
3. Write the layout function (measure height → check fit → place or split)
4. If it's a container, use snapshot-and-collect to nest children
5. Add drawing in `write_element` in `pdf/mod.rs` if it has visual output

## Testing Strategy

- **Unit tests** in each module (`#[cfg(test)]` blocks): flex distribution, page break decisions, text line breaking, PDF string escaping
- **Integration tests** in `tests/integration.rs`: full pipeline from Document → PDF bytes, verifying page counts, PDF structural validity, JSON deserialization
- **Visual regression** in `tests/visual_regression.rs`: render known documents, compare pixel-by-pixel against reference images. Used for table header repetition, page break aesthetics, flex layout correctness.

When making layout changes, always test with:
1. The example invoice (`cargo run -- --example | cargo run -- -o test.pdf`)
2. A document with enough content to overflow multiple pages
3. A table with 50+ rows (verifies header repetition)

## Dependencies

Engine (Rust):
- `serde` + `serde_json`: JSON deserialization of document tree
- `miniz_oxide`: DEFLATE compression for PDF content streams
- `ttf-parser`: Font file parsing for real glyph metrics and subsetting
- `qrcode`: QR code generation (pure Rust, WASM-safe)
- `rustybuzz`: OpenType shaping (GSUB/GPOS)
- `unicode-bidi` + `unicode-script`: Bidirectional text support
- `unicode-linebreak`: UAX#14 line break algorithm
- `hypher`: Hyphenation dictionaries (35+ languages)

## Code Style

- Comments explain WHY, not WHAT
- The doc comments at the top of each module explain the design intent
- Use `///` doc comments on all public items
- Err on the side of explicitness (no implicit conversions, no magic)
- `f64` everywhere for coordinates (sufficient precision, matches PDF spec)
- Prefix unused variables with `_` to suppress warnings
