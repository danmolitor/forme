/**
 * Framework single-file-component input paths: `.svelte` and `.vue`.
 *
 * These are the fourth and fifth inputs to the renderer, alongside JSX
 * (`render.ts`) and HTML (`html.ts`). An SFC is compiled to a server
 * component, run through its adapter's `serialize()` (which returns the same
 * `FormeDocument` a JSX render produces), and handed to the shared
 * `renderDocToResult` tail — so tree, inspector, and overlays light up for
 * Svelte and Vue with zero downstream changes, exactly as they did for HTML.
 *
 * Compilation follows the JSX path's philosophy: the bundler ships with the
 * host, but every framework package resolves from the *user's* workspace.
 * `svelte/compiler` and `@vue/compiler-sfc` are loaded from the user's
 * `node_modules` (both ship with the framework the adapter peer-depends on),
 * and the framework runtime (`svelte`, `vue`) stays external so the compiled
 * template and its adapter share one instance. Nothing framework-specific is
 * bundled into the extension — the same reason the JSX path externalizes
 * `react`/`@formepdf/*`.
 *
 * `@formepdf/svelte` ships raw `.svelte` component files (svelte-package
 * output), so it cannot be imported by Node directly; it is bundled and
 * compiled by the same plugin as the user's template. `@formepdf/vue` ships
 * compiled JS, but is bundled the same way for symmetry. Only the framework
 * runtime is external.
 */
import { build, type Plugin } from 'esbuild';
import { writeFile, unlink, readFile } from 'node:fs/promises';
import { resolve, dirname, join } from 'node:path';
import { pathToFileURL } from 'node:url';
import { createRequire } from 'node:module';
import type { RenderOptions, RenderResult } from './render.js';
import { renderDocToResult, tempName } from './render.js';
import { friendlyDependencyError } from './workspace.js';

type Framework = 'svelte' | 'vue';

interface FrameworkConfig {
  ext: string;
  adapter: string;
  /** Module specifiers kept external — the framework runtime, resolved from
   *  the user's workspace so template and adapter share one instance. */
  external: string[];
}

const FRAMEWORKS: Record<Framework, FrameworkConfig> = {
  svelte: {
    ext: '.svelte',
    adapter: '@formepdf/svelte',
    external: ['svelte', 'svelte/*'],
  },
  vue: {
    ext: '.vue',
    adapter: '@formepdf/vue',
    external: ['vue', 'vue/*', '@vue/*'],
  },
};

/// Resolve a module specifier from the user's workspace (the directory
/// containing their template), not the renderer's own node_modules.
function resolveFromDir(spec: string, dir: string): string {
  const require = createRequire(pathToFileURL(join(dir, '__forme_resolver__.js')));
  return require.resolve(spec);
}

/// Dynamically import a package from the user's workspace, unwrapping the
/// CJS default-export interop (svelte/compiler is CJS; its real exports sit
/// under `.default` when imported as ESM).
async function importFromDir(spec: string, dir: string): Promise<Record<string, any>> {
  const url = pathToFileURL(resolveFromDir(spec, dir)).href;
  const mod = await import(url);
  return (mod.default ?? mod) as Record<string, any>;
}

let cachedSvelte: Record<string, any> | undefined;
let cachedVue: Record<string, any> | undefined;

/// esbuild plugin compiling the SFC entry (and any imported SFCs, including
/// the adapter's own components) to server-render JS. The compiler is loaded
/// once per process from the workspace `resolveDir`.
function sfcPlugin(resolveDir: string): Plugin {
  return {
    name: 'forme-sfc',
    setup(pluginBuild) {
      pluginBuild.onLoad({ filter: /\.svelte$/ }, async (args) => {
        const compiler = (cachedSvelte ??= await importFromDir('svelte/compiler', resolveDir));
        const source = await readFile(args.path, 'utf-8');
        const { js } = compiler.compile(source, { generate: 'server', filename: args.path });
        return { contents: js.code, loader: 'js', resolveDir: dirname(args.path) };
      });

      pluginBuild.onLoad({ filter: /\.vue$/ }, async (args) => {
        const compiler = (cachedVue ??= await importFromDir('@vue/compiler-sfc', resolveDir));
        const source = await readFile(args.path, 'utf-8');
        const { descriptor } = compiler.parse(source, { filename: args.path });
        // A stable per-file id ties the compiled script's binding metadata to
        // the template's SSR render, and scopes any `<style scoped>`.
        const id = 'forme' + Math.abs(hashString(args.path)).toString(36);
        const script = compiler.compileScript(descriptor, {
          id,
          inlineTemplate: false,
          templateOptions: { ssr: true },
        });
        let code = compiler.rewriteDefault(script.content, '__forme_sfc__', ['typescript']);
        if (descriptor.template) {
          const template = compiler.compileTemplate({
            source: descriptor.template.content,
            filename: args.path,
            id,
            ssr: true,
            ssrCssVars: [],
            compilerOptions: { bindingMetadata: script.bindings },
          });
          code += `\n${template.code}\n__forme_sfc__.ssrRender = ssrRender;`;
        }
        code += `\nexport default __forme_sfc__;`;
        // `loader: 'ts'` lets esbuild strip any residual type syntax from a
        // `<script setup lang="ts">` block and resolve the template's imports.
        return { contents: code, loader: 'ts', resolveDir: dirname(args.path) };
      });
    },
  };
}

function hashString(s: string): number {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = ((h << 5) - h + s.charCodeAt(i)) | 0;
  return h;
}

/// Bundle an SFC entry into an ESM string that re-exports the compiled
/// template component (`__formeTemplate`) plus its adapter's `serialize`
/// (`__formeSerialize`) — both from the *same* bundle so they share the
/// framework runtime and font-registration singleton.
async function bundleSfc(framework: Framework, entryAbs: string): Promise<string> {
  const config = FRAMEWORKS[framework];
  const resolveDir = dirname(entryAbs);
  const stub =
    `export { default as __formeTemplate } from ${JSON.stringify(entryAbs)};\n` +
    `export { serialize as __formeSerialize } from ${JSON.stringify(config.adapter)};\n`;

  const result = await build({
    stdin: { contents: stub, resolveDir, sourcefile: '__forme_entry__.js', loader: 'js' },
    bundle: true,
    format: 'esm',
    platform: 'node',
    target: 'node20',
    write: false,
    external: config.external,
    plugins: [sfcPlugin(resolveDir)],
    absWorkingDir: resolveDir,
  });
  return result.outputFiles[0].text;
}

/// Compile → import → serialize → shared render tail. `data` becomes the
/// template's props (the SFC analogue of the JSX path passing data to a
/// component function). Unlike JSX, an SFC with no data renders with its own
/// default props rather than erroring.
async function renderSfc(
  framework: Framework,
  entryAbs: string,
  options: RenderOptions | undefined,
  cleanupEntry: boolean,
): Promise<RenderResult> {
  const start = performance.now();
  const basePath = dirname(entryAbs);
  const ext = FRAMEWORKS[framework].ext;

  // A missing compiler (svelte/compiler, @vue/compiler-sfc) surfaces here as a
  // resolution failure inside the esbuild plugin; a missing runtime surfaces
  // at import time below. Both should read as guidance naming the package.
  let code: string;
  try {
    code = await bundleSfc(framework, entryAbs);
  } catch (err) {
    throw friendlyDependencyError(err, ext);
  } finally {
    if (cleanupEntry) await unlink(entryAbs).catch(() => {});
  }

  // Import the bundle from a temp module inside the user's directory so Node
  // resolves the external framework runtime from their node_modules — the
  // same trick the JSX path uses for react/@formepdf/*.
  const tmpFile = join(basePath, tempName('.forme-render-', '.mjs'));
  await writeFile(tmpFile, code);

  let mod: Record<string, unknown>;
  try {
    mod = await import(pathToFileURL(tmpFile).href);
  } catch (err) {
    throw friendlyDependencyError(err, ext);
  } finally {
    await unlink(tmpFile).catch(() => {});
  }

  const template = mod.__formeTemplate;
  const serialize = mod.__formeSerialize as (
    component: unknown,
    opts?: { props?: unknown },
  ) => Promise<Record<string, unknown>>;

  const props = await resolveProps(options);
  const doc = await serialize(template, { props });

  return renderDocToResult(doc, { pageSize: options?.pageSize, basePath, startTime: start });
}

async function resolveProps(options: RenderOptions | undefined): Promise<Record<string, unknown>> {
  if (options?.data !== undefined) return options.data as Record<string, unknown>;
  if (options?.dataPath) {
    const raw = await readFile(resolve(options.dataPath), 'utf-8');
    return JSON.parse(raw) as Record<string, unknown>;
  }
  return {};
}

/// Write an editor buffer to a temp SFC next to the user's file so relative
/// imports resolve, then render it. Mirrors `renderFromSource` for JSX.
async function renderSfcFromSource(
  framework: Framework,
  source: string,
  resolveDir: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  const tmpEntry = join(resolveDir, tempName('.forme-input-', FRAMEWORKS[framework].ext));
  await writeFile(tmpEntry, source);
  return renderSfc(framework, tmpEntry, options, /* cleanupEntry */ true);
}

// ── Public API ───────────────────────────────────────────────────────

/// Render a `.svelte` template file to PDF + LayoutInfo. Mirrors
/// `renderFromFile` for JSX.
export function renderSvelteFromFile(filePath: string, options?: RenderOptions): Promise<RenderResult> {
  return renderSfc('svelte', resolve(filePath), options, false);
}

/// Render `.svelte` source (e.g. an unsaved editor buffer). `resolveDir`
/// controls import resolution — typically the file's directory.
export function renderSvelteFromSource(
  source: string,
  resolveDir: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  return renderSfcFromSource('svelte', source, resolveDir, options);
}

/// Render a `.vue` template file to PDF + LayoutInfo.
export function renderVueFromFile(filePath: string, options?: RenderOptions): Promise<RenderResult> {
  return renderSfc('vue', resolve(filePath), options, false);
}

/// Render `.vue` source (e.g. an unsaved editor buffer).
export function renderVueFromSource(
  source: string,
  resolveDir: string,
  options?: RenderOptions,
): Promise<RenderResult> {
  return renderSfcFromSource('vue', source, resolveDir, options);
}
