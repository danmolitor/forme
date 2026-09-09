// FAILS-FIRST — the product's promise, written before the component exists.
//
// The whole point of @formepdf/preview is that what you see in the browser is
// what the server produces. This test locks that: it renders the SAME
// (html, options) through the server path (@formepdf/html Node entry) and
// through the browser path (the /worker entry — the in-Node proxy for the
// browser WASM, which embeds the byte-identical module, hash-asserted by the
// html package's own targets-determinism gate), and requires byte-identical
// PDFs. If anyone ever gives the preview a divergent options default or an
// extra transform, this goes red — which is exactly when it should.

import { describe, it, expect, beforeAll } from 'vitest';
import { readFileSync } from 'node:fs';
import { createRequire } from 'node:module';

import { renderHtml as renderServer } from '@formepdf/html';
import { init as initBrowserProxy, renderHtml as renderBrowserProxy } from '@formepdf/html/worker';

// The unit under test — does not exist yet (this is the fails-first import).
import { renderForPreview, fontFingerprint, standardFonts } from '../src/render.js';

const require = createRequire(import.meta.url);
const WASM = require.resolve('@formepdf/html/pkg-web/forme_pdf_html_bg.wasm');

// A small doc that references custom-registered families, so the font
// embedding path is exercised (not just default base-14 metrics).
const HTML = `<!doctype html><html lang="en"><head><style>
  body { font-family: 'Liberation Sans', sans-serif; font-size: 12pt; margin: 24pt; }
  h1 { font-family: 'Liberation Serif', serif; font-size: 20pt; }
  table { width: 100%; border-collapse: collapse; margin-top: 12pt; }
  td, th { border: 1px solid #333; padding: 4pt; text-align: left; }
</style></head><body>
  <h1>Invoice INV-001</h1>
  <p>Prepared for Mandare. Amounts in EUR.</p>
  <table>
    <thead><tr><th>Item</th><th>Qty</th><th>Amount</th></tr></thead>
    <tbody>
      <tr><td>Rendering seat</td><td>3</td><td>€30.00</td></tr>
      <tr><td>Support</td><td>1</td><td>€99.00</td></tr>
    </tbody>
  </table>
</body></html>`;

beforeAll(async () => {
  await initBrowserProxy(readFileSync(WASM));
});

describe('preview ↔ server parity', () => {
  it('renderForPreview (browser path) is byte-identical to server renderHtml', async () => {
    // ONE options object — the same one a server handler would pass. Fonts are
    // bytes (the only form the browser HTML path accepts), from the shared
    // standardFonts() adapter, so both sides embed the identical glyph program.
    const options = { fonts: standardFonts() };

    const server = await renderServer(HTML, options);
    const preview = await renderForPreview(HTML, options, renderBrowserProxy);

    expect(preview.pdf.length).toBe(server.pdf.length);
    expect(Buffer.from(preview.pdf).equals(Buffer.from(server.pdf))).toBe(true);
  });

  it('renderForPreview does not mutate or default the options object', async () => {
    const options = { fonts: standardFonts() };
    const before = JSON.stringify({ ...options, fonts: options.fonts.map((f) => f.family) });
    await renderForPreview(HTML, options, renderBrowserProxy);
    const after = JSON.stringify({ ...options, fonts: options.fonts.map((f) => f.family) });
    expect(after).toBe(before); // no previewOptions: the object it renders is the object it was given
  });
});

describe('font fingerprint — converts the silent case to a loud one', () => {
  it('is order-independent and content-sensitive', () => {
    const a = standardFonts();
    const aReordered = [...a].reverse();
    expect(fontFingerprint(a)).toBe(fontFingerprint(aReordered)); // same set, any order → same print

    const bMissingOne = a.slice(0, a.length - 1); // server registered one more font than the preview
    expect(fontFingerprint(a)).not.toBe(fontFingerprint(bMissingOne));
  });

  it('distinguishes different bytes under the same family/weight/style', () => {
    const a = standardFonts();
    const tampered = a.map((f, i) =>
      i === 0 ? { ...f, data: new Uint8Array([...(f.data as Uint8Array), 0]) } : f,
    );
    expect(fontFingerprint(a)).not.toBe(fontFingerprint(tampered));
  });

  it('treats undefined and empty font sets identically and stably', () => {
    expect(fontFingerprint(undefined)).toBe(fontFingerprint([]));
    expect(fontFingerprint([])).toBe(fontFingerprint([]));
  });
});
