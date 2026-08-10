/**
 * SvelteKit preview route helper.
 *
 * `formePreview(Template)` returns a GET handler for a catch-all route
 * (`src/routes/<anything>/[...forme]/+server.ts`) that serves the same
 * preview UI (layout overlays, click-to-inspect) the CLI dev server
 * gives react users. The HTML asset is copied from @formepdf/renderer
 * at build time (see scripts/embed-preview-html.mjs); serving it from a
 * route instead of a dedicated server needs two serve-time adjustments:
 *
 * 1. The asset fetches absolute `/pdf` and `/layout`; those are
 *    rewritten to resolve under the mount prefix.
 * 2. The asset's reload channel is a WebSocket, which a SvelteKit
 *    `+server.ts` cannot provide; its `connectWs` is swapped for a
 *    `/version` polling loop that reuses the page's own `reload()`.
 *
 * Rendering goes through `renderDocumentWithLayout`, so the helper adds
 * zero dependencies beyond the already-optional `@formepdf/core` peer.
 * The render result is cached per handler instance: under the Vite dev
 * server, saving the template invalidates the module that called
 * `formePreview`, so the next request builds a fresh handler (and thus
 * a fresh render) while polls against an unchanged template stay cheap.
 */

import type { Component } from 'svelte';
import type { LayoutInfo } from '@formepdf/core';
import { renderDocumentWithLayout } from '../render-document.js';
import { PREVIEW_HTML } from './preview-html.generated.js';

/** Options for {@link formePreview}. */
export interface FormePreviewOptions<Props extends Record<string, any>> {
  /** Props passed to the template component. */
  props?: Props;
  /**
   * Reload poll interval in milliseconds; `0` disables polling.
   * Defaults to 1000 in dev and 0 when `NODE_ENV` is `production`.
   */
  pollMs?: number;
}

/**
 * The slice of SvelteKit's `RequestEvent` the handler reads. Structural,
 * so the helper needs no dependency on @sveltejs/kit types.
 */
export interface PreviewRequestEvent {
  url: URL;
  params?: Partial<Record<string, string>>;
}

/** Rest paths the catch-all route serves besides the preview page. */
const RESOURCE_ENDPOINTS = ['pdf', 'layout', 'version'] as const;

/** Endpoints the catch-all route dispatches to. */
export type PreviewEndpoint = (typeof RESOURCE_ENDPOINTS)[number] | 'page' | 'unknown';

interface Rendered {
  /** ArrayBuffer-backed so it satisfies `BodyInit` for `Response`. */
  pdf: Uint8Array<ArrayBuffer>;
  layout: LayoutInfo;
  version: string;
}

/**
 * Create a GET handler serving the Forme preview UI from a SvelteKit
 * catch-all route. Mount it at any prefix:
 *
 * ```ts
 * // src/routes/dev/pdf/[...forme]/+server.ts
 * import { formePreview } from '@formepdf/svelte/preview';
 * import Invoice from '$lib/Invoice.svelte';
 *
 * export const GET = formePreview(Invoice, { props: { ... } });
 * ```
 */
export function formePreview<Props extends Record<string, any>>(
  template: Component<Props>,
  options: FormePreviewOptions<Props> = {},
): (event: PreviewRequestEvent) => Promise<Response> {
  const isProduction =
    typeof process !== 'undefined' && process.env?.NODE_ENV === 'production';
  const pollMs = options.pollMs ?? (isProduction ? 0 : 1000);

  let cache: Rendered | null = null;
  let inFlight: Promise<Rendered> | null = null;

  async function renderOnce(): Promise<Rendered> {
    if (cache) return cache;
    if (!inFlight) {
      inFlight = renderDocumentWithLayout(template, { props: options.props })
        .then((result) => {
          cache = {
            pdf: result.pdf,
            layout: result.layout,
            version: hashBytes(result.pdf),
          };
          return cache;
        })
        .finally(() => {
          inFlight = null;
        });
    }
    return inFlight;
  }

  return async (event) => {
    const { endpoint, prefix } = dispatchPreviewPath(event.url.pathname, event.params);

    if (endpoint === 'page') {
      return previewResponse(
        rewritePreviewHtml(PREVIEW_HTML, prefix, pollMs),
        'text/html; charset=utf-8',
      );
    }

    if (endpoint === 'unknown') {
      return previewResponse('Not found', 'text/plain; charset=utf-8', 404);
    }

    let result: Rendered;
    try {
      result = await renderOnce();
    } catch (err) {
      // The error message IS the dev UX (the preview page shows template
      // errors in the browser), but in production a mounted preview route
      // must not leak render internals.
      const message = isProduction
        ? 'PDF render failed'
        : err instanceof Error
          ? err.message
          : String(err);
      return previewResponse(message, 'text/plain; charset=utf-8', 503);
    }

    switch (endpoint) {
      case 'pdf':
        return previewResponse(result.pdf, 'application/pdf');
      case 'layout':
        return previewResponse(JSON.stringify(result.layout), 'application/json');
      case 'version':
        return previewResponse(result.version, 'text/plain; charset=utf-8');
    }
  };
}

/**
 * Resolve a request path to a preview endpoint plus the mount prefix
 * the HTML's endpoint fetches must resolve under.
 *
 * The catch-all param value (the only param SvelteKit can bind to an
 * empty string or a multi-segment value) pins down where the mount
 * prefix ends; matching it against the pathname tail keeps the helper
 * agnostic to the param's name. Without params (or when no value
 * matches, e.g. percent-encoded segments), known endpoint suffixes are
 * sniffed instead - ambiguous only for a mount prefix itself ending in
 * `/pdf`, `/layout`, or `/version`, which the param path disambiguates.
 */
export function dispatchPreviewPath(
  pathname: string,
  params?: Partial<Record<string, string>>,
): { endpoint: PreviewEndpoint; prefix: string } {
  const path = normalizePath(pathname);

  const rest = restFromParams(path, params);
  if (rest !== null) {
    const prefix = rest === '' ? path : path.slice(0, path.length - rest.length - 1);
    return { endpoint: endpointForRest(rest), prefix: normalizePath(prefix) };
  }

  for (const known of RESOURCE_ENDPOINTS) {
    if (path.endsWith(`/${known}`)) {
      return {
        endpoint: known,
        prefix: normalizePath(path.slice(0, path.length - known.length - 1)),
      };
    }
  }
  return { endpoint: 'page', prefix: path };
}

/** Strip trailing slashes; the root path becomes the empty prefix. */
function normalizePath(pathname: string): string {
  return pathname.replace(/\/+$/, '');
}

/**
 * Find the catch-all rest path among the route params: an empty value
 * is unambiguously the catch-all (other param kinds can't be empty),
 * otherwise the longest value that is a suffix of the pathname wins
 * (the catch-all spans the whole tail; a shorter match would be a
 * parent-segment param that happens to equal the last segment).
 */
function restFromParams(
  path: string,
  params?: Partial<Record<string, string>>,
): string | null {
  if (!params) return null;
  let best: string | null = null;
  for (const value of Object.values(params)) {
    if (value === undefined) continue;
    if (value === '') return '';
    if (path === `/${value}` || path.endsWith(`/${value}`)) {
      if (best === null || value.length > best.length) best = value;
    }
  }
  return best;
}

function endpointForRest(rest: string): PreviewEndpoint {
  if (rest === '') return 'page';
  const known = RESOURCE_ENDPOINTS.find((endpoint) => endpoint === rest);
  return known ?? 'unknown';
}

/**
 * Serve-time adjustments to the copied preview HTML: endpoint
 * fetches become prefix-relative, and the WebSocket reload channel is
 * replaced by a polling loop. The polling function keeps the `connectWs`
 * name so the asset's init call site stays untouched; the original body
 * is parked on a never-called function to keep braces balanced. Each
 * marker must occur exactly once - a miss means the renderer asset
 * changed shape, and failing loudly here (covered by unit tests against
 * the real asset) beats serving a preview with a dead reload channel.
 */
export function rewritePreviewHtml(html: string, prefix: string, pollMs: number): string {
  const replacements: Array<[needle: string, replacement: string]> = [
    [`fetch('/pdf')`, `fetch(${JSON.stringify(`${prefix}/pdf`)})`],
    [`fetch('/layout')`, `fetch(${JSON.stringify(`${prefix}/layout`)})`],
    [
      `function connectWs() {`,
      `${pollingScript(prefix, pollMs)}\n  function __formeUnusedConnectWs() {`,
    ],
  ];

  for (const [needle, replacement] of replacements) {
    const first = html.indexOf(needle);
    if (first === -1 || html.indexOf(needle, first + needle.length) !== -1) {
      throw new Error(
        `@formepdf/svelte preview: expected exactly one occurrence of ${JSON.stringify(
          needle,
        )} in the preview HTML asset; the copy from @formepdf/renderer may have drifted`,
      );
    }
    html = html.slice(0, first) + replacement + html.slice(first + needle.length);
  }
  return html;
}

/**
 * The polling replacement for the WebSocket channel. Runs inside the
 * asset's module script, so it reuses the in-scope `reload()`,
 * `zoomToFit`, `statusDot`, and `errorEl`. Version mismatches trigger a
 * reload; render failures surface through the error overlay exactly
 * like the WebSocket `error` message did, and the first healthy poll
 * after a failure reloads to clear it.
 */
function pollingScript(prefix: string, pollMs: number): string {
  if (pollMs <= 0) {
    return `function connectWs() {
    statusDot.className = 'status-dot connected';
  }`;
  }
  const versionUrl = JSON.stringify(`${prefix}/version`);
  return `function connectWs() {
    statusDot.className = 'status-dot connected';
    let lastVersion = null;
    let hadError = false;
    const showError = (message) => {
      errorEl.querySelector('.error-dismiss')?.nextSibling?.remove();
      errorEl.appendChild(document.createTextNode(message));
      errorEl.style.display = 'block';
    };
    const tick = async () => {
      try {
        const resp = await fetch(${versionUrl}, { cache: 'no-store' });
        if (resp.ok) {
          const version = await resp.text();
          statusDot.className = 'status-dot connected';
          if (hadError || (lastVersion !== null && version !== lastVersion)) {
            await reload();
            setTimeout(zoomToFit, 300);
          }
          hadError = false;
          lastVersion = version;
        } else {
          hadError = true;
          showError(await resp.text());
        }
      } catch {
        statusDot.className = 'status-dot disconnected';
      }
      setTimeout(tick, ${pollMs});
    };
    tick();
  }`;
}

/**
 * FNV-1a over the PDF bytes. Cheap change detection for the polling
 * loop - not a content address, so the length is appended to keep
 * accidental 32-bit collisions from suppressing a reload.
 */
function hashBytes(bytes: Uint8Array): string {
  let hash = 0x811c9dc5;
  for (let i = 0; i < bytes.length; i++) {
    hash ^= bytes[i];
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }
  return `${hash.toString(16)}-${bytes.length.toString(16)}`;
}

function previewResponse(body: BodyInit, contentType: string, status = 200): Response {
  return new Response(body, {
    status,
    headers: {
      'Content-Type': contentType,
      'Cache-Control': 'no-store',
    },
  });
}
