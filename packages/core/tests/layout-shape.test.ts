/**
 * Runtime-conformance test for the layout-metadata types exported from
 * `@formepdf/core`.
 *
 * The types (`LayoutInfo`, `PageInfo`, `ElementInfo`, `ElementStyleInfo`,
 * and the enum unions) claim a specific shape. The engine emits some other
 * shape. These have drifted twice now — the third time will be caught
 * here, in FormePDF's own CI, before it ships downstream.
 *
 * Structure:
 *
 * 1. `describe('shape claims — enforced invariants')` covers the eight
 *    documented transforms from the `ElementInfo` JSDoc. Each assertion
 *    fails with a specific message naming the transform and where it
 *    broke (e.g. "no `Table` wrapper — `<Table>` must unwrap to sibling
 *    `TableRow` nodes at the page level. Found nodeType=Table at
 *    pages[0].elements[3]").
 *
 * 2. `describe('shape claims — types match runtime')` walks every node
 *    and asserts:
 *      - `nodeType` is one of `ElementNodeType`
 *      - `kind` is one of `ElementKind`
 *      - every style enum-string is a valid value of its declared union
 *      - required fields are present with the right type
 *
 * Failures always identify the specific node (path from root) and the
 * specific claim that broke — not just "assertion failed".
 */

import { describe, it, expect } from 'vitest';
import {
  renderDocumentWithLayout,
  type ElementInfo,
  type ElementNodeType,
  type ElementKind,
  type ElementFlexDirection,
  type ElementJustifyContent,
  type ElementAlignItems,
  type ElementAlignContent,
  type ElementFlexWrap,
  type ElementFontStyle,
  type ElementTextAlign,
  type ElementTextDecoration,
  type ElementTextTransform,
  type ElementOverflow,
  type ElementPosition,
} from '../src/index.js';
import {
  Document, Page, View, Text,
  H1, H2, H3, H4, H5, H6,
  Strong, Em, Code, Link,
  OrderedList, UnorderedList, ListItem,
  Table, Row, Cell,
  Image, Svg, QrCode, Barcode, Canvas, Watermark,
  BarChart, LineChart, PieChart, AreaChart, DotPlot,
  TextField, Checkbox, Dropdown, RadioButton,
  Fixed, PageBreak,
} from '@formepdf/react';
import { createElement as h } from 'react';

// ── Fixture ────────────────────────────────────────────────────────

/**
 * Exercises every JSX component we ship. If a future component doesn't
 * appear here, the "no unknown nodeTypes" test won't catch its drift.
 * When you add a new component to `@formepdf/react`, add it here too.
 */
const RICH_FIXTURE = h(
  Document,
  { title: 'shape-audit' },
  h(
    Page,
    { size: 'Letter', margin: 36 },
    h(Fixed, { position: 'header' }, h(Text, null, 'header')),
    h(H1, null, 'H1'), h(H2, null, 'H2'), h(H3, null, 'H3'),
    h(H4, null, 'H4'), h(H5, null, 'H5'), h(H6, null, 'H6'),
    h(Text, null, 'plain text'),
    h(Text, null, 'mixed ',
      h(Strong, null, 'bold'), ' ',
      h(Em, null, 'italic'), ' ',
      h(Code, null, 'monospace'), ' ',
      h(Link, { href: 'https://formepdf.com' }, 'link')),
    h(View, { style: { flexDirection: 'row', gap: 8 } }, h(Text, null, 'in view')),
    // `bookmark` on a container emits a zero-height 'Bookmark' marker node.
    // BOTH paths are exercised deliberately: a container that fits the page
    // (layout_view) and one that overflows it (layout_breakable_view). They
    // are separate code paths that emit the marker independently, and for two
    // days only the overflow path did — a fixture covering just one of them
    // reports full coverage while half the behaviour is missing.
    h(View, { bookmark: 'Fitting Section' }, h(Text, null, 'in fitting bookmarked view')),
    h(View, { bookmark: 'Bookmarked Section', style: { height: 900, backgroundColor: '#eee' } },
      h(Text, null, 'in bookmarked view')),
    h(OrderedList, null, h(ListItem, null, 'one'), h(ListItem, null, 'two')),
    h(UnorderedList, null, h(ListItem, null, 'a'), h(ListItem, null, 'b')),
    h(Table, { columns: [{ width: { fraction: 1 } }, { width: { fraction: 1 } }] },
      h(Row, { header: true },
        h(Cell, null, h(Text, null, 'A')),
        h(Cell, null, h(Text, null, 'B'))),
      h(Row, null,
        h(Cell, null, h(Text, null, '1')),
        h(Cell, null, h(Text, null, '2'))),
    ),
    h(Image, {
      src: 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNkYAAAAAYAAjCB0C8AAAAASUVORK5CYII=',
      width: 20, height: 20,
    }),
    h(Svg, { width: 20, height: 20, content: '<rect x="0" y="0" width="20" height="20" fill="black"/>' }),
    h(QrCode, { data: 'test', size: 40 }),
    h(Barcode, { data: '123', format: 'Code128', width: 60, height: 20 }),
    h(Canvas, {
      width: 40, height: 40,
      draw: (ctx: any) => { ctx.rect(0, 0, 20, 20); ctx.fill(); },
    }),
    h(Watermark, { text: 'DRAFT', fontSize: 30 }),
    h(BarChart, { width: 200, height: 60, data: [{ label: 'a', value: 1 }] }),
    h(LineChart, { width: 200, height: 60, labels: ['x'], series: [{ name: 's', data: [1] }] }),
    h(PieChart, { width: 100, height: 100, data: [{ label: 'a', value: 1 }, { label: 'b', value: 1 }] }),
    h(AreaChart, { width: 200, height: 60, labels: ['x'], series: [{ name: 's', data: [1] }] }),
    h(DotPlot, { width: 200, height: 60, groups: [{ name: 'g', data: [[1, 1]] }] }),
    h(TextField, { name: 'name', width: 100 }),
    h(Checkbox, { name: 'agree', checked: true }),
    h(Dropdown, { name: 'sel', options: ['a', 'b'], value: 'a', width: 80 }),
    h(RadioButton, { name: 'r', value: 'y', checked: true }),
    h(PageBreak),
    h(Text, null, 'page 2'),
    h(Fixed, { position: 'footer' }, h(Text, null, 'footer')),
  ),
);

// ── Enum value sets (the runtime-authoritative source of truth) ────
//
// These MUST match the exported literal union types exactly, in BOTH
// directions:
//
// 1. Every key in the record → assignable to the union. Enforced by
//    `satisfies Record<Union, unknown>` — TypeScript errors if a key
//    isn't a valid union member.
//
// 2. Every union member → a key in the record. ALSO enforced by
//    `satisfies Record<Union, unknown>` — TypeScript errors if the
//    union grows without a matching key.
//
// The runtime tests below then verify: (runtime emits value → value
// is in the set). That's the third direction. All three together mean
// no drift can slip through: adding to the union without updating
// this file is a compile error; the runtime emitting a value not in
// the union is a test failure; the fixture not exercising a declared
// value is the coverage tripwire.
//
// If a compile error points at the `as const satisfies Record<...>`
// clause below with a "Property 'X' is missing" message, that means
// the corresponding union in `packages/core/src/index.ts` grew and
// this file wasn't updated. Add the missing key with value `1`.

const NODE_TYPE_KEYS = {
  View: 1, Text: 1, TextLine: 1,
  H1: 1, H2: 1, H3: 1, H4: 1, H5: 1, H6: 1,
  TableRow: 1, TableCell: 1,
  List: 1, ListItem: 1, Lbl: 1,
  FixedHeader: 1, FixedFooter: 1,
  Bookmark: 1,
  Image: 1, Svg: 1, QrCode: 1, Barcode: 1, Canvas: 1, Watermark: 1,
  BarChart: 1, LineChart: 1, PieChart: 1, AreaChart: 1, DotPlot: 1,
  TextField: 1, Checkbox: 1, Dropdown: 1, RadioButton: 1,
} as const satisfies Record<ElementNodeType, unknown>;
const NODE_TYPES: Set<ElementNodeType> = new Set(
  Object.keys(NODE_TYPE_KEYS) as ElementNodeType[],
);

const KIND_KEYS = {
  None: 1, Rect: 1, Text: 1, Image: 1, Svg: 1,
  QrCode: 1, Barcode: 1, Chart: 1, FormField: 1, Watermark: 1,
} as const satisfies Record<ElementKind, unknown>;
const KINDS: Set<ElementKind> = new Set(
  Object.keys(KIND_KEYS) as ElementKind[],
);

const FLEX_DIRECTION_KEYS = {
  Row: 1, Column: 1, RowReverse: 1, ColumnReverse: 1,
} as const satisfies Record<ElementFlexDirection, unknown>;
const FLEX_DIRECTIONS: Set<ElementFlexDirection> = new Set(
  Object.keys(FLEX_DIRECTION_KEYS) as ElementFlexDirection[],
);

const JUSTIFY_CONTENT_KEYS = {
  FlexStart: 1, FlexEnd: 1, Center: 1,
  SpaceBetween: 1, SpaceAround: 1, SpaceEvenly: 1,
} as const satisfies Record<ElementJustifyContent, unknown>;
const JUSTIFY_CONTENTS: Set<ElementJustifyContent> = new Set(
  Object.keys(JUSTIFY_CONTENT_KEYS) as ElementJustifyContent[],
);

const ALIGN_ITEMS_KEYS = {
  FlexStart: 1, FlexEnd: 1, Center: 1, Stretch: 1, Baseline: 1,
} as const satisfies Record<ElementAlignItems, unknown>;
const ALIGN_ITEMS: Set<ElementAlignItems> = new Set(
  Object.keys(ALIGN_ITEMS_KEYS) as ElementAlignItems[],
);

const ALIGN_CONTENT_KEYS = {
  FlexStart: 1, FlexEnd: 1, Center: 1,
  SpaceBetween: 1, SpaceAround: 1, SpaceEvenly: 1, Stretch: 1,
} as const satisfies Record<ElementAlignContent, unknown>;
const ALIGN_CONTENTS: Set<ElementAlignContent> = new Set(
  Object.keys(ALIGN_CONTENT_KEYS) as ElementAlignContent[],
);

const FLEX_WRAP_KEYS = {
  NoWrap: 1, Wrap: 1, WrapReverse: 1,
} as const satisfies Record<ElementFlexWrap, unknown>;
const FLEX_WRAPS: Set<ElementFlexWrap> = new Set(
  Object.keys(FLEX_WRAP_KEYS) as ElementFlexWrap[],
);

const FONT_STYLE_KEYS = {
  Normal: 1, Italic: 1, Oblique: 1,
} as const satisfies Record<ElementFontStyle, unknown>;
const FONT_STYLES: Set<ElementFontStyle> = new Set(
  Object.keys(FONT_STYLE_KEYS) as ElementFontStyle[],
);

const TEXT_ALIGN_KEYS = {
  Left: 1, Right: 1, Center: 1, Justify: 1,
} as const satisfies Record<ElementTextAlign, unknown>;
const TEXT_ALIGNS: Set<ElementTextAlign> = new Set(
  Object.keys(TEXT_ALIGN_KEYS) as ElementTextAlign[],
);

const TEXT_DECORATION_KEYS = {
  None: 1, Underline: 1, LineThrough: 1,
} as const satisfies Record<ElementTextDecoration, unknown>;
const TEXT_DECORATIONS: Set<ElementTextDecoration> = new Set(
  Object.keys(TEXT_DECORATION_KEYS) as ElementTextDecoration[],
);

const TEXT_TRANSFORM_KEYS = {
  None: 1, Uppercase: 1, Lowercase: 1, Capitalize: 1,
} as const satisfies Record<ElementTextTransform, unknown>;
const TEXT_TRANSFORMS: Set<ElementTextTransform> = new Set(
  Object.keys(TEXT_TRANSFORM_KEYS) as ElementTextTransform[],
);

const OVERFLOW_KEYS = {
  Visible: 1, Hidden: 1,
} as const satisfies Record<ElementOverflow, unknown>;
const OVERFLOWS: Set<ElementOverflow> = new Set(
  Object.keys(OVERFLOW_KEYS) as ElementOverflow[],
);

const POSITION_KEYS = {
  Relative: 1, Absolute: 1,
} as const satisfies Record<ElementPosition, unknown>;
const POSITIONS: Set<ElementPosition> = new Set(
  Object.keys(POSITION_KEYS) as ElementPosition[],
);

// ── Utilities ──────────────────────────────────────────────────────

/** Walk every node in the layout tree, yielding a path string alongside. */
function* walk(el: ElementInfo, path: string): Generator<{ el: ElementInfo; path: string }> {
  yield { el, path };
  for (let i = 0; i < el.children.length; i++) {
    yield* walk(el.children[i], `${path}.children[${i}]`);
  }
}

function findByNodeType(root: ElementInfo | { elements: ElementInfo[] }, nt: ElementNodeType): { el: ElementInfo; path: string }[] {
  const results: { el: ElementInfo; path: string }[] = [];
  const elements = 'elements' in root ? root.elements : [root];
  const prefix = 'elements' in root ? 'pages[?].elements' : 'root';
  for (let i = 0; i < elements.length; i++) {
    for (const hit of walk(elements[i], `${prefix}[${i}]`)) {
      if (hit.el.nodeType === nt) results.push(hit);
    }
  }
  return results;
}

// ── The test ───────────────────────────────────────────────────────

describe('layout shape conformance', () => {
  // Render once, share across all assertions. If layout throws, the whole
  // suite fails immediately with the render error — no cascading noise.
  const renderPromise = renderDocumentWithLayout(RICH_FIXTURE);

  describe('enforced transforms (from ElementInfo JSDoc)', () => {
    it('<Table> unwraps into sibling TableRow nodes — no `Table` wrapper node exists', async () => {
      const { layout } = await renderPromise;
      const tables: string[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if ((hit.el.nodeType as string) === 'Table') tables.push(hit.path);
          }
        }
      }
      expect(tables, `Expected no \`Table\` wrapper nodes (\`<Table>\` must unwrap to sibling \`TableRow\` nodes at the containing page/View level). Found nodeType=Table at: ${tables.join(', ')}`).toEqual([]);

      // Also assert positive: TableRow nodes appear at the containing page level
      const rows: string[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          if (page.elements[i].nodeType === 'TableRow') rows.push(`pages[?].elements[${i}]`);
        }
      }
      expect(rows.length, `Expected TableRow nodes as direct children of a Page (the unwrap target). Found none.`).toBeGreaterThan(0);
    });

    it('<Fixed> splits by position — emits FixedHeader / FixedFooter, never generic `Fixed`', async () => {
      const { layout } = await renderPromise;
      const generic: string[] = [];
      let sawHeader = false;
      let sawFooter = false;
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if ((hit.el.nodeType as string) === 'Fixed') generic.push(hit.path);
            if (hit.el.nodeType === 'FixedHeader') sawHeader = true;
            if (hit.el.nodeType === 'FixedFooter') sawFooter = true;
          }
        }
      }
      expect(generic, `Expected no generic \`Fixed\` nodeType (\`<Fixed position="header">\` must produce \`FixedHeader\`, \`<Fixed position="footer">\` must produce \`FixedFooter\`). Found generic Fixed at: ${generic.join(', ')}`).toEqual([]);
      expect(sawHeader, 'Expected at least one FixedHeader node from `<Fixed position="header">`').toBe(true);
      expect(sawFooter, 'Expected at least one FixedFooter node from `<Fixed position="footer">`').toBe(true);
    });

    it('Headings emit discrete H1–H6, never a generic `Heading` with a level field', async () => {
      const { layout } = await renderPromise;
      const badHeading: { path: string; hasLevel: boolean }[] = [];
      const seenTags = new Set<string>();
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            const nt = hit.el.nodeType as string;
            if (nt === 'Heading') {
              badHeading.push({ path: hit.path, hasLevel: 'level' in (hit.el as any) });
            }
            if (/^H[1-6]$/.test(nt)) seenTags.add(nt);
          }
        }
      }
      expect(badHeading, `Expected no generic \`Heading\` nodeType (headings must be discrete \`H1\`–\`H6\`). Found: ${JSON.stringify(badHeading)}`).toEqual([]);
      expect(seenTags, `Expected all six discrete heading tags H1–H6. Missing: ${[...['H1','H2','H3','H4','H5','H6']].filter(t => !seenTags.has(t)).join(', ')}`).toEqual(new Set(['H1', 'H2', 'H3', 'H4', 'H5', 'H6']));
    });

    it('`textContent` lives on TextLine leaves — non-TextLine nodes emit null', async () => {
      const { layout } = await renderPromise;
      const violators: { path: string; nodeType: string; textContent: unknown }[] = [];
      let sawPopulatedTextLine = false;
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            const el = hit.el;
            if (el.nodeType === 'TextLine') {
              if (typeof el.textContent === 'string' && el.textContent.length > 0) {
                sawPopulatedTextLine = true;
              }
            } else if (el.textContent != null && el.textContent !== '') {
              violators.push({ path: hit.path, nodeType: el.nodeType, textContent: el.textContent });
            }
          }
        }
      }
      expect(violators, `Expected \`textContent\` to be null on every non-TextLine node — but it was populated on: ${JSON.stringify(violators.slice(0, 3))}`).toEqual([]);
      expect(sawPopulatedTextLine, 'Expected at least one TextLine node with populated `textContent` (the load-bearing invariant). None found — either the fixture has no rendered text or the invariant broke.').toBe(true);
    });

    it('<OrderedList> / <UnorderedList> both produce List with ListItem children, each having a Lbl marker', async () => {
      const { layout } = await renderPromise;
      const lists = findByNodeType({ elements: layout.pages.flatMap(p => p.elements) }, 'List');
      expect(lists.length, 'Expected at least one `List` node from `<OrderedList>` / `<UnorderedList>`. Found none.').toBeGreaterThan(0);
      for (const { el, path } of lists) {
        const itemPaths = el.children.map((c, i) => ({ nodeType: c.nodeType, path: `${path}.children[${i}]` }));
        const nonItems = itemPaths.filter(x => x.nodeType !== 'ListItem');
        expect(nonItems, `Expected every child of a \`List\` node to be a \`ListItem\`. Found: ${JSON.stringify(nonItems)}`).toEqual([]);

        // Each ListItem must have a Lbl child (marker "1." / "•")
        for (let ci = 0; ci < el.children.length; ci++) {
          const item = el.children[ci];
          const lbls = item.children.filter(c => c.nodeType === 'Lbl');
          expect(lbls.length, `Expected every \`ListItem\` to contain a \`Lbl\` marker child. \`${path}.children[${ci}]\` had children: ${item.children.map(c => c.nodeType).join(', ')}`).toBeGreaterThanOrEqual(1);
        }
      }
    });

    it('inline formatting (<Strong>/<Em>/<Code>/<Link>) does not emit its own nodeTypes — contributes to TextLine runs', async () => {
      const { layout } = await renderPromise;
      const forbidden = ['Strong', 'Em', 'Code', 'Link'];
      const found: { path: string; nodeType: string }[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if (forbidden.includes(hit.el.nodeType as string)) {
              found.push({ path: hit.path, nodeType: hit.el.nodeType });
            }
          }
        }
      }
      expect(found, `Inline elements should not produce their own layout nodeTypes (they contribute to TextLine style runs). Found: ${JSON.stringify(found)}`).toEqual([]);
    });

    it('<PageBreak> produces no node — it triggers page splits but is otherwise invisible', async () => {
      const { layout } = await renderPromise;
      const found: string[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if ((hit.el.nodeType as string) === 'PageBreak') found.push(hit.path);
          }
        }
      }
      expect(found, `\`<PageBreak>\` must not produce a layout node. Found PageBreak nodeType at: ${found.join(', ')}`).toEqual([]);
      expect(layout.pages.length, 'Expected the fixture to render >= 2 pages (PageBreak used).').toBeGreaterThanOrEqual(2);
    });
  });

  describe('types match runtime (enums + shapes)', () => {
    it('every emitted nodeType is a member of the declared ElementNodeType union', async () => {
      const { layout } = await renderPromise;
      const unknowns: { path: string; nodeType: string }[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if (!NODE_TYPES.has(hit.el.nodeType as ElementNodeType)) {
              unknowns.push({ path: hit.path, nodeType: hit.el.nodeType });
            }
          }
        }
      }
      expect(unknowns, `Runtime emitted nodeType(s) not in the declared ElementNodeType union. Either the type is out of date, or the emit changed. Offenders: ${JSON.stringify(unknowns.slice(0, 5))}`).toEqual([]);
    });

    it('every declared ElementNodeType appears at least once in the fixture (coverage tripwire)', async () => {
      // The symmetric drift alarm to the test above. That test catches
      // "runtime emitted X, but X is not declared"; this test catches
      // "X is declared, but the fixture never renders anything that
      // produces X" — the specific failure mode that caused the whole
      // audit chain in the first place (someone shipped a new component
      // without any structural coverage, so the drift went unnoticed
      // until an external consumer's dogfood test caught it).
      //
      // If this test fails, you have exactly one job: add a `<Component>`
      // to `RICH_FIXTURE` above that produces the missing nodeType.
      const { layout } = await renderPromise;
      const seen = new Set<ElementNodeType>();
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            seen.add(hit.el.nodeType);
          }
        }
      }
      const missing = [...NODE_TYPES].filter(nt => !seen.has(nt));
      expect(
        missing,
        `RICH_FIXTURE does not exercise the following declared ElementNodeType(s): ${missing.join(', ')}.\n\n` +
        `This test's purpose is to prevent silent drift where a new component ships without structural coverage.\n` +
        `Fix: add the corresponding <Component> to RICH_FIXTURE at the top of this file. ` +
        `If the missing type is intentionally not producible from JSX (e.g. an internal-only nodeType), ` +
        `remove it from the ElementNodeType union in packages/core/src/index.ts instead.`,
      ).toEqual([]);
    });

    it('every emitted kind is a member of the declared ElementKind union', async () => {
      const { layout } = await renderPromise;
      const unknowns: { path: string; kind: string }[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            if (!KINDS.has(hit.el.kind as ElementKind)) {
              unknowns.push({ path: hit.path, kind: hit.el.kind });
            }
          }
        }
      }
      expect(unknowns, `Runtime emitted kind(s) not in the declared ElementKind union. Offenders: ${JSON.stringify(unknowns.slice(0, 5))}`).toEqual([]);
    });

    it('every enum-string style value is a member of its declared union', async () => {
      const { layout } = await renderPromise;
      const violations: { path: string; field: string; value: string; validValues: string[] }[] = [];
      const check = (path: string, field: string, value: unknown, set: Set<string>): void => {
        if (typeof value !== 'string') {
          violations.push({ path, field, value: `<not-a-string: ${typeof value}>`, validValues: [...set] });
          return;
        }
        if (!set.has(value)) {
          violations.push({ path, field, value, validValues: [...set] });
        }
      };
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            const s = hit.el.style;
            check(hit.path, 'flexDirection', s.flexDirection, FLEX_DIRECTIONS as Set<string>);
            check(hit.path, 'justifyContent', s.justifyContent, JUSTIFY_CONTENTS as Set<string>);
            check(hit.path, 'alignItems', s.alignItems, ALIGN_ITEMS as Set<string>);
            check(hit.path, 'alignContent', s.alignContent, ALIGN_CONTENTS as Set<string>);
            check(hit.path, 'flexWrap', s.flexWrap, FLEX_WRAPS as Set<string>);
            check(hit.path, 'fontStyle', s.fontStyle, FONT_STYLES as Set<string>);
            check(hit.path, 'textAlign', s.textAlign, TEXT_ALIGNS as Set<string>);
            check(hit.path, 'textDecoration', s.textDecoration, TEXT_DECORATIONS as Set<string>);
            check(hit.path, 'textTransform', s.textTransform, TEXT_TRANSFORMS as Set<string>);
            check(hit.path, 'overflow', s.overflow, OVERFLOWS as Set<string>);
            check(hit.path, 'position', s.position, POSITIONS as Set<string>);
          }
        }
      }
      expect(violations, `Style enum-string values disagree with declared literal unions. First offender: ${JSON.stringify(violations[0])}`).toEqual([]);
    });

    it('required ElementInfo fields present with the right type on every node', async () => {
      const { layout } = await renderPromise;
      const violations: string[] = [];
      for (const page of layout.pages) {
        for (let i = 0; i < page.elements.length; i++) {
          for (const hit of walk(page.elements[i], `pages[?].elements[${i}]`)) {
            const el = hit.el as unknown as Record<string, unknown>;
            for (const field of ['x', 'y', 'width', 'height'] as const) {
              if (typeof el[field] !== 'number') {
                violations.push(`${hit.path}.${field}: expected number, got ${typeof el[field]}`);
              }
            }
            if (typeof el.kind !== 'string') violations.push(`${hit.path}.kind: expected string, got ${typeof el.kind}`);
            if (typeof el.nodeType !== 'string') violations.push(`${hit.path}.nodeType: expected string, got ${typeof el.nodeType}`);
            if (typeof el.style !== 'object' || el.style === null) violations.push(`${hit.path}.style: expected object, got ${typeof el.style}`);
            if (!Array.isArray(el.children)) violations.push(`${hit.path}.children: expected array, got ${typeof el.children}`);
          }
        }
      }
      expect(violations, `ElementInfo required-field violations (first 5):\n  ${violations.slice(0, 5).join('\n  ')}`).toEqual([]);
    });

    it('PageInfo required fields present with the right type on every page', async () => {
      const { layout } = await renderPromise;
      const violations: string[] = [];
      for (let i = 0; i < layout.pages.length; i++) {
        const p = layout.pages[i] as unknown as Record<string, unknown>;
        for (const field of ['width', 'height', 'contentX', 'contentY', 'contentWidth', 'contentHeight'] as const) {
          if (typeof p[field] !== 'number') violations.push(`pages[${i}].${field}: expected number, got ${typeof p[field]}`);
        }
        if (!Array.isArray(p.elements)) violations.push(`pages[${i}].elements: expected array, got ${typeof p.elements}`);
      }
      expect(violations, `PageInfo violations:\n  ${violations.join('\n  ')}`).toEqual([]);
    });
  });
});
