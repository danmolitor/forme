import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import {
  Document,
  Page,
  View,
  Text,
  Image,
  Table,
  Row,
  Cell,
  Fixed,
  PageBreak,
  Svg,
  serialize,
  render,
  mapStyle,
  mapDimension,
  parseColor,
  expandEdges,
  expandCorners,
  StyleSheet,
  Font,
} from '../src/index';

// ─── Component → JSON structure ─────────────────────────────────────

describe('Component serialization', () => {
  it('Text produces correct kind', () => {
    const doc = serialize(<Document><Text>hello</Text></Document>);
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'hello' });
  });

  it('View produces correct kind with children', () => {
    const doc = serialize(
      <Document>
        <View>
          <Text>child</Text>
        </View>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'View' });
    expect(doc.children[0].children).toHaveLength(1);
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: 'child' });
  });

  it('Image produces correct kind', () => {
    const doc = serialize(<Document><Image src="logo.png" width={100} height={50} /></Document>);
    expect(doc.children[0].kind).toEqual({ type: 'Image', src: 'logo.png', width: 100, height: 50 });
  });

  it('Image omits undefined width/height', () => {
    const doc = serialize(<Document><Image src="logo.png" /></Document>);
    const kind = doc.children[0].kind;
    expect(kind).toEqual({ type: 'Image', src: 'logo.png' });
    expect('width' in kind).toBe(false);
    expect('height' in kind).toBe(false);
  });

  it('Table/Row/Cell structure', () => {
    const doc = serialize(
      <Document>
        <Table columns={[{ width: { fraction: 0.5 } }, { width: { fixed: 100 } }]}>
          <Row header>
            <Cell><Text>Header 1</Text></Cell>
            <Cell colSpan={2}><Text>Header 2</Text></Cell>
          </Row>
          <Row>
            <Cell><Text>Data 1</Text></Cell>
            <Cell><Text>Data 2</Text></Cell>
          </Row>
        </Table>
      </Document>
    );

    const table = doc.children[0];
    expect(table.kind).toEqual({
      type: 'Table',
      columns: [
        { width: { Fraction: 0.5 } },
        { width: { Fixed: 100 } },
      ],
    });

    const headerRow = table.children[0];
    expect(headerRow.kind).toEqual({ type: 'TableRow', is_header: true });

    const dataRow = table.children[1];
    expect(dataRow.kind).toEqual({ type: 'TableRow', is_header: false });

    const cell2 = headerRow.children[1];
    expect(cell2.kind).toEqual({ type: 'TableCell', col_span: 2, row_span: 1 });
  });

  it('Fixed header/footer', () => {
    const doc = serialize(
      <Document>
        <Fixed position="header"><Text>Header</Text></Fixed>
        <Fixed position="footer"><Text>Footer</Text></Fixed>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'Fixed', position: 'Header' });
    expect(doc.children[1].kind).toEqual({ type: 'Fixed', position: 'Footer' });
  });

  it('PageBreak', () => {
    const doc = serialize(
      <Document>
        <Text>Before</Text>
        <PageBreak />
        <Text>After</Text>
      </Document>
    );
    expect(doc.children[1].kind).toEqual({ type: 'PageBreak' });
  });
});

// ─── Style mapping ──────────────────────────────────────────────────

describe('Style mapping', () => {
  it('flexDirection mapping', () => {
    expect(mapStyle({ flexDirection: 'row' }).flexDirection).toBe('Row');
    expect(mapStyle({ flexDirection: 'column' }).flexDirection).toBe('Column');
    expect(mapStyle({ flexDirection: 'row-reverse' }).flexDirection).toBe('RowReverse');
    expect(mapStyle({ flexDirection: 'column-reverse' }).flexDirection).toBe('ColumnReverse');
  });

  it('justifyContent mapping', () => {
    expect(mapStyle({ justifyContent: 'space-between' }).justifyContent).toBe('SpaceBetween');
    expect(mapStyle({ justifyContent: 'space-around' }).justifyContent).toBe('SpaceAround');
    expect(mapStyle({ justifyContent: 'space-evenly' }).justifyContent).toBe('SpaceEvenly');
    expect(mapStyle({ justifyContent: 'flex-start' }).justifyContent).toBe('FlexStart');
    expect(mapStyle({ justifyContent: 'flex-end' }).justifyContent).toBe('FlexEnd');
    expect(mapStyle({ justifyContent: 'center' }).justifyContent).toBe('Center');
  });

  it('alignItems mapping', () => {
    expect(mapStyle({ alignItems: 'flex-start' }).alignItems).toBe('FlexStart');
    expect(mapStyle({ alignItems: 'flex-end' }).alignItems).toBe('FlexEnd');
    expect(mapStyle({ alignItems: 'center' }).alignItems).toBe('Center');
    expect(mapStyle({ alignItems: 'stretch' }).alignItems).toBe('Stretch');
    expect(mapStyle({ alignItems: 'baseline' }).alignItems).toBe('Baseline');
  });

  it('flexWrap mapping', () => {
    expect(mapStyle({ flexWrap: 'nowrap' }).flexWrap).toBe('NoWrap');
    expect(mapStyle({ flexWrap: 'wrap' }).flexWrap).toBe('Wrap');
    expect(mapStyle({ flexWrap: 'wrap-reverse' }).flexWrap).toBe('WrapReverse');
  });

  it('fontWeight mapping', () => {
    expect(mapStyle({ fontWeight: 'bold' }).fontWeight).toBe(700);
    expect(mapStyle({ fontWeight: 'normal' }).fontWeight).toBe(400);
    expect(mapStyle({ fontWeight: 600 }).fontWeight).toBe(600);
  });

  it('fontStyle mapping', () => {
    expect(mapStyle({ fontStyle: 'italic' }).fontStyle).toBe('Italic');
    expect(mapStyle({ fontStyle: 'oblique' }).fontStyle).toBe('Oblique');
    expect(mapStyle({ fontStyle: 'normal' }).fontStyle).toBe('Normal');
  });

  it('textAlign mapping', () => {
    expect(mapStyle({ textAlign: 'left' }).textAlign).toBe('Left');
    expect(mapStyle({ textAlign: 'right' }).textAlign).toBe('Right');
    expect(mapStyle({ textAlign: 'center' }).textAlign).toBe('Center');
    expect(mapStyle({ textAlign: 'justify' }).textAlign).toBe('Justify');
  });

  it('textDecoration mapping', () => {
    expect(mapStyle({ textDecoration: 'underline' }).textDecoration).toBe('Underline');
    expect(mapStyle({ textDecoration: 'line-through' }).textDecoration).toBe('LineThrough');
    expect(mapStyle({ textDecoration: 'none' }).textDecoration).toBe('None');
  });

  it('textTransform mapping', () => {
    expect(mapStyle({ textTransform: 'uppercase' }).textTransform).toBe('Uppercase');
    expect(mapStyle({ textTransform: 'lowercase' }).textTransform).toBe('Lowercase');
    expect(mapStyle({ textTransform: 'capitalize' }).textTransform).toBe('Capitalize');
    expect(mapStyle({ textTransform: 'none' }).textTransform).toBe('None');
  });

  it('color hex parsing', () => {
    expect(mapStyle({ color: '#ff0000' }).color).toEqual({ r: 1, g: 0, b: 0, a: 1 });
    expect(mapStyle({ color: '#00ff00' }).color).toEqual({ r: 0, g: 1, b: 0, a: 1 });
    expect(mapStyle({ color: '#0000ff' }).color).toEqual({ r: 0, g: 0, b: 1, a: 1 });
  });

  it('dimension mapping', () => {
    expect(mapDimension(100)).toEqual({ Pt: 100 });
    expect(mapDimension('50%')).toEqual({ Percent: 50 });
    expect(mapDimension('auto')).toBe('Auto');
  });

  it('padding shorthand', () => {
    expect(mapStyle({ padding: 8 }).padding).toEqual({ top: 8, right: 8, bottom: 8, left: 8 });
  });

  it('padding with edges', () => {
    expect(mapStyle({ padding: { top: 10, right: 20, bottom: 30, left: 40 } }).padding).toEqual({
      top: 10, right: 20, bottom: 30, left: 40,
    });
  });

  it('borderRadius shorthand', () => {
    expect(mapStyle({ borderRadius: 4 }).borderRadius).toEqual({
      top_left: 4, top_right: 4, bottom_right: 4, bottom_left: 4,
    });
  });

  it('borderRadius with corners', () => {
    expect(mapStyle({
      borderRadius: { topLeft: 1, topRight: 2, bottomRight: 3, bottomLeft: 4 },
    }).borderRadius).toEqual({
      top_left: 1, top_right: 2, bottom_right: 3, bottom_left: 4,
    });
  });

  it('borderWidth shorthand', () => {
    expect(mapStyle({ borderWidth: 2 }).borderWidth).toEqual({
      top: 2, right: 2, bottom: 2, left: 2,
    });
  });

  it('borderColor string', () => {
    const result = mapStyle({ borderColor: '#ff0000' });
    const expected = { r: 1, g: 0, b: 0, a: 1 };
    expect(result.borderColor).toEqual({
      top: expected, right: expected, bottom: expected, left: expected,
    });
  });

  it('dimension width and height on style', () => {
    const style = mapStyle({ width: 200, height: '50%' });
    expect(style.width).toEqual({ Pt: 200 });
    expect(style.height).toEqual({ Percent: 50 });
  });

  it('flex properties pass through', () => {
    const style = mapStyle({ flexGrow: 1, flexShrink: 0, gap: 10, rowGap: 5, columnGap: 15 });
    expect(style.flexGrow).toBe(1);
    expect(style.flexShrink).toBe(0);
    expect(style.gap).toBe(10);
    expect(style.rowGap).toBe(5);
    expect(style.columnGap).toBe(15);
  });

  it('opacity and backgroundColor', () => {
    const style = mapStyle({ opacity: 0.5, backgroundColor: '#ffffff' });
    expect(style.opacity).toBe(0.5);
    expect(style.backgroundColor).toEqual({ r: 1, g: 1, b: 1, a: 1 });
  });
});

// ─── Color parsing ──────────────────────────────────────────────────

describe('parseColor', () => {
  it('parses 3-char hex', () => {
    expect(parseColor('#fff')).toEqual({ r: 1, g: 1, b: 1, a: 1 });
    expect(parseColor('#000')).toEqual({ r: 0, g: 0, b: 0, a: 1 });
  });

  it('parses 6-char hex', () => {
    expect(parseColor('#ff0000')).toEqual({ r: 1, g: 0, b: 0, a: 1 });
    expect(parseColor('#808080')).toEqual({
      r: 128 / 255,
      g: 128 / 255,
      b: 128 / 255,
      a: 1,
    });
  });

  it('parses 8-char hex with alpha', () => {
    expect(parseColor('#ff000080')).toEqual({
      r: 1,
      g: 0,
      b: 0,
      a: 128 / 255,
    });
  });

  it('handles missing # prefix', () => {
    expect(parseColor('ff0000')).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('returns black for invalid input', () => {
    expect(parseColor('invalid')).toEqual({ r: 0, g: 0, b: 0, a: 1 });
  });
});

// ─── Style shorthand properties ─────────────────────────────────────

describe('Style shorthand properties', () => {
  it('paddingTop only', () => {
    expect(mapStyle({ paddingTop: 10 }).padding).toEqual({ top: 10, right: 0, bottom: 0, left: 0 });
  });

  it('paddingHorizontal sets left and right', () => {
    expect(mapStyle({ paddingHorizontal: 16 }).padding).toEqual({ top: 0, right: 16, bottom: 0, left: 16 });
  });

  it('paddingVertical sets top and bottom', () => {
    expect(mapStyle({ paddingVertical: 12 }).padding).toEqual({ top: 12, right: 0, bottom: 12, left: 0 });
  });

  it('padding base + paddingTop override', () => {
    expect(mapStyle({ padding: 8, paddingTop: 12 }).padding).toEqual({ top: 12, right: 8, bottom: 8, left: 8 });
  });

  it('paddingVertical + paddingLeft override', () => {
    expect(mapStyle({ paddingVertical: 8, paddingLeft: 4 }).padding).toEqual({ top: 8, right: 0, bottom: 8, left: 4 });
  });

  it('paddingHorizontal + paddingVertical combined', () => {
    expect(mapStyle({ paddingVertical: 6, paddingHorizontal: 12 }).padding).toEqual({ top: 6, right: 12, bottom: 6, left: 12 });
  });

  it('padding base + axis + individual (full cascade)', () => {
    expect(mapStyle({ padding: 4, paddingVertical: 8, paddingTop: 16 }).padding).toEqual({ top: 16, right: 4, bottom: 8, left: 4 });
  });

  it('marginHorizontal sets left and right', () => {
    expect(mapStyle({ marginHorizontal: 20 }).margin).toEqual({ top: 0, right: 20, bottom: 0, left: 20 });
  });

  it('marginVertical + marginBottom override', () => {
    expect(mapStyle({ marginVertical: 10, marginBottom: 20 }).margin).toEqual({ top: 10, right: 0, bottom: 20, left: 0 });
  });

  it('marginBottom only', () => {
    expect(mapStyle({ marginBottom: 12 }).margin).toEqual({ top: 0, right: 0, bottom: 12, left: 0 });
  });

  it('borderBottomWidth only', () => {
    expect(mapStyle({ borderBottomWidth: 1 }).borderWidth).toEqual({ top: 0, right: 0, bottom: 1, left: 0 });
  });

  it('borderWidth base + borderTopWidth override', () => {
    expect(mapStyle({ borderWidth: 1, borderTopWidth: 3 }).borderWidth).toEqual({ top: 3, right: 1, bottom: 1, left: 1 });
  });

  it('borderTopColor only', () => {
    const result = mapStyle({ borderTopColor: '#ff0000' });
    expect(result.borderColor).toEqual({
      top: { r: 1, g: 0, b: 0, a: 1 },
      right: { r: 0, g: 0, b: 0, a: 1 },
      bottom: { r: 0, g: 0, b: 0, a: 1 },
      left: { r: 0, g: 0, b: 0, a: 1 },
    });
  });

  it('borderColor base + borderBottomColor override', () => {
    const result = mapStyle({ borderColor: '#000000', borderBottomColor: '#ff0000' });
    expect(result.borderColor!.top).toEqual({ r: 0, g: 0, b: 0, a: 1 });
    expect(result.borderColor!.bottom).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('borderTopLeftRadius only', () => {
    expect(mapStyle({ borderTopLeftRadius: 8 }).borderRadius).toEqual({ top_left: 8, top_right: 0, bottom_right: 0, bottom_left: 0 });
  });

  it('borderRadius base + corner overrides', () => {
    expect(mapStyle({ borderRadius: 4, borderTopLeftRadius: 8, borderBottomRightRadius: 12 }).borderRadius).toEqual({
      top_left: 8, top_right: 4, bottom_right: 12, bottom_left: 4,
    });
  });

  it('no shorthands returns undefined edges', () => {
    const style = mapStyle({ fontSize: 14 });
    expect(style.padding).toBeUndefined();
    expect(style.margin).toBeUndefined();
    expect(style.borderWidth).toBeUndefined();
    expect(style.borderColor).toBeUndefined();
    expect(style.borderRadius).toBeUndefined();
  });
});

// ─── Dimension mapping ──────────────────────────────────────────────

describe('mapDimension', () => {
  it('number to Pt', () => {
    expect(mapDimension(42)).toEqual({ Pt: 42 });
  });

  it('percentage string to Percent', () => {
    expect(mapDimension('75%')).toEqual({ Percent: 75 });
  });

  it('"auto" to Auto', () => {
    expect(mapDimension('auto')).toBe('Auto');
  });

  it('numeric string to Pt', () => {
    expect(mapDimension('100')).toEqual({ Pt: 100 });
  });
});

// ─── Edge expansion ─────────────────────────────────────────────────

describe('expandEdges', () => {
  it('uniform number', () => {
    expect(expandEdges(10)).toEqual({ top: 10, right: 10, bottom: 10, left: 10 });
  });

  it('explicit edges', () => {
    expect(expandEdges({ top: 1, right: 2, bottom: 3, left: 4 })).toEqual({
      top: 1, right: 2, bottom: 3, left: 4,
    });
  });
});

// ─── Corner expansion ───────────────────────────────────────────────

describe('expandCorners', () => {
  it('uniform number', () => {
    expect(expandCorners(5)).toEqual({
      top_left: 5, top_right: 5, bottom_right: 5, bottom_left: 5,
    });
  });

  it('explicit corners', () => {
    expect(expandCorners({ topLeft: 1, topRight: 2, bottomRight: 3, bottomLeft: 4 })).toEqual({
      top_left: 1, top_right: 2, bottom_right: 3, bottom_left: 4,
    });
  });
});

// ─── Document structure ─────────────────────────────────────────────

describe('Document structure', () => {
  it('Document with metadata', () => {
    const doc = serialize(
      <Document title="Invoice" author="Forme" subject="Test">
        <Text>Content</Text>
      </Document>
    );
    expect(doc.metadata).toEqual({ title: 'Invoice', author: 'Forme', subject: 'Test' });
  });

  it('Page with config', () => {
    const doc = serialize(
      <Document>
        <Page size="Letter" margin={36}>
          <Text>Content</Text>
        </Page>
      </Document>
    );

    const page = doc.children[0];
    expect(page.kind).toEqual({
      type: 'Page',
      config: {
        size: 'Letter',
        margin: { top: 36, right: 36, bottom: 36, left: 36 },
        wrap: true,
      },
    });
  });

  it('Page with custom size', () => {
    const doc = serialize(
      <Document>
        <Page size={{ width: 400, height: 600 }}>
          <Text>Content</Text>
        </Page>
      </Document>
    );

    const page = doc.children[0];
    const kind = page.kind as { type: 'Page'; config: { size: unknown } };
    expect(kind.config.size).toEqual({ Custom: { width: 400, height: 600 } });
  });

  it('default page config', () => {
    const doc = serialize(<Document><Text>hi</Text></Document>);
    expect(doc.defaultPage).toEqual({
      size: 'A4',
      margin: { top: 54, right: 54, bottom: 54, left: 54 },
      wrap: true,
    });
  });

  it('empty Document produces valid structure', () => {
    const doc = serialize(<Document />);
    expect(doc).toEqual({
      children: [],
      metadata: {},
      defaultPage: {
        size: 'A4',
        margin: { top: 54, right: 54, bottom: 54, left: 54 },
        wrap: true,
      },
    });
  });
});

// ─── Edge cases ─────────────────────────────────────────────────────

describe('Edge cases', () => {
  it('null children skipped', () => {
    const doc = serialize(
      <Document>
        <View>{null}</View>
      </Document>
    );
    expect(doc.children[0].children).toEqual([]);
  });

  it('boolean children skipped', () => {
    const doc = serialize(
      <Document>
        <View>{false}{true}</View>
      </Document>
    );
    expect(doc.children[0].children).toEqual([]);
  });

  it('string children auto-wrapped in Text node', () => {
    const doc = serialize(
      <Document>
        <View>hello</View>
      </Document>
    );
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: 'hello' });
  });

  it('number children auto-wrapped in Text node', () => {
    const doc = serialize(
      <Document>
        <View>{42}</View>
      </Document>
    );
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: '42' });
  });

  it('Text with nested Text produces runs', () => {
    const doc = serialize(
      <Document>
        <Text>Hello <Text>world</Text></Text>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({
      type: 'Text',
      content: '',
      runs: [
        { content: 'Hello ' },
        { content: 'world' },
      ],
    });
  });

  it('Text without nested Text still flattens to content', () => {
    const doc = serialize(
      <Document>
        <Text>Hello world</Text>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'Hello world' });
  });

  it('missing optional style props not included in output', () => {
    const style = mapStyle({ fontSize: 14 });
    expect(style.fontSize).toBe(14);
    expect('flexDirection' in style).toBe(false);
    expect('color' in style).toBe(false);
    expect('padding' in style).toBe(false);
  });

  it('top-level must be Document', () => {
    expect(() => serialize(<View />)).toThrow('Top-level element must be <Document>');
  });

  it('View wrap prop sets style.wrap', () => {
    const doc = serialize(
      <Document>
        <View wrap={false}><Text>content</Text></View>
      </Document>
    );
    expect(doc.children[0].style.wrap).toBe(false);
  });

  it('handles function components', () => {
    function MyComponent() {
      return <Text>from component</Text>;
    }
    const doc = serialize(
      <Document>
        <MyComponent />
      </Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'from component' });
  });

  it('handles column width Auto', () => {
    const doc = serialize(
      <Document>
        <Table columns={[{ width: 'auto' }]}>
          <Row><Cell><Text>data</Text></Cell></Row>
        </Table>
      </Document>
    );
    const kind = doc.children[0].kind as { type: 'Table'; columns: { width: unknown }[] };
    expect(kind.columns[0].width).toBe('Auto');
  });

  it('Fragment children are flattened', () => {
    const doc = serialize(
      <Document>
        <View>
          <>
            <Text>one</Text>
            <Text>two</Text>
          </>
        </View>
      </Document>
    );
    expect(doc.children[0].children).toHaveLength(2);
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: 'one' });
    expect(doc.children[0].children[1].kind).toEqual({ type: 'Text', content: 'two' });
  });

  it('conditional Fragment with Table children', () => {
    const showTable = true;
    const doc = serialize(
      <Document>
        <View>
          {showTable ? (
            <>
              <Table columns={[{ width: { fraction: 1 } }]}>
                <Row><Cell><Text>data</Text></Cell></Row>
              </Table>
              <Text>after table</Text>
            </>
          ) : (
            <Text>no table</Text>
          )}
        </View>
      </Document>
    );
    expect(doc.children[0].children).toHaveLength(2);
    expect((doc.children[0].children[0].kind as { type: string }).type).toBe('Table');
    expect(doc.children[0].children[1].kind).toEqual({ type: 'Text', content: 'after table' });
  });
});

// ─── Nesting validation ──────────────────────────────────────────────

describe('Nesting validation', () => {
  it('Row outside Table throws', () => {
    expect(() => serialize(
      <Document>
        <View>
          <Row><Cell><Text>oops</Text></Cell></Row>
        </View>
      </Document>
    )).toThrow(/Row.*must be inside.*Table/);
  });

  it('Cell outside Row throws', () => {
    expect(() => serialize(
      <Document>
        <Table>
          <Cell><Text>oops</Text></Cell>
        </Table>
      </Document>
    )).toThrow(/Cell.*must be inside.*Row/);
  });

  it('Row inside Table works', () => {
    expect(() => serialize(
      <Document>
        <Table>
          <Row><Cell><Text>ok</Text></Cell></Row>
        </Table>
      </Document>
    )).not.toThrow();
  });

  it('Cell inside Row works', () => {
    expect(() => serialize(
      <Document>
        <Table>
          <Row><Cell><Text>ok</Text></Cell></Row>
        </Table>
      </Document>
    )).not.toThrow();
  });

  it('Page inside View throws', () => {
    expect(() => serialize(
      <Document>
        <View>
          <Page><Text>oops</Text></Page>
        </View>
      </Document>
    )).toThrow(/Page.*must be.*child of.*Document/);
  });

  it('Text as child of Document still works', () => {
    expect(() => serialize(
      <Document><Text>hello</Text></Document>
    )).not.toThrow();
  });
});

// ─── Style mapping: widow/orphan lines ──────────────────────────────

describe('Widow/orphan style mapping', () => {
  it('minWidowLines maps through', () => {
    expect(mapStyle({ minWidowLines: 3 }).minWidowLines).toBe(3);
  });

  it('minOrphanLines maps through', () => {
    expect(mapStyle({ minOrphanLines: 2 }).minOrphanLines).toBe(2);
  });
});

// ─── Full round-trip ────────────────────────────────────────────────

describe('Full round-trip', () => {
  it('Invoice example', () => {
    const doc = serialize(
      <Document title="Invoice #001" author="Forme">
        <Page size="A4" margin={54}>
          <Fixed position="header">
            <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
              <Text style={{ fontSize: 24, fontWeight: 'bold' }}>INVOICE</Text>
              <Text style={{ fontSize: 12, color: '#666666' }}>Invoice #001</Text>
            </View>
          </Fixed>

          <View style={{ margin: { top: 40, right: 0, bottom: 20, left: 0 } }}>
            <Text style={{ fontSize: 14 }}>Bill To: Customer Inc.</Text>
          </View>

          <Table columns={[{ width: { fraction: 0.5 } }, { width: { fraction: 0.25 } }, { width: { fraction: 0.25 } }]}>
            <Row header>
              <Cell style={{ backgroundColor: '#333333', padding: 8 }}>
                <Text style={{ color: '#ffffff', fontWeight: 'bold' }}>Item</Text>
              </Cell>
              <Cell style={{ backgroundColor: '#333333', padding: 8 }}>
                <Text style={{ color: '#ffffff', fontWeight: 'bold' }}>Qty</Text>
              </Cell>
              <Cell style={{ backgroundColor: '#333333', padding: 8 }}>
                <Text style={{ color: '#ffffff', fontWeight: 'bold' }}>Price</Text>
              </Cell>
            </Row>
            <Row>
              <Cell style={{ padding: 8 }}><Text>Widget A</Text></Cell>
              <Cell style={{ padding: 8 }}><Text>10</Text></Cell>
              <Cell style={{ padding: 8 }}><Text>$100.00</Text></Cell>
            </Row>
          </Table>

          <Fixed position="footer">
            <Text style={{ fontSize: 10, textAlign: 'center', color: '#999999' }}>
              Page 1
            </Text>
          </Fixed>
        </Page>
      </Document>
    );

    // Verify top-level structure
    expect(doc.metadata.title).toBe('Invoice #001');
    expect(doc.metadata.author).toBe('Forme');
    expect(doc.children).toHaveLength(1); // one Page

    const page = doc.children[0];
    expect((page.kind as { type: string }).type).toBe('Page');

    // Page has: Fixed header, View, Table, Fixed footer
    expect(page.children).toHaveLength(4);
    expect((page.children[0].kind as { type: string }).type).toBe('Fixed');
    expect((page.children[1].kind as { type: string }).type).toBe('View');
    expect((page.children[2].kind as { type: string }).type).toBe('Table');
    expect((page.children[3].kind as { type: string }).type).toBe('Fixed');

    // Verify table structure
    const table = page.children[2];
    expect(table.children).toHaveLength(2); // header row + data row
    expect((table.children[0].kind as { type: string; is_header: boolean }).is_header).toBe(true);
  });

  it('render() produces JSON string', () => {
    const json = render(
      <Document>
        <Text>Hello Forme</Text>
      </Document>
    );

    const parsed = JSON.parse(json);
    expect(parsed.children).toHaveLength(1);
    expect(parsed.children[0].kind.type).toBe('Text');
    expect(parsed.children[0].kind.content).toBe('Hello Forme');
    expect(parsed.metadata).toBeDefined();
    expect(parsed.defaultPage).toBeDefined();
  });

  it('render() output is valid JSON', () => {
    const json = render(
      <Document title="Test">
        <View style={{ flexDirection: 'row', padding: 10 }}>
          <Text style={{ fontSize: 16 }}>Item 1</Text>
          <Text style={{ fontSize: 16 }}>Item 2</Text>
        </View>
      </Document>
    );

    expect(() => JSON.parse(json)).not.toThrow();
    const parsed = JSON.parse(json);
    expect(parsed.children[0].style.flexDirection).toBe('Row');
    expect(parsed.children[0].style.padding).toEqual({ top: 10, right: 10, bottom: 10, left: 10 });
  });
});

// ─── StyleSheet ─────────────────────────────────────────────────────

describe('StyleSheet', () => {
  it('StyleSheet.create returns the same object', () => {
    const styles = StyleSheet.create({
      heading: { fontSize: 24, fontWeight: 700 },
      body: { fontSize: 10 },
    });
    expect(styles.heading.fontSize).toBe(24);
    expect(styles.body.fontSize).toBe(10);
  });
});

// ─── Font registration ──────────────────────────────────────────────

describe('Font registration', () => {
  afterEach(() => {
    Font.clear();
  });

  it('Font.register() stores fonts and getRegistered() returns them', () => {
    Font.register({ family: 'Inter', src: 'inter.ttf' });
    const fonts = Font.getRegistered();
    expect(fonts).toHaveLength(1);
    expect(fonts[0].family).toBe('Inter');
    expect(fonts[0].fontWeight).toBe(400);
    expect(fonts[0].fontStyle).toBe('normal');
  });

  it('Font.register() normalizes weight strings', () => {
    Font.register({ family: 'Inter', src: 'inter-bold.ttf', fontWeight: 'bold' });
    expect(Font.getRegistered()[0].fontWeight).toBe(700);
  });

  it('Font.register() normalizes weight "normal"', () => {
    Font.register({ family: 'Inter', src: 'inter.ttf', fontWeight: 'normal' });
    expect(Font.getRegistered()[0].fontWeight).toBe(400);
  });

  it('Font.clear() removes all registrations', () => {
    Font.register({ family: 'Inter', src: 'inter.ttf' });
    Font.register({ family: 'Roboto', src: 'roboto.ttf' });
    expect(Font.getRegistered()).toHaveLength(2);
    Font.clear();
    expect(Font.getRegistered()).toHaveLength(0);
  });

  it('getRegistered() returns a copy', () => {
    Font.register({ family: 'Inter', src: 'inter.ttf' });
    const fonts = Font.getRegistered();
    fonts.push({ family: 'Fake', src: 'fake.ttf' });
    expect(Font.getRegistered()).toHaveLength(1);
  });
});

// ─── Font serialization ──────────────────────────────────────────────

describe('Font serialization', () => {
  afterEach(() => {
    Font.clear();
  });

  it('global fonts are included in serialized document', () => {
    Font.register({ family: 'Inter', src: 'data:font/ttf;base64,AAAA' });
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.fonts).toHaveLength(1);
    expect(doc.fonts![0]).toEqual({
      family: 'Inter',
      src: 'data:font/ttf;base64,AAAA',
      weight: 400,
      italic: false,
    });
  });

  it('document fonts prop is included', () => {
    const doc = serialize(
      <Document fonts={[{ family: 'Roboto', src: 'roboto.ttf', fontWeight: 700 }]}>
        <Text>Hello</Text>
      </Document>
    );
    expect(doc.fonts).toHaveLength(1);
    expect(doc.fonts![0]).toEqual({
      family: 'Roboto',
      src: 'roboto.ttf',
      weight: 700,
      italic: false,
    });
  });

  it('document fonts override global fonts on conflict', () => {
    Font.register({ family: 'Inter', src: 'global.ttf' });
    const doc = serialize(
      <Document fonts={[{ family: 'Inter', src: 'document.ttf' }]}>
        <Text>Hello</Text>
      </Document>
    );
    expect(doc.fonts).toHaveLength(1);
    expect(doc.fonts![0].src).toBe('document.ttf');
  });

  it('global and document fonts merge when no conflict', () => {
    Font.register({ family: 'Inter', src: 'inter.ttf' });
    const doc = serialize(
      <Document fonts={[{ family: 'Roboto', src: 'roboto.ttf' }]}>
        <Text>Hello</Text>
      </Document>
    );
    expect(doc.fonts).toHaveLength(2);
    const families = doc.fonts!.map(f => f.family);
    expect(families).toContain('Inter');
    expect(families).toContain('Roboto');
  });

  it('italic font style is serialized correctly', () => {
    Font.register({ family: 'Inter', src: 'inter-italic.ttf', fontStyle: 'italic' });
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.fonts![0].italic).toBe(true);
  });

  it('oblique font style is serialized as italic', () => {
    Font.register({ family: 'Inter', src: 'inter-oblique.ttf', fontStyle: 'oblique' });
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.fonts![0].italic).toBe(true);
  });

  it('no fonts omits fonts field from output', () => {
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.fonts).toBeUndefined();
  });

  it('Uint8Array src passes through in serialized output', () => {
    const bytes = new Uint8Array([0, 1, 2, 3]);
    Font.register({ family: 'Inter', src: bytes });
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.fonts![0].src).toBeInstanceOf(Uint8Array);
  });
});

// ─── CSS border shorthand ────────────────────────────────────────────

describe('CSS border shorthand', () => {
  it('border: "1px solid #000" sets width and color on all sides', () => {
    const style = mapStyle({ border: '1px solid #000' });
    expect(style.borderWidth).toEqual({ top: 1, right: 1, bottom: 1, left: 1 });
    expect(style.borderColor).toEqual({
      top: { r: 0, g: 0, b: 0, a: 1 },
      right: { r: 0, g: 0, b: 0, a: 1 },
      bottom: { r: 0, g: 0, b: 0, a: 1 },
      left: { r: 0, g: 0, b: 0, a: 1 },
    });
  });

  it('border: "2px #ff0000" sets width and color (no style keyword)', () => {
    const style = mapStyle({ border: '2px #ff0000' });
    expect(style.borderWidth).toEqual({ top: 2, right: 2, bottom: 2, left: 2 });
    expect(style.borderColor!.top).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('border width-only: "3px" sets width, no borderColor emitted', () => {
    const style = mapStyle({ border: '3px' });
    expect(style.borderWidth).toEqual({ top: 3, right: 3, bottom: 3, left: 3 });
    // No color token → borderColor not emitted (engine uses default black)
    expect(style.borderColor).toBeUndefined();
  });

  it('per-side borderTop overrides all-side border', () => {
    const style = mapStyle({ border: '1px solid #000', borderTop: '3px solid #ff0000' });
    expect(style.borderWidth!.top).toBe(3);
    expect(style.borderWidth!.right).toBe(1);
    expect(style.borderColor!.top).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('per-side borderBottom as number sets width only', () => {
    const style = mapStyle({ border: '1px solid #000', borderBottom: 5 });
    expect(style.borderWidth!.bottom).toBe(5);
    expect(style.borderWidth!.top).toBe(1);
  });

  it('borderWidth overrides border shorthand', () => {
    const style = mapStyle({ border: '1px solid #000', borderWidth: 4 });
    expect(style.borderWidth).toEqual({ top: 4, right: 4, bottom: 4, left: 4 });
  });

  it('borderTopWidth overrides border shorthand + borderWidth', () => {
    const style = mapStyle({ border: '1px solid #000', borderWidth: 2, borderTopWidth: 8 });
    expect(style.borderWidth!.top).toBe(8);
    expect(style.borderWidth!.right).toBe(2);
  });

  it('borderColor overrides border shorthand color', () => {
    const style = mapStyle({ border: '1px solid #ff0000', borderColor: '#00ff00' });
    expect(style.borderColor!.top).toEqual({ r: 0, g: 1, b: 0, a: 1 });
  });

  it('borderTopColor overrides everything', () => {
    const style = mapStyle({ border: '1px solid #000', borderColor: '#ff0000', borderTopColor: '#0000ff' });
    expect(style.borderColor!.top).toEqual({ r: 0, g: 0, b: 1, a: 1 });
    expect(style.borderColor!.right).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });
});

// ─── CSS edge string/array shorthands ────────────────────────────────

describe('CSS edge string/array shorthands', () => {
  it('padding: "8" → all sides 8', () => {
    expect(mapStyle({ padding: '8' }).padding).toEqual({ top: 8, right: 8, bottom: 8, left: 8 });
  });

  it('padding: "8 16" → vertical 8, horizontal 16', () => {
    expect(mapStyle({ padding: '8 16' }).padding).toEqual({ top: 8, right: 16, bottom: 8, left: 16 });
  });

  it('padding: "8 16 24" → top 8, horizontal 16, bottom 24', () => {
    expect(mapStyle({ padding: '8 16 24' }).padding).toEqual({ top: 8, right: 16, bottom: 24, left: 16 });
  });

  it('padding: "8 16 24 32" → top/right/bottom/left', () => {
    expect(mapStyle({ padding: '8 16 24 32' }).padding).toEqual({ top: 8, right: 16, bottom: 24, left: 32 });
  });

  it('padding with px suffix: "8px 16px"', () => {
    expect(mapStyle({ padding: '8px 16px' }).padding).toEqual({ top: 8, right: 16, bottom: 8, left: 16 });
  });

  it('margin array form: [20, 40, 20, 40]', () => {
    expect(mapStyle({ margin: [20, 40, 20, 40] }).margin).toEqual({ top: 20, right: 40, bottom: 20, left: 40 });
  });

  it('margin array form: [8] → all sides', () => {
    expect(mapStyle({ margin: [8] }).margin).toEqual({ top: 8, right: 8, bottom: 8, left: 8 });
  });

  it('margin array form: [8, 16] → vertical/horizontal', () => {
    expect(mapStyle({ margin: [8, 16] }).margin).toEqual({ top: 8, right: 16, bottom: 8, left: 16 });
  });

  it('padding string + paddingTop override', () => {
    expect(mapStyle({ padding: '8 16', paddingTop: 24 }).padding).toEqual({ top: 24, right: 16, bottom: 8, left: 16 });
  });

  it('Page margin="36 72"', () => {
    const doc = serialize(<Document><Page margin="36 72"><Text>hi</Text></Page></Document>);
    const config = (doc.children[0].kind as { config: { margin: unknown } }).config;
    expect(config.margin).toEqual({ top: 36, right: 72, bottom: 36, left: 72 });
  });

  it('Page margin={[36, 72]}', () => {
    const doc = serialize(<Document><Page margin={[36, 72]}><Text>hi</Text></Page></Document>);
    const config = (doc.children[0].kind as { config: { margin: unknown } }).config;
    expect(config.margin).toEqual({ top: 36, right: 72, bottom: 36, left: 72 });
  });

  it('expandEdges with string', () => {
    expect(expandEdges('10 20')).toEqual({ top: 10, right: 20, bottom: 10, left: 20 });
  });

  it('expandEdges with array', () => {
    expect(expandEdges([10, 20, 30, 40])).toEqual({ top: 10, right: 20, bottom: 30, left: 40 });
  });
});

// ─── Image alt and href ─────────────────────────────────────────────

describe('Image alt and href', () => {
  it('Image with href produces node href', () => {
    const doc = serialize(<Document><Image src="logo.png" href="https://example.com" /></Document>);
    expect(doc.children[0].href).toBe('https://example.com');
  });

  it('Image with alt produces node alt', () => {
    const doc = serialize(<Document><Image src="logo.png" alt="Company logo" /></Document>);
    expect(doc.children[0].alt).toBe('Company logo');
  });

  it('Image without href/alt omits them', () => {
    const doc = serialize(<Document><Image src="logo.png" /></Document>);
    expect(doc.children[0].href).toBeUndefined();
    expect(doc.children[0].alt).toBeUndefined();
  });
});

// ─── SVG alt and href ───────────────────────────────────────────────

describe('SVG alt and href', () => {
  it('Svg with href produces node href', () => {
    const doc = serialize(<Document><Svg width={100} height={100} content="<rect />" href="https://example.com" /></Document>);
    expect(doc.children[0].href).toBe('https://example.com');
  });

  it('Svg with alt produces node alt', () => {
    const doc = serialize(<Document><Svg width={100} height={100} content="<rect />" alt="Decorative icon" /></Document>);
    expect(doc.children[0].alt).toBe('Decorative icon');
  });
});

// ─── Document lang ──────────────────────────────────────────────────

describe('Document lang', () => {
  it('lang is included in metadata', () => {
    const doc = serialize(<Document lang="en-US"><Text>Hello</Text></Document>);
    expect(doc.metadata.lang).toBe('en-US');
  });

  it('lang is omitted when not set', () => {
    const doc = serialize(<Document><Text>Hello</Text></Document>);
    expect(doc.metadata.lang).toBeUndefined();
  });
});
