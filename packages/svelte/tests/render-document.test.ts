/**
 * renderDocument wrapper tests: the one-call convenience over the
 * optional @formepdf/core peer (installed here as a devDependency).
 * Core render options must behave exactly as they do on the react
 * path: embedData round-trips through core's extraction API and
 * flattenForms strips interactive fields.
 */
import { describe, it, expect } from 'vitest';
import { extractData } from '@formepdf/core';
import { renderDocument, renderDocumentWithLayout } from '../src/index.js';
import { decompressedStreams } from './pdf-streams.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FormFieldsFixture from './fixtures/form-fields.svelte';

const helloProps = { name: 'Svelte', items: ['alpha', 'beta'], showFooter: true };

describe('renderDocument', () => {
  it('returns PDF bytes from a SvelteKit-endpoint-shaped handler', async () => {
    // The three-line +server.ts route from PRD story 3, verbatim.
    const GET = async () =>
      new Response(await renderDocument(HelloWorld, { props: helloProps }), {
        headers: { 'Content-Type': 'application/pdf' },
      });

    const response = await GET();
    const pdf = new Uint8Array(await response.arrayBuffer());

    expect(response.headers.get('Content-Type')).toBe('application/pdf');
    expect(pdf.length).toBeGreaterThan(500);
    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
  });

  it('renderDocumentWithLayout returns pages JSON alongside the PDF', async () => {
    const { pdf, layout } = await renderDocumentWithLayout(HelloWorld, { props: helloProps });

    expect(pdf).toBeInstanceOf(Uint8Array);
    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
    // Same shape core's react-facing renderDocumentWithLayout returns:
    // pages with dimensions, content box, and positioned elements.
    expect(Array.isArray(layout.pages)).toBe(true);
    expect(layout.pages.length).toBeGreaterThan(0);
    const page = layout.pages[0];
    expect(page.width).toBeGreaterThan(0);
    expect(page.height).toBeGreaterThan(0);
    expect(page.contentWidth).toBeGreaterThan(0);
    expect(page.contentHeight).toBeGreaterThan(0);
    expect(Array.isArray(page.elements)).toBe(true);
    expect(page.elements.length).toBeGreaterThan(0);
  });

  it('round-trips embedData through core extraction', async () => {
    const embedData = { invoice: 'INV-042', total: 129.5, lines: [{ sku: 'A1', qty: 2 }] };
    const pdf = await renderDocument(HelloWorld, { props: helloProps, embedData });

    expect(new TextDecoder().decode(pdf.slice(0, 5))).toBe('%PDF-');
    expect(extractData(pdf)).toEqual(embedData);
  });

  it('renders no embedded data when embedData is omitted', async () => {
    const pdf = await renderDocument(HelloWorld, { props: helloProps });
    expect(extractData(pdf)).toBeNull();
  });

  it('flattenForms passes through and strips interactive fields', async () => {
    const interactive = await renderDocument(FormFieldsFixture);
    const flattened = await renderDocument(FormFieldsFixture, { flattenForms: true });

    const interactiveRaw = Buffer.from(interactive).toString('latin1');
    expect(interactiveRaw).toContain('/AcroForm');
    expect(interactiveRaw).toContain('/FT /Tx');

    const flattenedRaw = Buffer.from(flattened).toString('latin1');
    expect(flattenedRaw).not.toContain('/AcroForm');
    expect(flattenedRaw).not.toContain('/FT /Tx');
    // Field values survive as static page content.
    const text = decompressedStreams(flattened);
    expect(text).toContain('(Hello!)');
    expect(text).toContain('(UK)');
  });
});
