# Migrating from react-pdf

Forme and react-pdf share a similar component API, so migration is mostly renaming imports. This guide covers the differences and what to watch for.

## Component mapping

| react-pdf | Forme | Notes |
|-----------|-------|-------|
| `<Document>` | `<Document>` | Same |
| `<Page>` | `<Page>` | Same props, same behavior |
| `<View>` | `<View>` | Same |
| `<Text>` | `<Text>` | Same |
| `<Image>` | `<Image>` | Same API, supports file paths and data URIs |
| `<Link>` | Not yet supported | Use plain Text for now |
| `<Note>` | Not yet supported | - |
| `<Canvas>` | Not yet supported | - |
| `<SVG>` | Not yet supported | - |

## Import changes

**react-pdf:**
```tsx
import { Document, Page, View, Text, Image, StyleSheet } from '@react-pdf/renderer';
```

**Forme:**
```tsx
import { Document, Page, View, Text, Image } from '@forme/react';
```

Forme does not have a `StyleSheet.create()` utility. Styles are plain objects passed directly to the `style` prop. If you prefer to define styles separately, use a regular JavaScript object:

```tsx
const styles = {
  heading: { fontSize: 24, fontWeight: 700 },
  body: { fontSize: 10, lineHeight: 1.6 },
};

<Text style={styles.heading}>Title</Text>
```

## Style differences

Most style properties are the same between react-pdf and Forme. Key differences:

| Property | react-pdf | Forme |
|----------|-----------|-------|
| `fontWeight` | `"bold"` or number | `"bold"`, `"normal"`, or number (100-900) |
| `borderWidth` | Single number | Number or `{ top, right, bottom, left }` |
| `borderColor` | Single string | String or `{ top, right, bottom, left }` |
| `borderRadius` | Single number | Number or `{ topLeft, topRight, bottomRight, bottomLeft }` |
| `margin` | `"auto"` supported | `"auto"` not supported (use flexbox alignment instead) |
| `position` | `"absolute"` supported | Use `<Fixed>` for repeating elements. Absolute positioning is not supported. |

## Rendering

**react-pdf:**
```tsx
import { renderToBuffer } from '@react-pdf/renderer';
const buffer = await renderToBuffer(<MyDocument />);
```

**Forme:**
```tsx
import { renderDocument } from '@forme/core';
const pdfBytes = await renderDocument(<MyDocument />);
```

Both return PDF bytes. Forme returns a `Uint8Array`, react-pdf returns a Node.js `Buffer`.

## Tables

react-pdf does not have a built-in Table component. Most projects use `<View>` with row/column flex layout to simulate tables. Forme has first-class table support:

**react-pdf (common pattern):**
```tsx
<View style={{ flexDirection: 'row', borderBottom: '1px solid #ccc' }}>
  <View style={{ width: '50%', padding: 8 }}><Text>Name</Text></View>
  <View style={{ width: '50%', padding: 8 }}><Text>Price</Text></View>
</View>
```

**Forme:**
```tsx
<Table columns={[{ width: { fraction: 0.5 } }, { width: { fraction: 0.5 } }]}>
  <Row header>
    <Cell style={{ padding: 8 }}><Text>Name</Text></Cell>
    <Cell style={{ padding: 8 }}><Text>Price</Text></Cell>
  </Row>
</Table>
```

The Forme version gets automatic header repetition on page breaks and correct row-level page splitting.

## Fixed headers and footers

**react-pdf:**
```tsx
<Page>
  <View fixed style={{ position: 'absolute', top: 0, left: 0, right: 0 }}>
    <Text>Header</Text>
  </View>
</Page>
```

**Forme:**
```tsx
<Page>
  <Fixed position="header">
    <Text>Header</Text>
  </Fixed>
</Page>
```

In Forme, fixed elements automatically reduce the content area, so body content never overlaps with headers or footers. In react-pdf, you need to manually add padding to avoid overlap.

## Page numbers

**react-pdf:**
```tsx
<Text render={({ pageNumber, totalPages }) => `Page ${pageNumber} of ${totalPages}`} />
```

**Forme:**
```tsx
<Text>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
```

Both approaches produce the same result. Forme uses template placeholders instead of a render callback.

## What works better in Forme

1. **Page breaks.** Flex containers, tables, and text all break correctly across pages. In react-pdf, flex layouts produce incorrect proportions after page splits.

2. **Table header repetition.** Mark a row as `header` and it repeats on every page. No workarounds needed.

3. **Dev server.** `forme dev` gives you live preview with debug overlays and click-to-inspect. react-pdf requires rendering to a file and opening it manually.

4. **Speed.** Forme's Rust/WASM engine renders in milliseconds. react-pdf is typically 5-10x slower for complex documents.

5. **Dependencies.** Forme is a single WASM binary with no native dependencies. react-pdf depends on yoga-layout (native binary).

## What works in react-pdf but not Forme (yet)

Be honest about these gaps before migrating:

1. **SVG rendering.** react-pdf has full SVG support. Forme does not render SVG elements. Workaround: convert SVGs to PNG/JPEG before embedding.

2. **Links.** react-pdf supports clickable links with `<Link>`. Forme does not support PDF link annotations yet.

3. **Absolute positioning.** react-pdf supports `position: "absolute"`. Forme only supports fixed headers/footers via `<Fixed>`, not arbitrary absolute positioning.

4. **Bookmarks/outlines.** react-pdf can generate PDF bookmarks. Forme does not support this yet.

5. **Text run styling.** react-pdf supports inline styling within text (e.g., bold a single word). Forme treats each `<Text>` element as a single styled run.

6. **Emoji rendering.** react-pdf handles emoji via system fonts. Forme does not have special emoji support.

## Migration checklist

1. Replace `@react-pdf/renderer` imports with `@forme/react` and `@forme/core`
2. Remove `StyleSheet.create()` calls (keep the style objects, just remove the wrapper)
3. Replace `fixed` prop + absolute positioning with `<Fixed position="header">` or `<Fixed position="footer">`
4. Replace render callbacks for page numbers with `{{pageNumber}}` / `{{totalPages}}` placeholders
5. Convert View-based table layouts to `<Table>`, `<Row>`, `<Cell>` components
6. Replace any SVG usage with rasterized images
7. Remove any `<Link>` components (or replace with styled Text for now)
8. Test page break behavior, especially for tables and flex layouts
