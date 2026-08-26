import { describe, it, expect } from 'vitest';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { getTemplate, listTemplates } from '@formepdf/templates';
import {
  invoiceExample,
  receiptExample,
  reportExample,
  shippingLabelExample,
  letterExample,
  type InvoiceData,
  type ReceiptData,
  type ReportData,
  type ShippingLabelData,
  type LetterData,
} from '@formepdf/templates/schemas';
import { renderDocumentWithLayout } from '../src/index';
// Registers `toMatchPDFSnapshot` on Vitest's `expect`.
import '@pdf-testkit/vitest';

/**
 * Structural regression coverage for the templates we actually ship on npm
 * (`@formepdf/templates`). These are the documents users get by name via
 * `getTemplate()`, so a silent layout regression here ships to every consumer.
 *
 * This suite lives in `packages/core` rather than `packages/templates` for two
 * reasons: rendering needs the WASM engine that only core builds, and core is
 * already wired into CI's `Test Core` step (CI builds Templates first). It sits
 * alongside `pdf-testkit.dogfood.test.ts` and is purely additive to it — that
 * test covers an ad-hoc document, this one covers the shipped surface.
 *
 * The fixture data is each schema's own `*Example` export, which
 * `packages/templates/src/schemas/index.ts` already validates against its Zod
 * schema at import time. So the examples cannot drift from the schemas without
 * failing loudly, and this suite gets realistic shapes for free: the invoice has
 * a 5-row line-item table that pages, the report has 6 pages of sections.
 */

/** Every template `@formepdf/templates` exposes, paired with its example data. */
const CASES = {
  invoice: invoiceExample satisfies InvoiceData,
  receipt: receiptExample satisfies ReceiptData,
  report: reportExample satisfies ReportData,
  'shipping-label': shippingLabelExample satisfies ShippingLabelData,
  letter: letterExample satisfies LetterData,
} as const;

describe('shipped template regressions (@formepdf/templates)', () => {
  it.each(Object.entries(CASES))(
    '%s layout matches the committed structural baseline',
    async (name, data) => {
      const template = getTemplate(name);
      if (!template) throw new Error(`getTemplate(${JSON.stringify(name)}) returned null`);

      const { layout } = await renderDocumentWithLayout(template(data));

      // Pass LayoutInfo directly — the authoritative FormePDF fast path
      // (no PDF parsing, every node at confidence 1.0).
      await expect(layout).toMatchPDFSnapshot({ snapshotName: `template-${name}` });
    },
  );

  /**
   * Fidelity tripwire for the CI regression demo.
   *
   * `scripts/render-template-layout.mjs` is a *second* place this repo renders a
   * shipped template: CI runs it to produce the `current` file that
   * @pdf-testkit/action diffs and comments on the PR. If it ever drifts from
   * what this suite asserts on — different entry point, different fixture data,
   * different render options — the PR comment would describe a document nobody
   * actually gates, which is worse than no comment at all.
   *
   * Byte equality rather than a structural diff on purpose: a semantic
   * comparison is exactly what the demo itself performs, so using one here
   * would blind this check to any drift the differ happens to treat as
   * equivalent.
   */
  it('the CI demo producer renders byte-identically to this suite', async () => {
    // The script imports the built `dist/`, so it can't run against a bare
    // checkout. CI always builds Core first; locally, skip rather than fail
    // with a module-resolution error that says nothing about templates.
    if (!existsSync(new URL('../dist/index.js', import.meta.url))) {
      console.warn('skipping: packages/core/dist not built');
      return;
    }

    const out = join(tmpdir(), `forme-demo-parity-${process.pid}.json`);
    try {
      execFileSync(process.execPath, ['scripts/render-template-layout.mjs', 'invoice', out], {
        cwd: fileURLToPath(new URL('..', import.meta.url)),
        stdio: 'pipe',
      });

      const template = getTemplate('invoice');
      if (!template) throw new Error('getTemplate("invoice") returned null');
      const { layout } = await renderDocumentWithLayout(template(invoiceExample));

      // Must match how the script serializes, or this compares formatting.
      expect(readFileSync(out, 'utf8')).toBe(JSON.stringify(layout, null, 2) + '\n');
    } finally {
      rmSync(out, { force: true });
    }
  });

  /**
   * Coverage tripwire. Adding a template to `@formepdf/templates` without adding
   * it here would ship an untested document — the failure mode this suite exists
   * to prevent. Fail by name rather than on a bare count mismatch.
   */
  it('covers every template exposed by listTemplates()', () => {
    const exposed = listTemplates().map(t => t.name).sort();
    const covered = Object.keys(CASES).sort();

    const untested = exposed.filter(n => !covered.includes(n));
    const stale = covered.filter(n => !exposed.includes(n));

    expect(
      untested,
      `Template(s) [${untested.join(', ')}] are exported by @formepdf/templates but have no ` +
        `regression baseline. Add them to CASES in this file with their schema's *Example data.`,
    ).toEqual([]);
    expect(
      stale,
      `CASES lists [${stale.join(', ')}], which listTemplates() no longer exposes. ` +
        `Remove the entry and delete its stale snapshot.`,
    ).toEqual([]);
  });
});
