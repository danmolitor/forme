import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Text, View } from '@formepdf/react';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

/** A document with an external link and an internal (#bookmark) link, rendered
 *  tagged + pdfUa. */
function linkedDoc() {
  const doc = serialize(
    h(
      Document,
      null,
      h(
        Page,
        { size: 'Letter', margin: 48 },
        h(View, { bookmark: 'Target' }, h(Text, { style: { fontSize: 14 } }, 'Target Section')),
        h(Text, { href: 'https://example.com', style: { fontSize: 12 } }, 'External link'),
        h(Text, { href: '#Target', style: { fontSize: 12 } }, 'Internal link'),
      ),
    ),
  ) as Record<string, unknown>;
  doc.pdfUa = true;
  doc.tagged = true;
  doc.metadata = { lang: 'en-US' };
  return JSON.stringify(doc);
}

describe('pdfUa link tagging (7.18.5-1 /Link structure elements)', () => {
  it('emits /Link structure elements, OBJR references, and annotation /StructParent', async () => {
    const { pdf } = await renderPdfWithLayout(linkedDoc());
    const s = Buffer.from(pdf).toString('latin1');

    // Links tagged as /Link structure elements (not just P/Span).
    expect(s).toContain('/S /Link');
    // The /Link element references its annotation via an OBJR.
    expect(s).toContain('/Type /OBJR');
    // The link annotation is connected back via /StructParent.
    expect(s).toMatch(/\/Subtype \/Link[\s\S]*?\/StructParent \d+/);
    // The annotation still carries its /Contents (7.18.1-2 / 7.18.5-2).
    expect(s).toContain('/Contents (https://example.com)');
  });
});
