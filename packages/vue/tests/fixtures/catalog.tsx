import { Document, Page, View, Text, Table, Row, Cell, Image, Strong } from '@formepdf/react';

const products = [
  { name: 'Widget', price: '9.00', badge: 'SALE' },
  { name: 'Gadget', price: '19.00', badge: 'NEW' },
];

/** A catalog-equivalent doc: tables, absolute badges in POSITIONED (relative)
 *  cards (per 7b), an image, and flex. */
export default function Catalog({ title }: { title: string }) {
  return (
    <Document title={title}>
      <Page size="A4" style={{ padding: 24 }}>
        <Text style={{ fontSize: 18, fontWeight: 700 }}>{title}</Text>
        <View style={{ flexDirection: 'row', gap: 12, marginTop: 12 }}>
          {products.map((p, i) => (
            <View
              key={i}
              style={{ position: 'relative', flexBasis: '48%', padding: 12, borderWidth: 1, borderColor: '#e2e8f0', borderRadius: 6 }}
            >
              <View style={{ position: 'absolute', top: -8, right: -8, backgroundColor: '#dc2626', padding: 4, borderRadius: 4 }}>
                <Text style={{ fontSize: 7, color: '#ffffff', fontWeight: 700 }}>{p.badge}</Text>
              </View>
              <Image src="logo.png" width={40} height={40} />
              <Text style={{ marginTop: 6 }}>{p.name}</Text>
            </View>
          ))}
        </View>
        <Table columns={[{ width: { fraction: 0.6 } }, { width: { fraction: 0.4 } }]} style={{ marginTop: 16 }}>
          <Row header>
            <Cell><Text>Product</Text></Cell>
            <Cell><Text>Price</Text></Cell>
          </Row>
          {products.map((p, i) => (
            <Row key={i}>
              <Cell><Text>{p.name}</Text></Cell>
              <Cell><Text><Strong>{p.price}</Strong></Text></Cell>
            </Row>
          ))}
        </Table>
      </Page>
    </Document>
  );
}
