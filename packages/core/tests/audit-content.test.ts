import { describe, it, expect } from 'vitest';
import React from 'react';
import { serialize, Document, Page, View, Text } from '@formepdf/react';
import { renderSerializedDocWithLayout } from '../src/index';

const h = React.createElement;

/** White text on a white background — the audit's "painted in exactly the
 *  colour of the ground under it" case, invisible by construction. */
function invisibleTextDoc() {
  return serialize(
    h(
      Document,
      null,
      h(
        Page,
        { size: 'Letter', margin: 48 },
        h(
          View,
          { style: { backgroundColor: '#ffffff', padding: 8 } },
          h(Text, { style: { color: '#ffffff' } }, 'invisible confirmation code'),
        ),
      ),
    ),
  ) as Record<string, unknown>;
}

describe('auditContent through @formepdf/core (mirrors the HTML path)', () => {
  it('reports invisible content as render defects in warnings when enabled', async () => {
    const { warnings } = await renderSerializedDocWithLayout(invisibleTextDoc(), {
      auditContent: true,
    });
    expect(
      warnings.some((w) => w.startsWith('render defect:')),
      `expected an audit defect, got: ${JSON.stringify(warnings)}`,
    ).toBe(true);
  });

  it('costs nothing when off: byte-identical output, no audit warnings', async () => {
    const plain = await renderSerializedDocWithLayout(invisibleTextDoc());
    const explicitOff = await renderSerializedDocWithLayout(invisibleTextDoc(), {
      auditContent: false,
    });
    expect(Buffer.from(explicitOff.pdf).equals(Buffer.from(plain.pdf))).toBe(true);
    expect(plain.warnings.some((w) => w.startsWith('render defect:'))).toBe(false);
    expect(explicitOff.warnings.some((w) => w.startsWith('render defect:'))).toBe(false);
  });

  it('the audit changes warnings only — PDF bytes stay byte-identical', async () => {
    const off = await renderSerializedDocWithLayout(invisibleTextDoc());
    const on = await renderSerializedDocWithLayout(invisibleTextDoc(), { auditContent: true });
    expect(Buffer.from(on.pdf).equals(Buffer.from(off.pdf))).toBe(true);
  });
});
