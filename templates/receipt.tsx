import { Document, Page, View, Text } from '@forme/react';

export default function Receipt(data: any) {
  const items = data.items || [];
  const subtotal = items.reduce((sum: number, item: any) => sum + item.price * (item.quantity || 1), 0);
  const tax = subtotal * (data.taxRate || 0);
  const total = subtotal + tax;

  return (
    <Document title={`Receipt ${data.receiptNumber}`} author={data.store.name}>
      <Page size="Letter" margin={{ top: 72, right: 120, bottom: 72, left: 120 }}>
        {/* Store Header */}
        <View style={{ alignItems: 'center', marginBottom: 24 }}>
          <Text style={{ fontSize: 20, fontWeight: 700, color: '#1e293b' }}>{data.store.name}</Text>
          <Text style={{ fontSize: 9, color: '#64748b', marginTop: 4 }}>{data.store.address}</Text>
          <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.store.cityStateZip}</Text>
          <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.store.phone}</Text>
        </View>

        {/* Divider */}
        <View style={{ borderTopWidth: 1, borderColor: '#e2e8f0', marginBottom: 16 }} />

        {/* Receipt Info */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginBottom: 16 }}>
          <Text style={{ fontSize: 9, color: '#64748b' }}>Receipt #{data.receiptNumber}</Text>
          <Text style={{ fontSize: 9, color: '#64748b' }}>{data.date}</Text>
        </View>

        {/* Items */}
        {items.map((item: any, i: number) => (
          <View key={i} style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 6, paddingBottom: 6 }}>
            <View style={{ flexDirection: 'row', gap: 8, flexGrow: 1 }}>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>{item.name}</Text>
              {(item.quantity || 1) > 1 && (
                <Text style={{ fontSize: 9, color: '#94a3b8' }}>x{item.quantity}</Text>
              )}
            </View>
            <Text style={{ fontSize: 9, color: '#1e293b' }}>${(item.price * (item.quantity || 1)).toFixed(2)}</Text>
          </View>
        ))}

        {/* Divider */}
        <View style={{ borderTopWidth: 1, borderColor: '#e2e8f0', marginTop: 12, marginBottom: 12 }} />

        {/* Subtotal */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 4, paddingBottom: 4 }}>
          <Text style={{ fontSize: 9, color: '#64748b' }}>Subtotal</Text>
          <Text style={{ fontSize: 9, color: '#1e293b' }}>${subtotal.toFixed(2)}</Text>
        </View>

        {/* Tax */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 4, paddingBottom: 4 }}>
          <Text style={{ fontSize: 9, color: '#64748b' }}>Tax ({(data.taxRate * 100).toFixed(1)}%)</Text>
          <Text style={{ fontSize: 9, color: '#1e293b' }}>${tax.toFixed(2)}</Text>
        </View>

        {/* Total */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', paddingTop: 12, paddingBottom: 12, borderTopWidth: 2, borderColor: '#1e293b', marginTop: 4 }}>
          <Text style={{ fontSize: 12, fontWeight: 700, color: '#1e293b' }}>Total</Text>
          <Text style={{ fontSize: 12, fontWeight: 700, color: '#1e293b' }}>${total.toFixed(2)}</Text>
        </View>

        {/* Payment Method */}
        <View style={{ marginTop: 16, paddingTop: 12, paddingBottom: 12, borderTopWidth: 1, borderColor: '#e2e8f0' }}>
          <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
            <Text style={{ fontSize: 9, color: '#64748b' }}>Payment Method</Text>
            <Text style={{ fontSize: 9, color: '#1e293b' }}>{data.paymentMethod}</Text>
          </View>
          {data.cardLastFour && (
            <View style={{ flexDirection: 'row', justifyContent: 'space-between', marginTop: 4 }}>
              <Text style={{ fontSize: 9, color: '#64748b' }}>Card</Text>
              <Text style={{ fontSize: 9, color: '#1e293b' }}>****{data.cardLastFour}</Text>
            </View>
          )}
        </View>

        {/* Thank You */}
        <View style={{ alignItems: 'center', marginTop: 32 }}>
          <Text style={{ fontSize: 10, color: '#64748b' }}>Thank you for your purchase!</Text>
          <Text style={{ fontSize: 8, color: '#94a3b8', marginTop: 8 }}>{data.store.website}</Text>
        </View>
      </Page>
    </Document>
  );
}
