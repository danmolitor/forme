import { describe, it, expect } from 'vitest';
import { resolve } from 'node:path';
import type { LayoutInfo } from '@formepdf/core';
import {
  renderFromFile,
  renderSvelteFromFile,
  renderVueFromFile,
  type RenderResult,
} from '../src/index.js';

const fx = (name: string) => resolve(__dirname, 'fixtures', name);

/** A framework-agnostic structural signature of the laid-out tree: page count
 *  plus the recursive nodeType nesting on each page. Geometry and text runs are
 *  excluded — those are where run-splitting legitimately differs between React's
 *  per-child serialization and SSR's merged output. Structure must be identical. */
function layoutSignature(layout: LayoutInfo): unknown {
  const walk = (el: { nodeType: string; children?: unknown[] }): unknown => ({
    nodeType: el.nodeType,
    children: (el.children ?? []).map((c) => walk(c as typeof el)),
  });
  return layout.pages.map((p) => p.elements.map(walk));
}

const isPdf = (r: RenderResult) =>
  r.pdf.subarray(0, 5).toString() === new Uint8Array([0x25, 0x50, 0x44, 0x46, 0x2d]).toString();

describe('SFC + JSX render paths (smoke)', () => {
  it('renders a .svelte template to a PDF + layout', async () => {
    const r = await renderSvelteFromFile(fx('catalog.svelte'), { data: { title: 'Catalog' } });
    expect(isPdf(r)).toBe(true);
    expect(r.layout.pages.length).toBeGreaterThan(0);
    expect(r.warnings).toEqual([]);
  });

  it('renders a .vue template to a PDF + layout', async () => {
    const r = await renderVueFromFile(fx('catalog.vue'), { data: { title: 'Catalog' } });
    expect(isPdf(r)).toBe(true);
    expect(r.layout.pages.length).toBeGreaterThan(0);
    expect(r.warnings).toEqual([]);
  });

  it('renders a Preact .tsx template (jsxImportSource: preact) to a PDF + layout', async () => {
    const r = await renderFromFile(fx('catalog.preact.tsx'), { data: { title: 'Catalog' } });
    expect(isPdf(r)).toBe(true);
    expect(r.layout.pages.length).toBeGreaterThan(0);
    expect(r.warnings).toEqual([]);
  });
});

describe('cross-framework equivalence through the render pipeline (React === Preact === Svelte === Vue)', () => {
  it('same document, four frameworks → identical page count and layout tree', async () => {
    const data = { title: 'Catalog' };
    const [react, preact, svelte, vue] = await Promise.all([
      renderFromFile(fx('catalog.tsx'), { data }),
      renderFromFile(fx('catalog.preact.tsx'), { data }),
      renderSvelteFromFile(fx('catalog.svelte'), { data }),
      renderVueFromFile(fx('catalog.vue'), { data }),
    ]);

    // Page count parity
    const pages = [react, preact, svelte, vue].map((r) => r.layout.pages.length);
    expect(pages).toEqual([pages[0], pages[0], pages[0], pages[0]]);
    expect(pages[0]).toBeGreaterThan(0);

    // Layout-tree parity (the equivalence gate observed end-to-end)
    const ref = layoutSignature(react.layout);
    expect(layoutSignature(preact.layout)).toEqual(ref);
    expect(layoutSignature(svelte.layout)).toEqual(ref);
    expect(layoutSignature(vue.layout)).toEqual(ref);
  });
});
