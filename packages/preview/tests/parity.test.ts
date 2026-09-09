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
import { renderForPreview, fontFingerprint, fontsMatch, standardFonts } from '../src/render.js';

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

// A hermetic corpus of diverse shapes, inline so the gate needs no external
// fixtures (a fresh CI checkout has everything). Rendering the WHOLE set — not
// one doc — is what makes this a gate rather than an anecdote: any
// preview-introduced divergence on any shape trips it. `page-counters` is the
// important one: `counter(page)`/`counter(pages)` drive the 2–3 pass
// sentinel-width path, the trickiest to keep byte-identical.
const LONG_ROWS = Array.from({ length: 60 }, (_, i) => `<tr><td>Row ${i + 1}</td><td>€${i}.00</td></tr>`).join('');
const CORPUS: Array<[string, string]> = [
  ['invoice-table', HTML],
  [
    'multipage',
    `<!doctype html><html lang="en"><head><style>
       body { font-family: 'Liberation Sans', sans-serif; font-size: 11pt; margin: 20pt; }
       table { width: 100%; border-collapse: collapse; }
       td { border-bottom: 1px solid #ccc; padding: 3pt; }
     </style></head><body><table><tbody>${LONG_ROWS}</tbody></table></body></html>`,
  ],
  [
    'page-counters',
    `<!doctype html><html lang="en"><head><style>
       @page { margin: 40pt; @bottom-center { content: "Page " counter(page) " of " counter(pages); } }
       body { font-family: 'Liberation Serif', serif; font-size: 11pt; }
       p { margin: 0 0 8pt; }
     </style></head><body>${Array.from({ length: 40 }, (_, i) => `<p>Paragraph ${i + 1}. The quick brown fox jumps over the lazy dog, repeatedly, to fill the page and force breaks.</p>`).join('')}</body></html>`,
  ],
  [
    'borders-backgrounds',
    `<!doctype html><html lang="en"><head><style>
       body { font-family: 'Liberation Sans', sans-serif; margin: 24pt; }
       .card { background: #eef2ff; border: 2px solid #6366f1; border-radius: 6px; padding: 12pt; margin-bottom: 10pt; }
       .muted { color: #6b7280; }
     </style></head><body>
       <div class="card"><strong>Statement</strong><p class="muted">Balance carried forward.</p></div>
       <div class="card">Line item — rendering seat.</div>
     </body></html>`,
  ],
];

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

  // The gate: every corpus doc, preview bytes === server bytes.
  for (const [name, html] of CORPUS) {
    it(`corpus parity: ${name} — preview bytes === server bytes`, async () => {
      const options = { fonts: standardFonts() };
      const server = await renderServer(html, options);
      const preview = await renderForPreview(html, options, renderBrowserProxy);
      expect(Buffer.from(preview.pdf).equals(Buffer.from(server.pdf))).toBe(true);
    });
  }

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

describe('fontsMatch — the mismatch banner decision', () => {
  it('fires (returns false) when the preview fonts differ from the server print', () => {
    const server = standardFonts();
    const serverPrint = fontFingerprint(server);
    const previewMissingOne = server.slice(0, server.length - 1);
    expect(fontsMatch(previewMissingOne, serverPrint)).toBe(false); // banner shows
    expect(fontsMatch(server, serverPrint)).toBe(true); // banner hidden
  });

  it('no expected fingerprint means no expectation → always matches', () => {
    expect(fontsMatch(undefined, undefined)).toBe(true);
    expect(fontsMatch(standardFonts(), undefined)).toBe(true);
  });
});
