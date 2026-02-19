# @formepdf/core

WASM-powered PDF rendering engine for [Forme](https://github.com/formepdf/forme).

## Install

```bash
npm install @formepdf/react @formepdf/core
```

## Usage

```tsx
import { Document, Page, Text } from '@formepdf/react';
import { renderDocument } from '@formepdf/core';
import { writeFileSync } from 'fs';

const doc = (
  <Document>
    <Page size="Letter" margin={54}>
      <Text style={{ fontSize: 24 }}>Hello Forme</Text>
    </Page>
  </Document>
);

const pdfBytes = await renderDocument(doc);
writeFileSync('output.pdf', pdfBytes);
```

## What this package does

This package contains the compiled WASM binary of Forme's Rust layout engine. It handles:

- Page-native layout with automatic content splitting
- Font loading, subsetting, and TrueType embedding
- PDF generation with links, bookmarks, images, and SVG
- Table layout with automatic header repetition

You write components with `@formepdf/react`. This package turns them into PDF bytes.

## API

### `renderDocument(jsx)`

Takes a JSX document tree and returns a `Uint8Array` of PDF bytes.

### `renderDocumentWithLayout(jsx)`

Returns both the PDF bytes and layout metadata (element positions, sizes, page info). Used by the dev server for the element inspector.

## Docs

Full documentation at [docs.formepdf.com](https://docs.formepdf.com)
