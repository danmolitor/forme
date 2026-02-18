# CLAUDE.md

## What This Is

Forme is a **page-native PDF rendering engine** written in Rust. It takes a tree of document nodes (like a simplified DOM) and produces PDF bytes. The key differentiator is that layout happens INTO pages rather than on an infinite canvas that gets sliced afterward. This means page breaks, table header repetition, and flex layout across pages all work correctly.

## Project Structure

```
forme/
├── CLAUDE.md               # You are here
├── README.md               # Product readme
├── engine/                 # Rust rendering engine
│   ├── Cargo.toml          # Deps: serde, serde_json, miniz_oxide, ttf-parser
│   ├── src/
│   │   ├── lib.rs          # Public API: render(), render_json(), render_with_layout()
│   │   ├── main.rs         # CLI binary + example invoice JSON
│   │   ├── model/mod.rs    # Document tree: Node, NodeKind, PageConfig, Edges
│   │   ├── style/mod.rs    # CSS-like styles, resolution with inheritance
│   │   ├── layout/
│   │   │   ├── mod.rs      # THE CORE: page-aware layout engine + element nesting
│   │   │   ├── flex.rs     # Flex grow/shrink/wrap distribution helpers
│   │   │   └── page_break.rs # Break decision logic (split/move/place)
│   │   ├── text/mod.rs     # Line breaking + text measurement
│   │   ├── font/mod.rs     # Font registry + custom font subsetting
│   │   ├── image_loader/   # JPEG/PNG decoding from file paths and data URIs
│   │   └── pdf/mod.rs      # PDF 1.7 serializer (from scratch)
│   └── tests/
│       └── integration.rs  # Full pipeline tests
└── packages/
    ├── core/               # WASM bridge: compiles engine to WebAssembly
    │   ├── src/index.ts    # JS API: renderDocument(), renderDocumentWithLayout()
    │   └── build.sh        # wasm-pack build + wasm-opt
    ├── react/              # JSX component library: <Document>, <Page>, <View>, etc.
    │   └── src/index.tsx   # Components + serialize() to JSON document tree
    └── cli/                # `forme dev` and `forme build` commands
        ├── src/dev.ts      # Dev server with live reload, PDF + layout endpoints
        ├── src/preview/    # Browser UI: preview, overlays, click-to-inspect
        └── package.json    # Build: tsc + copy preview assets to dist/
```

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

## Known Issues & Limitations (Current State)

### NICE TO HAVE (LATER)

7. No Knuth-Plass line breaking (using greedy algorithm — fine for documents).
8. No hyphenation.
9. No BiDi text support (Arabic, Hebrew).
10. No CSS Grid.
11. No PDF/A compliance.

## How the Layout Engine Works (for making changes)

The core loop in `layout/mod.rs`:

```rust
fn layout_node(&self, node, cursor, pages, x, available_width, parent_style) {
    match node.kind {
        Text { content } => layout_text(content, ...),     // Line break, place lines
        View => layout_view(node, ...),                     // Flex container
        Table { columns } => layout_table(node, ...),       // Row-by-row with headers
        Image { .. } => layout_image(node, ...),            // Block placement
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
- **`layout_table_row`**: Row wraps Cells, each Cell wraps its content
- **`layout_text`**: Text container wraps TextLine elements (flushes on page breaks)

**Not** used in `layout_breakable_view` (no wrapper) or `layout_flex_row` (items are laid out individually via `layout_view` which handles its own nesting).

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
- **Visual regression** (not yet built): render known documents, compare pixel-by-pixel against reference images. Use this for table header repetition, page break aesthetics, flex layout correctness.

When making layout changes, always test with:
1. The example invoice (`cargo run -- --example | cargo run -- -o test.pdf`)
2. A document with enough content to overflow multiple pages
3. A table with 50+ rows (verifies header repetition)

## Dependencies

Engine (Rust):
- `serde` + `serde_json`: JSON deserialization of document tree
- `miniz_oxide`: DEFLATE compression for PDF content streams
- `ttf-parser`: Font file parsing for real glyph metrics and subsetting

To add:
- `unicode-linebreak`: UAX#14 line break algorithm (proper Unicode line breaking)

## Code Style

- Comments explain WHY, not WHAT
- The doc comments at the top of each module explain the design intent
- Use `///` doc comments on all public items
- Err on the side of explicitness (no implicit conversions, no magic)
- `f64` everywhere for coordinates (sufficient precision, matches PDF spec)
- Prefix unused variables with `_` to suppress warnings
