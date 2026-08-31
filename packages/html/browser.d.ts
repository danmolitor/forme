// Type surface for `@formepdf/html/browser` (bundler target). Identical render
// API to the Node entry; `init` is a no-op because the bundler instantiates
// the WASM at module load.
export type {
  RenderHtmlOptions,
  RenderHtmlResult,
  RenderHtmlLayoutResult,
  LayoutInfo,
} from './index.js';
import type { RenderHtmlOptions, RenderHtmlResult, RenderHtmlLayoutResult } from './index.js';

/** No-op under the bundler-target build (WASM auto-instantiates at load). */
export function init(): Promise<void>;

export function renderHtml(html: string, options?: RenderHtmlOptions): RenderHtmlResult;
export function renderHtmlWithLayout(
  html: string,
  options?: RenderHtmlOptions,
): RenderHtmlLayoutResult;
