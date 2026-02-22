import React from 'react';
import { describe, it, expect } from 'vitest';
import {
  Document,
  Page,
  View,
  Text,
  Image,
  Table,
  Row,
  Cell,
  serializeTemplate,
  createDataProxy,
  expr,
} from '../src/index';

// ─── Proxy → $ref ───────────────────────────────────────────────────

describe('Data proxy to $ref', () => {
  it('simple property access produces $ref in text content', () => {
    const data = createDataProxy() as { title: string };
    const result = serializeTemplate(
      <Document><Text>{data.title}</Text></Document>
    );
    const textNode = result.children[0] as Record<string, unknown>;
    const kind = textNode.kind as Record<string, unknown>;
    expect(kind.content).toEqual({ $ref: 'title' });
  });

  it('nested property access produces dotted $ref', () => {
    const data = createDataProxy() as { user: { name: string } };
    const result = serializeTemplate(
      <Document><Text>{data.user.name}</Text></Document>
    );
    const textNode = result.children[0] as Record<string, unknown>;
    const kind = textNode.kind as Record<string, unknown>;
    expect(kind.content).toEqual({ $ref: 'user.name' });
  });

  it('string interpolation produces $concat', () => {
    const data = createDataProxy() as { name: string };
    const result = serializeTemplate(
      <Document><Text>Hello {data.name}!</Text></Document>
    );
    const textNode = result.children[0] as Record<string, unknown>;
    const kind = textNode.kind as Record<string, unknown>;
    expect(kind.content).toEqual({ $concat: ['Hello ', { $ref: 'name' }, '!'] });
  });
});

// ─── .map() → $each ─────────────────────────────────────────────────

describe('.map() to $each', () => {
  it('array .map() produces $each node', () => {
    const data = createDataProxy() as { items: Array<{ name: string }> };
    const result = serializeTemplate(
      <Document>
        <View>
          {data.items.map((item: { name: string }) => (
            <Text>{item.name}</Text>
          ))}
        </View>
      </Document>
    );
    const view = result.children[0] as Record<string, unknown>;
    const children = view.children as unknown[];
    expect(children).toHaveLength(1);
    const each = children[0] as Record<string, unknown>;
    expect(each.$each).toEqual({ $ref: 'items' });
    expect(each.as).toBe('$item');
    const template = each.template as Record<string, unknown>;
    const kind = template.kind as Record<string, unknown>;
    expect(kind.content).toEqual({ $ref: '$item.name' });
  });
});

// ─── expr helpers ───────────────────────────────────────────────────

describe('Expression helpers', () => {
  it('expr.eq produces $eq node', () => {
    const data = createDataProxy() as { status: string };
    const result = serializeTemplate(
      <Document>
        <View>
          {expr.if(
            expr.eq(data.status, 'active'),
            <Text>Active</Text>,
            <Text>Inactive</Text>
          ) as React.ReactNode}
        </View>
      </Document>
    );
    const view = result.children[0] as Record<string, unknown>;
    const children = view.children as unknown[];
    // The expr.if result is an expr marker, which gets serialized as the expression object
    const ifNode = children[0] as Record<string, unknown>;
    expect(ifNode.$if).toEqual({ $eq: [{ $ref: 'status' }, 'active'] });
  });

  it('expr.format produces $format node', () => {
    const data = createDataProxy() as { price: number };
    const result = serializeTemplate(
      <Document><Text>{expr.format(data.price, '0.00') as unknown as string}</Text></Document>
    );
    const textNode = result.children[0] as Record<string, unknown>;
    const kind = textNode.kind as Record<string, unknown>;
    // The expr marker in text content gets detected
    expect(kind.content).toEqual({ $format: [{ $ref: 'price' }, '0.00'] });
  });

  it('expr.add produces $add node', () => {
    const data = createDataProxy() as { a: number; b: number };
    const marker = expr.add(data.a, data.b);
    // Check the marker itself
    expect((marker as { expr: unknown }).expr).toEqual({
      $add: [{ $ref: 'a' }, { $ref: 'b' }],
    });
  });

  it('expr.concat produces $concat node', () => {
    const data = createDataProxy() as { first: string; last: string };
    const marker = expr.concat(data.first, ' ', data.last);
    expect((marker as { expr: unknown }).expr).toEqual({
      $concat: [{ $ref: 'first' }, ' ', { $ref: 'last' }],
    });
  });

  it('expr.count produces $count node', () => {
    const data = createDataProxy() as { items: unknown[] };
    const marker = expr.count(data.items);
    expect((marker as { expr: unknown }).expr).toEqual({
      $count: { $ref: 'items' },
    });
  });

  it('expr.cond produces $cond node', () => {
    const data = createDataProxy() as { premium: boolean };
    const marker = expr.cond(data.premium, 'gold', 'standard');
    expect((marker as { expr: unknown }).expr).toEqual({
      $cond: [{ $ref: 'premium' }, 'gold', 'standard'],
    });
  });
});

// ─── Non-dynamic passthrough ─────────────────────────────────────────

describe('Non-dynamic content passthrough', () => {
  it('static text passes through unchanged', () => {
    const result = serializeTemplate(
      <Document><Text>Hello World</Text></Document>
    );
    const textNode = result.children[0] as Record<string, unknown>;
    const kind = textNode.kind as Record<string, unknown>;
    expect(kind.content).toBe('Hello World');
  });

  it('static View with style passes through', () => {
    const result = serializeTemplate(
      <Document>
        <View style={{ backgroundColor: '#ff0000', padding: 10 }}>
          <Text>Static</Text>
        </View>
      </Document>
    );
    const view = result.children[0] as Record<string, unknown>;
    expect(view.kind).toEqual({ type: 'View' });
    const style = view.style as Record<string, unknown>;
    expect(style.backgroundColor).toEqual({ r: 1, g: 0, b: 0, a: 1 });
    expect(style.padding).toEqual({ top: 10, right: 10, bottom: 10, left: 10 });
  });

  it('page structure preserved', () => {
    const result = serializeTemplate(
      <Document>
        <Page size="Letter">
          <Text>Page content</Text>
        </Page>
      </Document>
    );
    const page = result.children[0] as Record<string, unknown>;
    const kind = page.kind as Record<string, unknown>;
    expect(kind.type).toBe('Page');
    expect((kind.config as Record<string, unknown>).size).toBe('Letter');
  });
});

// ─── Document structure ──────────────────────────────────────────────

describe('Template document structure', () => {
  it('produces correct top-level structure', () => {
    const result = serializeTemplate(
      <Document title="Test">
        <Text>Hello</Text>
      </Document>
    );
    expect(result.metadata).toEqual({ title: 'Test' });
    expect(result.defaultPage).toBeDefined();
    expect(result.children).toHaveLength(1);
  });

  it('metadata with proxy values produces $ref', () => {
    const data = createDataProxy() as { docTitle: string };
    const result = serializeTemplate(
      <Document title={`${data.docTitle}`}>
        <Text>Hello</Text>
      </Document>
    );
    const metadata = result.metadata as Record<string, unknown>;
    expect(metadata.title).toEqual({ $ref: 'docTitle' });
  });
});
