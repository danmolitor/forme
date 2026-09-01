// Workerd smoke test for the @formepdf/html/worker entry. Runs entirely inside
// workerd via @cloudflare/vitest-pool-workers, so it exercises the real
// Cloudflare WASM-as-ESM path: `import wasm from '…/forme_pdf_html_bg.wasm'`
// yields a WebAssembly.Module, and the web-target glue must load and render
// without the top-level-init crash that the bundler target hits under workerd.

import { describe, expect, it } from 'vitest';

// A representative invoice: a table with a header row and a spanning total, so
// the render exercises real layout, not just a blank page.
const INVOICE = `<!DOCTYPE html>
<html lang="en"><head><style>
  @page { size: A4; margin: 48pt }
  body { font-family: Helvetica }
  table { width: 100%; border-collapse: collapse }
  th, td { border: 1px solid #333; padding: 6px; text-align: left }
  .total { text-align: right; font-weight: bold }
</style></head>
<body>
  <h1>Invoice INV-2048</h1>
  <table>
    <thead><tr><th>Item</th><th>Qty</th><th>Price</th></tr></thead>
    <tbody>
      <tr><td>Widget</td><td>3</td><td>$30.00</td></tr>
      <tr><td>Gadget</td><td>1</td><td>$45.00</td></tr>
      <tr><td class="total" colspan="2">Total</td><td>$75.00</td></tr>
    </tbody>
  </table>
</body></html>`;

describe('@formepdf/html/worker', () => {
  it('module load does not throw under workerd', async () => {
    const mod = await import('../worker.js');
    expect(typeof mod.init).toBe('function');
    expect(typeof mod.renderHtml).toBe('function');
    expect(typeof mod.renderHtmlWithLayout).toBe('function');
  });

  it('init(wasmModule) + renderHtml produces a valid PDF', async () => {
    const { init, renderHtml } = await import('../worker.js');
    // @ts-expect-error -- *.wasm import shape is provided by workerd at runtime
    const wasm = (await import('../pkg-web/forme_pdf_html_bg.wasm')).default;
    await init(wasm);

    const { pdf, warnings } = renderHtml(INVOICE, {});
    expect(pdf.byteLength).toBeGreaterThan(500);
    expect(String.fromCharCode(...pdf.slice(0, 5))).toBe('%PDF-');
    expect(Array.isArray(warnings)).toBe(true);
  });

  it('renderHtmlWithLayout returns paginated layout + warnings', async () => {
    const { init, renderHtmlWithLayout } = await import('../worker.js');
    // @ts-expect-error -- runtime-provided WASM module shape
    const wasm = (await import('../pkg-web/forme_pdf_html_bg.wasm')).default;
    await init(wasm);

    const { pdf, layout, warnings } = renderHtmlWithLayout(INVOICE, {});
    expect(String.fromCharCode(...pdf.slice(0, 5))).toBe('%PDF-');
    expect(layout.pages.length).toBeGreaterThanOrEqual(1);
    expect(Array.isArray(warnings)).toBe(true);
  });

  it('init is idempotent', async () => {
    const { init } = await import('../worker.js');
    // @ts-expect-error -- runtime-provided WASM module shape
    const wasm = (await import('../pkg-web/forme_pdf_html_bg.wasm')).default;
    await init(wasm);
    await init(wasm); // second call resolves without re-instantiating or throwing
    expect(true).toBe(true);
  });
});
