/** @jsxImportSource preact */
import { Document, Page, Table, Row, Cell, Text } from '@formepdf/preact';

export default function TableFixture() {
  const rows = Array.from({ length: 8 }, (_, i) => ({
    id: `SKU-${String(i + 1).padStart(3, '0')}`,
    name: `Widget ${i + 1}`,
    qty: i + 1,
    price: (i + 1) * 9.99,
  }));
  return (
    <Document title="Table">
      <Page size="Letter" margin={36}>
        <Table
          columns={[
            { width: { fixed: 100 } },
            { width: { fraction: 1 } },
            { width: { fixed: 60 } },
            { width: { fixed: 80 } },
          ]}
        >
          <Row header>
            <Cell>
              <Text style={{ fontWeight: 700 }}>SKU</Text>
            </Cell>
            <Cell>
              <Text style={{ fontWeight: 700 }}>Item</Text>
            </Cell>
            <Cell>
              <Text style={{ fontWeight: 700 }}>Qty</Text>
            </Cell>
            <Cell>
              <Text style={{ fontWeight: 700 }}>Price</Text>
            </Cell>
          </Row>
          {rows.map((r) => (
            <Row key={r.id}>
              <Cell><Text>{r.id}</Text></Cell>
              <Cell><Text>{r.name}</Text></Cell>
              <Cell><Text>{r.qty}</Text></Cell>
              <Cell><Text>${r.price.toFixed(2)}</Text></Cell>
            </Row>
          ))}
        </Table>
      </Page>
    </Document>
  );
}
