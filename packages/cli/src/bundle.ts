import { build, type BuildFailure, type Plugin } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

/// The temp directory for bundled output — placed inside CLI package
/// so that Node's module resolution finds @forme/react, @forme/core, react.
export const BUNDLE_DIR = join(__dirname, '..');

/// esbuild plugin that intercepts react/jsx-dev-runtime to capture source
/// locations in a global WeakMap. React 19 no longer stores _source on
/// elements, so we wrap jsxDEV to do it ourselves.
const formeJsxSourcePlugin: Plugin = {
  name: 'forme-jsx-source',
  setup(pluginBuild) {
    pluginBuild.onResolve({ filter: /^react\/jsx-dev-runtime$/ }, () => ({
      path: 'forme-jsx-dev-runtime',
      namespace: 'forme-jsx',
    }));

    pluginBuild.onLoad({ filter: /.*/, namespace: 'forme-jsx' }, () => {
      const cwd = pluginBuild.initialOptions.absWorkingDir || process.cwd();
      return {
        contents: `
          import { jsx, Fragment } from 'react/jsx-runtime';
          import { resolve, isAbsolute } from 'node:path';
          export { Fragment };
          if (!globalThis.__formeSourceMap) globalThis.__formeSourceMap = new WeakMap();
          const _cwd = ${JSON.stringify(cwd)};
          export function jsxDEV(type, props, key, isStaticChildren, source, self) {
            const el = jsx(type, props, key);
            if (source && source.fileName) {
              try {
                const file = isAbsolute(source.fileName) ? source.fileName : resolve(_cwd, source.fileName);
                globalThis.__formeSourceMap.set(el, { file, line: source.lineNumber, column: source.columnNumber });
              } catch(e) {}
            }
            return el;
          }
        `,
        resolveDir: cwd,
        loader: 'js',
      };
    });
  },
};

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
      jsxDev: true,
      target: 'node20',
      external: ['react', '@forme/react', '@forme/core'],
      plugins: [formeJsxSourcePlugin],
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
