# @formepdf/fonts-standard

Embeddable, metric-compatible replacements for the 14 standard PDF fonts, for producing **accessible (PDF/UA)** and **archival (PDF/A)** documents with [Forme](https://github.com/formepdf/forme).

PDF/UA and PDF/A both require every font to be embedded. The 14 standard PDF fonts (Helvetica, Times, Courier, …) are *not* embedded — viewers substitute them — so a document that relies on them cannot conform. This package provides the [Liberation](https://github.com/liberationfonts/liberation-fonts) family (Sans/Serif/Mono), which is **metric-compatible** with Helvetica/Times/Courier by design: substituting it changes nothing about layout. Forme lays out on the standard font metrics and swaps only the embedded glyph program at write time, so `pdfUa` output is geometry-identical to the default.

## Install

```bash
npm install @formepdf/fonts-standard
```

## Usage

Register the fonts, then enable `pdfUa` — Forme maps the standard base-14 families to the embedded Liberation programs automatically.

```ts
import { Font } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';

for (const font of standardFonts()) Font.register(font);

// Now render with { pdfUa: true } — Helvetica/Times/Courier embed as
// Liberation Sans/Serif/Mono; layout is unchanged.
```

If `pdfUa` is set and a standard font is used but this package is **not** registered, Forme emits the document but warns by name with the remedy (install and register `@formepdf/fonts-standard`) — never silently.

## Exports

- `standardFonts(): FontSpec[]` — the 12 Liberation fonts (Sans/Serif/Mono × Regular/Bold/Italic/BoldItalic) as registrations with `Uint8Array` buffers. No file IO — works in Node, WASM, and the browser.
- `BASE14_ALIASES` — base-14 family → Liberation family, the same mapping the engine uses. Symbol and ZapfDingbats have no metric-compatible substitute and are omitted.

## PDF/A note

For **PDF/UA-1**, the Liberation programs are embedded with the standard fonts' AFM widths, which is fully conformant. For **PDF/A-2b**, ISO 19005's font-consistency clause requires declared widths to agree with the embedded program's advances; Liberation matches the AFM widths for the entire common glyph set, with a small enumerated set of rare accent/symbol glyphs (per proportional family) where Forme declares the program's actual advance instead. See the engine's PDF/A width carve-out.

## License

The Liberation fonts are redistributed **unmodified** under the **SIL Open Font License, Version 1.1** — see [`OFL.txt`](./OFL.txt) and [`AUTHORS`](./AUTHORS).

> Digitized data copyright © 2010 Google Corporation with Reserved Font Arimo, Tinos and Cousine.
> Copyright © 2012 Red Hat, Inc. with Reserved Font Name Liberation.

The package's own code is MIT-compatible; the distributed font binaries are OFL-1.1.
