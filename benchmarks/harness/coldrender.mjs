// Cold-start probe: a fresh process that instantiates the node WASM and renders
// one document. Run as `node benchmarks/harness/coldrender.mjs <doc.html>` from
// the repo root. Prints instantiate/render split + passes to stdout.
import { readFileSync } from 'node:fs';
const t0 = performance.now();
const { renderHtml } = await import('@formepdf/html'); // self-instantiates the WASM at import
const t1 = performance.now();
const html = readFileSync(process.argv[2], 'utf8');
const out = renderHtml(html, {});
const t2 = performance.now();
process.stdout.write(JSON.stringify({ instantiate: Math.round(t1 - t0), render: Math.round(t2 - t1), passes: out.passes }));
