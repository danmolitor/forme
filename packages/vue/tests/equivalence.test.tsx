/**
 * Cross-framework equivalence — the acceptance gate for @formepdf/vue.
 *
 * The same catalog-equivalent document (tables, absolute badges in
 * positioned cards, image, flex) authored in React and in Vue must
 * serialize to deep-equal Forme document JSON. React is the reference
 * serializer; @formepdf/svelte's own parity suite proves Svelte === React,
 * so Vue === React here closes the three-way loop transitively.
 *
 * Runs are normalized before comparison: React splits text runs at JSX
 * child boundaries (`<Strong>{price}</Strong>` etc.), while Vue's SSR
 * merges contiguous text before the shared parser sees it — an SSR
 * artifact, identical to Svelte's. Adjacent runs with equal style render
 * identically; runs with DIFFERENT styles are never merged, so real style
 * drift still fails.
 */
import { describe, it, expect } from 'vitest';
import { serialize as serializeReact } from '@formepdf/react';
import { serialize as serializeVue } from '../src/index.js';
import CatalogReact from './fixtures/catalog';
// @ts-expect-error .vue fixtures have no type declarations in tests
import CatalogVue from './fixtures/catalog.vue';

function normalizeRuns(doc: unknown): unknown {
  return JSON.parse(
    JSON.stringify(doc, (_key, value) => {
      if (
        value !== null &&
        typeof value === 'object' &&
        !Array.isArray(value) &&
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
    }),
  );
}

describe('cross-framework equivalence (Vue === React)', () => {
  it('catalog-equivalent doc: tables, positioned-parent absolute badges, image, flex', async () => {
    const title = 'Q3 Catalog';
    const vueDoc = await serializeVue(CatalogVue, { props: { title } });
    const reactDoc = serializeReact(<CatalogReact title={title} />);
    expect(normalizeRuns(vueDoc)).toEqual(normalizeRuns(reactDoc));
  });
});
