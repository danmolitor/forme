/**
 * WASM smoke test: the serialized document model renders to real PDF
 * bytes through @formepdf/core (devDependency - the adapter itself
 * never depends on WASM).
 */
import { readFile } from 'node:fs/promises';
import { describe, it, expect } from 'vitest';
import { renderPdf } from '@formepdf/core';
import { render, Font } from '../src/index.js';
import { decompressedStreams } from './pdf-streams.js';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorld from './fixtures/hello-world.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FixedPageNumbers from './fixtures/fixed-page-numbers.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import TableFixture from './fixtures/table.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import MediaFixture from './fixtures/media.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import VectorExtras from './fixtures/vector-extras.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import ChartsFixture from './fixtures/charts.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FormFieldsFixture from './fixtures/form-fields.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FontsFixture from './fixtures/fonts.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import SemanticsFixture from './fixtures/semantics.svelte';

describe('WASM smoke', () => {
  it('renders a serialized .svelte template to valid PDF bytes', async () => {
    const json = await render(HelloWorld, {
      props: { name: 'Svelte', items: ['alpha', 'beta'], showFooter: true },
    });
    const pdf = await renderPdf(json);

    expect(pdf).toBeInstanceOf(Uint8Array);
    expect(pdf.length).toBeGreaterThan(500);
    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('renders a 50-row table with a header row to a multi-page PDF', async () => {
    const json = await render(TableFixture);
    const pdf = await renderPdf(json);

    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    // One /Type /Page per page (the page-tree root is /Type /Pages).
    const raw = Buffer.from(pdf).toString('latin1');
    const pageCount = (raw.match(/\/Type\s*\/Page[^s]/g) ?? []).length;
    expect(pageCount).toBeGreaterThan(1);
    // The header row repeats on continuation pages: "SKU" is drawn
    // once per page in the content streams.
    const text = decompressedStreams(pdf);
    const headerDraws = (text.match(/\(SKU\)/g) ?? []).length;
    expect(headerDraws).toBe(pageCount);
  });

  it('renders media leaves (Image, Svg, QrCode, Barcode) in one document', async () => {
    const json = await render(MediaFixture, { props: { ticketId: 'TKT-0042' } });
    const pdf = await renderPdf(json);

    expect(pdf).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    // QR modules and barcode bars are drawn as rectangle ops (`re`) in
    // the content stream - a media-free page has nowhere near this
    // many. The data-URI image becomes an XObject (`/Im0 Do`).
    const text = decompressedStreams(pdf);
    const rectOps = (text.match(/\bre\b/g) ?? []).length;
    expect(rectOps).toBeGreaterThan(100);
    expect(text).toMatch(/\/Im\d+ Do/);
  });

  it('renders Canvas, Watermark, and an explicit PageBreak to a two-page PDF', async () => {
    const json = await render(VectorExtras);
    const pdf = await renderPdf(json);

    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    // The explicit <PageBreak /> splits the document into exactly two
    // pages (the page-tree root is /Type /Pages).
    const raw = Buffer.from(pdf).toString('latin1');
    const pageCount = (raw.match(/\/Type\s*\/Page[^s]/g) ?? []).length;
    expect(pageCount).toBe(2);
    // The watermark text repeats on both pages (drawn as hex strings:
    // "DRAFT" and "CONFIDENTIAL"); canvas ops land in the first page's
    // content stream as vector path commands (bezier `c` operators).
    const text = decompressedStreams(pdf);
    expect((text.match(/<4452414654> Tj/g) ?? []).length).toBe(2);
    expect((text.match(/<434F4E464944454E5449414C> Tj/g) ?? []).length).toBe(2);
    expect(text).toContain(' c\n');
    expect(text).toContain('(Second page)');
  });

  it('renders a dashboard with all five chart types', async () => {
    const json = await render(ChartsFixture);
    const pdf = await renderPdf(json);

    expect(pdf).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    const text = decompressedStreams(pdf);
    // Axis/legend labels and titles are drawn as literal text with the
    // built-in Helvetica: one recognizable string per chart type.
    expect(text).toContain('(Revenue by quarter)');
    expect(text).toContain('(Traffic sources)');
    expect(text).toContain('(Monthly actives)');
    expect(text).toContain('(Server load)');
    // The dot plot draws its x-axis label (the engine currently never
    // draws y_label, though it round-trips through serialization).
    expect(text).toContain('(Dose)');
    // Chart geometry lands as vector ops: bars as rect fills, pie
    // sectors as bezier curves.
    expect((text.match(/\bre\b/g) ?? []).length).toBeGreaterThan(5);
    expect(text).toContain(' c\n');
  });

  it('renders a registration form with AcroForm fields', async () => {
    const json = await render(FormFieldsFixture);
    const pdf = await renderPdf(json);

    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    // AcroForm widgets are plain (uncompressed) annotation dictionaries:
    // the catalog carries /AcroForm, each field a /T (name) entry with
    // its field type (/FT /Tx text, /Btn button, /Ch choice).
    const raw = Buffer.from(pdf).toString('latin1');
    expect(raw).toContain('/AcroForm');
    for (const name of ['full_name', 'bio', 'access_code', 'agree_terms', 'newsletter', 'country', 'plan_type']) {
      expect(raw).toContain(`/T (${name})`);
    }
    expect(raw).toContain('/FT /Tx');
    expect(raw).toContain('/FT /Btn');
    expect(raw).toContain('/FT /Ch');
    // The radio group serializes as one field with three kid widgets
    // exporting /free, /pro, and /team states.
    expect(raw).toContain('/T (plan)');
    expect(raw).toContain('/free');
    expect(raw).toContain('/pro');
    expect(raw).toContain('/team');
  });

  it('renders with a registered custom TrueType font embedded', async () => {
    // NotoSans ships in the repo (engine/fonts), so this never depends
    // on system fonts and runs the same on CI.
    const ttf = await readFile(new URL('../../../engine/fonts/NotoSans-Regular.ttf', import.meta.url));
    Font.register({ family: 'NotoSans', src: `data:font/ttf;base64,${ttf.toString('base64')}` });
    try {
      const json = await render(FontsFixture, {
        props: { fonts: [], bodyFamily: 'NotoSans' },
      });
      const pdf = await renderPdf(json);

      const header = new TextDecoder().decode(pdf.slice(0, 5));
      expect(header).toBe('%PDF-');
      // The custom font is embedded, not substituted: its BaseFont name
      // and a TrueType font program appear in the PDF.
      const raw = Buffer.from(pdf).toString('latin1');
      expect(raw).toContain('NotoSans');
      expect(raw).toContain('/FontFile2');
    } finally {
      Font.clear();
    }
  });

  it('substitutes PAGE_NUMBER / TOTAL_PAGES in a multi-page footer', async () => {
    const json = await render(FixedPageNumbers, { props: { paragraphs: 40 } });
    const pdf = await renderPdf(json);
    const text = decompressedStreams(pdf);

    const first = text.match(/\(Page 1 of (\d+)\)/);
    expect(first).not.toBeNull();
    const total = Number(first![1]);
    expect(total).toBeGreaterThan(1);
    // The footer repeats on every page with the running page number.
    for (let page = 1; page <= total; page++) {
      expect(text).toContain(`(Page ${page} of ${total})`);
    }
    expect(text).not.toContain('{{pageNumber}}');
    expect(text).not.toContain('{{totalPages}}');
  });

  it('renders headings, lists, inline formatting, and transforms', async () => {
    const json = await render(SemanticsFixture, { props: { productName: 'Forme PDF' } });
    const pdf = await renderPdf(json);

    const header = new TextDecoder().decode(pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
    const text = decompressedStreams(pdf);
    // Heading and inline-run content is drawn.
    expect(text).toContain('Getting started with Forme PDF');
    expect(text).toContain('(both)');
    // Ordered list markers respect type + start (lower-roman from iii).
    expect(text).toContain('(iii.)');
    expect(text).toContain('(iv.)');
    // The transformed view emits a transform matrix (cm operator).
    expect(text).toMatch(/[\d.-]+ [\d.-]+ [\d.-]+ [\d.-]+ [\d.-]+ [\d.-]+ cm/);
  });
});
