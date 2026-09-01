import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, Text, View } from '@formepdf/react';
import { renderPdfWithLayout } from '../src/index';

const h = React.createElement;

/** A small multi-node doc. `tagged` is passed through only when defined. */
function doc(tagged?: boolean) {
  const props: Record<string, unknown> = {};
  if (tagged !== undefined) props.tagged = tagged;
  const el = serialize(
    h(
      Document,
      props,
      h(
        Page,
        { size: 'Letter', margin: 48 },
        h(Text, { style: { fontSize: 18 } }, 'Heading'),
        h(View, { style: { padding: 8 } }, h(Text, null, 'Body copy that wraps across the page width to exercise layout.')),
      ),
    ),
  ) as Record<string, unknown>;
  return JSON.stringify(el);
}

describe('tagged-on-by-default', () => {
  it('emits a structure tree when tagged is omitted', async () => {
    const { pdf } = await renderPdfWithLayout(doc(undefined));
    const s = Buffer.from(pdf).toString('latin1');
    expect(s).toContain('/StructTreeRoot');
    expect(s).toContain('/MarkInfo');
  });

  it('tagged: false restores an untagged PDF', async () => {
    const { pdf } = await renderPdfWithLayout(doc(false));
    const s = Buffer.from(pdf).toString('latin1');
    expect(s).not.toContain('/StructTreeRoot');
  });

  it('tagging is layout-neutral: LayoutInfo is identical with and without tags', async () => {
    const on = await renderPdfWithLayout(doc(undefined));
    const off = await renderPdfWithLayout(doc(false));
    // Geometry must not shift — the tag tree is built after layout.
    expect(JSON.stringify(on.layout)).toBe(JSON.stringify(off.layout));
  });
});
