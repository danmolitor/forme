import { describe, it, expect } from 'vitest';
import { renderDocumentWithLayout } from '../src/index';
// Registers `toMatchPDFSnapshot` on Vitest's `expect`.
import '@pdf-testkit/vitest';

import Invoice from '../../../templates/invoice';
import Receipt from '../../../templates/receipt';
import Catalog from '../../../templates/catalog';
import Report from '../../../templates/report';
import ShippingLabel from '../../../templates/shipping-label';
import ChartsShowcase from '../../../templates/charts-showcase';
import FeatureShowcase from '../../../templates/feature-showcase';
import GridDashboard from '../../../templates/grid-dashboard';
import Typography from '../../../templates/typography';

import invoiceData from '../../../templates/invoice-data.json';
import receiptData from '../../../templates/receipt-data.json';
import catalogData from '../../../templates/catalog-data.json';
import reportData from '../../../templates/report-data.json';
import shippingLabelData from '../../../templates/shipping-label-data.json';
import chartsShowcaseData from '../../../templates/charts-showcase-data.json';
import gridDashboardData from '../../../templates/grid-dashboard-data.json';
import typographyData from '../../../templates/typography-data.json';

/**
 * Structural regression coverage for the demo templates in `/templates`.
 *
 * These are distinct from the npm-shipped set covered by
 * `templates.regression.test.ts`: same five document *types*, but different,
 * richer implementations. They're what `forme dev` renders and what the docs
 * and screenshots are built from, and between them they exercise engine
 * surface the shipped package doesn't — `catalog` uses bookmarks and inline
 * `<Svg>` badges, `report` pulls in `@formepdf/tailwind` and `<PageBreak>`.
 *
 * Both suites are worth having: the shipped set guards the public API, this
 * set guards the engine features the demos advertise.
 *
 * ## Opaque nodes
 *
 * `catalog` and `report` draw their charts and swatches as hand-rolled `<Svg>`
 * content strings, not engine-native chart nodes. `shipping-label` uses
 * `<QrCode>`. All three surface to pdf-testkit as single nodes with a bbox but
 * no inspectable interior — a regression *inside* that SVG/QR payload will not
 * be caught here. Position, size, and page placement are covered; glyph-level
 * chart content is not.
 */
const CASES: Record<string, [(data: any) => any, unknown]> = {
  invoice: [Invoice, invoiceData],
  receipt: [Receipt, receiptData],
  catalog: [Catalog, catalogData],
  report: [Report, reportData],
  'shipping-label': [ShippingLabel, shippingLabelData],
  // Gallery templates (docs template-gallery). charts-showcase and
  // feature-showcase are self-contained (data ignored / none exists);
  // all four render deterministically on undefined data as of the
  // 2026-09 contract fixes.
  'charts-showcase': [ChartsShowcase, chartsShowcaseData],
  'feature-showcase': [FeatureShowcase, undefined],
  'grid-dashboard': [GridDashboard, gridDashboardData],
  typography: [Typography, typographyData],
};

describe('demo template regressions (/templates)', () => {
  it.each(Object.entries(CASES))(
    '%s layout matches the committed structural baseline',
    async (name, [template, data]) => {
      const { layout } = await renderDocumentWithLayout(template(data));

      // Pass LayoutInfo directly — the authoritative FormePDF fast path
      // (no PDF parsing, every node at confidence 1.0).
      await expect(layout).toMatchPDFSnapshot({ snapshotName: `demo-${name}` });
    },
  );
});
