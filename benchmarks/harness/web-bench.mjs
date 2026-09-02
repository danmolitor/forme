// Web-WASM cold start + peak linear memory, measured inside a real Chromium
// page (puppeteer-core -> system Chrome). Each corpus doc renders in a FRESH
// page (fresh isolate) so its own peak WASM linear memory is isolated. Cold
// start is decomposed: module fetch, compile+instantiate, first render — so
// the ~7.1MB module cost is a visible line, not hidden in a total.
import http from 'node:http';
import { readFileSync, statSync } from 'node:fs';
import { extname, join } from 'node:path';
import puppeteer from 'puppeteer-core';

const ROOT = process.cwd();
const CHROME = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const MIME = { '.js':'text/javascript','.wasm':'application/wasm','.html':'text/html','.json':'application/json' };
const server = http.createServer((req, res) => {
  if (req.url === '/blank') { res.setHeader('content-type','text/html'); return res.end('<!doctype html><html><head><meta charset=utf-8></head><body></body></html>'); }
  try { const p = join(ROOT, decodeURIComponent(req.url.split('?')[0])); statSync(p);
    res.setHeader('content-type', MIME[extname(p)] || 'application/octet-stream');
    res.end(readFileSync(p));
  } catch { res.statusCode = 404; res.end('nf'); }
});
await new Promise(r => server.listen(0, r));
const PORT = server.address().port;
const base = `http://localhost:${PORT}`;

const browser = await puppeteer.launch({ executablePath: CHROME, headless: true, args: ['--no-sandbox','--js-flags=--max-old-space-size=4096'] });
const docs = ['receipt','report-6p','letterhead-paged','compliance','invoice-50p','ledger-500p'];
const modUrl = `${base}/packages/html/pkg-web/forme_pdf_html.js`;
const wasmUrl = `${base}/packages/html/pkg-web/forme_pdf_html_bg.wasm`;

console.log('doc            fetch  compile+inst  firstRender  passes  wasmMemMB  pdfKB');
for (const d of docs) {
  const page = await browser.newPage();
  await page.goto(`${base}/blank`);
  try {
    const r = await page.evaluate(async (modUrl, wasmUrl, docUrl) => {
      const t0 = performance.now();
      const resp = await fetch(wasmUrl); const bytes = await resp.arrayBuffer();
      const tFetch = performance.now() - t0;
      const mod = await import(modUrl);
      const t1 = performance.now();
      const wexports = await mod.default({ module_or_path: bytes });   // compile + instantiate
      const tInst = performance.now() - t1;
      const html = await (await fetch(docUrl)).text();
      const t2 = performance.now();
      const result = mod.render_html_wasm(html, '{}');
      const tRender = performance.now() - t2;
      const pdfLen = result.pdf.length, passes = result.passes;
      result.free();
      const memMB = wexports.memory.buffer.byteLength / 1048576;
      return { tFetch, tInst, tRender, passes, memMB, pdfLen };
    }, modUrl, wasmUrl, `${base}/benchmarks/corpus/${d}.html`);
    console.log(`${d.padEnd(14)} ${r.tFetch.toFixed(0).padStart(4)}  ${r.tInst.toFixed(0).padStart(11)}  ${r.tRender.toFixed(0).padStart(10)}  ${String(r.passes).padStart(5)}  ${r.memMB.toFixed(0).padStart(8)}  ${String(Math.round(r.pdfLen/1024)).padStart(5)}`);
  } catch (e) {
    console.log(`${d.padEnd(14)} ERROR: ${String(e.message).split('\n')[0].slice(0,60)}`);
  }
  await page.close();
}
await browser.close(); server.close();
