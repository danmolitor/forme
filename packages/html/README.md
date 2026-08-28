# @formepdf/html

HTML + print-CSS to PDF on the [Forme](https://formepdf.com) engine.
No headless browser, no Chromium cold start — a Rust layout engine
compiled to WASM, running wherever Node runs.

```
npx @formepdf/html invoice.html
```

```js
const { renderHtml } = require('@formepdf/html');

const { pdf, warnings } = renderHtml(html, { pageSize: 'Letter' });
fs.writeFileSync('invoice.pdf', pdf);
warnings.forEach((w) => console.warn(w)); // everything outside the subset, named
```

## What works

The paged-media CSS browsers took two decades to ship — and the parts
they still haven't:

- **`@page`**: size (named or dimensions), margins, `:first` variants
- **Margin boxes** (`@top-center`, `@bottom-right`, ...): running
  headers/footers in the page margin, with `content()` strings and
  `counter(page)` / `counter(pages)`
- **Break control**: `break-before/after/inside` plus the legacy
  `page-break-*` spellings every wkhtmltopdf-era template still carries
- **`orphans` / `widows`**, `<thead>` repetition on every page
- Stylesheets with a documented selector subset, the full cascade with
  `!important`, flexbox, tables with colspan/rowspan, lists, images

The complete property-by-property subset table lives in the
[repository README](https://github.com/formepdf/forme/tree/main/html) —
everything outside it lands in `warnings`, never silently dropped.

## CLI

```
forme-html <input.html> [-o out.pdf] [--css print.css]
           [--page-size Letter] [--margin 36] [-q]
```

CLI/API options override the document's `@page` rule, the way a print
dialog overrides a stylesheet.
