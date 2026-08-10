/**
 * Cross-adapter parity: equivalent documents authored in TSX and
 * .svelte must serialize to deep-equal document-model JSON. This is
 * the drift alarm between @formepdf/react and @formepdf/svelte.
 */
import { describe, it, expect } from 'vitest';
import { serialize as serializeReact } from '@formepdf/react';
import { serialize, Font } from '../src/index.js';
import HelloWorldReact from './fixtures/hello-world';
import KitchenSinkReact from './fixtures/kitchen-sink';
import TextRunsReact from './fixtures/text-runs';
import FixedPageNumbersReact from './fixtures/fixed-page-numbers';
import TableReact from './fixtures/table';
import MediaReact from './fixtures/media';
import VectorExtrasReact from './fixtures/vector-extras';
import ChartsReact from './fixtures/charts';
import FormFieldsReact from './fixtures/form-fields';
import FontsReact from './fixtures/fonts';
import TailwindReact from './fixtures/tailwind';
import SemanticsReact from './fixtures/semantics';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import HelloWorldSvelte from './fixtures/hello-world.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import KitchenSinkSvelte from './fixtures/kitchen-sink.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import TextRunsSvelte from './fixtures/text-runs.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FixedPageNumbersSvelte from './fixtures/fixed-page-numbers.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import TableSvelte from './fixtures/table.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import MediaSvelte from './fixtures/media.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import VectorExtrasSvelte from './fixtures/vector-extras.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import ChartsSvelte from './fixtures/charts.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FormFieldsSvelte from './fixtures/form-fields.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import FontsSvelte from './fixtures/fonts.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import TailwindSvelte from './fixtures/tailwind.svelte';
// @ts-expect-error .svelte fixtures have no type declarations in tests
import SemanticsSvelte from './fixtures/semantics.svelte';

/**
 * Merge adjacent text runs whose style + href are identical, recursively
 * through the node tree. React splits runs at JSX child boundaries
 * (`<Strong>${'$'}{price}.00</Strong>` is three string children, so three
 * runs); Svelte's SSR output merges contiguous text before the parser
 * ever sees it, so those boundaries cannot be reproduced. Adjacent runs
 * with equal style render identically, so parity is compared on the
 * normalized form. Runs with DIFFERENT styles are never merged - real
 * style drift still fails.
 */
function normalizeRuns(doc: unknown): unknown {
  return JSON.parse(JSON.stringify(doc, (_key, value) => {
    if (
      value !== null && typeof value === 'object' && !Array.isArray(value) &&
      Array.isArray((value as { runs?: unknown }).runs)
    ) {
      const node = value as { runs: { content: string; style?: unknown; href?: string }[] };
      const merged: typeof node.runs = [];
      for (const run of node.runs) {
        const prev = merged[merged.length - 1];
        if (
          prev &&
          JSON.stringify(prev.style) === JSON.stringify(run.style) &&
          prev.href === run.href
        ) {
          prev.content += run.content;
        } else {
          merged.push({ ...run });
        }
      }
      return { ...node, runs: merged };
    }
    return value;
  }));
}

function expectParity(svelteDoc: unknown, reactDoc: unknown): void {
  expect(normalizeRuns(svelteDoc)).toEqual(normalizeRuns(reactDoc));
}

describe('cross-adapter parity', () => {
  it('hello-world: interpolation, #each/#if vs map/&&', async () => {
    const props = { name: 'Svelte', items: ['alpha', 'beta'], showFooter: true };
    const svelteDoc = await serialize(HelloWorldSvelte, { props });
    const reactDoc = serializeReact(<HelloWorldReact {...props} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('hello-world with default props', async () => {
    const svelteDoc = await serialize(HelloWorldSvelte);
    const reactDoc = serializeReact(<HelloWorldReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('kitchen-sink: document props, CSS shorthands, multi-line text', async () => {
    const svelteDoc = await serialize(KitchenSinkSvelte, { props: { discount: 25 } });
    const reactDoc = serializeReact(<KitchenSinkReact discount={25} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('text-runs: nested spans, run styles, boundary whitespace', async () => {
    const svelteDoc = await serialize(TextRunsSvelte, { props: { price: 42 } });
    const reactDoc = serializeReact(<TextRunsReact price={42} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('table: 50-row #each, header row, column widths, spans, view cells', async () => {
    const svelteDoc = await serialize(TableSvelte);
    const reactDoc = serializeReact(<TableReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('media: Image src pass-through, Svg content, QrCode, Barcode defaults', async () => {
    const svelteDoc = await serialize(MediaSvelte, { props: { ticketId: 'TKT-7777' } });
    const reactDoc = serializeReact(<MediaReact ticketId="TKT-7777" />);
    expectParity(svelteDoc, reactDoc);
  });

  it('media with default props', async () => {
    const svelteDoc = await serialize(MediaSvelte);
    const reactDoc = serializeReact(<MediaReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('vector-extras: Canvas draw recording, Watermark rgba/defaults, PageBreak', async () => {
    const props = { accent: [239, 68, 68] as [number, number, number] };
    const svelteDoc = await serialize(VectorExtrasSvelte, { props });
    const reactDoc = serializeReact(<VectorExtrasReact {...props} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('vector-extras with default props', async () => {
    const svelteDoc = await serialize(VectorExtrasSvelte);
    const reactDoc = serializeReact(<VectorExtrasReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('charts: all five types, multi-series, groups, per-datum color overrides', async () => {
    const props = { highlight: '#dc2626' };
    const svelteDoc = await serialize(ChartsSvelte, { props });
    const reactDoc = serializeReact(<ChartsReact {...props} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('charts with default props (defaulted chart options included)', async () => {
    const svelteDoc = await serialize(ChartsSvelte);
    const reactDoc = serializeReact(<ChartsReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('form-fields: TextField flags, Dropdown options, radio group', async () => {
    const svelteDoc = await serialize(FormFieldsSvelte, { props: { plan: 'team' } });
    const reactDoc = serializeReact(<FormFieldsReact plan="team" />);
    expectParity(svelteDoc, reactDoc);
  });

  it('form-fields with default props (radio group default selection)', async () => {
    const svelteDoc = await serialize(FormFieldsSvelte);
    const reactDoc = serializeReact(<FormFieldsReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('fixed-page-numbers: header/footer, placeholder constants', async () => {
    const svelteDoc = await serialize(FixedPageNumbersSvelte, { props: { paragraphs: 5 } });
    const reactDoc = serializeReact(<FixedPageNumbersReact paragraphs={5} />);
    expectParity(svelteDoc, reactDoc);
  });

  it('fonts: Font.register globals merge with document fonts, byte srcs intact', async () => {
    // The Font store is the shared singleton: one registration is
    // visible to both adapters' serializers.
    Font.register({ family: 'Global', src: 'global.ttf', fontWeight: 'bold' });
    Font.register({ family: 'Inter', src: 'global-inter.ttf' });
    try {
      const svelteDoc = await serialize(FontsSvelte);
      const reactDoc = serializeReact(<FontsReact />);
      expectParity(svelteDoc, reactDoc);
      // Document fonts win the family:weight:italic conflict with globals.
      const inter = svelteDoc.fonts!.find(f => f.family === 'Inter' && f.weight === 400);
      expect(inter!.src).toBe('fonts/Inter-Regular.ttf');
      // Byte sources survive the markup round-trip as real byte arrays.
      const bytes = svelteDoc.fonts!.find(f => f.family === 'Bytes');
      expect(bytes!.src).toBeInstanceOf(Uint8Array);
    } finally {
      Font.clear();
    }
  });

  it('fonts with default props (no globals registered)', async () => {
    const svelteDoc = await serialize(FontsSvelte);
    const reactDoc = serializeReact(<FontsReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('tailwind: templates styled entirely with tw() classes', async () => {
    const svelteDoc = await serialize(TailwindSvelte, { props: { total: '$99.00' } });
    const reactDoc = serializeReact(<TailwindReact total="$99.00" />);
    expectParity(svelteDoc, reactDoc);
  });

  it('tailwind with default props', async () => {
    const svelteDoc = await serialize(TailwindSvelte);
    const reactDoc = serializeReact(<TailwindReact />);
    expectParity(svelteDoc, reactDoc);
  });

  it('semantics: headings, lists, inline formatting, transform styles', async () => {
    const svelteDoc = await serialize(SemanticsSvelte, { props: { productName: 'Forme PDF' } });
    const reactDoc = serializeReact(<SemanticsReact productName="Forme PDF" />);
    expectParity(svelteDoc, reactDoc);
  });

  it('semantics with default props', async () => {
    const svelteDoc = await serialize(SemanticsSvelte);
    const reactDoc = serializeReact(<SemanticsReact />);
    expectParity(svelteDoc, reactDoc);
  });
});
