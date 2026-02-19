# @formepdf/cli

Dev server and build tool for [Forme](https://github.com/formepdf/forme) PDF generation.

## Install

```bash
npm install -D @formepdf/cli
```

## Commands

### `forme dev`

Live preview dev server with element inspector, component tree, click-to-source, and responsive page size switching.

```bash
forme dev invoice.tsx --data invoice-data.json
```

Open `http://localhost:4242` to see your PDF update in real time as you edit.

### `forme build`

Render a PDF to a file.

```bash
forme build invoice.tsx --data invoice-data.json -o invoice.pdf
```

## Dev server features

- **Live preview** with hot reload on file changes
- **Element inspector** with box model, computed styles, and breadcrumb navigation
- **Component tree** showing document structure
- **Click-to-source** opens your editor (VS Code, Cursor, WebStorm, Zed) at the exact line
- **Responsive preview** with page size switching (Letter, A4, Legal, Tabloid, custom)
- **Data editor** for live JSON editing when using `--data`
- **Debug overlays** for margins, padding, and element bounds

## Options

```
forme dev <template>         Start dev server
  --data <file>              Load JSON data file
  --port <number>            Server port (default: 4242)

forme build <template>       Render PDF
  --data <file>              Load JSON data file
  -o, --output <file>        Output file path
```

## Docs

Full documentation at [docs.formepdf.com](https://docs.formepdf.com)
