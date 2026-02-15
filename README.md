# Forme

A page-native PDF rendering engine written in Rust.

## The Problem

Every PDF renderer you've used does the same thing wrong: it lays content out on an infinite vertical canvas, then slices that canvas into pages. This is why:

- Tables break in the middle of rows
- Flexbox layouts collapse on page boundaries
- Headers don't repeat when tables span pages
- Content gets "mashed together" after a page split

These bugs have been open in react-pdf for 7 years. They're not bugs — they're an architectural flaw. You can't fix page breaks by slicing an infinite canvas. The slicing is the problem.

## The Solution

Forme never creates an infinite canvas. **The page is the fundamental unit of layout.** Every layout decision — every flex calculation, every line break, every table row placement — is made with the page boundary as a hard constraint.

When a flex container splits across pages, both fragments get their own independent flex layout pass. When a table crosses a page boundary, header rows are automatically repeated. When text breaks across pages, widow and orphan rules are respected.

Content flows INTO pages. It doesn't get sliced after the fact.

## Architecture

```
Input (JSON / React reconciler output)
      ↓
  [model]    Document tree: nodes, styles, content
      ↓
  [style]    Resolve cascade, inheritance, defaults
      ↓
  [layout]   Page-aware layout engine ← this is the product
      ↓
  [pdf]      Serialize to PDF 1.7 bytes
```

## Usage

```bash
# Generate an example invoice
forme --example > invoice.json

# Render to PDF
forme invoice.json -o invoice.pdf

# Pipe from stdin
echo '{"children": [...]}' | forme -o output.pdf
```

## Document Format

Documents are JSON trees of nodes. Each node has a `kind`, optional `style`, and optional `children`:

```json
{
  "kind": { "type": "View" },
  "style": {
    "flexDirection": "Row",
    "gap": 12,
    "padding": { "top": 8, "right": 8, "bottom": 8, "left": 8 },
    "backgroundColor": { "r": 0.95, "g": 0.95, "b": 0.97, "a": 1.0 },
    "borderRadius": { "top_left": 4, "top_right": 4, "bottom_right": 4, "bottom_left": 4 }
  },
  "children": [
    {
      "kind": { "type": "Text", "content": "Hello, World." },
      "style": { "fontSize": 14, "fontWeight": 700 }
    }
  ]
}
```

### Node Types

| Type | Description |
|------|------------|
| `Page` | Explicit page boundary with size/margin config |
| `View` | Flexbox container (like `<div>`) |
| `Text` | Text content with line wrapping |
| `Image` | Image with src, width, height |
| `Table` | Table container with column definitions |
| `TableRow` | Row inside a table. Set `is_header: true` for repeating headers |
| `TableCell` | Cell inside a row |
| `Fixed` | Element that repeats on every page (header/footer) |
| `PageBreak` | Force a page break |

### Style Properties

**Box model:** `width`, `height`, `minWidth`, `maxWidth`, `padding`, `margin`

**Flexbox:** `flexDirection`, `justifyContent`, `alignItems`, `alignSelf`, `flexGrow`, `flexShrink`, `flexBasis`, `flexWrap`, `gap`

**Typography:** `fontFamily`, `fontSize`, `fontWeight`, `fontStyle`, `lineHeight`, `textAlign`, `letterSpacing`

**Visual:** `color`, `backgroundColor`, `opacity`, `borderWidth`, `borderColor`, `borderRadius`

**Page behavior:** `wrap` (breakable across pages), `breakBefore` (force page break), `minWidowLines`, `minOrphanLines`

## What Makes This Different

| Feature | react-pdf | Puppeteer | Forme |
|---------|-----------|-----------|--------|
| Page breaks in tables | Broken | N/A | ✓ Header repetition |
| Flex after page split | Broken | N/A | ✓ Re-runs flex per fragment |
| Widow/orphan control | No | No | ✓ Configurable |
| Render speed (invoice) | ~200ms | ~1500ms | ~15ms |
| Binary dependencies | Node.js | Chrome | None |
| Output size (invoice) | ~45KB | ~120KB | ~12KB |

## Building

```bash
cargo build --release

# Run tests
cargo test

# Build for WASM (for browser/Node.js integration)
wasm-pack build --target nodejs
```

## Roadmap

- [x] Core document model
- [x] Style resolution with inheritance
- [x] Page-aware flexbox (column + row)
- [x] Text layout with line breaking
- [x] Table layout with header repetition on page break
- [x] PDF 1.7 serialization
- [x] Widow/orphan control
- [ ] Custom font embedding (TrueType/OpenType via ttf-parser)
- [ ] Image embedding (JPEG/PNG)
- [ ] CSS Grid layout
- [ ] Bidirectional text (Arabic, Hebrew)
- [ ] Knuth-Plass line breaking
- [ ] Hyphenation
- [ ] PDF/A compliance
- [ ] WASM build for browser preview
- [ ] React reconciler (@forme/react)
- [ ] Node.js SDK (@forme/sdk)

## License

MIT
