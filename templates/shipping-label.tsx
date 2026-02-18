import { Document, Page, View, Text } from '@forme/react';

export default function ShippingLabel(data: any) {
  return (
    <Document title={`Shipping Label - ${data.tracking}`}>
      <Page size={{ width: 288, height: 432 }} margin={16}>
        {/* From Address */}
        <View style={{ marginBottom: 12, padding: 8 }}>
          <Text style={{ fontSize: 7, fontWeight: 700, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 4 }}>From</Text>
          <Text style={{ fontSize: 8, color: '#334155' }}>{data.from.name}</Text>
          <Text style={{ fontSize: 8, color: '#334155' }}>{data.from.address}</Text>
          <Text style={{ fontSize: 8, color: '#334155' }}>{data.from.cityStateZip}</Text>
        </View>

        {/* Divider */}
        <View style={{ borderWidth: { top: 2, right: 0, bottom: 0, left: 0 }, borderColor: '#0f172a', marginBottom: 12 }} />

        {/* To Address */}
        <View style={{ padding: 12, marginBottom: 12 }}>
          <Text style={{ fontSize: 7, fontWeight: 700, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1, marginBottom: 8 }}>To</Text>
          <Text style={{ fontSize: 14, fontWeight: 700, color: '#0f172a' }}>{data.to.name}</Text>
          <Text style={{ fontSize: 12, color: '#0f172a', marginTop: 4 }}>{data.to.address}</Text>
          {data.to.address2 && (
            <Text style={{ fontSize: 12, color: '#0f172a' }}>{data.to.address2}</Text>
          )}
          <Text style={{ fontSize: 12, fontWeight: 700, color: '#0f172a', marginTop: 2 }}>{data.to.cityStateZip}</Text>
        </View>

        {/* Barcode Placeholder */}
        <View style={{ marginBottom: 8, padding: 8 }}>
          <View style={{ alignItems: 'center', marginBottom: 8 }}>
            <View style={{ flexDirection: 'row', gap: 2 }}>
              {/* Simulate barcode with thin rectangles */}
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 2, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 1, height: 40, backgroundColor: '#0f172a' }} />
              <View style={{ width: 3, height: 40, backgroundColor: '#0f172a' }} />
            </View>
          </View>
          <Text style={{ fontSize: 10, fontWeight: 700, color: '#0f172a', letterSpacing: 2, textAlign: 'center' }}>{data.tracking}</Text>
        </View>

        {/* Divider */}
        <View style={{ borderWidth: { top: 1, right: 0, bottom: 0, left: 0 }, borderColor: '#cbd5e1', marginBottom: 8 }} />

        {/* Details Row */}
        <View style={{ flexDirection: 'row', justifyContent: 'space-between', padding: 8 }}>
          <View>
            <Text style={{ fontSize: 7, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1 }}>Weight</Text>
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#0f172a', marginTop: 2 }}>{data.weight}</Text>
          </View>
          <View>
            <Text style={{ fontSize: 7, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1 }}>Dimensions</Text>
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#0f172a', marginTop: 2 }}>{data.dimensions}</Text>
          </View>
          <View>
            <Text style={{ fontSize: 7, color: '#64748b', textTransform: 'uppercase', letterSpacing: 1 }}>Service</Text>
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#0f172a', marginTop: 2 }}>{data.service}</Text>
          </View>
        </View>

        {/* Stamps */}
        {data.stamps && data.stamps.length > 0 && (
          <View style={{ flexDirection: 'row', gap: 8, marginTop: 8, padding: 8 }}>
            {data.stamps.map((stamp: string, i: number) => (
              <View key={i} style={{ padding: { top: 6, right: 12, bottom: 6, left: 12 }, borderWidth: 2, borderColor: '#dc2626', borderRadius: 2 }}>
                <Text style={{ fontSize: 10, fontWeight: 700, color: '#dc2626', textTransform: 'uppercase' }}>{stamp}</Text>
              </View>
            ))}
          </View>
        )}
      </Page>
    </Document>
  );
}
