/**
 * Component coverage: every exported component serializes to its expected
 * document-model node, mirroring @formepdf/svelte's parity suite shape.
 * The cross-framework equivalence test proves Vue === React on a full
 * document; these tests pin the per-component mapping so a regression
 * names the component that broke.
 */
import { describe, it, expect } from 'vitest';
import { serialize } from '../src/index.js';
// @ts-expect-error .vue fixtures have no type declarations in tests
import KitchenSink from './fixtures/kitchen-sink.vue';
// @ts-expect-error .vue fixtures have no type declarations in tests
import Booleans from './fixtures/booleans.vue';

type Node = { kind: Record<string, unknown> & { type: string }; style: Record<string, unknown>; children: Node[] };

function flat(n: Node): Node[] {
  return [n, ...n.children.flatMap(flat)];
}

describe('kitchen sink: every component maps to its node type', async () => {
  const doc = await serialize(KitchenSink);
  const page = (doc as { children: Node[] }).children[0];
  const nodes = flat(page);
  const byType = (t: string) => nodes.filter((n) => n.kind.type === t);

  it('document metadata and page config', () => {
    expect((doc as { metadata: unknown }).metadata).toEqual({ title: 'Kitchen sink' });
    expect(page.kind).toEqual({
      type: 'Page',
      config: {
        size: 'Letter',
        margin: { top: 36, right: 36, bottom: 36, left: 36 },
        wrap: true,
      },
    });
  });

  it('Fixed header wraps its content', () => {
    const [fixed] = byType('Fixed');
    expect(fixed.kind).toEqual({ type: 'Fixed', position: 'Header' });
    expect(fixed.children[0].kind).toEqual({ type: 'Text', content: 'Running head' });
  });

  it('H1–H6 map to Heading nodes with the right levels', () => {
    const headings = byType('Heading');
    expect(headings.map((h) => h.kind.level)).toEqual([1, 2, 3, 4, 5, 6]);
    expect(headings[0].kind.content).toBe('Heading one');
    expect(headings[5].kind.content).toBe('Heading six');
  });

  it('inline formatting produces styled runs inside one Text', () => {
    const runsText = nodes.find(
      (n) => n.kind.type === 'Text' && Array.isArray(n.kind.runs) && (n.kind.runs as { content: string }[]).some((r) => r.content === 'bold'),
    )!;
    const runs = runsText.kind.runs as { content: string; style?: Record<string, unknown> }[];
    expect(runs.find((r) => r.content === 'bold')?.style).toMatchObject({ fontWeight: 700 });
    expect(runs.find((r) => r.content === 'italic')?.style).toMatchObject({ fontStyle: 'Italic' });
    expect(runs.find((r) => r.content === 'mono')?.style).toMatchObject({ fontFamily: 'Courier' });
  });

  it('Link produces an underlined run with an href', () => {
    const linkText = nodes.find(
      (n) => n.kind.type === 'Text' && Array.isArray(n.kind.runs) && (n.kind.runs as { content: string }[]).some((r) => r.content === 'A link'),
    )!;
    const run = (linkText.kind.runs as { content: string; href?: string; style?: Record<string, unknown> }[]).find(
      (r) => r.content === 'A link',
    )!;
    expect(run.href).toBe('https://example.com');
    expect(run.style).toMatchObject({ textDecoration: 'Underline' });
  });

  it('ordered and unordered lists with items', () => {
    const lists = byType('List');
    expect(lists[0].kind).toMatchObject({ ordered: true, marker_type: 'decimal', start: 1 });
    expect(lists[1].kind).toMatchObject({ ordered: false, marker_type: 'disc' });
    expect(lists[0].children.map((c) => c.kind.type)).toEqual(['ListItem', 'ListItem']);
    expect(lists[0].children[0].children[0].kind).toEqual({ type: 'Text', content: 'First' });
  });

  it('table with fraction columns, header row, and cells', () => {
    const [table] = byType('Table');
    expect(table.kind.columns).toEqual([{ width: { Fraction: 0.5 } }, { width: { Fraction: 0.5 } }]);
    const rows = table.children;
    expect(rows.map((r) => r.kind)).toEqual([
      { type: 'TableRow', is_header: true },
      { type: 'TableRow', is_header: false },
    ]);
    expect(rows[0].children[0].kind).toEqual({ type: 'TableCell', col_span: 1, row_span: 1 });
    expect(rows[1].children[1].children[0].kind).toEqual({ type: 'Text', content: 'b1' });
  });

  it('media and vector leaves: Image, Svg, QrCode, Barcode', () => {
    expect(byType('Image')[0].kind).toEqual({ type: 'Image', src: 'logo.png', width: 40, height: 40 });
    expect(byType('Svg')[0].kind).toMatchObject({ width: 50, height: 20 });
    expect(byType('QrCode')[0].kind).toEqual({ type: 'QrCode', data: 'https://formepdf.com', size: 60 });
    expect(byType('Barcode')[0].kind).toEqual({
      type: 'Barcode', data: 'ABC-123', format: 'Code128', width: 120, height: 40,
    });
  });

  it('all five chart kinds serialize with snake_case engine props', () => {
    expect(byType('BarChart')[0].kind).toMatchObject({
      data: [{ label: 'A', value: 3 }, { label: 'B', value: 5 }], width: 200, height: 100,
    });
    expect(byType('LineChart')[0].kind).toMatchObject({ series: [{ name: 'S', data: [1, 2] }], width: 200, height: 100, show_points: false });
    expect(byType('PieChart')[0].kind).toMatchObject({ width: 120, height: 120 });
    expect(byType('AreaChart')[0].kind).toMatchObject({ series: [{ name: 'S', data: [1, 3] }], width: 200, height: 100, show_grid: false });
    expect(byType('DotPlot')[0].kind).toMatchObject({
      groups: [{ name: 'G', data: [[1, 1]] }], show_legend: false,
    });
  });

  it('Canvas, Watermark, PageBreak', () => {
    expect(byType('Canvas')[0].kind).toEqual({ type: 'Canvas', width: 80, height: 40, operations: [] });
    expect(byType('Watermark')[0].kind).toEqual({ type: 'Watermark', text: 'DRAFT', font_size: 48, angle: -45 });
    expect(byType('PageBreak')).toHaveLength(1);
  });

  it('all four form field kinds with defaults', () => {
    expect(byType('TextField')[0].kind).toMatchObject({ name: 'fullName', multiline: false, password: false });
    expect(byType('Checkbox')[0].kind).toMatchObject({ name: 'agree', checked: false });
    expect(byType('Dropdown')[0].kind).toMatchObject({ name: 'color', options: ['red', 'green'] });
    expect(byType('RadioButton')[0].kind).toMatchObject({ name: 'size', value: 'L', checked: false });
  });
});

describe('boolean-attribute coercion (Vue SSR "" → true)', async () => {
  // Vue SSR renders a bare attribute as "" where React/Svelte yield true;
  // encode.ts normalizes known boolean props back. Regression tests for
  // that fix — and for the inverse invariant: absence stays undefined and
  // explicit false stays false, which the shared parser depends on.
  const doc = (await serialize(Booleans)) as {
    tagged?: boolean;
    children: Node[];
  };
  const page = doc.children[0];
  const nodes = flat(page);

  it('bare tagged on Document coerces to true', () => {
    expect(doc.tagged).toBe(true);
  });

  it('bare wrap on Page coerces to true in the page config', () => {
    expect((page.kind.config as { wrap: boolean }).wrap).toBe(true);
  });

  it('bare header on Row coerces to is_header: true', () => {
    const rows = nodes.filter((n) => n.kind.type === 'TableRow');
    expect(rows.map((r) => r.kind.is_header)).toEqual([true, false]);
  });

  it('bare multiline on TextField coerces to true', () => {
    const fields = nodes.filter((n) => n.kind.type === 'TextField');
    expect(fields.find((f) => f.kind.name === 'notes')!.kind.multiline).toBe(true);
    expect(fields.find((f) => f.kind.name === 'plain')!.kind.multiline).toBe(false);
  });

  it('explicit :wrap="false" is preserved as false, absence as undefined', () => {
    const views = nodes.filter((n) => n.kind.type === 'View');
    expect(views[0].style.wrap).toBe(false);
    expect('wrap' in views[1].style).toBe(false);
  });
});

describe('undefined-vs-false at the document level', () => {
  it('a document without the tagged attribute serializes without a tagged key', async () => {
    const { default: HelloWorld } = await import('./fixtures/hello-world.vue');
    const doc = (await serialize(HelloWorld)) as { tagged?: boolean };
    expect('tagged' in doc).toBe(false);
  });
});
