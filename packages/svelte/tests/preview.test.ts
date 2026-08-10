/**
 * Preview route helper: dispatch and HTML rewriting are unit-tested
 * without a browser against the real copied asset, and the demo
 * fixture route ([...forme]/+server.ts) exercises the full flow -
 * template in, PDF bytes / layout JSON / version polling out - through
 * @formepdf/core (devDependency, same as the WASM smoke tests).
 */
import { describe, it, expect } from 'vitest';
import {
  dispatchPreviewPath,
  formePreview,
  rewritePreviewHtml,
} from '../src/preview/index.js';
import { PREVIEW_HTML } from '../src/preview/preview-html.generated.js';
import { GET } from './fixtures/preview-route/[...forme]/+server.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import BadProp from './fixtures/bad-prop.svelte';

const MOUNT = '/dev/pdf-preview';

function eventFor(rest: string, base = MOUNT) {
  const pathname = rest === '' ? base || '/' : `${base}/${rest}`;
  return { url: new URL(`http://localhost:5173${pathname}`), params: { forme: rest } };
}

describe('dispatchPreviewPath', () => {
  it('serves the page at the mount prefix and keeps endpoint fetches under it', () => {
    expect(dispatchPreviewPath('/dev/pdf-preview', { forme: '' })).toEqual({
      endpoint: 'page',
      prefix: '/dev/pdf-preview',
    });
    expect(dispatchPreviewPath('/dev/pdf-preview/pdf', { forme: 'pdf' })).toEqual({
      endpoint: 'pdf',
      prefix: '/dev/pdf-preview',
    });
    expect(dispatchPreviewPath('/dev/pdf-preview/layout', { forme: 'layout' })).toEqual({
      endpoint: 'layout',
      prefix: '/dev/pdf-preview',
    });
    expect(dispatchPreviewPath('/dev/pdf-preview/version', { forme: 'version' })).toEqual({
      endpoint: 'version',
      prefix: '/dev/pdf-preview',
    });
  });

  it('handles a root mount and trailing slashes', () => {
    expect(dispatchPreviewPath('/', { forme: '' })).toEqual({ endpoint: 'page', prefix: '' });
    expect(dispatchPreviewPath('/pdf', { forme: 'pdf' })).toEqual({
      endpoint: 'pdf',
      prefix: '',
    });
    expect(dispatchPreviewPath('/dev/pdf-preview/', { forme: '' })).toEqual({
      endpoint: 'page',
      prefix: '/dev/pdf-preview',
    });
  });

  it('disambiguates a mount prefix that itself ends in an endpoint name', () => {
    // Mounted at /pdf/[...forme]: the page URL is /pdf, not the bytes.
    expect(dispatchPreviewPath('/pdf', { forme: '' })).toEqual({
      endpoint: 'page',
      prefix: '/pdf',
    });
    expect(dispatchPreviewPath('/pdf/pdf', { forme: 'pdf' })).toEqual({
      endpoint: 'pdf',
      prefix: '/pdf',
    });
  });

  it('resolves the catch-all among multiple route params', () => {
    // /[lang]/preview/[...rest]
    expect(dispatchPreviewPath('/en/preview/layout', { lang: 'en', rest: 'layout' })).toEqual({
      endpoint: 'layout',
      prefix: '/en/preview',
    });
    expect(dispatchPreviewPath('/en/preview', { lang: 'en', rest: '' })).toEqual({
      endpoint: 'page',
      prefix: '/en/preview',
    });
  });

  it('falls back to suffix sniffing when params are absent', () => {
    expect(dispatchPreviewPath('/x/pdf')).toEqual({ endpoint: 'pdf', prefix: '/x' });
    expect(dispatchPreviewPath('/x/layout')).toEqual({ endpoint: 'layout', prefix: '/x' });
    expect(dispatchPreviewPath('/x')).toEqual({ endpoint: 'page', prefix: '/x' });
  });

  it('rejects unknown rest paths', () => {
    expect(dispatchPreviewPath('/dev/pdf-preview/foo/bar', { forme: 'foo/bar' }).endpoint).toBe(
      'unknown',
    );
  });
});

describe('rewritePreviewHtml', () => {
  it('rewrites endpoint fetches to be prefix-relative', () => {
    const html = rewritePreviewHtml(PREVIEW_HTML, MOUNT, 1000);
    expect(html).toContain(`fetch("${MOUNT}/pdf")`);
    expect(html).toContain(`fetch("${MOUNT}/layout")`);
    expect(html).not.toContain(`fetch('/pdf')`);
    expect(html).not.toContain(`fetch('/layout')`);
  });

  it('replaces the WebSocket channel with version polling, keeping the init call site', () => {
    const html = rewritePreviewHtml(PREVIEW_HTML, MOUNT, 500);
    expect(html).toContain(`fetch("${MOUNT}/version", { cache: 'no-store' })`);
    expect(html).toContain('setTimeout(tick, 500)');
    // The original body is parked, and the asset's connectWs() call
    // now reaches the polling implementation.
    expect(html).toContain('function __formeUnusedConnectWs() {');
    expect(html).toContain('connectWs();');
    expect(html).toMatch(/function connectWs\(\) \{\s*statusDot\.className = 'status-dot connected';/);
  });

  it('disables polling when pollMs is 0 but still neutralizes the WebSocket', () => {
    const html = rewritePreviewHtml(PREVIEW_HTML, MOUNT, 0);
    expect(html).not.toContain('/version');
    expect(html).toContain('function __formeUnusedConnectWs() {');
  });

  it('fails loudly when the asset drifts and a marker disappears', () => {
    expect(() => rewritePreviewHtml('<html></html>', MOUNT, 1000)).toThrow(
      /exactly one occurrence/,
    );
  });

  it('produces a syntactically valid module script (the connectWs swap keeps braces balanced)', async () => {
    const { transformWithEsbuild } = await import('vite');
    for (const pollMs of [1000, 0]) {
      const html = rewritePreviewHtml(PREVIEW_HTML, MOUNT, pollMs);
      const match = html.match(/<script type="module">([\s\S]*?)<\/script>/u);
      expect(match).not.toBeNull();
      // Parse as an ES module (top-level await included); a syntax
      // error from the rewrite surgery throws here.
      await expect(
        transformWithEsbuild(match![1], 'preview-inline.js', { loader: 'js' }),
      ).resolves.toBeTruthy();
    }
  });
});

describe('preview route (fixture E2E)', () => {
  it('serves the preview page with prefix-relative endpoints and polling', async () => {
    const resp = await GET(eventFor(''));
    expect(resp.status).toBe(200);
    expect(resp.headers.get('Content-Type')).toContain('text/html');
    const html = await resp.text();
    expect(html).toContain(`fetch("${MOUNT}/pdf")`);
    expect(html).toContain(`fetch("${MOUNT}/layout")`);
    expect(html).toContain(`fetch("${MOUNT}/version", { cache: 'no-store' })`);
    expect(html).toContain('setTimeout(tick, 500)');
  });

  it('serves rendered PDF bytes', async () => {
    const resp = await GET(eventFor('pdf'));
    expect(resp.status).toBe(200);
    expect(resp.headers.get('Content-Type')).toBe('application/pdf');
    const bytes = new Uint8Array(await resp.arrayBuffer());
    expect(bytes.length).toBeGreaterThan(500);
    expect(new TextDecoder().decode(bytes.slice(0, 5))).toBe('%PDF-');
  });

  it('serves layout pages JSON for the rendered template', async () => {
    const resp = await GET(eventFor('layout'));
    expect(resp.status).toBe(200);
    expect(resp.headers.get('Content-Type')).toContain('application/json');
    const layout = await resp.json();
    expect(Array.isArray(layout.pages)).toBe(true);
    expect(layout.pages.length).toBeGreaterThanOrEqual(1);
  });

  it('serves a stable version for an unchanged template and a new one after a change', async () => {
    const first = await (await GET(eventFor('version'))).text();
    const second = await (await GET(eventFor('version'))).text();
    expect(first).toMatch(/^[0-9a-f]+-[0-9a-f]+$/);
    expect(second).toBe(first);

    // Saving the template gives a fresh handler closure under Vite;
    // simulate that with a second handler rendering different content.
    const changed = formePreview(HelloWorld, {
      props: { name: 'Changed', items: ['gamma'], showFooter: false },
      pollMs: 500,
    });
    const changedVersion = await (await changed(eventFor('version'))).text();
    expect(changedVersion).not.toBe(first);
  });

  it('returns 404 for unknown rest paths', async () => {
    const resp = await GET(eventFor('nope'));
    expect(resp.status).toBe(404);
  });

  it('returns 503 with the render error for pdf, layout, and version', async () => {
    const broken = formePreview(BadProp, { pollMs: 500 });
    for (const rest of ['pdf', 'layout', 'version']) {
      const resp = await broken(eventFor(rest));
      expect(resp.status).toBe(503);
      expect(await resp.text()).toBeTruthy();
    }
  });
});
