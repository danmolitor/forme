/**
 * renderDocument / renderDocumentWithLayout — the one-call PDF path over
 * the optional @formepdf/core peer (a devDependency here, as in svelte's
 * wasm-smoke suite). Verifies real WASM output shape, including warnings.
 */
import { describe, it, expect } from 'vitest';
import { renderDocument, renderDocumentWithLayout } from '../src/index.js';
// @ts-expect-error .vue fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.vue';
// @ts-expect-error .vue fixtures have no type declarations in tests
import KitchenSink from './fixtures/kitchen-sink.vue';

describe('renderDocument', () => {
  it('renders a .vue template to valid PDF bytes', async () => {
    const pdf = await renderDocument(HelloWorld, {
      props: { name: 'Vue', items: ['alpha'], showFooter: true },
    });
    expect(pdf).toBeInstanceOf(Uint8Array);
    expect(pdf.length).toBeGreaterThan(500);
    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
  });

  it('renders the kitchen sink (every component) without error', async () => {
    const pdf = await renderDocument(KitchenSink);
    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
  });
});

describe('renderDocumentWithLayout', () => {
  it('returns pdf bytes, layout info, and a warnings array', async () => {
    const result = await renderDocumentWithLayout(HelloWorld, {
      props: { name: 'Vue' },
    });
    expect(result.pdf).toBeInstanceOf(Uint8Array);
    expect(new TextDecoder().decode(result.pdf.slice(0, 5))).toBe('%PDF-');
    expect(result.layout).toBeTypeOf('object');
    expect(Array.isArray(result.layout.pages)).toBe(true);
    expect(result.layout.pages.length).toBeGreaterThan(0);
    expect(Array.isArray(result.warnings)).toBe(true);
  });

  it('layout page count matches the document (kitchen sink has a PageBreak)', async () => {
    const result = await renderDocumentWithLayout(KitchenSink);
    expect(result.layout.pages.length).toBeGreaterThanOrEqual(2);
  });
});
