import { Document, Page, View, Text } from '@formepdf/react';

export interface Props {
  discount?: number;
}

/** TSX twin of kitchen-sink.svelte for cross-adapter parity tests. */
export default function KitchenSink({ discount = 10 }: Props) {
  return (
    <Document
      title="Spec"
      author="Ada"
      subject="Parity"
      creator="Test"
      lang="en-US"
      pdfUa
      pdfa="2a"
      certification={{ certificatePem: 'CERT', privateKeyPem: 'KEY', reason: 'Approved' }}
      style={{ fontFamily: 'Helvetica', fontSize: 11 }}
    >
      <Page size="Letter" margin="36 72">
        <View style={{ border: '1px solid #333', padding: [8, 16], gap: 4 }} bookmark="Box" wrap={false}>
          <Text style={{ textAlign: 'justify' }}>
            A paragraph that spans
            multiple source lines with    interior spacing
            and a {discount}% interpolation.
          </Text>
          <Text href="https://forme.dev">link text</Text>
        </View>
      </Page>
    </Document>
  );
}
