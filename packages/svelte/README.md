# @formepdf/svelte

Svelte components for [Forme](https://github.com/formepdf/forme) PDF generation.

## Install

```bash
npm install @formepdf/svelte @formepdf/core
```

Requires Svelte 5 (`^5.30.0`) as a peer dependency.
`@formepdf/core` is an optional peer: it is only needed to render PDF bytes locally (`renderDocument`, the preview helper).
Serializing for the hosted API works without it.

## Usage

Templates are ordinary `.svelte` files - `{#each}`, `{#if}`, snippets, and text interpolation all work.
`Invoice.svelte`:

```svelte
<script lang="ts">
  import { Document, Page, Text } from '@formepdf/svelte';

  let { name = 'World' }: { name?: string } = $props();
</script>

<Document>
  <Page size="Letter" margin={54}>
    <Text style={{ fontSize: 24, fontWeight: 700, marginBottom: 12 }}>Hello {name}</Text>
    <Text style={{ fontSize: 10, lineHeight: 1.6 }}>Page breaks that actually work.</Text>
  </Page>
</Document>
```

Render it in a SvelteKit endpoint:

```ts
import { renderDocument } from '@formepdf/svelte';
import Invoice from '$lib/Invoice.svelte';

export async function GET() {
  const pdf = await renderDocument(Invoice, { props: { name: 'Forme' } });
  return new Response(pdf, { headers: { 'Content-Type': 'application/pdf' } });
}
```

## Components

The same components with the same props as `@formepdf/react`.

### Layout
- `Document` - Root container (fonts, metadata, tagged PDF, PDF/A, certification)
- `Page` - A page with size, margins, and orientation
- `View` - Flex container (like div)
- `Text` - Text content with font styling
- `H1`-`H6` - Semantic headings with per-level default styling and tagged-PDF roles
- `Strong`, `Em`, `Code`, `Link` - Inline formatting inside `Text` and headings
- `OrderedList`, `UnorderedList`, `ListItem` - Numbered and bulleted lists
- `Image` - JPEG, PNG, and WebP images
- `Fixed` - Fixed headers and footers
- `PageBreak` - Explicit page break

### Tables
- `Table`, `Row`, `Cell` - Tables with automatic header repetition across pages

### Graphics
- `Svg` - SVG rendering via `content` string
- `QrCode` - Vector QR codes
- `Barcode` - 1D barcodes (Code 128, Code 39, EAN-13, EAN-8, Codabar)
- `Canvas` - Arbitrary vector drawing via callback API (the `draw` callback runs during serialization and must be synchronous and pure)
- `Watermark` - Rotated text behind page content

### Charts
- `BarChart` - Vertical bar charts with grouped series
- `LineChart` - Line charts with multiple series
- `PieChart` - Pie and donut charts
- `AreaChart` - Filled area charts
- `DotPlot` - Dot plot with grouped data points

### Form Fields
- `TextField` - Text input field
- `Checkbox` - Checkbox with label
- `Dropdown` - Select dropdown with options
- `RadioButton` - Radio button with group support

## API

- `serialize(Template, { props })` - template to Forme document object
- `render(Template, { props })` - template to Forme JSON string
- `renderToObject(Template, { props })` - alias of `serialize` mirroring the react API
- `renderDocument(Template, { props, ...renderOptions })` - template to PDF bytes (requires `@formepdf/core`); forwards core render options like `embedData` and `flattenForms`
- `renderDocumentWithLayout(Template, options)` - PDF bytes plus layout info for overlays
- `formePreview(Template, options)` (from `@formepdf/svelte/preview`) - GET handler for a SvelteKit catch-all route serving the live preview UI
- `Font.register()`, `StyleSheet.create()` - identical to the react adapter
- `PAGE_NUMBER`, `TOTAL_PAGES` - page-number placeholder constants (`{{pageNumber}}` cannot be typed literally in Svelte markup)

Compiled templates (`forme build --template`) are TSX-only today.

## Docs

Full documentation at [docs.formepdf.com](https://docs.formepdf.com)
