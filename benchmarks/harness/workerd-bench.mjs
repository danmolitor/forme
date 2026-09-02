// workerd cold start via miniflare. First request to a freshly started isolate
// (module eval + WASM instantiate + first render) vs warm requests.
//
// CAVEAT (published): miniflare-local runs a workerd subprocess, which adds
// process overhead the real Cloudflare edge isolate does not pay — so the
// external wall time OVERSTATES production edge cold start (true edge isolate
// spin-up is ~5ms). The instantiate/render split the worker reports itself is
// accurate; the wall time is a loose upper bound. Real production cold start
// needs a deployment, which is out of scope.
import { Miniflare } from 'miniflare';
import { readFileSync, mkdirSync, copyFileSync, writeFileSync } from 'node:fs';

// Self-setup: stage the web WASM + a minimal worker entry in a project-local
// dir (miniflare can't resolve a scriptPath that uses ".." to leave the root).
const WK = 'benchmarks/harness/_wk';
mkdirSync(WK, { recursive: true });
for (const f of ['forme_pdf_html.js', 'forme_pdf_html_bg.wasm']) {
  copyFileSync(`packages/html/pkg-web/${f}`, `${WK}/${f}`);
}
writeFileSync(
  `${WK}/entry.mjs`,
  `import initWasm, { render_html_wasm } from './forme_pdf_html.js';
import wasm from './forme_pdf_html_bg.wasm';
let ready = false;
export default {
  async fetch(req) {
    const t0 = Date.now();
    if (!ready) { await initWasm({ module_or_path: wasm }); ready = true; }
    const tInit = Date.now() - t0;
    const html = await req.text();
    const t1 = Date.now();
    const r = render_html_wasm(html, '{}');
    const tRender = Date.now() - t1;
    const passes = r.passes; r.free();
    return new Response(JSON.stringify({ tInit, tRender, passes }), { headers: { 'content-type': 'application/json' } });
  }
};
`,
);

const median = (a) => { const s = [...a].sort((x, y) => x - y); return s[Math.floor(s.length / 2)]; };
const read = (d) => readFileSync(`benchmarks/corpus/${d}.html`, 'utf8');
const makeMf = () => new Miniflare({
  scriptPath: `${WK}/entry.mjs`,
  modules: true,
  modulesRules: [
    { type: 'ESModule', include: ['**/*.js', '**/*.mjs'] },
    { type: 'CompiledWasm', include: ['**/*.wasm'] },
  ],
  compatibilityDate: '2024-09-01',
});

// COLD: fresh isolate, first request (receipt)
{
  const mf = makeMf();
  await mf.ready;
  const t0 = performance.now();
  const res = await mf.dispatchFetch('http://x/', { method: 'POST', body: read('receipt') });
  const body = await res.json();
  const wall = performance.now() - t0;
  console.log(`COLD (first request, receipt): wall ${wall.toFixed(0)}ms  [isolate wasm-instantiate ${body.tInit}ms + first render ${body.tRender}ms]  passes=${body.passes}`);
  console.log('  caveat: wall includes miniflare/workerd subprocess routing; real edge isolate spin-up is ~5ms\n');
  await mf.dispose();
}

// WARM: one isolate, reused
const mf = makeMf();
await mf.ready;
await mf.dispatchFetch('http://x/', { method: 'POST', body: read('receipt') });
console.log('WARM (isolate reused):');
console.log('doc            render(worker)  wall   passes');
for (const [d, iters] of [['receipt', 15], ['report-6p', 15], ['letterhead-paged', 15], ['compliance', 15], ['invoice-50p', 5], ['ledger-500p', 1]]) {
  const html = read(d);
  try {
    const rt = [], wt = []; let passes = 0;
    for (let i = 0; i < iters; i++) {
      const t0 = performance.now();
      const res = await mf.dispatchFetch('http://x/', { method: 'POST', body: html });
      const b = await res.json();
      wt.push(performance.now() - t0); rt.push(b.tRender); passes = b.passes;
    }
    console.log(`${d.padEnd(14)} ${median(rt).toFixed(0).padStart(11)}ms ${median(wt).toFixed(0).padStart(6)}ms  ${passes}`);
  } catch (e) {
    console.log(`${d.padEnd(14)} ERROR: ${String(e.message).split('\n')[0].slice(0, 50)}`);
  }
}
await mf.dispose();
