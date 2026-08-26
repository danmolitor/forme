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
    await expect(layout).toMatchPDFSnapshot({ snapshotName: 'invoice' });
  });
});
