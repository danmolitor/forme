import * as Preact from 'preact';
const React = { createElement: Preact.createElement };
import { describe, it, expect } from 'vitest';
import * as Forme from '../src/index';

// The two serializers, and why this file exists
// (Ported from the react adapter. This package is a hand-maintained fork of
// that serializer, so it is exactly where a dispatch chain drifts unnoticed.)
// ---------------------------------------------
// `serialize()` and `serializeTemplate()` are two dispatch chains over the
// same component set, written out by hand, one under the other. They drifted:
// H1-H6, OrderedList, UnorderedList and ListItem had arms in the first and
// none in the second, so on the template path each of those elements fell
// through to the "unknown function component, call it" fallback, returned
// `null`, and was deleted from the document ALONG WITH ITS ENTIRE SUBTREE.
// Silently. `serializeTemplate` is public API and drives `forme build
// --template`.
//
// The guard has to be DERIVED, not listed. A hand-written list of components
// to check is the same artifact that drifted in the first place — it guards
// only the names somebody remembered, which is exactly how a `Pick<>` of five
// claim props missed a sixth. So this walks every component the package
// actually exports and asserts the two paths agree about each one. Add a
// component to the package and it is covered here the moment it is exported;
// there is nothing to remember to update.

/** Every exported value that is usable as a JSX component. */
function exportedComponents(): [string, Preact.ComponentType<any>][] {
  const skip = new Set([
    // Not components: the document root (tested separately, it is the entry
    // point rather than a child), and the authoring helpers.
    'Document',
    'Font',
    'serialize',
    'serializeTemplate',
    'createDataProxy',
    'expr',
    'renderDocument',
    'renderDocumentWithLayout',
  ]);
  return Object.entries(Forme as Record<string, unknown>)
    .filter(([name, v]) => {
      if (skip.has(name)) return false;
      if (typeof v !== 'function') return false;
      // Components are capitalised; helpers and hooks are not.
      return /^[A-Z]/.test(name);
    })
    .map(([name, v]) => [name, v as Preact.ComponentType<any>]);
}

/**
 * A minimal, valid element for a component — enough props that the
 * serializer has something to chew on, and correct nesting where the
 * serializer validates it.
 */
function sample(name: string, C: Preact.ComponentType<any>): Preact.VNode {
  const text = React.createElement(Forme.Text, null, 'content');
  switch (name) {
    case 'Row':
      return React.createElement(Forme.Table, { columns: [{ width: 'auto' }] }, React.createElement(C, null, React.createElement(Forme.Cell, null, text)));
    case 'Cell':
      return React.createElement(Forme.Table, { columns: [{ width: 'auto' }] }, React.createElement(Forme.Row, null, React.createElement(C, null, text)));
    case 'ListItem':
      return React.createElement(Forme.UnorderedList, null, React.createElement(C, null, 'item'));
    case 'Image':
      return React.createElement(C, { src: 'data:image/png;base64,iVBORw0KGgo=', width: 10, height: 10 });
    case 'Svg':
      return React.createElement(C, { width: 10, height: 10, content: '<rect width="10" height="10"/>' });
    case 'QrCode':
      return React.createElement(C, { data: 'hello', size: 40 });
    case 'Barcode':
      return React.createElement(C, { data: '12345', format: 'Code39', width: 80, height: 30 });
    case 'TextField':
    case 'Checkbox':
    case 'Dropdown':
    case 'RadioButton':
      return React.createElement(C, { name: 'f', width: 80, options: ['a', 'b'], value: 'a' });
    case 'Canvas':
      return React.createElement(C, { width: 20, height: 20, draw: () => {} });
    case 'Watermark':
      return React.createElement(C, { text: 'DRAFT' });
    case 'BarChart':
    case 'LegacyBarChart':
      return React.createElement(C, { width: 100, height: 60, data: [{ label: 'a', value: 1 }] });
    case 'LineChart':
    case 'AreaChart':
    case 'LegacyLineChart':
      return React.createElement(C, {
        width: 100, height: 60, labels: ['a', 'b'],
        series: [{ name: 's', color: '#123456', values: [1, 2] }],
        data: [{ label: 'a', value: 1 }],
      });
    case 'PieChart':
      return React.createElement(C, { width: 100, height: 60, data: [{ label: 'a', value: 1 }] });
    case 'LegacyPieChart':
      return React.createElement(C, {
        width: 100, height: 60,
        data: [{ label: 'a', value: 1, color: '#123456' }],
      });
    case 'DotPlot':
      return React.createElement(C, {
        width: 100, height: 60,
        groups: [{ name: 'g', color: '#123456', points: [{ x: 1, y: 2 }] }],
      });
    case 'OrderedList':
    case 'UnorderedList':
      return React.createElement(C, null, React.createElement(Forme.ListItem, null, 'content'));
    case 'PageBreak':
      return React.createElement(C, null);
    case 'Page':
      return React.createElement(C, null, text);
    case 'Fixed':
      return React.createElement(C, { position: 'footer' }, text);
    default:
      // View, Text, headings, lists, inline formatting, links.
      return React.createElement(C, null, 'content');
  }
}

/**
 * How many nodes the serializer produced for this document.
 *
 * Counting `kind`-bearing nodes rather than grepping for content strings:
 * a deleted element leaves nothing behind, and this measures that directly
 * without depending on what any one component renders its text as.
 */
function nodeCount(value: unknown): number {
  if (Array.isArray(value)) return value.reduce<number>((n, v) => n + nodeCount(v), 0);
  if (value === null || typeof value !== 'object') return 0;
  const obj = value as Record<string, unknown>;
  const self = 'kind' in obj ? 1 : 0;
  return self + Object.values(obj).reduce<number>((n, v) => n + nodeCount(v), 0);
}

/**
 * Did the component contribute anything to the document?
 *
 * Measured as the node count WITH the component minus the node count of the
 * same document without it. A plain `count > 0` is vacuous: the enclosing
 * Page is itself a node, so every document scores at least one and a deleted
 * component looks identical to a present one. Subtracting the empty baseline
 * is what makes this measure the component rather than the wrapper.
 */
function contribution(
  body: Preact.VNode,
  isPage: boolean,
  serializer: (e: Preact.VNode) => unknown,
): number {
  const withBody = React.createElement(
    Forme.Document,
    null,
    isPage ? body : React.createElement(Forme.Page, null, body),
  );
  const withoutBody = React.createElement(
    Forme.Document,
    null,
    React.createElement(Forme.Page, null),
  );
  return nodeCount(serializer(withBody)) - nodeCount(serializer(withoutBody));
}

describe('serialize() and serializeTemplate() agree about every exported component', () => {
  const components = exportedComponents();

  it('finds a non-trivial set of components to check', () => {
    // If this ever collapses to a handful, the filter above has gone wrong
    // and every case below would pass vacuously.
    expect(components.length).toBeGreaterThan(20);
  });

  for (const [name, C] of components) {
    it(`${name} survives both paths`, () => {
      const body = sample(name, C);
      const isPage = name === 'Page';

      const viaSerialize =
        contribution(body, isPage, Forme.serialize as (e: Preact.VNode) => unknown) > 0;
      const viaTemplate =
        contribution(body, isPage, Forme.serializeTemplate as (e: Preact.VNode) => unknown) > 0;

      // The assertion is agreement, not "both true". A component neither
      // path supports is a different conversation; a component one path
      // silently deletes is this bug.
      expect(
        { component: name, serialize: viaSerialize, serializeTemplate: viaTemplate },
      ).toEqual(
        { component: name, serialize: viaSerialize, serializeTemplate: viaSerialize },
      );
    });
  }
});
