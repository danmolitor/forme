import { describe, it, expect } from 'vitest';
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
