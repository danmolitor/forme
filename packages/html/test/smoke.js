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

// ── declared-type ↔ runtime shape (node target) ────────────────────────
// Exact key sets, both directions. The canonical lists are compile-checked
// against index.d.ts in tests/shape.ts (npm run typecheck); these mirror
// them for the plain-node runner.
import { renderHtmlWithLayout } from '../index.js';

const RESULT_KEYS = ['passes', 'pdf', 'warnings'];
const LAYOUT_KEYS = ['layout', 'passes', 'pdf', 'warnings'];

const shapeRes = renderHtml(html, {});
assert.deepStrictEqual(Object.keys(shapeRes).sort(), RESULT_KEYS,
  'node renderHtml must return exactly the declared RenderHtmlResult');
assert.ok(Number.isInteger(shapeRes.passes) && shapeRes.passes >= 1, 'passes is a count');

const shapeLay = renderHtmlWithLayout(html, {});
assert.deepStrictEqual(Object.keys(shapeLay).sort(), LAYOUT_KEYS,
  'node renderHtmlWithLayout must return exactly the declared RenderHtmlLayoutResult');
assert.ok(Number.isInteger(shapeLay.passes) && shapeLay.passes >= 1, 'layout passes is a count');
assert.strictEqual(shapeLay.passes, shapeRes.passes, 'both paths report the same count');

console.log('ok — result shapes match the declared types (node target)');

// ── embedStandardFonts reaches the engine (#197) ───────────────────────
// Through the real wire: the camelCase option must survive toWireOptions
// and the WASM options parser, or the fonts stay unembedded.
import { readFileSync } from 'node:fs';

const liberation = (file, weight) => ({
  family: 'Liberation Sans',
  data: new Uint8Array(readFileSync(new URL(`../../fonts-standard/fonts/${file}`, import.meta.url))),
  weight,
});
const plain = '<p><b>Invoice</b> total due</p>';
const substitutes = [liberation('LiberationSans-Regular.ttf', 400), liberation('LiberationSans-Bold.ttf', 700)];
const embedded = (bytes) => new TextDecoder('latin1').decode(bytes).split('/FontFile2').length - 1;

assert.strictEqual(embedded(renderHtml(plain, { fonts: substitutes }).pdf), 0, 'off by default');
const on = renderHtml(plain, { fonts: substitutes, embedStandardFonts: true });
assert.strictEqual(embedded(on.pdf), 2, 'regular and bold embedded');
assert.ok(!new TextDecoder('latin1').decode(on.pdf).includes('pdfuaid'), 'no PDF/UA claim');
const missing = renderHtml(plain, { embedStandardFonts: true });
assert.ok(missing.warnings.some((w) => w.startsWith('embedStandardFonts:')), `warns: ${missing.warnings}`);

console.log('ok — embedStandardFonts embeds the substitutes with no claim');
