import { readFileSync } from 'node:fs';
import { describe, it, expect } from 'vitest';
import { renderHtmlWithLayout } from '@formepdf/html';
// Registers `toMatchPDFSnapshot` on Vitest's `expect`.
import '@pdf-testkit/vitest';

/**
 * Structural-regression baselines for the HTML input path's fixture corpus.
 *
 * The dogfood experiment's sample-size finding (2026-09-05): the last weeks
 * of layout churn happened almost entirely in the HTML path — the table
 * auto-layout fix, the measure/layout gate fixes, the float transformation —
 * and NONE of it was visible to pdf-testkit, because only the React-template
 * surface had baselines. Ten times the sample was sitting here, invisible.
 * These baselines close that: every future engine or mapper change that
 * moves one of these fixtures now produces a semantic diff in the same CI
 * surface as the shipped templates, instead of relying on the byte wall
 * being run by hand.
 *
 * `renderHtmlWithLayout` resolves to the WORKSPACE @formepdf/html (built by
 * the "Build HTML WASM package" CI step before this suite runs), so the
 * layouts track the current branch's engine, not the last published WASM.
 * Same fixture set as scripts/byte-wall.sh, same reason: these six are the
 * float-free corpus every layout change is measured against.
 */
const FIXTURES = [
  'invoice',
  'letterhead',
  'report',
  'statement',
  'zebra-invoice',
  'dashed-borders',
] as const;

describe('html fixture regressions (the byte-wall corpus, structurally)', () => {
  it.each(FIXTURES.map((f) => [f] as const))(
    '%s layout matches the committed structural baseline',
    async (name) => {
      const html = readFileSync(
        new URL(`../../../html/tests/fixtures/${name}.html`, import.meta.url),
        'utf8',
      );
      const { layout } = renderHtmlWithLayout(html);
      await expect(layout).toMatchPDFSnapshot({ snapshotName: `html-fixture-${name}` });
    },
  );
});
