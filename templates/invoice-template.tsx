import { Document, Page, View, Text, Table, Row, Cell, Fixed, expr } from '@formepdf/react';

/**
 * Template-friendly invoice. All computed values (subtotal, tax, total,
 * per-item amounts) are expected in the data rather than calculated here.
 *
 * Compile: forme build --template templates/invoice-template.tsx -o invoice.template.json
 *
 * Expected data shape:
 * {
 *   invoiceNumber: "INV-001",
 *   date: "2026-01-15",
 *   dueDate: "2026-02-15",
 *   company: { name, initials, address, cityStateZip, email },
 *   billTo: { name, company, address, cityStateZip, email },
 *   shipTo: { name, address, cityStateZip },
 *   items: [{ description, quantity, unitPrice, amount }],
 *   subtotal: "$3,500.00",
 *   taxLabel: "Tax (8%)",
 *   tax: "$280.00",
 *   total: "$3,780.00",
 *   paymentTerms: "Net 30",
 *   notes: "Optional notes" | null
 * }
 */
export default function Invoice(data: any) {
  return (
    <Document title={`Invoice ${data.invoiceNumber}`} author={data.company.name}>
      <Page size="Letter" margin={48}>
        <Fixed position="footer">
          <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 8, borderTopWidth: 1, borderColor: '#e2e8f0' }}>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>{data.company.name}</Text>
            <Text style={{ fontSize: 8, color: '#94a3b8' }}>Page {'{{pageNumber}}'} of {'{{totalPages}}'}</Text>
          </View>
        </Fixed>

        {/* Company Header */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginBottom: 32 }}>
          <View>
            <View style={{ width: 48, height: 48, backgroundColor: '#2563eb', borderRadius: 8, marginBottom: 12, justifyContent: 'center', alignItems: 'center' }}>
              <Text style={{ fontSize: 18, fontWeight: 700, color: '#ffffff', textAlign: 'center', lineHeight: 1.2 }}>{data.company.initials}</Text>
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
          {data.items.map((item: any) => (
            <Row>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#1e293b' }}>{item.description}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#475569', textAlign: 'center' }}>{item.quantity}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#475569', textAlign: 'right' }}>{item.unitPrice}</Text>
              </Cell>
              <Cell style={{ padding: 10 }}>
                <Text style={{ fontSize: 9, color: '#1e293b', textAlign: 'right' }}>{item.amount}</Text>
              </Cell>
            </Row>
          ))}
        </Table>

        {/* Totals */}
        <View style={{ flexDirection: 'row', justifyContent: 'flex-end', marginTop: 16 }}>
          <View style={{ width: 200 }}>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 8 }}>
              <Text style={{ fontSize: 9, color: '#64748b' }}>Subtotal</Text>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>{data.subtotal}</Text>
            </View>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 8 }}>
              <Text style={{ fontSize: 9, color: '#64748b' }}>{data.taxLabel}</Text>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>{data.tax}</Text>
            </View>
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 12, backgroundColor: '#2563eb', borderRadius: 4, marginTop: 4 }}>
              <Text style={{ fontSize: 11, fontWeight: 700, color: '#ffffff' }}>Total Due</Text>
              <Text style={{ fontSize: 11, fontWeight: 700, color: '#ffffff' }}>{data.total}</Text>
            </View>
          </View>
        </View>

        {/* Payment Terms */}
        <View style={{ marginTop: 32, padding: 16, backgroundColor: '#f8fafc', borderRadius: 4 }}>
          <Text style={{ fontSize: 9, fontWeight: 700, color: '#1e293b', marginBottom: 8 }}>Payment Terms</Text>
          <Text style={{ fontSize: 9, color: '#64748b' }}>{data.paymentTerms}</Text>
        </View>

        {/* Notes */}
        {expr.if(
          data.notes,
          <View style={{ marginTop: 16 }}>
            <Text style={{ fontSize: 9, fontWeight: 700, color: '#1e293b', marginBottom: 4 }}>Notes</Text>
            <Text style={{ fontSize: 9, color: '#64748b' }}>{data.notes}</Text>
          </View>
        ) as any}
      </Page>
    </Document>
  );
}
