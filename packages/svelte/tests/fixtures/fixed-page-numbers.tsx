import { Document, Page, Text, Fixed } from '@formepdf/react';

export interface Props {
  paragraphs?: number;
}

/** TSX twin of fixed-page-numbers.svelte for cross-adapter parity tests. */
export default function FixedPageNumbers({ paragraphs = 5 }: Props) {
  return (
    <Document>
      <Page>
        <Fixed position="header" style={{ paddingBottom: 8 }}>
          <Text style={{ fontSize: 9, color: '#666666' }}>Quarterly Report</Text>
        </Fixed>
        <Fixed position="footer" style={{ paddingTop: 8 }}>
          <Text style={{ fontSize: 9, textAlign: 'center' }}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
        </Fixed>
        {Array.from({ length: paragraphs }, (_unused, i) => (
          <Text key={i} style={{ marginBottom: 12 }}>Paragraph {i + 1}: repeated body copy that fills the page so a long document breaks across multiple pages and the footer repeats.</Text>
        ))}
      </Page>
    </Document>
  );
}
