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
const TEMPLATES_DIR = join(PKG, '..', '..', 'templates');
const NORTHMOOR = [
  'invoice-standard',
  'invoice-detailed',
  'credit-note',
  'receipt',
  'quote',
  'statement',
  'purchase-order',
  'remittance-advice',
  'expense-report',
  'payslip',
  'letterhead',
  'cover-letter',
  'memo',
  'employment-contract',
  'nda',
  'service-agreement',
  'offer-letter',
  'termination-letter',
  'reference-letter',
  'policy-acknowledgement',
  'report-annual',
  'report-monthly',
  'lab-report',
  'inspection-report',
  'shipping-label',
  'packing-slip',
  'delivery-note',
  'certificate',
  'product-catalog',
  'meeting-minutes',
];
function loadNorthmoor(name) {
  const shared = readFileSync(join(TEMPLATES_DIR, 'northmoor-shared.css'), 'utf8');
  const own = readFileSync(join(TEMPLATES_DIR, name, 'style.css'), 'utf8');
  return readFileSync(join(TEMPLATES_DIR, name, 'index.html'), 'utf8')
    .replace('<link rel="stylesheet" href="../northmoor-shared.css">', `<style>${shared}</style>`)
    .replace('<link rel="stylesheet" href="style.css">', `<style>${own}</style>`);
}

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

// 4. The Northmoor template set, same gate.
for (const name of NORTHMOOR) {
  const html = loadNorthmoor(name);
  const node = renderNode(html, {});
  const web = renderWorker(html, {});
  assert.strictEqual(sha(node.pdf), sha(web.pdf), `northmoor/${name}: node vs web bytes diverge`);
  assert.deepStrictEqual(node.warnings, web.warnings, `northmoor/${name}: warnings diverge`);
  assert.strictEqual(node.warnings.length, 0, `northmoor/${name}: templates must render warning-free: ${node.warnings}`);
}
console.log(`ok — northmoor: ${NORTHMOOR.length} templates identical across node + web, all warning-free`);

console.log(`ok — cross-target determinism: ${CORPUS.length} fixtures identical across node + web`);
