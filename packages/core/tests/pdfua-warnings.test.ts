import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Text } from '@formepdf/react';
import { standardFonts } from '@formepdf/fonts-standard';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

/** pdfUa doc; when `withFonts` is false, no embeddable font is registered. */
function docWith(withFonts: boolean) {
  const doc = serialize(
    h(Document, null, h(Page, { size: 'Letter', margin: 48 }, h(Text, { style: { fontSize: 12 } }, 'Hello World'))),
  ) as Record<string, unknown>;
  doc.pdfUa = true;
  doc.tagged = true;
  doc.metadata = { lang: 'en-US' };
  if (withFonts) {
    doc.fonts = standardFonts().map((f) => ({
      family: f.family,
      src: Buffer.from(f.src).toString('base64'),
      weight: f.fontWeight,
      italic: f.fontStyle === 'italic',
    }));
  }
  return JSON.stringify(doc);
}

describe('pdfUa warnings channel', () => {
  it('surfaces a warning when pdfUa is requested but no embeddable font is registered', async () => {
    const { warnings } = await renderPdfWithLayout(docWith(false));
    expect(warnings.some((w) => w.startsWith('pdfUa:') && /not embedded/.test(w) && /fonts-standard/.test(w))).toBe(true);
  });

  it('produces no font warnings when fonts-standard is registered', async () => {
    const { warnings } = await renderPdfWithLayout(docWith(true));
    expect(warnings.some((w) => w.startsWith('pdfUa:') && /not embedded/.test(w))).toBe(false);
  });
});
