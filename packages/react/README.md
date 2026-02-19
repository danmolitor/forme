# @formepdf/react

React components for [Forme](https://github.com/formepdf/forme) PDF generation.

## Install

```bash
npm install @formepdf/react @formepdf/core
```

## Usage

```tsx
import { Document, Page, View, Text, StyleSheet } from '@formepdf/react';
import { renderDocument } from '@formepdf/core';

const styles = StyleSheet.create({
  title: { fontSize: 24, fontWeight: 700, marginBottom: 12 },
  body: { fontSize: 10, lineHeight: 1.6 },
});

const doc = (
  <Document>
    <Page size="Letter" margin={54}>
      <Text style={styles.title}>Hello Forme</Text>
      <Text style={styles.body}>Page breaks that actually work.</Text>
    </Page>
  </Document>
);

const pdfBytes = await renderDocument(doc);
```

## Components

- `Document` - Root container
- `Page` - A page with size, margins, and orientation
- `View` - Flex container (like div)
- `Text` - Text content with font styling
- `Image` - JPEG and PNG images
- `Table`, `Row`, `Cell` - Tables with automatic header repetition
- `Fixed` - Fixed headers and footers
- `PageBreak` - Explicit page break
- `Svg` - Basic SVG rendering

## Docs

Full documentation at [docs.formepdf.com](https://docs.formepdf.com)
