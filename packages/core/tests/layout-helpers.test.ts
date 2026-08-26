/**
 * Behavior tests for the layout helper module (`@formepdf/core/layout`).
 *
 * These tests verify the helpers produce the right answers on top of a
 * conforming runtime shape. They do NOT re-verify runtime invariants
 * themselves — that's `layout-shape.test.ts`'s job. Together the two
 * files form the drift alarm: if the runtime shape drifts, the
 * conformance test screams; if the helpers drift from their contract,
 * these tests scream.
 */

import { describe, it, expect } from 'vitest';
import { renderDocumentWithLayout } from '../src/index.js';
import {
  walkElements,
  findElements,
  findFirstElement,
  getNodeText,
  getTextLines,
  getHeadingLevel,
  getTableRows,
  getFixedRegions,
  getListItems,
  getListItemMarker,
  isNodeType,
} from '../src/layout.js';
import {
  Document, Page, View, Text,
  H1, H3,
  OrderedList, UnorderedList, ListItem,
  Table, Row, Cell,
  Fixed,
} from '@formepdf/react';
import { createElement as h } from 'react';

// Fixture — smaller than layout-shape's, focused on what the helpers do.
const FIXTURE = h(
  Document,
  { title: 'helper-tests' },
  h(
    Page,
    { size: 'Letter', margin: 36 },
    h(Fixed, { position: 'header' }, h(Text, null, 'my header')),
    h(H1, null, 'Report Title'),
    h(H3, null, 'Section'),
    h(Text, null, 'first body paragraph'),
    h(View, { style: { flexDirection: 'row' } },
      h(Text, null, 'nested one'),
      h(Text, null, 'nested two'),
    ),
    h(OrderedList, null,
      h(ListItem, null, 'alpha'),
      h(ListItem, null, 'beta'),
      h(ListItem, null, 'gamma'),
    ),
    h(UnorderedList, null,
      h(ListItem, null, 'x'),
      h(ListItem, null, 'y'),
    ),
    h(Table, { columns: [{ width: { fraction: 1 } }, { width: { fraction: 1 } }] },
      h(Row, { header: true },
        h(Cell, null, h(Text, null, 'H1')),
        h(Cell, null, h(Text, null, 'H2')),
      ),
      h(Row, null,
        h(Cell, null, h(Text, null, 'a1')),
        h(Cell, null, h(Text, null, 'a2')),
      ),
      h(Row, null,
        h(Cell, null, h(Text, null, 'b1')),
        h(Cell, null, h(Text, null, 'b2')),
      ),
    ),
    h(Fixed, { position: 'footer' }, h(Text, null, 'my footer')),
  ),
);

// Reuse the render across tests — helpers are pure, no interference.
const renderPromise = renderDocumentWithLayout(FIXTURE);

describe('layout helpers', () => {
  // ── Traversal ────────────────────────────────────────────────

  describe('walkElements', () => {
    it('accepts LayoutInfo', async () => {
      const { layout } = await renderPromise;
      let count = 0;
      walkElements(layout, () => { count++; });
      expect(count).toBeGreaterThan(0);
    });

    it('accepts PageInfo', async () => {
      const { layout } = await renderPromise;
      let count = 0;
      walkElements(layout.pages[0], () => { count++; });
      expect(count).toBeGreaterThan(0);
    });

    it('accepts ElementInfo', async () => {
      const { layout } = await renderPromise;
      const first = layout.pages[0].elements[0];
      let count = 0;
      walkElements(first, () => { count++; });
      expect(count).toBeGreaterThanOrEqual(1); // at least the root itself
    });

    it('accepts ElementInfo[]', async () => {
      const { layout } = await renderPromise;
      let count = 0;
      walkElements(layout.pages[0].elements, () => { count++; });
      expect(count).toBeGreaterThan(0);
    });

    it('passes a human-readable path to the callback', async () => {
      const { layout } = await renderPromise;
      const paths: string[] = [];
      walkElements(layout, (_, path) => { paths.push(path); });
      // Path always starts with `[N]` for the top-level index
      expect(paths[0]).toMatch(/^\[0\]/);
      // Some paths have `.children[N]` for nested nodes
      expect(paths.some(p => p.includes('.children['))).toBe(true);
    });

    it('respects `return false` to skip descent', async () => {
      const { layout } = await renderPromise;
      let visited = 0;
      let visitedIfSkipped = 0;
      walkElements(layout, (n) => { visited++; if (n.nodeType === 'TableRow') return false; });
      walkElements(layout, () => { visitedIfSkipped++; });
      expect(visited).toBeLessThan(visitedIfSkipped);
    });

    it('visits nodes in source order', async () => {
      const { layout } = await renderPromise;
      const topLevelInOrder: string[] = [];
      walkElements(layout.pages[0].elements, (n, p) => {
        if (!p.includes('.children[')) topLevelInOrder.push(n.nodeType);
      });
      // FixedHeader / Watermark come first; heading H1 should be present early
      const h1Index = topLevelInOrder.indexOf('H1');
      const h3Index = topLevelInOrder.indexOf('H3');
      expect(h1Index).toBeGreaterThanOrEqual(0);
      expect(h3Index).toBeGreaterThan(h1Index);
    });
  });

  describe('findElements', () => {
    it('returns every node matching a predicate, in order', async () => {
      const { layout } = await renderPromise;
      const listItems = findElements(layout, (n) => n.nodeType === 'ListItem');
      // 3 OL items + 2 UL items = 5
      expect(listItems).toHaveLength(5);
    });

    it('composes with isNodeType() type-guard', async () => {
      const { layout } = await renderPromise;
      const rows = findElements(layout, isNodeType('TableRow'));
      expect(rows).toHaveLength(3);
      // TypeScript-level check: no cast needed to know rows[i].nodeType is 'TableRow'
      const first: 'TableRow' = rows[0].nodeType;
      expect(first).toBe('TableRow');
    });
  });

  describe('findFirstElement', () => {
    it('returns the first match, source order', async () => {
      const { layout } = await renderPromise;
      const h1 = findFirstElement(layout, (n) => n.nodeType === 'H1');
      expect(h1).not.toBeNull();
      expect(h1!.nodeType).toBe('H1');
    });

    it('returns null when no match', async () => {
      const { layout } = await renderPromise;
      // 'PageBreak' never appears as a nodeType at runtime
      const none = findFirstElement(layout, (n) => (n.nodeType as string) === 'PageBreak');
      expect(none).toBeNull();
    });
  });

  // ── Text access (the load-bearing case) ──────────────────────

  describe('getTextLines', () => {
    it('returns all TextLine descendants of a Text block', async () => {
      const { layout } = await renderPromise;
      const firstTextBlock = findFirstElement(layout, (n) => n.nodeType === 'Text');
      expect(firstTextBlock).not.toBeNull();
      const lines = getTextLines(firstTextBlock!);
      expect(lines.length).toBeGreaterThanOrEqual(1);
      for (const line of lines) {
        expect(line.nodeType).toBe('TextLine');
      }
    });

    it('returns [node] when node is itself a TextLine', async () => {
      const { layout } = await renderPromise;
      const line = findFirstElement(layout, (n) => n.nodeType === 'TextLine');
      expect(line).not.toBeNull();
      const result = getTextLines(line!);
      expect(result).toEqual([line]);
    });

    it('returns [] for a subtree with no TextLine descendants', async () => {
      // A synthetic node with no children
      const empty: any = {
        x: 0, y: 0, width: 0, height: 0,
        nodeType: 'View', kind: 'None',
        style: {}, children: [], textContent: null,
      };
      expect(getTextLines(empty)).toEqual([]);
    });
  });

  describe('getNodeText', () => {
    it('extracts the text of a Text block from its TextLine children', async () => {
      const { layout } = await renderPromise;
      const first = findFirstElement(layout, (n) =>
        n.nodeType === 'Text' &&
        // Skip the fixed header/footer's Text nodes — find the first content Text
        getNodeText(n).includes('first body'),
      );
      expect(first).not.toBeNull();
      expect(getNodeText(first!)).toBe('first body paragraph');
    });

    it('concatenates multi-line output with newlines, preserving line structure', async () => {
      const { layout } = await renderPromise;
      // H1 heading text ("Report Title") should come out on one or more lines
      const h1 = findFirstElement(layout, (n) => n.nodeType === 'H1');
      expect(h1).not.toBeNull();
      const text = getNodeText(h1!);
      // Non-empty, contains the source string, joined structure preserved
      expect(text.replace(/\n/g, ' ')).toContain('Report Title');
    });

    it('returns "" for a node with no TextLine descendants', () => {
      const empty: any = {
        x: 0, y: 0, width: 0, height: 0,
        nodeType: 'View', kind: 'None',
        style: {}, children: [], textContent: null,
      };
      expect(getNodeText(empty)).toBe('');
    });

    it('does not include the parent Text block textContent (which is null)', async () => {
      const { layout } = await renderPromise;
      const textBlock = findFirstElement(layout, (n) => n.nodeType === 'Text');
      // Even if a consumer set textContent on a Text block (shouldn't happen),
      // getNodeText only reads from TextLine children.
      const text = getNodeText(textBlock!);
      expect(text.length).toBeGreaterThan(0); // came from TextLine, not from the block
    });
  });

  // ── Structural queries ──────────────────────────────────────

  describe('getHeadingLevel', () => {
    it('returns 1 for H1', async () => {
      const { layout } = await renderPromise;
      const h1 = findFirstElement(layout, (n) => n.nodeType === 'H1');
      expect(getHeadingLevel(h1!)).toBe(1);
    });

    it('returns 3 for H3', async () => {
      const { layout } = await renderPromise;
      const h3 = findFirstElement(layout, (n) => n.nodeType === 'H3');
      expect(getHeadingLevel(h3!)).toBe(3);
    });

    it('returns null for non-heading nodes', async () => {
      const { layout } = await renderPromise;
      const text = findFirstElement(layout, (n) => n.nodeType === 'Text');
      expect(getHeadingLevel(text!)).toBeNull();
    });
  });

  describe('getTableRows', () => {
    it('returns direct TableRow children of a Page', async () => {
      const { layout } = await renderPromise;
      const rows = getTableRows(layout.pages[0]);
      // 1 header row + 2 body rows in the fixture
      expect(rows).toHaveLength(3);
      for (const r of rows) expect(r.nodeType).toBe('TableRow');
    });

    it('returns direct TableRow children of a View', async () => {
      const { layout } = await renderPromise;
      // A view with no direct TableRow children returns []
      const view = findFirstElement(layout, (n) => n.nodeType === 'View');
      expect(getTableRows(view!)).toEqual([]);
    });

    it('returns [] when there are no rows', async () => {
      const empty: any = {
        x: 0, y: 0, width: 0, height: 0,
        nodeType: 'View', kind: 'None',
        style: {}, children: [], textContent: null,
      };
      expect(getTableRows(empty)).toEqual([]);
    });
  });

  describe('getFixedRegions', () => {
    it('returns FixedHeader + FixedFooter direct children of a page', async () => {
      const { layout } = await renderPromise;
      const { header, footer } = getFixedRegions(layout.pages[0]);
      expect(header.length).toBeGreaterThanOrEqual(1);
      expect(footer.length).toBeGreaterThanOrEqual(1);
      for (const h of header) expect(h.nodeType).toBe('FixedHeader');
      for (const f of footer) expect(f.nodeType).toBe('FixedFooter');
    });

    it('never returns a generic Fixed node', async () => {
      const { layout } = await renderPromise;
      const { header, footer } = getFixedRegions(layout.pages[0]);
      for (const n of [...header, ...footer]) {
        expect(n.nodeType).not.toBe('Fixed' as any);
      }
    });
  });

  describe('getListItems', () => {
    it('returns ListItem children of a List', async () => {
      const { layout } = await renderPromise;
      const lists = findElements(layout, (n) => n.nodeType === 'List');
      expect(lists.length).toBeGreaterThanOrEqual(2); // OL + UL
      for (const list of lists) {
        const items = getListItems(list);
        expect(items.length).toBeGreaterThanOrEqual(1);
        for (const item of items) expect(item.nodeType).toBe('ListItem');
      }
    });

    it('returns [] when given a non-List node', async () => {
      const { layout } = await renderPromise;
      const text = findFirstElement(layout, (n) => n.nodeType === 'Text');
      expect(getListItems(text!)).toEqual([]);
    });
  });

  describe('getListItemMarker', () => {
    it('returns the marker text of an ordered list item ("1." / "2." / …)', async () => {
      const { layout } = await renderPromise;
      const lists = findElements(layout, (n) => n.nodeType === 'List');
      // First list in the fixture is the OL
      const ol = lists[0];
      const items = getListItems(ol);
      const marker = getListItemMarker(items[0]);
      expect(marker).not.toBeNull();
      expect(marker).toMatch(/^\d+\./);
    });

    it('returns the marker text of an unordered list item ("•" or similar)', async () => {
      const { layout } = await renderPromise;
      const lists = findElements(layout, (n) => n.nodeType === 'List');
      // Second list in the fixture is the UL
      const ul = lists[1];
      const items = getListItems(ul);
      const marker = getListItemMarker(items[0]);
      expect(marker).not.toBeNull();
      expect(marker!.length).toBeGreaterThan(0);
    });

    it('returns null for a non-ListItem node', async () => {
      const { layout } = await renderPromise;
      const text = findFirstElement(layout, (n) => n.nodeType === 'Text');
      expect(getListItemMarker(text!)).toBeNull();
    });
  });

  // ── Type-guard ──────────────────────────────────────────────

  describe('isNodeType', () => {
    it('returns true for matching nodeType', async () => {
      const { layout } = await renderPromise;
      const h1 = findFirstElement(layout, (n) => n.nodeType === 'H1');
      expect(isNodeType('H1')(h1!)).toBe(true);
    });

    it('returns false for non-matching nodeType', async () => {
      const { layout } = await renderPromise;
      const h1 = findFirstElement(layout, (n) => n.nodeType === 'H1');
      expect(isNodeType('H2')(h1!)).toBe(false);
    });

    it('narrows the type at compile time (filter chain)', async () => {
      const { layout } = await renderPromise;
      const allNodes = findElements(layout, () => true);
      const tableRows = allNodes.filter(isNodeType('TableRow'));
      // TypeScript now knows tableRows[i].nodeType is exactly 'TableRow'
      if (tableRows.length > 0) {
        const nt: 'TableRow' = tableRows[0].nodeType;
        expect(nt).toBe('TableRow');
      }
    });
  });
});
