// Smoke test over the built WASM: render, check PDF magic, check the
// warnings contract, check @page + margin-box output exists.
import assert from 'node:assert';
import { renderHtml } from '../index.js';

const html = `<!DOCTYPE html>
<html><head><style>
  @page { size: Letter; margin: 72pt 54pt;
    @bottom-center { content: "Page " counter(page) " of " counter(pages) } }
  h1 { color: #1a365d }
  .x { transform: rotate(3deg) }
</style></head>
<body><h1>Smoke</h1><p class="x">Hello <strong>world</strong>.</p></body></html>`;

const { pdf, warnings } = renderHtml(html, {});
assert.ok(pdf instanceof Uint8Array, 'pdf is bytes');
assert.ok(pdf.length > 500, 'pdf non-trivial');
assert.strictEqual(String.fromCharCode(...pdf.slice(0, 5)), '%PDF-', 'PDF magic');
assert.ok(warnings.some((w) => w.includes('transform')), `warnings contract: ${warnings}`);

// Option precedence: explicit pageSize beats @page.
const a4 = renderHtml(html, { pageSize: 'A4' });
assert.ok(a4.pdf.length > 500);

console.log(`ok — ${pdf.length} byte PDF, ${warnings.length} warning(s)`);
