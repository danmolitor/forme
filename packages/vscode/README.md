# Forme PDF Preview

Live PDF preview for [Forme](https://formepdf.com) templates. Edit JSX, see the rendered PDF instantly — without leaving VS Code.

![Forme PDF Preview showing split pane with component tree, inspector, and live PDF](https://raw.githubusercontent.com/danmolitor/forme/main/packages/vscode/screenshot.png)

## Features

- Live preview updates as you type (400ms debounce) or on save (immediate)
- Component tree in the VS Code sidebar
- Inspector with box model, layout, spacing, and typography properties
- Click any element to jump to source line
- Click any element in the PDF to select in tree and inspector
- Preview / Layout / Margins / Breaks overlay modes
- Zoom controls

## Usage

Open any `.tsx` file that imports from `@formepdf/react` and run:

**Forme: Preview to Side** from the command palette (`Cmd+Shift+P`)

## Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `1` | Preview mode |
| `2` | Layout mode |
| `3` | Margins mode |
| `4` | Breaks mode |
| `Cmd+` / `Cmd-` | Zoom in/out |
| `Cmd+0` | Fit to width |

## Settings

| Setting | Default | Description |
|---------|---------|-------------|
| `forme.autoOpen` | `false` | Automatically open the preview when opening a Forme template file |

## Requirements

Your project needs `@formepdf/react` and `@formepdf/core` installed.

```bash
npm install @formepdf/react @formepdf/core
```

## Documentation

Full docs at [docs.formepdf.com](https://docs.formepdf.com/vscode)

## License

MIT
