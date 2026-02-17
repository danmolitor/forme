# forme

PDF generation with JSX. Page breaks that actually work.

[screenshot/GIF placeholder, founder will add this]

## Why

Every PDF tool makes you choose: fight with CSS page breaks (react-pdf, Puppeteer) or use a drag-and-drop editor that can't handle dynamic data. Forme is a layout engine built for pages from the ground up. No infinite canvas. No hoping Chrome respects your `page-break-inside: avoid`.

## Quick Start

```bash
npm install forme @forme/react @forme/core
```

```tsx
import { Document, Page, View, Text } from '@forme/react';
import { renderDocument } from '@forme/core';

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

[dev server screenshot placeholder]

## Features

- **Page-native layout**: Content flows into pages, not onto an infinite canvas. Page breaks happen at the right place, every time.
- **React components**: Document, Page, View, Text, Image, Table. If you know React, you know Forme.
- **Live preview**: `forme dev` shows your PDF updating in real time as you edit.
- **Click-to-inspect**: Select any element to see its box model, computed styles, and position.
- **Debug overlays**: Toggle bounding boxes, margins, and page break points.
- **Fast**: Rust engine compiled to WASM. Renders in milliseconds, not seconds.
- **Custom fonts**: TrueType font embedding with automatic subsetting.
- **Images**: JPEG and PNG with transparency support.
- **Dynamic page numbers**: `{{pageNumber}}` and `{{totalPages}}` in any text element.

## Components

| Component | Description |
|-----------|-------------|
| `<Document>` | Root element. Contains pages. |
| `<Page>` | A page. Size, margins, orientation. |
| `<View>` | Container. Flexbox layout. |
| `<Text>` | Text content. Fonts, sizes, colors. |
| `<Image>` | JPEG or PNG. Aspect ratio preserved. |
| `<Table>` | Table with column definitions. |
| `<Row>` | Table row. |
| `<Cell>` | Table cell. |
| `<Fixed>` | Repeating header or footer. |
| `<PageBreak>` | Force a page break. |

## Comparison

| | Forme | react-pdf | Puppeteer | HTML-to-PDF APIs |
|---|---|---|---|---|
| Page breaks | Page-native | Broken for 7 years | CSS `page-break` (fragile) | Depends on engine |
| Live preview | Built-in dev server | Render to file | Run script, open file | Upload, wait, download |
| Element inspector | Click-to-inspect | No | No | No |
| Render speed | ~10ms (WASM) | ~100-500ms | ~1-5s (Chrome boot) | Network round trip |
| Custom fonts | TTF with subsetting | Yes | Yes | Varies |
| Dependencies | None (WASM) | yoga-layout | Chrome/Chromium | External service |
| Runs in-process | Yes | Yes | No (subprocess) | No (HTTP API) |

## Templates

See the [templates/](./templates) directory for production-ready examples:
- Invoice
- Receipt
- Report
- Shipping Label

## Documentation

Full docs at [formepdf.com/docs](https://formepdf.com/docs)

## License

MIT
