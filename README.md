# forme

PDF generation with JSX. Page breaks that actually work.

![Forme dev server](./assets/dev-server.gif)

## Why

Every PDF tool makes you choose: fight with CSS page breaks or use an editor that can't handle dynamic data. Forme is a layout engine built for pages. No headless browser. No Chrome. Renders in milliseconds. Runs anywhere Node runs.

**[Try it in the playground](https://playground.formepdf.com/)**

## Quick Start

```bash
npm install @formepdf/cli @formepdf/react @formepdf/core
```

```tsx
import { Document, Page, View, Text } from '@formepdf/react';
import { renderDocument } from '@formepdf/core';

const pdf = await renderDocument(
  <Document>
    <Page size="Letter" margin={36}>
      <Text style={{ fontSize: 24, fontWeight: 'bold' }}>Invoice #2024-001</Text>
      <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginTop: 24 }}>
        <Text>Widget Pro</Text>
        <Text>$49.00</Text>
      </View>
    </Page>
  </Document>
);

// pdf is a Uint8Array: save it, serve it, email it
```

## Dev Server

```bash
npx forme dev invoice.tsx --data sample.json
```

Live preview with debug overlays. Click any element to inspect its computed styles.

## Features

- **Page-native layout**: Content flows into pages, not onto an infinite canvas. Page breaks happen at the right place, every time.
- **React components**: Document, Page, View, Text, Image, Table. If you know React, you know Forme.
- **Live preview**: `forme dev` shows your PDF updating in real time as you edit.
- **Click-to-inspect**: Select any element to see its box model, computed styles, and position.
- **Debug overlays**: Toggle bounding boxes, margins, and page break points.
- **Fast**: Rust engine compiled to WASM. Renders in milliseconds, not seconds.
- **OpenType shaping**: Real GSUB/GPOS shaping via rustybuzz. Ligatures (fi, ffi), kerning (AV), and contextual forms render correctly with custom fonts.
- **Optimal line breaking**: Knuth-Plass algorithm (the same one TeX uses) considers the entire paragraph to minimize awkward spacing. Falls back to greedy when needed.
- **Hyphenation**: Automatic hyphenation in 35+ languages. Set `hyphens: 'auto'` and a `lang` tag. Uses the hypher crate with language-specific dictionaries.
- **BiDi text**: Right-to-left text (Arabic, Hebrew) with automatic direction detection. Mixed LTR/RTL paragraphs reorder correctly. Set `direction: 'rtl'` or `direction: 'auto'`.
- **CSS Grid**: 2D grid layout with `display: 'grid'`. Fixed, fractional (`fr`), and auto track sizing. Explicit placement, auto-placement, column/row spanning, and row-level page breaks.
- **Flex wrap + align-content**: Flex containers wrap across pages correctly. `align-content` distributes wrapped lines (`center`, `space-between`, `space-around`, `space-evenly`, `flex-end`, `stretch`).
- **Widow/orphan control**: Text paragraphs never leave a single orphan line at the bottom of a page or a single widow line at the top. Configurable via `minWidowLines` and `minOrphanLines`.
- **Table overflow**: Table cells with content taller than a page are preserved across page breaks, not silently clipped.
- **Absolute positioning**: `position: 'absolute'` with `top`, `right`, `bottom`, `left` relative to the parent View.
- **Column flex**: `justifyContent` and `alignItems` work in both row and column directions.
- **SVG**: Inline SVG rendering with support for `rect`, `circle`, `ellipse`, `line`, `polyline`, `polygon`, and `path` elements.
- **Custom fonts**: TrueType font embedding with automatic subsetting.
- **Links**: Add `href` to any `<Text>`, `<View>`, `<Image>`, or `<Svg>` for clickable PDF links.
- **Bookmarks**: Add `bookmark` to any element for PDF outline entries. Navigate long documents from the bookmark panel.
- **Inline text styling**: Nest `<Text>` inside `<Text>` to bold a word, change colors mid-sentence, or apply strikethrough.
- **Images**: JPEG, PNG, and WebP with transparency support. `alt` text for accessibility.
- **CSS shorthands**: `border: "1px solid #000"`, `padding: "8 16"`, `margin: [20, 40]` — CSS-style shorthand strings and arrays parse automatically.
- **Document language**: `<Document lang="en-US">` sets the PDF `/Lang` tag for accessibility.
- **Dynamic page numbers**: `{{pageNumber}}` and `{{totalPages}}` in any text element.

## Custom Fonts

Register TrueType fonts globally or per-document:

```tsx
import { Font, Document, Text } from '@formepdf/react';
import { renderDocument } from '@formepdf/core';

// Global registration (works like react-pdf)
Font.register({
  family: 'Inter',
  src: './fonts/Inter-Regular.ttf',
});

Font.register({
  family: 'Inter',
  src: './fonts/Inter-Bold.ttf',
  fontWeight: 'bold',
});

const pdf = await renderDocument(
  <Document>
    <Text style={{ fontFamily: 'Inter', fontSize: 16 }}>
      Regular text
    </Text>
    <Text style={{ fontFamily: 'Inter', fontSize: 16, fontWeight: 'bold' }}>
      Bold text
    </Text>
  </Document>
);
```

Or pass fonts directly on the Document:

```tsx
<Document fonts={[
  { family: 'Roboto', src: './fonts/Roboto-Regular.ttf' },
  { family: 'Roboto', src: './fonts/Roboto-Italic.ttf', fontStyle: 'italic' },
]}>
```

Font sources can be file paths, data URIs, or `Uint8Array`. Fonts are automatically subsetted — only glyphs used in the document are embedded.

## Components

| Component | Description |
|-----------|-------------|
| `<Document>` | Root element. `title`, `author`, `lang`, `fonts`. |
| `<Page>` | A page. `size`, `margin` (number, string, array, or edges). |
| `<View>` | Container. Flexbox layout. `href`, `bookmark`. |
| `<Text>` | Text content. Fonts, sizes, colors. `href`, `bookmark`. |
| `<Image>` | JPEG or PNG. `href`, `alt`. Aspect ratio preserved. |
| `<Table>` | Table with column definitions. |
| `<Row>` | Table row. `header` for repeating on page breaks. |
| `<Cell>` | Table cell. `colSpan`, `rowSpan`. |
| `<Svg>` | Inline SVG graphics. `href`, `alt`. |
| `<Fixed>` | Repeating header or footer. |
| `<PageBreak>` | Force a page break. |

## Comparison

| | Forme | react-pdf | Puppeteer |
|---|---|---|---|
| Page breaks | Page-native (widow/orphan aware) | Broken for 7 years | CSS `page-break` (fragile) |
| Table header repetition | Automatic on every page | Not built in | Inconsistent `<thead>` |
| Line breaking | Knuth-Plass optimal (TeX algorithm) | Greedy | Browser engine |
| Hyphenation | 35+ languages, automatic | Via callback | Browser engine |
| Text shaping | OpenType GSUB/GPOS (ligatures, kerning) | Basic | Full browser shaping |
| BiDi text | RTL, mixed LTR/RTL, auto-detection | No | Full browser BiDi |
| CSS Grid | `display: 'grid'` with fr/auto/fixed tracks | No | Full CSS Grid |
| Live preview | Built-in dev server | Render to file | Run script, open file |
| Click-to-inspect | VS Code, Cursor, WebStorm | No | No |
| Render speed | ~28ms (4-page report) | ~100-500ms | ~1-5s (Chrome boot) |
| Memory per render | No browser process (WASM) | ~50-100MB | ~50-200MB |
| SVG | Basic shapes and paths | Yes | Full browser SVG |
| Links | `href` prop on Text/View/Image/Svg | `<Link>` component | HTML `<a>` tags |
| Bookmarks | `bookmark` prop on any element | Yes | No |
| Custom fonts | TTF with OpenType shaping | Yes | Yes |
| Dependencies | None (WASM) | yoga-layout | Chrome/Chromium |
| Runs in-process | Yes | Yes | No (subprocess) |

## Templates

See the [templates/](./templates) directory for production-ready examples:
- Invoice
- Product Catalog
- Receipt
- Report
- Shipping Label

## Documentation

Full docs at [docs.formepdf.com](https://docs.formepdf.com)

## Contributing

Issues and PRs welcome.

## License

MIT
