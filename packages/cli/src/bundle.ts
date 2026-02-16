import { build, type BuildFailure } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

/// The temp directory for bundled output — placed inside CLI package
/// so that Node's module resolution finds @forme/react, @forme/core, react.
export const BUNDLE_DIR = join(__dirname, '..');

/// Bundle a TSX/JSX file into an ESM string that can be dynamically imported.
export async function bundleFile(filePath: string): Promise<string> {
  try {
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
  } catch (err) {
    // Format esbuild errors with file location and source context
    if (isBuildFailure(err)) {
      const messages: string[] = [];
      for (const error of err.errors) {
        let loc = '';
        if (error.location) {
          const { file, line, column, lineText } = error.location;
          loc = `  ${file}:${line}:${column}\n`;
          if (lineText) {
            loc += `  ${lineText}\n`;
            loc += `  ${' '.repeat(column)}^\n`;
          }
        }
        messages.push(`${error.text}\n${loc}`);
      }
      throw new Error(`Build error:\n${messages.join('\n')}`);
    }
    throw err;
  }
}

function isBuildFailure(err: unknown): err is BuildFailure {
  return (
    err !== null &&
    typeof err === 'object' &&
    'errors' in err &&
    Array.isArray((err as BuildFailure).errors)
  );
}
