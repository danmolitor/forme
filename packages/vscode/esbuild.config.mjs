import { build } from 'esbuild';
import { cpSync, mkdirSync, existsSync } from 'node:fs';
import { dirname, resolve, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = dirname(fileURLToPath(import.meta.url));
const root = resolve(__dirname, '../..');

// Copy WASM to pkg/ (core's ensureInit does join(__dirname, '..', 'pkg'))
// and preview HTML to dist/preview/
mkdirSync(resolve(__dirname, 'pkg'), { recursive: true });
mkdirSync(resolve(__dirname, 'dist/preview'), { recursive: true });

const corePkgDir = resolve(root, 'packages/core');
const rendererPkgDir = resolve(root, 'packages/renderer');

const corePkg = existsSync(join(corePkgDir, 'pkg'))
  ? corePkgDir
  : resolve(__dirname, 'node_modules/@formepdf/core');
const rendererPkg = existsSync(join(rendererPkgDir, 'dist/preview'))
  ? rendererPkgDir
  : resolve(__dirname, 'node_modules/@formepdf/renderer');

cpSync(resolve(corePkg, 'pkg'), resolve(__dirname, 'pkg'), { recursive: true });
cpSync(resolve(rendererPkg, 'dist/preview'), resolve(__dirname, 'dist/preview'), { recursive: true });

await build({
  entryPoints: [resolve(__dirname, 'src/extension.ts')],
  bundle: true,
  outfile: resolve(__dirname, 'dist/extension.js'),
  format: 'cjs',
  platform: 'node',
  target: 'node20',
  external: ['vscode', 'esbuild'],
  sourcemap: true,
  define: {
    'import.meta.url': 'FORME_IMPORT_META_URL',
  },
  banner: {
    js: `const FORME_IMPORT_META_URL = require('url').pathToFileURL(__filename).href;`,
  },
});

console.log('Built extension');
