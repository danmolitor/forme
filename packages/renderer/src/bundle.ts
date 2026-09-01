import { build, type BuildFailure, type Plugin } from 'esbuild';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

/// The temp directory for bundled output — placed inside renderer package
/// so that Node's module resolution finds @formepdf/react, @formepdf/core, react.
export const BUNDLE_DIR = join(__dirname, '..');

/// The two JSX reconcilers Forme adapters target. React and Preact are the
/// same input format (JSX) through the same dispatch, differing only in the
/// jsx runtime, the externalized runtime package, and the adapter that
/// serializes their elements. `flavor` is the toggle between them — a
/// dispatch value where there used to be a react constant.
export type JsxFlavor = 'react' | 'preact';

interface JsxFlavorConfig {
  /// esbuild `jsxImportSource` — which package's jsx-runtime the automatic
  /// runtime compiles calls against.
  jsxImportSource: string;
  /// Runtime packages kept external, resolved from the user's workspace so the
  /// template, adapter, and jsx-runtime share one reconciler instance.
  external: string[];
}

const JSX_FLAVORS: Record<JsxFlavor, JsxFlavorConfig> = {
  react: { jsxImportSource: 'react', external: ['react', '@formepdf/react', '@formepdf/core'] },
  preact: { jsxImportSource: 'preact', external: ['preact', '@formepdf/preact', '@formepdf/core'] },
};

/// Detect which reconciler a template targets from its import signature. The
/// `@formepdf/preact` adapter is the only Preact tell; everything else is
/// react (the default). Same signal the extension gates on.
export function detectJsxFlavor(source: string): JsxFlavor {
  return /@formepdf\/preact/.test(source) ? 'preact' : 'react';
}

/// esbuild plugin that intercepts the reconciler's jsx-dev-runtime to capture
/// source locations in a global WeakMap. React 19 no longer stores _source on
/// elements (and Preact never did), so we wrap jsxDEV to do it ourselves. The
/// adapter serializers read the same `__formeSourceMap` for inspector
/// source-jump, so this lights up identically for both flavors.
function formeJsxSourcePlugin(runtime: string): Plugin {
  return {
    name: 'forme-jsx-source',
    setup(pluginBuild) {
      const filter = new RegExp(`^${runtime}/jsx-dev-runtime$`);
      pluginBuild.onResolve({ filter }, () => ({
        path: 'forme-jsx-dev-runtime',
        namespace: 'forme-jsx',
      }));

      pluginBuild.onLoad({ filter: /.*/, namespace: 'forme-jsx' }, () => {
        const cwd = pluginBuild.initialOptions.absWorkingDir || process.cwd();
        return {
          contents: `
            import { jsx, Fragment } from '${runtime}/jsx-runtime';
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
}

/// Bundle a TSX/JSX file into an ESM string that can be dynamically imported.
export async function bundleFile(filePath: string, flavor: JsxFlavor = 'react'): Promise<string> {
  const config = JSX_FLAVORS[flavor];
  try {
    const result = await build({
      entryPoints: [filePath],
      bundle: true,
      format: 'esm',
      platform: 'node',
      write: false,
      jsx: 'automatic',
      jsxDev: true,
      jsxImportSource: config.jsxImportSource,
      target: 'node20',
      external: config.external,
      plugins: [formeJsxSourcePlugin(config.jsxImportSource)],
    });

    return result.outputFiles[0].text;
  } catch (err) {
    throw formatBuildError(err);
  }
}

/// Bundle TSX/JSX source code (string) into an ESM string.
/// `resolveDir` controls where imports are resolved from (typically the file's directory).
export async function bundleSource(
  source: string,
  resolveDir: string,
  sourcefile?: string,
  flavor: JsxFlavor = 'react',
): Promise<string> {
  const config = JSX_FLAVORS[flavor];
  try {
    const result = await build({
      stdin: {
        contents: source,
        resolveDir,
        sourcefile: sourcefile ?? 'input.tsx',
        loader: 'tsx',
      },
      bundle: true,
      format: 'esm',
      platform: 'node',
      write: false,
      jsx: 'automatic',
      jsxDev: true,
      jsxImportSource: config.jsxImportSource,
      target: 'node20',
      external: config.external,
      plugins: [formeJsxSourcePlugin(config.jsxImportSource)],
      absWorkingDir: resolveDir,
    });

    return result.outputFiles[0].text;
  } catch (err) {
    throw formatBuildError(err);
  }
}

function formatBuildError(err: unknown): Error {
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
    return new Error(`Build error:\n${messages.join('\n')}`);
  }
  return err instanceof Error ? err : new Error(String(err));
}

function isBuildFailure(err: unknown): err is BuildFailure {
  return (
    err !== null &&
    typeof err === 'object' &&
    'errors' in err &&
    Array.isArray((err as BuildFailure).errors)
  );
}
