/**
 * Stable accessor helpers for the layout tree returned by
 * `renderDocumentWithLayout()`.
 *
 * These helpers exist for one specific reason: the raw `ElementInfo` /
 * `LayoutInfo` shape is documented but the documentation is not a
 * contract, and it has drifted from runtime twice now. Consumers that
 * hard-code invariants like "the text lives on `TextLine` children,
 * not the parent `Text`" break silently every time an internal refactor
 * shifts them.
 *
 * The helpers in this module encapsulate those invariants once, in a
 * narrow, deliberately-maintained surface. Each helper corresponds to
 * one of the layout-time transforms documented on `ElementInfo`. When
 * an invariant changes in the engine, this file is where you update it
 * — the runtime-conformance test (`tests/layout-shape.test.ts`) catches
 * engine drift; helper tests (`tests/layout-helpers.test.ts`) catch
 * drift in the helpers themselves.
 *
 * **Prefer these helpers to raw tree walking unless you specifically
 * need raw shape access** (e.g. custom snapshot comparison, structural
 * analysis the helpers don't cover).
 */

import type {
  LayoutInfo,
  PageInfo,
  ElementInfo,
  ElementNodeType,
} from './index.js';

// ── Root normalization ─────────────────────────────────────────────

/**
 * Any of the layout-tree root shapes a helper might reasonably accept.
 * All helpers that traverse the tree accept this union so consumers
 * don't have to unwrap `layout.pages[0].elements` themselves.
 */
export type LayoutRoot =
  | LayoutInfo
  | PageInfo
  | PageInfo[]
  | ElementInfo
  | ElementInfo[];

function normalizeRoot(root: LayoutRoot): ElementInfo[] {
  if (Array.isArray(root)) {
    // ElementInfo[] or PageInfo[]
    if (root.length === 0) return [];
    if ('elements' in root[0]) {
      // PageInfo[]
      return (root as PageInfo[]).flatMap((p) => p.elements);
    }
    return root as ElementInfo[];
  }
  if ('pages' in root) return root.pages.flatMap((p) => p.elements); // LayoutInfo
  if ('elements' in root) return root.elements; // PageInfo
  return [root]; // ElementInfo
}

// ── Traversal ──────────────────────────────────────────────────────

/**
 * Depth-first walk of every node in `root`, calling `cb` for each.
 *
 * `path` is a human-readable string identifying where in the tree the
 * node lives (e.g. `"[0].children[2]"` for the third child of the first
 * root element). Useful for building error messages; ignore if you
 * don't need it.
 *
 * Callbacks that return `false` skip descent into that node's children.
 * Any other return (including `void`) descends normally.
 */
export function walkElements(
  root: LayoutRoot,
  cb: (node: ElementInfo, path: string) => void | boolean,
): void {
  const roots = normalizeRoot(root);
  const stack: { node: ElementInfo; path: string }[] = [];
  // Reverse-push so we process in source order.
  for (let i = roots.length - 1; i >= 0; i--) {
    stack.push({ node: roots[i], path: `[${i}]` });
  }
  while (stack.length > 0) {
    const { node, path } = stack.pop()!;
    const skip = cb(node, path) === false;
    if (skip) continue;
    for (let i = node.children.length - 1; i >= 0; i--) {
      stack.push({ node: node.children[i], path: `${path}.children[${i}]` });
    }
  }
}

/**
 * Every node in `root` for which `predicate` returns truthy. Order is
 * depth-first, source-order.
 */
export function findElements(
  root: LayoutRoot,
  predicate: (node: ElementInfo, path: string) => boolean,
): ElementInfo[] {
  const hits: ElementInfo[] = [];
  walkElements(root, (node, path) => {
    if (predicate(node, path)) hits.push(node);
  });
  return hits;
}

/**
 * First node in `root` for which `predicate` returns truthy, or `null`
 * if none. Order is depth-first, source-order. Stops as soon as a match
 * is found (does not descend into the match's children).
 */
export function findFirstElement(
  root: LayoutRoot,
  predicate: (node: ElementInfo, path: string) => boolean,
): ElementInfo | null {
  let hit: ElementInfo | null = null;
  walkElements(root, (node, path) => {
    if (hit !== null) return false;
    if (predicate(node, path)) {
      hit = node;
      return false;
    }
  });
  return hit;
}

// ── Text access ────────────────────────────────────────────────────

/**
 * Every `TextLine` leaf descendant of `node`, in source order. If
 * `node` is itself a `TextLine`, returns `[node]`.
 *
 * This is the load-bearing helper: it encapsulates the invariant that
 * text lives on `TextLine` children, never on the parent `Text` block.
 */
export function getTextLines(node: ElementInfo): ElementInfo[] {
  if (node.nodeType === 'TextLine') return [node];
  const lines: ElementInfo[] = [];
  walkElements(node.children, (n) => {
    if (n.nodeType === 'TextLine') {
      lines.push(n);
      return false; // TextLines don't nest further; skip descent
    }
  });
  return lines;
}

/**
 * Concatenated text of every `TextLine` descendant of `node`, joined
 * with `"\n"`. If `node` is itself a `TextLine`, returns its own text.
 *
 * Lines are joined with newlines rather than spaces because the layout
 * engine may have wrapped a single JSX string across multiple lines,
 * and preserving that structure is more useful than silently smushing
 * or space-joining. If you want a flat string, `.replace(/\n/g, ' ')`
 * the result.
 *
 * Returns `""` if no `TextLine` descendants exist.
 */
export function getNodeText(node: ElementInfo): string {
  const lines = getTextLines(node);
  return lines
    .map((l) => (typeof l.textContent === 'string' ? l.textContent : ''))
    .join('\n');
}

// ── Structural queries encapsulating layout-time transforms ────────

/**
 * If `node` is a heading (`H1`–`H6`), returns the numeric level (1–6).
 * Returns `null` otherwise.
 *
 * Encapsulates the invariant that headings render as six discriminated
 * nodeTypes (`H1`, `H2`, …) rather than a generic `Heading` node with a
 * separate `level` field.
 */
export function getHeadingLevel(node: ElementInfo): 1 | 2 | 3 | 4 | 5 | 6 | null {
  const nt = node.nodeType;
  if (nt === 'H1') return 1;
  if (nt === 'H2') return 2;
  if (nt === 'H3') return 3;
  if (nt === 'H4') return 4;
  if (nt === 'H5') return 5;
  if (nt === 'H6') return 6;
  return null;
}

/**
 * Every `TableRow` that is a direct child of `parent`.
 *
 * Encapsulates the invariant that `<Table>` unwraps at layout time —
 * its `<Row>` children become sibling `TableRow` nodes on the
 * containing page/View, and there is no `Table` wrapper node to hang
 * rows off of. Pass the containing `page` (or the parent `View` that
 * held the `<Table>` in JSX) as `parent`.
 *
 * Returns rows in source order.
 */
export function getTableRows(parent: PageInfo | ElementInfo): ElementInfo[] {
  const kids = 'elements' in parent ? parent.elements : parent.children;
  return kids.filter((k) => k.nodeType === 'TableRow');
}

/**
 * The `FixedHeader` and `FixedFooter` nodes on `page`. Encapsulates the
 * invariant that `<Fixed position="header">` and `<Fixed position="footer">`
 * produce two distinct nodeTypes rather than a shared `Fixed` node with
 * a `position` field.
 *
 * Both arrays may be empty. Multiple entries in either array indicate
 * repeating fixed regions (e.g. when the same `<Fixed>` repeats across
 * page breaks — the layout may emit one node per occurrence).
 */
export function getFixedRegions(page: PageInfo): {
  header: ElementInfo[];
  footer: ElementInfo[];
} {
  const header: ElementInfo[] = [];
  const footer: ElementInfo[] = [];
  for (const el of page.elements) {
    if (el.nodeType === 'FixedHeader') header.push(el);
    else if (el.nodeType === 'FixedFooter') footer.push(el);
  }
  return { header, footer };
}

/**
 * Every `ListItem` child of a `List` node. Returns `[]` if `list` is
 * not a `List` (silently — no throw, so this composes cleanly with
 * `.flatMap`).
 */
export function getListItems(list: ElementInfo): ElementInfo[] {
  if (list.nodeType !== 'List') return [];
  return list.children.filter((c) => c.nodeType === 'ListItem');
}

/**
 * Rendered text of the marker (`Lbl`) child of a `ListItem` — e.g.
 * `"1."` for the first item of an `<OrderedList>`, `"•"` for an
 * `<UnorderedList>` item. Returns `null` if `item` is not a `ListItem`
 * or has no `Lbl` child.
 *
 * Encapsulates the invariant that list markers are separate `Lbl`
 * children of each `ListItem` rather than a field on `ListItem` itself.
 */
export function getListItemMarker(item: ElementInfo): string | null {
  if (item.nodeType !== 'ListItem') return null;
  const lbl = item.children.find((c) => c.nodeType === 'Lbl');
  if (!lbl) return null;
  return getNodeText(lbl);
}

// ── Node-type narrowing utility ────────────────────────────────────

/**
 * Type-guard for narrowing to a specific `ElementNodeType`. Useful for
 * `.filter(isNodeType('TableRow'))` chains.
 */
export function isNodeType<T extends ElementNodeType>(
  nodeType: T,
): (node: ElementInfo) => node is ElementInfo & { nodeType: T } {
  return (node): node is ElementInfo & { nodeType: T } => node.nodeType === nodeType;
}
