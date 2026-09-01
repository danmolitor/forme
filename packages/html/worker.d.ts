// Type surface for `@formepdf/html/worker` (web target). Same render API as
// the other entries, but WASM must be initialized explicitly first: call
// `await init(module)` once at request time before rendering.
export type {
  RenderHtmlOptions,
  RenderHtmlResult,
  RenderHtmlLayoutResult,
  LayoutInfo,
} from './index.js';
import type { RenderHtmlOptions, RenderHtmlResult, RenderHtmlLayoutResult } from './index.js';

/**
 * Initialize the WASM engine. Pass the `WebAssembly.Module` you imported (the
 * default shape Wrangler/esbuild give a `.wasm` import), or a `URL`,
 * `Response`, `Promise<Response>`, raw bytes, or `undefined` to fetch the
 * `.wasm` next to the module. Idempotent; must resolve before rendering.
 */
export function init(module?: unknown): Promise<void>;

export function renderHtml(html: string, options?: RenderHtmlOptions): RenderHtmlResult;
export function renderHtmlWithLayout(
  html: string,
  options?: RenderHtmlOptions,
): RenderHtmlLayoutResult;
