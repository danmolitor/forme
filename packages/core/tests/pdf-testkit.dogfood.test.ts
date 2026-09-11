import { describe, it, expect } from 'vitest';
import React from 'react';
import * as C from '@formepdf/react';
import { renderDocumentWithLayout } from '../src/index';
// Registers `toMatchPDFSnapshot` on Vitest's `expect`. This is the dogfood:
// FormePDF verifies its own rendered output with pdf-testkit.
import '@pdf-testkit/vitest';

const { Document, View, Text, Table, Row, Cell, H1, H2 } = C;
const h = React.createElement;

/** A representative multi-section invoice-like document. */
function invoice() {
  return h(
    Document,
    null,
    h(View, { style: { padding: 24 } },
      h(H1, null, 'Acme Corp — Invoice #1042'),
      h(H2, null, 'Bill To'),
      h(Text, null, 'Wile E. Coyote, 1 Desert Rd, AZ'),
      h(H2, null, 'Line Items'),
      h(Table, { columns: [{ width: { fraction: 0.6 } }, { width: { fraction: 0.2 } }, { width: { fraction: 0.2 } }] },
        h(Row, { header: true },
          h(Cell, null, h(Text, null, 'Item')),
          h(Cell, null, h(Text, null, 'Qty')),
          h(Cell, null, h(Text, null, 'Price')),
        ),
        h(Row, null,
          h(Cell, null, h(Text, null, 'Rocket Skates')),
          h(Cell, null, h(Text, null, '2')),
          h(Cell, null, h(Text, null, '$199.00')),
        ),
        h(Row, null,
          h(Cell, null, h(Text, null, 'Giant Magnet')),
          h(Cell, null, h(Text, null, '1')),
          h(Cell, null, h(Text, null, '$89.00')),
        ),
      ),
    ),
  );
}

describe('pdf-testkit dogfood — FormePDF verifies its own layout', () => {
  it('invoice layout matches the committed structural baseline', async () => {
    const { layout } = await renderDocumentWithLayout(invoice());
    // Pass the LayoutInfo directly — the authoritative FormePDF fast path
    // (no PDF parsing, every node at confidence 1.0).
    //
    // `contentChanges` (pdf-testkit 0.5.0, opt-in) also reports a text edit
    // at a stable slot — structural diffing is silent on those by design.
    // These baselines render FIXED fixtures, so any text change is the
    // renderer changing what it emits (an encoding or glyph-substitution
    // regression, the "≤ printed as ?" class), never legitimate data drift.
    // Verified to bite: a $98.00 -> $98.99 cell edit in the invoice fixture
    // raises element-content-changed (warn), and the matcher fails on any
    // severity above info.
    await expect(layout).toMatchPDFSnapshot({ snapshotName: 'invoice', contentChanges: true });
  });
});
