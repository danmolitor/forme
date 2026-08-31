// Cross-target byte-determinism for @formepdf/html.
//
// All three wasm-pack targets embed the SAME compiled .wasm (asserted by hash
// below), so the compute is identical by construction and only the JS glue
// differs. This harness proves the glue too: it renders the four-fixture
// corpus through the Node target (index.js) and the web target (worker.js,
// the backing for @formepdf/html/worker) and requires byte-identical PDFs.
//
// The bundler target (pkg/) can't be loaded outside a bundler, so its
// determinism rests on the identical-wasm hash assertion plus the browser and
// Workers render checks; it is not byte-diffed here.

import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import { createHash as hash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

import { renderHtml as renderNode } from '../index.js';
import { init as initWorker, renderHtml as renderWorker } from '../worker.js';

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG = join(HERE, '..');
const FIXTURES = join(PKG, '..', '..', 'html', 'tests', 'fixtures');
const CORPUS = ['letterhead', 'report', 'zebra-invoice', 'dashed-borders'];

function sha(bytes) {
  return hash('sha256').update(bytes).digest('hex');
}

// 1. The three target wasm blobs must be byte-identical.
const wasmHashes = ['pkg', 'pkg-web', 'pkg-node'].map((d) =>
  sha(readFileSync(join(PKG, d, 'forme_pdf_html_bg.wasm'))),
);
assert.strictEqual(new Set(wasmHashes).size, 1, `wasm blobs diverge across targets: ${wasmHashes}`);
console.log(`ok — all 3 target wasm blobs identical (${wasmHashes[0].slice(0, 12)}…)`);

// 2. Initialize the web target from the same wasm bytes.
await initWorker(readFileSync(join(PKG, 'pkg-web', 'forme_pdf_html_bg.wasm')));

// 3. node-target vs web-target bytes, per fixture.
for (const name of CORPUS) {
  const html = readFileSync(join(FIXTURES, `${name}.html`), 'utf8');
  const node = renderNode(html, {});
  const web = renderWorker(html, {});
  assert.strictEqual(String.fromCharCode(...node.pdf.slice(0, 5)), '%PDF-', `${name}: node PDF magic`);
  assert.strictEqual(sha(node.pdf), sha(web.pdf), `${name}: node vs web bytes diverge`);
  assert.deepStrictEqual(node.warnings, web.warnings, `${name}: warnings diverge`);
  console.log(`ok — ${name}: node == web (${node.pdf.length} bytes, ${node.warnings.length} warning(s))`);
}

console.log(`ok — cross-target determinism: ${CORPUS.length} fixtures identical across node + web`);
