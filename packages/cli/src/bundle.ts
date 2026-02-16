import { build } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

/// The temp directory for bundled output — placed inside CLI package
/// so that Node's module resolution finds @forme/react, @forme/core, react.
export const BUNDLE_DIR = join(__dirname, '..');

/// Bundle a TSX/JSX file into an ESM string that can be dynamically imported.
export async function bundleFile(filePath: string): Promise<string> {
  const result = await build({
    entryPoints: [filePath],
    bundle: true,
    format: 'esm',
    platform: 'node',
    write: false,
    jsx: 'automatic',
    target: 'node20',
    external: ['react', '@forme/react', '@forme/core'],
  });

  return result.outputFiles[0].text;
}
