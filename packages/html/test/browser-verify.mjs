// Real-browser verification for @formepdf/html.
//
// Serves the package over HTTP and loads the *web* build (worker.js →
// pkg-web/) inside a headless Chromium page — the exact path a browser
// consumer takes: `import { init, renderHtmlWithLayout } from
// '@formepdf/html/worker'` (or /browser), init the WASM, render. Asserts a
// %PDF header, a non-trivial byte length, and the expected page count from the
// returned LayoutInfo.
//
// Playwright is a TEST-ONLY harness invoked via npx — never a product
// dependency of the package.

import { readFileSync } from 'node:fs';
import { createServer } from 'node:http';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { dirname, join, extname } from 'node:path';
import { createRequire } from 'node:module';
import assert from 'node:assert';
import { renderHtmlWithLayout as renderNode } from '../index.js';

// Resolve playwright as a test-only harness — it is deliberately NOT a
// dependency of this package. Look in the local tree first, then common global
// locations (Homebrew, the default global prefix), then PLAYWRIGHT_PATH.
const require = createRequire(import.meta.url);
function loadPlaywright() {
  const searchPaths = [
    undefined, // local node_modules
    process.env.PLAYWRIGHT_PATH,
    '/opt/homebrew/lib/node_modules',
    '/usr/local/lib/node_modules',
    '/usr/lib/node_modules',
  ];
  for (const paths of searchPaths) {
    try {
      const p = paths
        ? require.resolve('playwright', { paths: [paths] })
        : require.resolve('playwright');
      return import(pathToFileURL(p).href);
    } catch {
      /* try next location */
    }
  }
  throw new Error(
    'playwright not found — install it (npm i -D playwright && npx playwright install chromium) ' +
      'or set PLAYWRIGHT_PATH to a node_modules dir',
  );
}
const pw = await loadPlaywright();
const chromium = pw.chromium ?? pw.default?.chromium;

const HERE = dirname(fileURLToPath(import.meta.url));
const PKG = join(HERE, '..');
const FIXTURES = join(PKG, '..', '..', 'html', 'tests', 'fixtures');
const fixture = readFileSync(join(FIXTURES, 'letterhead.html'), 'utf8');

const MIME = {
  '.js': 'text/javascript',
  '.mjs': 'text/javascript',
  '.wasm': 'application/wasm',
  '.json': 'application/json',
  '.html': 'text/html',
  '.ts': 'text/plain',
};

const page = `<!doctype html><meta charset="utf8">
<script id="fx" type="application/json">${JSON.stringify(fixture)}</script>
<script type="module">
  import { init, renderHtmlWithLayout } from '/worker.js';
  (async () => {
    try {
      await init('/pkg-web/forme_pdf_html_bg.wasm');
      const html = JSON.parse(document.getElementById('fx').textContent);
      const { pdf, layout, warnings } = renderHtmlWithLayout(html, {});
      window.__RESULT__ = {
        magic: new TextDecoder().decode(pdf.slice(0, 5)),
        length: pdf.length,
        pages: layout.pages.length,
        warnings: warnings.length,
      };
    } catch (e) {
      window.__RESULT__ = { error: String(e && e.stack || e) };
    }
  })();
</script>`;

const server = createServer((req, res) => {
  const url = req.url === '/' ? null : decodeURIComponent(req.url.split('?')[0]);
  if (url === null) {
    res.setHeader('content-type', 'text/html');
    res.end(page);
    return;
  }
  try {
    const buf = readFileSync(join(PKG, url));
    res.setHeader('content-type', MIME[extname(url)] ?? 'application/octet-stream');
    res.end(buf);
  } catch {
    res.statusCode = 404;
    res.end('not found');
  }
});

await new Promise((r) => server.listen(0, r));
const port = server.address().port;
const base = `http://127.0.0.1:${port}/`;

const browser = await chromium.launch();
let result;
try {
  const p = await browser.newPage();
  const errors = [];
  p.on('pageerror', (e) => errors.push(String(e)));
  await p.goto(base, { waitUntil: 'load' });
  await p.waitForFunction('window.__RESULT__ !== undefined', null, { timeout: 20000 });
  result = await p.evaluate('window.__RESULT__');
  if (errors.length) console.error('page errors:', errors);
} finally {
  await browser.close();
  server.close();
}

if (result?.error) {
  console.error('✗ browser render threw:\n' + result.error);
  process.exit(1);
}
// Expected page count is whatever the Node target produces for the same
// fixture — the browser must agree, not match a hand-picked number.
const expectedPages = renderNode(fixture, {}).layout.pages.length;

assert.strictEqual(result.magic, '%PDF-', `expected %PDF- header, got ${JSON.stringify(result.magic)}`);
assert.ok(result.length > 500, `PDF too small: ${result.length}`);
assert.strictEqual(
  result.pages,
  expectedPages,
  `browser page count ${result.pages} != node page count ${expectedPages}`,
);
console.log(
  `ok — headless Chromium rendered letterhead via the web build: ` +
    `${result.length}-byte %PDF, ${result.pages} page(s) (matches Node), ${result.warnings} warning(s)`,
);
