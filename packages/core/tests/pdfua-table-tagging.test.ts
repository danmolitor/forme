import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Table, Row, Cell, Text } from '@formepdf/react';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

/** A table with a header row and a spanning cell, tagged + pdfUa. */
function tableDoc() {
  const doc = serialize(
    h(
      Document,
      null,
      h(
        Page,
        { size: 'Letter', margin: 48 },
        h(
          Table,
          { columns: [{ width: { fraction: 0.5 } }, { width: { fraction: 0.5 } }] },
          h(
            Row,
            { header: true },
            h(Cell, null, h(Text, null, 'A')),
            h(Cell, null, h(Text, null, 'B')),
          ),
          h(Row, null, h(Cell, { colSpan: 2 }, h(Text, null, 'spans two'))),
        ),
      ),
    ),
  ) as Record<string, unknown>;
  doc.pdfUa = true;
  doc.tagged = true;
  doc.metadata = { lang: 'en-US' };
  return JSON.stringify(doc);
}

describe('pdfUa table tagging (7.5-1 Scope, 7.2-43 ColSpan)', () => {
  it('emits /Scope on TH and /ColSpan on a spanning cell in one /A dict', async () => {
    const { pdf } = await renderPdfWithLayout(tableDoc());
    const s = Buffer.from(pdf).toString('latin1');

    // Header cells carry column scope.
    expect(s).toMatch(/\/S \/TH[\s\S]*?\/A << \/O \/Table \/Scope \/Column/);
    // The spanning cell declares /ColSpan 2.
    expect(s).toMatch(/\/S \/TD[\s\S]*?\/A << \/O \/Table \/ColSpan 2 >>/);
  });
});
