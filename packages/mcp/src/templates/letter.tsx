import { Document, Page, View, Text } from '@formepdf/react';
import type { LetterData } from '../schemas/letter.js';

export default function Letter(data: LetterData) {
  return (
    <Document title={`Letter to ${data.recipient.name}`} author={data.sender.name}>
      <Page size="Letter" margin={{ top: 72, right: 72, bottom: 72, left: 72 }}>
        {/* Letterhead */}
        <View style={{ marginBottom: 32 }}>
          <Text style={{ fontSize: 16, fontWeight: 700, color: '#1e293b' }}>{data.sender.company}</Text>
          <View style={{ borderTopWidth: 2, borderColor: '#2563eb', marginTop: 8, marginBottom: 12 }} />
          <View style={{ flexDirection: 'row', justifyContent: 'space-between' }}>
            <View>
              <Text style={{ fontSize: 9, color: '#64748b' }}>{data.sender.address}</Text>
              <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.sender.cityStateZip}</Text>
            </View>
            <View style={{ alignItems: 'flex-end' }}>
              {data.sender.phone && (
                <Text style={{ fontSize: 9, color: '#64748b' }}>{data.sender.phone}</Text>
              )}
              {data.sender.email && (
                <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.sender.email}</Text>
              )}
            </View>
          </View>
        </View>

        {/* Date */}
        <Text style={{ fontSize: 10, color: '#1e293b', marginBottom: 24 }}>{data.date}</Text>

        {/* Recipient Address */}
        <View style={{ marginBottom: 24 }}>
          <Text style={{ fontSize: 10, color: '#1e293b' }}>{data.recipient.name}</Text>
          {data.recipient.title && (
            <Text style={{ fontSize: 10, color: '#1e293b', marginTop: 2 }}>{data.recipient.title}</Text>
          )}
          {data.recipient.company && (
            <Text style={{ fontSize: 10, color: '#1e293b', marginTop: 2 }}>{data.recipient.company}</Text>
          )}
          <Text style={{ fontSize: 10, color: '#1e293b', marginTop: 2 }}>{data.recipient.address}</Text>
          <Text style={{ fontSize: 10, color: '#1e293b', marginTop: 2 }}>{data.recipient.cityStateZip}</Text>
        </View>

        {/* Salutation */}
        <Text style={{ fontSize: 10, color: '#1e293b', marginBottom: 16 }}>{data.salutation}</Text>

        {/* Body */}
        {data.body.map((paragraph, i) => (
          <Text key={i} style={{ fontSize: 10, color: '#334155', lineHeight: 1.6, marginBottom: 12 }}>{paragraph}</Text>
        ))}

        {/* Closing */}
        <View style={{ marginTop: 16 }}>
          <Text style={{ fontSize: 10, color: '#1e293b' }}>{data.closing}</Text>
          <View style={{ height: 48 }} />
          <Text style={{ fontSize: 10, fontWeight: 700, color: '#1e293b' }}>{data.signatureName}</Text>
          {data.signatureTitle && (
            <Text style={{ fontSize: 9, color: '#64748b', marginTop: 2 }}>{data.signatureTitle}</Text>
          )}
        </View>
      </Page>
    </Document>
  );
}
