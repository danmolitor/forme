import { describe, it, expect } from 'vitest';
import { renderPdf, renderPdfWithLayout, renderDocument } from '../src/index';

function minimalDoc(children: unknown[]) {
  return JSON.stringify({ children });
}

describe('renderPdf', () => {
  it('renders minimal text to PDF', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Hello' }, style: {} },
    ]);
    const bytes = await renderPdf(json);
    expect(bytes).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(bytes.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('renders multi-page document with PageBreak', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Page 1' }, style: {} },
      { kind: { type: 'PageBreak' }, style: {}, children: [] },
      { kind: { type: 'Text', content: 'Page 2' }, style: {} },
    ]);
    const bytes = await renderPdf(json);
    const content = new TextDecoder().decode(bytes);
    const pageCount = (content.match(/\/Type\s*\/Page[^s]/g) || []).length;
    expect(pageCount).toBeGreaterThanOrEqual(2);
  });

  it('rejects invalid JSON with error', async () => {
    await expect(renderPdf('not valid json {{')).rejects.toThrow('Failed to parse document');
  });

  it('renders empty document to valid PDF', async () => {
    const json = JSON.stringify({ children: [] });
    const bytes = await renderPdf(json);
    const header = new TextDecoder().decode(bytes.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('produces reasonably sized output', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Simple test' }, style: {} },
    ]);
    const bytes = await renderPdf(json);
    expect(bytes.length).toBeGreaterThan(100);
    expect(bytes.length).toBeLessThan(100_000);
  });
});

describe('renderPdfWithLayout', () => {
  it('returns object with pdf and layout', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Hello' }, style: {} },
    ]);
    const result = await renderPdfWithLayout(json);
    expect(result).toHaveProperty('pdf');
    expect(result).toHaveProperty('layout');
    expect(result.pdf).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(result.pdf.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('layout has pages array with dimensions', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Hello' }, style: {} },
    ]);
    const { layout } = await renderPdfWithLayout(json);
    expect(layout.pages).toBeInstanceOf(Array);
    expect(layout.pages.length).toBeGreaterThanOrEqual(1);

    const page = layout.pages[0];
    expect(page.width).toBeGreaterThan(0);
    expect(page.height).toBeGreaterThan(0);
    expect(typeof page.contentX).toBe('number');
    expect(typeof page.contentY).toBe('number');
    expect(page.contentWidth).toBeGreaterThan(0);
    expect(page.contentHeight).toBeGreaterThan(0);
  });

  it('layout elements have position, size, and kind', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Hello' }, style: {} },
    ]);
    const { layout } = await renderPdfWithLayout(json);
    const elements = layout.pages[0].elements;
    expect(elements.length).toBeGreaterThan(0);

    const el = elements[0];
    expect(typeof el.x).toBe('number');
    expect(typeof el.y).toBe('number');
    expect(typeof el.width).toBe('number');
    expect(typeof el.height).toBe('number');
    expect(typeof el.kind).toBe('string');
    expect(['Text', 'Rect', 'Image', 'None', 'ImagePlaceholder']).toContain(el.kind);
  });

  it('multi-page document has multiple pages in layout', async () => {
    const json = minimalDoc([
      { kind: { type: 'Text', content: 'Page 1' }, style: {} },
      { kind: { type: 'PageBreak' }, style: {}, children: [] },
      { kind: { type: 'Text', content: 'Page 2' }, style: {} },
    ]);
    const { layout } = await renderPdfWithLayout(json);
    expect(layout.pages.length).toBeGreaterThanOrEqual(2);
  });
});

describe('renderDocument', () => {
  // Dynamic import so these tests can be skipped if @formepdf/react isn't built
  let React: typeof import('react');
  let Components: typeof import('@formepdf/react');

  async function loadModules() {
    React = await import('react');
    Components = await import('@formepdf/react');
  }

  it('renders JSX Document with Text to PDF', async () => {
    await loadModules();
    const { Document, Text } = Components;
    const element = React.createElement(Document, null,
      React.createElement(Text, null, 'Hello from JSX')
    );
    const bytes = await renderDocument(element);
    expect(bytes).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(bytes.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('renders JSX with styles to PDF', async () => {
    await loadModules();
    const { Document, View, Text } = Components;
    const element = React.createElement(Document, null,
      React.createElement(View, { style: { backgroundColor: '#f0f0f0', padding: 20 } },
        React.createElement(Text, { style: { fontSize: 24, color: '#333333' } }, 'Styled text')
      )
    );
    const bytes = await renderDocument(element);
    expect(bytes).toBeInstanceOf(Uint8Array);
    const header = new TextDecoder().decode(bytes.slice(0, 5));
    expect(header).toBe('%PDF-');
  });

  it('renders JSX with table to PDF', async () => {
    await loadModules();
    const { Document, Table, Row, Cell, Text } = Components;
    const element = React.createElement(Document, null,
      React.createElement(Table, {
        columns: [
          { width: { fraction: 0.5 } },
          { width: { fraction: 0.5 } },
        ],
      },
        React.createElement(Row, { header: true },
          React.createElement(Cell, null, React.createElement(Text, null, 'Col A')),
          React.createElement(Cell, null, React.createElement(Text, null, 'Col B')),
        ),
        React.createElement(Row, null,
          React.createElement(Cell, null, React.createElement(Text, null, 'A1')),
          React.createElement(Cell, null, React.createElement(Text, null, 'B1')),
        ),
      )
    );
    const bytes = await renderDocument(element);
    expect(bytes).toBeInstanceOf(Uint8Array);
    expect(bytes.length).toBeGreaterThan(100);
  });
});
