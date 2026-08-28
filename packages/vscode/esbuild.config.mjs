import { build } from 'esbuild';
import { cpSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '../..');

// dist/forme_bg.wasm is where the bundled extension.js looks for WASM
// at runtime. esbuild inlines core/pkg-node/forme.js (the nodejs target
// CJS glue) into extension.js; that glue does
//   `${__dirname}/forme_bg.wasm` + require('fs').readFileSync
// and after bundling, __dirname is <ext>/dist/. Pre-0.10.1 the path
// was '..'/pkg/forme_bg.wasm, which is why we used to copy into pkg/.
mkdirSync(resolve(__dirname, 'dist/preview'), { recursive: true });

const corePkgDir = resolve(root, 'packages/core');
const htmlPkgDir = resolve(root, 'packages/html');
const rendererPkgDir = resolve(root, 'packages/renderer');

const corePkg = existsSync(join(corePkgDir, 'pkg-node'))
  ? corePkgDir
  : resolve(__dirname, 'node_modules/@formepdf/core');
const htmlPkg = existsSync(join(htmlPkgDir, 'pkg-node'))
  ? htmlPkgDir
  : resolve(__dirname, 'node_modules/@formepdf/html');
const rendererPkg = existsSync(join(rendererPkgDir, 'dist/preview'))
  ? rendererPkgDir
  : resolve(__dirname, 'node_modules/@formepdf/renderer');

cpSync(resolve(rendererPkg, 'dist/preview'), resolve(__dirname, 'dist/preview'), { recursive: true });

await build({
  entryPoints: [resolve(__dirname, 'src/extension.ts')],
  bundle: true,
  outfile: resolve(__dirname, 'dist/extension.js'),
  format: 'cjs',
  platform: 'node',
  target: 'node20',
  external: ['vscode', 'esbuild-wasm'],
  alias: { 'esbuild': 'esbuild-wasm' },
  sourcemap: true,
  define: {
    'import.meta.url': 'FORME_IMPORT_META_URL',
  },
  banner: {
    js: `const FORME_IMPORT_META_URL = require('url').pathToFileURL(__filename).href;`,
  },
});

// Drop the WASM next to the bundled extension.js. esbuild only writes
// the outfile, so we do this post-build to avoid any ordering surprises.
cpSync(
  resolve(corePkg, 'pkg-node/forme_bg.wasm'),
  resolve(__dirname, 'dist/forme_bg.wasm'),
);

// The HTML input path is a second, sibling WASM blob — core consumers must
// not pay for html5ever/cssparser, so it stays out of the core bundle. The
// nodejs glue for @formepdf/html loads `${__dirname}/forme_pdf_html_bg.wasm`,
// which resolves to dist/ after esbuild inlines it into extension.js.
cpSync(
  resolve(htmlPkg, 'pkg-node/forme_pdf_html_bg.wasm'),
  resolve(__dirname, 'dist/forme_pdf_html_bg.wasm'),
);

console.log('Built extension');
