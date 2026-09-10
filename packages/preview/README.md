# @formepdf/preview

In-browser PDF preview for [Forme](https://formepdf.com). It renders your HTML
through the **same engine your server uses** and shows the **actual PDF** in an
iframe — so the preview and the produced file agree by construction, not by
approximation.

```tsx
import { FormePreview, standardFonts } from '@formepdf/preview';

// The SAME options object you pass to your server's renderHtml.
const options = { fonts: standardFonts() };

<FormePreview html={html} options={options} style={{ height: 600 }} />
```

## Read this first: bundle cost

This component renders in the browser, which means it loads the Forme WASM
engine — **~7.7 MB**, fetched on first preview (lazily, so it's not in your
initial bundle; cached by the browser thereafter). That is the unavoidable cost
of rendering-in-browser: it's what buys you a preview that can't disagree with
your server. If you only need to *show* a server-produced PDF and don't need
live in-browser rendering, you don't need this package — point an `<iframe>` at
your server's PDF.

## Why it agrees with your output

Three things could make a preview diverge from the produced PDF. This component
closes all three:

- **Display.** It shows the real PDF bytes in the browser's native viewer (blob
  → iframe) — no canvas rasterizer approximating the file. What you see is the
  file, in the same viewer your recipient opens.
- **Options.** There is no `previewOptions`. You pass the *same* `options`
  object to `<FormePreview>` and to your server's `renderHtml`. One
  configuration can't drift from itself.
- **Fonts.** See below — the one that bites silently.

## The font-parity contract

The silent failure: a template references a font the **server** embeds but the
**preview** doesn't, so each side renders differently with *no error*. Forme's
browser render path cannot fetch fonts — it takes bytes — so:

1. **Express fonts as bytes, once, for both sides.** Use `standardFonts()` for
   the embeddable base-14 replacements, or pass your own
   `{ family, data: Uint8Array | base64, weight, italic }`. Hand the same array
   to your server `renderHtml` and to `<FormePreview options>`.
2. **Make a mismatch loud.** Fingerprint what the server was given and pass it
   in; the preview shows a banner if the fonts it holds don't match:

   ```tsx
   import { fontFingerprint } from '@formepdf/preview';
   const print = fontFingerprint(options.fonts);   // compute where you build server options
   <FormePreview html={html} options={options} expectedFontFingerprint={print} />
   ```

## Bundler setup

The engine is self-instantiating WASM, so your bundler needs WASM + top-level
await support. For Vite, add `vite-plugin-wasm` and target `esnext` (which emits
native top-level await):

```ts
// vite.config.ts
import { defineConfig } from 'vite';
import wasm from 'vite-plugin-wasm';

export default defineConfig({
  plugins: [wasm()],
  build: { target: 'esnext' },
  optimizeDeps: { esbuildOptions: { target: 'esnext' } },
});
```

You can instead add `vite-plugin-top-level-await`, but it bundles `@swc/core`
and can break on some SWC versions — the `esnext` target is the more robust
route. (Framework equivalents exist for webpack/Next/etc.)

## Props

| Prop | Type | Notes |
|---|---|---|
| `html` | `string` | The document HTML — same string you render server-side. |
| `options` | `RenderHtmlOptions` | **The same object you pass to the server.** Keep it referentially stable (memoize). |
| `expectedFontFingerprint` | `string?` | Server's `fontFingerprint(fonts)`; shows a banner on mismatch. |
| `debounceMs` | `number?` | Debounce before re-render (default 150ms), for live-edit. |
| `onWarnings` | `(warnings: string[]) => void` | Engine warnings (missing PDF/UA fonts, dropped content, …) — never silent. |
| `onError` / `onRender` | callbacks | Error, and `{ passes, warnings, bytes }` on success. |
| `className` / `style` / `title` | — | Sizes and labels the iframe container. |

## What it does NOT do

Not an editor. Not an annotating or form-filling viewer. Not a replacement for a
full PDF viewer, and no toolbar of its own — you get the browser's native PDF
controls (scroll, zoom, print). It renders the HTML and shows the file, pages in
order. If you need programmatic zoom or custom page layout, that's a
canvas-based viewer (a documented approximation) — out of scope for a
fidelity-first component.

## Notes & caveats

- **v1 is the HTML input path.** JSX/React-element input is planned as a second
  input; the display and parity story are identical.
- **`blob:` under a strict CSP** needs `frame-src blob:`.
- **Environments without a native PDF viewer** (some embedded webviews) won't
  display the iframe; use `onRender`'s bytes to offer a download instead.

MIT.
