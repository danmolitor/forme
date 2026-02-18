import { Document, Page, View, Text, Table, Row, Cell, Fixed } from '@forme/react';

export default function Invoice(data: any) {
  const items = data.items || [];
  const subtotal = items.reduce((sum: number, item: any) => sum + item.quantity * item.unitPrice, 0);
  const tax = subtotal * (data.taxRate || 0);
  const total = subtotal + tax;

  return (
    <Document title={`Invoice ${data.invoiceNumber}`} author={data.company.name}>
      <Page size="Letter" margin={48}>
        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 8, borderWidth: { top: 1, right: 0, bottom: 0, left: 0 }, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.company.name}</Text>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Company Header */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginBottom: 32 }}>
          <View>
            <View style={{ width: 48, height: 48, backgroundColor: '#2563eb', borderRadius: 8, marginBottom: 12, justifyContent: 'center', alignItems: 'center' }}>
              <Text style={{ fontSize: 18, fontWeight: 700, color: '#ffffff', textAlign: 'center', }}>{data.company.initials}</Text>
            </View>
            <Text style={{ fontSize: 16, fontWeight: 700, color: '#1e293b' }}>{data.company.name}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 4 }}>{data.company.address}</Text>
            <Text style={{ fontSize: 9, color: '#64748b' }}>{data.company.cityStateZip}</Text>
            <Text style={{ fontSize: 9, color: '#64748b' }}>{data.company.email}</Text>
          </View>
          <View style={{ alignItems: 'flex-end' }}>
            <Text style={{ fontSize: 32, fontWeight: 700, color: '#2563eb' }}>INVOICE</Text>
            <Text style={{ fontSize: 10, color: '#64748b', marginTop: 8 }}>Invoice No: {data.invoiceNumber}</Text>
            <Text style={{ fontSize: 10, color: '#64748b', marginTop: 2 }}>Date: {data.date}</Text>
            <Text style={{ fontSize: 10, color: '#64748b', marginTop: 2 }}>Due: {data.dueDate}</Text>
          </View>
        </View>

        {/* Bill To / Ship To */}
        <View style={{ flexDirection: 'row', gap: 32, marginBottom: 24 }}>
          <View style={{ flexGrow: 1 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#2563eb', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 8 }}>Bill To</Text>
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#1e293b' }}>{data.billTo.name}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.billTo.company}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.billTo.address}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.billTo.cityStateZip}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.billTo.email}</Text>
          </View>
          <View style={{ flexGrow: 1 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#2563eb', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 8 }}>Ship To</Text>
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#1e293b' }}>{data.shipTo.name}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.shipTo.address}</Text>
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.shipTo.cityStateZip}</Text>
          </View>
        </View>

        {/* Line Items Table */}
        <Table columns={[
          { width: { fraction: 0.45 } },
          { width: { fraction: 0.15 } },
          { width: { fraction: 0.2 } },
          { width: { fraction: 0.2 } }
        ]}>
          <Row header style={{ backgroundColor: '#2563eb' }}>
            <Cell style={{ padding: 10 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff' }}>Description</Text></Cell>
            <Cell style={{ padding: 10 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'center' }}>Qty</Text></Cell>
            <Cell style={{ padding: 10 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Unit Price</Text></Cell>
            <Cell style={{ padding: 10 }}><Text style={{ fontSize: 9, fontWeight: 700, color: '#ffffff', textAlign: 'right' }}>Amount</Text></Cell>
          </Row>
          {items.map((item: any, i: number) => (
            <Row key={i} style={{ backgroundColor: i % 2 === 0 ? '#ffffff' : '#f8fafc' }}>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#1e293b' }}>{item.description}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#475569', textAlign: 'center' }}>{item.quantity}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#475569', textAlign: 'right' }}>${item.unitPrice.toFixed(2)}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#1e293b', textAlign: 'right' }}>${(item.quantity * item.unitPrice).toFixed(2)}</Text>
              </Cell>
            </Row>
          ))}
        </Table>

        {/* Totals */}
        <View style={{ flexDirection: 'row', justifyContent: 'flex-end', marginTop: 16 }}>
          <View style={{ width: 200 }}>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 8 }}>
              <Text style={{ fontSize: 9, color: '#64748b' }}>Subtotal</Text>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>${subtotal.toFixed(2)}</Text>
            </View>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 8 }}>
              <Text style={{ fontSize: 9, color: '#64748b' }}>Tax ({(data.taxRate * 100).toFixed(0)}%)</Text>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>${tax.toFixed(2)}</Text>
            </View>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 12, backgroundColor: '#2563eb', borderRadius: 4, marginTop: 4 }}>
              <Text style={{ fontSize: 11, fontWeight: 700, color: '#ffffff' }}>Total Due</Text>
              <Text style={{ fontSize: 11, fontWeight: 700, color: '#ffffff' }}>${total.toFixed(2)}</Text>
            </View>
          </View>
        </View>

        {/* Payment Terms */}
        <View style={{ marginTop: 32, padding: 16, backgroundColor: '#f8fafc', borderRadius: 4 }}>
          <Text style={{ fontSize: 9, fontWeight: 700, color: '#1e293b', marginBottom: 8 }}>Payment Terms</Text>
          <Text style={{ fontSize: 9, color: '#64748b' }}>{data.paymentTerms}</Text>
        </View>

        {/* Notes */}
        {data.notes && (
          <View style={{ marginTop: 16 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#1e293b', marginBottom: 4 }}>Notes</Text>
            <Text style={{ fontSize: 9, color: '#64748b' }}>{data.notes}</Text>
          </View>
        )}
      </Page>
    </Document>
  );
}
