/**
 * Every code sample in docs/svelte.mdx and the package README must
 * serialize - and, where the docs promise PDF bytes, render - when
 * pasted into a fixture. The fixtures under tests/fixtures/docs/ are
 * verbatim copies of the samples (same '@formepdf/svelte' imports,
 * resolved to the source tree via the vitest alias); keep them in sync
 * when the docs change.
 */
import { describe, it, expect, afterEach } from 'vitest';
import { serialize, renderDocument, Font } from '@formepdf/svelte';
import { formePreview } from '@formepdf/svelte/preview';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import Invoice from './fixtures/docs/invoice.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import TextRuns from './fixtures/docs/text-runs.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import PageNumbers from './fixtures/docs/page-numbers.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import Tailwind from './fixtures/docs/tailwind.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import CanvasDoc from './fixtures/docs/canvas.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import ReadmeHello from './fixtures/docs/readme-hello.svelte';

afterEach(() => {
  Font.clear();
});

function expectPdf(bytes: Uint8Array) {
  expect(bytes).toBeInstanceOf(Uint8Array);
  expect(new TextDecoder().decode(bytes.slice(0, 5))).toBe('%PDF-');
}

describe('docs/svelte.mdx samples', () => {
  it('quickstart: Invoice.svelte serializes and the endpoint renderDocument call produces a PDF response', async () => {
    const doc = await serialize(Invoice, { props: { invoiceNo: '001' } });
    expect(doc.metadata).toEqual({ title: 'Invoice #001' });
    const page = doc.children[0];
    const texts = JSON.stringify(page);
    expect(texts).toContain('Website Redesign');
    expect(texts).toContain('$3500.00');
    expect(texts).toContain('Total: $4100.00');

    // The +server.ts sample body, minus the $lib import.
    const pdf = await renderDocument(Invoice, {
      props: { invoiceNo: '001', customer: 'Jane Smith' },
    });
    expectPdf(pdf);
    const response = new Response(pdf, { headers: { 'Content-Type': 'application/pdf' } });
    expect(response.status).toBe(200);
    expect(response.headers.get('Content-Type')).toBe('application/pdf');
  });

  it('component parity: nested <Text> spans serialize to styled text runs', async () => {
    const doc = await serialize(TextRuns, { props: { price: 42 } });
    const text = doc.children[0].children[0];
    expect(text.kind.type).toBe('Text');
    const runs = (text.kind as { runs?: Array<{ content: string }> }).runs!;
    expect(runs.map(r => r.content).join('')).toBe('Was $56.00 $42.00 due now');
    expect(runs.some(r => JSON.stringify(r).includes('LineThrough'))).toBe(true);
  });

  it('page numbers: PAGE_NUMBER / TOTAL_PAGES interpolate as engine placeholders', async () => {
    const doc = await serialize(PageNumbers);
    const json = JSON.stringify(doc);
    expect(json).toContain('Page {{pageNumber}} of {{totalPages}}');
  });

  it('fonts: <script module> Font.register() feeds the serialized document', async () => {
    // Dynamic import so module-scope registration happens inside this
    // test (and Font.clear() in afterEach undoes it).
    // @ts-expect-error .svelte fixtures have no type declarations in tests
    const { default: Fonts } = await import('./fixtures/docs/fonts.svelte');
    const doc = await serialize(Fonts);
    expect(doc.fonts).toEqual([
      { family: 'Inter', src: './fonts/Inter-Regular.ttf', weight: 400, italic: false },
      { family: 'Inter', src: './fonts/Inter-Bold.ttf', weight: 700, italic: false },
    ]);
    expect(doc.defaultStyle).toEqual({ fontFamily: 'Inter' });
  });

  it('tailwind: tw() styles serialize', async () => {
    const doc = await serialize(Tailwind);
    const view = doc.children[0].children[0];
    expect(view.style?.flexDirection).toBe('Row');
    expect(view.style?.justifyContent).toBe('SpaceBetween');
    expect(view.children[0].style?.fontWeight).toBe(700);
  });

  it('custom graphics: the Canvas draw callback records operations', async () => {
    const doc = await serialize(CanvasDoc);
    const canvas = doc.children[0].children[0];
    expect(canvas.kind.type).toBe('Canvas');
    const ops = (canvas.kind as { operations?: Array<{ op: string }> }).operations!;
    expect(ops.map(o => o.op)).toEqual(['SetFillColor', 'Circle', 'Fill']);
  });

  it('live preview: the formePreview sample serves the preview page', async () => {
    const GET = formePreview(Invoice, { props: { invoiceNo: '001' } });
    const response = await GET({
      url: new URL('http://localhost:5173/dev/pdf'),
      params: { forme: '' },
    });
    expect(response.status).toBe(200);
    expect(response.headers.get('Content-Type')).toContain('text/html');
  });

  it('render options: embedData and flattenForms pass through to core', async () => {
    const pdf = await renderDocument(Invoice, {
      props: { invoiceNo: '001' },
      embedData: { invoiceNo: '001', total: 4100 },
      flattenForms: true,
    });
    expectPdf(pdf);
    // The embedded forme-data.json FileSpec proves embedData reached core.
    expect(Buffer.from(pdf).toString('latin1')).toContain('forme-data.json');
  });
});

describe('README samples', () => {
  it('usage: the hello template serializes and renders through the endpoint sample', async () => {
    const doc = await serialize(ReadmeHello, { props: { name: 'Forme' } });
    expect(JSON.stringify(doc)).toContain('Hello Forme');

    const pdf = await renderDocument(ReadmeHello, { props: { name: 'Forme' } });
    expectPdf(pdf);
  });
});
