import { describe, it, expect } from 'vitest';
import React from 'react';
import { Document, Page, Text } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderDocument } from '../src/index';

const h = React.createElement;

// `embedStandardFonts` on <Document> (#197): the base-14 fonts embedded as
// their fonts-standard substitutes, with no conformance claim.
function invoice(props: Record<string, unknown>) {
  return h(
    Document,
    props,
    h(Page, { size: 'Letter', margin: 48 },
      h(Text, { style: { fontWeight: 700 } }, 'Invoice INV-0042'),
      h(Text, null, 'Total due: $1,250.00')),
  );
}

const latin1 = (pdf: Uint8Array) => Buffer.from(pdf).toString('latin1');
const embedded = (pdf: Uint8Array) => latin1(pdf).split('/FontFile2').length - 1;

describe('embedStandardFonts', () => {
  it('embeds regular and bold with no PDF/UA or PDF/A claim', async () => {
    const pdf = await renderDocument(invoice({ fonts: standardFonts(), embedStandardFonts: true }));
    expect(embedded(pdf)).toBe(2);
    for (const claim of ['pdfuaid', 'pdfaid', '/OutputIntents']) {
      expect(latin1(pdf)).not.toContain(claim);
    }
  });

  it('is off by default', async () => {
    const pdf = await renderDocument(invoice({ fonts: standardFonts() }));
    expect(embedded(pdf)).toBe(0);
  });
});
