import React from 'react';
import { describe, it, expect, afterEach } from 'vitest';
import {
  Document,
  Page,
  View,
  Text,
  H1,
  H2,
  H3,
  H4,
  H5,
  H6,
  OrderedList,
  UnorderedList,
  ListItem,
  Strong,
  Em,
  Code,
  Link,
  Image,
  Table,
  Row,
  Cell,
  Fixed,
  PageBreak,
  Svg,
  QrCode,
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

  // ─── transform parsing ─────────────────────────────────────────

  it('transform parses rotate(deg)', () => {
    const style = mapStyle({ transform: 'rotate(45deg)' });
    expect(style.transform).toEqual([{ type: 'rotate', deg: 45 }]);
  });

  it('transform parses negative rotate', () => {
    const style = mapStyle({ transform: 'rotate(-15deg)' });
    expect(style.transform).toEqual([{ type: 'rotate', deg: -15 }]);
  });

  it('transform parses rad and turn angle units', () => {
    const rad = mapStyle({ transform: 'rotate(3.14159rad)' });
    expect((rad.transform as { deg: number }[])[0].deg).toBeCloseTo(180, 1);
    const turn = mapStyle({ transform: 'rotate(0.5turn)' });
    expect(turn.transform).toEqual([{ type: 'rotate', deg: 180 }]);
  });

  it('transform parses uniform scale', () => {
    const style = mapStyle({ transform: 'scale(1.2)' });
    expect(style.transform).toEqual([{ type: 'scale', x: 1.2, y: 1.2 }]);
  });

  it('transform parses non-uniform scale', () => {
    const style = mapStyle({ transform: 'scale(2, 0.5)' });
    expect(style.transform).toEqual([{ type: 'scale', x: 2, y: 0.5 }]);
  });

  it('transform parses translate', () => {
    const style = mapStyle({ transform: 'translate(10, -4)' });
    expect(style.transform).toEqual([{ type: 'translate', x: 10, y: -4 }]);
  });

  it('transform parses translate with px/pt suffixes', () => {
    const style = mapStyle({ transform: 'translate(10px, -4pt)' });
    expect(style.transform).toEqual([{ type: 'translate', x: 10, y: -4 }]);
  });

  it('transform composes multiple ops in declaration order', () => {
    const style = mapStyle({ transform: 'rotate(45deg) scale(1.2) translate(5, 10)' });
    expect(style.transform).toEqual([
      { type: 'rotate', deg: 45 },
      { type: 'scale', x: 1.2, y: 1.2 },
      { type: 'translate', x: 5, y: 10 },
    ]);
  });

  it('transform rejects unknown ops by dropping the whole transform', () => {
    const style = mapStyle({ transform: 'rotate(45deg) skew(10deg)' });
    expect(style.transform).toBeUndefined();
  });

  it('transformOrigin parses percentage string', () => {
    const style = mapStyle({ transformOrigin: '50% 50%' });
    expect(style.transformOrigin).toEqual([0.5, 0.5]);
  });

  it('transformOrigin parses CSS keywords', () => {
    expect(mapStyle({ transformOrigin: 'left top' }).transformOrigin).toEqual([0, 0]);
    expect(mapStyle({ transformOrigin: 'right bottom' }).transformOrigin).toEqual([1, 1]);
    expect(mapStyle({ transformOrigin: 'center' }).transformOrigin).toEqual([0.5, 0.5]);
  });

  it('fontFeatureSettings parses CSS feature settings', () => {
    expect(mapStyle({ fontFeatureSettings: '"tnum" 1' }).fontFeatures).toEqual([{ tag: 'tnum', value: 1 }]);
    expect(mapStyle({ fontFeatureSettings: "'zero', \"ss01\" on, \"liga\" off, \"salt\" 2" }).fontFeatures).toEqual([
      { tag: 'zero', value: 1 },
      { tag: 'ss01', value: 1 },
      { tag: 'liga', value: 0 },
      { tag: 'salt', value: 2 },
    ]);
    expect(mapStyle({ fontFeatureSettings: 'normal' }).fontFeatures).toEqual([]);
    expect(mapStyle({ fontFeatureSettings: 'nonsense' }).fontFeatures).toBeUndefined();
  });

  it('fontVariantNumeric maps to features, and fontFeatureSettings wins', () => {
    expect(mapStyle({ fontVariantNumeric: 'tabular-nums slashed-zero' }).fontFeatures).toEqual([
      { tag: 'tnum', value: 1 },
      { tag: 'zero', value: 1 },
    ]);
    expect(
      mapStyle({ fontVariantNumeric: 'tabular-nums', fontFeatureSettings: '"tnum" 0' }).fontFeatures,
    ).toEqual([{ tag: 'tnum', value: 0 }]);
  });

  it('transformOrigin accepts tuple form', () => {
    const style = mapStyle({ transformOrigin: [0.25, 0.75] });
    expect(style.transformOrigin).toEqual([0.25, 0.75]);
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

// ─── Wrapper component resolution ───────────────────────────────────

describe('Wrapper component resolution', () => {
  it('resolves a function component that returns <Document>', () => {
    function MyReport({ title }: { title: string }) {
      return (
        <Document title={title}>
          <Text>Hello</Text>
        </Document>
      );
    }
    const doc = serialize(<MyReport title="Test" />);
    expect(doc.metadata).toEqual({ title: 'Test' });
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'Hello' });
  });

  it('resolves nested wrapper components', () => {
    function Inner() {
      return (
        <Document>
          <Text>Nested</Text>
        </Document>
      );
    }
    function Outer() {
      return <Inner />;
    }
    const doc = serialize(<Outer />);
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'Nested' });
  });

  it('resolves user components inside the tree', () => {
    function MyHeader() {
      return <Text>Header</Text>;
    }
    const doc = serialize(
      <Document>
        <MyHeader />
        <Text>Body</Text>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'Text', content: 'Header' });
    expect(doc.children[1].kind).toEqual({ type: 'Text', content: 'Body' });
  });

  it('throws for non-Document top-level after resolution', () => {
    function MyView() {
      return <View><Text>Hi</Text></View>;
    }
    expect(() => serialize(<MyView />)).toThrow('Top-level element must be <Document>');
  });

  it('accepts Document with __formeType marker (version mismatch)', () => {
    // Simulate a different instance of Document from another package version
    function AnotherDocument(_props: any): null { return null; }
    (AnotherDocument as any).__formeType = 'Document';
    const element = React.createElement(AnotherDocument, null,
      React.createElement(Text, null, 'Hello'),
    );
    const doc = serialize(element);
    expect(doc.children).toHaveLength(1);
  });

  it('rejects component without Document marker', () => {
    function NotADocument(_props: any): null { return null; }
    Object.defineProperty(NotADocument, 'name', { value: 'MyComponent' });
    const element = React.createElement(NotADocument, null);
    expect(() => serialize(element)).toThrow('Top-level element must be <Document>');
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

  // ─── Inline formatting components ───────────────────────────────────

  it('Strong produces a TextRun with fontWeight 700', () => {
    const doc = serialize(
      <Document>
        <Text>Read the <Strong>fine print</Strong>.</Text>
      </Document>
    );
    expect(doc.children[0].kind).toEqual({
      type: 'Text',
      content: '',
      runs: [
        { content: 'Read the ' },
        { content: 'fine print', style: { fontWeight: 700 } },
        { content: '.' },
      ],
    });
  });

  it('Em produces a TextRun with italic fontStyle', () => {
    const doc = serialize(
      <Document>
        <Text>This is <Em>important</Em>.</Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; style?: { fontStyle?: string } }[] }).runs;
    expect(runs[1]).toEqual({ content: 'important', style: { fontStyle: 'Italic' } });
  });

  it('Code produces a TextRun with mono font + background', () => {
    const doc = serialize(
      <Document>
        <Text>Run <Code>npm install</Code> now.</Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; style?: Record<string, unknown> }[] }).runs;
    expect(runs[1].content).toBe('npm install');
    expect(runs[1].style?.fontFamily).toBe('Courier');
    expect(runs[1].style?.backgroundColor).toEqual({ r: 0xf4 / 255, g: 0xf4 / 255, b: 0xf5 / 255, a: 1 });
  });

  it('Link produces a TextRun with href + blue + underline', () => {
    const doc = serialize(
      <Document>
        <Text>See the <Link href="https://docs.formepdf.com">docs</Link>.</Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; href?: string; style?: Record<string, unknown> }[] }).runs;
    expect(runs[1].content).toBe('docs');
    expect(runs[1].href).toBe('https://docs.formepdf.com');
    expect(runs[1].style?.color).toEqual({ r: 0x25 / 255, g: 0x63 / 255, b: 0xeb / 255, a: 1 });
    expect(runs[1].style?.textDecoration).toBe('Underline');
  });

  it('nested Strong + Em composes (bold + italic on the same run)', () => {
    const doc = serialize(
      <Document>
        <Text><Strong><Em>both</Em></Strong></Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; style?: Record<string, unknown> }[] }).runs;
    expect(runs).toEqual([
      { content: 'both', style: { fontWeight: 700, fontStyle: 'Italic' } },
    ]);
  });

  it('user style overrides component defaults', () => {
    const doc = serialize(
      <Document>
        <Text><Strong style={{ fontWeight: 400 }}>not really bold</Strong></Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; style?: Record<string, unknown> }[] }).runs;
    expect(runs[0].style?.fontWeight).toBe(400);
  });

  it('Link href inside Strong preserves href and composes bold styling', () => {
    const doc = serialize(
      <Document>
        <Text><Strong><Link href="/x">bold link</Link></Strong></Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string; href?: string; style?: Record<string, unknown> }[] }).runs;
    expect(runs[0].content).toBe('bold link');
    expect(runs[0].href).toBe('/x');
    expect(runs[0].style?.fontWeight).toBe(700);
    expect(runs[0].style?.textDecoration).toBe('Underline');
  });

  it('mixed strings + multiple inline components produce runs in order', () => {
    const doc = serialize(
      <Document>
        <Text>a <Strong>b</Strong> c <Em>d</Em> e</Text>
      </Document>
    );
    const runs = (doc.children[0].kind as { runs: { content: string }[] }).runs;
    expect(runs.map(r => r.content)).toEqual(['a ', 'b', ' c ', 'd', ' e']);
  });

  // ─── Headings (H1-H6) ───────────────────────────────────────────────

  it('H1 emits a Heading node with level 1 + default styling', () => {
    const doc = serialize(
      <Document>
        <H1>Annual Report</H1>
      </Document>
    );
    const node = doc.children[0];
    expect(node.kind).toEqual({ type: 'Heading', level: 1, content: 'Annual Report' });
    expect(node.style.fontSize).toBe(32);
    expect(node.style.fontWeight).toBe(700);
    expect(node.style.margin).toBeDefined(); // marginTop/marginBottom were converted
  });

  it('each level emits the matching level number', () => {
    const doc = serialize(
      <Document>
        <H1>1</H1>
        <H2>2</H2>
        <H3>3</H3>
        <H4>4</H4>
        <H5>5</H5>
        <H6>6</H6>
      </Document>
    );
    const levels = doc.children.map((c) => (c.kind as { level?: number }).level);
    expect(levels).toEqual([1, 2, 3, 4, 5, 6]);
  });

  it('default font sizes scale down by level', () => {
    const doc = serialize(
      <Document>
        <H1>a</H1>
        <H2>a</H2>
        <H3>a</H3>
        <H4>a</H4>
        <H5>a</H5>
        <H6>a</H6>
      </Document>
    );
    const sizes = doc.children.map((c) => c.style.fontSize);
    expect(sizes).toEqual([32, 24, 20, 18, 16, 14]);
    // Strictly descending
    for (let i = 1; i < sizes.length; i++) {
      expect(sizes[i]).toBeLessThan(sizes[i - 1]!);
    }
  });

  it('user style overrides heading defaults', () => {
    const doc = serialize(
      <Document>
        <H1 style={{ fontSize: 48, fontWeight: 400, color: '#ff0000' }}>Big</H1>
      </Document>
    );
    expect(doc.children[0].style.fontSize).toBe(48);
    expect(doc.children[0].style.fontWeight).toBe(400);
    expect(doc.children[0].style.color).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('inline formatting composes inside headings', () => {
    const doc = serialize(
      <Document>
        <H1>Chapter <Em>1</Em></H1>
      </Document>
    );
    const kind = doc.children[0].kind as {
      type: 'Heading';
      runs?: { content: string; style?: { fontStyle?: string } }[];
    };
    expect(kind.type).toBe('Heading');
    expect(kind.runs).toEqual([
      { content: 'Chapter ' },
      { content: '1', style: { fontStyle: 'Italic' } },
    ]);
  });

  it('heading with single text child uses content (no runs)', () => {
    const doc = serialize(
      <Document>
        <H2>Simple heading</H2>
      </Document>
    );
    const kind = doc.children[0].kind as { content: string; runs?: unknown };
    expect(kind.content).toBe('Simple heading');
    expect(kind.runs).toBeUndefined();
  });

  it('heading carries href + bookmark when set', () => {
    const doc = serialize(
      <Document>
        <H1 href="#top" bookmark="Top">Title</H1>
      </Document>
    );
    expect((doc.children[0].kind as { href?: string }).href).toBe('#top');
    expect(doc.children[0].bookmark).toBe('Top');
  });

  // ─── Lists (OrderedList / UnorderedList / ListItem) ────────────────

  it('UnorderedList defaults to ordered:false marker:disc start:1', () => {
    const doc = serialize(
      <Document>
        <UnorderedList>
          <ListItem>a</ListItem>
          <ListItem>b</ListItem>
        </UnorderedList>
      </Document>
    );
    const kind = doc.children[0].kind as {
      type: string;
      ordered: boolean;
      marker_type: string;
      start: number;
    };
    expect(kind.type).toBe('List');
    expect(kind.ordered).toBe(false);
    expect(kind.marker_type).toBe('disc');
    expect(kind.start).toBe(1);
    expect(doc.children[0].children).toHaveLength(2);
  });

  it('OrderedList defaults to ordered:true marker:decimal start:1', () => {
    const doc = serialize(
      <Document>
        <OrderedList>
          <ListItem>a</ListItem>
        </OrderedList>
      </Document>
    );
    const kind = doc.children[0].kind as {
      ordered: boolean;
      marker_type: string;
      start: number;
    };
    expect(kind.ordered).toBe(true);
    expect(kind.marker_type).toBe('decimal');
    expect(kind.start).toBe(1);
  });

  it('OrderedList type prop maps to engine marker type (kebab → camel)', () => {
    const cases: Array<[string, string]> = [
      ['lower-alpha', 'lowerAlpha'],
      ['upper-alpha', 'upperAlpha'],
      ['lower-roman', 'lowerRoman'],
      ['upper-roman', 'upperRoman'],
      ['decimal', 'decimal'],
    ];
    for (const [reactType, engineType] of cases) {
      const doc = serialize(
        <Document>
          <OrderedList type={reactType as any}>
            <ListItem>a</ListItem>
          </OrderedList>
        </Document>
      );
      const kind = doc.children[0].kind as { marker_type: string };
      expect(kind.marker_type).toBe(engineType);
    }
  });

  it('UnorderedList marker prop maps through correctly', () => {
    for (const marker of ['disc', 'circle', 'square', 'none'] as const) {
      const doc = serialize(
        <Document>
          <UnorderedList marker={marker}>
            <ListItem>a</ListItem>
          </UnorderedList>
        </Document>
      );
      const kind = doc.children[0].kind as { marker_type: string };
      expect(kind.marker_type).toBe(marker);
    }
  });

  it('OrderedList start prop is preserved', () => {
    const doc = serialize(
      <Document>
        <OrderedList start={5}>
          <ListItem>five</ListItem>
        </OrderedList>
      </Document>
    );
    expect((doc.children[0].kind as { start: number }).start).toBe(5);
  });

  it('ListItem children are serialized — string children auto-wrapped as Text', () => {
    const doc = serialize(
      <Document>
        <UnorderedList>
          <ListItem>plain string</ListItem>
        </UnorderedList>
      </Document>
    );
    const items = doc.children[0].children;
    expect(items).toHaveLength(1);
    expect(items[0].kind).toEqual({ type: 'ListItem' });
    expect(items[0].children).toHaveLength(1);
    expect(items[0].children[0].kind).toEqual({ type: 'Text', content: 'plain string' });
  });

  it('ListItem accepts mixed JSX content (Text, nested list)', () => {
    const doc = serialize(
      <Document>
        <OrderedList>
          <ListItem>
            <Text>outer</Text>
            <UnorderedList>
              <ListItem>nested</ListItem>
            </UnorderedList>
          </ListItem>
        </OrderedList>
      </Document>
    );
    const item = doc.children[0].children[0];
    expect(item.kind).toEqual({ type: 'ListItem' });
    const kindTypes = item.children.map((c) => c.kind.type);
    expect(kindTypes).toEqual(['Text', 'List']);
  });

  it('non-ListItem children of a list are dropped silently', () => {
    const doc = serialize(
      <Document>
        <UnorderedList>
          <ListItem>real</ListItem>
          <Text>stray</Text>
          {null}
          {false}
        </UnorderedList>
      </Document>
    );
    expect(doc.children[0].children).toHaveLength(1);
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

describe('CSS Grid serialization', () => {
  it('maps display: grid', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '1fr 2fr 200' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.display).toBe('Grid');
  });

  it('parses gridTemplateColumns string shorthand', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '1fr 2fr 200' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateColumns).toEqual([{ Fr: 1 }, { Fr: 2 }, { Pt: 200 }]);
  });

  it('parses gridTemplateColumns array', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: [100, '1fr', 'auto'] }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateColumns).toEqual([{ Pt: 100 }, { Fr: 1 }, 'Auto']);
  });

  it('maps gridTemplateRows', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '1fr', gridTemplateRows: 'auto 100' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateRows).toEqual(['Auto', { Pt: 100 }]);
  });

  it('maps gridAutoRows and gridAutoColumns', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '1fr', gridAutoRows: 50, gridAutoColumns: '2fr' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridAutoRows).toEqual({ Pt: 50 });
    expect(view.style.gridAutoColumns).toEqual({ Fr: 2 });
  });

  it('maps grid placement properties', () => {
    const doc = serialize(
      <Document>
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr 1fr' }}>
          <View style={{ gridColumnStart: 2, gridColumnEnd: 4, gridRowStart: 1 }}><Text>A</Text></View>
        </View>
      </Document>
    );
    const child = doc.children[0].children[0];
    expect(child.style.gridPlacement).toEqual({
      columnStart: 2,
      columnEnd: 4,
      rowStart: 1,
    });
  });

  it('maps gridColumnSpan and gridRowSpan', () => {
    const doc = serialize(
      <Document>
        <View style={{ display: 'grid', gridTemplateColumns: '1fr 1fr' }}>
          <View style={{ gridColumnSpan: 2, gridRowSpan: 3 }}><Text>A</Text></View>
        </View>
      </Document>
    );
    const child = doc.children[0].children[0];
    expect(child.style.gridPlacement).toEqual({
      columnSpan: 2,
      rowSpan: 3,
    });
  });

  it('display defaults to flex (omitted)', () => {
    const doc = serialize(
      <Document><View><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.display).toBeUndefined();
  });

  it('expands repeat(N, track) in gridTemplateColumns', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateColumns).toEqual([{ Fr: 1 }, { Fr: 1 }, { Fr: 1 }]);
  });

  it('expands repeat with multiple tracks', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: 'repeat(2, 100 1fr)' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateColumns).toEqual([{ Pt: 100 }, { Fr: 1 }, { Pt: 100 }, { Fr: 1 }]);
  });

  it('expands repeat mixed with other tracks', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '200 repeat(2, 1fr) 200' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateColumns).toEqual([{ Pt: 200 }, { Fr: 1 }, { Fr: 1 }, { Pt: 200 }]);
  });

  it('handles repeat in gridTemplateRows', () => {
    const doc = serialize(
      <Document><View style={{ display: 'grid', gridTemplateColumns: '1fr', gridTemplateRows: 'repeat(3, 50)' }}><Text>A</Text></View></Document>
    );
    const view = doc.children[0];
    expect(view.style.gridTemplateRows).toEqual([{ Pt: 50 }, { Pt: 50 }, { Pt: 50 }]);
  });
});

describe('textOverflow serialization', () => {
  it('maps textOverflow ellipsis', () => {
    const doc = serialize(
      <Document><Text style={{ textOverflow: 'ellipsis' }}>Long text</Text></Document>
    );
    expect(doc.children[0].style.textOverflow).toBe('Ellipsis');
  });

  it('maps textOverflow clip', () => {
    const doc = serialize(
      <Document><Text style={{ textOverflow: 'clip' }}>Long text</Text></Document>
    );
    expect(doc.children[0].style.textOverflow).toBe('Clip');
  });

  it('maps textOverflow wrap', () => {
    const doc = serialize(
      <Document><Text style={{ textOverflow: 'wrap' }}>Long text</Text></Document>
    );
    expect(doc.children[0].style.textOverflow).toBe('Wrap');
  });

  it('omits textOverflow when not set', () => {
    const doc = serialize(
      <Document><Text>hello</Text></Document>
    );
    expect(doc.children[0].style.textOverflow).toBeUndefined();
  });
});

describe('QrCode serialization', () => {
  it('produces correct kind with data', () => {
    const doc = serialize(
      <Document><QrCode data="https://formepdf.com" /></Document>
    );
    expect(doc.children[0].kind).toEqual({ type: 'QrCode', data: 'https://formepdf.com' });
  });

  it('includes size when provided', () => {
    const doc = serialize(
      <Document><QrCode data="test" size={100} /></Document>
    );
    const kind = doc.children[0].kind as { type: string; data: string; size?: number };
    expect(kind.type).toBe('QrCode');
    expect(kind.data).toBe('test');
    expect(kind.size).toBe(100);
  });

  it('applies color prop to style', () => {
    const doc = serialize(
      <Document><QrCode data="test" color="#ff0000" /></Document>
    );
    expect(doc.children[0].style.color).toEqual({ r: 1, g: 0, b: 0, a: 1 });
  });

  it('applies style prop', () => {
    const doc = serialize(
      <Document><QrCode data="test" style={{ margin: 10 }} /></Document>
    );
    expect(doc.children[0].style.margin).toEqual({ top: 10, right: 10, bottom: 10, left: 10 });
  });
});

describe('background gradient parsing', () => {
  it('parses linear-gradient with explicit angle and 2 stops', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'linear-gradient(90deg, #ff0000 0%, #0000ff 100%)' }} />
      </Document>
    );
    const bg = (doc.children[0].style as { background?: { type: string; angleDeg: number; stops: { position: number; color: { r: number; g: number; b: number; a: number } }[] } }).background;
    expect(bg).toBeDefined();
    expect(bg!.type).toBe('linear');
    expect(bg!.angleDeg).toBe(90);
    expect(bg!.stops).toHaveLength(2);
    expect(bg!.stops[0]).toEqual({ position: 0, color: { r: 1, g: 0, b: 0, a: 1 } });
    expect(bg!.stops[1]).toEqual({ position: 1, color: { r: 0, g: 0, b: 1, a: 1 } });
  });

  it('defaults to 180deg when angle omitted', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'linear-gradient(#fff, #000)' }} />
      </Document>
    );
    const bg = (doc.children[0].style as { background?: { angleDeg: number } }).background;
    expect(bg!.angleDeg).toBe(180);
  });

  it('translates "to right" side keyword to 90deg', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'linear-gradient(to right, #fff, #000)' }} />
      </Document>
    );
    const bg = (doc.children[0].style as { background?: { angleDeg: number } }).background;
    expect(bg!.angleDeg).toBe(90);
  });

  it('parses radial-gradient with circle keyword', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'radial-gradient(circle, #10b981 0%, #059669 100%)' }} />
      </Document>
    );
    const bg = (doc.children[0].style as { background?: { type: string; stops: { position: number }[] } }).background;
    expect(bg).toBeDefined();
    expect(bg!.type).toBe('radial');
    expect(bg!.stops).toHaveLength(2);
    expect(bg!.stops[0].position).toBe(0);
    expect(bg!.stops[1].position).toBe(1);
  });

  it('preserves 3+ stops (multi-stop gradients via Type 3 stitching)', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'linear-gradient(180deg, #ff0000 0%, #00ff00 50%, #0000ff 100%)' }} />
      </Document>
    );
    const bg = (doc.children[0].style as { background?: { stops: { position: number; color: { r: number; g: number; b: number; a: number } }[] } }).background;
    expect(bg!.stops).toHaveLength(3);
    expect(bg!.stops[0]).toEqual({ position: 0, color: { r: 1, g: 0, b: 0, a: 1 } });
    expect(bg!.stops[1]).toEqual({ position: 0.5, color: { r: 0, g: 1, b: 0, a: 1 } });
    expect(bg!.stops[2]).toEqual({ position: 1, color: { r: 0, g: 0, b: 1, a: 1 } });
  });

  it('routes solid color to backgroundColor', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: '#1e293b' }} />
      </Document>
    );
    const style = doc.children[0].style as { backgroundColor?: { r: number; g: number; b: number; a: number }; background?: unknown };
    expect(style.background).toBeUndefined();
    expect(style.backgroundColor).toEqual({ r: 30 / 255, g: 41 / 255, b: 59 / 255, a: 1 });
  });

  it('rgba() solid color in background routes to backgroundColor', () => {
    const doc = serialize(
      <Document>
        <View style={{ background: 'rgba(255, 0, 0, 0.5)' }} />
      </Document>
    );
    const style = doc.children[0].style as { backgroundColor?: { r: number; g: number; b: number; a: number } };
    expect(style.backgroundColor).toEqual({ r: 1, g: 0, b: 0, a: 0.5 });
  });
});

// ─── .map() children flattening ─────────────────────────────────────

describe('.map() children in serialize()', () => {
  it('View with .map() children preserves all items', () => {
    const items = ['Alpha', 'Beta', 'Gamma'];
    const doc = serialize(
      <Document>
        <View>
          {items.map((item, i) => (
            <Text key={i}>{item}</Text>
          ))}
        </View>
      </Document>
    );
    expect(doc.children[0].children).toHaveLength(3);
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: 'Alpha' });
    expect(doc.children[0].children[1].kind).toEqual({ type: 'Text', content: 'Beta' });
    expect(doc.children[0].children[2].kind).toEqual({ type: 'Text', content: 'Gamma' });
  });

  it('Table with header row and .map() data rows preserves all rows', () => {
    const data = [
      { name: 'Widget', price: 10 },
      { name: 'Gadget', price: 20 },
    ];
    const doc = serialize(
      <Document>
        <Table columns={[{ width: { fraction: 1 } }, { width: { fraction: 1 } }]}>
          <Row header>
            <Cell><Text>Name</Text></Cell>
            <Cell><Text>Price</Text></Cell>
          </Row>
          {data.map((item, i) => (
            <Row key={i}>
              <Cell><Text>{item.name}</Text></Cell>
              <Cell><Text>{`$${item.price}`}</Text></Cell>
            </Row>
          ))}
        </Table>
      </Document>
    );
    // 1 header row + 2 data rows = 3 total
    expect(doc.children[0].children).toHaveLength(3);
  });

  it('mixed static and .map() children are all preserved', () => {
    const items = ['A', 'B'];
    const doc = serialize(
      <Document>
        <View>
          <Text>Header</Text>
          {items.map((item, i) => (
            <Text key={i}>{item}</Text>
          ))}
          <Text>Footer</Text>
        </View>
      </Document>
    );
    // Header + 2 mapped + Footer = 4
    expect(doc.children[0].children).toHaveLength(4);
    expect(doc.children[0].children[0].kind).toEqual({ type: 'Text', content: 'Header' });
    expect(doc.children[0].children[1].kind).toEqual({ type: 'Text', content: 'A' });
    expect(doc.children[0].children[2].kind).toEqual({ type: 'Text', content: 'B' });
    expect(doc.children[0].children[3].kind).toEqual({ type: 'Text', content: 'Footer' });
  });
});
