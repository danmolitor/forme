import { Document, Page, View, Text, QrCode } from '@formepdf/react';

export default function EventTicket(data: any) {
  return (
    <Document title={`Event Ticket - ${data.ticketNumber}`}>
      <Page size={{ width: 612, height: 288 }} margin={24}>
        <View style={{ flexDirection: 'row' }}>
          {/* Main Content */}
          <View style={{ flex: 1, paddingRight: 20 }}>
            {/* Presenter */}
            <Text style={{ fontSize: 8, fontWeight: 700, color: '#2563eb', textTransform: 'uppercase', letterSpacing: 1.5, marginBottom: 4, fontFamily: 'Inter, Helvetica' }}>
              {data.presenter}
            </Text>

            {/* Event Name — ellipsis overflow */}
            <Text style={{ fontSize: 16, fontWeight: 700, color: '#0f172a', lineHeight: 1.2, marginBottom: 3, textOverflow: 'ellipsis', fontFamily: 'Inter, Helvetica' }}>
              {data.eventName}
            </Text>

            {/* Venue — ellipsis overflow */}
            <Text style={{ fontSize: 9, color: '#475569', marginBottom: 4, textOverflow: 'ellipsis', fontFamily: 'Inter, Helvetica' }}>
              {data.venue}
            </Text>

            {/* Date & Time */}
            <Text style={{ fontSize: 10, fontWeight: 700, color: '#1e293b', marginBottom: 14, fontFamily: 'Inter, Helvetica' }}>
              {data.date} · {data.time}
            </Text>

            {/* Seat Details Grid — repeat(4, 1fr) */}
            <View style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 1fr)', gap: 0, marginBottom: 10 }}>
              <View style={{ backgroundColor: '#1e293b', padding: 5 }}>
                <Text style={{ fontSize: 7, fontWeight: 700, color: '#ffffff', textTransform: 'uppercase', letterSpacing: 0.5, fontFamily: 'Inter, Helvetica' }}>Section</Text>
              </View>
              <View style={{ backgroundColor: '#1e293b', padding: 5 }}>
                <Text style={{ fontSize: 7, fontWeight: 700, color: '#ffffff', textTransform: 'uppercase', letterSpacing: 0.5, fontFamily: 'Inter, Helvetica' }}>Row</Text>
              </View>
              <View style={{ backgroundColor: '#1e293b', padding: 5 }}>
                <Text style={{ fontSize: 7, fontWeight: 700, color: '#ffffff', textTransform: 'uppercase', letterSpacing: 0.5, fontFamily: 'Inter, Helvetica' }}>Seat</Text>
              </View>
              <View style={{ backgroundColor: '#1e293b', padding: 5 }}>
                <Text style={{ fontSize: 7, fontWeight: 700, color: '#ffffff', textTransform: 'uppercase', letterSpacing: 0.5, fontFamily: 'Inter, Helvetica' }}>Gate</Text>
              </View>

              <View style={{ backgroundColor: '#f8fafc', padding: 5, borderBottomWidth: 1, borderColor: '#e2e8f0' }}>
                <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', fontFamily: 'Inter, Helvetica' }}>{data.section}</Text>
              </View>
              <View style={{ backgroundColor: '#f8fafc', padding: 5, borderBottomWidth: 1, borderColor: '#e2e8f0' }}>
                <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', fontFamily: 'Inter, Helvetica' }}>{data.row}</Text>
              </View>
              <View style={{ backgroundColor: '#f8fafc', padding: 5, borderBottomWidth: 1, borderColor: '#e2e8f0' }}>
                <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', fontFamily: 'Inter, Helvetica' }}>{data.seat}</Text>
              </View>
              <View style={{ backgroundColor: '#f8fafc', padding: 5, borderBottomWidth: 1, borderColor: '#e2e8f0' }}>
                <Text style={{ fontSize: 11, fontWeight: 700, color: '#0f172a', fontFamily: 'Inter, Helvetica' }}>{data.gate}</Text>
              </View>
            </View>

            {/* Admission & Policy */}
            <Text style={{ fontSize: 8, color: '#64748b', fontFamily: 'Inter, Helvetica' }}>
              {data.admissionType} · Doors open {data.doorsOpen}
            </Text>
            <Text style={{ fontSize: 7, color: '#94a3b8', marginTop: 2, fontFamily: 'Inter, Helvetica' }}>
              {data.policy}
            </Text>
          </View>

          {/* Divider */}
          <View style={{ width: 1, backgroundColor: '#e2e8f0' }} />

          {/* QR Stub */}
          <View style={{ width: 130, alignItems: 'center', justifyContent: 'center', paddingLeft: 16 }}>
            <QrCode data={data.ticketUrl} size={76} />

            <Text style={{ fontSize: 8, fontWeight: 700, color: '#0f172a', marginTop: 10, fontFamily: 'Inter, Helvetica' }}>
              {data.ticketNumber}
            </Text>

            <View style={{ marginTop: 8, paddingVertical: 3, paddingHorizontal: 6, backgroundColor: '#f1f5f9', borderRadius: 3 }}>
              <Text style={{ fontSize: 7, fontWeight: 700, color: '#64748b', textTransform: 'uppercase', letterSpacing: 0.5, fontFamily: 'Inter, Helvetica' }}>
                Scan for entry
              </Text>
            </View>
          </View>
        </View>
      </Page>
    </Document>
  );
}
