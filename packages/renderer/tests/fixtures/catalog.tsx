import { Document, Page, View, Text, H1, Image, Table, Row, Cell, Strong } from '@formepdf/react';

/** The 4-way cross-framework equivalence document (React reference). The same
 *  document authored in Preact, Svelte, and Vue must render — through the
 *  extension's own pipeline — to the identical page count and layout tree. */
export default function Catalog({ title }: { title: string }) {
  return (
    <Document title={title}>
      <Page size="A4" style={{ padding: 24 }}>
        <H1>{title}</H1>
        <Text style={{ fontSize: 10 }}>Two products</Text>
        <View style={{ flexDirection: 'row', gap: 12, marginTop: 12 }}>
          <Image src="logo.png" width={40} height={40} />
          <Text>Catalog</Text>
        </View>
        <Table columns={[{ width: { fraction: 0.6 } }, { width: { fraction: 0.4 } }]} style={{ marginTop: 16 }}>
          <Row header>
            <Cell><Text>Product</Text></Cell>
            <Cell><Text>Price</Text></Cell>
          </Row>
          <Row>
            <Cell><Text>Widget</Text></Cell>
            <Cell><Text><Strong>$9.00</Strong></Text></Cell>
          </Row>
        </Table>
      </Page>
    </Document>
  );
}
