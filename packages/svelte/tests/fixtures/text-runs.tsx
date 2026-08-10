import { Document, Page, Text } from '@formepdf/react';

export interface Props {
  price?: number;
}

/** TSX twin of text-runs.svelte for cross-adapter parity tests. */
export default function TextRuns({ price = 42 }: Props) {
  return (
    <Document>
      <Page>
        <Text style={{ fontSize: 12 }}>Was <Text style={{ textDecoration: 'line-through', color: '#999999' }}>$56.00</Text> <Text style={{ fontWeight: 700 }}>${price}.00</Text> due now</Text>
        <Text>Visit <Text href="https://forme.dev" style={{ color: '#0000ff' }}>our site</Text> for details</Text>
      </Page>
    </Document>
  );
}
